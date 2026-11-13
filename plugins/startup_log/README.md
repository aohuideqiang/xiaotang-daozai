# 启动日志

记录每次启动停在哪一步，方便排查「启动即消失」：

- `last-startup.log`：最近一次，一行；
- `startup-history.log`：追加历史，超过 256 KB 轮换成 `startup-history.previous.log`。

只写时间、版本、进程号和阶段名，不写账号或用户内容。文件在
`%APPDATA%/Daozai/diagnostics/`。

自启项带 `--autostart`，日志里能区分登录自启和手动启动（见 `tray_startup`）。
