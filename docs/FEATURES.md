# Notifications, overlays, recording and more

## Notifications

Pious's own pop-ups (Settings → General → Notifications) appear in the corner of the
screen, over games too if you like. Each kind has its own look:

- **Messages** look like a messaging app: the sender's picture, name and the
  message. Click to reply in Pious's chat window.
- **A friend started playing** shows the game: its banner, icon, name and
  how many are playing. Click to join them.
- **Friend requests** and **Roblox notifications** have their own cards.

Choose **Rich** (each kind its own look) or **Compact** (one small card for
everything), how long they stay, and a sound. **Try one** shows samples.
Plugins and themes can replace the sound.

## Taskbar

Settings → General → Taskbar:

- **Flash** the taskbar button when something arrives while Pious isn't in
  front.
- **Badge**: a red count on the taskbar button of pop-ups you haven't seen,
  cleared when you open Pious.
- **Progress**: downloads (Roblox versions) show on the taskbar button.

Windows doesn't show badges with small taskbar buttons on Windows 10, and
some taskbar replacements show none of these; Pious just carries on.

## Regions

Where public servers are joined: Settings → General → **Server region**, or
per launch in the Play with… dialog.

- **Automatic**: Roblox's own matchmaking (as without Pious).
- **Best ping**: the server Roblox reports the lowest ping for.
- **A region** (US East, Europe, Asia…): Pious checks a handful of servers
  (asking Roblox which address each has, as the account that's about to
  play, then where that address is) and joins the first one in that region.
  If none is, Roblox picks as usual and Pious says so.

Regions apply only to public servers when you don't pick a specific one:
joining a friend, a server link or a private server isn't affected. Server
hopping and rejoining follow the setting.

## Auto arrange

Instances → **Arrange** lays every Roblox window out over your screens:
**Grid** (sized to stay close to 16:9), **Side by side**, **Stacked** or
**Cascade**. With several monitors, windows are shared out, bigger screens
taking more. Windows are kept inside each screen's work area (never under
the taskbar or off-screen). Pious's own instances come first, oldest first.

## Keeping Roblox up to date

Settings → General → **Keep Roblox up to date**: every half hour Pious
checks Roblox's current version and, if it isn't installed, installs it in
the background, once. A failed update isn't retried for six hours. You're
only told when it's done. (Pious also updates Roblox right before a game
starts, as before.)

Pious always resolves the public (LIVE) channel: an account's rollout
channel is ignored, and the client is told it's on LIVE before each launch
(the same registry value Roblox's installer and Bloxstrap use).

## Choosing a version

Every game Pious starts itself runs one build, picked in this order: the
game's own version or **version profile** (a name pointing at a build
GUID, `version_profiles` in `bootstrapper.json`), then the pinned build
(`pinned_version_guid`), then the default version, then LIVE's current
build. A chosen or pinned build is never checked online: if its folder in
`%LOCALAPPDATA%\Pious\Versions\version-<GUID>` is complete it starts
straight away, otherwise it's downloaded first. Every downloaded file is
checked against Roblox's `rbxManifest.txt`, and an install with a missing or
different file fails. Before each start Pious writes the GUID to the
folder's `version.txt` and to `Software\Roblox\RobloxPlayer\Version` (under
HKEY_CURRENT_USER, and under HKEY_LOCAL_MACHINE's WOW6432Node when a
machine-wide Roblox install made that key).

To go back to an older build, pick it in Versions → builds (Roblox's
`DeployHistory.txt`; since March 2026 Roblox hides new builds' GUIDs
there, so those come from weao.xyz) and choose it for a game, the default
or a profile. Roblox's servers can turn away builds older than LIVE.

## Crash reports

When Pious itself or a Roblox window it started crashes, Pious writes a
report (Settings → About → Crash reports): what happened, the versions, and
the end of the relevant log, with your user name and any Roblox session
removed. Open one to read it, or **Report** to open a GitHub issue you fill
in yourself. Nothing is sent automatically. Turn them off in Settings →
About.

Pious tells a crash from a normal close by how the Roblox process ended
(Windows reports an error code for crashes).

## Adding games

**Add Game** searches Roblox as you type, like the search on roblox.com, and
shows each game's icon, creator, players and how liked it is; pick one (or
double-click it) to add it. Pasting a game's link or place ID still looks
that game up directly.

## Multi-instance

Run several Roblox clients at once (Tweaks → Launching). Roblox normally
allows one: each client keeps a named "singleton" open, and a new one that
finds it hands its game over and quits. Pious takes that name before
Roblox does, and when a client already has it (Roblox started before Pious,
from another launcher, or from the website), Pious closes it inside that
client, so the next one starts on its own.

**Ban risk:** Roblox doesn't support multi-instance and some games kick or
ban for it. Pious asks before turning it on.

## Game stats overlay

Settings → Overlay → **Game stats on screen** draws live numbers over the
game, like the keys overlay: pick which, their order, a row or a column,
size, colors, and whether recordings show it.

| Stat | Where it comes from |
| --- | --- |
| FPS | Frames the game's window showed in the last second, updated every 0.3 seconds |
| Ping | The server's average ping, as Roblox's public server list reports it. Roblox servers don't answer pings, so this is the server's own figure. Private servers aren't listed, so they show **Private** |
| Players | Players in your server and its size (server list) |
| Server FPS | How fast the server runs (server list) |
| Server location | The city of your server |
| Time played | Since the game opened |
| Roblox CPU, Roblox memory | What Roblox uses on your PC |
| Clock | The time |

Server figures refresh every 30 seconds. Games Pious didn't start show
FPS, CPU, memory and the clock only.

## Discord

Settings → Presence → **Show as** picks the Discord app your game status
comes from: **Roblox** (the default; its name and logo head the card) or
**Pious**. Separately, **Activity name** picks whether Discord says
"Playing *game name*" or the app's name. Pious registers itself with
Windows and Discord when it starts (its app ID and a `discord-<app>` link
to `pious.exe`), the way Discord's own libraries do; the Roblox app's link
is left alone if another launcher already owns it.

## Recording

Pious records with your graphics card through Desktop Duplication, into
short segments, so saving a clip or recording copies what's recorded
instead of encoding it again.

- **Format:** MOV by default. MP4 is the same speed (both are a copy of the
  recording); MKV survives a crash mid-save best.
- FFmpeg is built into Pious; nothing to set up.
- On AMD graphics, the whole screen is captured with AMD's own capture,
  straight into AMD's encoder, so frames never leave the graphics card (a
  full 60 FPS on integrated Radeon graphics, at a quarter of the processor
  time of copying frames out). Recording only part of the screen uses
  Desktop Duplication instead.
- Starting is quick: the encoders that work are remembered between runs.
- FFmpeg runs below normal priority, so Roblox comes first.
- Saving doesn't rewrite the file a second time, so long recordings save in
  about the time it takes to copy them.
- For the lightest recording: H.264, the graphics card's encoder, encoder
  speed **Fast** or **Fastest**, and your game's own resolution.
