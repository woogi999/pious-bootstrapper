# Plugin API reference

Everything a plugin's page and engine can ask Pious for. The basics
(folders, `plugin.json`, permissions in words) are in **Plugins and making
one**; this page lists every method and event.

## Its parts

A plugin can have:

- a **page** (`"page": "index.html"`): shown in Pious's sidebar while you
  look at it;
- an **overlay panel** (`"overlay": "overlay.html"`): a small page shown as a
  panel in the in-game overlay (`Home`), next to Friends, Recording and the
  rest. It's off until the user turns it on (the overlay's **+** chips,
  Settings → Overlay, or your page calling `pious.ui.overlay.show(true)`),
  and it can be moved, resized, pinned and hidden like Pious's own panels.
  The panel takes the height of your page's content (up to 60% of the
  screen). It has the same permissions as your page, and gets your engine's
  `send` messages too, so page and panel always agree;
- an **engine** (`"main": "engine.js"`): a script that runs in the
  background for as long as the plugin is on, whether its page is open or
  not. Macros, timers, hotkeys and anything that has to keep working live
  here.

All of them load `pious-plugin.js` (Pious keeps a current copy in every plugin's
folder) and use the global `pious`. The engine runs in a hidden page, so
`document` exists but nothing is shown; use `pious.sleep()` rather than
`setTimeout` for anything that must be on time (hidden pages' timers are
slowed down by the browser).

```js
// engine.js
pious.handle("run", async ({ name }) => { /* … */ return "Started"; });
pious.on("hotkey", ({ id, pressed }) => { /* … */ });
await pious.hotkeys.set([{ id: "go", keys: "F6", gameOnly: true }]);
```

## Calling Pious

`pious.call(method, ...args)` returns a promise; it rejects with a message
if the plugin lacks the permission or the call fails. The helpers below
(`pious.input.key(…)` and so on) are the same calls, spelled nicer.

### Always allowed

| Method | Does |
| --- | --- |
| `sleep(ms)` | Waits `ms` milliseconds (timed by Pious, accurate even in the hidden engine page). |
| `storage.read(name)` | The text of `data/<name>` in the plugin's folder, or `null`. |
| `storage.write(name, text)` | Saves it (written whole, never half). |
| `storage.list()` / `storage.remove(name)` | The files in `data/`; delete one. Names: letters, digits, `.`, `-`, `_`. |
| `send(message)` | Sends any JSON value to the plugin's other half (page ↔ engine): its `"message"` event. |
| `status(info)` | What the plugin is doing, as a chip in Pious's top bar: `{ text, active }`, or `null` to remove it. Clicking it opens the plugin's page. |
| `respond(rid, value, error)` | Answers a `"request"` (the `pious.handle` helper does this for you). |
| `ui.icons(names)` | Pious's icons as SVG markup: `{ name: svg }` (helpers: `pious.ui.icon(name)`, `pious.ui.icons([...])`). |
| `overlay.shown()` | Whether this plugin's overlay panel is turned on (`pious.ui.overlay.shown()`). |
| `overlay.show(on)` | Turns this plugin's overlay panel on or off (`pious.ui.overlay.show(true)`). |

### `read`, `launch`, `notify`, `navigate`, `links`

| Method | Permission | Does |
| --- | --- | --- |
| `snapshot()` | `read` | Your games, accounts, friends and running games (no sign-ins or paths). |
| `launch(gameId)`, `join(placeId)` | `launch` | Play a library game; join a place. |
| `toast(text)` | `notify` | A notice in Pious. |
| `hud(kind, text)` | `notify` | A notice over the game (`kind`: `"macro"`, `"autoclick"`, `"saved"`, `"error"`). |
| `navigate(page)` | `navigate` | Open a Pious page. |
| `openLink(url)` | `links` | Open a web address. |

### `input`: keys, mouse and Roblox windows

`target` is `null` (the real keyboard and mouse: whatever is in front) or a
window number from `windows.roblox()` (input goes straight to that window
in the background; the window doesn't need to be in front).

| Method | Does |
| --- | --- |
| `input.key(code, down, target)` | Presses (`down` true) or lets go of a key. Codes like `KeyW`, `Space`, `ShiftLeft`, `F5`, `Digit1`, `ArrowUp`. |
| `input.text(text, target)` | Types text (any characters, emoji included). |
| `input.button(button, down, target, at)` | A mouse button: `Left`, `Right`, `Middle`, `Back`, `Forward`. `at`: `{x, y}` on the screen (background only). |
| `input.move(x, y, relative, target)` | Moves the pointer to a screen spot, or by an amount (`relative`; turns a game's camera). |
| `input.scroll(notches, horizontal, target, at)` | Scrolls (positive = up/right). |
| `input.cursor()` | `{ x, y }` where the pointer is. |
| `input.pixel(x, y, target)` | The color at a screen spot (or in a window's picture) as `#RRGGBB`, or `null`. |
| `input.windowPoint(window, fx, fy)` | The screen spot at fractions (0–1) of a window. |
| `input.watch(on)` | While on, the plugin gets `"input"` events for the real keyboard and mouse. |
| `input.recordStart()` / `input.recordStop()` | Records the real keyboard and mouse; stop returns the events (see below). |
| `windows.roblox()` | The Roblox windows: `[{ window, pid, account: { id, username } \| null, game, front, last }]` (`last`: the one used most recently). |
| `windows.focus(window)` | Brings a window to the front. |
| `windows.foreground()` | `{ window, pid, roblox }` for the window in front. |
| `idle()` | Seconds since you last touched the keyboard or mouse. |

Recorded events: `{ ms, kind: "key", code, down }`, `{ ms, kind: "button",
button, down }`, `{ ms, kind: "move", x, y }`, `{ ms, kind: "scroll",
amount, horizontal }`; `ms` counts from the start.

### `hotkeys`

| Method | Does |
| --- | --- |
| `hotkeys.set([{ id, keys, gameOnly }])` | The plugin's hotkeys (replacing its previous ones). `keys` like `F6` or `Control+Shift+KeyM`; `gameOnly` keys only work (and are kept from the game) while Roblox is in front. Each press and release comes as a `"hotkey"` event `{ id, pressed }`. Problems are returned as a list of messages. |

### `run`

| Method | Does |
| --- | --- |
| `system.open(target)` | Opens a program, a file or a web address, like double-clicking it. |

### `full`: change anything in Pious

A plugin with `full` can do everything Pious's own window can. Settings
lists it as **Full access to Pious**; only turn on plugins you trust.

| Method | Does |
| --- | --- |
| `settings.get()` | Every setting (what Settings and Tweaks change). |
| `settings.set(patch)` | Changes settings: only the keys given, deeply merged. |
| `app.invoke(command, args)` | Any of Pious's own commands, exactly as its interface calls them. |
| `ui.css(text)` | A stylesheet for every Pious window while the plugin is on (`""` removes it). |

## Events

`pious.on(event, listener)` returns a function that stops listening.

| Event | Permission | Value |
| --- | --- | --- |
| `snapshot` | `read` | The summary, whenever something changes. |
| `hotkey` | `hotkeys` | `{ id, pressed }` |
| `input` | `input` (after `input.watch(true)`) | `{ code, down }` (`code` is a key code or `MouseLeft`, `MouseRight`…) |
| `message` | none | What the other half sent with `send`. |
| `request` | none | `{ rid, method, args }`: Pious (or another part of it) asks the engine for something. Use `pious.handle(method, fn)`; whatever `fn` returns (or throws) is the answer. |

## Requests Pious sends

Pious itself asks plugins for things with requests. A plugin that
`provides` a feature answers that feature's requests:

| Feature | Requests |
| --- | --- |
| `macros` | `list()` → `[{ id, name, hotkey, steps, running }]`; `run({ id } or { name })` → text (starts it, or stops it if running); `stop()` (everything); `status()` → `{ running: [ids], clicking, recording }` |

AI apps' macro tools, the overlay's macro buttons and "Clicking the logo:
Run macro" all go through these, so a different macros plugin can replace
Pious's.
