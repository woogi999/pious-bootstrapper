<p align="center"><img src="assets/brand/lockup-white.svg" alt="Pious" height="72"></p>

# Pious

A Roblox launcher for Windows. Pious keeps your games, accounts, friends, private servers and Roblox versions in one place, and adds the features bootstrappers have: multi-instance, FastFlags and mods, recording, overlays and more.

**[Download Pious-Setup.exe](https://github.com/woogi999/pious-bootstrapper/releases/latest)**

## Features

### 

- Pick a game, an account and (optionally) a version and server, then Play.
- Search Roblox for games and add them in one click, or paste a link.
- Join any player by username, browse and sort public servers, server hop, and save private servers.
- Multi-instance: run several Roblox clients, each on its own account.
- Rejoin automatically after a disconnect, and stay online with anti-AFK.

### 

- Sign in on Roblox's own page, with Quick Login, or from your browser. Pious never sees your password; sessions stay in Windows Credential Manager.
- See what friends are playing, join them, and chat in a window of its own.
- Per-account Roblox settings.

### 

- Frame rate unlock, graphics API, anti-aliasing (including off), texture and mesh quality, the 21-level render quality, pause voxelizer, CPU priority and which graphics card runs Roblox.
- FastFlag profiles with an editor, mods, cursors, shift lock crosshairs, custom skies, fonts and emoji styles.
- Every replaced file is backed up and restored when tweaks are turned off.

### 

- An in-game overlay (`Home`) with friends, servers, recording and media controls.
- Overlays for your keys and mouse, and for game stats: FPS, the server's ping and players, server location, time played, CPU and memory.
- Recording and instant clips on your graphics card, with FFmpeg built in.
- Keybinds, emoji shortcodes and Discord Rich Presence (as Roblox or Pious).

### 

- Themes, colors, backgrounds and fonts.
- Plugins: add pages, Roblox files and features. Macros and the auto-clicker are a plugin. See [Plugins](docs/PLUGINS.md), the [Plugin API](docs/PLUGIN-API.md) and the [Plugin UI kit](docs/PLUGIN-UI.md).
- Connect AI apps through MCP ([docs/MCP.md](docs/MCP.md)).

Every guide is also in the app under **Help**.

## Install

Run `Pious-Setup.exe`. It asks for administrator permission, installs Pious to `%LOCALAPPDATA%\Programs\Pious`, adds shortcuts, and keeps Pious updated. Pious itself runs as you, never as administrator. A portable `pious.exe` is on the releases page too. Details: [docs/INSTALL.md](docs/INSTALL.md).

Updates are checked on start and verified against GitHub's checksums before they're installed.

## Your data

Everything stays on your PC: settings in `data\` next to Pious, sign-ins in Windows Credential Manager, videos in `Videos\Pious`. Pious has no accounts, analytics or servers of its own. See the [Privacy Policy](PRIVACY.md) and [Terms of Service](TERMS.md).

Multi-instance, macros and the auto-clicker can break a game's rules. Use them at your own risk.

## Build from source

Needs [Rust](https://rustup.rs) and [Node.js](https://nodejs.org) on Windows.

```sh
npm install
npm run tauri dev                    # development build
npm run tauri build -- --no-bundle   # src-tauri/target/release/pious.exe
```

To build FFmpeg into Pious, run `scripts\get-ffmpeg.ps1` first. Releases: write the notes under `## <version>` in `CHANGELOG.md`, then run `update_release.bat patch` (see [SETUP.md](SETUP.md)).

The interface is Svelte in a Tauri window; the rest is Rust (`src-tauri/`). The installer is in `installer/`.

## Credits

- [Heroicons](https://heroicons.com) (MIT) and [Manrope](https://github.com/sharanda/manrope) (SIL OFL 1.1)
- [FFmpeg](https://ffmpeg.org), the GPL v3 build from [gyan.dev](https://www.gyan.dev/ffmpeg/builds/), built into `pious.exe` and run as a separate process. Its source code is at [ffmpeg.org](https://ffmpeg.org/download.html) and [gyan.dev](https://www.gyan.dev/ffmpeg/builds/).
- Emoji, fonts, cursors and icons from the open-source projects listed in [docs/MODS.md](docs/MODS.md), downloaded when chosen

Pious isn't made by, affiliated with, or endorsed by Roblox Corporation.
