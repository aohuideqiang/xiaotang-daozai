//! 个人 CalDAV 日历：在上游只读日历之上补写入能力，并把今日日程并入待办页。
//!
//! - 读取沿用上游的发现、认证和 ICS 解析；
//! - 写入（新建、修改、删除）和「完成后只存本机」的勾选在这里；
//! - 编辑窗口是插件自己的独立窗口，不占上游的页面数组。
mod tasks;
mod write;

pub(crate) use tasks::{count, done_count, has_personal, open_editor, open_new, rows, toggle};

/// 当前同步或写入的提示；待办页只在非空时显示。
pub(crate) fn error() -> String {
    tasks::last_error()
}

/// 每帧调用：处理后台结果并画编辑窗口。
pub(crate) fn frame(ctx: &eframe::egui::Context) {
    tasks::frame(ctx);
}
