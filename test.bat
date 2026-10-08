@echo off
setlocal EnableExtensions
rem ---------------------------------------------------------------------
rem  Pious - build and run it for testing.
rem
rem  A development build: quick to build, and changes to the interface
rem  (src\) show up live without a restart. Close Pious, or press Ctrl C
rem  here, to stop. Rust changes rebuild and restart it by themselves.
rem
rem  Needs Rust (https://rustup.rs) and Node.js (https://nodejs.org).
rem ---------------------------------------------------------------------

cd /d "%~dp0"
title Pious (test)

where cargo >nul 2>nul || (echo Rust isn't installed. Get it from https://rustup.rs, then run this again. & pause & exit /b 1)
where npm >nul 2>nul || (echo Node.js isn't installed. Get it from https://nodejs.org, then run this again. & pause & exit /b 1)

rem Interface dependencies, only when they're missing or out of date.
powershell -NoProfile -Command "$l='package-lock.json'; $i='node_modules\.package-lock.json'; if ((Test-Path $i) -and (Get-Item $l).LastWriteTime -le (Get-Item $i).LastWriteTime) { exit 0 } else { exit 1 }"
if errorlevel 1 (
    echo Installing interface dependencies...
    call npm install --no-audit --no-fund --loglevel=error || (pause & exit /b 1)
)

call npm run tauri dev
if errorlevel 1 pause
