@echo off
REM Memryzed installer for Windows Command Prompt.
REM
REM Usage:
REM   curl -fsSL https://memryzed.com/install.cmd -o install.cmd ^&^& install.cmd
REM
REM Batch is unsuited to HTTPS download and architecture detection,
REM so this shim bootstraps PowerShell to run install.ps1.
powershell -NoProfile -ExecutionPolicy Bypass -Command "irm https://memryzed.com/install.ps1 | iex"
