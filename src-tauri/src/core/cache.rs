//! A small disk cache for things fetched from the internet (friends,
//! recommendations, build lists, chats), so pages that were visited before
//! show their last contents right away while fresh ones load.

use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;

fn path(key: &str) -> PathBuf {
    let safe: String = key.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    crate::core::store::data_dir().join("cache").join("data").join(format!("{safe}.json"))
}

/// The cached value for `key`, if there is one and it still reads.
pub fn load<T: DeserializeOwned>(key: &str) -> Option<T> {
    let path = path(key);
    let bytes = std::fs::read(&path).ok()?;
    match serde_json::from_slice(&bytes) {
        Ok(value) => Some(value),
        Err(_) => {
            // Damaged (or from another version): it's only a cache, so it
            // goes, and is made again from fresh data.
            let _ = std::fs::remove_file(path);
            None
        }
    }
}

/// Caches `value` under `key` (in the background).
pub fn save<T: Serialize>(key: &str, value: &T) {
    let Ok(bytes) = serde_json::to_vec(value) else { return };
    let path = path(key);
    std::thread::spawn(move || {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4().simple()));
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    });
}
