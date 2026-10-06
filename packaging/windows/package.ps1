<#
.SYNOPSIS
  Build, sign and package VectorCraft for Windows.

.DESCRIPTION
  Produces, in $env:DIST (default: dist/release):
    vectorcraft-<version>-windows-<arch>.msi            per-machine installer (WiX v5)
    vectorcraft-<version>-windows-<arch>-portable.zip   vectorcraft.exe + vectorcraft-cli.exe

  The binaries link the C runtime statically (+crt-static), so neither the MSI nor the portable
  zip needs the Visual C++ redistributable. Signing is delegated to sign.ps1 (skipped with a
  warning when no signing secrets are set).

  Needs: Rust (MSVC toolchain + the target), the Windows SDK (rc.exe, signtool.exe),
  and WiX v5: dotnet tool install --global wix --version 5.0.2

.EXAMPLE
  pwsh packaging/windows/package.ps1 -Arch x64
  pwsh packaging/windows/package.ps1 -Arch x86 -SkipBuild
#>
param(
  [ValidateSet('x64', 'x86')] [string] $Arch = 'x64',
  [switch] $SkipBuild
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

function Invoke-Native([string] $What, [scriptblock] $Block) {
  Write-Output "==> $What"
  & $Block
  if ($LASTEXITCODE -ne 0) { throw "$What failed with exit code $LASTEXITCODE" }
}

# The version lives in one place: [workspace.package] version in the root Cargo.toml.
$Version = $env:VECTORCRAFT_VERSION
if (-not $Version) {
  $inPkg = $false
  foreach ($line in Get-Content (Join-Path $Root 'Cargo.toml')) {
    if ($line -match '^\s*\[') { $inPkg = ($line.Trim() -eq '[workspace.package]'); continue }
    if ($inPkg -and $line -match '^\s*version\s*=\s*"([^"]+)"') { $Version = $Matches[1]; break }
  }
}
if (-not $Version) { throw 'could not read [workspace.package] version from Cargo.toml' }
# MSI ProductVersion is numeric (major.minor.build); pre-release tags are dropped there.
$MsiVersion = ($Version -split '-')[0]

$Target = if ($Arch -eq 'x64') { 'x86_64-pc-windows-msvc' } else { 'i686-pc-windows-msvc' }
$Dist = if ($env:DIST) { $env:DIST } else { Join-Path $Root 'dist\release' }
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
New-Item -ItemType Directory -Force -Path $Dist | Out-Null

if (-not $env:VECTORCRAFT_BUILD_SHA) { $env:VECTORCRAFT_BUILD_SHA = (git -C $Root rev-parse HEAD 2>$null) }
if (-not $env:VECTORCRAFT_BUILD_DATE) { $env:VECTORCRAFT_BUILD_DATE = (Get-Date).ToUniversalTime().ToString('yyyy-MM-dd') }

Write-Output "VectorCraft $Version for Windows $Arch ($Target)"

if (-not $SkipBuild) {
  # Static CRT: no VC++ redistributable needed. Scoped to the target so host build scripts and
  # proc-macros are unaffected.
  $flagVar = 'CARGO_TARGET_' + ($Target.ToUpper() -replace '-', '_') + '_RUSTFLAGS'
  [Environment]::SetEnvironmentVariable($flagVar, '-C target-feature=+crt-static')
  # Fail the build (rather than warn) if the icon/VERSIONINFO can't be embedded.
  $env:VECTORCRAFT_REQUIRE_WINRES = '1'
  Invoke-Native "cargo build ($Target)" { cargo build --release --locked -p vectorcraft -p vectorcraft-cli --target $Target }
}

$Bin = Join-Path $TargetDir "$Target\release"
$Stage = Join-Path $TargetDir "windows-package\$Arch"
Remove-Item -Recurse -Force $Stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Stage | Out-Null
Copy-Item (Join-Path $Bin 'vectorcraft.exe'), (Join-Path $Bin 'vectorcraft-cli.exe') $Stage

& (Join-Path $PSScriptRoot 'sign.ps1') (Join-Path $Stage 'vectorcraft.exe') (Join-Path $Stage 'vectorcraft-cli.exe')

# ---- MSI ---------------------------------------------------------------------------------------
$Msi = Join-Path $Dist "vectorcraft-$Version-windows-$Arch.msi"
Invoke-Native 'wix build' {
  wix build (Join-Path $PSScriptRoot 'vectorcraft.wxs') -arch $Arch `
    -d "Version=$MsiVersion" -d "BinDir=$Stage" -d "IconPath=$(Join-Path $Root 'assets\app-icon\vectorcraft.ico')" `
    -o $Msi
}
# wix writes its debug symbols (.wixpdb) next to the MSI; keep them out of the release assets.
Remove-Item -Force -ErrorAction SilentlyContinue ([IO.Path]::ChangeExtension($Msi, '.wixpdb'))
& (Join-Path $PSScriptRoot 'sign.ps1') $Msi

# ---- portable zip ------------------------------------------------------------------------------
$Portable = Join-Path $TargetDir "windows-package\vectorcraft-$Version-windows-$Arch-portable"
Remove-Item -Recurse -Force $Portable -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Portable | Out-Null
Copy-Item (Join-Path $Stage '*.exe') $Portable
foreach ($f in 'README.md', 'LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE') {
  $p = Join-Path $Root $f
  if (Test-Path $p) { Copy-Item $p $Portable }
}
# Built with craft-fonts (CRAFT_FONTS_DIR, release builds): the embedded fonts' licences.
if ($env:CRAFT_FONTS_DIR) {
  foreach ($ofl in Get-ChildItem -Path (Join-Path $env:CRAFT_FONTS_DIR 'fonts\*\OFL.txt') -ErrorAction SilentlyContinue) {
    Copy-Item $ofl.FullName (Join-Path $Portable "OFL-$($ofl.Directory.Name).txt")
  }
}
$Zip = Join-Path $Dist "vectorcraft-$Version-windows-$Arch-portable.zip"
Remove-Item -Force $Zip -ErrorAction SilentlyContinue
Compress-Archive -Path $Portable -DestinationPath $Zip

Invoke-Native 'vectorcraft-cli --version' { & (Join-Path $Stage 'vectorcraft-cli.exe') --version }
Get-Item $Msi, $Zip | Format-Table Name, Length
