# Downloads the Wintun driver (wintun.dll) required by the VPN data plane and
# places it next to the Tauri project so it is bundled into the installer.
#
# Run this once before `npm run tauri dev` or `npm run tauri build`:
#   ./scripts/fetch-wintun.ps1

$ErrorActionPreference = "Stop"

$version = "0.14.1"
$url = "https://www.wintun.net/builds/wintun-$version.zip"

$appRoot = Split-Path -Parent $PSScriptRoot
$dest = Join-Path $appRoot "src-tauri/wintun.dll"
$zip = Join-Path $env:TEMP "wintun-$version.zip"
$extract = Join-Path $env:TEMP "wintun-$version"

if (Test-Path $dest) {
    Write-Host "wintun.dll already present at $dest"
    exit 0
}

Write-Host "Downloading Wintun $version..."
Invoke-WebRequest -Uri $url -OutFile $zip

if (Test-Path $extract) { Remove-Item $extract -Recurse -Force }
Expand-Archive $zip -DestinationPath $extract -Force

Copy-Item (Join-Path $extract "wintun/bin/amd64/wintun.dll") $dest -Force
Write-Host "Wintun driver placed at $dest"
