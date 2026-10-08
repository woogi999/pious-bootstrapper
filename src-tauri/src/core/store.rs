//! Disk persistence for Pious's saved data and the artwork cache.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::core::model::Bootstrapper;

/// Root directory for all Pious data. `PIOUS_DATA` (or the older
/// `PIOUS_LIBRARY_DATA`) overrides it, which is useful for portable installs
/// and testing.
pub fn data_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        if let Some(dir) = std::env::var_os("PIOUS_DATA").or_else(|| std::env::var_os("PIOUS_LIBRARY_DATA")) {
            return PathBuf::from(dir);
        }
        let root = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from(".")).join("Pious");
        let dir = root.join("Bootstrapper");
        // Older builds kept everything in Pious\Library.
        let old = root.join("Library");
        if old.join("library.json").is_file() {
            move_old_folder(&old, &dir);
        }
        dir
    })
    .clone()
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
            // Both hold history: the old first.
            if let (Ok(old), Ok(new)) = (std::fs::read(&source), std::fs::read(&target)) {
                if std::fs::write(&target, [old, new].concat()).is_ok() {
                    let _ = std::fs::remove_file(&source);
                }
            }
        }
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

pub fn default_versions_dir() -> PathBuf {
    data_dir().join("Versions")
}

/// Why the saved data couldn't be loaded.
#[derive(Debug, Clone)]
pub struct LoadError {
    pub message: String,
    /// Whether starting over with nothing saved may replace the file on
    /// disk. Only true once the unreadable file has been backed up.
    pub can_overwrite: bool,
}

/// Loads the saved data. A missing file yields an empty one. A corrupt
/// file is backed up first, after which an empty one may take
/// its place. If the file can't be read at all (locked, no permission), the
/// file is left alone and must not be overwritten.
pub async fn load() -> Result<Bootstrapper, LoadError> {
    let path = data_file();
    match tokio::fs::read(&path).await {
        Ok(bytes) => match serde_json::from_slice::<Bootstrapper>(&bytes) {
            Ok(data) => Ok(data),
            Err(error) => {
                let backup = path.with_extension("corrupt.json");
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
                })
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Bootstrapper::default()),
        Err(error) => Err(LoadError {
            message: format!(
                "Could not open your Pious data ({error}). Changes won't be saved until Pious is restarted."
            ),
            can_overwrite: false,
        }),
    }
}

/// Atomically writes the saved data to disk.
pub async fn save(data: Bootstrapper) -> Result<(), String> {
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
}
