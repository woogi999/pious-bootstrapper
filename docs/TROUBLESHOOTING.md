# Troubleshooting

## Pious won't open, or opens empty

- Another copy may be running in the tray: click the Pious icon there.
- Interface errors are written to `ui-errors.log` in the data folder
  (Settings → About shows the folder).
- If Pious says your data couldn't be read, it kept everything that still
  reads and saved the original as `bootstrapper.corrupt.json`. See
  Settings and your files.

## A game doesn't start

- **"needs to sign in again"**: Accounts → sign that account in again.
- **"Install a Roblox version"**: Versions → Install version.
- With **Launch through** a bootstrapper (Settings), that bootstrapper
  starts the game; check it works on its own.

## Tweaks don't apply

- Tweaks go on right before a game starts, on games Pious starts itself.
- A Roblox version that's already running can't be changed: close it first.
- Roblox ignores FastFlags that aren't on its allowlist; the FastFlags tab
  lists the ones it refused.
- Turning Tweaks off, or **Reset to default**, puts everything back.

## Notifications or the overlay don't show

- Check they're on (Settings → General → Notifications, Settings → Overlay).
- **Try one** in Settings → General → Notifications shows samples. If Pious says it
  couldn't show the pop-up, the reason is in the message.
- The overlay's hotkey only works while a Roblox window is in front, unless
  you turn that off.

## Emoji shortcodes don't work in Roblox

- Turn on emoji shortcodes **in Roblox** (Tweaks → In game).
- Type `:` then the name, e.g. `:sob`. Pick with the arrows and Enter or
  Tab, or finish with `:` (`:sob:`).
- They only work in Roblox windows, while Pious is running.

## Recording

- **Nothing recorded / black video**: Settings → Recording → try another
  encoder, or the whole screen instead of the game window.
- **The recorder stopped: …**: Pious stops using an encoder that failed and
  picks the next one; press record again.
- FFmpeg's log for the current capture is in `data\cache\capture`.

## AI apps (MCP)

- **"Pious isn't running"**: open Pious and turn on **Let AI apps use
  Pious** (Settings → Plugins). AI apps don't start Pious by themselves
  unless you turn on **Start Pious when an AI app needs it**.
- More in AI apps (MCP).

## Plugins and themes

- A plugin or theme that doesn't read is listed with the reason. Fix its
  `plugin.json`/`theme.json` (watch for trailing commas) and it appears
  within a few seconds.
- **Macros are gone**: the Macros plugin folder (`plugins\macros`) was
  removed. Reinstalling Pious puts it back.

## Crashes

Settings → About → Crash reports lists what Pious recorded. To report a
problem, open the report, check it holds nothing you'd rather not share,
then **Report** (it opens a GitHub issue to fill in).
