<p align="center"><img src="assets/brand/lockup-white.svg" alt="Pious" height="72"></p>

# Pious

Pious is a desktop app I made for keeping all my Roblox stuff in one place: my games, my accounts, my friends, my private servers, the Roblox versions I have installed, every Roblox window I have open, and recordings of what I play.

You pick a game, pick an account (and a version and server if you want), and hit Play. That's it.

## Why I made this

I got tired of the false promises other bootstrappers make. Half of them say they do multi-instance and don't. When you finally find one that really does, it turns out to be an infostealer that grabs your Roblox cookie.

So I made my own, and I made it the way I'd want one to work:

- **Multi-instance that actually works.** Run as many Roblox clients as you want, each on a different account.
- **Your login stays yours.** I never ask for your password. When you sign in, you type it into Roblox's own page. Pious only keeps the session it gets back, and it keeps it in your computer's built-in password vault (Windows Credential Manager), never in a file. Your session is only ever sent to Roblox.
- **You can read every line.** All of the source code is right here.

## What it does

- **Home.** Jump back into your last game in one click. You also get your recent games, your servers, your accounts, what's running right now, game recommendations based on what all your accounts play (with **See more** for dozens more), and **games your friends are playing**, with a button to join them.
- **Games.** Your library as cards, with search, favorites, collections and sorting. Right-click anything for more options. **Recents** lists everything you've played, including games you never added; those show **Add** instead of Play. Opening or playing a recommended game doesn't add it to your library.
- **Join a player.** Type anyone's username and Pious joins the exact server they're in, friend or not, as long as their settings let everyone join them. (It turns the username into a user ID, asks Roblox's presence API where they are, and joins that server.)
- **Server hop.** Jump to a random other public server of a game, or move a running window to a new server.
- **Server browser.** See a game's public servers in a list and sort them by players, free slots, how full they are, ping or FPS. Join any of them in one click.
- **Private servers.** Save server links (both the old `privateServerLinkCode` links and the new `roblox.com/share` ones), add notes, and join with one click. Leave the name empty and Pious uses the server's own name (or whose server it is); leave the game empty and it's taken from the link.
- **Join a private server link** (`Ctrl L`). Paste any private server link and join it straight away, like Minecraft's Direct Connect. Saving it is optional.
- **Friends.** See the friends of any of your accounts: who's online, who's in a game and which one, and join them in one click. Message them through Roblox's own chat in a chat window of its own, like Steam's: friends and recent chats on the left, open chats as tabs.
- **Accounts.** Add as many as you like, shown as cards or a compact list. The account picked at the bottom of the sidebar is the one Play uses; it doesn't change your default account. A game only sticks to one account if you pin it in that game's launch configuration. There's a way to sign in for everyone:
  - **Sign in with Roblox.** Roblox's real login page opens inside the app, so passwords, email codes, passkeys and 2-step verification all work.
  - **Quick Login.** Approve a code from a phone or another device that's already signed in.
  - **Create an account**, also inside the app.
  - **From your browser.** Already signed in to roblox.com in Edge, Chrome, Firefox, Brave or Opera? Pious finds that sign-in and adds the account. (Chrome's newest versions lock their sign-ins to Chrome itself; use another way there.)
  - **Session token**, if you know what you're doing.
- **Roblox settings per account.** Turn on "Keep its own settings" for an account and it gets its own volume, mouse sensitivity, graphics quality and other Roblox settings. Pious swaps them in before that account's game starts and saves them when it closes. You can also edit your PC's Roblox settings from Settings.
- **Switch games on the same account.** Playing another game with an account that's already in one asks if you want to switch. Pious closes that window and opens the new game in the same spot and size. (Roblox can't change games inside a window while multi-instance is on, so this is as close as it gets.)
- **Versions.** See which Roblox versions you already have, download any version straight from Roblox, choose which one each game uses, and remove the ones you don't need. **Install version** lists the current, previous and upcoming builds from [weao.xyz](https://weao.xyz) and hundreds of older ones from Roblox's deploy history, searchable, so you can roll back to any version. You can also uninstall Roblox itself from here.
- **Bootstrappers.** Pious finds Bloxstrap, Fishstrap and similar launchers and shows what each one does to your PC (opening Roblox links, Discord presence, FastFlags, mods), and can run their uninstallers. It never edits their settings, and it leaves their Discord presence alone for games they start.
- **Launch all Roblox through Pious** (Settings). Pious becomes what opens Play on roblox.com, so every Roblox window, including ones from different browsers signed in to different accounts, runs as its own instance and shows up in Instances. Turning it off hands Roblox links back to whatever opened them before.
- **DNS for Roblox and compatibility** (Tweaks). Send only Roblox's lookups to another DNS (Cloudflare, Google, Quad9, OpenDNS, AdGuard, Control D or your own) through Windows' name resolution policy, check the connection, clear the DNS cache; turn off fullscreen optimizations, override high-DPI scaling and raise Roblox's priority.
- **Discord Rich Presence** (Tweaks), with Bloxstrap's options: show game activity (as "Playing <game name>" or "Playing Pious"), allow activity joining, show Roblox account, show Roblox Studio activity, and "Playing Pious" while idle.
- **Appear online.** While Pious is open, your Roblox friends see you as online, like with the Roblox app open.
- **Notifications.** Pious's own pop-ups in the corner of your screen when a friend messages you, sends a friend request, starts playing, or Roblox has new notifications. Click one to open it.
- **Tweaks.** Bootstrapper features built in:
  - **Performance:** unlock the frame rate (pick one or type any number; set in Roblox's own settings file like Fishstrap, since Roblox refuses the old FastFlag), Roblox's graphics quality, the graphics API, anti-aliasing, texture quality, mesh detail, simpler lighting, a gray sky and still grass, turn off DPI scaling, and use Alt+Enter for fullscreen. Roblox is updated before a game starts, like bootstrappers do, so the tweaks are on the build that runs. The FastFlags tab shows any flag Roblox refused at the last launch.
  - **FastFlag editor:** keep as many FastFlag profiles as you want, switch between them, edit flags in a searchable table, and import or copy them as JSON.
  - **Mods:** old cursors, old character sounds, the old avatar background, a custom Roblox window title, a shift lock crosshair (game-style presets or your own), the Roblox window's icon, and your own mods folder (sounds, cursors, icons…). See [docs/MODS.md](docs/MODS.md).
  - **Fonts** (with a live preview): any of Roblox's own fonts (Builder Sans, the classic Source Sans Pro, Gotham-style Montserrat and more), game fonts (Minecraft, Monocraft, Pokémon Game Boy, GBA and DS), fun ones from Google Fonts, or your own .ttf/.otf.
  - **Emoji:** 15 styles, including Windows 11 Fluent, Windows 10, Twitter, EmojiOne, pixel and classic phone emoji, and an experimental iOS (Apple) style that Pious builds on your PC from Apple's emoji pictures.
  - **Cleaner:** clears out old Roblox logs and cache files.

  Pious backs up every file it replaces and puts the originals back when you turn tweaks off.
- **Choose your launcher** (Settings). Pick what opens Play on roblox.com (Pious, Roblox, or any installed bootstrapper), and whether Pious starts games itself or through a bootstrapper so its own mods apply.
- **Game overlay.** Press `Home` (or any key combination you set by pressing it) in a game to bring up Pious on top of it: friends (join them or chat), private servers, what's running, recording, quick play, in-game options and media controls for whatever's playing on your PC (Spotify, YouTube…). `Ctrl K` searches like in the app. Its panels sit around the edges so the middle stays clear, and you can drag them anywhere, resize them, pin them, or hide them. The game behind it can be blurred at any strength, blurred live by Windows, or only dimmed. By default the hotkey only works while a Roblox window is in front, so `Home` keeps working everywhere else.
- **Input overlay** (Settings → Overlay). The keys and mouse buttons you press, drawn over the game in slanted streamer-style keys, with clicks, keys and frames per second. Pick a layout, size and colors, and show it always, while playing, or only while recording or clipping.
- **Keybinds.** Make a key or mouse button press something else in Roblox, for every game, one account or one game.
- **Recording and clips.** Pious can take over Roblox's `F12` to record your games, and keep the last few minutes so you can save a clip of what just happened (`F8` for the last 30 seconds, `Shift F8` to type how many seconds back from the moment you pressed it). It records with your graphics card (NVIDIA, AMD or Intel) through Desktop Duplication, so games keep their frame rate, and saving a clip only copies what's already recorded, so it's instant and loses no quality. Sound can be just Roblox, everything on your PC, and/or your microphone, mixed or on separate tracks. Settings → Recording has the simple choices up front (quality, resolution, frame rate, sound) and everything else under Advanced: encoder, H.264/HEVC/AV1, constant quality or bitrate, encoder speed, keyframes, 10-bit, audio format and volumes. It uses FFmpeg, which comes with the installer.
- **Anti-AFK.** Keeps every Roblox window from being kicked after 20 idle minutes. On a schedule you pick, each window gets a real key press (jump, a step, or a zoom). Pious only sends it after you've stopped typing for a few seconds, then gives focus straight back to what you were using.
- **Rejoin when disconnected.** Pious reads Roblox's log like bootstrappers do; when a game kicks you or loses connection, it closes the error and joins the same game again with the same account. Leaving on purpose and teleports are left alone.
- **Instances.** See every Roblox window Pious opened: which account, which game, how long it's been running. Focus or close any of them.
- **Macros.** Record what you do (keys, clicks, scrolling and the timing between them) with `F7`, or build a macro step by step: press or hold keys, type text, click, move the pointer (to a spot or by an amount, instantly or gliding), scroll, wait (with optional random extra time), repeat a group of steps, wait until a spot on screen turns a color, bring Roblox forward, or open a program or link. Each macro can have its own hotkey and run once, a number of times, until stopped, or while its key is held, at any speed, in the Roblox window you used last, every window, or only the windows of accounts you pick. Import AutoHotkey scripts as macros, or save a macro as one. `Shift Esc` stops everything.
- **Auto-clicker.** Toggle it or hold its key (`F6`), or let it click while you hold your mouse button or add extra clicks to each of yours; click with any mouse button or press a key over and over; single or double clicks; any interval with random extra time; how long each click is held; a fixed spot or wherever the pointer is; and a limit by clicks or seconds.
- **Plugins.** Drop a folder with a `plugin.json` and a page into the plugins folder and it gets its own page in the sidebar. Plugins run walled off from Pious and can only do what they ask for (see your games and friends, launch games, show notices, run macros…). Settings → Plugins makes an example to start from. See [docs/PLUGINS.md](docs/PLUGINS.md).
- **AI apps (MCP).** Connect Claude Desktop, Claude Code, Cursor or Codex in one click (Settings → Plugins), or ChatGPT through a tunnel. AI apps can list your games, accounts and friends, launch and join games, record clips, run macros and send messages, and play: look at a Roblox window, hear it, walk, press keys, type and click in it. You can limit them to looking only. See [docs/MCP.md](docs/MCP.md).
- **Streamer mode** (`Ctrl Shift S`). Shows only the first two letters of every Roblox name.
- **Links open where you like**: in a Pious window or your web browser.
- **Click the logo** at the top left to go home. Settings can make it do anything else instead, even run a macro.
- **Your stats.** Pious keeps every statistic it can (play time per game, sessions, launches, clips, messages, macros…) on your PC, for a yearly recap one day. A summary is in Settings → About.
- **A welcome tour and tips** the first time you open Pious and each page.
- **What's new** after every update, and the full changelog in Settings → About.
- **Search everything** with `Ctrl K`.
- **Keyboard shortcuts** for everything: `Ctrl 1`–`0` for pages, `Ctrl P` to play your last game, `Ctrl N` to add a game, `Ctrl J` to join a player, `Ctrl T` to keep Pious on top, and more. Every one can be changed in Settings → Keyboard.
- **Cards or lists, sorted your way.** Games, servers, friends, accounts, versions, macros and running windows can each be shown as cards or as a compact list, and sorted however you like.
- **Opens fast.** Friends, chats, recommendations and build lists are kept from last time, so pages you've visited show up at once while fresh data loads.
- **Tray and start-up** (Settings). Choose whether the close button quits Pious or keeps it in the tray. Choose what Pious does when a game starts: stay open, minimize, or hide and come back when Roblox closes. Pious can also start with Windows, in the tray if you like.

## Making it yours

- Pick any colors you want for the accent, background, glass and text. There are no presets; you choose exactly the color.
- Set your own background picture, and blur or dim it so it stays out of the way.
- Turn on **See-through** to let your desktop show through the window. **Frosted** uses Windows' own blur. **Adjustable** blurs at any strength you pick, but Pious won't appear in screenshots while it's on.
- **Interface size**: make everything bigger or smaller.
- Choose how much the app blurs behind `Ctrl K` search and dialogs, or turn that blur off.
- **Keep on top** (the pin in the title bar, or `Ctrl T`), fullscreen (`F11`), and a sidebar you can shrink down to icons (`Ctrl B`).
- Sliders show their value; click it to type an exact number.
- Everything moves smoothly. If you'd rather it didn't, turn on **Reduce motion** in Settings.

## Updates

Pious checks GitHub for a new version when it starts. When one is out, you'll see it in the top bar. Click it, hit Download, then Restart: the new installer updates Pious in place and opens it again, then shows what's new. Before anything installs, the download is checked against the checksum GitHub publishes, so a broken or tampered download won't be installed. You can turn automatic checks off in Settings.

## Running it

**Easiest way:** download `Pious-Setup.exe` from the [latest release](https://github.com/woogi999/pious-bootstrapper/releases/latest) and run it. It installs Pious (with FFmpeg for recording) into a folder of its own, by default `%LOCALAPPDATA%\Programs\Pious`, adds Start menu and desktop shortcuts, and appears in Windows' installed apps so you can uninstall it. No administrator rights needed. (Pious uses Microsoft Edge WebView2, which comes with Windows 10 and 11.) `pious.exe` alone is there too, if you'd rather not install.

**From the source code** (Windows): install [Rust](https://rustup.rs) and [Node.js](https://nodejs.org), then double-click `test.bat`, which builds a development copy and starts it. Or run it yourself:

```sh
npm install
npm run tauri build -- --no-bundle   # src-tauri/target/release/pious.exe
npm run tauri dev                    # or: live-reloading development build
```

## Where your stuff is kept

- Your games, servers and settings live in `%LOCALAPPDATA%\Pious\Bootstrapper\bootstrapper.json` (older builds used `Pious\Library`; Pious moves it over by itself).
- Roblox versions installed by Pious go to `%LOCALAPPDATA%\Pious\Bootstrapper\Versions`.
- Account sessions live only in Windows Credential Manager, never in those files.
- Recordings and clips go to `Videos\Pious` unless you pick another folder. FFmpeg is kept in `tools` in that same folder.
- The sign-in window uses a private browser profile, so nothing is left behind after you sign in.
- Statistics go to `stats.jsonl`, and plugins live in `plugins`, both in the same folder.
- Pious itself is installed to `%LOCALAPPDATA%\Programs\Pious` unless you chose another folder.
- If the interface ever misbehaves, errors are written to `ui-errors.log` in the same folder.

To keep everything in a different folder (like on a USB stick), set the `PIOUS_DATA` environment variable to that folder.

## For developers

The interface is [Svelte](https://svelte.dev) in a [Tauri](https://tauri.app) window; everything else is Rust. The window never owns data: it calls Rust commands and draws the snapshot of the app's state that Rust pushes to it after every change.

```text
src/                Svelte + TypeScript interface
├── pages/          One file per page
├── dialogs/        The dialogs
├── components/     Cards, the sidebar, menus and other building blocks
└── lib/            Talking to Rust, app state, theme, formatting, icons
src-tauri/          Rust
├── src/core/       The actual logic: Roblox APIs, friends and chat, launching, versions, tweaks, fonts, recording, saving, updates
├── src/service/    The app's state and everything it does, shared with the window
├── src/commands.rs The commands the window can call
└── src/platform/   Windows-specific bits (the registry, single instance)
installer/          Pious Setup: installs, updates and uninstalls (its own small Tauri app)
scripts/            release.ps1, the steps update_release.bat runs
CHANGELOG.md        What changed in each version (shown in the app and on GitHub)
```

The icons are [Heroicons](https://heroicons.com), and the font is [Manrope](https://github.com/sharanda/manrope). The logo is in `assets/brand/`; the full brand kit and brand bible live next to this repository, in `../brand` and `../Pious Brand Bible.html`.

To publish a new version, write what changed in `CHANGELOG.md` under a `## <version>` heading, then run `update_release.bat patch` (or `minor` / `major`; plain `update_release.bat` releases the version already in `Cargo.toml`). It bumps the version, builds Pious on your PC, builds the installer with FFmpeg inside, commits, tags, pushes and uploads `Pious-Setup.exe`, `pious.exe` and `ffmpeg.zip` to a GitHub release whose notes come from the changelog. Everyone's app picks it up. See `SETUP.md` for the first-time setup.

## Credits

- [Heroicons](https://heroicons.com) (MIT), see `assets/icons/Heroicons-LICENSE.txt`
- [Manrope](https://github.com/sharanda/manrope) (SIL OFL 1.1), see `assets/fonts/Manrope-LICENSE.txt`
- [FFmpeg](https://ffmpeg.org) (GPL build from [gyan.dev](https://www.gyan.dev/ffmpeg/builds/)), downloaded on first use for recording, never bundled
- Emoji fonts from [rbxcustom-fontemojis](https://github.com/niksavc/rbxcustom-fontemojis) and emoji pictures from [emoji-datasource](https://github.com/iamcal/emoji-data), downloaded when you choose them
- Game fonts from [Pokémon Essentials](https://github.com/Maruno17/pokemon-essentials), [minecraft-font](https://github.com/idreesinc/minecraft-font) and [Monocraft](https://github.com/IdreesInc/Monocraft), and [Google Fonts](https://github.com/google/fonts), downloaded when you choose them

Pious isn't made by, affiliated with, or endorsed by Roblox Corporation.
