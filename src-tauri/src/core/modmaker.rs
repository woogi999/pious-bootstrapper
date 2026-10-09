//! The mod maker (Tweaks → Mods): recolors Roblox's own interface pictures
//! (top bar, menus, chat, cursors, the logo…) with a color or a gradient,
//! like Froststrap's mod generator.
//!
//! It's made from each Roblox version's original files whenever tweaks are
//! applied, so it follows Roblox's updates and is undone like every other
//! tweak. Recolored pictures are kept in the cache, so it's only slow the
//! first time.

use std::path::{Path, PathBuf};

use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

/// What to recolor and how. Saved with the tweaks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiMod {
    pub enabled: bool,
    /// Which parts (see [`PARTS`]).
    pub parts: Vec<String>,
    /// One color, or two for a gradient (`#RRGGBB`).
    pub colors: Vec<String>,
    /// The gradient's direction in degrees (0: left to right, 90: top to
    /// bottom).
    pub angle: f32,
    /// Keep the pictures' own light and shade (off: a flat color).
    pub keep_shading: bool,
}

impl Default for UiMod {
    fn default() -> Self {
        Self {
            enabled: false,
            parts: ["top_bar", "menu", "chat"].map(String::from).to_vec(),
            colors: vec!["#7C8CFF".into()],
            angle: 0.0,
            keep_shading: true,
        }
    }
}

/// The parts: ID, name, and the folders or files (under `content/textures`)
/// they're made of.
pub const PARTS: [(&str, &str, &[&str]); 11] = [
    ("top_bar", "Top bar", &["ui/TopBar"]),
    ("menu", "Menus and settings", &["ui/Settings", "ui/InGameMenu", "ui/Menu", "ui/MenuBar"]),
    ("chat", "Chat", &["ui/Chat"]),
    ("backpack", "Backpack", &["ui/Backpack"]),
    ("emotes", "Emotes", &["ui/Emotes"]),
    ("player_list", "Player list", &["ui/PlayerList"]),
    ("voice", "Voice chat", &["ui/VoiceChat"]),
    ("cursors", "Cursors", &["Cursors"]),
    ("shift_lock", "Shift lock", &["MouseLockedCursor.png"]),
    ("loading", "Loading screen", &["loading", "ui/LoadingScreen"]),
    ("logo", "Roblox logo", &["ui/TopBar/coloredlogo.png", "ui/TopBar/coloredlogo@2x.png", "ui/TopBar/coloredlogo@3x.png", "ui/ScreenshotHud/RobloxLogo.png", "ui/ScreenshotHud/RobloxLogo@2x.png", "ui/ScreenshotHud/RobloxLogo@3x.png", "loading/robloxlogo.png"]),
];

const TEXTURES: &str = "content/textures";
/// Bump when the recoloring changes, so cached pictures are made again.
const RECIPE: u32 = 1;

fn parse(color: &str) -> Option<[f32; 3]> {
    let hex = color.trim().trim_start_matches('#');
    if hex.len() < 6 {
        return None;
    }
    let channel = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok().map(|v| v as f32 / 255.0);
    Some([channel(0)?, channel(2)?, channel(4)?])
}

/// Every picture a set of parts covers in a build, relative to the build.
pub fn pictures(build: &Path, parts: &[String]) -> Vec<String> {
    let root = build.join(TEXTURES);
    let mut out = Vec::new();
    for (id, _, entries) in PARTS {
        if !parts.iter().any(|p| p == id) {
            continue;
        }
        for entry in entries.iter() {
            let path = root.join(entry);
            if path.is_file() {
                out.push(format!("{TEXTURES}/{entry}"));
            } else if path.is_dir() {
                let mut found = Vec::new();
                walk(&path, &mut found);
                for file in found {
                    if let Ok(relative) = file.strip_prefix(build) {
                        out.push(relative.to_string_lossy().replace('\\', "/"));
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) {
            out.push(path);
        }
    }
}

/// Recolors one picture: every visible pixel takes the color (or the
/// gradient's color at that spot), keeping its see-through and, with
/// `keep_shading`, how light it was.
pub fn recolor(picture: &RgbaImage, ui: &UiMod) -> RgbaImage {
    let colors: Vec<[f32; 3]> = ui.colors.iter().filter_map(|c| parse(c)).collect();
    let (first, last) = match colors.as_slice() {
        [] => ([1.0; 3], [1.0; 3]),
        [one] => (*one, *one),
        [a, .., b] => (*a, *b),
    };
    let (w, h) = picture.dimensions();
    let (dx, dy) = (ui.angle.to_radians().cos(), ui.angle.to_radians().sin());
    // How far along the gradient the corners reach, to spread it over the
    // whole picture at any angle.
    let reach = 0.5 * (dx.abs() + dy.abs()).max(1e-3);
    let mut out = RgbaImage::new(w, h);
    for (x, y, pixel) in picture.enumerate_pixels() {
        let Rgba([r, g, b, a]) = *pixel;
        if a == 0 {
            out.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            continue;
        }
        let fx = (x as f32 + 0.5) / w as f32 - 0.5;
        let fy = (y as f32 + 0.5) / h as f32 - 0.5;
        let t = ((fx * dx + fy * dy) / reach * 0.5 + 0.5).clamp(0.0, 1.0);
        let color = [0, 1, 2].map(|i| first[i] + (last[i] - first[i]) * t);
        let light = if ui.keep_shading { (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0 } else { 1.0 };
        let [cr, cg, cb] = color.map(|c| (c * light * 255.0).round().clamp(0.0, 255.0) as u8);
        out.put_pixel(x, y, Rgba([cr, cg, cb, a]));
    }
    out
}

fn encode(picture: &RgbaImage) -> Option<Vec<u8>> {
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(picture.clone()).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    Some(png)
}

/// A short fingerprint of the settings, naming the cache folder.
fn fingerprint(ui: &UiMod) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    RECIPE.hash(&mut hasher);
    ui.colors.hash(&mut hasher);
    ui.angle.to_bits().hash(&mut hasher);
    ui.keep_shading.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

/// The recolored pictures for `build` as (target, bytes), made from the
/// build's original files. `cache` keeps them between launches.
pub fn files(build: &Path, ui: &UiMod, cache: &Path) -> Vec<(String, Vec<u8>)> {
    if !ui.enabled || ui.parts.is_empty() {
        return Vec::new();
    }
    let folder = cache.join("modmaker").join(fingerprint(ui));
    let mut out = Vec::new();
    for relative in pictures(build, &ui.parts) {
        let source = crate::core::tweaks::original(build, &relative);
        let Ok(original) = std::fs::read(&source) else { continue };
        // Cached by the original's size too, so a Roblox update that
        // changes a picture makes it again.
        let cached = folder.join(format!("{}-{}", original.len(), relative.replace('/', "~")));
        if let Ok(bytes) = std::fs::read(&cached) {
            out.push((relative, bytes));
            continue;
        }
        let Ok(picture) = image::load_from_memory(&original) else { continue };
        let Some(bytes) = encode(&recolor(&picture.to_rgba8(), ui)) else { continue };
        let _ = std::fs::create_dir_all(&folder);
        let _ = std::fs::write(&cached, &bytes);
        out.push((relative, bytes));
    }
    out
}

/// A few of the build's pictures for each chosen part, recolored, as data
/// URLs for the preview in Tweaks.
pub fn preview(build: &Path, ui: &UiMod, per_part: usize) -> Vec<(String, String)> {
    use base64::Engine;
    let mut out = Vec::new();
    for (id, name, _) in PARTS {
        if !ui.parts.iter().any(|p| p == id) {
            continue;
        }
        let mut shown = 0;
        for relative in pictures(build, &[id.to_owned()]) {
            if shown >= per_part {
                break;
            }
            let Ok(bytes) = std::fs::read(crate::core::tweaks::original(build, &relative)) else { continue };
            let Ok(picture) = image::load_from_memory(&bytes) else { continue };
            let picture = picture.to_rgba8();
            // Icons, not huge backgrounds or tiny slivers.
            let (w, h) = picture.dimensions();
            if w < 16 || h < 16 || w > 256 || h > 256 || relative.contains("@2x") || relative.contains("@3x") {
                continue;
            }
            let Some(png) = encode(&recolor(&picture, &UiMod { enabled: true, ..ui.clone() })) else { continue };
            out.push((name.to_owned(), format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))));
            shown += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn icon() -> RgbaImage {
        let mut p = RgbaImage::new(4, 2);
        p.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        p.put_pixel(3, 0, Rgba([255, 255, 255, 128]));
        p.put_pixel(1, 1, Rgba([128, 128, 128, 255]));
        p
    }

    #[test]
    fn one_color_keeps_see_through_and_shading() {
        let ui = UiMod { enabled: true, colors: vec!["#FF0000".into()], ..UiMod::default() };
        let out = recolor(&icon(), &ui);
        assert_eq!(out.get_pixel(0, 0).0, [255, 0, 0, 255]);
        assert_eq!(out.get_pixel(3, 0).0[3], 128, "see-through stays");
        assert!(out.get_pixel(1, 1).0[0] < 200, "grey stays darker");
        assert_eq!(out.get_pixel(2, 0).0[3], 0, "empty stays empty");
        let flat = recolor(&icon(), &UiMod { keep_shading: false, ..ui });
        assert_eq!(flat.get_pixel(1, 1).0, [255, 0, 0, 255]);
    }

    #[test]
    fn gradients_run_across_the_picture() {
        let ui = UiMod { enabled: true, colors: vec!["#000000".into(), "#FFFFFF".into()], angle: 0.0, ..UiMod::default() };
        let out = recolor(&icon(), &ui);
        assert!(out.get_pixel(0, 0).0[0] < out.get_pixel(3, 0).0[0], "left darker than right");
    }

    #[test]
    fn finds_the_parts_in_a_build_and_makes_them() {
        let build = std::env::temp_dir().join(format!("pious-modmaker-{}", uuid::Uuid::new_v4().simple()));
        let topbar = build.join("content/textures/ui/TopBar");
        std::fs::create_dir_all(&topbar).unwrap();
        icon().save(topbar.join("icon.png")).unwrap();
        icon().save(build.join("content/textures/MouseLockedCursor.png")).unwrap();
        let ui = UiMod { enabled: true, parts: vec!["top_bar".into(), "shift_lock".into()], colors: vec!["#00FF00".into()], ..UiMod::default() };
        assert_eq!(pictures(&build, &ui.parts), ["content/textures/MouseLockedCursor.png", "content/textures/ui/TopBar/icon.png"]);
        let made = files(&build, &ui, &build.join("cache"));
        assert_eq!(made.len(), 2);
        let first = image::load_from_memory(&made[0].1).unwrap().to_rgba8();
        assert_eq!(first.get_pixel(0, 0).0, [0, 255, 0, 255]);
        // Off: nothing.
        assert!(files(&build, &UiMod { enabled: false, ..ui }, &build.join("cache")).is_empty());
        let _ = std::fs::remove_dir_all(build);
    }
}
