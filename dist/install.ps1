# Memryzed installer for Windows (PowerShell).
#
# Usage:
#   irm https://memryzed.com/install.ps1 | iex
#
# Detects architecture, downloads the matching release archive from
# GitHub, verifies its SHA-256 checksum, installs the binary to
# %LOCALAPPDATA%\memryzed\bin\memryzed.exe, and adds that directory
# to the user PATH via the registry.
#
# Interim status: hand-written reference served from
# https://memryzed.com/install.ps1 until cargo-dist generates the
# canonical installer.
#
# Environment overrides:
#   MEMRYZED_VERSION  install a specific tag instead of latest

$ErrorActionPreference = "Stop"

$Repo = "memryzed/memryzed"
$InstallDir = Join-Path $env:LOCALAPPDATA "memryzed\bin"

function Fail($msg) { Write-Error "error: $msg"; exit 1 }

# Architecture detection.
$arch = $env:PROCESSOR_ARCHITECTURE
switch ($arch) {
  "AMD64" { $archPart = "x86_64" }
  "ARM64" { $archPart = "aarch64" }
  default { Fail "unsupported architecture: $arch" }
}
$target = "$archPart-pc-windows-msvc"

# Resolve version.
if ($env:MEMRYZED_VERSION) {
  $tag = $env:MEMRYZED_VERSION
} else {
  Write-Host "Resolving latest release..."
  $latest = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
  $tag = $latest.tag_name
  if (-not $tag) { Fail "could not determine latest release tag" }
}

$archive = "memryzed-$target.zip"
$base = "https://github.com/$Repo/releases/download/$tag"
$tmp = Join-Path $env:TEMP ("memryzed-" + [System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
  Write-Host "Downloading $archive ($tag)..."
  Invoke-WebRequest "$base/$archive" -OutFile (Join-Path $tmp $archive)
  Invoke-WebRequest "$base/$archive.sha256" -OutFile (Join-Path $tmp "$archive.sha256")

  Write-Host "Verifying checksum..."
  $expected = (Get-Content (Join-Path $tmp "$archive.sha256") -Raw).Split()[0].Trim().ToLower()
  $actual = (Get-FileHash (Join-Path $tmp $archive) -Algorithm SHA256).Hash.ToLower()
  if ($expected -ne $actual) { Fail "checksum mismatch (expected $expected, got $actual)" }

  Write-Host "Extracting..."
  Expand-Archive -Path (Join-Path $tmp $archive) -DestinationPath $tmp -Force

  New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
  Copy-Item (Join-Path $tmp "memryzed-$target\memryzed.exe") (Join-Path $InstallDir "memryzed.exe") -Force

  # Add to user PATH if missing.
  $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
  if ($userPath -notlike "*$InstallDir*") {
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$InstallDir", "User")
    Write-Host "Added $InstallDir to your user PATH."
  }

  Write-Host ""
  Write-Host "Memryzed $tag installed to $InstallDir\memryzed.exe"
  Write-Host ""
  Write-Host "Next:"
  Write-Host "  1. Open a new terminal so PATH changes take effect."
  Write-Host "  2. Initialize:           memryzed init"
  Write-Host "  3. Wire into your agent: memryzed install"
}
finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
