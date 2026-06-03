# Memryzed installer for Windows (PowerShell 5.1+).
#
# Usage:
#   irm https://memryzed.com/install.ps1 | iex
#
# Detects architecture, downloads the matching release archive from
# memryzed.com, verifies its SHA-256 checksum, installs the binary to
# %LOCALAPPDATA%\memryzed\bin\memryzed.exe, and adds that directory to
# the user PATH.
#
# Uses only built-in PowerShell cmdlets, so no prerequisites are
# needed to run the script. The installed binary may require the
# Microsoft Visual C++ Redistributable; this script checks and warns.
#
# Environment overrides:
#   MEMRYZED_VERSION  install a specific version instead of latest

$ErrorActionPreference = "Stop"

$BaseUrl = "https://memryzed.com/releases"
$InstallDir = Join-Path $env:LOCALAPPDATA "memryzed\bin"

function Fail($msg) { Write-Error "error: $msg"; exit 1 }

# Architecture detection.
switch ($env:PROCESSOR_ARCHITECTURE) {
  "AMD64" { $archPart = "x86_64" }
  "ARM64" { $archPart = "aarch64" }
  default { Fail "unsupported architecture: $env:PROCESSOR_ARCHITECTURE" }
}
$target = "$archPart-pc-windows-msvc"

# Resolve version from memryzed.com/releases/latest.txt.
if ($env:MEMRYZED_VERSION) {
  $version = $env:MEMRYZED_VERSION
} else {
  Write-Host "Resolving latest version..."
  $version = (Invoke-WebRequest "$BaseUrl/latest.txt" -UseBasicParsing).Content.Trim()
  if (-not $version) { Fail "could not determine the latest version" }
}

$archive = "memryzed-$target.zip"
$base = "$BaseUrl/v$version"
$tmp = Join-Path $env:TEMP ("memryzed-" + [System.Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
  Write-Host "Downloading $archive (v$version)..."
  try {
    Invoke-WebRequest "$base/$archive" -OutFile (Join-Path $tmp $archive) -UseBasicParsing
    Invoke-WebRequest "$base/$archive.sha256" -OutFile (Join-Path $tmp "$archive.sha256") -UseBasicParsing
  } catch {
    Fail "download failed; no asset for $target in v$version"
  }

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

  # The ONNX Runtime used by the embedder may need the VC++ runtime.
  $vcRuntime = Join-Path $env:SystemRoot "System32\vcruntime140.dll"
  if (-not (Test-Path $vcRuntime)) {
    Write-Host ""
    Write-Host "Note: the Microsoft Visual C++ Redistributable was not detected."
    Write-Host "If 'memryzed' fails to start with a missing-DLL error, install it:"
    Write-Host "  winget install Microsoft.VCRedist.2015+.x64"
  }

  Write-Host ""
  Write-Host "Memryzed v$version installed to $InstallDir\memryzed.exe"
  Write-Host ""
  Write-Host "Next:"
  Write-Host "  1. Open a new terminal so PATH changes take effect."
  Write-Host "  2. Initialize:           memryzed init"
  Write-Host "  3. Wire into your agent: memryzed install"
}
finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}
