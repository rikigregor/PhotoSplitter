@echo off
cd /d "%~dp0"
cargo test
if errorlevel 1 exit /b 1
cargo build --release
pause
