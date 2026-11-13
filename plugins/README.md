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
