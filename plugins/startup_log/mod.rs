//! 启动日志：记录每次启动停在哪一步，并保留最近的一段历史。
//!
//! 上游只在 `last-startup.log` 里覆盖最近一次；这里加上时间和滚动历史，
//! 启动崩溃时能看到是第几次、停在哪一步。只写阶段名，不记账号或用户内容。
use std::io::Write;

/// 历史文件超过 256 KB 就轮换一份，避免无限增长。
const MAX_HISTORY: u64 = 256 * 1024;

/// 记录一个启动阶段；`stage` 只写阶段名，不写账号或用户内容。
pub fn record(stage: &str) {
    let directory = crate::model::config_dir().join("diagnostics");
    if std::fs::create_dir_all(&directory).is_err() { return; }
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f %:z");
    let line = format!("{now} · v{} · pid={} · {stage}\n", env!("CARGO_PKG_VERSION"), std::process::id());
    let _ = std::fs::write(directory.join("last-startup.log"), &line);
    let history = directory.join("startup-history.log");
    if std::fs::metadata(&history).is_ok_and(|metadata| metadata.len() > MAX_HISTORY) {
        let _ = std::fs::rename(&history, directory.join("startup-history.previous.log"));
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&history) {
        let _ = file.write_all(line.as_bytes());
    }
}
