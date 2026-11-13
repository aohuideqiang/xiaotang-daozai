# KarMa 定制发布

本目录存放和二开发布有关、但不属于运行逻辑的东西：发布说明、打包脚本，以及没有
Windows SDK 时的资源编译。

## 版本

- 上游基线：**v0.6.64**（`upstream/main`），二开以插件形式叠加，核心代码尽量保持上游原样；
- 桌面程序名：`KarMa-岛仔.exe`（构建后由 `build.ps1` 重命名）；
- 仓库：<https://github.com/aohuideqiang/karma-daozai>，上游：<https://github.com/Renly1994/xiaotang-daozai>。

## 打包

```powershell
pwsh -File plugins/branding/build.ps1
```

脚本用 GNU 工具链编译 release，并把 `target/x86_64-pc-windows-gnu/release/daozai.exe`
复制成 `KarMa-岛仔.exe`。等价的原始命令：

```powershell
cargo build --locked --release --bin daozai --target x86_64-pc-windows-gnu
```

没有 `rc.exe`（Windows SDK）时，`build.rs` 会用 `windres` 编译 `app.rc`，图标和版本
信息与 MSVC 构建一致；见 `build_resources.rs`。

## 许可证

继续沿用上游的 MIT 许可证与版权声明（根目录 `LICENSE`），并保留
`THIRD_PARTY_NOTICES.txt`。`KarMa-岛仔` 是这个定制版本的名称，不代表原作者发布或背书。
