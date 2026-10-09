//! The fonts installed on this PC, by family name, for the interface font
//! picker. Windows lists every installed font file in the registry
//! ("Arial Bold (TrueType)"); the family is that name without its style.

/// Style words that aren't part of a family's name.
const STYLES: [&str; 22] = [
    "Regular", "Bold", "Italic", "Oblique", "Light", "Semilight", "SemiLight", "Semibold", "SemiBold", "Demibold", "DemiBold", "Black",
    "Heavy", "Medium", "Thin", "ExtraLight", "ExtraBold", "UltraLight", "UltraBold", "Condensed", "Narrow", "Variable",
];

/// "Segoe UI Semibold Italic (TrueType)" → "Segoe UI". `None` for bitmap
/// fonts and other things the interface can't use.
pub fn family(entry: &str) -> Vec<String> {
    // Without "(TrueType)" or "(OpenType)" it's an old bitmap font.
    let Some((name, kind)) = entry.rsplit_once(" (") else { return Vec::new() };
    if !(kind.starts_with("TrueType") || kind.starts_with("OpenType")) {
        return Vec::new();
    }
    name.split(" & ")
        .map(|part| {
            let mut words: Vec<&str> = part.split_whitespace().collect();
            while words.len() > 1 && words.last().is_some_and(|w| STYLES.iter().any(|s| s.eq_ignore_ascii_case(w))) {
                words.pop();
            }
            words.join(" ")
        })
        .filter(|f| !f.is_empty() && !f.starts_with('@'))
        .collect()
}

/// Every installed font family, sorted, without repeats.
pub fn installed() -> Vec<String> {
    let mut out: Vec<String> = entries().iter().flat_map(|e| family(e)).collect();
    out.sort_by_key(|f| f.to_lowercase());
    out.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    out
}

#[cfg(windows)]
fn entries() -> Vec<String> {
    use windows_sys::Win32::System::Registry::{HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RegCloseKey, RegEnumValueW, RegOpenKeyExW};
    const KEY: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Fonts";
    let key_name: Vec<u16> = KEY.encode_utf16().chain([0]).collect();
    let mut out = Vec::new();
    for root in [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER] {
        let mut key: HKEY = std::ptr::null_mut();
        if unsafe { RegOpenKeyExW(root, key_name.as_ptr(), 0, KEY_READ, &mut key) } != 0 {
            continue;
        }
        for index in 0.. {
            let mut name = vec![0u16; 512];
            let mut len = name.len() as u32;
            let status = unsafe {
                RegEnumValueW(key, index, name.as_mut_ptr(), &mut len, std::ptr::null(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut())
            };
            if status != 0 {
                break;
            }
            out.push(String::from_utf16_lossy(&name[..len as usize]));
        }
        unsafe { RegCloseKey(key) };
    }
    out
}

#[cfg(not(windows))]
fn entries() -> Vec<String> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_from_registry_names() {
        assert_eq!(family("Segoe UI Semibold Italic (TrueType)"), ["Segoe UI"]);
        assert_eq!(family("Arial (TrueType)"), ["Arial"]);
        assert_eq!(family("Cambria & Cambria Math (TrueType)"), ["Cambria", "Cambria Math"]);
        assert_eq!(family("Bahnschrift (TrueType)"), ["Bahnschrift"]);
        assert!(family("Courier 10,12,15").len() <= 1);
        assert!(family("Modern (All res)").is_empty());
    }

    #[test]
    fn this_pc_has_fonts() {
        if cfg!(windows) {
            let fonts = installed();
            assert!(fonts.iter().any(|f| f == "Arial" || f == "Segoe UI"), "{fonts:?}");
        }
    }
}
