# 插件（plugins）

本目录是和原作者上游分开维护的二开内容。项目根目录的 `plugins/` 不参与上游的
`src/` 目录，合并更新时基本不会冲突；每个子文件夹只放一个功能。

```
plugins/
├── mod.rs              插件注册：统一每帧入口
├── ui.rs               插件窗口共用的外观与系统调用
├── netease/            网易云音乐：商店版日志目录、原生进度跳转
├── caldav/             个人 CalDAV 日历：条件写入、待办联动
├── desktop_tools/      桌面工具：课表、下载、快捷启动、性能、服务器任务
├── island_gestures/    岛体交互：两侧滚轮翻页、左右滑切
├── startup_log/        启动日志：阶段时间与滚动历史
├── tray_startup/       开机自启参数、首次启动窗口等待
└── branding/           KarMa 定制发布说明、打包脚本、GNU 资源编译
```

## 接入点

插件只通过下面这些地方接触核心代码，全部带 `plugin:` 注记，跟上游更新时按注记核对即可：

| 文件 | 接入内容 |
| --- | --- |
| `src/lib.rs` | `#[path = "../plugins/mod.rs"] pub mod plugins;` |
| `src/main.rs` | 每帧 `daozai::plugins::frame(ctx, &mut self.app)`；网易云跳转提示文案 |
| `src/app.rs` | 滚轮分支交给 `plugins::island_gestures::edge_wheel`；`panel_body` 开头交给 `swipe`；删掉了不再使用的 `wheel_page_at` 之外的改动 |
| `src/pages.rs` | 待办页并入 `plugins::caldav` 的今日日程与错误提示 |
| `src/native.rs` | 窗口建立后 `plugins::desktop_tools::install_drop` 注册 URL / 文件拖放 |
| `src/extras.rs` | 设置「常规」页末尾的「插件」入口 |
| `src/tray.rs` | 自启命令行与「已有实例」等待由 `plugins::tray_startup` 提供 |
| `src/diagnostics.rs` | 启动日志改由 `plugins::startup_log` 记录 |
| `src/smtc.rs` | 网易云 `seek` 交给插件；暴露 `netease_pid` 给插件确认窗口归属 |
| `src/netease_playback.rs` | 日志目录交给插件选择；读取器记录当前文件，目录变化时重置 |
| `src/calendar.rs` | 暴露发现/请求等内部函数；自定义 CalDAV 凭据按服务器隔离；回环地址失败提示 |
| `build.rs` | 没有 `rc.exe` 时用 `plugins/branding/build_resources.rs` 的 `windres` 分支 |

## 跟上游同步

```powershell
git fetch upstream
git switch karma/plugins
git merge upstream/main
```

- 冲突只会出现在上表的接入点，都是几行的改动；
- `plugins/` 不在 `src/` 里，上游改动不会碰它；
- 上游新增页面、改设置布局时，只需要核对 `PLUGIN 注记` 是否还在原位。

上游的回归测试继续保留并运行（`cargo test --lib`）。本机验证记录见各插件目录的 README。

## 构建环境

本项目在 Windows 上使用 GNU 工具链（`x86_64-pc-windows-gnu` + 自带的 w64devkit）：

```powershell
cargo build --locked --release --bin daozai --target x86_64-pc-windows-gnu
```

两个和上游 MSVC 构建不同的地方：

1. **图标资源**：没有 Windows SDK 的 `rc.exe` 时，`build.rs` 调 `windres` 把同一份 `app.rc`
   编成 COFF 目标文件再链接（`plugins/branding/build_resources.rs`）。
2. **`winsqlite3`**：上游的波点音乐、豆包登录和通知来源模块直接链接系统的
   `winsqlite3.dll`，MinGW 没有同名导入库。本机在 w64devkit 的
   `x86_64-w64-mingw32/lib/libwinsqlite3.a` 放了一份由
   `gendef C:\Windows\System32\winsqlite3.dll` + `dlltool` 生成的导入库；
   这是本机工具链的准备步骤，不进仓库。

## 与上游行为的已知差异

- 滚轮：两侧边缘向下翻到下一页、向上回上一页；中间区域保持上游的列表滚动逻辑
  （上游的滚轮回归测试全部保留通过）。
- 个人 CalDAV：上游只读，插件补上新建、修改、删除；完成状态只存本机。
- 桌面工具是独立窗口，不占用岛体固定的页面数组，避免影响上游的页面管理。
- 剪贴板：上游置物架已有完整的剪贴板分栏（含固定、排除密码内容），插件不再重复采集。

## 上一版二开（v0.6.50 基线）逐项对照

旧二开是对上游 `b93dd84`（v0.6.50）打的一批补丁，上游后来重建了不少同类功能。
逐项核对后的去向如下；「未移植」的都写了原因，不需要再找回来。

| 旧二开内容 | 现在的位置 |
| --- | --- |
| `src/netease_paths.rs` 商店版日志目录 | `plugins/netease/paths.rs` |
| `netease_playback` 目录切换时重置偏移 | `src/netease_playback.rs`（接入点内） |
| `player_seek` 网易云播放条点击 | `plugins/netease/seek.rs` |
| `smtc::seek` 网易云分支 | `src/smtc.rs` 委托 `plugins::netease::seek` |
| `netease_timeline` 额外构建时间戳 | 未移植：跳转改用日志回读，不再依赖客户端版本 |
| 自定义 CalDAV（`Provider::Custom`、地址输入） | 上游已内置（CalDAV / iCloud / Exchange） |
| `Config::from_input` / `validate_server` | 上游 `normalize_server` + 设置页已内置 |
| 凭据按服务器隔离 | `src/calendar.rs` 的 `credential_name`（接入点） |
| 回环地址（隧道/本机）失败提示 | `src/calendar.rs` 的 `Failure::unreachable` |
| 发现流程 home == principal 的修正 | 上游已修（队列 + calendar-home-set） |
| `calendar/write.rs` 条件写入 | `plugins/caldav/write.rs` |
| `calendar_tasks.rs` 今日日程并入待办 | `plugins/caldav/tasks.rs` + `src/pages.rs`（接入点） |
| 日程页「课表」小入口 | 由桌面工具窗口统一入口替代 |
| `desktop_tools.rs` 六个页面 | `plugins/desktop_tools/`（独立窗口六个分页） |
| `url_drop.rs` 链接/文件拖放 | `plugins/desktop_tools/drop.rs` |
| `runtime/*.py` | `plugins/desktop_tools/runtime/*.py` |
| 岛体上的六个页面 | 并入桌面工具窗口（上游页面数组固定 9 页，不扩数组） |
| 剪贴板页面 | 上游置物架已有剪贴板分栏，不再重复采集 |
| `media_seeking` 串行化与「跳转中…」 | 未移植：上游 `ControlWorker` 已合并连续跳转，不会重复排队 |
| QQ 暂停状态滞后导致假失败 | `plugins/music_seek/`（本次补上） |
| `tray` 自启参数 `--autostart` | `plugins/tray_startup` |
| 激活已有窗口时等待首次启动 | `plugins/tray_startup::retry` |
| 托盘按 pid 查找窗口 | 未移植：上游按标题 + pid 校验已覆盖，极少冲突 |
| `diagnostics` 启动日志格式 | `plugins/startup_log` |
| `settings_theme::island_style` | `plugins/ui.rs` |
| `build.rs` 的 windres 分支 | `plugins/branding/build_resources.rs` |
| `Cargo.toml` 的 Windows 特性 | `Cargo.toml`（接入） |
| README 定制说明、`docs/KARMA-*.md` | `KARMA.md` + 各插件 README |
| `examples/calendar-write-check.rs` | 未移植：写入 API 在插件内部，不对外暴露 |
| 删除 `ui_tests.rs` / `tests/` | 不采用：保留上游回归测试 |
| CI 改为只编译 | 不采用：保留上游 CI |
