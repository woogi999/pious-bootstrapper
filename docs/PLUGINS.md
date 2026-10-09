# Plugins and making one

A plugin is a folder with a `plugin.json`. It can bring any of:

- **a page** in Pious's sidebar, which runs walled off from Pious, in a
  sandboxed frame, and reaches Pious only through messages, for the
  permissions its `plugin.json` asks for;
- **Roblox files**: a death sound, cursors, the shift lock cursor, or any
  sound or texture (see *Roblox files* below);
- **themes**, a **stylesheet**, **icons** and **sounds** for Pious itself;
- **a feature built into Pious** (`provides`), like Macros.

A plugin that can't be read never stops Pious: Settings → Plugins lists it
with the reason, and nothing from it is used.

## Installing and removing plugins

- **Install:** unzip the plugin's folder into the plugins folder (Settings →
  Plugins → **Plugins folder**), then turn it on in Settings → Plugins.
  Pious notices new folders within a few seconds.
- **Turn off:** its switch in Settings → Plugins. Its Roblox files are taken
  back out the next time a game starts.
- **Remove:** delete its folder.

The quickest start is **Settings → Plugins → Example plugin**. That
writes a working plugin to the plugins folder; open its folder and edit it.

## Where plugins live

```
<Pious's install folder>\plugins\      (or <data folder>\plugins without installing)
└── my-plugin\
    ├── plugin.json        what it is and what it may do
    ├── index.html         its page (any name, set in plugin.json)
    ├── pious-plugin.js    the API file; Pious keeps it current
    └── …                  anything else the page uses (CSS, JS, pictures)
```

Settings → Plugins → **Plugins folder** opens it.

After you add or change a plugin, turn it on in **Settings → Plugins**.
Pious checks the folder each time that page opens.

## plugin.json

```json
{
  "id": "my-plugin",
  "name": "My plugin",
  "version": "1.0.0",
  "author": "You",
  "description": "One sentence about what it does.",
  "page": "index.html",
  "icon": "puzzle",
  "permissions": ["read", "notify"]
}
```

| Field | Needed | What it is |
| --- | --- | --- |
| `id` | No (the folder name is used) | A unique ID. Changing it makes Pious treat it as a new plugin, which starts off. |
| `name` | No (the folder name is used) | What the sidebar and Settings show. |
| `version`, `author`, `description` | No | Shown in Settings → Plugins. |
| `page` | No | The HTML file to show, relative to the folder. It must stay inside the folder. Without a page, the plugin has nothing to show. |
| `icon` | No (`puzzle`) | One of Pious's icon names, e.g. `game`, `friends`, `server`, `chat`, `stats`, `sparkles`, `bell`, `media`, `keyboard`, `macros`, `news`. |
| `permissions` | No (none) | What the page may ask Pious for. See below. Unknown names are ignored. |
| `provides` | No | A feature built into Pious this plugin turns on: `macros`. |
| `client` | No | A folder in the plugin laid out like a Roblox version folder; each file replaces Roblox's at the same path. |
| `replace` | No | Roblox file → plugin file, e.g. `{ "content/sounds/ouch.ogg": "oof.ogg" }`. |
| `death_sound` | No | A sound for when your character dies (`content/sounds/ouch.ogg`). |
| `cursor`, `cursor_far` | No | The mouse pointer pictures. |
| `shiftlock` | No | The shift lock cursor picture. |
| `themes` | No | Folders in the plugin, each with a `theme.json` (see Making a theme). |
| `css` | No | A stylesheet applied to Pious's windows while the plugin is on. |
| `icons` | No | Pious icon name → picture in the plugin (SVG or PNG). |
| `sounds` | No | Pious sound → sound file (`notification`). |

Every path is relative to the plugin's folder and must stay inside it.

A plugin.json that can't be read doesn't stop Pious: Settings → Plugins
shows the plugin with the reason it can't be used.

## Permissions

| Permission | Lets the page |
| --- | --- |
| `read` | Read a summary of your games, accounts, friends, running games and macro names, and get live updates. |
| `launch` | Start a game from your library, or join a place. |
| `notify` | Show a notice in Pious (prefixed with the plugin's name). |
| `navigate` | Open one of Pious's pages. |
| `macros` | Run one of your macros by name (needs the Macros plugin on). |
| `input` | Press keys, click, move and scroll (to the front window, or straight into a Roblox window in the background), read the screen, record the keyboard and mouse, and see Roblox's windows. |
| `hotkeys` | Use keyboard shortcuts. |
| `run` | Open programs, files and web links. |
| `full` | Do anything Pious can: change every setting, call any of Pious's commands, restyle it. |
| `links` | Open a web link (in Pious or your browser, as set in Settings). |

Ask only for what the plugin needs. Settings lists each plugin's
permissions in words before it's turned on.

A plugin can never see your Roblox sign-ins, passwords or session tokens,
change Pious's settings, or reach your files.

## The page

Load the API file first, then use the global `pious` object:

```html
<script src="pious-plugin.js"></script>
<script>
  // Ask once…
  const snap = await pious.call("snapshot");

  // …or follow along as things change.
  const stop = pious.on("snapshot", (snap) => draw(snap));
</script>
```

`pious.call(method, ...args)` returns a promise. It rejects with a message
if the plugin lacks the permission or the call fails.

| Method | Permission | Does |
| --- | --- | --- |
| `snapshot` | `read` | Returns the summary below. |
| `launch(gameId)` | `launch` | Plays a library game with its own setup (its `id` from the snapshot). |
| `join(placeId)` | `launch` | Joins any place by its place ID, as the play-as account. |
| `toast(text)` | `notify` | Shows a notice (up to 200 characters). |
| `navigate(page)` | `navigate` | Opens a page: `home`, `games`, `servers`, `friends`, `accounts`, `versions`, `tweaks`, `keybinds`, `news`, `running` (Instances), `settings`, `macros`. |
| `runMacro(name)` | `macros` | Starts (or stops) the macro with that name. Case doesn't matter. |
| `openLink(url)` | `links` | Opens a web address. |

`pious.on("snapshot", listener)` (needs `read`) calls `listener` with a new
summary whenever something changes, and returns a function that stops it.

### The snapshot

```js
{
  games:    [{ id, name, place_id, favorite, last_played }],
  accounts: [{ id, username, display_name, nickname }],
  friends:  [{ id, username, display_name, status, playing, place_id }],
  running:  [{ id, game, started }],
  macros:   [{ name }],
}
```

`status` is `in_game`, `in_studio`, `online` or `offline`. `playing` is the
game a friend is in, when Roblox shares it. Dates are ISO 8601 strings.

### Looking like Pious

The page is told Pious's colors when it loads. They're set as CSS variables
on `<html>`, as space-separated `r g b` so you can add transparency:

```css
body { color: rgb(var(--text)); background: transparent; }
.card { background: rgb(var(--surface) / 0.06); border-radius: 12px; }
button { background: rgb(var(--accent)); color: rgb(var(--accent-ink)); }
```

The variables are `--bg`, `--text`, `--muted`, `--faint`, `--accent`,
`--accent-ink`, `--surface` and `--panel`. `<html>` also gets the class
`pious`. Keep the page background transparent so Pious's glass shows
through.

## Rules of the sandbox

- The page runs without access to Pious's own window, storage or files.
- Scripts and styles must come from the plugin's folder (or be inline).
- `localStorage` works for the plugin's own settings.
- Calls are answered in order; there's no limit on how often you call
  `snapshot`, but prefer `pious.on("snapshot", …)` to polling.

## Sharing a plugin

Zip the folder. Whoever installs it unzips it into their plugins folder
(Settings → Plugins → Plugins folder) and turns it on. Since `pious-plugin.js`
is rewritten by Pious, there's no need to ship your own copy, but it does
no harm.

## Roblox files

A cursor pack, death sound or sound pack is a plugin with Roblox files and
nothing else:

```json
{
  "id": "classic-oof",
  "name": "Classic oof",
  "description": "The old death sound and the 2013 cursor.",
  "death_sound": "oof.ogg",
  "cursor": "cursor.png",
  "cursor_far": "cursor-far.png"
}
```

They're put in place with the tweaks, right before a game starts, while
the plugin is on **and** Tweaks are on, and undone the same way: turn the
plugin or Tweaks off and Roblox's own files come back. Order: Pious's own
mods first, then plugins (by name), then your mods folder, which wins.
Paths must stay inside Roblox's folder.

## Plugins that come with Pious

**Macros** (macros and the auto-clicker) is a plugin in `plugins\macros`,
off until you turn it on in Settings → Plugins. All of it lives in that
folder: its engine (`engine.js`: running macros, the auto-clicker,
recording, AutoHotkey import and export), its page, and your macros
(`data\macros.json`). Pious only gives it capabilities, the same ones any
plugin can ask for: pressing keys and clicking (also straight into Roblox
windows in the background), recording the keyboard and mouse, hotkeys,
accurate timing and storage. Delete the folder and Macros is gone; put it
back (or reinstall Pious) and it returns. AI apps' macro tools, the
overlay and the logo's "Run macro" ask whichever plugin provides
`macros`, so another plugin can replace it.

## Engines: plugins that run in the background

Add `"main": "engine.js"` to `plugin.json` and that script runs for as
long as the plugin is on, whether its page is open or not. Use it for
anything that must keep working: hotkeys, timers, watching games. A plugin
can change anything in Pious with the `full` permission. Every method and
event is in the **Plugin API reference**.

## When something's wrong

- **It doesn't show in the sidebar.** Turn it on in Settings → Plugins, and
  check it has a `page`.
- **"It has no plugin.json" / "doesn't read".** The file is missing or
  isn't valid JSON (watch for trailing commas).
- **"Its page is missing."** The `page` path doesn't exist inside the
  folder.
- **A call says it "didn't ask to…".** Add the permission to
  `permissions`, then turn the plugin off and on.
- Interface errors are written to `ui-errors.log` in Pious's data folder.
