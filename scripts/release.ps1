# Builds the installer and publishes a GitHub release. Run by
# update_release.bat; see there for how to use it.
#
#   1. Version: the one in src-tauri\Cargo.toml, or bumped first (patch,
#      minor, major). If GitHub already has it, you're asked to bump.
#   2. Notes:   the "## <version>" section of CHANGELOG.md.
#   3. Build:   Pious (release), FFmpeg, then the installer with both inside.
#   4. Publish: commit, tag, push, and a GitHub release with
#      Pious-Setup.exe, pious.exe and ffmpeg.zip.
#
# Every step stops the whole release when it fails; nothing loops.
param([string]$Bump = "")

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
foreach ($tool in "git", "cargo", "npm", "gh") {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        Stop-Release "$tool isn't installed. Rust: https://rustup.rs  Node.js: https://nodejs.org  GitHub CLI: winget install GitHub.cli"
    }
}
if (-not (Test-Quiet "gh auth status")) {
    Write-Host "Sign in to GitHub (once):"
    Run "GitHub sign-in" { gh auth login --web --git-protocol https }
}
$branch = (git rev-parse --abbrev-ref HEAD).Trim()
if ($branch -ne "main") { Stop-Release "You're on '$branch'. Switch to main first." }

# ── Version ──────────────────────────────────────────────────────────────
function Get-Version {
    [regex]::Match([IO.File]::ReadAllText($cargoToml), '(?m)^version = "([^"]+)"').Groups[1].Value
}

function Set-Version([string]$part) {
    $v = (Get-Version).Split(".") | ForEach-Object { [int]$_ }
    switch ($part) {
        "major" { $v = @($v[0] + 1, 0, 0) }
        "minor" { $v = @($v[0], $v[1] + 1, 0) }
        default { $v = @($v[0], $v[1], $v[2] + 1) }
    }
    $new = $v -join "."
    $first = [regex]'(?m)^version = "[^"]*"'
    foreach ($file in "src-tauri\Cargo.toml", "installer\Cargo.toml") {
        $path = Join-Path $root $file
        [IO.File]::WriteAllText($path, $first.Replace([IO.File]::ReadAllText($path), "version = `"$new`"", 1))
    }
    $json = [regex]'"version":\s*"[^"]*"'
    foreach ($file in "package.json", "src-tauri\tauri.conf.json") {
        $path = Join-Path $root $file
        [IO.File]::WriteAllText($path, $json.Replace([IO.File]::ReadAllText($path), "`"version`": `"$new`"", 1))
    }
    $new
}

function Test-Released([string]$version) {
    Test-Quiet "gh release view v$version"
}

if ($Bump) {
    if ($Bump -notin "patch", "minor", "major") { Stop-Release "Use patch, minor or major (or nothing)." }
    $version = Set-Version $Bump
} else {
    $version = Get-Version
}
if (Test-Released $version) {
    Write-Host ""
    Write-Host "Pious $version is already on GitHub." -ForegroundColor Yellow
    $answer = Read-Host "Release it as the next version instead? [P]atch / [M]inor / [N]o"
    switch -regex ($answer) {
        "^[pP]" { $version = Set-Version "patch" }
        "^[mM]" { $version = Set-Version "minor" }
        default { Stop-Release "Nothing was released." }
    }
    if (Test-Released $version) { Stop-Release "Pious $version is on GitHub too. Bump the version in src-tauri\Cargo.toml." }
}
Write-Host "Releasing Pious $version" -ForegroundColor Green

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
New-Item -ItemType Directory -Force $out | Out-Null
$notesFile = Join-Path $out "notes.md"
[IO.File]::WriteAllText($notesFile, $notes)
Write-Host $notes

# ── Build ────────────────────────────────────────────────────────────────
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

Step "Building Pious (the first build takes a while, later ones are quick)"
Run "The Pious build" { npm run tauri build -- --no-bundle }
$app = Join-Path $root "src-tauri\target\release\pious.exe"

Step "FFmpeg"
$vendor = Join-Path $root "installer\vendor"
$ffmpeg = Join-Path $vendor "ffmpeg.exe"
New-Item -ItemType Directory -Force $vendor | Out-Null
if (-not (Test-Path $ffmpeg)) {
    $local = @("Bootstrapper", "Library") | ForEach-Object { Join-Path $env:LOCALAPPDATA "Pious\$_\tools\ffmpeg.exe" } | Where-Object { Test-Path $_ } | Select-Object -First 1
    if ($local) {
        Copy-Item $local $ffmpeg
    } else {
        Write-Host "Downloading FFmpeg (about 100 MB, once)..."
        $zip = Join-Path $env:TEMP "pious-ffmpeg.zip"
        Invoke-WebRequest "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip" -OutFile $zip
        Add-Type -AssemblyName System.IO.Compression.FileSystem
        $archive = [IO.Compression.ZipFile]::OpenRead($zip)
        try {
            $entry = $archive.Entries | Where-Object { $_.FullName -like "*/bin/ffmpeg.exe" } | Select-Object -First 1
            if (-not $entry) { Stop-Release "The FFmpeg download has no ffmpeg.exe." }
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $ffmpeg, $true)
        } finally {
            $archive.Dispose()
            Remove-Item $zip -ErrorAction SilentlyContinue
        }
    }
}
Write-Host "Ready."

Step "Building the installer"
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
function New-Zip($path, $files) {
    Remove-Item $path -ErrorAction SilentlyContinue
    $zip = [IO.Compression.ZipFile]::Open($path, "Create")
    try {
        foreach ($file in $files) {
            [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($zip, $file, (Split-Path $file -Leaf), "Optimal") | Out-Null
        }
    } finally { $zip.Dispose() }
}
foreach ($name in "Pious-Setup.exe", "pious.exe", "ffmpeg.zip", "payload.zip") {
    Remove-Item (Join-Path $out $name) -ErrorAction SilentlyContinue
}
$payload = Join-Path $out "payload.zip"
New-Zip $payload @($app, $ffmpeg)
New-Zip (Join-Path $out "ffmpeg.zip") @($ffmpeg)
Copy-Item $app (Join-Path $out "pious.exe")
# The installer keeps its own build folder, so it isn't rebuilt from
# scratch each time (it used to share the app's, and they kept undoing
# each other's work).
$env:PIOUS_PAYLOAD = $payload
$env:CARGO_TARGET_DIR = Join-Path $root "installer\target"
Run "The installer build" { cargo build --release --manifest-path (Join-Path $root "installer\Cargo.toml") }
Remove-Item Env:\CARGO_TARGET_DIR, Env:\PIOUS_PAYLOAD
Copy-Item (Join-Path $root "installer\target\release\pious-setup.exe") (Join-Path $out "Pious-Setup.exe")
Remove-Item $payload
Get-ChildItem $out -Filter *.exe | ForEach-Object { "{0,-18} {1,6:N1} MB" -f $_.Name, ($_.Length / 1MB) }

# ── Publish ──────────────────────────────────────────────────────────────
Step "Publishing Pious $version"
Run "git add" { git add -A }
git diff --cached --quiet
if ($LASTEXITCODE -ne 0) { Run "git commit" { git commit -q -m "Release v$version" } }
if (-not (Test-Quiet "git rev-parse -q --verify refs/tags/v$version")) { Run "git tag" { git tag "v$version" } }
Run "git push" { git push origin main }
Run "Pushing the tag" { git push origin "v$version" }
Run "The GitHub release" {
    gh release create "v$version" (Join-Path $out "Pious-Setup.exe") (Join-Path $out "pious.exe") (Join-Path $out "ffmpeg.zip") `
        --title "Pious $version" --notes-file $notesFile --latest
}
Write-Host ""
Write-Host "Pious $version is out. Installed copies offer it on their next start." -ForegroundColor Green
gh release view "v$version" --json url -q .url
