//! Third-party Roblox bootstrappers (Bloxstrap and its forks).
//!
//! Pious works alongside them rather than over them: it reads what each one
//! is set up to do (open Roblox links, show Discord activity, apply
//! FastFlags and mods) so it can avoid doing the same job twice, and so
//! taking over Roblox links can be undone exactly.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::platform::system;

/// Bootstrappers Pious knows how to read. They all share Bloxstrap's
/// folder layout and settings file.
const KNOWN: [&str; 5] = ["Bloxstrap", "Fishstrap", "Voidstrap", "Lunastrap", "Froststrap"];

/// What one installed bootstrapper does to the system.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct InstalledBootstrapper {
    pub name: &'static str,
    pub folder: PathBuf,
    /// Its program, which accepts `-player <roblox-player link>`.
    pub exe: PathBuf,
    /// It's the program that opens `roblox-player:` links.
    pub opens_links: bool,
    /// It shows the game you're in on Discord.
    pub discord_presence: bool,
    /// It watches the Roblox log to track servers and activity.
    pub activity_tracking: bool,
    /// It lets several Roblox windows run at once.
    pub multi_instance: bool,
    /// FastFlags it writes into the client.
    pub fast_flags: usize,
    /// Files it copies over the client (fonts, sounds, cursors…).
    pub mods: usize,
    /// The build it last installed, when it says so.
    pub version: Option<String>,
    /// Windows has an uninstaller for it.
    pub uninstallable: bool,
}


/// Finds every installed bootstrapper.
pub fn detect() -> Vec<InstalledBootstrapper> {
    let Some(local) = dirs::data_local_dir() else {
        return Vec::new();
    };
    let handler = system::link_handler("roblox-player").unwrap_or_default().to_ascii_lowercase();
    let handler_version = system::read_user_value("Software\\Classes\\roblox-player\\shell\\open\\command", "version");

    KNOWN
        .into_iter()
        .filter_map(|name| {
            let folder = local.join(name);
            let exe = folder.join(format!("{name}.exe"));
            if !exe.is_file() {
                return None;
            }
            let settings = read_json(&folder.join("Settings.json")).unwrap_or(Value::Null);
            let flag = |key: &str| settings[key].as_bool().unwrap_or(false);
            let opens_links = handler.contains(&format!("\\{}\\", name.to_ascii_lowercase()));
            Some(InstalledBootstrapper {
                name,
                opens_links,
                discord_presence: flag("UseDiscordRichPresence"),
                activity_tracking: flag("EnableActivityTracking"),
                multi_instance: flag("MultiInstanceLaunching"),
                fast_flags: read_json(&folder.join("Modifications").join("ClientSettings").join("ClientAppSettings.json"))
                    .and_then(|v| v.as_object().map(|o| o.len()))
                    .unwrap_or(0),
                mods: count_files(&folder.join("Modifications"), &["ClientSettings"]),
                version: opens_links.then(|| handler_version.clone()).flatten(),
                uninstallable: system::uninstall_command(name).is_some(),
                exe,
                folder,
            })
        })
        .collect()
}

/// The bootstrapper that opens Roblox links, if one does.
pub fn link_owner(found: &[InstalledBootstrapper]) -> Option<&InstalledBootstrapper> {
    found.iter().find(|b| b.opens_links)
}

/// The FastFlags each installed bootstrapper has saved: (name, flags).
pub fn saved_flags() -> Vec<(&'static str, serde_json::Map<String, Value>)> {
    let Some(local) = dirs::data_local_dir() else {
        return Vec::new();
    };
    KNOWN
        .into_iter()
        .filter_map(|name| {
            let path = local.join(name).join("Modifications").join("ClientSettings").join("ClientAppSettings.json");
            match read_json(&path)? {
                Value::Object(flags) if !flags.is_empty() => Some((name, flags)),
                _ => None,
            }
        })
        .collect()
}

fn read_json(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    // Bloxstrap writes its files with a byte-order mark.
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
    serde_json::from_slice(bytes).ok()
}

fn count_files(dir: &Path, skip: &[&str]) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                let name = entry.file_name();
                if skip.iter().any(|s| name.eq_ignore_ascii_case(s)) {
                    0
                } else {
                    count_files(&path, &[])
                }
            } else {
                1
            }
        })
        .sum()
}
