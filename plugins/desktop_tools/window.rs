//! 桌面工具窗口：课表、下载、快捷启动、系统性能和服务器任务。
//!
//! 和岛体分开的独立窗口，避免把插件页面塞进上游固定的页面数组。
use super::sampler::download_progress;
use chrono::Datelike;
use eframe::egui::{self, Color32};

const TABS: [&str; 6] = ["课表", "下载", "快捷启动", "系统性能", "服务器任务", "设置"];

pub(crate) fn draw(ctx: &egui::Context, tools: &mut super::Tools) {
    let mut open = tools.open;
    let builder = egui::ViewportBuilder::default()
        .with_title("岛仔 · 桌面工具")
        .with_inner_size([if tools.week { 980.0 } else { 700.0 }, 560.0])
        .with_min_inner_size([620.0, 420.0]);
    ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("desktop-tools-window"), builder, |ctx, _class| {
        if ctx.input(|input| input.viewport().close_requested()) { open = false; }
        let original = ctx.style();
        ctx.set_style(crate::plugins::ui::island_style(&original, true));
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(crate::plugins::ui::WINDOW_FILL).inner_margin(20))
            .show(ctx, |ui| {
                ui.heading("桌面工具");
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    for (index, name) in TABS.iter().enumerate() {
                        if ui.selectable_label(tools.tab == index, *name).clicked() { tools.tab = index; }
                    }
                });
                ui.separator();
                let snapshot = tools.shared.lock().map(|snapshot| snapshot.clone()).unwrap_or_default();
                egui::ScrollArea::vertical().show(ui, |ui| match tools.tab {
                    0 => courses_tab(ui, tools, ctx),
                    1 => downloads_tab(ui, tools, &snapshot),
                    2 => shortcuts_tab(ui, tools),
                    3 => performance_tab(ui, tools, &snapshot),
                    4 => server_tab(ui, tools, &snapshot),
                    _ => settings_tab(ui, tools),
                });
                if !tools.note.is_empty() {
                    ui.add_space(6.0);
                    ui.colored_label(Color32::from_rgb(240, 170, 90), &tools.note);
                }
            });
        ctx.set_style(original);
    });
    tools.open = open;
}

fn courses_tab(ui: &mut egui::Ui, tools: &mut super::Tools, ctx: &egui::Context) {
    ui.horizontal(|ui| {
        let label = if tools.week { "‹ 两日预览" } else { "展开完整一周 ›" };
        if ui.button(label).clicked() {
            tools.week = !tools.week;
            tools.offset = 0;
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(if tools.week { 980.0 } else { 700.0 }, 560.0)));
        }
        if ui.small_button("上一周").clicked() { tools.offset -= 7; }
        if ui.small_button("今天").clicked() { tools.offset = 0; }
        if ui.small_button("下一周").clicked() { tools.offset += 7; }
        if ui.small_button("刷新课表").clicked() { tools.read_courses(); }
    });
    if tools.courses.is_null() {
        ui.label("请在「设置」里选择独立课表数据文件（JSON）。");
        return;
    }
    let today = chrono::Local::now().date_naive();
    let anchor = today + chrono::Duration::days(tools.offset);
    let start = if tools.week {
        anchor - chrono::Duration::days(anchor.weekday().num_days_from_monday() as i64)
    } else {
        anchor
    };
    let count = if tools.week { 7 } else { 2 };
    ui.add_space(10.0);
    ui.columns(count, |columns| {
        for (index, column) in columns.iter_mut().enumerate() {
            let date = start + chrono::Duration::days(index as i64);
            egui::Frame::group(column.style()).corner_radius(12.0).show(column, |ui| {
                let label = format!(
                    "{} {}",
                    ["周一", "周二", "周三", "周四", "周五", "周六", "周日"][date.weekday().num_days_from_monday() as usize],
                    date.format("%m/%d")
                );
                if ui.selectable_label(date == today, label).clicked() { tools.week = true; }
                let rows = super::courses::course_day(&tools.courses, date);
                if rows.is_empty() { ui.weak("当天无课"); }
                for row in rows {
                    ui.add_space(8.0);
                    ui.label(row);
                    ui.separator();
                }
            });
        }
    });
}

fn downloads_tab(ui: &mut egui::Ui, tools: &mut super::Tools, snapshot: &super::sampler::Snapshot) {
    ui.weak("Edge 正在下载的文件与岛仔自己的下载任务 · 可粘贴 http/https 地址");
    ui.add_enabled_ui(tools.config.enabled[0], |ui| {
        ui.add(egui::TextEdit::singleline(&mut tools.url).hint_text("https://…").desired_width(f32::INFINITY));
        if ui.button("开始下载到 Downloads").clicked() { tools.start_download(); }
    });
    if !snapshot.edge_error.is_empty() {
        ui.colored_label(Color32::LIGHT_RED, &snapshot.edge_error);
    }
    let own: Vec<super::sampler::Download> = tools.downloads.iter()
        .filter_map(|download| download.lock().ok().map(|download| download.clone()))
        .collect();
    let rows: Vec<(bool, super::sampler::Download)> = snapshot.edge.iter().cloned().map(|row| (true, row))
        .chain(own.into_iter().map(|row| (false, row)))
        .collect();
    if rows.is_empty() {
        ui.add_space(10.0);
        ui.weak("暂无下载任务");
    }
    for (from_edge, row) in rows {
        egui::Frame::group(ui.style()).corner_radius(10.0).show(ui, |ui| {
            let name = std::path::Path::new(&row.path).file_name().and_then(|name| name.to_str()).unwrap_or("下载文件");
            ui.add(egui::Label::new(format!("{} · {}", if from_edge { "Edge" } else { "岛仔" }, name)).truncate());
            if row.total > 0 {
                ui.add(egui::ProgressBar::new((row.bytes as f32 / row.total as f32).clamp(0.0, 1.0)).show_percentage());
            }
            ui.label(download_progress(&row));
            if row.status == "已完成" && ui.small_button("打开所在目录").clicked() {
                if let Some(parent) = std::path::Path::new(&row.path).parent() {
                    let _ = crate::plugins::ui::open_path(parent);
                }
            }
        });
    }
}

fn shortcuts_tab(ui: &mut egui::Ui, tools: &mut super::Tools) {
    if !tools.config.enabled[1] { ui.label("快捷启动已关闭，可在「设置」里重新打开。"); return; }
    ui.label("名称");
    ui.text_edit_singleline(&mut tools.label);
    ui.label("应用、文件夹路径或 https 网址");
    ui.text_edit_singleline(&mut tools.target);
    let can_add = !tools.label.trim().is_empty() && !tools.target.trim().is_empty() && tools.config.shortcuts.len() < 50;
    if ui.add_enabled(can_add, egui::Button::new("添加快捷入口")).clicked() {
        tools.config.shortcuts.push((tools.label.trim().into(), tools.target.trim().into()));
        tools.label.clear();
        tools.target.clear();
        tools.save();
    }
    let mut remove = None;
    for (index, (name, target)) in tools.config.shortcuts.iter().enumerate() {
        ui.horizontal(|ui| {
            if ui.button(name).clicked() { open_shortcut(target); }
            if ui.small_button("移除").clicked() { remove = Some(index); }
        });
    }
    if let Some(index) = remove {
        tools.config.shortcuts.remove(index);
        tools.save();
    }
}

fn performance_tab(ui: &mut egui::Ui, tools: &mut super::Tools, snapshot: &super::sampler::Snapshot) {
    if !tools.config.enabled[2] { ui.label("系统采样已关闭。"); return; }
    ui.heading(format!("CPU  {:.1}%", snapshot.cpu));
    ui.add(egui::ProgressBar::new(snapshot.cpu as f32 / 100.0));
    ui.heading(format!("内存  {:.1}%", snapshot.memory));
    ui.add(egui::ProgressBar::new(snapshot.memory as f32 / 100.0));
    ui.weak("每 2 秒采样一次；第一次 CPU 数值需要等下一次更新。");
}

fn server_tab(ui: &mut egui::Ui, tools: &mut super::Tools, snapshot: &super::sampler::Snapshot) {
    let connected = serde_json::from_str::<serde_json::Value>(&snapshot.tasks).ok()
        .and_then(|value| value["connected"].as_bool())
        .unwrap_or(true);
    ui.horizontal(|ui| {
        ui.label(format!("最近更新 {} · 每 30 秒同步", if snapshot.updated.is_empty() { "—" } else { &snapshot.updated }));
        if ui.button("刷新").clicked() { tools.save(); }
        if !connected {
            ui.colored_label(Color32::from_rgb(240, 95, 95), "· 连不上服务器");
        }
    });
    if !tools.config.enabled[3] {
        ui.label("服务器同步已关闭。");
        return;
    }
    let Ok(data) = serde_json::from_str::<serde_json::Value>(&snapshot.tasks) else {
        ui.label(if snapshot.tasks.is_empty() { "正在读取服务器状态…" } else { &snapshot.tasks });
        return;
    };
    egui::Grid::new("server-overview-table").striped(true).show(ui, |ui| {
        ui.strong("名称");
        ui.strong("用途");
        ui.strong("状态");
        ui.end_row();
        if let Some(rows) = data["services"].as_array() {
            for row in rows {
                ui.label(row["name"].as_str().unwrap_or("—"));
                ui.label(row["description"].as_str().unwrap_or("—"));
                let color = match row["state"].as_str() {
                    Some("ok") => Color32::from_rgb(71, 200, 120),
                    Some("down") => Color32::from_rgb(240, 95, 95),
                    _ => Color32::GRAY,
                };
                ui.colored_label(color, row["label"].as_str().unwrap_or("未知"));
                ui.end_row();
            }
        }
    });
}

fn settings_tab(ui: &mut egui::Ui, tools: &mut super::Tools) {
    let mut changed = false;
    for (index, label) in super::config::ENABLED_LABELS.iter().enumerate() {
        changed |= ui.checkbox(&mut tools.config.enabled[index], *label).changed();
    }
    ui.separator();
    ui.label("独立课表 JSON 文件");
    changed |= ui.text_edit_singleline(&mut tools.config.courses).changed();
    ui.add_space(6.0);
    ui.label(egui::RichText::new("服务器连接").strong());
    ui.add(egui::Label::new(egui::RichText::new("连不上时在这里改写，岛仔每 30 秒用新配置重试。").size(11.0)));
    ui.horizontal(|ui| {
        ui.label("主机");
        changed |= ui.add(egui::TextEdit::singleline(&mut tools.config.ssh_host).hint_text("117.72.148.76").desired_width(180.0)).changed();
        ui.label("端口");
        changed |= ui.add(egui::TextEdit::singleline(&mut tools.config.ssh_port).hint_text("22").desired_width(48.0)).changed();
    });
    ui.horizontal(|ui| {
        ui.label("用户名");
        changed |= ui.add(egui::TextEdit::singleline(&mut tools.config.ssh_user).hint_text("root").desired_width(180.0)).changed();
    });
    ui.label("SSH 密钥路径（留空则用 ssh 的默认密钥）");
    changed |= ui.add(egui::TextEdit::singleline(&mut tools.config.ssh_key).hint_text(r"C:\Users\你\.ssh\id.pem").desired_width(f32::INFINITY)).changed();
    if ui.button("保存并立即重试").clicked() {
        tools.note.clear();
        tools.config.write_connection();
        tools.save();
    }
    ui.separator();
    ui.label("服务器总览读取脚本（只运行你配置的本机 Python 文件）");
    changed |= ui.text_edit_singleline(&mut tools.config.bridge).changed();
    if changed {
        tools.courses = serde_json::Value::Null;
        tools.save();
    }
    ui.weak("桌面工具的连接配置保存在本机用户数据目录，不随仓库上传。");
}

/// 打开快捷入口：网址交给默认浏览器，其他交给系统关联程序。
fn open_shortcut(target: &str) {    if target.starts_with("https://") || target.starts_with("http://") {
        if let Some(path) = crate::mail::web_url(target) {
            let _ = crate::plugins::ui::open_path(std::path::Path::new(&path));
        }
        return;
    }
    let _ = crate::plugins::ui::open_path(std::path::Path::new(target));
}

impl super::Tools {
    pub(crate) fn start_download(&mut self) {
        let active = self.downloads.iter().filter(|task| task.lock().is_ok_and(|task| task.status == "下载中")).count();
        if active >= 3 {
            self.note = "最多同时下载 3 个任务".into();
            return;
        }
        let Ok(url) = url::Url::parse(self.url.trim()) else {
            self.note = "请输入完整的下载地址".into();
            return;
        };
        if !matches!(url.scheme(), "https" | "http") || !url.username().is_empty() || url.password().is_some() {
            self.note = "下载地址需要 http/https，且不能带账号密码".into();
            return;
        }
        let name = url.path_segments().and_then(|mut segments| segments.next_back()).filter(|name| !name.is_empty()).unwrap_or("download.bin");
        let name: String = name.chars().filter(|character| character.is_ascii_alphanumeric() || ".-_".contains(*character)).take(100).collect();
        let root = std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).unwrap_or_else(crate::model::config_dir).join("Downloads");
        let path = root.join(format!("daozai-{}-{}", chrono::Local::now().timestamp_millis(), if name.is_empty() { "download.bin" } else { &name }));
        let task = std::sync::Arc::new(std::sync::Mutex::new(super::sampler::Download {
            path: path.to_string_lossy().into(),
            status: "下载中".into(),
            ..Default::default()
        }));
        self.downloads.retain(|download| download.lock().is_ok_and(|download| download.status == "下载中"));
        self.downloads.push(task.clone());
        let url = url.to_string();
        let target = path.clone();
        std::thread::spawn(move || {
            let _ = std::fs::create_dir_all(&root);
            super::sampler::download(&url, &target, task);
        });
        self.url.clear();
    }
}
