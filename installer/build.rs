//! Embeds the app (a zip made by scripts/release.ps1) in the installer.
//! `PIOUS_PAYLOAD` names the zip; without it the installer is built empty
//! (it says so when run), which is enough for working on its interface.

fn main() {
    println!("cargo:rerun-if-env-changed=PIOUS_PAYLOAD");
    let payload = std::env::var("PIOUS_PAYLOAD").ok().filter(|p| std::path::Path::new(p).is_file());
    let path = match payload {
        Some(path) => path,
        None => {
            // An empty zip: just the end-of-archive record.
            let mut empty = vec![0x50, 0x4b, 0x05, 0x06];
            empty.extend([0u8; 18]);
            let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("empty.zip");
            std::fs::write(&out, empty).unwrap();
            out.display().to_string()
        }
    };
    println!("cargo:rerun-if-changed={path}");
    println!("cargo:rustc-env=PIOUS_PAYLOAD_FILE={path}");

    // The version being installed is the app's.
    let manifest = std::fs::read_to_string("../src-tauri/Cargo.toml").unwrap_or_default();
    let version = manifest
        .lines()
        .find_map(|l| l.strip_prefix("version = \"").and_then(|v| v.strip_suffix('"')))
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .to_owned();
    println!("cargo:rerun-if-changed=../src-tauri/Cargo.toml");
    println!("cargo:rustc-env=PIOUS_VERSION={version}");

    // This version's highlights from the changelog: its bullet points.
    let changelog = std::fs::read_to_string("../CHANGELOG.md").unwrap_or_default();
    let mut inside = false;
    let mut notes = Vec::new();
    for line in changelog.lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            if inside {
                break;
            }
            inside = heading.trim() == version;
            continue;
        }
        if inside {
            if let Some(item) = line.trim().strip_prefix("- ") {
                notes.push(item.replace("**", ""));
            }
        }
    }
    let out = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("notes.txt");
    std::fs::write(&out, notes.join("
")).unwrap();
    println!("cargo:rerun-if-changed=../CHANGELOG.md");

    tauri_build::build();
}
