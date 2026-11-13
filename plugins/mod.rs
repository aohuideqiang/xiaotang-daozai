//! 插件：KarMa 定制功能按「一个文件夹一个功能」放在这里。
//!
//! 插件不修改上游的模块结构，只通过下面这些带 `plugin:` 注释的接入点接触核心代码；
//! 跟着原作者更新时，按注释逐处核对即可：
//!
//! - `lib.rs`：`pub mod plugins;`
//! - `main.rs`：每帧调用 `plugins::frame(ctx)`。
//! - `app.rs`：滚轮分页、页面滑切交给 `plugins::island_gestures`。
//! - `pages.rs`：待办列表末尾并入 `plugins::caldav` 的今日日程。
//! - `smtc.rs`：网易云进度跳转交给 `plugins::netease`。
//! - `netease_playback.rs`：播放日志目录交给 `plugins::netease` 解析。
//! - `native.rs`：窗口建立后注册 URL / 文件拖放。
//! - `extras.rs`、`tray.rs`：插件入口（设置窗口与托盘菜单）。
//! - `calendar.rs`：个人日历的凭据隔离与回环地址提示。
//! - `diagnostics.rs`：启动日志由 `plugins::startup_log` 记录。
//! - `build.rs`：GNU 工具链的资源编译由 `plugins::branding` 提供。

pub mod caldav;
pub mod desktop_tools;
pub mod island_gestures;
pub mod netease;
pub mod startup_log;
pub mod tray_startup;
pub mod ui;

/// 每帧调用一次：驱动插件自己的后台任务与独立窗口。
pub fn frame(ctx: &eframe::egui::Context, app: &mut crate::app::App) {
    desktop_tools::frame(ctx, app);
    caldav::frame(ctx);
}
