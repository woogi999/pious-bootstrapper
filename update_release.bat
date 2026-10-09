@echo off
setlocal EnableExtensions
rem ---------------------------------------------------------------------
rem  Pious - start a release.
rem
rem    update_release.bat          release the version in Cargo.toml (if it's
rem                                already released, Enter picks the next patch)
rem    update_release.bat patch    1.0.0 -> 1.0.1 first
rem    update_release.bat minor    1.0.0 -> 1.1.0 first
rem    update_release.bat major    1.0.0 -> 2.0.0 first
rem    update_release.bat -Local   build the release files into release\
rem                                on this PC (nothing is published);
rem                                can be combined: patch -Local
rem
rem  The release notes are the "## <version>" section of CHANGELOG.md.
rem  This bumps the version, commits, tags vX.Y.Z and pushes the tag;
rem  GitHub Actions then builds and publishes the release (manifest.json,
rem  pious-X.Y.Z-win-x64.zip, Pious-Setup.exe, pious.exe; FFmpeg is built
rem  into pious.exe), and installed copies of Pious update from it.
rem
rem  Needs Git (and Rust + Node.js for -Local). The steps are in
rem  scripts\release.ps1; the build is .github\workflows\release.yml.
rem ---------------------------------------------------------------------

cd /d "%~dp0"
title Pious release

powershell -NoProfile -ExecutionPolicy Bypass -File scripts\release.ps1 %*
if errorlevel 1 (
    echo.
    echo Release stopped. Nothing was published.
)
pause
