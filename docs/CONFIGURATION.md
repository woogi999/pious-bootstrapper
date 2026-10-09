# Settings and your files

Every setting is in the app (**Settings** and **Tweaks**); you never need to
edit a file. This page says where things are kept, and what Pious does when
something is damaged.

## The data folder

Installed Pious keeps your data in `data` inside its install folder. A
`pious.exe` run without installing uses `%LOCALAPPDATA%\Pious\Bootstrapper`.
Settings → About shows the exact folder. To keep everything somewhere else
(say, a USB stick), set the `PIOUS_DATA` environment variable to that
folder.

| File or folder | What it holds |
| --- | --- |
| `bootstrapper.json` | Your games, servers, accounts list, versions and every setting |
| `Versions\` | Roblox versions Pious installed (unless you chose another folder) |
| `Modifications\` | Your own Roblox mods (see Roblox mods) |
| `RobloxSettings\` | Each account's own Roblox settings, when turned on |
| `tweaks-settings.json` | The Roblox settings values tweaks replaced, to put them back |
| `cache\` | Pictures, friends, chats, build lists, downloaded fonts and mods. Safe to delete |
| `crashes\` | Crash reports |
| `stats.jsonl` | Your play statistics |
| `mcp-token.txt` | The secret AI apps use to reach Pious. Delete it for a new one |
| `ui-errors.log` | Errors from the interface, for troubleshooting |

Account sign-ins are **not** in this folder: they're in Windows
Credential Manager.

`plugins` and `themes` sit next to `pious.exe` when Pious is installed (in
the data folder otherwise).

## When something is damaged

Pious checks what it loads and never refuses to start because of it:

- **`bootstrapper.json` partly unreadable** (say, a setting with a value
  Pious doesn't understand): every game, server, account, version and
  setting that still reads is kept; only the broken parts go back to their
  defaults. The original file is saved next to it as
  `bootstrapper.corrupt.json`, and Pious says what was reset.
- **Not readable at all:** it's backed up the same way and Pious starts
  fresh. If the file can't even be opened (locked by another program),
  Pious leaves it alone and doesn't save over it.
- **Cached files** that don't read are deleted and fetched again.
- **A tweaks record** inside a Roblox version that's damaged: Pious puts
  back every original it backed up anyway.
- **Plugins and themes** that don't read are listed with the reason and
  left unused.

Saving always writes a new file first and swaps it in, so a crash while
saving can't leave a half-written file.

## Settings worth knowing

- **Settings → General:** start with Windows, what the close button does,
  what Pious does when a game starts, keep Roblox up to date in the
  background, where public servers are joined (region), how Auto arrange
  lays out windows.
- **Settings → Appearance:** themes, colors, gradient, font, roundness,
  see-through window and blur. See Customizing Pious.
- **Settings → General → Notifications and Taskbar:** pop-ups, their look, sound, and the taskbar
  (flash, badge, progress).
- **Settings → Recording:** see Notifications, regions and more.
- **Settings → Plugins:** plugins, AI apps (MCP).
- **Settings → About:** updates, crash reports, statistics, the changelog.
