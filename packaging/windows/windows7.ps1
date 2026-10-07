<# Build on Windows 10/11 with the VS 2022 x64 C++ tools and Windows SDK.
   From the repository root: powershell -File packaging/windows/windows7.ps1
   The resulting portable ZIP targets Windows 7 SP1 x64 with an OpenGL 3.3 driver.
   The compiler runs on the build host, not on Windows 7. #>
param([string] $Toolchain = 'nightly-2026-10-01')
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$Target = 'x86_64-win7-windows-msvc'
function Invoke-Checked([scriptblock] $Command) {
  & $Command
  if ($LASTEXITCODE -ne 0) { throw "Command failed with exit code $LASTEXITCODE" }
}
Push-Location $Root
$FlagName = 'CARGO_TARGET_X86_64_WIN7_WINDOWS_MSVC_RUSTFLAGS'
$OldFlags = [Environment]::GetEnvironmentVariable($FlagName)
$OldWinres = $env:VECTORCRAFT_REQUIRE_WINRES
try {
  Invoke-Checked { rustup toolchain install $Toolchain --profile minimal --component rust-src }
  # The ordinary pc-windows-msvc standard library requires Windows 10. Rebuild std for win7.
  [Environment]::SetEnvironmentVariable($FlagName, '-C target-feature=+crt-static')
  $env:VECTORCRAFT_REQUIRE_WINRES = '1'
  $BuildArgs = @('-Z', 'build-std=std,panic_unwind', '--locked',
    '--target', $Target, '--no-default-features', '--features', 'vectorcraft/windows7',
    '-p', 'vectorcraft', '-p', 'vectorcraft-cli')
  Invoke-Checked { cargo "+$Toolchain" build @BuildArgs --release }
  $TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { 'target' }
  $Bin = Join-Path $TargetDir "$Target\release"
  foreach ($Name in 'vectorcraft.exe', 'vectorcraft-cli.exe') {
    $Exe = Join-Path $Bin $Name
    $Bytes = [IO.File]::ReadAllBytes($Exe)
    $Pe = [BitConverter]::ToInt32($Bytes, 0x3c)
    if ([BitConverter]::ToUInt16($Bytes, $Pe + 4) -ne 0x8664) { throw "$Name is not x64" }
    $Major = [BitConverter]::ToUInt16($Bytes, $Pe + 0x48)
    $Minor = [BitConverter]::ToUInt16($Bytes, $Pe + 0x4a)
    if ($Major -gt 6 -or ($Major -eq 6 -and $Minor -gt 1)) { throw "$Name requires subsystem $Major.$Minor" }
    $Imports = & dumpbin /nologo /imports $Exe
    if ($LASTEXITCODE -ne 0) { throw "Could not inspect $Name imports" }
    # Regression gate for known loader failures; actual Win7 runtime QA remains necessary.
    if ($Imports -match '(?i)\b(GetSystemTimePreciseAsFileTime|ProcessPrng|WaitOnAddress|WakeByAddressSingle|WakeByAddressAll)\b|combase\.dll|bcryptprimitives\.dll|d3d12\.dll|api-ms-win-core-path-') {
      throw "$Name directly imports a known post-Windows-7 API"
    }
  }
  Invoke-Checked { & (Join-Path $Bin 'vectorcraft-cli.exe') --version }
  $Stage = Join-Path $TargetDir ('windows7-portable-' + [Guid]::NewGuid().ToString('N'))
  New-Item -ItemType Directory -Force $Stage | Out-Null
  Copy-Item (Join-Path $Bin 'vectorcraft.exe'), (Join-Path $Bin 'vectorcraft-cli.exe') $Stage
  Copy-Item LICENSE-MIT, LICENSE-APACHE, NOTICE $Stage
  Copy-Item vendor/windows-link/license-mit (Join-Path $Stage 'LICENSE-windows-link-MIT')
  Copy-Item vendor/windows-link/license-apache-2.0 (Join-Path $Stage 'LICENSE-windows-link-Apache-2.0')
  Copy-Item docs/windows7.md (Join-Path $Stage 'WINDOWS7.md')
  if ($env:CRAFT_FONTS_DIR) {
    foreach ($ofl in Get-ChildItem (Join-Path $env:CRAFT_FONTS_DIR 'fonts\*\OFL.txt')) {
      Copy-Item $ofl.FullName (Join-Path $Stage "OFL-$($ofl.Directory.Name).txt")
    }
  }
  New-Item -ItemType Directory -Force dist/release | Out-Null
  $Zip = Join-Path $Root 'dist/release/vectorcraft-windows7-x64-portable.zip'
  Compress-Archive -Path (Join-Path $Stage '*') -DestinationPath $Zip -Force
  Remove-Item -Recurse -Force $Stage
  Get-Item $Zip
} finally {
  [Environment]::SetEnvironmentVariable($FlagName, $OldFlags)
  $env:VECTORCRAFT_REQUIRE_WINRES = $OldWinres
  Pop-Location
}
