//! Windows compatibility settings for Roblox: what the Compatibility tab of
//! RobloxPlayerBeta.exe's Properties sets, per Roblox build. Windows keeps
//! them in the registry under the program's full path, so each installed
//! version gets its own entry, and Pious only ever touches those entries.

use std::path::Path;

use crate::core::model::Tweaks;

const LAYERS: &str = "Software\\Microsoft\\Windows NT\\CurrentVersion\\AppCompatFlags\\Layers";
pub const PLAYER_EXE: &str = "RobloxPlayerBeta.exe";

/// The compatibility flags the tweaks ask for, as Windows writes them.
pub fn layers(tweaks: &Tweaks) -> Vec<&'static str> {
    let mut out = Vec::new();
    if tweaks.disable_fullscreen_optimizations {
        out.push("DISABLEDXMAXIMIZEDWINDOWEDMODE");
    }
    match tweaks.dpi_override.as_deref() {
        Some("application") => out.push("HIGHDPIAWARE"),
        Some("system") => out.push("DPIUNAWARE"),
        Some("system_enhanced") => out.extend(["GDIDPISCALING", "DPIUNAWARE"]),
        _ => {}
    }
    out
}

/// Sets one Roblox build's compatibility flags (none clears them).
pub fn apply(build: &Path, flags: &[&str]) {
    let exe = build.join(PLAYER_EXE);
    if !exe.is_file() {
        return;
    }
    let name = exe.display().to_string();
    if flags.is_empty() {
        registry::delete(LAYERS, &name);
    } else {
        registry::set(LAYERS, &name, &format!("~ {}", flags.join(" ")));
    }
}

/// How much of the CPU Roblox gets ahead of other programs.
pub fn set_priority(pid: u32, priority: Option<&str>) {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            ABOVE_NORMAL_PRIORITY_CLASS, GetPriorityClass, HIGH_PRIORITY_CLASS, NORMAL_PRIORITY_CLASS, OpenProcess,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_INFORMATION, SetPriorityClass,
        };
        let class = match priority {
            Some("above_normal") => ABOVE_NORMAL_PRIORITY_CLASS,
            Some("high") => HIGH_PRIORITY_CLASS,
            _ => NORMAL_PRIORITY_CLASS,
        };
        let process = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return;
        }
        if GetPriorityClass(process) != class {
            SetPriorityClass(process, class);
        }
        CloseHandle(process);
    }
    #[cfg(not(windows))]
    let _ = (pid, priority);
}

#[cfg(windows)]
mod registry {
    use windows_sys::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteValueW,
        RegSetValueExW,
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn open(path: &str) -> Option<HKEY> {
        let mut key: HKEY = std::ptr::null_mut();
        let ok = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(path).as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        (ok == 0).then_some(key)
    }

    pub fn set(path: &str, name: &str, value: &str) {
        let Some(key) = open(path) else { return };
        let data = wide(value);
        unsafe {
            RegSetValueExW(key, wide(name).as_ptr(), 0, REG_SZ, data.as_ptr() as *const u8, (data.len() * 2) as u32);
            RegCloseKey(key);
        }
    }

    pub fn delete(path: &str, name: &str) {
        let Some(key) = open(path) else { return };
        unsafe {
            RegDeleteValueW(key, wide(name).as_ptr());
            RegCloseKey(key);
        }
    }
}

#[cfg(not(windows))]
mod registry {
    pub fn set(_path: &str, _name: &str, _value: &str) {}
    pub fn delete(_path: &str, _name: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_follow_the_tweaks() {
        let mut t = Tweaks::default();
        assert!(layers(&t).is_empty());
        t.disable_fullscreen_optimizations = true;
        t.dpi_override = Some("system_enhanced".into());
        assert_eq!(layers(&t), ["DISABLEDXMAXIMIZEDWINDOWEDMODE", "GDIDPISCALING", "DPIUNAWARE"]);
    }
}
