# Mods and client tweaks

Pious changes the Roblox client it launches: FastFlags, a font, cursors,
sounds, the shift lock cursor and anything you put in its mods folder. It
does this to each Roblox version just before it starts, and keeps a record
so every change can be undone.

All of this is in **Tweaks**. The switch at the top of that page turns it
all on or off; off puts every file back the next time a game starts.

## How it works

1. Before a game starts, Pious checks Roblox's current version and, if the
   game follows the latest one, installs it first, like Bloxstrap. (Started
   out of date, Roblox would update itself into a new folder that has none
   of the tweaks.) Then it undoes whatever it changed in that Roblox version
   last time, using the originals it backed up.
2. It writes the files for the tweaks you have on, in this order (later
   ones win when two change the same file):
   1. `ClientSettings\ClientAppSettings.json` with your FastFlags (merged with flags
      another bootstrapper may have put there; Pious's win).
   2. The font.
   3. Presets: old cursors, old sounds, the old avatar background, emoji.
   4. The shift lock cursor.
   5. Your **mods folder**.
   6. The frame rate cap and graphics quality, in Roblox's own settings file
      (`%LOCALAPPDATA%\Roblox\GlobalBasicSettings_13.xml`, `FramerateCap` and
      `SavedQualityLevel`), like Fishstrap. Roblox refuses the old frame rate
      FastFlag. This happens after the account's own Roblox settings are put
      in place, so they don't undo it.
3. Each original file is copied to `.pious-backup\` inside the version folder
   before it's replaced, and listed in `.pious-tweaks.json`. Even if a write
   fails halfway, the record is already saved, so the next launch restores
   everything.

Roblox versions Pious installs live in
`Versions` in the data folder. Versions from Roblox's own
installer or another bootstrapper are changed the same way, and restored the
same way.

> A running copy of Roblox locks its files. If a version is already
> running, close it before changing tweaks, or Pious can't write them and
> says so.

## The mods folder

Tweaks → **Mods folder → Open** opens it:

```
<data folder>\Modifications\
```

Everything in it is copied over the Roblox version, **in the same folder
layout as the version folder**. Turn on the **Mods folder** switch to use
it.

```
Modifications\
├── content\
│   ├── sounds\
│   │   └── ouch.ogg                 the old "oof" death sound
│   ├── textures\
│   │   ├── MouseLockedCursor.png    shift lock crosshair
│   │   └── Cursors\KeyboardMouse\
│   │       ├── ArrowCursor.png      the pointer
│   │       └── ArrowFarCursor.png   the pointer over far things
│   └── fonts\
│       └── …                        font files, by Roblox's own names
└── ExtraContent\
    └── …
```

To find a file's path, open a Roblox version folder (Versions → right-click
a version → Open folder) and look for the file you want to replace. Put
your file at the same path under `Modifications`, with the same name.

### Common mods

| What | Path |
| --- | --- |
| Death sound | `content\sounds\ouch.ogg` |
| Walking, jumping, landing | `content\sounds\action_footsteps_plastic.mp3`, `action_jump.mp3`, `action_jump_land.mp3` |
| Mouse pointer | `content\textures\Cursors\KeyboardMouse\ArrowCursor.png`, `ArrowFarCursor.png` |
| Shift lock crosshair | `content\textures\MouseLockedCursor.png` |
| Emoji font | `content\fonts\TwemojiMozilla.ttf` |
| Avatar editor backdrop | `ExtraContent\places\Mobile.rbxl` |

Roblox renames and moves files now and then. If a mod stops working after a
Roblox update, check the path in the new version folder.

### Tips

- Keep sounds in the same format as the file you replace (`.ogg` for
  `.ogg`, `.mp3` for `.mp3`).
- Keep pictures the same size as the originals unless you know Roblox
  scales them; a 64×64 shift lock cursor is a safe choice.
- Empty a sound by replacing it with a silent file rather than deleting it.
- The mods folder wins over Pious's own presets, so a file there replaces
  the preset for the same path.

## Built-in mods

| Tweak | What it changes |
| --- | --- |
| Mouse cursor | The 2006 or 2013 pointer. |
| Shift lock cursor | A crosshair in the style of another game (Minecraft, Call of Duty, Battlefield, CS:GO, Valorant, Fortnite, old Roblox), in any color, or your own picture. |
| Roblox icon | The icon on Roblox's window and taskbar button: Pious, Bloxstrap, Fishstrap, Roblox's icons from 2008 to 2022, or your own picture. This doesn't change any file: Pious gives each Roblox window the icon when it opens. |
| Emoji style | Apple's or another set of emoji. |
| Old character sounds | The classic walk, jump and get-up sounds. |
| Old avatar editor background | The classic backdrop. |
| Font | One of Roblox's fonts, a preset game font, or any `.ttf`/`.otf`. |
| Roblox window title | What Roblox's window is called. No file changes either. |

The cursor, sound, avatar background and Roblox-icon presets come from
Fishstrap's open-source resources and are downloaded once, into
`cache\mods` in Pious's data folder. The shift lock presets are drawn by
Pious itself.

## FastFlags

Tweaks has presets that use only flags Roblox allows (graphics API,
anti-aliasing, textures, mesh detail, simpler lighting, gray sky, still
grass, DPI scaling, Alt+Enter) and a FastFlag editor with profiles. Every flag in the profile you use is written
to `ClientAppSettings.json` as is. You can import flags from a file, from
Bloxstrap, Fishstrap or Froststrap, or by pasting JSON or `Name=Value`
lines.

Roblox itself decides which flags it reads. Since late 2025 it only reads
the flags on its own allowlist from that file; Pious still writes every
flag you add, so nothing is dropped if Roblox changes that list. After each
launch Pious reads Roblox's log, and the FastFlags tab lists every flag
Roblox refused ("Denied local configuration") at the last launch.

## Undoing everything

- Turn off the switch at the top of Tweaks, then start a game: every file
  goes back.
- Or delete the version in Versions and install it again.
- Your mods folder and FastFlag profiles are never deleted by Pious.
