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
