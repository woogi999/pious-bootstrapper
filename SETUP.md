# Setting up to build and release Pious

Everything here is done once, on the PC you release from.

## 1. Tools

Install [Rust](https://rustup.rs) (with the Visual Studio C++ build tools it
asks for), [Node.js](https://nodejs.org) and Git. Your Git login needs push
access to `woogi999/pious-bootstrapper`. Releases themselves are built by
GitHub Actions, so the release PC doesn't need Rust for that (only for
`-Local` test builds).

## 2. Discord presence

Everyone's Discord status goes through one Discord application, Pious's.
Its name is what Discord shows when someone picks "Playing Pious".

1. Go to <https://discord.com/developers/applications> and sign in.
2. **New Application**, name it `Pious`, accept the terms, **Create**.
3. Optional: upload the logo as the **App Icon** and **Save Changes**.
   (The big picture on the status is the game's own, so no other art is
   needed.)
4. On **General Information**, copy the **Application ID**.
5. Paste it into `APP_ID` at the top of `src-tauri/src/core/discord.rs`,
   then build. The ID isn't a secret, so it's fine in the repository.

## 3. Build and try it

Double-click **`test.bat`**. It builds a development copy of Pious and opens
it. Changes to the interface (`src\`) show up live; changes to the Rust side
rebuild and restart it by themselves.

## 4. Release

1. Write what changed in `CHANGELOG.md`, under a `## <version>` heading at
   the top (newest first). If you forget, Notepad opens for it.
2. Double-click **`update_release.bat`** to release the version in
   `src-tauri/Cargo.toml`, or run `update_release.bat patch` (1.0.0 → 1.0.1),
   `minor` or `major`. If that version is already tagged, it asks whether
   to release it as the next one instead.

The script bumps the version, checks the interface, commits, tags `vX.Y.Z`
and pushes the tag. **GitHub Actions does the rest**
(`.github/workflows/release.yml`, also runnable by hand from the Actions
tab): it checks the tag matches `src-tauri/Cargo.toml`, builds Pious and the
installer, and publishes the release, using the changelog section as its
notes. Follow it under the repository's Actions tab; it takes several
minutes.

### What a release contains

| File | What it is |
| --- | --- |
| `manifest.json` | `{ "version", "files": [{ name, kind: "app", sha256, size }] }` |
| `pious-X.Y.Z-win-x64.zip` | The app as loose files: `pious.exe` (FFmpeg built in), `docs\`, `themes\`, `plugins\`, `version.txt` |
| `Pious-Setup.exe` | The installer. The same program every release: it downloads the latest release (checking SHA-256), so it never needs rebuilding for a new version |
| `pious.exe` | The bare app, for copies run without installing (they update themselves from it) |

Installed copies keep their own `pious-setup.exe` and use it to update:
Pious starts it, it downloads the release named in `manifest.json`, replaces
the files and reopens Pious. The `data\` folder in the install folder (your
library and settings) is never touched.

### Testing a release on this PC

`update_release.bat -Local` builds the same four files into `release\`
without committing, tagging or uploading anything (`scripts/package-release.ps1`
does the packaging, for both this and the workflow). The installer fetches
from GitHub, but if a `pious-*-win-x64.zip` sits next to it it offers to
install from that when GitHub can't be reached.

### FFmpeg

FFmpeg is built into `pious.exe`: `src-tauri/build.rs` compresses
`installer\vendor\ffmpeg.exe` (or the file `PIOUS_FFMPEG` names) into the
app, and Pious unpacks it to `data\tools` on first use, checked by its
SHA-256. `scripts\get-ffmpeg.ps1` downloads it (gyan.dev's "essentials"
build); the release workflow and `-Local` run it before building. Without
it, builds still work and recording falls back to downloading FFmpeg.
The first build after FFmpeg changes takes a few extra minutes to compress
it; later builds reuse the result.

### Pious Setup runs as administrator

Setup starts as whoever starts it (`installer/pious-setup.manifest` says
"asInvoker") and then asks Windows to run it again as administrator
(`relaunch_elevated`), so it can close every Pious and Roblox process and
replace files in use. Demanding administrator in the manifest instead
would make Windows refuse Pious 1.0.0, which starts Setup the plain way
("requires elevation"). It only asks when the signed-in account is an
administrator, and carries on without if you say no. Setup opens Pious
again through Explorer, so Pious (and Roblox) never run as administrator.
