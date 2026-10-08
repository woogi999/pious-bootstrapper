//! Clearing old Roblox logs and caches, like bootstrappers' cleaners.

use std::time::{Duration, SystemTime};

/// Deletes files older than `days` from Roblox's log and cache folders.
/// Returns how many files were removed. Files in use are skipped.
pub fn clean(days: u32) -> usize {
    let Some(local) = dirs::data_local_dir() else { return 0 };
    let roblox = local.join("Roblox");
    let temp = std::env::temp_dir().join("Roblox");
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(days as u64 * 24 * 3600))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    [roblox.join("logs"), roblox.join("rbx-storage"), roblox.join("http"), temp]
        .iter()
        .map(|dir| clean_dir(dir, cutoff))
        .sum()
}

fn clean_dir(dir: &std::path::Path, cutoff: SystemTime) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            removed += clean_dir(&path, cutoff);
            let _ = std::fs::remove_dir(&path); // only if now empty
        } else {
            let old = entry.metadata().and_then(|m| m.modified()).is_ok_and(|t| t < cutoff);
            if old && std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}
