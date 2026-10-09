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
# Versions are always three numbers (1.0.1): Cargo refuses anything else.
$semver = '^\d+\.\d+\.\d+$'

function Get-Version {
    [regex]::Match([IO.File]::ReadAllText($cargoToml), '(?m)^version = "([^"]+)"').Groups[1].Value
}

function Compare-Version([string]$a, [string]$b) {
    ([version]$a).CompareTo([version]$b)
}

# Writes `$new` into Cargo.toml, package.json and tauri.conf.json. (The
# installer is the same program for every release, so its version stays.)
function Write-Version([string]$new) {
    $first = [regex]'(?m)^version = "[^"]*"'
    [IO.File]::WriteAllText($cargoToml, $first.Replace([IO.File]::ReadAllText($cargoToml), "version = `"$new`"", 1))
    $json = [regex]'"version":\s*"[^"]*"'
    foreach ($file in "package.json", "src-tauri\tauri.conf.json") {
        $path = Join-Path $root $file
        [IO.File]::WriteAllText($path, $json.Replace([IO.File]::ReadAllText($path), "`"version`": `"$new`"", 1))
    }
    $new
}

function Get-Bumped([string]$from, [string]$part) {
    $v = $from.Split(".") | ForEach-Object { [int]$_ }
    # Parentheses matter: in PowerShell `a, b + 1` is `(a, b) + 1`, which
    # appends a number (1.0.0 became 1.0.0.1).
    switch ($part) {
        "major" { $v = @(($v[0] + 1), 0, 0) }
        "minor" { $v = @($v[0], ($v[1] + 1), 0) }
        default { $v = @($v[0], $v[1], ($v[2] + 1)) }
    }
    $v -join "."
}

# A version is taken once its tag exists here or on GitHub.
function Test-Released([string]$version) {
    if (Test-Quiet "git rev-parse -q --verify refs/tags/v$version") { return $true }
    $remote = git ls-remote --tags origin "refs/tags/v$version"
    return [bool]$remote
}

# The newest released version (from the tags here and on GitHub).
function Get-LastReleased {
    $tags = @(git tag --list "v*") + @(git ls-remote --tags origin "refs/tags/v*" | ForEach-Object { ($_ -split "refs/tags/")[1] })
    $tags | ForEach-Object { "$_".Trim() -replace '^v', '' -replace '\^\{\}$', '' } | Where-Object { $_ -match $semver } |
        Sort-Object { [version]$_ } -Descending | Select-Object -First 1
}

# Every "## x.y.z" in CHANGELOG.md that has real notes, newest first.
function Get-ChangelogVersions {
    $lines = [IO.File]::ReadAllLines((Join-Path $root "CHANGELOG.md"))
    $current = $null
    $body = @{}
    foreach ($line in $lines) {
        if ($line -match '^##\s+(\S+)\s*$') {
            $current = $Matches[1]
            $body[$current] = ""
            continue
        }
        if ($current) { $body[$current] += $line.Trim() }
    }
    $body.Keys | Where-Object { $_ -match $semver -and $body[$_] -and $body[$_] -ne "-" } | Sort-Object { [version]$_ } -Descending
}

$found = Get-Version
$last = Get-LastReleased
if ($found -notmatch $semver) {
    # Like "1.0.0.1": put it back to the last release, then pick below.
    Write-Host "The version in src-tauri\Cargo.toml ($found) isn't a valid version (it must be three numbers, like 1.0.1)." -ForegroundColor Yellow
    $found = if ($last) { $last } else { "0.0.0" }
}

if ($Bump) {
    if ($Bump -notin "patch", "minor", "major") { Stop-Release "Use patch, minor or major (or nothing)." }
    $base = if ($last -and (Compare-Version $last $found) -gt 0) { $last } else { $found }
    $version = Get-Bumped $base $Bump
} else {
    # The newest version in CHANGELOG.md with notes that isn't released yet
    # (so writing "## 1.0.1" and its notes is all a release needs), else the
    # version in Cargo.toml.
    $pending = Get-ChangelogVersions | Where-Object { -not $last -or (Compare-Version $_ $last) -gt 0 } | Select-Object -First 1
    $version = if ($pending) { $pending } else { $found }
}

if (-not $Local -and (Test-Released $version)) {
    Write-Host ""
    Write-Host "Pious $version is already released, and CHANGELOG.md has no newer version with notes." -ForegroundColor Yellow
    # Enter (or P) is the usual answer: the next patch version.
    $answer = Read-Host "Release the next version instead? [P]atch (Enter) / [M]inor / [N]o"
    switch -regex ($answer) {
        "^[nN]" { Stop-Release "Nothing was released." }
        "^[mM]" { $version = Get-Bumped $version "minor" }
        default { $version = Get-Bumped $version "patch" }
    }
    if (Test-Released $version) { Stop-Release "Pious $version is released too. Add a newer '## x.y.z' section to CHANGELOG.md." }
}
if ((Get-Version) -ne $version) {
    [void](Write-Version $version)
    Write-Host "Set the version to $version (Cargo.toml, package.json, tauri.conf.json)."
}
Write-Host "Releasing Pious $version$(if ($Local) { ' (local build, nothing is published)' })" -ForegroundColor Green

# What goes out: everything since the last release.
if ($last -and (Test-Quiet "git rev-parse -q --verify refs/tags/v$last")) {
    $since = @(git log --oneline "v$last..HEAD")
    Write-Host "$($since.Count) commit(s) since v$last$(if ($since.Count) { ':' })"
    $since | Select-Object -First 15 | ForEach-Object { Write-Host "  $_" }
}
$uncommitted = @(git status --porcelain)
if ($uncommitted.Count) { Write-Host "$($uncommitted.Count) changed file(s) not committed yet: they go into this release's commit." }

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
