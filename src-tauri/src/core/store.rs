//! Disk persistence for Pious's saved data and the artwork cache.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::core::model::Bootstrapper;

/// The folder Pious is installed in, when it was installed (loose files:
/// `pious.exe`, `docs`, `themes`, `plugins`, `data`…). `None` for a lone
/// `pious.exe` or a development build.
pub fn install_root() -> Option<PathBuf> {
    static ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
    ROOT.get_or_init(|| {
        let exe = std::env::current_exe().ok()?;
        let dir = exe.parent()?.to_path_buf();
        ["install.json", "pious-setup.exe", "uninstall.exe"].iter().any(|f| dir.join(f).is_file()).then_some(dir)
    })
    .clone()
}

/// Where older builds kept everything: `%LOCALAPPDATA%\Pious\Bootstrapper`.
pub fn legacy_data_dir() -> PathBuf {
    dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("Pious").join("Bootstrapper")
}

/// Whether Pious can write in `dir` (an install in Program Files can't).
fn writable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(format!(".write-test-{}", std::process::id()));
    let ok = std::fs::write(&probe, b"ok").is_ok();
    let _ = std::fs::remove_file(probe);
    ok
}

/// Root directory for all Pious data: `data` in the install folder, or
/// `%LOCALAPPDATA%\Pious\Bootstrapper` for a lone exe (or an install folder
/// Pious can't write to). `PIOUS_DATA` (or the older `PIOUS_LIBRARY_DATA`)
/// overrides it, which is useful for portable installs and testing.
pub fn data_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        if let Some(dir) = std::env::var_os("PIOUS_DATA").or_else(|| std::env::var_os("PIOUS_LIBRARY_DATA")) {
            return PathBuf::from(dir);
        }
        match install_root().map(|root| root.join("data")) {
            Some(dir) if writable(&dir) => dir,
            _ => legacy_data_dir(),
        }
    })
    .clone()
}

/// The data folder for a helper process (`--mcp`) that must not create or
/// move anything: the install's `data` once Pious has moved in there, else
/// the old place.
pub fn data_dir_readonly() -> PathBuf {
    if let Some(dir) = std::env::var_os("PIOUS_DATA").or_else(|| std::env::var_os("PIOUS_LIBRARY_DATA")) {
        return PathBuf::from(dir);
    }
    match install_root().map(|root| root.join("data")) {
        Some(dir) if dir.join("bootstrapper.json").is_file() => dir,
        _ => legacy_data_dir(),
    }
}

pub fn data_file_readonly() -> PathBuf {
    data_dir_readonly().join("bootstrapper.json")
}

/// Whether the data folder was chosen with `PIOUS_DATA`.
fn overridden() -> bool {
    std::env::var_os("PIOUS_DATA").or_else(|| std::env::var_os("PIOUS_LIBRARY_DATA")).is_some()
}

/// Plugins: next to Pious when installed (so they're plain folders anyone
/// can add or remove), else in the data folder.
pub fn plugins_dir() -> PathBuf {
    beside_pious("plugins")
}

/// Themes, like plugins.
pub fn themes_dir() -> PathBuf {
    beside_pious("themes")
}

fn beside_pious(name: &str) -> PathBuf {
    match install_root() {
        Some(root) if data_dir().starts_with(&root) => root.join(name),
        _ => data_dir().join(name),
    }
}

/// Brings older data folders to where this copy keeps its data. Runs once,
/// at start-up, in the copy of Pious that owns the data (never in helper
/// processes). Nothing is deleted: whatever can't be moved stays where it
/// was, with a note saying where the rest went.
pub fn migrate() {
    // A data folder chosen with PIOUS_DATA (a portable copy, a test) is
    // used as it is: the usual folders belong to the normal install and
    // are never moved into it. (Moving them there once took a real
    // library into a throwaway test folder.)
    if overridden() {
        return;
    }
    let root = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("Pious");
    // Older builds kept everything in Pious\Library.
    let library = root.join("Library");
    let legacy = legacy_data_dir();
    if library.join("library.json").is_file() {
        move_old_folder(&library, &legacy);
    }
    let target = data_dir();
    if target != legacy && legacy.join("bootstrapper.json").is_file() {
        relocate(&legacy, &target);
    }
    // Plugins and themes live next to Pious when it's installed.
    for (name, dir) in [("plugins", plugins_dir()), ("themes", themes_dir())] {
        let inside = target.join(name);
        if dir != inside && inside.is_dir() {
            merge_into(&inside, &dir);
            let _ = remove_empty_dirs(&inside);
        }
    }
}

/// Moves the data in `old` into `new` (a fresh install folder's `data`),
/// pointing saved paths at the files' new places. Runs only when `new`
/// doesn't already hold more saved data than `old`.
fn relocate(old: &Path, new: &Path) {
    // Statistics are history, not a choice between two copies: whatever
    // happens below, the old folder's are joined to the new folder's.
    join_stats(&old.join("stats.jsonl"), &new.join("stats.jsonl"));
    let current = new.join("bootstrapper.json");
    if current.is_file() && amount(&current) >= amount(&old.join("bootstrapper.json")) {
        return;
    }
    if std::fs::create_dir_all(new).is_err() {
        return;
    }
    if current.is_file() {
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let _ = std::fs::rename(&current, new.join(format!("bootstrapper.json.bak-{stamp}")));
    }
    let Ok(entries) = std::fs::read_dir(old) else { return };
    for entry in entries.flatten() {
        let source = entry.path();
        let target = new.join(entry.file_name());
        if target.exists() {
            if source.is_dir() && target.is_dir() {
                merge_into(&source, &target);
            }
            continue;
        }
        // Same drive: a rename, instant. Another drive: copy small things;
        // Roblox versions (gigabytes) stay where they are, and the saved
        // data keeps pointing at them.
        if std::fs::rename(&source, &target).is_err() && entry.file_name() != "Versions" && copy_all(&source, &target).is_ok() {
            if source.is_dir() {
                let _ = std::fs::remove_dir_all(&source);
            } else {
                let _ = std::fs::remove_file(&source);
            }
        }
    }
    // Saved paths into the old folder now point into the new one, where the
    // file actually arrived.
    if let Ok(text) = std::fs::read_to_string(&current) {
        if let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&text) {
            repoint(&mut value, old, new);
            if let Ok(json) = serde_json::to_vec_pretty(&value) {
                let partial = new.join("bootstrapper.json.part");
                if std::fs::write(&partial, json).is_ok() {
                    let _ = std::fs::rename(&partial, &current);
                }
            }
        }
    }
    if remove_empty_dirs(old).is_err() {
        let _ = std::fs::write(
            old.join("MOVED.txt"),
            format!("Pious now keeps its data in {}.\r\nWhat's left here couldn't be moved and is still used where Pious needs it.\r\n", new.display()),
        );
    }
}

/// Every string in `value` that is a path inside `old` whose file now
/// exists inside `new` is changed to that new path.
fn repoint(value: &mut serde_json::Value, old: &Path, new: &Path) {
    match value {
        serde_json::Value::String(text) => {
            if let Ok(rest) = Path::new(text.as_str()).strip_prefix(old) {
                let moved = new.join(rest);
                if moved.exists() {
                    *text = moved.display().to_string();
                }
            }
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(|v| repoint(v, old, new)),
        serde_json::Value::Object(map) => map.values_mut().for_each(|v| repoint(v, old, new)),
        _ => {}
    }
}

fn copy_all(from: &Path, to: &Path) -> std::io::Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)?.flatten() {
            copy_all(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(from, to).map(|_| ())
    }
}

/// Moves the folder older builds used (Pious\Library) to its new place,
/// pointing saved paths inside it (installed Roblox versions) at the new
/// folder.
///
/// A build with a broken move started over in a new, nearly empty folder,
/// so the new folder may already exist. The old folder's contents then go
/// into it, and whichever saved data holds more (games, accounts, servers)
/// is kept; the other is set aside next to it, never deleted.
fn move_old_folder(old: &Path, new: &Path) {
    if !new.exists() {
        if std::fs::rename(old, new).is_err() {
            // In use (an older Pious still open): try again next start.
            return;
        }
    } else {
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let current = new.join("bootstrapper.json");
        let previous = old.join("library.json");
        if current.is_file() && amount(&current) > amount(&previous) {
            // The new data is the real one: keep the old aside.
            let _ = std::fs::rename(&previous, old.join(format!("library.json.old-{stamp}")));
        } else if current.is_file() {
            let _ = std::fs::rename(&current, new.join(format!("bootstrapper.json.bak-{stamp}")));
        }
        merge_into(old, new);
    }
    let saved = new.join("library.json");
    if let Ok(text) = std::fs::read_to_string(&saved) {
        let escape = |p: &Path| serde_json::to_string(&p.display().to_string()).unwrap_or_default().trim_matches('"').to_owned();
        let text = text.replace(&escape(old), &escape(new));
        // Written whole before the old file goes.
        let partial = new.join("bootstrapper.json.part");
        if std::fs::write(&partial, text).is_ok() && std::fs::rename(&partial, new.join("bootstrapper.json")).is_ok() {
            let _ = std::fs::remove_file(saved);
        }
    }
    // Whatever couldn't be moved stays; an empty old folder goes.
    let _ = remove_empty_dirs(old);
}

/// How much a saved data file holds: its games, accounts and servers.
fn amount(path: &Path) -> usize {
    let Ok(text) = std::fs::read_to_string(path) else { return 0 };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { return 0 };
    ["games", "accounts", "servers"].iter().map(|k| value[k].as_array().map_or(0, Vec::len)).sum()
}

/// Moves everything in `from` into `to`, keeping what `to` already has.
fn merge_into(from: &Path, to: &Path) {
    let Ok(entries) = std::fs::read_dir(from) else { return };
    let _ = std::fs::create_dir_all(to);
    for entry in entries.flatten() {
        let source = entry.path();
        let target = to.join(entry.file_name());
        if !target.exists() {
            let _ = std::fs::rename(&source, &target);
        } else if source.is_dir() && target.is_dir() {
            merge_into(&source, &target);
        } else if entry.file_name() == "library.json" {
            let _ = std::fs::rename(&source, &target);
        } else if entry.file_name() == "stats.jsonl" {
            join_stats(&source, &target);
        }
    }
}

/// Puts the statistics in `from` into `to` (both history: the older lines
/// first, in time order), then removes `from`. Lines already in `to` aren't
/// added twice. Nothing is removed unless the joined file was written.
fn join_stats(from: &Path, to: &Path) {
    let Ok(old) = std::fs::read_to_string(from) else { return };
    let new = std::fs::read_to_string(to).unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    let mut lines: Vec<&str> = old.lines().chain(new.lines()).filter(|l| !l.trim().is_empty() && seen.insert(*l)).collect();
    // By the time each line says it happened (lines without one keep their place).
    let at = |line: &str| serde_json::from_str::<serde_json::Value>(line).ok().and_then(|v| v["at"].as_str().map(str::to_owned));
    lines.sort_by_cached_key(|l| at(l).unwrap_or_default());
    let mut text = lines.join("\n");
    text.push('\n');
    if let Some(parent) = to.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let partial = to.with_extension("jsonl.part");
    if std::fs::write(&partial, text).is_ok() && std::fs::rename(&partial, to).is_ok() {
        let _ = std::fs::remove_file(from);
    }
}

fn remove_empty_dirs(dir: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)?.flatten() {
        if entry.path().is_dir() {
            let _ = remove_empty_dirs(&entry.path());
        }
    }
    std::fs::remove_dir(dir)
}

pub fn data_file() -> PathBuf {
    let file = data_dir().join("bootstrapper.json");
    // Older builds named it library.json.
    let old = data_dir().join("library.json");
    if !file.exists() && old.exists() {
        let _ = std::fs::rename(&old, &file);
    }
    file
}

pub fn artwork_dir() -> PathBuf {
    data_dir().join("cache").join("artwork")
}

/// `%LOCALAPPDATA%\Pious\Versions`, whatever the data folder is (builds are
/// big and per-user). The `versions_dir` preference overrides it.
pub fn default_versions_dir() -> PathBuf {
    match dirs::data_local_dir() {
        Some(local) => local.join("Pious").join("Versions"),
        None => data_dir().join("Versions"),
    }
}

/// Why the saved data couldn't be loaded.
#[derive(Debug, Clone)]
pub struct LoadError {
    pub message: String,
    /// Whether starting over with nothing saved may replace the file on
    /// disk. Only true once the unreadable file has been backed up.
    pub can_overwrite: bool,
    /// What could still be read, to start from instead of nothing.
    pub recovered: Option<Box<Bootstrapper>>,
}

/// Loads the saved data. A missing file yields an empty one. A corrupt
/// file is backed up first, after which an empty one may take
/// its place. If the file can't be read at all (locked, no permission), the
/// file is left alone and must not be overwritten.
pub async fn load() -> Result<Bootstrapper, LoadError> {
    let path = data_file();
    match tokio::fs::read(&path).await {
        // Saved by a newer Pious (after a downgrade, or an old copy still
        // installed somewhere): show what reads, but never save over it, or
        // whatever this one doesn't understand would be lost.
        Ok(bytes) if newer_than_us(&bytes).is_some() => {
            let newer = newer_than_us(&bytes).unwrap_or_default();
            let data = serde_json::from_slice::<Bootstrapper>(&bytes)
                .ok()
                .or_else(|| serde_json::from_slice::<serde_json::Value>(&bytes).ok().map(|v| recover(v).0))
                .unwrap_or_default();
            Err(LoadError {
                message: format!(
                    "Your Pious data was saved by Pious {newer}, which is newer than this one ({}). Nothing will be saved here, so none of it is lost: update Pious, or use the newer copy.",
                    crate::core::updater::CURRENT
                ),
                can_overwrite: false,
                recovered: Some(Box::new(data)),
            })
        }
        Ok(bytes) => match serde_json::from_slice::<Bootstrapper>(&bytes) {
            Ok(data) => Ok(data),
            // Readable JSON with a bad part (a field from a newer or broken
            // build): keep everything that still reads, back up the file,
            // and say what was reset.
            Err(_) if serde_json::from_slice::<serde_json::Value>(&bytes).is_ok_and(|v| v.is_object()) => {
                let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
                let (data, dropped) = recover(value.clone());
                let backup = backup_path(&path);
                let backed_up = tokio::fs::copy(&path, &backup).await.is_ok();
                // Every game or every account unreadable means the file is
                // fine and this Pious is what can't read it: don't save over
                // it with an empty library.
                let lost_all = |key: &str, kept: usize| value[key].as_array().is_some_and(|list| !list.is_empty()) && kept == 0;
                if lost_all("games", data.games.len()) || lost_all("accounts", data.accounts.len()) {
                    return Err(LoadError {
                        message: format!(
                            "This Pious couldn't read your games or accounts, so it won't save anything over them. Update Pious; your data is untouched (a copy is at {}).",
                            backup.display()
                        ),
                        can_overwrite: false,
                        recovered: Some(Box::new(data)),
                    });
                }
                Err(LoadError {
                    message: if backed_up {
                        format!(
                            "Some of your Pious data couldn't be read and was reset ({}). Everything else was kept. The original is saved as {}.",
                            dropped.join(", "),
                            backup.display()
                        )
                    } else {
                        // Without a backup the original must not be replaced:
                        // use what was recovered, but don't save over it.
                        format!(
                            "Some of your Pious data couldn't be read ({}). Pious couldn't back the file up, so it won't save changes until it's restarted.",
                            dropped.join(", ")
                        )
                    },
                    can_overwrite: backed_up,
                    recovered: Some(Box::new(data)),
                })
            }
            Err(error) => {
                let backup = backup_path(&path);
                let backed_up = tokio::fs::copy(&path, &backup).await.is_ok();
                Err(LoadError {
                    message: if backed_up {
                        format!(
                            "Your Pious data could not be read ({error}). A backup was saved to {}.",
                            backup.display()
                        )
                    } else {
                        format!("Your Pious data could not be read ({error}). It was left untouched.")
                    },
                    can_overwrite: backed_up,
                    recovered: None,
                })
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Bootstrapper::default()),
        Err(error) => Err(LoadError {
            message: format!(
                "Could not open your Pious data ({error}). Changes won't be saved until Pious is restarted."
            ),
            can_overwrite: false,
            recovered: None,
        }),
    }
}

/// Where an unreadable file is copied to: a new name each time
/// (`bootstrapper.corrupt-20261009-191500.json`), so a later problem can
/// never replace the copy of an earlier one.
fn backup_path(path: &Path) -> PathBuf {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let mut backup = path.with_extension(format!("corrupt-{stamp}.json"));
    let mut n = 1;
    while backup.exists() {
        n += 1;
        backup = path.with_extension(format!("corrupt-{stamp}-{n}.json"));
    }
    backup
}

/// The version that saved `bytes`, when it's newer than this Pious.
fn newer_than_us(bytes: &[u8]) -> Option<String> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let saved = value["saved_by"].as_str()?.to_owned();
    let parse = |v: &str| -> Option<Vec<u64>> { v.split('.').map(|p| p.parse().ok()).collect() };
    let (theirs, ours) = (parse(&saved)?, parse(crate::core::updater::CURRENT)?);
    (theirs > ours).then_some(saved)
}

#[cfg(test)]
mod real_file {
    /// Reads a real saved file (`PIOUS_CHECK_FILE`) the way Pious does and
    /// says what doesn't read: `cargo test --bin pious -- --ignored reads_real_file --nocapture`.
    #[test]
    #[ignore]
    fn reads_real_file() {
        let path = std::env::var("PIOUS_CHECK_FILE").expect("PIOUS_CHECK_FILE");
        let bytes = std::fs::read(&path).unwrap();
        match serde_json::from_slice::<crate::core::model::Bootstrapper>(&bytes) {
            Ok(data) => println!("reads fine: {} games, {} accounts", data.games.len(), data.accounts.len()),
            Err(error) => {
                println!("doesn't read: {error}");
                let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                for game in value["games"].as_array().into_iter().flatten().take(1) {
                    println!("first game: {:?}", serde_json::from_value::<crate::core::model::Game>(game.clone()).err());
                }
                for account in value["accounts"].as_array().into_iter().flatten().take(1) {
                    println!("first account: {:?}", serde_json::from_value::<crate::core::model::Account>(account.clone()).err());
                }
                let (data, dropped) = super::recover(value);
                println!("recovered: {} games, {} accounts; dropped {dropped:?}", data.games.len(), data.accounts.len());
            }
        }
    }
}

/// Keeps every part of damaged saved data that still reads: each game,
/// server, account, version and activity on its own, and each preference
/// on its own. Returns what was reset.
fn recover(value: serde_json::Value) -> (Bootstrapper, Vec<String>) {
    use serde_json::Value;
    let mut dropped = Vec::new();
    let mut out = serde_json::Map::new();
    let Value::Object(input) = value else { return (Bootstrapper::default(), vec!["everything".into()]) };
    fn items<T: serde::de::DeserializeOwned>(value: &Value, name: &str, dropped: &mut Vec<String>) -> Value {
        let list = value.as_array().cloned().unwrap_or_default();
        let total = list.len();
        let kept: Vec<Value> = list.into_iter().filter(|item| serde_json::from_value::<T>(item.clone()).is_ok()).collect();
        if kept.len() < total {
            dropped.push(format!("{} of your {name}", total - kept.len()));
        }
        Value::Array(kept)
    }
    for (key, value) in input {
        let fixed = match key.as_str() {
            "games" => items::<crate::core::model::Game>(&value, "games", &mut dropped),
            "servers" => items::<crate::core::model::PrivateServer>(&value, "servers", &mut dropped),
            "accounts" => items::<crate::core::model::Account>(&value, "accounts", &mut dropped),
            "versions" => items::<crate::core::model::VersionRecord>(&value, "versions", &mut dropped),
            "activity" => items::<crate::core::model::Activity>(&value, "history entries", &mut dropped),
            "preferences" => {
                // Each setting on its own, on top of the defaults.
                let mut prefs = serde_json::to_value(crate::core::model::Preferences::default()).unwrap_or_default();
                for (name, setting) in value.as_object().cloned().unwrap_or_default() {
                    let mut trial = prefs.clone();
                    trial[&name] = setting;
                    if serde_json::from_value::<crate::core::model::Preferences>(trial.clone()).is_ok() {
                        prefs = trial;
                    } else {
                        dropped.push(format!("the {} setting", name.replace('_', " ")));
                    }
                }
                prefs
            }
            _ => value,
        };
        out.insert(key, fixed);
    }
    let data = serde_json::from_value(Value::Object(out)).unwrap_or_default();
    if dropped.is_empty() {
        dropped.push("an unreadable part".into());
    }
    (data, dropped)
}

/// Atomically writes the saved data to disk.
pub async fn save(mut data: Bootstrapper) -> Result<(), String> {
    data.saved_by = crate::core::updater::CURRENT.to_owned();
    let path = data_file();
    let json = serde_json::to_vec_pretty(&data).map_err(|e| e.to_string())?;
    write_atomic(&path, &json).await.map_err(|e| e.to_string())
}

async fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    // A unique temporary name, so overlapping writes can't interleave.
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
    tokio::fs::write(&tmp, bytes).await?;
    tokio::fs::rename(&tmp, path).await
}

/// Where Roblox links opened in a browser wait for the running app.
fn inbox_dir() -> PathBuf {
    data_dir().join("inbox")
}

/// Queues a Roblox link for the running app to launch.
pub fn post_link(link: &str) -> std::io::Result<()> {
    let dir = inbox_dir();
    std::fs::create_dir_all(&dir)?;
    let name = format!("{}-{}.link", chrono::Utc::now().timestamp_millis(), uuid::Uuid::new_v4().simple());
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, link)?;
    std::fs::rename(tmp, dir.join(name))
}

/// Takes every queued Roblox link, oldest first. Links older than a
/// minute are dropped: their launch tickets have expired by then.
pub async fn take_links() -> Vec<String> {
    tokio::task::spawn_blocking(|| {
        let Ok(entries) = std::fs::read_dir(inbox_dir()) else {
            return Vec::new();
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "link"))
            .collect();
        files.sort();
        files
            .into_iter()
            .filter_map(|path| {
                // Renaming claims the link: only one reader (this poll, the
                // start-up check, or another copy of Pious) can win it.
                let claimed = path.with_extension(format!("claimed-{}", uuid::Uuid::new_v4().simple()));
                std::fs::rename(&path, &claimed).ok()?;
                let fresh = std::fs::metadata(&claimed)
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age.as_secs() < 60);
                let link = std::fs::read_to_string(&claimed).ok();
                let _ = std::fs::remove_file(&claimed);
                link.filter(|_| fresh)
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

/// Every cached artwork file, keyed by its cache key.
pub fn cached_artwork() -> std::collections::HashMap<String, String> {
    let Ok(entries) = std::fs::read_dir(artwork_dir()) else {
        return Default::default();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .filter_map(|p| Some((p.file_stem()?.to_str()?.to_owned(), p.display().to_string())))
        .collect()
}

/// Caches a picture and returns where it was saved.
pub async fn store_artwork(key: &str, bytes: &[u8]) -> Option<PathBuf> {
    let path = artwork_dir().join(format!("{key}.png"));
    write_atomic(&path, bytes).await.ok()?;
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_folder_wins_over_an_empty_start() {
        let root = std::env::temp_dir().join(format!("pious-store-{}", uuid::Uuid::new_v4().simple()));
        let (old, new) = (root.join("Library"), root.join("Bootstrapper"));
        std::fs::create_dir_all(old.join("Versions").join("a")).unwrap();
        std::fs::create_dir_all(new.join("Versions").join("b")).unwrap();
        let path = serde_json::to_string(&old.join("Versions").join("a").display().to_string()).unwrap();
        std::fs::write(old.join("library.json"), format!("{{\"games\":[1],\"path\":{path}}}")).unwrap();
        std::fs::write(new.join("bootstrapper.json"), "{}").unwrap();

        move_old_folder(&old, &new);

        let saved = std::fs::read_to_string(new.join("bootstrapper.json")).unwrap();
        assert!(saved.contains("Bootstrapper"), "{saved}");
        let kept = std::fs::read_dir(&new).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("bootstrapper.json.bak-"));
        assert!(kept);
        assert!(new.join("Versions").join("a").is_dir() && new.join("Versions").join("b").is_dir());
        assert!(!old.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn relocates_into_the_install_folder_keeping_everything() {
        let root = std::env::temp_dir().join(format!("pious-relocate-{}", uuid::Uuid::new_v4().simple()));
        let (old, new) = (root.join("AppData").join("Bootstrapper"), root.join("Programs").join("Pious").join("data"));
        std::fs::create_dir_all(old.join("Versions").join("version-abc")).unwrap();
        std::fs::write(old.join("Versions").join("version-abc").join("RobloxPlayerBeta.exe"), b"exe").unwrap();
        std::fs::create_dir_all(old.join("plugins").join("mine")).unwrap();
        let build = old.join("Versions").join("version-abc").display().to_string();
        let elsewhere = r"D:\Somewhere\else.ttf";
        std::fs::write(
            old.join("bootstrapper.json"),
            serde_json::json!({ "games": [1, 2], "versions": [{ "path": build }], "font": elsewhere }).to_string(),
        )
        .unwrap();
        std::fs::write(old.join("stats.jsonl"), b"{}\n").unwrap();

        relocate(&old, &new);

        let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(new.join("bootstrapper.json")).unwrap()).unwrap();
        let moved = new.join("Versions").join("version-abc");
        assert_eq!(saved["versions"][0]["path"], moved.display().to_string());
        assert_eq!(saved["font"], elsewhere, "paths outside the old folder stay");
        assert!(moved.join("RobloxPlayerBeta.exe").is_file());
        assert!(new.join("stats.jsonl").is_file() && new.join("plugins").join("mine").is_dir());
        assert!(!old.exists(), "an emptied old folder goes");

        // Running again (next start) changes nothing.
        relocate(&old, &new);
        assert_eq!(amount(&new.join("bootstrapper.json")), 2);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn data_from_a_newer_pious_is_never_saved_over() {
        let ours = crate::core::updater::CURRENT;
        let newer = serde_json::json!({ "saved_by": "999.0.0", "games": [] }).to_string();
        assert_eq!(newer_than_us(newer.as_bytes()).as_deref(), Some("999.0.0"));
        for older in [format!(r#"{{ "saved_by": "{ours}" }}"#), r#"{ "saved_by": "0.1.0" }"#.into(), "{}".into(), r#"{ "saved_by": "" }"#.into()] {
            assert_eq!(newer_than_us(older.as_bytes()), None, "{older}");
        }
        // Numbers compare as numbers (1.10.0 is newer than 1.9.0).
        assert!(newer_than_us(br#"{ "saved_by": "1000.10.0" }"#).is_some());
    }

    #[test]
    fn statistics_are_joined_when_data_moves() {
        let root = std::env::temp_dir().join(format!("pious-stats-{}", uuid::Uuid::new_v4().simple()));
        let (old, new) = (root.join("old"), root.join("new"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(old.join("stats.jsonl"), "{\"at\":\"2026-01-01T00:00:00Z\",\"event\":\"launch\"}\n{\"at\":\"2026-02-01T00:00:00Z\",\"event\":\"session\",\"seconds\":60}\n").unwrap();
        std::fs::write(new.join("stats.jsonl"), "{\"at\":\"2026-03-01T00:00:00Z\",\"event\":\"app_open\"}\n").unwrap();
        // The new folder already has more data: the library stays, the
        // history is still joined.
        let library = serde_json::json!({ "games": [{}, {}] }).to_string();
        std::fs::write(new.join("bootstrapper.json"), &library).unwrap();
        std::fs::write(old.join("bootstrapper.json"), "{}").unwrap();
        relocate(&old, &new);
        let joined = std::fs::read_to_string(new.join("stats.jsonl")).unwrap();
        let lines: Vec<&str> = joined.lines().collect();
        assert_eq!(lines.len(), 3, "{joined}");
        assert!(lines[0].contains("2026-01") && lines[2].contains("2026-03"), "oldest first");
        assert!(!old.join("stats.jsonl").exists());
        // Joining again adds nothing twice.
        std::fs::write(old.join("stats.jsonl"), lines[0]).unwrap();
        join_stats(&old.join("stats.jsonl"), &new.join("stats.jsonl"));
        assert_eq!(std::fs::read_to_string(new.join("stats.jsonl")).unwrap().lines().count(), 3);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn backups_never_replace_each_other() {
        let dir = std::env::temp_dir().join(format!("pious-backup-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("bootstrapper.json");
        let first = backup_path(&file);
        std::fs::write(&first, "a").unwrap();
        let second = backup_path(&file);
        assert_ne!(first, second);
        assert!(first.file_name().unwrap().to_string_lossy().starts_with("bootstrapper.corrupt-"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn damaged_data_keeps_what_still_reads() {
        let id = uuid::Uuid::new_v4();
        let good = serde_json::json!({ "id": id, "place_id": 920587237u64, "name": "Adopt Me!", "added_at": "2025-01-01T00:00:00Z" });
        let bad = serde_json::json!({ "id": "not-a-uuid", "place_id": "x" });
        let value = serde_json::json!({
            "games": [good, bad],
            "preferences": { "multi_instance": false, "ui_scale": "huge", "streamer_mode": true },
        });
        assert!(serde_json::from_value::<Bootstrapper>(value.clone()).is_err());
        let (data, dropped) = recover(value);
        assert_eq!(data.games.len(), 1);
        assert_eq!(data.games[0].id, id);
        assert!(!data.preferences.multi_instance, "good settings are kept");
        assert!(data.preferences.streamer_mode);
        assert_eq!(data.preferences.ui_scale, 1.0, "the bad one is reset");
        assert!(dropped.iter().any(|d| d.contains("ui scale")), "{dropped:?}");
        assert!(dropped.iter().any(|d| d.contains("1 of your games")), "{dropped:?}");
    }

    #[test]
    fn relocation_never_replaces_richer_data() {
        let root = std::env::temp_dir().join(format!("pious-relocate2-{}", uuid::Uuid::new_v4().simple()));
        let (old, new) = (root.join("old"), root.join("new"));
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(&new).unwrap();
        std::fs::write(old.join("bootstrapper.json"), r#"{"games":[1]}"#).unwrap();
        std::fs::write(new.join("bootstrapper.json"), r#"{"games":[1,2,3]}"#).unwrap();
        relocate(&old, &new);
        assert_eq!(amount(&new.join("bootstrapper.json")), 3);
        assert!(old.join("bootstrapper.json").is_file(), "the old data is left alone");
        let _ = std::fs::remove_dir_all(root);
    }
}
