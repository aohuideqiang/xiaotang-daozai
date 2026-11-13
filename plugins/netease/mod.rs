//! 网易云音乐适配：商店版数据目录 + 原生进度跳转。
//!
//! 上游只读普通安装版的日志、也没有进度跳转；插件补上这两点，其余逻辑不变。

mod paths;
mod seek;

/// 播放日志目录：商店版隔离目录优先，没有日志时回退普通安装版。
pub(crate) fn log_path(local: &std::path::Path, pid: Option<u32>) -> std::path::PathBuf {
    let family = pid.and_then(paths::package_family);
    paths::log_path(local, family.as_deref())
}

/// 用本地播放日志回读确认的进度跳转；不支持时返回 `false`。
pub(crate) fn seek(seconds: f64) -> bool {
    seek::seek(seconds)
}
