//! 桌面工具页面：把课表、下载、快捷启动、系统性能和服务器任务放进岛体的翻页里。
//!
//! 上游页面数组是固定长度，插件页面接在最后（`app::PLUGIN_PAGE_BASE` 起），
//! 每页只在 `render_page` 里调一行；左右滑动和边缘滚轮按页面数循环，自然覆盖新页。

mod config;
mod courses;
mod drop;
mod sampler;
mod window;

use eframe::egui;
use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

/// 插件页面数量，与 `app::PAGE_NAMES` 尾部五项一致。
pub const PAGES: usize = 5;
/// 每页在设置里的说明，供「页面管理」显示。
pub const PAGE_LABELS: [(&str, &str); PAGES] = [
    ("课表", "独立课表：今明两天与完整一周，按学期周次和节假日排课"),
    ("下载", "Edge 正在下载的文件和岛仔自己的下载任务"),
    ("快捷启动", "常用应用、文件夹或网址，一键打开"),
    ("系统性能", "CPU 与内存占用，每 2 秒采样"),
    ("服务器任务", "通过 SSH 读取服务器容器与服务状态，每 30 秒刷新"),
];

pub(crate) struct Tools {
    pub open: bool,
    pub tab: usize,
    pub week: bool,
    pub offset: i64,
    pub config: config::Config,
    pub shared: Arc<Mutex<sampler::Snapshot>>,
    tx: Option<std::sync::mpsc::Sender<config::Config>>,
    pub downloads: Vec<Arc<Mutex<sampler::Download>>>,
    pub url: String,
    pub label: String,
    pub target: String,
    pub note: String,
    pub courses: Value,
    /// 服务器任务页的滚轮偏移（每次 3 行）。
    pub server_offset: usize,
    /// 岛体页面里正在输入下载地址时，请求键盘焦点。
    pub focus_url: bool,
}

impl Default for Tools {
    fn default() -> Self {
        Self {
            open: false,
            tab: 0,
            week: false,
            offset: 0,
            config: config::Config::load(),
            shared: Arc::default(),
            tx: None,
            downloads: Vec::new(),
            url: String::new(),
            label: String::new(),
            target: String::new(),
            note: String::new(),
            courses: Value::Null,
            server_offset: 0,
            focus_url: false,
        }
    }
}

static TOOLS: OnceLock<Mutex<Tools>> = OnceLock::new();

fn tools() -> &'static Mutex<Tools> {
    TOOLS.get_or_init(|| Mutex::new(Tools::default()))
}

/// 岛体每次绘制插件页时调用；`index` 是 0..PAGES（页面序号减 `PLUGIN_PAGE_BASE`）。
pub(crate) fn page(app: &mut crate::app::App, index: usize, c: &crate::paint::Card, env: &crate::app::Env) {
    let Ok(mut tools) = tools().lock() else { return; };
    tools.start();
    let snapshot = tools.shared.lock().map(|snapshot| snapshot.clone()).unwrap_or_default();
    match index {
        0 => window::page_courses(c, env, &mut tools),
        1 => window::page_downloads(c, &mut tools, &snapshot),
        2 => window::page_shortcuts(c, &mut tools),
        3 => window::page_performance(c, &mut tools, &snapshot),
        4 => window::page_server(c, &mut tools, &snapshot),
        _ => {}
    }
    // 拖链接进来的下载只有独立窗口能输入，岛体页给一个入口。
    if index == 1 && tools.focus_url {
        tools.focus_url = false;
        tools.open = true;
        tools.tab = 1;
    }
    let _ = app;
}

/// 每帧调用：处理拖放进来的内容，并在打开时画独立窗口。
pub(crate) fn frame(ctx: &egui::Context, app: &mut crate::app::App) {
    for payload in drop::take() {
        match payload {
            drop::Payload::Url(url) => open_download(url),
            drop::Payload::Files(paths) => {
                app.on_drop(paths);
                app.show_page(3);
            }
        }
    }
    let Ok(mut tools) = tools().lock() else { return; };
    tools.start();
    if tools.open { window::draw(ctx, &mut tools); }
}

/// 在岛体窗口上注册拖放目标；文件仍交给置物架。
pub(crate) fn install_drop(hwnd: windows::Win32::Foundation::HWND, ctx: egui::Context) -> windows::core::Result<()> {
    drop::install(hwnd, ctx)
}

/// 打开独立窗口（设置里的入口用）；`tab` 是窗口分页下标。
pub fn open(tab: usize) {
    if let Ok(mut tools) = tools().lock() {
        tools.open = true;
        tools.tab = tab.min(window::TABS.len() - 1);
    }
}

/// 拖进一个链接：切到「下载」页并打开下载窗口。
pub(crate) fn open_download(url: String) {
    if let Ok(mut tools) = tools().lock() {
        if !tools.config.enabled[0] {
            tools.note = "下载功能已关闭，请先在设置里打开".into();
            return;
        }
        tools.url = url;
        tools.open = true;
        tools.tab = 1;
        tools.start_download();
    }
}

/// 岛体「服务器任务」页用滚轮翻行：一次 3 行，返回是否滚动了。
pub(crate) fn scroll_server(next: bool) -> bool {
    match tools().lock() {
        Ok(mut tools) => window::scroll_server(&mut tools, next),
        Err(_) => false,
    }
}

impl Tools {
    fn start(&mut self) {
        if self.tx.is_some() { return; }
        self.ensure_scripts();
        self.tx = sampler::start(self.shared.clone(), self.config.clone());
    }

    /// 把随程序打包的读取脚本放到用户数据目录；用户自定义过路径时不覆盖。
    fn ensure_scripts(&mut self) {
        let mut dirty = false;
        let directory = crate::model::config_dir();
        let target = directory.join("server-overview.py");
        let bundled = include_bytes!("runtime/server-overview.py");
        let manages = self.config.bridge.trim().is_empty() || std::path::Path::new(self.config.bridge.trim()) == target;
        let stale = !std::fs::read(&target).is_ok_and(|current| current == bundled);
        if manages && (self.config.bridge.trim().is_empty() || stale) {
            let written = std::fs::create_dir_all(&directory).is_ok()
                && crate::model::atomic_write(&target, bundled).is_ok();
            if written {
                self.config.bridge = target.to_string_lossy().into_owned();
                dirty = true;
            }
        }
        if self.config.courses.is_empty() {
            let candidate = directory.join("courses.json");
            if candidate.exists() {
                self.config.courses = candidate.to_string_lossy().into_owned();
                dirty = true;
            }
        }
        if dirty { let _ = self.config.save(); }
    }

    fn save(&mut self) {
        self.note = match self.config.save() {
            Ok(()) => String::new(),
            Err(error) => format!("保存失败：{error}"),
        };
        if let Some(tx) = &self.tx { let _ = tx.send(self.config.clone()); }
    }

    fn read_courses(&mut self) {
        let configured = self.config.courses.trim();
        let path = if configured.is_empty() {
            crate::model::config_dir().join("courses.json")
        } else {
            std::path::PathBuf::from(configured)
        };
        self.courses = std::fs::read(&path).ok()
            .filter(|raw| raw.len() < 2 * 1024 * 1024)
            .and_then(|raw| serde_json::from_slice(&raw).ok())
            .unwrap_or(Value::Null);
    }
}
