//! Finds Roblox sign-ins saved in the user's web browsers, so an account
//! can be added without signing in again.
//!
//! Chromium browsers (Edge, Chrome, Brave, Opera, Vivaldi) keep cookies in
//! an SQLite file, encrypted with a key Windows protects for the user
//! (DPAPI). Firefox keeps them unencrypted. Chrome's newer "app-bound"
//! encryption only lets Chrome itself read its cookies, so those are
//! reported as unreadable rather than worked around.

use std::path::{Path, PathBuf};

/// A Roblox session found in a browser.
#[derive(Debug, Clone)]
pub struct Found {
    pub browser: String,
    pub token: String,
}

/// What looking through the browsers turned up.
#[derive(Debug, Clone, Default)]
pub struct Search {
    pub found: Vec<Found>,
    /// Browsers that have a sign-in Pious can't read, and why.
    pub problems: Vec<String>,
}

const COOKIE: &str = ".ROBLOSECURITY";

/// Chromium browsers: (name, user data folder).
fn chromium_browsers() -> Vec<(&'static str, PathBuf)> {
    let local = dirs::data_local_dir().unwrap_or_default();
    let roaming = dirs::data_dir().unwrap_or_default();
    vec![
        ("Microsoft Edge", local.join("Microsoft/Edge/User Data")),
        ("Google Chrome", local.join("Google/Chrome/User Data")),
        ("Brave", local.join("BraveSoftware/Brave-Browser/User Data")),
        ("Vivaldi", local.join("Vivaldi/User Data")),
        ("Chromium", local.join("Chromium/User Data")),
        ("Opera", roaming.join("Opera Software/Opera Stable")),
        ("Opera GX", roaming.join("Opera Software/Opera GX Stable")),
    ]
}

/// Looks through every browser on this PC (blocking).
pub fn search() -> Search {
    let mut search = Search::default();
    for (name, root) in chromium_browsers() {
        if root.is_dir() {
            chromium(name, &root, &mut search);
        }
    }
    firefox(&mut search);
    // The same account signed in in two browsers.
    let mut seen = std::collections::HashSet::new();
    search.found.retain(|f| seen.insert(f.token.clone()));
    search
}

/// Copies a database aside (browsers keep theirs open) and opens the copy.
fn open_copy(file: &Path, browser: &str) -> Result<(rusqlite::Connection, PathBuf), String> {
    let copy = std::env::temp_dir().join(format!("pious-cookies-{}.db", uuid::Uuid::new_v4().simple()));
    if let Err(error) = std::fs::copy(file, &copy) {
        return Err(if error.raw_os_error() == Some(32) {
            format!("{browser} is open and keeps its cookies locked. Close it and try again.")
        } else {
            format!("Couldn't read {browser}'s cookies ({error}).")
        });
    }
    // Recent changes may still be in the write-ahead log.
    let wal = PathBuf::from(format!("{}-wal", file.display()));
    if wal.exists() {
        let _ = std::fs::copy(&wal, PathBuf::from(format!("{}-wal", copy.display())));
    }
    rusqlite::Connection::open_with_flags(&copy, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map(|c| (c, copy.clone()))
        .map_err(|e| format!("Couldn't read {browser}'s cookies ({e})."))
}

fn remove_copy(copy: &Path) {
    let _ = std::fs::remove_file(copy);
    let _ = std::fs::remove_file(format!("{}-wal", copy.display()));
    let _ = std::fs::remove_file(format!("{}-shm", copy.display()));
}

fn chromium(browser: &str, root: &Path, search: &mut Search) {
    let key = match master_key(root) {
        Some(key) => key,
        None => return,
    };
    // Opera keeps its one profile in the root itself.
    let mut profiles = vec![root.to_path_buf()];
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "Default" || name.starts_with("Profile ") {
                profiles.push(entry.path());
            }
        }
    }
    for profile in profiles {
        let file = [profile.join("Network").join("Cookies"), profile.join("Cookies")].into_iter().find(|f| f.is_file());
        let Some(file) = file else { continue };
        let (db, copy) = match open_copy(&file, browser) {
            Ok(opened) => opened,
            Err(problem) => {
                if !search.problems.contains(&problem) {
                    search.problems.push(problem);
                }
                continue;
            }
        };
        let rows: Vec<Vec<u8>> = db
            .prepare("SELECT encrypted_value FROM cookies WHERE name = ?1 AND host_key LIKE '%roblox.com'")
            .and_then(|mut statement| {
                statement.query_map([COOKIE], |row| row.get::<_, Vec<u8>>(0)).map(|rows| rows.flatten().collect())
            })
            .unwrap_or_default();
        drop(db);
        remove_copy(&copy);
        for value in rows {
            match decrypt(&value, &key) {
                Ok(token) => search.found.push(Found { browser: browser.to_owned(), token }),
                Err(problem) => {
                    let problem = format!("{browser}: {problem}");
                    if !search.problems.contains(&problem) {
                        search.problems.push(problem);
                    }
                }
            }
        }
    }
}

/// The browser's cookie key, unwrapped by Windows for this user.
fn master_key(root: &Path) -> Option<Vec<u8>> {
    use base64::Engine as _;
    let text = std::fs::read_to_string(root.join("Local State")).ok()?;
    let state: serde_json::Value = serde_json::from_str(&text).ok()?;
    let wrapped = base64::engine::general_purpose::STANDARD.decode(state["os_crypt"]["encrypted_key"].as_str()?).ok()?;
    unprotect(wrapped.strip_prefix(b"DPAPI")?)
}

fn decrypt(value: &[u8], key: &[u8]) -> Result<String, String> {
    use aes_gcm::aead::{Aead, KeyInit};
    let token = if value.starts_with(b"v10") || value.starts_with(b"v11") {
        if value.len() < 3 + 12 + 16 {
            return Err("an unreadable cookie".into());
        }
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(key).map_err(|_| "an unreadable cookie")?;
        let nonce = aes_gcm::Nonce::from_slice(&value[3..15]);
        cipher.decrypt(nonce, &value[15..]).map_err(|_| "an unreadable cookie".to_owned())?
    } else if value.starts_with(b"v20") {
        return Err("its sign-ins are locked to the browser itself. Use another way to sign in.".into());
    } else {
        // Very old versions: the whole value is protected by Windows.
        unprotect(value).ok_or("an unreadable cookie")?
    };
    // Newer versions put a hash of the site in front of the value.
    let text = String::from_utf8_lossy(&token);
    let start = text.find("_|WARNING").unwrap_or(0);
    let token = text[start..].trim().to_owned();
    if token.len() < 100 {
        return Err("an unreadable cookie".into());
    }
    Ok(token)
}

#[cfg(windows)]
fn unprotect(data: &[u8]) -> Option<Vec<u8>> {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{CRYPT_INTEGER_BLOB, CryptUnprotectData};
    unsafe {
        let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
        let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
        let ok = CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            &mut output,
        );
        if ok == 0 || output.pbData.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        LocalFree(output.pbData as _);
        Some(bytes)
    }
}

#[cfg(not(windows))]
fn unprotect(_data: &[u8]) -> Option<Vec<u8>> {
    None
}

fn firefox(search: &mut Search) {
    let Some(root) = dirs::data_dir().map(|d| d.join("Mozilla/Firefox/Profiles")) else { return };
    let Ok(entries) = std::fs::read_dir(root) else { return };
    for entry in entries.flatten() {
        let file = entry.path().join("cookies.sqlite");
        if !file.is_file() {
            continue;
        }
        let (db, copy) = match open_copy(&file, "Firefox") {
            Ok(opened) => opened,
            Err(problem) => {
                if !search.problems.contains(&problem) {
                    search.problems.push(problem);
                }
                continue;
            }
        };
        let rows: Vec<String> = db
            .prepare("SELECT value FROM moz_cookies WHERE name = ?1 AND host LIKE '%roblox.com'")
            .and_then(|mut statement| {
                statement.query_map([COOKIE], |row| row.get::<_, String>(0)).map(|rows| rows.flatten().collect())
            })
            .unwrap_or_default();
        drop(db);
        remove_copy(&copy);
        for token in rows {
            if token.len() >= 100 {
                search.found.push(Found { browser: "Firefox".into(), token });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_new_style_values() {
        use aes_gcm::aead::{Aead, KeyInit};
        let key = [7u8; 32];
        let cipher = aes_gcm::Aes256Gcm::new_from_slice(&key).unwrap();
        let nonce = [1u8; 12];
        let mut plain = vec![0xAB; 32];
        let token = format!("_|WARNING:-DO-NOT-SHARE-THIS.--{}", "X".repeat(120));
        plain.extend_from_slice(token.as_bytes());
        let mut value = b"v10".to_vec();
        value.extend_from_slice(&nonce);
        value.extend(cipher.encrypt(aes_gcm::Nonce::from_slice(&nonce), plain.as_slice()).unwrap());
        assert_eq!(decrypt(&value, &key).unwrap(), token);
        assert!(decrypt(b"v20whatever-long-enough-to-matter", &key).is_err());
    }
}
