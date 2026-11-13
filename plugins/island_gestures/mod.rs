//! 岛体交互：左右边缘滚轮翻页，左右滑动切页。
//!
//! 两侧边缘单独翻页：向上滚回上一页，向下滚到下一页；中间区域继续用岛体原来的
//! 列表滚动逻辑。滑切在进度条上不生效，否则松手会先翻页，
//! 这一帧不再画音乐页，跳转命令也就发不出去。
use eframe::egui;
use eframe::egui::{Pos2, Rect, Vec2};
use std::sync::Mutex;

/// 左右边缘的热区宽度。
const EDGE: f32 = 36.0;
/// 翻一页需要的滚轮累计量。
const WHEEL_STEP: f32 = 8.0;
/// 连续翻页的最短间隔，避免一次滚动连跳几页。
const PAGE_GAP: f64 = 0.20;

/// 一次滚轮的处理结果。
pub(crate) enum WheelAction {
    /// 不在边缘，交回岛体原来的列表滚动逻辑。
    Pass,
    /// 在边缘但还没到翻页阈值，吞掉这次滚轮。
    Consume,
    /// 翻页；`true` 表示下一页。
    Page(bool),
}

struct Wheel {
    delta: f32,
    at: f64,
    page_at: f64,
}

static WHEEL: Mutex<Wheel> = Mutex::new(Wheel { delta: 0.0, at: -1.0, page_at: -1.0 });

/// 两侧边缘的滚轮：向下到下一页，向上回上一页。`cursor` 是岛体内坐标。
pub(crate) fn edge_wheel(cursor: Pos2, t: f64, scroll: f32) -> WheelAction {
    let in_edge = cursor.x <= EDGE || cursor.x >= crate::app::PW - EDGE;
    if !in_edge { return WheelAction::Pass; }
    let Ok(mut wheel) = WHEEL.lock() else { return WheelAction::Consume; };
    // 停顿或反向时丢弃上一段余量，避免半格残留导致下一段直接翻页。
    if t - wheel.at > 0.18 || wheel.delta * scroll < 0.0 { wheel.delta = 0.0; }
    wheel.at = t;
    wheel.delta += scroll;
    if wheel.delta.abs() < WHEEL_STEP { return WheelAction::Consume; }
    let next = wheel.delta < 0.0;
    wheel.delta = 0.0;
    if t - wheel.page_at < PAGE_GAP { return WheelAction::Consume; }
    wheel.page_at = t;
    WheelAction::Page(next)
}

static SWIPE: Mutex<Option<Pos2>> = Mutex::new(None);

/// 观察指针按下与抬起；返回要翻的方向（`true` 表示下一页）。
pub(crate) fn swipe(app: &crate::app::App, c: &crate::paint::Card) -> Option<bool> {
    if !c.live || app.is_editing() { return None; }
    let events = c.ui.ctx().input(|input| input.events.clone());
    let Ok(mut start) = SWIPE.lock() else { return None; };
    for event in events {
        if let egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, .. } = event {
            if pressed {
                *start = starts_swipe(app.page, !app.media.title.is_empty(), app.show_lyrics, c.to_local(pos)).then_some(pos);
            } else if let Some(from) = start.take() {
                let delta = pos - from;
                if delta.x.abs() > 55.0 * c.s && delta.x.abs() > delta.y.abs() * 1.5 {
                    return Some(delta.x < 0.0);
                }
            }
        }
    }
    None
}

/// 这个位置按下是否算开始滑切；进度条区域整块排除。
fn starts_swipe(page: usize, has_music: bool, lyrics: bool, local: Pos2) -> bool {
    let zone = Rect::from_min_size(Pos2::new(36.0, 31.0), Vec2::new(347.0, 81.0));
    if !zone.contains(local) { return false; }
    if page == 0 && has_music {
        let (x, width, y) = if lyrics { (44.0, 176.0, 82.0) } else { (96.0, 221.0, 92.6) };
        let seek = Rect::from_min_size(Pos2::new(x - 8.0, y - 18.0), Vec2::new(width + 16.0, 36.0));
        if seek.contains(local) { return false; }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_drag_does_not_start_page_swipe() {
        assert!(!starts_swipe(0, true, true, Pos2::new(80.0, 82.0)), "有歌词时的进度条");
        assert!(!starts_swipe(0, true, false, Pos2::new(200.0, 92.6)), "无歌词时的进度条");
        assert!(starts_swipe(0, true, true, Pos2::new(120.0, 50.0)), "封面和标题仍可左右滑切页");
        assert!(starts_swipe(1, false, false, Pos2::new(200.0, 70.0)));
        assert!(!starts_swipe(0, true, true, Pos2::new(10.0, 82.0)), "进度条左侧外面不在切页区");
    }
}
