# Starts a Pious release. Run by update_release.bat; see there for how to use it.
#
#   1. Version: the one in src-tauri\Cargo.toml, or bumped first (patch,
#      minor, major). If that tag already exists, you're asked to bump.
#   2. Notes:   the "## <version>" section of CHANGELOG.md.
#   3. Check:   the interface (npm run check).
#   4. Publish: commit, tag vX.Y.Z and push. GitHub Actions
#      (.github/workflows/release.yml) then builds and publishes the release:
#      manifest.json, pious-X.Y.Z-win-x64.zip, Pious-Setup.exe and pious.exe
#      (FFmpeg is built into pious.exe).
#
# With -Local nothing is committed, tagged or pushed: the same four files
# are built into release\ on this PC, for testing.
#
# Every step stops the whole release when it fails; nothing loops.
param([string]$Bump = "", [switch]$Local)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $root
$out = Join-Path $root "release"
$cargoToml = Join-Path $root "src-tauri\Cargo.toml"

function Step($text) { Write-Host ""; Write-Host "== $text" -ForegroundColor Cyan }
function Stop-Release($text) { throw $text }
# Whether a command succeeds, without its output. (Redirecting a program's
# errors inside Windows PowerShell would stop the script.)
function Test-Quiet([string]$command) {
    cmd /c "$command >nul 2>nul"
    $LASTEXITCODE -eq 0
}
function Run($what, [scriptblock]$block) {
    & $block
    if ($LASTEXITCODE -ne 0) { Stop-Release "$what failed (see above)." }
}

# ── Tools ────────────────────────────────────────────────────────────────
Step "Checking tools"
$tools = @("git", "npm")
if ($Local) { $tools += "cargo" }
foreach ($tool in $tools) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        Stop-Release "$tool isn't installed. Rust: https://rustup.rs  Node.js: https://nodejs.org  Git: https://git-scm.com"
    }
}
if (-not $Local) {
    $branch = (git rev-parse --abbrev-ref HEAD).Trim()
    if ($branch -ne "main") { Stop-Release "You're on '$branch'. Switch to main first." }
}

# ── Version ──────────────────────────────────────────────────────────────
function Get-Version {
    [regex]::Match([IO.File]::ReadAllText($cargoToml), '(?m)^version = "([^"]+)"').Groups[1].Value
}

# The installer is the same program for every release, so its version stays.
function Set-Version([string]$part) {
    $v = (Get-Version).Split(".") | ForEach-Object { [int]$_ }
    switch ($part) {
        "major" { $v = @($v[0] + 1, 0, 0) }
        "minor" { $v = @($v[0], $v[1] + 1, 0) }
        default { $v = @($v[0], $v[1], $v[2] + 1) }
    }
    $new = $v -join "."
    $first = [regex]'(?m)^version = "[^"]*"'
    $path = Join-Path $root "src-tauri\Cargo.toml"
    [IO.File]::WriteAllText($path, $first.Replace([IO.File]::ReadAllText($path), "version = `"$new`"", 1))
    $json = [regex]'"version":\s*"[^"]*"'
    foreach ($file in "package.json", "src-tauri\tauri.conf.json") {
        $path = Join-Path $root $file
        [IO.File]::WriteAllText($path, $json.Replace([IO.File]::ReadAllText($path), "`"version`": `"$new`"", 1))
    }
    $new
}

# A version is taken once its tag exists here or on GitHub.
function Test-Released([string]$version) {
    if (Test-Quiet "git rev-parse -q --verify refs/tags/v$version") { return $true }
    $remote = git ls-remote --tags origin "refs/tags/v$version"
    return [bool]$remote
}

if ($Bump) {
    if ($Bump -notin "patch", "minor", "major") { Stop-Release "Use patch, minor or major (or nothing)." }
    $version = Set-Version $Bump
} else {
    $version = Get-Version
}
if (-not $Local -and (Test-Released $version)) {
    Write-Host ""
    Write-Host "Pious $version is already tagged." -ForegroundColor Yellow
    $answer = Read-Host "Release it as the next version instead? [P]atch / [M]inor / [N]o"
    switch -regex ($answer) {
        "^[pP]" { $version = Set-Version "patch" }
        "^[mM]" { $version = Set-Version "minor" }
        default { Stop-Release "Nothing was released." }
    }
    if (Test-Released $version) { Stop-Release "Pious $version is tagged too. Bump the version in src-tauri\Cargo.toml." }
}
Write-Host "Releasing Pious $version$(if ($Local) { ' (local build, nothing is published)' })" -ForegroundColor Green

# ── Release notes ────────────────────────────────────────────────────────
function Get-Notes([string]$version) {
    $lines = [IO.File]::ReadAllLines((Join-Path $root "CHANGELOG.md"))
    $inside = $false
    $notes = foreach ($line in $lines) {
        if ($line -match '^##\s+(\S+)\s*$') {
            if ($inside) { break }
            $inside = $Matches[1] -eq $version
            continue
        }
        if ($inside) { $line }
    }
    ($notes -join "`n").Trim()
}

Step "Release notes (CHANGELOG.md)"
$notes = Get-Notes $version
if (-not $notes -or $notes -eq "-") {
    $changelog = Join-Path $root "CHANGELOG.md"
    $text = [IO.File]::ReadAllText($changelog).Replace("`r`n", "`n")
    if ($text -notmatch "(?m)^## $([regex]::Escape($version))\s*$") {
        $at = $text.IndexOf("`n## ")
        if ($at -lt 0) { $at = $text.Length }
        [IO.File]::WriteAllText($changelog, $text.Insert($at, "`n## $version`n`n- `n").Replace("`n", "`r`n"))
    }
    Write-Host "CHANGELOG.md has nothing under '## $version'. Write what changed, save, and close Notepad."
    Start-Process notepad $changelog -Wait
    $notes = Get-Notes $version
    if (-not $notes -or $notes -eq "-") { Stop-Release "No release notes, so nothing was released." }
}
Write-Host $notes

# ── Check (and, for -Local, build) ───────────────────────────────────────
Step "Interface dependencies"
$lock = Join-Path $root "package-lock.json"
$installed = Join-Path $root "node_modules\.package-lock.json"
if (-not (Test-Path $installed) -or (Get-Item $lock).LastWriteTime -gt (Get-Item $installed).LastWriteTime) {
    Run "npm install" { npm install --no-audit --no-fund --loglevel=error }
} else {
    Write-Host "Up to date."
}

Step "Checking the interface"
Run "The interface check" { npm run check }

if ($Local) {
    Step "FFmpeg (built into pious.exe)"
    $vendored = Join-Path $root "installer\vendor\ffmpeg.exe"
    if (-not (Test-Path $vendored)) {
        # A copy this PC already has saves the 100 MB download.
        $have = @("Bootstrapper", "Library") | ForEach-Object { Join-Path $env:LOCALAPPDATA "Pious\$_\tools\ffmpeg.exe" } | Where-Object { Test-Path $_ } | Select-Object -First 1
        if ($have) {
            New-Item -ItemType Directory -Force (Split-Path $vendored) | Out-Null
            Copy-Item $have $vendored
        }
    }
    & (Join-Path $PSScriptRoot "get-ffmpeg.ps1")

    Step "Building Pious (the first build takes a while, later ones are quick)"
    Run "The Pious build" { npm run tauri build -- --no-bundle }

    Step "Building the installer"
    # The installer keeps its own build folder, so the two builds don't
    # undo each other's work.
    $env:CARGO_TARGET_DIR = Join-Path $root "installer\target"
    try {
        Run "The installer build" { cargo build --release --manifest-path (Join-Path $root "installer\Cargo.toml") }
    } finally {
        Remove-Item Env:\CARGO_TARGET_DIR -ErrorAction SilentlyContinue
    }

    Step "Packaging the release files into release\"
    New-Item -ItemType Directory -Force $out | Out-Null
    [IO.File]::WriteAllText((Join-Path $out "notes.md"), $notes)
    & (Join-Path $PSScriptRoot "package-release.ps1") -Version $version `
        -App (Join-Path $root "src-tauri\target\release\pious.exe") `
        -Setup (Join-Path $root "installer\target\release\pious-setup.exe") `
        -Out $out
    Write-Host ""
    Write-Host "Local build done: $out (nothing was committed, tagged or published)." -ForegroundColor Green
    return
}

# ── Publish: the tag starts the GitHub Actions build ──────────────────────
Step "Tagging Pious $version"
Run "git add" { git add -A }
git diff --cached --quiet
if ($LASTEXITCODE -ne 0) { Run "git commit" { git commit -q -m "Release v$version" } }
Run "git tag" { git tag "v$version" }
Run "git push" { git push origin main }
Run "Pushing the tag" { git push origin "v$version" }
Write-Host ""
Write-Host "Pious $version was tagged and pushed. GitHub Actions is building the release now;" -ForegroundColor Green
Write-Host "installed copies offer it once it appears (a few minutes)."
$remote = (git remote get-url origin).Trim() -replace '\.git$', ''
Write-Host "$remote/actions"
