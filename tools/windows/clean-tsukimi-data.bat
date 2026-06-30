@echo off
setlocal

rem Runs the PowerShell cleanup script beside this file.
rem No administrator privileges are required. Confirmation is requested unless
rem /Force or -Force is supplied.

set "FORCE_ARGUMENT="
if "%~1"=="" goto run
if /I "%~1"=="/Force" (
  set "FORCE_ARGUMENT=-Force"
  goto run
)
if /I "%~1"=="-Force" (
  set "FORCE_ARGUMENT=-Force"
  goto run
)

echo Usage: %~nx0 [/Force]
exit /b 2

:run
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" ^
  -NoProfile ^
  -ExecutionPolicy Bypass ^
  -File "%~dp0clean-tsukimi-data.ps1" %FORCE_ARGUMENT%
exit /b %ERRORLEVEL%
