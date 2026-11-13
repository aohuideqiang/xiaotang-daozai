//! 网易云 3.x 的底部播放条由 CEF 渲染，不提供系统进度接口，也没有 UIA 控件。
//!
//! 做法：找到播放器窗口里的渲染控件，把播放条位置换算成控件内坐标，直接发送
//! 鼠标消息；窗口最小化时先把窗口临时透明还原（不激活、不抢前台），点击后恢复
//! 原状态并重新最小化。是否跳转成功由插件在本地播放日志里回读确认。
use windows::core::w;
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::IsWindowEnabled;
use windows::Win32::UI::HiDpi::{GetDpiForWindow, SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumChildWindows, FindWindowW, GetClassNameW, GetClientRect, GetWindowLongPtrW, GetWindowThreadProcessId,
    IsIconic, SendMessageTimeoutW, SetLayeredWindowAttributes, SetWindowLongPtrW, ShowWindowAsync,
    GWL_EXSTYLE, LWA_ALPHA, SMTO_ABORTIFHUNG, SMTO_ERRORONEXIT, SW_MINIMIZE, SW_SHOWNOACTIVATE, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEMOVE, WS_EX_LAYERED,
};

/// 播放条距离窗口底部的高度（96 DPI 下的像素）。
const BAR_FROM_BOTTOM: f64 = 82.0;

/// 跳转并回读确认：先用本地播放日志取当前歌曲和时长，再点播放条，
/// 最后每 100 ms 回读位置；确认不到就返回失败，让界面提示在客户端里调整。
pub(crate) fn seek(seconds: f64) -> bool {
    let mut reader = crate::smtc::netease_playback::Reader::default();
    let Some(before) = reader.poll() else { return false; };
    if before.duration <= 0.0 { return false; }
    let target = seconds.min((before.duration - 0.1).max(0.0));
    if !click_progress_bar(target, before.duration) { return false; }
    let started = std::time::Instant::now();
    while started.elapsed() < std::time::Duration::from_secs(3) {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let arrived = reader.poll().is_some_and(|current| {
            current.song_id == before.song_id && reached(current.position, target, started.elapsed().as_secs_f64())
        });
        if arrived { return true; }
    }
    false
}

/// 和核心 `smtc::seek_arrived` 同一判定：跳转到位，或已随播放继续推进。
fn reached(current: f64, target: f64, elapsed: f64) -> bool {
    current.is_finite() && target.is_finite() && current >= target - 3.0 && current <= target + elapsed + 3.0
}

/// 线程 DPI 上下文，离开作用域时恢复。
struct DpiContext(DPI_AWARENESS_CONTEXT);
impl Drop for DpiContext {
    fn drop(&mut self) {
        if !self.0 .0.is_null() {
            unsafe { SetThreadDpiAwarenessContext(self.0); }
        }
    }
}

/// 最小化窗口临时还原用的守卫：离开作用域时恢复透明度和最小化状态。
struct Restore {
    host: HWND,
    previous_style: isize,
    was_layered: bool,
}
impl Drop for Restore {
    fn drop(&mut self) {
        unsafe {
            let _ = SetLayeredWindowAttributes(self.host, COLORREF::default(), 255, LWA_ALPHA);
            if !self.was_layered {
                let _ = SetWindowLongPtrW(self.host, GWL_EXSTYLE, self.previous_style);
            }
            let _ = ShowWindowAsync(self.host, SW_MINIMIZE);
        }
    }
}

/// 点击播放条跳到第 `position` 秒；返回是否把点击消息送达。
pub(crate) fn click_progress_bar(position: f64, duration: f64) -> bool {
    if !position.is_finite() || !duration.is_finite() || duration <= 0.0 { return false; }
    unsafe {
        let Ok(host) = FindWindowW(w!("OrpheusBrowserHost"), windows::core::PCWSTR::null()) else { return false; };
        if !host_matches(host) || !IsWindowEnabled(host).as_bool() { return false; }
        let _dpi = DpiContext(SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2));
        let scale = GetDpiForWindow(host) as f64 / 96.0;
        // 只有最小化才需要临时还原；守卫在本函数结束时恢复窗口状态。
        let mut restore: Option<Restore> = None;
        let target = match render_widget(host, scale) {
            Some(widget) => widget,
            None => {
                if !IsIconic(host).as_bool() { return false; }
                restore = Some(restore_window(host));
                let deadline = std::time::Instant::now() + std::time::Duration::from_millis(1200);
                loop {
                    if let Some(widget) = render_widget(host, scale) { break widget; }
                    if std::time::Instant::now() >= deadline { return false; }
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
            }
        };
        let mut rect = RECT::default();
        if GetClientRect(target, &mut rect).is_err() || rect.right <= rect.left { return false; }
        let Some(mut point) = bar_point(rect, position, duration, 1.0) else { return false; };
        // 播放条在渲染控件底部；控件本身比可见的播放条高。
        point.y = rect.bottom - (BAR_FROM_BOTTOM * scale).round() as i32;
        let lparam = LPARAM(((point.y as u32 & 0xffff) << 16 | (point.x as u32 & 0xffff)) as isize);
        let send = |message, buttons| {
            SendMessageTimeoutW(target, message, WPARAM(buttons), lparam, SMTO_ABORTIFHUNG | SMTO_ERRORONEXIT, 500, None).0 != 0
        };
        let sent = send(WM_MOUSEMOVE, 0) && send(WM_LBUTTONDOWN, 1) && send(WM_LBUTTONUP, 0);
        // 先恢复窗口状态，再返回结果。
        drop(restore.take());
        sent
    }
}

/// 只接受正在运行的网易云自己的窗口，避免点到其他 CEF 程序。
fn host_matches(host: HWND) -> bool {
    let Some(pid) = crate::smtc::netease_pid() else { return false; };
    let mut window_pid = 0;
    unsafe { GetWindowThreadProcessId(host, Some(&mut window_pid)); }
    window_pid == pid
}

/// 渲染控件：旧版叫 `Chrome_RenderWidgetHostHWND`，新版是 `Chrome_WidgetWin_0`。
fn render_widget(host: HWND, scale: f64) -> Option<HWND> {
    struct Search {
        found: Option<HWND>,
        scale: f64,
    }
    unsafe extern "system" fn visit(hwnd: HWND, param: LPARAM) -> BOOL {
        let search = &mut *(param.0 as *mut Search);
        let mut name = [0u16; 80];
        let length = GetClassNameW(hwnd, &mut name).max(0) as usize;
        let class = String::from_utf16_lossy(&name[..length]);
        if class == "Chrome_RenderWidgetHostHWND" || class == "Chrome_WidgetWin_0" {
            let mut rect = RECT::default();
            let big_enough = GetClientRect(hwnd, &mut rect).is_ok() && search.scale > 0.0
                && rect.right as f64 / search.scale >= 800.0 && rect.bottom as f64 / search.scale >= 550.0;
            if big_enough {
                search.found = Some(hwnd);
                return BOOL(0);
            }
        }
        BOOL(1)
    }
    let mut search = Search { found: None, scale };
    unsafe { let _ = EnumChildWindows(host, Some(visit), LPARAM(&mut search as *mut _ as isize)); }
    search.found
}

/// 把最小化的窗口变成透明可见、不激活，等待渲染控件出现。
fn restore_window(host: HWND) -> Restore {
    let previous_style = unsafe { GetWindowLongPtrW(host, GWL_EXSTYLE) };
    let was_layered = previous_style & WS_EX_LAYERED.0 as isize != 0;
    if !was_layered {
        unsafe { let _ = SetWindowLongPtrW(host, GWL_EXSTYLE, previous_style | WS_EX_LAYERED.0 as isize); }
    }
    unsafe {
        let _ = SetLayeredWindowAttributes(host, COLORREF::default(), 0, LWA_ALPHA);
        let _ = ShowWindowAsync(host, SW_SHOWNOACTIVATE);
    }
    Restore { host, previous_style, was_layered }
}

/// 播放条上第 `position` 秒对应的控件坐标；返回的是控件内的 x 和垂直中线。
fn bar_point(rect: RECT, position: f64, duration: f64, inset: f64) -> Option<POINT> {
    let width = (rect.right - rect.left) as f64;
    if duration <= 0.0 || position < 0.0 || position > duration || width <= inset * 2.0 + 10.0 || rect.bottom <= rect.top {
        return None;
    }
    let x = (rect.left as f64 + inset + (width - 2.0 * inset) * position / duration).round() as i32;
    Some(POINT { x: x.min(rect.right - 1), y: rect.top + (rect.bottom - rect.top) / 2 })
}
