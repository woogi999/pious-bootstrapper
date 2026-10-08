# Making a Pious plugin

A plugin is a folder with a `plugin.json` and a web page. Pious shows the
page in its sidebar. The page runs walled off from Pious, in a sandboxed
frame. It can only reach Pious through messages, and only for the
permissions its `plugin.json` asks for.

The quickest start is **Settings → Plugins → Example plugin**. That
writes a working plugin to the plugins folder; open its folder and edit it.

## Where plugins live

```
%LOCALAPPDATA%\Pious\Bootstrapper\plugins\
└── my-plugin\
    ├── plugin.json        what it is and what it may do
    ├── index.html         its page (any name, set in plugin.json)
    ├── pious-plugin.js    the API file; Pious keeps it current
    └── …                  anything else the page uses (CSS, JS, pictures)
```

If you set `PIOUS_DATA` to keep Pious somewhere else, the plugins folder
moves with it.

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

## Plugins that come with Pious

**Macros** (macros and the auto-clicker) is built in and off until you turn
it on in Settings → Plugins. It has no folder: its page is part of Pious.

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
