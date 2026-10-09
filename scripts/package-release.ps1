# Makes the four release files in -Out from a finished build. Used by both
# .github/workflows/release.yml and `release.ps1 -Local`, so a local test
# release is exactly what CI would publish.
#
#   manifest.json               version + sha256/size of the app zip
#   pious-<v>-win-x64.zip       pious.exe, docs\, themes\, plugins\, version.txt
#   Pious-Setup.exe             the reusable installer
#   pious.exe                   the bare app (portable copies update from it)
#
# FFmpeg is inside pious.exe (built in by src-tauri\build.rs from
# installer\vendor\ffmpeg.exe, see get-ffmpeg.ps1), so there's no
# ffmpeg.zip any more.
param(
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$App,
    [Parameter(Mandatory)][string]$Setup,
    [string]$Out = "release"
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $root
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

if (-not [IO.Path]::IsPathRooted($Out)) { $Out = Join-Path $root $Out }
foreach ($file in $App, $Setup) { if (-not (Test-Path $file)) { throw "$file doesn't exist. Build it first." } }
# A release without its recorder would be a broken release.
if ((Get-Item $App).Length -lt 30MB) { throw "$App has no FFmpeg built in. Run scripts\get-ffmpeg.ps1, then build Pious again." }

# Stage the loose files -----------------------------------------------------
$stage = Join-Path ([IO.Path]::GetTempPath()) "pious-stage-$PID"
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item $App (Join-Path $stage "pious.exe")
$docs = Join-Path $stage "docs"
New-Item -ItemType Directory -Force $docs | Out-Null
$sources = @()
if (Test-Path "docs") { $sources += Get-ChildItem "docs" -Filter *.md -File }
foreach ($name in "PRIVACY.md", "TERMS.md") { if (Test-Path $name) { $sources += Get-Item $name } }
foreach ($file in $sources) { Copy-Item $file.FullName (Join-Path $docs $file.Name) -Force }
foreach ($folder in "themes", "plugins") {
    if (Test-Path $folder) { Copy-Item $folder (Join-Path $stage $folder) -Recurse -Force }
}
# Not shipped: plugins' tests, their saved data from a developer's PC, and
# the files Pious writes into plugin folders itself.
Get-ChildItem (Join-Path $stage "plugins") -Directory -ErrorAction SilentlyContinue | ForEach-Object {
    foreach ($name in "test", "data", "pious-plugin.js", "pious-ui.css", "pious-engine.html") {
        Remove-Item (Join-Path $_.FullName $name) -Recurse -Force -ErrorAction SilentlyContinue
    }
}
[IO.File]::WriteAllText((Join-Path $stage "version.txt"), $Version)

# Zip (CreateFromDirectory writes forward-slash entry names) -----------------------
New-Item -ItemType Directory -Force $Out | Out-Null
foreach ($old in "manifest.json", "ffmpeg.zip", "Pious-Setup.exe", "pious.exe", "pious-$Version-win-x64.zip") {
    # (ffmpeg.zip: left over from releases before FFmpeg was built in.)
    Remove-Item (Join-Path $Out $old) -ErrorAction SilentlyContinue
}
$appZip = Join-Path $Out "pious-$Version-win-x64.zip"
[IO.Compression.ZipFile]::CreateFromDirectory($stage, $appZip, [IO.Compression.CompressionLevel]::Optimal, $false)
Remove-Item $stage -Recurse -Force

Copy-Item $Setup (Join-Path $Out "Pious-Setup.exe")
Copy-Item $App (Join-Path $Out "pious.exe")

# Manifest -------------------------------------------------------------------
function Describe([string]$path, [string]$kind) {
    [ordered]@{
        name   = (Split-Path $path -Leaf)
        kind   = $kind
        sha256 = (Get-FileHash $path -Algorithm SHA256).Hash.ToLower()
        size   = (Get-Item $path).Length
    }
}
$manifest = [ordered]@{
    version = $Version
    files   = @((Describe $appZip "app"))
}
[IO.File]::WriteAllText((Join-Path $Out "manifest.json"), ($manifest | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding $false))

Get-ChildItem $Out -File | Where-Object { $_.Name -in "manifest.json", "Pious-Setup.exe", "pious.exe", "pious-$Version-win-x64.zip" } |
    ForEach-Object { "{0,-30} {1,8:N1} MB" -f $_.Name, ($_.Length / 1MB) }
