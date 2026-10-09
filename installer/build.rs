//! The installer is the same program for every release: it downloads the
//! app from GitHub Releases at run time, so nothing about a version is
//! baked in here.
//!
//! Release builds (what `update_release.bat` and the release workflow
//! publish) carry `pious-setup.manifest`, so Windows runs them as
//! administrator. Debug builds don't, so `cargo test` and development runs
//! work without elevation (Windows refuses to start a test program that
//! demands it).

fn main() {
    println!("cargo:rerun-if-changed=pious-setup.manifest");
    let release = std::env::var("PROFILE").as_deref() == Ok("release");
    let mut attributes = tauri_build::Attributes::new();
    if release {
        attributes = attributes.windows_attributes(tauri_build::WindowsAttributes::new().app_manifest(include_str!("pious-setup.manifest")));
    }
    tauri_build::try_build(attributes).expect("tauri build");
}
