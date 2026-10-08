//! Remembers whether files exist, so code holding the app's state lock
//! never waits on the disk (a sleeping hard drive or an antivirus scan can
//! make a single check take seconds, and the window waits on that lock).
//! A background loop keeps the answers fresh.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// After this long an answer is checked again on the next use.
const FRESH: Duration = Duration::from_secs(15);

static CACHE: Mutex<Option<HashMap<PathBuf, (Instant, bool)>>> = Mutex::new(None);

fn cached(path: &Path) -> Option<bool> {
    let cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.as_ref()?.get(path).filter(|(at, _)| at.elapsed() < FRESH).map(|(_, v)| *v)
}

fn store(path: PathBuf, value: bool) {
    let mut cache = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    cache.get_or_insert_with(HashMap::new).insert(path, (Instant::now(), value));
}

/// Whether `path` is a file, from the cache when it's fresh. The disk is
/// only touched (outside any lock) when the answer isn't known yet.
pub fn is_file(path: &Path) -> bool {
    if let Some(value) = cached(path) {
        return value;
    }
    let value = path.is_file();
    store(path.to_path_buf(), value);
    value
}

/// Checks these paths now (call from a background thread).
pub fn refresh(paths: impl IntoIterator<Item = PathBuf>) {
    for path in paths {
        let value = path.is_file();
        store(path, value);
    }
}

/// Forgets everything, after files were added or removed on purpose.
pub fn clear() {
    *CACHE.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_refreshes() {
        let path = std::env::temp_dir().join(format!("pious-fscache-{}", uuid::Uuid::new_v4().simple()));
        assert!(!is_file(&path));
        std::fs::write(&path, b"x").unwrap();
        // Still the remembered answer until refreshed.
        assert!(!is_file(&path));
        refresh([path.clone()]);
        assert!(is_file(&path));
        let _ = std::fs::remove_file(&path);
    }
}
