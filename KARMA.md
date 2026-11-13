# KarMa-岛仔（二开说明）

**本项目是对开源项目「岛仔」的二次开发与个人定制，并非从零独立原创。** 原项目作者为
**Renly1994**，原仓库为 [Renly1994/xiaotang-daozai](https://github.com/Renly1994/xiaotang-daozai)。
本版以原作者最新版本 **v0.6.64**（`upstream/main`）为基线，二开内容全部放在
[plugins/](plugins/README.md)，按「一个文件夹一个功能」组织。

原版的灵动岛界面设计、基础架构、动画、音乐控制、日程、邮件、天气、额度等既有功能
都来自上游；`KarMa-岛仔` 是这个定制版本的名字，不表示对原作的重新署名，也不代表
原作者的官方发布或背书。

## 这个版本多了什么

| 插件 | 内容 |
| --- | --- |
| [netease](plugins/netease/README.md) | 网易云商店版日志目录、原生进度跳转 |
| [caldav](plugins/caldav/README.md) | 个人 CalDAV 日历写入、今日日程并入待办 |
| [desktop_tools](plugins/desktop_tools/README.md) | 课表、下载、快捷启动、系统性能、服务器任务 |
| [island_gestures](plugins/island_gestures/README.md) | 两侧滚轮翻页、左右滑切 |
| [startup_log](plugins/startup_log/README.md) | 启动阶段日志与历史 |
| [tray_startup](plugins/tray_startup/README.md) | 自启参数、首次启动等待窗口 |
| [branding](plugins/branding/README.md) | 发布说明、打包脚本、GNU 资源编译 |

## 更新上游

上游更新频繁，本仓库按「上游原样 + plugins/」维护：

```powershell
git fetch upstream
git merge upstream/main
```

合并时只需要核对 [plugins/README.md](plugins/README.md) 里的接入点表（都带 `plugin:`
注释），`plugins/` 目录本身不受上游改动影响。

## 许可

继续沿用上游的 [MIT 许可证](LICENSE) 与 [第三方组件声明](THIRD_PARTY_NOTICES.txt)，
原作者的署名和许可条款保留。感谢原作者及上游贡献者。
