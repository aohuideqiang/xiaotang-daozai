# KarMa-岛仔：用 GNU 工具链打包
# 用法：pwsh -File plugins/branding/build.ps1 [-ToolchainRoot <工具链目录>]
#
# 默认使用项目里的 .tools（cargo / rustup / w64devkit），也可以用环境变量
# CARGO_HOME / RUSTUP_HOME 指向自己的工具链。
param(
    [string]$ToolchainRoot = (Join-Path (Resolve-Path (Join-Path $PSScriptRoot '..\..')) '.tools'),
    [string]$Name = 'KarMa-岛仔.exe'
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')

if (Test-Path (Join-Path $ToolchainRoot 'cargo\bin\cargo.exe')) {
    $env:CARGO_HOME = Join-Path $ToolchainRoot 'cargo'
    $env:RUSTUP_HOME = Join-Path $ToolchainRoot 'rustup'
    $env:PATH = (Join-Path $ToolchainRoot 'cargo\bin') + ';' + (Join-Path $ToolchainRoot 'w64devkit\bin') + ';' + $env:PATH
}

Push-Location $root
try {
    & cargo build --locked --release --bin daozai --target x86_64-pc-windows-gnu
    if ($LASTEXITCODE -ne 0) { throw "编译失败（退出码 $LASTEXITCODE）" }

    $built = Join-Path $root 'target\x86_64-pc-windows-gnu\release\daozai.exe'
    if (-not (Test-Path $built)) { throw "没有找到 $built" }

    $output = Join-Path $root $Name
    Copy-Item $built $output -Force
    $hash = (Get-FileHash $output -Algorithm SHA256).Hash
    Write-Host "已生成 $output"
    Write-Host "SHA256 $hash"
}
finally {
    Pop-Location
}
