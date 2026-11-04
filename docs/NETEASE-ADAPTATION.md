# 网易云音乐商店版适配

本地修改日期：2026-11-04。下载源码基线：`b93dd84fc9bdb95978fef72ffb7068bdf47ac5a9`（0.6.50）。

## 原因与修改

源码已有网易云原生播放事件、歌词、封面和本地播放控制，但只读取普通安装版目录。本机运行的是微软商店版，其播放日志与队列在包隔离目录中，因此原有路径无法读取。

- 新增 `src/netease_paths.rs`：只查询正在运行的播放器的 Windows 包身份，定位其隔离目录；没有隔离日志时保留普通目录回退。
- `src/netease_playback.rs`：日志和队列共同使用选定目录；目录发生变化时清空旧偏移和播放状态，避免沿用旧歌进度。
- 新增 `examples/netease-path-check.rs`：只读诊断，不输出日志正文或操作播放器。
- `build.rs`：没有 Windows SDK 的 GNU 构建可用 windres 嵌入原有图标和版本资源，MSVC 构建方式不变。

## 验证与边界

普通目录、商店目录、旧普通版日志并存、缺失目录回退以及目录切换的回归测试均通过。两项本机只读集成测试通过，读取到了当前歌曲、推进的进度、49 行同步歌词及 256×256 专辑封面，并验证了封面缓存复用。

没有执行真实切歌或暂停操作；沿用项目已有的网易云本地控制命令。进度跳转仍依赖播放器提供的系统媒体接口。本次没有增加针对未知版本的进程内存偏移。

开发环境中运行：

```powershell
cargo test --locked --all-targets --features record -- --test-threads=1
cargo run --locked --example netease-path-check
cargo build --locked --release --bin daozai
```

需要正在播放的网易云来执行可选的只读集成测试；使用独立数据目录：

```powershell
$env:DAOZAI_DATA_DIR = Join-Path $env:TEMP 'daozai-netease-check'
cargo test --locked --lib live_native_netease -- --ignored --nocapture --test-threads=1
```
