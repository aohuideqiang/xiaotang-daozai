//! 后台采样：Edge 下载、系统性能、服务器状态。
//!
//! 采样线程独立运行，界面每帧只读一份快照，不会因为远程命令或磁盘读取卡住绘制。
use super::config::Config;
use serde_json::Value;
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

/// 一条下载任务；`status` 是「Edge」或岛仔自己的任务状态。
#[derive(Clone, Default)]
pub(crate) struct Download {
    pub path: String,
    pub bytes: u64,
    pub total: u64,
    pub status: String,
}

/// 采样快照，界面只读。
#[derive(Clone, Default)]
pub(crate) struct Snapshot {
    pub cpu: f64,
    pub memory: f64,
    pub tasks: String,
    pub updated: String,
    pub edge: Vec<Download>,
    pub edge_error: String,
}

/// 启动采样线程；返回的发送端用来推送配置变化。
pub(crate) fn start(shared: Arc<Mutex<Snapshot>>, initial: Config) -> Option<mpsc::Sender<Config>> {
    let (tx, rx) = mpsc::channel();
    std::thread::Builder::new().name("daozai-desktop-tools".into()).spawn(move || run(rx, shared, initial)).ok()?;
    Some(tx)
}

fn run(rx: mpsc::Receiver<Config>, shared: Arc<Mutex<Snapshot>>, initial: Config) {
    let mut config = initial;
    let mut prior = (0u64, 0u64, 0u64);
    let mut ticked = Instant::now() - Duration::from_secs(60);
    let mut edge_at = Instant::now() - Duration::from_secs(60);
    let mut server_at = Instant::now() - Duration::from_secs(60);
    let server_pending = Arc::new(AtomicBool::new(false));
    let edge_pending = Arc::new(AtomicBool::new(false));
    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(next) => {
                config = next;
                // 服务器参数可能刚改过，立即重试一次。
                server_at = Instant::now() - Duration::from_secs(60);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if !config.enabled[0] {
            if let Ok(mut state) = shared.lock() { state.edge.clear(); state.edge_error.clear(); }
        }
        if config.enabled[0] && edge_at.elapsed() >= Duration::from_secs(2) && !edge_pending.swap(true, Ordering::Relaxed) {
            edge_at = Instant::now();
            let shared = shared.clone();
            let pending = edge_pending.clone();
            std::thread::spawn(move || {
                let (rows, error) = edge_downloads();
                if let Ok(mut state) = shared.lock() { state.edge = rows; state.edge_error = error; }
                pending.store(false, Ordering::Relaxed);
            });
        }
        if config.enabled[2] && ticked.elapsed() >= Duration::from_secs(2) {
            ticked = Instant::now();
            #[cfg(windows)]
            {
                let (cpu, memory) = perf(&mut prior);
                if let Ok(mut state) = shared.lock() { state.cpu = cpu; state.memory = memory; }
            }
        }
        if config.enabled[3] && server_at.elapsed() >= Duration::from_secs(30) && !server_pending.swap(true, Ordering::Relaxed) {
            server_at = Instant::now();
            config.write_connection();
            let bridge = config.bridge.clone();
            let shared = shared.clone();
            let pending = server_pending.clone();
            std::thread::spawn(move || {
                let message = if bridge.is_empty() { "尚未配置服务器总览来源".to_string() } else { server_overview(&bridge) };
                if let Ok(mut state) = shared.lock() {
                    state.tasks = message;
                    state.updated = chrono::Local::now().format("%H:%M:%S").to_string();
                }
                pending.store(false, Ordering::Relaxed);
            });
        }
    }
}

/// 读取 Edge 的下载记录；脚本只读 `History` 表，不查询浏览地址。
fn edge_downloads() -> (Vec<Download>, String) {
    let mut command = std::process::Command::new("python");
    command.args(["-I", "-c", include_str!("runtime/edge-downloads.py")]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    let data = command.output().ok().filter(|output| output.status.success())
        .and_then(|output| serde_json::from_slice::<Value>(&output.stdout).ok());
    match data {
        Some(value) => {
            let rows: Vec<Download> = value["downloads"].as_array().map(|rows| {
                rows.iter().take(50).filter_map(|row| Some(Download {
                    path: row["path"].as_str()?.into(),
                    bytes: row["bytes"].as_u64()?,
                    total: row["total"].as_u64()?,
                    status: "Edge".into(),
                })).collect()
            }).unwrap_or_default();
            let error = if value["error"].as_bool() == Some(true) { "部分 Edge 下载暂时无法读取，将自动重试".into() } else { String::new() };
            (rows, error)
        }
        None => (Vec::new(), "无法读取 Edge 下载，请确认本机 Python 可用；稍后自动重试".into()),
    }
}

/// 只运行用户配置的本机脚本，输出截断到 64 KB。
fn server_overview(path: &str) -> String {
    let mut command = std::process::Command::new("python");
    command.arg(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    match command.output() {
        Ok(output) if output.status.success() => {
            let bytes = &output.stdout[..output.stdout.len().min(65536)];
            String::from_utf8_lossy(bytes).into()
        }
        _ => "服务器总览暂时不可用（稍后自动重试）".into(),
    }
}

/// 两次采样之间的 CPU 占用与内存占用。
#[cfg(windows)]
fn perf(prior: &mut (u64, u64, u64)) -> (f64, f64) {
    #[repr(C)]
    struct Memory {
        length: u32,
        load: u32,
        total: u64,
        available: u64,
        page: u64,
        page_available: u64,
        virtual_total: u64,
        virtual_available: u64,
        extended: u64,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetSystemTimes(idle: *mut u64, kernel: *mut u64, user: *mut u64) -> i32;
        fn GlobalMemoryStatusEx(memory: *mut Memory) -> i32;
    }
    unsafe {
        let (mut idle, mut kernel, mut user) = (0u64, 0u64, 0u64);
        let valid = GetSystemTimes(&mut idle, &mut kernel, &mut user) != 0;
        let total = kernel.saturating_sub(prior.1) + user.saturating_sub(prior.2);
        let cpu = if valid && prior.1 != 0 && total > 0 {
            // kernel 时间里包含 idle，所以分子是「(kernel+user) 的增量 - idle 增量」。
            100.0 * total.saturating_sub(idle.saturating_sub(prior.0)) as f64 / total as f64
        } else { 0.0 };
        if valid { *prior = (idle, kernel, user); }
        let mut memory: Memory = std::mem::zeroed();
        memory.length = std::mem::size_of::<Memory>() as u32;
        let load = if GlobalMemoryStatusEx(&mut memory) != 0 { memory.load as f64 } else { 0.0 };
        (cpu.clamp(0.0, 100.0), load)
    }
}

#[cfg(not(windows))]
fn perf(_prior: &mut (u64, u64, u64)) -> (f64, f64) { (0.0, 0.0) }

/// 读一个下载任务的进度文案。
pub(crate) fn download_progress(download: &Download) -> String {
    if download.total > 0 {
        format!(
            "{} {:.0}% · {:.1}/{:.1} MB",
            download.status,
            (download.bytes as f64 / download.total as f64 * 100.0).min(100.0),
            download.bytes as f64 / 1048576.0,
            download.total as f64 / 1048576.0
        )
    } else {
        format!("{} · 已下载 {:.1} MB", download.status, download.bytes as f64 / 1048576.0)
    }
}

/// 岛仔自己的下载任务：写到 `Downloads`，先写 `.part` 再改名。
pub(crate) fn download(url: &str, path: &std::path::Path, task: Arc<Mutex<Download>>) {
    let result = (|| -> Result<(), String> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(10))
            .timeout_read(Duration::from_secs(30))
            .build();
        let response = agent.get(url).call().map_err(|_| "下载连接失败".to_string())?;
        task.lock().map_err(|_| "下载任务已失效".to_string())?.total =
            response.header("Content-Length").and_then(|value| value.parse().ok()).unwrap_or(0);
        let partial = path.with_extension(format!("{}.part", path.extension().and_then(|ext| ext.to_str()).unwrap_or("bin")));
        let mut input = response.into_reader();
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&partial).map_err(|error| error.to_string())?;
        let mut buffer = [0u8; 65536];
        loop {
            let read = input.read(&mut buffer).map_err(|_| "下载中断".to_string())?;
            if read == 0 { break; }
            std::io::Write::write_all(&mut file, &buffer[..read]).map_err(|error| error.to_string())?;
            if let Ok(mut task) = task.lock() { task.bytes += read as u64; }
        }
        file.sync_all().map_err(|error| error.to_string())?;
        drop(file);
        let (total, received) = task.lock().map(|task| (task.total, task.bytes)).unwrap_or((0, 0));
        if total > 0 && received != total { return Err("下载文件不完整".into()); }
        std::fs::rename(&partial, path).map_err(|error| error.to_string())?;
        Ok(())
    })();
    if let Ok(mut task) = task.lock() {
        task.status = match result { Ok(()) => "已完成".into(), Err(error) => format!("失败：{error}") };
    }
}
