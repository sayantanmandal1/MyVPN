<#
  Fetches the official Wintun driver and places the x64 wintun.dll where the
  Tauri bundler expects it (app/src-tauri/wintun.dll).

  Wintun is produced and signed by WireGuard LLC: https://www.wintun.net

  SECURITY_NOTE: the download uses HTTPS, so TLS authenticates www.wintun.net.
  For supply-chain hardening, pin the expected hash by setting the WINTUN_SHA256
  environment variable; the script then fails closed on any mismatch.
#>
$ErrorActionPreference = "Stop"

$version = "0.14.1"
$url = "https://www.wintun.net/builds/wintun-$version.zip"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Split-Path -Parent $scriptDir
$destDir = Join-Path $repoRoot "app/src-tauri"
$destDll = Join-Path $destDir "wintun.dll"

$tmpZip = Join-Path $env:TEMP "wintun-$version.zip"
$tmpDir = Join-Path $env:TEMP "wintun-$version-extract"

Write-Host "Downloading Wintun $version ..."
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Invoke-WebRequest -Uri $url -OutFile $tmpZip -UseBasicParsing

$sha = (Get-FileHash $tmpZip -Algorithm SHA256).Hash.ToLower()
Write-Host "SHA-256: $sha"
if ($env:WINTUN_SHA256) {
  if ($sha -ne $env:WINTUN_SHA256.ToLower()) {
    throw "Wintun SHA-256 mismatch: expected $($env:WINTUN_SHA256), got $sha"
  }
  Write-Host "SHA-256 verified against WINTUN_SHA256."
}

if (Test-Path $tmpDir) { Remove-Item $tmpDir -Recurse -Force }
Expand-Archive -Path $tmpZip -DestinationPath $tmpDir -Force

$src = Join-Path $tmpDir "wintun/bin/amd64/wintun.dll"
if (-not (Test-Path $src)) { throw "amd64/wintun.dll not found in the Wintun archive" }

New-Item -ItemType Directory -Force -Path $destDir | Out-Null
Copy-Item $src -Destination $destDll -Force
Write-Host "Placed wintun.dll at $destDll"
