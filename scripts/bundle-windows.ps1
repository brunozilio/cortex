<# Build the x64 Windows app and a per-machine MSI. Requires Python 3,
   VS Build Tools, Rust MSVC and WiX v5 (dotnet tool install --global wix --version 5.0.2).
   With the Windows engine's release pin the browser ships too; without it (no
   engine published yet) the MSI ships without the browser. #>
[CmdletBinding()]
param(
    [string]$EngineLock = "scripts/cef-release.windows-x86_64.lock.json",
    [string]$VCRuntimeDir = "",
    [switch]$SkipBuild
)
$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
Push-Location (Join-Path $PSScriptRoot "..")
try {
    $EngineArgs = @()
    if (Test-Path $EngineLock -PathType Leaf) {
        $EngineLock = (Resolve-Path $EngineLock).Path
        $EnginePath = & python scripts/fetch_engine.py --lock $EngineLock --target x86_64-pc-windows-msvc
        if ($LASTEXITCODE -ne 0) { throw "Could not fetch the pinned Windows engine." }
        $env:CEF_PATH = $EnginePath.Trim()
        $EngineArgs = @("--engine", $env:CEF_PATH, "--lock", $EngineLock)
    } else {
        Write-Warning "No Windows engine is published ($EngineLock): packaging without the browser."
    }
    if (-not $VCRuntimeDir) {
        $VsWhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio/Installer/vswhere.exe"
        $VsRoot = & $VsWhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($LASTEXITCODE -ne 0 -or -not $VsRoot) { throw "Visual Studio C++ tools were not found." }
        $Crt = Get-ChildItem "$VsRoot/VC/Redist/MSVC/*/x64/Microsoft.VC*.CRT" -Directory |
            Sort-Object FullName -Descending | Select-Object -First 1
        if (-not $Crt) { throw "Visual C++ x64 redistributable was not found; supply -VCRuntimeDir." }
        $VCRuntimeDir = $Crt.FullName
    }
    if (-not $SkipBuild) {
        & cargo build --release --locked --target x86_64-pc-windows-msvc -p superfik-app
        if ($LASTEXITCODE -ne 0) { throw "Windows app build failed." }
    }
    $Version = ((Get-Content Cargo.toml | Select-String '^version = "([^"]+)"' | Select-Object -First 1).Matches.Groups[1].Value)
    if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "MSI requires a stable three-part version: $Version" }
    $TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { "target" }
    $ReleaseDir = Join-Path $TargetDir "x86_64-pc-windows-msvc/release"
    $Out = Join-Path $ReleaseDir "bundle"
    New-Item $Out -ItemType Directory -Force | Out-Null
    $Stage = Join-Path $Out ("windows-payload-" + [guid]::NewGuid().ToString("N"))
    & cargo run -q -p xtask -- notices (Join-Path $ReleaseDir "THIRD_PARTY_NOTICES.md")
    if ($LASTEXITCODE -ne 0) { throw "Generating third-party notices failed." }
    try {
        & python scripts/package_windows.py --binary (Join-Path $ReleaseDir "superfik.exe") @EngineArgs --output $Stage --vc-runtime $VCRuntimeDir
        if ($LASTEXITCODE -ne 0) { throw "Windows payload validation failed." }
        $Msi = Join-Path $Out "Superfik-$Version-x64.msi"
        & wix build assets/windows/superfik.wxs -arch x64 -d "Version=$Version" -d "Payload=$((Resolve-Path $Stage).Path)" -d "Icon=$((Resolve-Path assets/windows/superfik.ico).Path)" -o $Msi
        if ($LASTEXITCODE -ne 0) { throw "WiX MSI build failed." }
        $Hash = (Get-FileHash -Algorithm SHA256 $Msi).Hash.ToLowerInvariant()
        "$Hash  $([IO.Path]::GetFileName($Msi))" | Set-Content "$Msi.sha256" -Encoding ascii
        Write-Output (Resolve-Path $Msi).Path
    } finally {
        if (Test-Path $Stage) { Remove-Item $Stage -Recurse -Force }
    }
} finally {
    Pop-Location
}
