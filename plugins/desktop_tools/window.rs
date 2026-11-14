//! 桌面工具：岛体页面（课表 / 下载 / 快捷启动 / 系统性能 / 服务器任务）和独立窗口共用。
//!
//! 岛体页面画在 419×140 的面板里，独立窗口画在 700×560 的窗口里；
//! 两者共用同一份采样数据，避免采样线程跑两遍。

use super::sampler::{download_progress, Download};
use chrono::Datelike;
use eframe::egui::{self, Color32};

/// 独立窗口的分页；岛体页面用前 5 个。
pub(crate) const TABS: [&str; 6] = ["课表", "下载", "快捷启动", "系统性能", "服务器任务", "设置"];

// ---------------------------------------------------------------- 岛体页面

/// 课表：今天和明天两栏，点右上角切到完整一周。
pub(crate) fn page_courses(c: &crate::paint::Card, _env: &crate::app::Env, tools: &mut super::Tools) {
    use crate::paint::H;
    use crate::theme::{self, W};
    if tools.courses.is_null() { tools.read_courses(); }
    let today = chrono::Local::now().date_naive();
    let card = Color32::from_rgb(29, 29, 33);
    if tools.week {
        let start = today - chrono::Duration::days(today.weekday().num_days_from_monday() as i64)
            + chrono::Duration::days(tools.offset);
        for index in 0..7 {
            let day = start + chrono::Duration::days(index);
            let y = 33.0 + index as f32 * 24.0;
            c.fill(38.0, y, 342.0, 22.0, 7.0, card);
            c.text(47.0, y + 15.0, &format!("周{} {}", ["一","二","三","四","五","六","日"][index as usize], day.format("%m/%d")), 9.0, W::Semibold, theme::WHITE, H::Left);
            let line = crate::plugins::ui::ellipsis(&courses_line(tools, day), 26);
            c.text(128.0, y + 15.0, &line, 9.0, W::Regular, theme::GRAY, H::Left);
        }
        c.text(340.0, 21.0, "两日", 10.0, W::Regular, theme::GRAY, H::Center);
        if c.hit(315.0, 3.0, 55.0, 27.0, "course-collapse").clicked() { tools.week = false; }
    } else {
        for index in 0..2 {
            let day = today + chrono::Duration::days(index);
            let x = 42.0 + index as f32 * 171.0;
            c.fill(x, 36.0, 164.0, 76.0, 10.0, card);
            c.text(x + 9.0, 51.0, &format!("{} {}", if index == 0 { "今天" } else { "明天" }, day.format("%m/%d")), 10.0, W::Semibold, theme::WHITE, H::Left);
            let rows = super::courses::course_day(&tools.courses, day);
            if rows.is_empty() {
                c.text(x + 9.0, 76.0, if tools.courses.is_null() { "尚未导入独立课表" } else { "当天无课" }, 10.0, W::Regular, theme::GRAY, H::Left);
            }
            for (row_index, row) in rows.iter().take(2).enumerate() {
                let line = crate::plugins::ui::ellipsis(&summary(row), 15);
                c.text(x + 9.0, 72.0 + row_index as f32 * 20.0, &line, 9.0, W::Regular, theme::GRAY, H::Left);
            }
            if c.hit(x, 36.0, 164.0, 76.0, ("native-course", index)).clicked() { tools.week = true; tools.offset = 0; }
        }
    }
}

/// 下载：Edge 的下载和岛仔自己的任务，最多显示 3 条。
pub(crate) fn page_downloads(c: &crate::paint::Card, tools: &mut super::Tools, snapshot: &super::sampler::Snapshot) {
    use crate::paint::H;
    use crate::theme::{self, W};
    if c.hit(40.0, 5.0, 335.0, 25.0, "download-input-open").on_hover_text("在「桌面工具」窗口里粘贴或拖入下载地址").clicked() {
        tools.open = true;
        tools.tab = 1;
    }
    let own: Vec<Download> = tools.downloads.iter()
        .filter_map(|task| task.lock().ok().map(|task| task.clone()))
        .collect();
    if own.is_empty() && snapshot.edge.is_empty() {
        c.text(45.0, 65.0, "暂无下载任务（拖链接到岛体会直接下载）", 11.0, W::Regular, theme::GRAY, H::Left);
        return;
    }
    let rows: Vec<(bool, Download)> = snapshot.edge.iter().cloned().map(|row| (true, row))
        .chain(own.into_iter().map(|row| (false, row)))
        .take(3)
        .collect();
    for (index, (from_edge, row)) in rows.into_iter().enumerate() {
        let y = 40.0 + index as f32 * 25.0;
        let name = std::path::Path::new(&row.path).file_name().and_then(|name| name.to_str()).unwrap_or("下载");
        c.text(45.0, y + 12.0, &crate::plugins::ui::ellipsis(name, 22), 10.0, W::Regular, theme::WHITE, H::Left);
        let progress = download_progress(&row);
        let progress = if from_edge { progress } else { progress.replace("Edge", "岛仔") };
        c.text(370.0, y + 12.0, &crate::plugins::ui::ellipsis(&progress, 24), 9.0, W::Regular, theme::GRAY, H::Right);
    }
}

/// 快捷启动：最多 6 个入口，点击直接打开。
pub(crate) fn page_shortcuts(c: &crate::paint::Card, tools: &mut super::Tools) {
    use crate::paint::H;
    use crate::theme::{self, W};
    let card = Color32::from_rgb(29, 29, 33);
    if tools.config.shortcuts.is_empty() {
        c.text(45.0, 65.0, "还没有快捷入口，可在「桌面工具 → 快捷启动」里添加", 10.0, W::Regular, theme::GRAY, H::Left);
        return;
    }
    for (index, (name, target)) in tools.config.shortcuts.iter().take(6).enumerate() {
        let x = 42.0 + (index % 3) as f32 * 114.0;
        let y = 37.0 + (index / 3) as f32 * 35.0;
        c.fill(x, y, 108.0, 29.0, 9.0, card);
        c.text(x + 54.0, y + 19.0, &crate::plugins::ui::ellipsis(name, 7), 10.0, W::Regular, theme::WHITE, H::Center);
        if c.hit(x, y, 108.0, 29.0, ("native-shortcut", index)).clicked() {
            open_shortcut(target);
        }
    }
}

/// 系统性能：CPU 与内存占用。
pub(crate) fn page_performance(c: &crate::paint::Card, tools: &mut super::Tools, snapshot: &super::sampler::Snapshot) {
    use crate::paint::H;
    use crate::theme::{self, W};
    let card = Color32::from_rgb(29, 29, 33);
    if !tools.config.enabled[2] {
        c.text(45.0, 62.0, "系统采样已关闭", 11.0, W::Regular, theme::GRAY, H::Left);
        return;
    }
    for (index, (label, value)) in [("CPU", snapshot.cpu), ("内存", snapshot.memory)].iter().enumerate() {
        let x = 45.0 + index as f32 * 170.0;
        c.fill(x, 37.0, 155.0, 70.0, 10.0, card);
        c.text(x + 12.0, 55.0, label, 10.0, W::Semibold, theme::GRAY, H::Left);
        c.text(x + 12.0, 84.0, &format!("{value:.1}%"), 22.0, W::Bold, theme::WHITE, H::Left);
        c.fill(x + 12.0, 94.0, 130.0, 4.0, 2.0, Color32::from_gray(60));
        c.fill(x + 12.0, 94.0, 130.0 * (*value as f32 / 100.0).clamp(0.0, 1.0), 4.0, 2.0, theme::BLUE);
    }
}

/// 服务器任务：名称 / 用途 / 状态三列，滚轮翻 3 行。
pub(crate) fn page_server(c: &crate::paint::Card, tools: &mut super::Tools, snapshot: &super::sampler::Snapshot) {
    use crate::paint::H;
    use crate::theme::{self, W};
    let card = Color32::from_rgb(29, 29, 33);
    if !tools.config.enabled[3] {
        c.text(45.0, 62.0, "服务器读取已关闭", 11.0, W::Regular, theme::GRAY, H::Left);
        return;
    }
    let parsed = serde_json::from_str::<serde_json::Value>(&snapshot.tasks).ok();
    let connected = parsed.as_ref().and_then(|value| value["connected"].as_bool()).unwrap_or(true);
    let Some(rows) = parsed.as_ref().and_then(|value| value["services"].as_array()) else {
        c.text(45.0, 62.0, if snapshot.tasks.is_empty() { "正在读取服务器状态…" } else { "服务器连接失败或数据不可用" }, 10.0, W::Regular, theme::GRAY, H::Left);
        return;
    };
    tools.server_offset = tools.server_offset.min(rows.len().saturating_sub(1) / 3 * 3);
    c.text(45.0, 43.0, "名称", 9.0, W::Semibold, theme::GRAY, H::Left);
    c.text(155.0, 43.0, "用途", 9.0, W::Semibold, theme::GRAY, H::Left);
    let status = if connected { "状态".to_string() } else { parsed.as_ref().and_then(|value| value["reason"].as_str()).unwrap_or("连接失败").to_string() };
    c.text(325.0, 43.0, &crate::plugins::ui::ellipsis(&status, 18), 9.0, W::Semibold, if connected { theme::GRAY } else { theme::RED_TOP }, H::Right);
    for (index, row) in rows.iter().skip(tools.server_offset).take(3).enumerate() {
        let y = 48.0 + index as f32 * 22.0;
        c.fill(39.0, y, 296.0, 20.0, 6.0, card);
        c.text(45.0, y + 14.0, &crate::plugins::ui::ellipsis(row["name"].as_str().unwrap_or("—"), 14), 9.0, W::Regular, theme::WHITE, H::Left);
        c.text(155.0, y + 14.0, &crate::plugins::ui::ellipsis(row["description"].as_str().unwrap_or("—"), 16), 9.0, W::Regular, theme::GRAY, H::Left);
        let color = match row["state"].as_str() { Some("ok") => theme::GREEN, Some("down") => theme::RED_TOP, _ => theme::GRAY };
        c.text(325.0, y + 14.0, row["label"].as_str().unwrap_or("未知"), 9.0, W::Semibold, color, H::Right);
    }
}

/// 岛体「服务器任务」页用滚轮翻行：一次 3 行，返回是否滚动了。
pub(crate) fn scroll_server(tools: &mut super::Tools, next: bool) -> bool {
    let rows = tools.shared.lock().ok()
        .and_then(|state| serde_json::from_str::<serde_json::Value>(&state.tasks).ok())
        .and_then(|value| value["services"].as_array().cloned())
        .unwrap_or_default();
    if rows.is_empty() { return false; }
    let max = rows.len().saturating_sub(1) / 3 * 3;
    let target = if next { (tools.server_offset + 3).min(max) } else { tools.server_offset.saturating_sub(3) };
    let changed = target != tools.server_offset;
    tools.server_offset = target;
    changed
}

fn summary(row: &str) -> String {
    let mut lines = row.lines();
    format!("{} {}", lines.next().unwrap_or(""), lines.next().unwrap_or("课程"))
}

fn courses_line(tools: &super::Tools, day: chrono::NaiveDate) -> String {
    let rows = super::courses::course_day(&tools.courses, day);
    if rows.is_empty() {
        return if tools.courses.is_null() { "尚未导入独立课表".into() } else { "无课".into() };
    }
    rows.iter().map(|row| summary(row)).collect::<Vec<_>>().join(" · ")
}

// ---------------------------------------------------------------- 独立窗口

/// 独立窗口：设置里的「桌面工具」入口，六页 + 设置。
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
    let own: Vec<Download> = tools.downloads.iter()
        .filter_map(|task| task.lock().ok().map(|task| task.clone()))
        .collect();
    let rows: Vec<(bool, Download)> = snapshot.edge.iter().cloned().map(|row| (true, row))
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
fn open_shortcut(target: &str) {
    if target.starts_with("https://") || target.starts_with("http://") {
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
