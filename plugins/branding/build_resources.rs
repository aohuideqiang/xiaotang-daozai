// GNU 工具链的资源编译：没有 Windows SDK 的 `rc.exe` 时，用 `windres` 编同一份 `app.rc`，
// 图标和版本信息与 MSVC 构建一致。
//
// 由 `build.rs` 用 `include!` 引入，直接使用它已经导入的 `Command`。

/// 用 windres 生成 COFF 目标文件并交给链接器；不适用或失败时返回 `false`。
fn gnu_resources(out: &std::path::Path) -> bool {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu") {
        return false;
    }
    let object = out.with_extension("o");
    let windres = std::env::var_os("WINDRES").unwrap_or_else(|| "windres.exe".into());
    let status = Command::new(windres)
        .args(["--codepage=65001", "-O", "coff", "-i", "app.rc", "-o"])
        .arg(&object)
        .status();
    if !status.map(|status| status.success()).unwrap_or(false) {
        return false;
    }
    println!("cargo:rustc-link-arg={}", object.display());
    true
}
