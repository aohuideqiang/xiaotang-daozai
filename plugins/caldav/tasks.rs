//! 今日个人日历：并入待办页展示，并在插件自己的窗口里新建、修改、删除。
//!
//! 完成状态只保存在本机（`calendar-completed.json`），不会写回日历；远端改动
//! 通过 5 分钟一次的静默刷新同步过来。
use super::write::{self, Item};
use crate::calendar::{self, Config};
use crate::local_calendar::{Draft, Entry};
use eframe::egui;
use std::collections::BTreeSet;
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::{Duration, Instant};

/// 待办页要画的一行。
pub(crate) struct Row {
    pub key: String,
    pub title: String,
    pub at: String,
    pub done: bool,
    pub readonly: bool,
}

enum Reply {
    List(Result<Vec<Item>, String>),
    Saved(Result<(), String>),
}

struct State {
    items: Vec<Item>,
    done: BTreeSet<String>,
    error: String,
    completion_error: String,
    draft: Option<Draft>,
    selected: Option<Item>,
    account: usize,
    day: chrono::NaiveDate,
    configs: Vec<Config>,
    rx: Option<mpsc::Receiver<Reply>>,
    next: Instant,
    config_checked: Instant,
}

static STATE: OnceLock<Mutex<State>> = OnceLock::new();

fn state() -> &'static Mutex<State> {
    STATE.get_or_init(|| Mutex::new(State::load()))
}

/// 每帧调用：先处理后台结果，需要时刷新今日日程；编辑窗口打开时再画表单。
pub(crate) fn frame(ctx: &egui::Context) {
    let Ok(mut state) = state().lock() else { return; };
    state.poll();
    if state.rx.is_some() { ctx.request_repaint_after(Duration::from_millis(500)); }
    if state.draft.is_some() { draw_form(ctx, &mut state); }
}

/// 待办页使用的快照；只复制要显示的行，不泄漏内部状态。
pub(crate) fn rows() -> Vec<Row> {
    let Ok(state) = state().lock() else { return Vec::new(); };
    state.items.iter().map(|item| {
        let key = State::key(item);
        Row {
            done: state.done.contains(&key),
            readonly: item.readonly,
            title: item.event.title.replace(['\r', '\n'], " "),
            at: item.event.at.clone(),
            key,
        }
    }).collect()
}

pub(crate) fn count() -> usize {
    state().lock().map(|state| state.items.len()).unwrap_or(0)
}

/// 最近一次同步或写入的提示；没有时返回空串。
pub(crate) fn last_error() -> String {
    state().lock().map(|state| state.error.clone()).unwrap_or_default()
}

pub(crate) fn done_count() -> usize {
    let Ok(state) = state().lock() else { return 0; };
    state.items.iter().filter(|item| state.done.contains(&State::key(item))).count()
}

/// 勾选只标记本机完成，不改动远端日历。
pub(crate) fn toggle(key: &str) {
    if let Ok(mut state) = state().lock() { state.toggle(key); }
}

/// 打开编辑窗口；`None` 表示新建。
pub(crate) fn open_editor(key: Option<&str>) {
    if let Ok(mut state) = state().lock() { state.edit(key); }
}

/// 新建日程的入口：先刷新一次今日日程，避免列表是空的。
pub(crate) fn open_new() {
    if let Ok(mut state) = state().lock() {
        if state.configs.is_empty() { state.refresh_configs(); }
        state.edit(None);
    }
}

/// 是否已连接个人（自建）CalDAV 日历；日历页据此决定要不要显示「写日历」按钮。
pub(crate) fn has_personal() -> bool {
    crate::calendar::load_configs().iter().any(|config| config.provider == crate::calendar::Provider::Custom)
}

impl State {
    fn load() -> Self {
        let (done, completion_error) = match std::fs::read(crate::model::config_dir().join("calendar-completed.json")) {
            Ok(raw) => match serde_json::from_slice(&raw) {
                Ok(done) => (done, String::new()),
                Err(_) => (BTreeSet::new(), "本机完成状态文件无法读取，已暂停修改，请检查文件".into()),
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (BTreeSet::new(), String::new()),
            Err(_) => (BTreeSet::new(), "无法读取本机完成状态".into()),
        };
        Self {
            items: Vec::new(),
            done,
            error: String::new(),
            completion_error,
            draft: None,
            selected: None,
            account: 0,
            day: chrono::Local::now().date_naive(),
            configs: Vec::new(),
            rx: None,
            next: Instant::now(),
            config_checked: Instant::now() - Duration::from_secs(10),
        }
    }

    fn key(item: &Item) -> String {
        format!("{}|{}", item.config.credential_name(), calendar::dedupe_key(&item.event))
    }

    fn refresh_configs(&mut self) {
        self.config_checked = Instant::now();
        let configs: Vec<Config> = calendar::load_configs().into_iter()
            .filter(|config| config.provider == calendar::Provider::Custom)
            .collect();
        if configs != self.configs {
            self.configs = configs;
            self.account = 0;
            self.next = Instant::now();
        }
    }

    fn poll(&mut self) {
        // 配置和日期变化时立即刷新；平时每 5 分钟同步一次。
        if self.config_checked.elapsed() >= Duration::from_secs(2) { self.refresh_configs(); }
        let today = chrono::Local::now().date_naive();
        if self.day != today {
            self.day = today;
            self.items.clear();
            self.next = Instant::now();
        }
        if let Some(reply) = self.rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.rx = None;
            match reply {
                Reply::List(Ok(items)) => { self.items = items; self.error.clear(); }
                Reply::List(Err(error)) => self.error = error,
                Reply::Saved(Ok(())) => {
                    self.draft = None;
                    self.selected = None;
                    self.error = "日历已同步，正在刷新".into();
                    self.next = Instant::now();
                }
                Reply::Saved(Err(error)) => self.error = error,
            }
        }
        if self.rx.is_none() && Instant::now() >= self.next {
            self.next = Instant::now() + Duration::from_secs(300);
            if self.configs.is_empty() { return; }
            let configs = self.configs.clone();
            let today = self.day;
            let (tx, rx) = mpsc::channel();
            self.rx = Some(rx);
            std::thread::spawn(move || {
                let result = (|| {
                    let mut items = Vec::new();
                    for config in configs {
                        let password = calendar::secrets::load(&config)?;
                        items.extend(write::list(&config, &password, today)?);
                    }
                    Ok(items)
                })();
                let _ = tx.send(Reply::List(result));
            });
        }
    }

    fn toggle(&mut self, key: &str) {
        if !self.completion_error.is_empty() {
            self.error = self.completion_error.clone();
            return;
        }
        let mut done = self.done.clone();
        if !done.remove(key) { done.insert(key.to_owned()); }
        let path = crate::model::config_dir().join("calendar-completed.json");
        match serde_json::to_vec(&done).map_err(|error| error.to_string())
            .and_then(|raw| crate::model::atomic_write(&path, &raw).map_err(|error| error.to_string())) {
            Ok(()) => self.done = done,
            Err(_) => self.error = "本机完成状态保存失败".into(),
        }
    }

    fn edit(&mut self, key: Option<&str>) {
        self.selected = key.and_then(|key| self.items.iter().find(|item| State::key(item) == key)).cloned();
        if self.selected.as_ref().is_some_and(|item| item.readonly) {
            self.error = "重复日程或缺少版本信息的日程只可展示".into();
            return;
        }
        self.draft = Some(match &self.selected {
            Some(item) => Draft::edit(&Entry { event: item.event.clone(), reminder: None }),
            None => Draft::new(chrono::Local::now().date_naive()),
        });
        self.error.clear();
    }

    fn save(&mut self, delete: bool) {
        if self.rx.is_some() { return; }
        let Some(draft) = &self.draft else { return; };
        let event = if delete {
            match &self.selected { Some(item) => item.event.clone(), None => return }
        } else {
            match draft.entry() {
                Ok(entry) => entry.event,
                Err(error) => { self.error = error; return; }
            }
        };
        let Some(config) = self.selected.as_ref().map(|item| item.config.clone()).or_else(|| self.configs.get(self.account).cloned()) else {
            self.error = "请先在设置里连接个人 CalDAV 日历".into();
            return;
        };
        let old = self.selected.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        self.error = "正在同步，请稍候…".into();
        std::thread::spawn(move || {
            let result = calendar::secrets::load(&config)
                .and_then(|password| write::save(&config, &password, &event, old.as_ref(), delete));
            let _ = tx.send(Reply::Saved(result));
        });
    }
}

fn draw_form(ctx: &egui::Context, state: &mut State) {
    let mut open = true;
    let (mut save, mut delete, mut cancel) = (false, false, false);
    let title = if state.selected.is_some() { "编辑个人日历日程" } else { "新建个人日历日程" };
    let builder = egui::ViewportBuilder::default()
        .with_title(title)
        .with_inner_size([430.0, 480.0])
        .with_min_inner_size([400.0, 420.0]);
    ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("calendar-task-form"), builder, |ctx, _class| {
        if ctx.input(|input| input.viewport().close_requested()) { open = false; }
        let original = ctx.style();
        ctx.set_style(crate::plugins::ui::island_style(&original, true));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(crate::plugins::ui::WINDOW_FILL).inner_margin(22))
            .show(ctx, |ui| {
                ui.heading(if state.selected.is_some() { "编辑日程" } else { "加一个" });
                ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
                ui.add_enabled_ui(state.rx.is_none(), |ui| {
                    if state.selected.is_none() {
                        egui::ComboBox::from_id_salt("personal-calendar-account")
                            .selected_text(state.configs.get(state.account).map(|config| config.username.as_str()).unwrap_or("尚未连接个人日历"))
                            .show_ui(ui, |ui| {
                                for (index, config) in state.configs.iter().enumerate() {
                                    ui.selectable_value(&mut state.account, index, &config.username);
                                }
                            });
                    }
                    let Some(draft) = state.draft.as_mut() else { return; };
                    ui.label("标题");
                    ui.add(egui::TextEdit::singleline(&mut draft.title).desired_width(330.0));
                    ui.label("开始时间 · 年-月-日 / 时:分");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut draft.start_date).desired_width(150.0));
                        ui.add(egui::TextEdit::singleline(&mut draft.start_time).desired_width(80.0));
                    });
                    ui.label("结束时间 · 年-月-日 / 时:分");
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut draft.end_date).desired_width(150.0));
                        ui.add(egui::TextEdit::singleline(&mut draft.end_time).desired_width(80.0));
                    });
                    ui.checkbox(&mut draft.all_day, "全天日程");
                    ui.weak("保存会同步到已连接的日历；勾选完成只存在本机。");
                    if state.selected.is_some() {
                        if draft.confirm_delete {
                            ui.colored_label(egui::Color32::LIGHT_RED, "确认删除？这会同步删除日历里的日程。");
                            ui.horizontal(|ui| {
                                delete = ui.button("确认同步删除").clicked();
                                if ui.button("保留日程").clicked() { draft.confirm_delete = false; }
                            });
                        } else if ui.button("删除日程…").clicked() {
                            draft.confirm_delete = true;
                        }
                    }
                    ui.horizontal(|ui| {
                        save = ui.button("保存到日历").clicked();
                        cancel = ui.button("取消").clicked();
                    });
                });
                if !state.error.is_empty() {
                    ui.colored_label(egui::Color32::from_rgb(245, 175, 100), &state.error);
                }
            });
        ctx.set_style(original);
    });
    if save {
        state.save(false);
    } else if delete {
        state.save(true);
    } else if (!open || cancel) && state.rx.is_none() {
        state.draft = None;
        state.selected = None;
    }
}
