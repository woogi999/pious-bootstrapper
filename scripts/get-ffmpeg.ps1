# Puts ffmpeg.exe in installer\vendor (downloaded once, kept after that).
# Pious builds it into pious.exe (see src-tauri\build.rs), so run this
# before building Pious for a release. Used by release.ps1 -Local and
# .github/workflows/release.yml.
param([string]$Folder = "installer\vendor")

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
if (-not [IO.Path]::IsPathRooted($Folder)) { $Folder = Join-Path $root $Folder }
Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem

New-Item -ItemType Directory -Force $Folder | Out-Null
$ffmpeg = Join-Path $Folder "ffmpeg.exe"
if (Test-Path $ffmpeg) {
    Write-Host "FFmpeg: $ffmpeg"
    return
}
Write-Host "Downloading FFmpeg (about 100 MB)..."
$download = Join-Path ([IO.Path]::GetTempPath()) "pious-ffmpeg-$PID.zip"
try {
    Invoke-WebRequest "https://www.gyan.dev/ffmpeg/builds/ffmpeg-release-essentials.zip" -OutFile $download
    $archive = [IO.Compression.ZipFile]::OpenRead($download)
    try {
        $entry = $archive.Entries | Where-Object { $_.FullName -like "*/bin/ffmpeg.exe" } | Select-Object -First 1
        if (-not $entry) { throw "The FFmpeg download has no ffmpeg.exe." }
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, "$ffmpeg.part", $true)
    } finally {
        $archive.Dispose()
    }
    Move-Item "$ffmpeg.part" $ffmpeg -Force
} finally {
    Remove-Item $download, "$ffmpeg.part" -ErrorAction SilentlyContinue
}
Write-Host "FFmpeg: $ffmpeg"
