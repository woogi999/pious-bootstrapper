//! Which graphics card Windows runs Roblox on: the per-app choice in
//! Settings → System → Display → Graphics. Windows keeps it in
//! `HKCU\Software\Microsoft\DirectX\UserGpuPreferences`, one value per
//! program path (so per Roblox build), as `key=value;` pairs, e.g.
//! `GpuPreference=2;` or, for one particular card,
//! `SpecificAdapter=1002&15D8&15D81002;GpuPreference=1073741824;`.
//! Only the two GPU fields are changed; others Windows keeps there stay.
//! What a value was before Pious changed it is recorded, so turning the
//! tweak off puts it back.

use std::collections::BTreeMap;
use std::path::Path;

use crate::core::compat::registry;

const KEY: &str = "Software\\Microsoft\\DirectX\\UserGpuPreferences";
/// What Windows writes for "Specific GPU".
const SPECIFIC: &str = "1073741824";

#[derive(Debug, Clone, PartialEq)]
pub struct Adapter {
    pub name: String,
    /// `VEN&DEV&SUBSYS`, in hex, as Windows writes it.
    pub id: String,
}

/// Real graphics cards, in Windows' order (no software renderer).
#[cfg(windows)]
pub fn adapters() -> Vec<Adapter> {
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, IDXGIFactory1};
    let mut out: Vec<Adapter> = Vec::new();
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else { return out };
        for i in 0..16u32 {
            let Ok(adapter) = factory.EnumAdapters1(i) else { break };
            let Ok(desc) = adapter.GetDesc1() else { continue };
            // The "Microsoft Basic Render Driver" isn't a card.
            if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 || desc.VendorId == 0x1414 {
                continue;
            }
            let name = String::from_utf16_lossy(&desc.Description[..desc.Description.iter().position(|&c| c == 0).unwrap_or(0)]);
            let id = format!("{:04X}&{:04X}&{:08X}", desc.VendorId, desc.DeviceId, desc.SubSysId);
            // A card with two outputs can show up twice.
            if !out.iter().any(|a| a.id == id) {
                out.push(Adapter { name, id });
            }
        }
    }
    out
}

#[cfg(not(windows))]
pub fn adapters() -> Vec<Adapter> {
    Vec::new()
}

/// The pairs Windows wrote, in order.
fn parse(value: &str) -> Vec<(String, String)> {
    value
        .split(';')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            let k = k.trim();
            (!k.is_empty()).then(|| (k.to_owned(), v.trim().to_owned()))
        })
        .collect()
}

fn join(pairs: &[(String, String)]) -> String {
    pairs.iter().map(|(k, v)| format!("{k}={v};")).collect()
}

/// `existing` with its GPU choice set to `choice` (see `Tweaks::gpu`).
/// `None` when `choice` isn't one Pious knows (or names a missing card).
pub fn merged(existing: Option<&str>, choice: &str, adapters: &[Adapter]) -> Option<String> {
    let mut pairs: Vec<(String, String)> =
        parse(existing.unwrap_or("")).into_iter().filter(|(k, _)| k != "GpuPreference" && k != "SpecificAdapter").collect();
    match choice {
        "default" => pairs.push(("GpuPreference".into(), "0".into())),
        "power_saving" => pairs.push(("GpuPreference".into(), "1".into())),
        "high_performance" => pairs.push(("GpuPreference".into(), "2".into())),
        other => {
            let n: usize = other.strip_prefix("adapter:")?.parse().ok()?;
            let card = adapters.get(n)?;
            // Windows' own order: the card, then the preference.
            pairs.push(("SpecificAdapter".into(), card.id.clone()));
            pairs.push(("GpuPreference".into(), SPECIFIC.into()));
        }
    }
    Some(join(&pairs))
}

fn record_path() -> std::path::PathBuf {
    crate::core::store::data_dir().join("gpu-preferences.json")
}

/// Program path → the value Windows had before Pious (`None`: there was none).
type Record = BTreeMap<String, Option<String>>;

fn load() -> Record {
    std::fs::read(record_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(record: &Record) {
    if record.is_empty() {
        let _ = std::fs::remove_file(record_path());
    } else if let Ok(json) = serde_json::to_vec_pretty(record) {
        let _ = std::fs::write(record_path(), json);
    }
}

/// Sets (or, with `None`, gives back) the graphics card for one Roblox build.
pub fn apply(build: &Path, choice: Option<&str>) {
    let exe = build.join(crate::core::compat::PLAYER_EXE);
    if !exe.is_file() {
        return;
    }
    let name = exe.display().to_string();
    let mut record = load();
    let current = registry::get(KEY, &name);
    match choice {
        Some(choice) => {
            let Some(value) = merged(current.as_deref(), choice, &adapters()) else { return };
            if current.as_deref() != Some(value.as_str()) {
                record.entry(name.clone()).or_insert(current);
                registry::set(KEY, &name, &value);
            }
        }
        None => {
            if let Some(original) = record.remove(&name) {
                match original {
                    Some(value) => registry::set(KEY, &name, &value),
                    None => registry::delete(KEY, &name),
                }
            }
        }
    }
    save(&record);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cards() -> Vec<Adapter> {
        vec![
            Adapter { name: "AMD Radeon".into(), id: "1002&15D8&15D81002".into() },
            Adapter { name: "NVIDIA".into(), id: "10DE&2684&89321043".into() },
        ]
    }

    #[test]
    fn keeps_what_windows_wrote() {
        assert_eq!(merged(Some("AppStatus=4;"), "high_performance", &cards()).unwrap(), "AppStatus=4;GpuPreference=2;");
        assert_eq!(merged(Some("AutoHDREnable=1;GpuPreference=2;"), "power_saving", &cards()).unwrap(), "AutoHDREnable=1;GpuPreference=1;");
        assert_eq!(merged(None, "default", &cards()).unwrap(), "GpuPreference=0;");
    }

    #[test]
    fn names_a_card_like_windows() {
        assert_eq!(
            merged(Some("SpecificAdapter=1002&15D8&15D81002;GpuPreference=1073741824;"), "adapter:1", &cards()).unwrap(),
            "SpecificAdapter=10DE&2684&89321043;GpuPreference=1073741824;"
        );
        assert_eq!(merged(Some("GpuPreference=1073741824;SpecificAdapter=x;"), "high_performance", &cards()).unwrap(), "GpuPreference=2;");
    }

    #[test]
    fn unknown_choices_change_nothing() {
        assert_eq!(merged(None, "adapter:5", &cards()), None);
        assert_eq!(merged(None, "turbo", &cards()), None);
    }
}
