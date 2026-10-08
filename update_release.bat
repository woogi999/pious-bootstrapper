@echo off
setlocal EnableExtensions
rem ---------------------------------------------------------------------
rem  Pious - make a new installer and publish it as a GitHub release.
rem
rem    update_release.bat          release the version in Cargo.toml
rem    update_release.bat patch    1.0.0 -> 1.0.1 first
rem    update_release.bat minor    1.0.0 -> 1.1.0 first
rem    update_release.bat major    1.0.0 -> 2.0.0 first
rem
rem  The release notes are the "## <version>" section of CHANGELOG.md.
rem  It uploads Pious-Setup.exe (the installer), pious.exe and ffmpeg.zip,
rem  and installed copies of Pious update from it.
rem
rem  Needs Rust, Node.js, Git and the GitHub CLI (winget install GitHub.cli).
rem  The steps are in scripts\release.ps1.
rem ---------------------------------------------------------------------

cd /d "%~dp0"
title Pious release

powershell -NoProfile -ExecutionPolicy Bypass -File scripts\release.ps1 %1
if errorlevel 1 (
    echo.
    echo Release stopped. Nothing was published.
)
pause
