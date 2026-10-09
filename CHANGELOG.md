# Changelog

Every version of Pious and what changed. `update_release.bat` publishes the newest
section below as the release notes, and Pious shows it after updating.

Write each version as a `## <version>` heading followed by the changes,
newest version first.

## 1.0.2

### Fixed
- **Updating from 1.0.0 failed** with a Windows error saying Pious couldn't restart to update. Pious Setup now starts however it's started and asks for administrator rights itself (if you say no, it still updates your own install).
- **Games opened as the wrong account.** Roblox rolls out new builds a few accounts at a time, and Pious only checked the public build, so an account already given a newer one got an "out of date" client. Roblox then closed it, ran its own installer, and reopened the game signed in as whoever was signed in to Roblox's own app. Pious now checks the build each account is meant to run (asking Roblox directly, not its cache), installs it first, and also plays it when the version picked for a game is out of date. If Roblox still updates itself mid-launch, Pious says what happened and gets the update for the next Play.
- **Games set to always use one account** now say so when they launch over the account picked in the sidebar, and where to change it.
- **The input overlay's Done button was cut off** while moving it. It now sits in its own row under the keys.
- **Statistics** are joined when Pious moves its data folder (an update could leave the old history behind).
- **No recommendations** for new accounts or without accounts: Pious now shows what's popular on Roblox until it knows what you like.
- **What's new** showed raw `**` and backticks instead of bold text and code.
- **The keep-on-top pin** is back at the left of the title bar.
- **Your data can't be reset by another Pious any more.** An older Pious (like one still installed, or one started by an AI app) could fail to read data saved by a newer one and start over with an empty library. Now Pious notes which version saved your data, and an older one never saves over it; if a Pious can't read your games or accounts, it leaves the file alone instead of saving an empty one. Copies of unreadable files get their own dated names, so one never replaces another.

### New
- **Always use the LIVE channel** (Settings → General): keeps every account on Roblox's public version, so an account Roblox picked to try a new version early isn't made to update before it can play.
- **Mod maker** (Tweaks → Mods): recolor Roblox's interface (top bar, menus, chat, backpack, emotes, player list, voice chat, cursors, shift lock, loading screen, the logo) in one color or a gradient, with a live preview. Made from each Roblox version's own pictures, so it follows Roblox's updates.
- **FPS updates every 0.3 seconds** in the game stats overlay (it's measured over the last second, so it stays steady).
- **Plugins in the in-game overlay**: a plugin can add a panel (`"overlay"` in its plugin.json). **Macros** has one: run or stop your macros and the auto-clicker without leaving the game (turn it on from the Macros page, Settings → Overlay, or the overlay's **+** chips).
- **A tidier in-game overlay**: panels line up in even columns (left, right, bottom, and plugins at the top), all the same width with the same spacing, and slide into place when one changes size. A panel you move or resize stays where you put it.

### Fixed in overlays
- **The stats overlay ran off the right side of the screen** when placed there. Overlays now grow toward the middle of the screen (left on the right half, up on the bottom half) and always stay on screen.

### Changed
- **FPS moved to the game stats overlay**; the keys overlay shows keys, mouse, clicks and keys per second. If you had FPS on there, it's turned on in the stats overlay for you.
- The shift lock setting now says that games with their own shift lock (many battlegrounds games) keep their own crosshair.

## 1.0.1

### Fixed
- **Recording lag.** On AMD graphics every frame was copied off the graphics card, converted and copied back, which used more than a whole processor core and still dropped to about 43 of 60 frames a second. The whole screen is now captured with AMD's own capture straight into AMD's encoder: a steady 60 FPS at a quarter of the processor time. If AMD's capture can't start, Pious quietly falls back to the old way.
- **FFmpeg is built in.** No more downloading FFmpeg for recording: it's inside `pious.exe` and unpacked on first use.
- **Game pictures and other images** didn't load after moving to the new data folder. Pious now notices missing pictures and downloads them again by itself.
- **The Macros page was a white screen.** Plugin pages couldn't load their own files. Fixed, and plugins' background engines no longer start twice.
- **Multi-instance** only worked when Pious started before any Roblox window. It now works however Roblox was started (from Pious, another launcher or the website): Pious lets go of Roblox's one-window lock inside clients that already hold it.
- **Plugins look like Pious again**: the Macros page uses Pious's own styles and icons, exactly as before it became a plugin.
- **Tweaks:** when they're off, every tweak is greyed out (some weren't), and the long notice is gone.
- **Tweaks that didn't show up:** games started from roblox.com never got tweaks, and builds another Roblox window was using were skipped. Tweaks now apply to every launch, and to every installed Roblox version as soon as you change them. Replaced files are read-only so Roblox can't swap its own back. The Roblox icon now changes on the taskbar too, not only the title bar, and on every Roblox window.
- **Tweaks:** turning them off now undoes everything: files in Roblox, the frame rate and graphics quality in Roblox's settings file, Windows compatibility settings, and the title, icon and priority of open Roblox windows. New **Reset to default**. Tweaks are on by default, with every tweak at Roblox's normal value, so nothing changes until you change something.
- **AI apps no longer start Pious.** Opening Claude Code, Cursor and other AI apps started Pious in the background every time. Now nothing starts it unless you turn on **Start Pious when an AI app needs it**, and the apps connect at once while Pious is closed.
- **Notifications:** "Try one" could leave pop-ups, or Pious, stuck. Pop-up windows are made safely now, and a failure is shown instead of swallowed.
- **Emoji shortcodes in Roblox:** emoji came out blank (they were typed in two halves), and finishing a `:shortcode:` could erase the wrong character. Both fixed; Roblox's own shortcodes no longer get in the way.
- **Overlays** (game overlay, recording notices, input overlay, emoji list) could fail to appear or show empty. Fixed at the root: how their windows are made and when they're told to show.
- **Discord:** errors from Discord were taken for success, so a status that Discord refused never showed. Pious now checks Discord's answer and falls back when your Discord is older.
- **Damaged settings** no longer reset everything: whatever still reads is kept, and Pious says what was reset.

### New
- **Game search**: Add Game searches Roblox as you type, with icons, players and ratings. No more copying links from the website.
- **Game stats overlay**: FPS, the server's ping, players and frame rate, server location, time played, Roblox's CPU and memory, and a clock, drawn over the game (Settings → Overlay).
- **Graphics card** for Roblox: let Windows decide, power saving, high performance, or a specific GPU, like Windows' own Graphics settings.
- **Render quality with 21 levels**, Roblox's old hidden quality setting: lower than the slider's lowest and higher than its highest.
- **Anti-aliasing off (×0)**.
- **Custom skies**: seven presets or your own six pictures.
- **Pause voxelizer** (renamed from Simpler lighting) and **Roblox priority** now sit together in Tweaks → Performance.
- **Discord: show as Roblox or Pious.** Your game status comes from a "Roblox" app by default (with Roblox's logo), or from Pious's. Pious also registers itself with Windows and Discord when it starts.
- **Plugin UI kit**: plugin pages can use Pious's own stylesheet, components and icons (Help → Plugin UI kit).
- **Ban-risk warnings** on multi-instance (it asks first) and on Macros and the auto-clicker.
- **Terms and Privacy**: bans and third-party plugins are your own responsibility; be careful which plugins you install.
- **Pious Setup runs as administrator**, so it can always close Pious and Roblox and replace their files. Pious itself still runs as you.
- **Plugins** can bring Roblox files (death sounds, cursors, shift lock cursors, any sound or texture), themes, styles, icons and sounds, run an engine in the background, and (with full access) change anything in Pious. **Macros is entirely a plugin now**: its engine, page and your macros live in `plugins\macros`; Pious only gives it capabilities (input, hotkeys, timing, storage).
- **More cursors**: Bibata Modern Ice, Clean, Dot, FPS, Stoofs, tiny and white dots (from Voidstrap), black-and-white dot and purple cross (from Froststrap), with pictures to pick from.
- **Themes** and much more appearance: gradients, a second accent, any installed font, roundness, text size. Two themes come with Pious.
- **Help** inside Pious: installing, tweaks, themes, plugins, troubleshooting, the Privacy Policy and Terms of Service.
- **Region** for public servers: automatic, best ping, or a region.
- **Rich notifications**: a game's banner when a friend starts playing, chat-style messages.
- **Taskbar**: flash, an unread badge and download progress.
- **Keep Roblox up to date** in the background.
- **Crash reports** for Pious and Roblox, kept on your PC.
- **Auto arrange** Roblox windows across your screens.
- **No grass** performance tweak.

### Faster
- Recording starts at once (working encoders are remembered and checked in parallel), saves in about half the time for long videos, and FFmpeg runs below the game's priority. **MOV** is the default format.
- Pious does less in the background while you play.

### Installing
- One installer for every release: it downloads the newest Pious from GitHub. Pious is installed as plain files, and keeps your data in its install folder (moved over from `%LOCALAPPDATA%\Pious\Bootstrapper` by itself, nothing deleted).
- Releases no longer include a separate `ffmpeg.zip`; updating removes the old `ffmpeg.exe`.

## 1.0.0

The first release of Pious.

### Play
- Your Roblox games in one library, with favorites, collections and per-game launch settings (account, Roblox version, server).
- Private servers: save them, join with one click, or paste any server link with Join a Server Link (Ctrl L). Unnamed servers take the server's own name.
- Server hop, a public server browser and joining friends or any player.
- Several Roblox windows at once, each with its own account, all listed in **Instances**. Auto-rejoin and anti-AFK keep you in game.
- Copy a link to the exact server you're in, and see where the server is.
- Recommendations from what all your accounts play, with See more, plus Games your friends are playing.
- Desktop shortcuts for games (right-click a game).

### Friends
- See who's online and what they're playing, and join them. See the friends of one account, or of all your accounts in one list.
- Message friends in a chat window like Steam's, with the same look as the app (frost, background picture and size included).
- **Notifications**: Pious's own pop-ups when a friend messages you, sends a friend request or starts playing, or Roblox has new notifications. Click one to open it.
- **Appear online** to your Roblox friends while Pious is open.
- Emoji shortcodes: type :sob: like on Discord, in Pious's chats and in Roblox.

### Discord Rich Presence
- Your Discord status shows the game you're playing, as "Playing <game name>" or "Playing Pious", all set in one place with Bloxstrap's options.
- Allow activity joining: a Join game button so friends can hop into your server.
- Show Roblox account: the avatar and name of the account you're playing as.
- Roblox Studio on your status too, and "Playing Pious" while Pious is open and no game is.

### Accounts
- Several Roblox accounts, each with its own Roblox settings.
- Sign in with Quick Login, the Roblox sign-in window, or the sign-in already saved in your web browser.
- Streamer mode hides most of every username, by itself while OBS or other streaming software is open if you like.

### Versions and tweaks
- Install any Roblox version: pick from weao.xyz's current, previous and upcoming builds or Roblox's full deploy history.
- Uninstall other bootstrappers and Roblox from Pious.
- Tweaks in tabs like Settings: Launching, Presence, Performance, Mods, FastFlags, In game, Internet and System.
- Client tweaks: an FPS limit you can type (set in Roblox's own settings file, as Fishstrap does, because Roblox refuses the FastFlag), Roblox's graphics quality, graphics settings, fonts (with live previews and presets such as Roblox's own fonts, a Pokémon pixel font and Minecraft's), emoji styles including Apple's, old sounds and cursors, and mods.
- **Compatibility**: turn off Windows' fullscreen optimizations for Roblox, choose who handles high-DPI scaling, and give Roblox a higher CPU priority.
- **DNS for Roblox**: send Roblox's lookups to Cloudflare, Google, Quad9 and others, or your own servers, without changing the rest of your PC. Check your connection to Roblox and clear Windows' DNS cache.
- **Shift lock cursor**: crosshairs in the style of Minecraft, Call of Duty, Battlefield, CS:GO, Valorant, Fortnite and old Roblox, in any color, or your own picture.
- **Roblox icon**: give Roblox's window Pious's icon, Bloxstrap's, Fishstrap's, Roblox's from 2008 to 2022, or your own picture.
- Roblox is updated before a game starts, like Bloxstrap, so tweaks are on the build that runs.
- FastFlags: profiles, a searchable editor, a list of the flags Roblox refused at the last launch, and import from a file, from Bloxstrap, Fishstrap or Froststrap, or pasted JSON or Name=Value lines. Every flag you add is written to Roblox.
- **News**: new Roblox versions, Roblox's announcements and release notes, and Hyperion (Byfron) news, on a page of its own and on Home.

### In game
- The overlay (Home key) with friends, servers, recording, quick play, in-game options and **media controls** for Spotify, YouTube or anything else playing on your PC. Drag its panels smoothly, resize them from the corner, pin or hide them. **Quick switch** puts another of your accounts into the server you're in.
- **Input overlay**: the keys and mouse buttons you press, in slanted streamer-style keys, with CPS, KPS and **FPS**. Pick a layout, size, colors and animations; it slides in and out, and can show always, while playing, or only while recording or clipping.
- **Keybinds**: make a key or mouse button press something else in Roblox, for every game, one account or one game, on a page of their own.

### Record and clip
- Pious's own recorder can take over Roblox's F12: GPU capture and encoding, full encoder settings, and game-only audio.
- Clip the last few seconds with one key, or choose how many.

### Macros (a plugin that comes with Pious)
- Record macros from your keyboard and mouse, or build them step by step: keys, typing, clicks, pointer moves, scrolling, waits with random variation, loops, waiting for a color on screen, focusing Roblox and opening programs.
- Send them to the Roblox window you used last, every Roblox window, or only the windows of accounts you pick, in the background so you can keep using your PC.
- Import AutoHotkey scripts as macros, and save macros as AutoHotkey scripts.
- An auto-clicker with toggle or hold, click-while-you-hold-the-mouse, or extra clicks for each of yours; mouse buttons or keys, double clicks, random timing, a fixed spot, and click or time limits.

### Pious
- Keyboard shortcuts for everything, all changeable.
- Plugins (with a guide in docs/PLUGINS.md).
- **AI apps (MCP)**: connect Claude Desktop, Claude Code, Cursor or Codex in one click, or ChatGPT through a tunnel. AI apps can see your games and friends, launch and record, and play: look at a Roblox window, hear it, walk, press keys, type and click.
- Click the logo to go home (it spins as a real 3D shape), or set it to anything, a macro included.
- A see-through, frosted window, themes, fonts and a background picture. Fullscreen hides the title bar, leaving one button to leave it.
- Light on memory in the background: minimized or unfocused windows give back what they're not using, and the window in the tray is let go entirely.
- Links open in Pious or your browser, as you like.
- An installer, automatic updates from GitHub, and these release notes in the app.
