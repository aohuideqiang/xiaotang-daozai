//! 桌面工具插件：独立课表、下载、快捷启动、系统性能与服务器任务总览。
//!
//! 这些功能全部在插件自己的窗口里，岛体只保留一个入口；这样上游调整页面、
//! 页面数组和渲染顺序时都不需要改动这里。
mod config;
mod courses;
mod drop;
mod sampler;
mod window;

use serde_json::Value;
use std::sync::{Arc, Mutex, OnceLock};

pub(crate) struct Tools {
    pub open: bool,
    pub tab: usize,
    pub week: bool,
    pub offset: i64,
    pub config: config::Config,
    pub shared: Arc<Mutex<sampler::Snapshot>>,
    /// 采样线程的配置发送端；线程启动后才有值。
    tx: Option<std::sync::mpsc::Sender<config::Config>>,
    pub downloads: Vec<Arc<Mutex<sampler::Download>>>,
    pub url: String,
    pub label: String,
    pub target: String,
    pub note: String,
    pub courses: Value,
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
        }
    }
}

static TOOLS: OnceLock<Mutex<Tools>> = OnceLock::new();

fn tools() -> &'static Mutex<Tools> {
    TOOLS.get_or_init(|| Mutex::new(Tools::default()))
}

/// 每帧调用：先处理拖放进来的内容，再按需画工具窗口。
pub(crate) fn frame(ctx: &eframe::egui::Context, app: &mut crate::app::App) {
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
pub(crate) fn install_drop(hwnd: windows::Win32::Foundation::HWND, ctx: eframe::egui::Context) -> windows::core::Result<()> {
    drop::install(hwnd, ctx)
}

/// 打开工具窗口；`tab` 是 `window::TABS` 的下标。
pub fn open(tab: usize) {
    if let Ok(mut tools) = tools().lock() {
        tools.open = true;
        tools.tab = tab.min(5);
    }
}

/// 拖进一个链接：直接切到下载页并开始下载。
pub(crate) fn open_download(url: String) {
    if let Ok(mut tools) = tools().lock() {
        tools.open = true;
        tools.tab = 1;
        if tools.config.enabled[0] {
            tools.url = url;
            tools.start_download();
        } else {
            tools.note = "下载功能已关闭，请先在「设置」里打开".into();
        }
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
