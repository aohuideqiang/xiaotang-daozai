//! 网易云音乐的播放日志目录：优先商店版隔离目录，没有日志时回退普通安装版。
//! 只查询正在运行的那个播放器进程，不扫描其他应用。
use std::path::{Path, PathBuf};

/// 播放日志位置。`family` 是商店版进程的包族名，普通安装版为 `None`。
pub(crate) fn log_path(local: &Path, family: Option<&str>) -> PathBuf {
    if let Some(family) = family {
        let redirected = local.join("Packages").join(family)
            .join("LocalCache/Local/NetEase/CloudMusic/cloudmusic.elog");
        // 商店版刚安装时还没有日志，继续回退普通目录。
        if redirected.is_file() { return redirected; }
    }
    local.join("NetEase/CloudMusic/cloudmusic.elog")
}

/// 已运行进程的包族名；商店版才有，普通安装版返回 `None`。
#[cfg(windows)]
pub(crate) fn package_family(pid: u32) -> Option<String> {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> *mut c_void;
        fn CloseHandle(handle: *mut c_void) -> i32;
        fn GetPackageFamilyName(process: *mut c_void, length: *mut u32, name: *mut u16) -> i32;
    }
    unsafe {
        let process = OpenProcess(0x1000, 0, pid); // PROCESS_QUERY_LIMITED_INFORMATION
        if process.is_null() { return None; }
        // 有界缓冲区，同时容纳 Windows 返回的结尾 NUL。
        let mut name = [0u16; 128];
        let mut length = name.len() as u32;
        let result = GetPackageFamilyName(process, &mut length, name.as_mut_ptr());
        CloseHandle(process);
        if result != 0 || length < 2 || length as usize > name.len() { return None; }
        String::from_utf16(&name[..length as usize - 1]).ok()
    }
}

#[cfg(not(windows))]
pub(crate) fn package_family(_pid: u32) -> Option<String> { None }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_log_takes_priority_over_an_old_desktop_install() {
        let root = std::env::temp_dir().join(format!("daozai-paths-{}", std::process::id()));
        let desktop = log_path(&root, None);
        let store = root.join("Packages/player_family/LocalCache/Local/NetEase/CloudMusic/cloudmusic.elog");
        std::fs::create_dir_all(desktop.parent().unwrap()).unwrap();
        std::fs::create_dir_all(store.parent().unwrap()).unwrap();
        std::fs::write(&desktop, b"old desktop log").unwrap();
        std::fs::write(&store, b"store log").unwrap();
        assert_eq!(log_path(&root, Some("player_family")), store);
        assert_eq!(log_path(&root, None), desktop);
        assert_eq!(log_path(&root, Some("another_family")), desktop);
        std::fs::remove_file(&store).unwrap();
        assert_eq!(log_path(&root, Some("player_family")), desktop);
        std::fs::remove_dir_all(root).unwrap();
    }
}
