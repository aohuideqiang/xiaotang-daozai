//! 插件窗口共用的外观与系统调用，和设置窗口保持一致。
use eframe::egui::{self, Color32};

/// 深色窗口底色，与设置窗口一致。
pub const WINDOW_FILL: Color32 = Color32::from_rgb(25, 25, 28);

/// 表单、插件窗口共用的字号、间距与圆角；`dark` 跟随系统主题。
pub fn island_style(base: &egui::Style, dark: bool) -> egui::Style {
    let mut style = base.clone();
    style.visuals = if dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(14.0, 8.0);
    style.spacing.interact_size.y = 30.0;
    style.visuals.window_corner_radius = egui::CornerRadius::same(14);
    style.visuals.window_stroke = egui::Stroke::new(1.0_f32, if dark { Color32::from_gray(55) } else { Color32::from_gray(215) });
    if dark {
        style.visuals.panel_fill = WINDOW_FILL;
        style.visuals.window_fill = Color32::from_rgb(29, 29, 33);
        style.visuals.extreme_bg_color = Color32::from_rgb(21, 21, 24);
        style.visuals.faint_bg_color = Color32::from_rgb(33, 33, 38);
        style.visuals.selection.bg_fill = Color32::from_rgb(40, 90, 145);
    }
    for widget in [&mut style.visuals.widgets.inactive, &mut style.visuals.widgets.hovered, &mut style.visuals.widgets.active] {
        widget.corner_radius = egui::CornerRadius::same(8);
    }
    if dark {
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(42, 42, 48);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(57, 57, 65);
        style.visuals.widgets.active.bg_fill = Color32::from_rgb(44, 80, 120);
    }
    for (text_style, size) in [
        (egui::TextStyle::Body, 13.0),
        (egui::TextStyle::Button, 13.0),
        (egui::TextStyle::Small, 11.0),
        (egui::TextStyle::Heading, 19.0),
    ] {
        style.text_styles.insert(text_style, egui::FontId::proportional(size));
    }
    style
}

/// 用系统默认程序打开文件或文件夹，失败时返回可以直接显示的提示。
pub fn open_path(path: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows::core::{HSTRING, PCWSTR};
        use windows::Win32::UI::Shell::ShellExecuteW;
        use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
        let target = HSTRING::from(path.as_os_str());
        let result = unsafe {
            ShellExecuteW(None, windows::core::w!("open"), &target, PCWSTR::null(), PCWSTR::null(), SW_SHOWNORMAL)
        };
        // ShellExecuteW 用返回值当 HINSTANCE；大于 32 才是成功，和 Win32 文档一致。
        if result.0 as isize > 32 { Ok(()) } else { Err(format!("无法打开 {}", path.display())) }
    }
    #[cfg(not(windows))]
    {
        Err(format!("无法打开 {}", path.display()))
    }
}

/// 按字符数截断，避免长标题把表格撑开。
pub fn ellipsis(text: &str, max_chars: usize) -> String {
    let mut result = String::new();
    for (index, character) in text.chars().enumerate() {
        if index >= max_chars { result.push('…'); break; }
        result.push(character);
    }
    result
}
