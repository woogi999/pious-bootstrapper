# Tweaks

**Tweaks** gathers what bootstrappers do: frame rate, graphics, FastFlags,
Roblox mods (cursors, sounds, fonts, icons), Windows compatibility and
more.

## The switch at the top

Tweaks are **on by default**, and every tweak starts at Roblox's normal
value, so having them on changes nothing until you change a tweak.

While it's off, every tweak is greyed out and can't be changed.

Turning the switch **off** undoes everything tweaks did:

- every file tweaks changed in your Roblox versions goes back to Roblox's
  original (Pious backed each one up first),
- the frame rate cap and graphics quality go back to what Roblox's settings
  file had before,
- Windows' compatibility settings for Roblox are cleared, and its graphics
  card choice goes back to what it was,
- open Roblox windows get their own title, icon and normal CPU priority
  back within a few seconds.

A Roblox version that's running locks its files, so it's undone the next
time it starts. Everything else is undone right away.

## Reset to default

**Reset to default** (top of the Tweaks page) puts every tweak back to
Roblox's normal value and undoes what they changed, like turning them off
and on again with nothing set. Your saved FastFlag profiles are kept, but
none is in use afterwards.

## When tweaks apply

Most tweaks change Roblox's files, so they're put in place **right before
a game starts** (Roblox is updated first, so they go on the build that
actually runs). The Roblox window's title, icon and CPU priority change as
soon as a window opens.

Tweaks apply to games Pious starts itself. If games start through another
bootstrapper (Settings → Launching), that bootstrapper's own mods apply
instead.

## Performance

| Tweak | What it does |
| --- | --- |
| Frame rate limit | Unlocks the frame rate (set in Roblox's own settings file; Roblox refuses the old FastFlag) |
| Graphics API | Direct3D 11, Vulkan or OpenGL instead of Roblox's choice |
| Anti-aliasing | Forced samples, or **Off (×0)** for none at all |
| Texture quality, mesh detail | Forced levels |
| Graphics quality | Roblox's own graphics slider (1–10), set before each game |
| Render quality (21 levels) | Roblox's old hidden quality setting: 21 steps, lower than the slider's lowest and higher than its highest. Overrides the slider while set |
| Pause voxelizer | Lighting and shadows stop updating: a big frame gain, flatter lighting |
| Roblox priority | Normal, above normal or high CPU priority for Roblox |
| Graphics card | Which GPU Windows runs Roblox on, like Settings → System → Display → Graphics: let Windows decide, power saving, high performance, or a specific card. Set for every Roblox version; Windows reads it when Roblox starts |
| Gray sky, still grass, no grass | Less to draw |
| Ignore display scaling, Alt+Enter fullscreen | Display behavior |

Only FastFlags on Roblox's allowlist are used. The FastFlags tab lists any
flag Roblox refused at the last launch.

## Mods

Cursor, shift lock cursor, Roblox icon, sky, emoji style, old sounds, old
avatar background, font, window title and your own mods folder.

**Sky** replaces Roblox's default sky with a preset (clear day, sunset,
dusk, starry night, overcast, pastel, void) or your own: a folder with six
pictures named for their side (`back`, `front`, `left`, `right`, `up`,
`down`, or Roblox's `bk`, `ft`, `lf`, `rt`, `up`, `dn`; ready-made `.tex`
files work too). Games that set their own sky keep it, and **Gray sky**
(Performance) wins over any sky.

See **Roblox mods
and FastFlags** for how files are replaced, and **Plugins** for packs of
cursors, sounds and death sounds you can install as plugins.

## System

Windows compatibility (fullscreen optimizations, high DPI scaling) and
cleaning old Roblox logs and cache.
