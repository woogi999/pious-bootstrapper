//! Roblox's local settings (volume, sensitivity, graphics, frame rate…),
//! kept separately for each account that wants its own.
//!
//! Roblox keeps these in one file per PC (`GlobalBasicSettings_13.xml`),
//! reads it when a client starts and writes it when a client closes. Pious
//! keeps a copy per account ("slots") plus the PC's own ("default"), puts
//! the right one in place before a launch, and saves the file back into
//! the right slot when a window closes.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The settings Pious lets you edit, with their XML types.
pub const FIELDS: [(&str, Kind); 11] = [
    ("MasterVolume", Kind::Float),
    ("MouseSensitivity", Kind::Float),
    ("SavedQualityLevel", Kind::Token),
    ("FramerateCap", Kind::Int),
    ("Fullscreen", Kind::Bool),
    ("CameraYInverted", Kind::Bool),
    ("PerformanceStatsVisible", Kind::Bool),
    ("ReducedMotion", Kind::Bool),
    ("ChatVisible", Kind::Bool),
    ("VoiceChatVolume", Kind::Float),
    ("PlayerListVisible", Kind::Bool),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Float,
    Int,
    Token,
    Bool,
}

impl Kind {
    fn tag(self) -> &'static str {
        match self {
            Kind::Float => "float",
            Kind::Int => "int",
            Kind::Token => "token",
            Kind::Bool => "bool",
        }
    }
}

/// Roblox's settings file for this PC (the newest format version).
pub fn live_file() -> Option<PathBuf> {
    let dir = dirs::data_local_dir()?.join("Roblox");
    let mut files: Vec<(u32, PathBuf)> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let version = name.strip_prefix("GlobalBasicSettings_")?.strip_suffix(".xml")?.parse().ok()?;
            Some((version, e.path()))
        })
        .collect();
    files.sort();
    files.pop().map(|(_, p)| p)
}

pub struct Profiles {
    dir: PathBuf,
}

impl Profiles {
    pub fn new(data_dir: &Path) -> Self {
        Self { dir: data_dir.join("RobloxSettings") }
    }

    fn slot(&self, account: Option<Uuid>) -> PathBuf {
        match account {
            Some(id) => self.dir.join(format!("{id}.xml")),
            None => self.dir.join("default.xml"),
        }
    }

    fn current_file(&self) -> PathBuf {
        self.dir.join("current.txt")
    }

    /// Whose settings are in Roblox's file right now (`None` = the PC's own).
    pub fn current(&self) -> Option<Uuid> {
        std::fs::read_to_string(self.current_file()).ok()?.trim().parse().ok()
    }

    fn set_current(&self, account: Option<Uuid>) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let text = account.map(|id| id.to_string()).unwrap_or_default();
        std::fs::write(self.current_file(), text).map_err(|e| e.to_string())
    }

    /// Puts `account`'s settings (or the PC's own, for `None`) into
    /// Roblox's file, saving what's there now into its owner's slot first.
    /// An account without saved settings starts from the PC's own.
    pub fn activate(&self, account: Option<Uuid>) -> Result<(), String> {
        let Some(live) = live_file() else { return Ok(()) };
        let current = self.current();
        if current == account && self.slot(account).is_file() {
            return Ok(());
        }
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        // Keep what's in the file now.
        std::fs::copy(&live, self.slot(current)).map_err(|e| format!("Couldn't save Roblox's settings ({e})."))?;
        let source = if self.slot(account).is_file() { self.slot(account) } else { self.slot(None) };
        if source != live {
            std::fs::copy(&source, &live).map_err(|e| format!("Couldn't load the account's Roblox settings ({e})."))?;
        }
        self.set_current(account)
    }

    /// A window of `account` closed and Roblox just wrote its settings:
    /// keep them in that account's slot (or the PC's), then put back
    /// whichever settings are supposed to be current.
    pub fn capture(&self, account: Option<Uuid>) -> Result<(), String> {
        let Some(live) = live_file() else { return Ok(()) };
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        std::fs::copy(&live, self.slot(account)).map_err(|e| e.to_string())?;
        let current = self.current();
        if current != account && self.slot(current).is_file() {
            std::fs::copy(self.slot(current), &live).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// The editable values of `account`'s settings (or the PC's own).
    /// Copies the settings Roblox is using right now into a profile.
    pub fn import_live(&self, account: Option<Uuid>) -> Result<(), String> {
        let live = live_file().ok_or("Roblox hasn't saved any settings on this PC yet. Play once, then try again.")?;
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        std::fs::copy(&live, self.slot(account)).map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn values(&self, account: Option<Uuid>) -> Vec<(String, Kind, String)> {
        let path = if self.slot(account).is_file() {
            self.slot(account)
        } else if self.slot(None).is_file() {
            self.slot(None)
        } else {
            match live_file() {
                Some(path) => path,
                None => return Vec::new(),
            }
        };
        let xml = std::fs::read_to_string(path).unwrap_or_default();
        FIELDS
            .iter()
            .filter_map(|(name, kind)| Some((name.to_string(), *kind, read(&xml, *kind, name)?)))
            .collect()
    }

    /// Changes values in `account`'s settings (also in Roblox's file when
    /// that account's settings are the ones in place).
    pub fn set_values(&self, account: Option<Uuid>, values: &[(String, String)]) -> Result<(), String> {
        let Some(live) = live_file() else {
            return Err("Roblox hasn't saved any settings on this PC yet. Play once, then try again.".into());
        };
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let slot = self.slot(account);
        if !slot.is_file() {
            let base = if self.slot(None).is_file() { self.slot(None) } else { live.clone() };
            std::fs::copy(base, &slot).map_err(|e| e.to_string())?;
        }
        let mut xml = std::fs::read_to_string(&slot).map_err(|e| e.to_string())?;
        for (name, value) in values {
            let Some((_, kind)) = FIELDS.iter().find(|(n, _)| n == name) else { continue };
            xml = write(&xml, *kind, name, value);
        }
        std::fs::write(&slot, &xml).map_err(|e| e.to_string())?;
        if self.current() == account {
            std::fs::write(&live, &xml).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

fn read(xml: &str, kind: Kind, name: &str) -> Option<String> {
    let open = format!("<{} name=\"{name}\">", kind.tag());
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find('<')? + start;
    Some(xml[start..end].to_owned())
}

/// One value as it is in Roblox's settings file right now.
pub fn live_value(name: &str, kind: Kind) -> Option<String> {
    let xml = std::fs::read_to_string(live_file()?).ok()?;
    read(&xml, kind, name)
}

/// Sets values straight in Roblox's settings file (the tweaks that live
/// there, like the frame rate cap), clearing read-only for the moment if
/// it's set. Roblox reads the file when a game starts.
pub fn apply_live(values: &[(&str, Kind, String)]) -> Result<(), String> {
    let Some(live) = live_file() else { return Ok(()) };
    let xml = std::fs::read_to_string(&live).map_err(|e| e.to_string())?;
    let mut next = xml.clone();
    for (name, kind, value) in values {
        next = write(&next, *kind, name, value);
    }
    if next == xml {
        return Ok(());
    }
    let mut permissions = std::fs::metadata(&live).map_err(|e| e.to_string())?.permissions();
    let read_only = permissions.readonly();
    if read_only {
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        let _ = std::fs::set_permissions(&live, permissions.clone());
    }
    let written = std::fs::write(&live, &next).map_err(|e| format!("Couldn't change Roblox's settings ({e})."));
    if read_only {
        permissions.set_readonly(true);
        let _ = std::fs::set_permissions(&live, permissions);
    }
    written
}

/// Takes values out of Roblox's settings file again (ones that weren't
/// there before a tweak added them), so Roblox goes back to its own.
pub fn remove_live(values: &[(&str, Kind)]) -> Result<(), String> {
    let Some(live) = live_file() else { return Ok(()) };
    let xml = std::fs::read_to_string(&live).map_err(|e| e.to_string())?;
    let next = values.iter().fold(xml.clone(), |xml, (name, kind)| remove(&xml, *kind, name));
    if next == xml {
        return Ok(());
    }
    std::fs::write(&live, next).map_err(|e| format!("Couldn't change Roblox's settings ({e})."))
}

fn remove(xml: &str, kind: Kind, name: &str) -> String {
    let open = format!("<{} name=\"{name}\">", kind.tag());
    let close = format!("</{}>", kind.tag());
    let Some(start) = xml.find(&open) else { return xml.to_owned() };
    let Some(end) = xml[start..].find(&close).map(|i| start + i + close.len()) else { return xml.to_owned() };
    // The line it was on goes too.
    let line_start = xml[..start].rfind('\n').filter(|&i| xml[i + 1..start].trim().is_empty()).unwrap_or(start);
    format!("{}{}", &xml[..line_start], &xml[end..])
}

fn write(xml: &str, kind: Kind, name: &str, value: &str) -> String {
    let open = format!("<{} name=\"{name}\">", kind.tag());
    let Some(start) = xml.find(&open).map(|i| i + open.len()) else {
        // Not saved yet (a new Roblox install): add it to the settings.
        return match xml.find("</Properties>") {
            Some(at) => format!("{}{open}{value}</{}>
		{}", &xml[..at], kind.tag(), &xml[at..]),
            None => xml.to_owned(),
        };
    };
    let Some(end) = xml[start..].find('<').map(|i| i + start) else {
        return xml.to_owned();
    };
    // Keep values in the forms Roblox writes.
    let value = match kind {
        Kind::Bool => if value == "true" { "true" } else { "false" }.to_owned(),
        Kind::Int | Kind::Token => value.parse::<i64>().map(|v| v.to_string()).unwrap_or_else(|_| xml[start..end].to_owned()),
        Kind::Float => value.parse::<f64>().map(|v| v.to_string()).unwrap_or_else(|_| xml[start..end].to_owned()),
    };
    format!("{}{}{}", &xml[..start], value, &xml[end..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_missing_values() {
        let xml = "<Item class=\"UserGameSettings\"><Properties>
		<bool name=\"Fullscreen\">true</bool>
	</Properties></Item>";
        let xml = write(xml, Kind::Int, "FramerateCap", "240");
        assert_eq!(read(&xml, Kind::Int, "FramerateCap").as_deref(), Some("240"));
        assert_eq!(read(&xml, Kind::Bool, "Fullscreen").as_deref(), Some("true"));
    }

    #[test]
    fn removes_added_values() {
        let xml = "<Properties>\n\t\t<bool name=\"Fullscreen\">true</bool>\n\t</Properties>";
        let added = write(xml, Kind::Int, "FramerateCap", "240");
        assert!(read(&added, Kind::Int, "FramerateCap").is_some());
        let back = remove(&added, Kind::Int, "FramerateCap");
        assert!(read(&back, Kind::Int, "FramerateCap").is_none());
        assert_eq!(read(&back, Kind::Bool, "Fullscreen").as_deref(), Some("true"));
        assert_eq!(remove(xml, Kind::Int, "Missing"), xml);
    }

    #[test]
    fn reads_and_writes_values() {
        let xml = r#"<Properties><float name="MasterVolume">0.5</float><bool name="Fullscreen">true</bool><int name="FramerateCap">165</int></Properties>"#;
        assert_eq!(read(xml, Kind::Float, "MasterVolume").as_deref(), Some("0.5"));
        let xml = write(xml, Kind::Float, "MasterVolume", "0.25");
        let xml = write(&xml, Kind::Bool, "Fullscreen", "false");
        let xml = write(&xml, Kind::Int, "FramerateCap", "oops");
        assert_eq!(read(&xml, Kind::Float, "MasterVolume").as_deref(), Some("0.25"));
        assert_eq!(read(&xml, Kind::Bool, "Fullscreen").as_deref(), Some("false"));
        assert_eq!(read(&xml, Kind::Int, "FramerateCap").as_deref(), Some("165"));
    }
}
