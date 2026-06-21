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

$openvpnBin = "C:\Program Files\OpenVPN\bin"
$openvpnExe = Join-Path $openvpnBin "openvpn.exe"

if (-not (Test-Path $openvpnExe)) {
  if (Get-Command choco -ErrorAction SilentlyContinue) {
    Write-Host "Installing OpenVPN via Chocolatey ..."
    choco install openvpn --no-progress -y | Out-Host
  } else {
    throw @"
OpenVPN client not found at $openvpnExe and Chocolatey is unavailable.
Install OpenVPN (https://openvpn.net/community-downloads/) and re-run, or run
this on CI where Chocolatey is present.
"@
  }
}

if (-not (Test-Path $openvpnExe)) {
  throw "OpenVPN install did not produce $openvpnExe"
}

Write-Host "Staging OpenVPN client from $openvpnBin ..."
# Copy the client binaries (openvpn.exe + its runtime DLLs). The driver service
# component is intentionally not bundled; we use the userspace Wintun adapter.
Copy-Item (Join-Path $openvpnBin "*") -Destination $destDir -Recurse -Force

# Ensure a Wintun DLL is present for OpenVPN's data plane.
if (Test-Path $wintun) {
  Copy-Item $wintun -Destination (Join-Path $destDir "wintun.dll") -Force
} else {
  Write-Warning "wintun.dll not found at $wintun; run scripts/fetch-wintun.ps1 first."
}

$staged = Join-Path $destDir "openvpn.exe"
if (-not (Test-Path $staged)) { throw "Failed to stage openvpn.exe at $staged" }
Write-Host "Staged OpenVPN client at $destDir"
