<#
  Stages the OpenVPN client where the Tauri bundler expects it
  (app/src-tauri/openvpn/), so the installer ships a turnkey client for the
  Public VPN mode.

  On CI (and locally if Chocolatey is present) this installs the official
  OpenVPN community build and copies its client binaries (openvpn.exe + DLLs).
  The Wintun data-plane DLL is copied alongside so OpenVPN can create its
  adapter without a separately-installed driver.

  OpenVPN is GPLv2 (https://openvpn.net/community/). Bundling it is permitted;
  the project ships its source/offer per the license in docs/PUBLIC_VPN.md.

  SECURITY_NOTE: Chocolatey verifies the OpenVPN package checksum on install.
  If you stage binaries another way, verify the publisher signature on
  openvpn.exe before bundling.
#>
$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptDir
$destDir = Join-Path $repoRoot "app/src-tauri/openvpn"
$wintun = Join-Path $repoRoot "app/src-tauri/wintun.dll"

New-Item -ItemType Directory -Force -Path $destDir | Out-Null

function Find-OpenVpnBin {
  foreach ($base in @("$env:ProgramFiles\OpenVPN\bin", "${env:ProgramFiles(x86)}\OpenVPN\bin")) {
    if ($base -and (Test-Path (Join-Path $base "openvpn.exe"))) { return $base }
  }
  return $null
}

$bin = Find-OpenVpnBin
if (-not $bin) {
  if (Get-Command choco -ErrorAction SilentlyContinue) {
    Write-Host "Installing OpenVPN via Chocolatey ..."
    # choco may exit non-zero if the (unneeded) TAP driver step is finicky; we
    # only need the client files, so success is verified by file presence below.
    & choco install openvpn --no-progress -y 2>&1 | Out-Host
    $bin = Find-OpenVpnBin
  }
}
if (-not $bin) {
  throw @"
OpenVPN client could not be found or installed.
On CI, ensure Chocolatey is available (it is on windows-latest runners).
Locally, install OpenVPN from https://openvpn.net/community-downloads/ and re-run.
"@
}

Write-Host "Staging OpenVPN client from $bin ..."
# Copy the client binaries (openvpn.exe + its runtime DLLs). The driver/service
# components are intentionally not bundled; we use the userspace Wintun adapter.
Copy-Item (Join-Path $bin "*") -Destination $destDir -Recurse -Force

# Ensure a Wintun DLL is present for OpenVPN's data plane.
if (Test-Path $wintun) {
  Copy-Item $wintun -Destination (Join-Path $destDir "wintun.dll") -Force
} else {
  Write-Warning "wintun.dll not found at $wintun; run scripts/fetch-wintun.ps1 first."
}

$staged = Join-Path $destDir "openvpn.exe"
if (-not (Test-Path $staged)) { throw "Failed to stage openvpn.exe at $staged" }

$count = (Get-ChildItem $destDir -File).Count
Write-Host "Staged $count OpenVPN client files at $destDir"
