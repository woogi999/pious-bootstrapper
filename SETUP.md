# Setting up to build and release Pious

Everything here is done once, on the PC you release from.

## 1. Tools

Install [Rust](https://rustup.rs) (with the Visual Studio C++ build tools it
asks for), [Node.js](https://nodejs.org), Git and the GitHub CLI
(`winget install GitHub.cli`). `update_release.bat` signs you in to GitHub
the first time it runs (GitHub.com, HTTPS, log in with a browser). The account needs write access to
`woogi999/pious-bootstrapper`.

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
   `minor` or `major`. If that version is already on GitHub, it asks whether
   to release it as the next one instead.

It checks the interface, builds Pious, makes the installer, commits, tags, pushes,
and creates the GitHub release with `Pious-Setup.exe`, `pious.exe` and
`ffmpeg.zip`, using the changelog section as the release notes. Installed
copies of Pious offer the update on their next start, and show the notes
after updating. The files also stay in `release\`.

The first build takes several minutes; later ones only rebuild what
changed. The steps live in `scripts/release.ps1`. GitHub Actions doesn't
build releases, so releases are made only by `update_release.bat`.
