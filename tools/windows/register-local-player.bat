@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0register-local-player.ps1" %*
if errorlevel 1 (
    echo Registration failed.
    pause
    exit /b 1
)
pause
