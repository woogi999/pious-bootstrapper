//! The installer is the same program for every release: it downloads the
//! app from GitHub Releases at run time, so nothing about a version is
//! baked in here.
//!
//! Its manifest (`pious-setup.manifest`) starts it as whoever started it;
//! it then asks for administrator rights itself (`relaunch_elevated`).

fn main() {
    println!("cargo:rerun-if-changed=pious-setup.manifest");
    let windows = tauri_build::WindowsAttributes::new().app_manifest(include_str!("pious-setup.manifest"));
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows)).expect("tauri build");
}
