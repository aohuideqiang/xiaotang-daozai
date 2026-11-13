//! 开机自启：写入启动参数，并在首次启动时等待窗口和托盘就绪。
//!
//! 自启项带 `--autostart`，启动日志里能区分「登录自启」和「手动启动」；
//! 互斥体先于图形窗口建立，第一次点托盘图标时窗口可能还没创建，所以要等一下。

/// 自启命令行：可执行文件路径加 `--autostart`。
pub(crate) fn autostart_command(exe: &str) -> String {
    format!("\"{exe}\" --autostart")
}

/// 反复尝试打开已有窗口，最多等 `limit`；互斥体先于窗口建立时避免直接失败。
pub(crate) fn retry(limit: std::time::Duration, mut attempt: impl FnMut() -> bool) -> bool {
    let started = std::time::Instant::now();
    loop {
        if attempt() { return true; }
        if started.elapsed() >= limit { return false; }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// 在指定进程里找标题为「岛仔」的主窗口。
///
/// 不能只按标题找：同时开着别的版本或别的程序时，`FindWindowW` 可能拿到别人的窗口，
/// 进程号对不上就永远不会激活。按进程号筛选后再比标题，就不会认错。
#[cfg(windows)]
pub(crate) fn island_window(pid: u32) -> Option<windows::Win32::Foundation::HWND> {
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowTextW, GetWindowThreadProcessId};
    struct Search {
        pid: u32,
        found: Option<HWND>,
    }
    unsafe extern "system" fn visit(hwnd: HWND, param: LPARAM) -> BOOL {
        let search = &mut *(param.0 as *mut Search);
        let mut window_pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut window_pid));
        if window_pid == search.pid {
            let mut title = [0u16; 64];
            let length = GetWindowTextW(hwnd, &mut title).max(0) as usize;
            if String::from_utf16_lossy(&title[..length]) == "岛仔" {
                search.found = Some(hwnd);
                return BOOL(0);
            }
        }
        BOOL(1)
    }
    let mut search = Search { pid, found: None };
    unsafe { let _ = EnumWindows(Some(visit), LPARAM(&mut search as *mut _ as isize)); }
    search.found
}
