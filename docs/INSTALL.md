# Installing and updating Pious

## Installing

1. Download `Pious-Setup.exe` from the
   [latest release](https://github.com/woogi999/pious-bootstrapper/releases/latest).
2. Run it and allow it to make changes (Windows asks: Setup runs as
   administrator so it can close Pious and Roblox and replace files that are
   in use). It shows the newest version and what changed, and where it will
   install (by default `%LOCALAPPDATA%\Programs\Pious`).
3. Click **Install**. Setup downloads Pious from GitHub, checks it against
   the checksum GitHub publishes, and puts the files in place. It adds Start
   menu (and, if you like, desktop) shortcuts and an entry in Windows'
   installed apps. Pious itself always opens as you, never as administrator.

FFmpeg, which records and clips, is built into `pious.exe`: there's nothing
else to download.

The installer is the same program for every release: it always fetches the
newest Pious, so an old `Pious-Setup.exe` installs the current version too.

**No internet?** Put `pious-X.Y.Z-win-x64.zip` from a release next to
`Pious-Setup.exe`; Setup offers to install from it when it can't reach
GitHub.

**Without installing:** `pious.exe` from a release runs on its own. It
keeps its data in `%LOCALAPPDATA%\Pious\Bootstrapper` and updates itself.

Pious uses Microsoft Edge WebView2, which comes with Windows 10 and 11.

## What gets installed

Pious is installed as plain files you can look at ("loose files"), so
plugins and themes are just folders:

```
Pious\
├── pious.exe           the app (with FFmpeg inside, for recording)
├── pious-setup.exe     updates and uninstalls Pious
├── docs\               this help (also built into the app)
├── plugins\            plugins, including Macros
├── themes\             themes
├── data\               YOUR library, settings, Roblox versions, cache…
├── install.json        which version is installed
└── install-files.txt   the files Setup put here (for updates and uninstalling)
```

Updates and uninstalling only touch the files Setup put there. Your
`data` folder, and any plugin or theme you added yourself, are never
replaced or removed (unless you choose to remove your data when
uninstalling).

## Updating

Pious checks GitHub for a new version when it starts (turn that off in
Settings → About). When one is out, the top bar says so: **Download**, then
**Restart**. Pious hands over to its `pious-setup.exe` (Windows asks for
permission), which downloads and checks the new version, waits for Pious to
close, replaces the files and opens Pious again, which then shows what's
new. Updating from a version that had a separate `ffmpeg.exe` removes it;
the built-in one takes over.

## Moving from an older Pious

Older versions kept everything in `%LOCALAPPDATA%\Pious\Bootstrapper`. The
first time a new installed Pious starts, it moves that folder's contents
into the install folder's `data` and points your saved paths (like
installed Roblox versions) at their new place. Nothing is deleted:

- If the install folder is on another drive, small files are copied and
  big Roblox versions stay where they are (Pious keeps using them there).
- Anything that couldn't be moved stays, with a `MOVED.txt` saying where the
  rest went.
- If the new `data` folder already holds more than the old one, the old
  one is left alone.
- Plugins and themes from the old folder move into `plugins` and `themes`.

If Pious can't write to its install folder (for example, installed in
Program Files by hand), it keeps using `%LOCALAPPDATA%\Pious\Bootstrapper`.

## Uninstalling

Windows Settings → Apps → Pious → Uninstall (or run `pious-setup.exe
--uninstall`). Tick **Remove my data** to also delete your library,
settings and downloaded Roblox versions. Account sign-ins live in Windows
Credential Manager; remove accounts in Pious first to delete those.

## For developers

Building from source and publishing releases is in `SETUP.md` in the
repository.
