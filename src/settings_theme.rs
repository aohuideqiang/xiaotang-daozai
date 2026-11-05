//! 设置窗口独立外观，不改变岛体的绘制样式。
use eframe::egui::{self,Color32};
/// Shared typography, spacing and rounded controls for settings and forms.
pub fn island_style(base:&egui::Style,dark:bool)->egui::Style {
    let mut s=base.clone();s.visuals=if dark{egui::Visuals::dark()}else{egui::Visuals::light()};
    s.spacing.item_spacing=egui::vec2(10.0,10.0);s.spacing.button_padding=egui::vec2(14.0,8.0);s.spacing.interact_size.y=30.0;
    s.visuals.window_corner_radius=egui::CornerRadius::same(14);s.visuals.window_stroke=egui::Stroke::new(1.0,if dark{Color32::from_gray(55)}else{Color32::from_gray(215)});
    if dark{s.visuals.panel_fill=Color32::from_rgb(25,25,28);s.visuals.window_fill=Color32::from_rgb(29,29,33);s.visuals.extreme_bg_color=Color32::from_rgb(21,21,24);s.visuals.faint_bg_color=Color32::from_rgb(33,33,38);s.visuals.selection.bg_fill=Color32::from_rgb(40,90,145);}
    for w in [&mut s.visuals.widgets.inactive,&mut s.visuals.widgets.hovered,&mut s.visuals.widgets.active]{w.corner_radius=egui::CornerRadius::same(8);}
    if dark{s.visuals.widgets.inactive.bg_fill=Color32::from_rgb(42,42,48);s.visuals.widgets.hovered.bg_fill=Color32::from_rgb(57,57,65);s.visuals.widgets.active.bg_fill=Color32::from_rgb(44,80,120);}
    for (t,size) in [(egui::TextStyle::Body,13.0),(egui::TextStyle::Button,13.0),(egui::TextStyle::Small,11.0),(egui::TextStyle::Heading,19.0)]{s.text_styles.insert(t,egui::FontId::proportional(size));}
    s
}

#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub enum SettingsTheme { #[default] System, Light, Dark }
impl SettingsTheme {
    pub fn key(self)->&'static str {match self {Self::System=>"system",Self::Light=>"light",Self::Dark=>"dark"}}
    pub fn parse(value:&str)->Self {match value {"light"=>Self::Light,"dark"=>Self::Dark,_=>Self::System}}
    pub fn dark(self,system_dark:bool)->bool {match self {Self::System=>system_dark,Self::Light=>false,Self::Dark=>true}}
}

pub fn system_dark(ctx:&egui::Context)->bool {
    let id=egui::Id::new("settings-system-theme");
    if let Some((at,dark))=ctx.data(|d|d.get_temp::<(std::time::Instant,bool)>(id)) {
        if at.elapsed()<std::time::Duration::from_secs(1){return dark;}
    }
    #[cfg(windows)]
    let dark={
        use windows::{core::w,Win32::System::Registry::*};
        let mut value=1u32;let mut size=4u32;
        unsafe { let _=RegGetValueW(HKEY_CURRENT_USER,w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),RRF_RT_REG_DWORD,None,Some((&mut value as *mut u32).cast()),Some(&mut size)); }
        value==0
    };
    #[cfg(not(windows))]
    let dark=ctx.input(|i|i.raw.system_theme)==Some(egui::Theme::Dark);
    ctx.data_mut(|d|d.insert_temp(id,(std::time::Instant::now(),dark)));
    dark
}

pub fn color(ctx:&egui::Context,dark:Color32,light:Color32)->Color32 {
    if ctx.style().visuals.dark_mode {dark}else{light}
}
