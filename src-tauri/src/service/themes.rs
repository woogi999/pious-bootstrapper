//! Themes: folders with a `theme.json` that set how Pious looks (colors, a
//! gradient, the font, corner roundness, glass, a background picture) and
//! may bring a stylesheet, icons, sounds and a font file. They live in the
//! themes folder (next to Pious when installed) or inside plugins.
//!
//! Using a theme copies its look into Settings → Appearance, so anything can
//! still be fine-tuned after; its extras (CSS, icons, font file) apply while
//! it's the theme in use. A theme that doesn't read is listed with the
//! reason and can't be used; it never stops Pious. See `docs/THEMES.md`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::plugins::Plugin;
use crate::core::model::{Appearance, Blur, Gradient};
use crate::core::store;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Manifest {
    id: String,
    name: String,
    version: String,
    author: String,
    description: String,
    colors: Colors,
    gradient: Option<Gradient>,
    /// A font installed on the PC, by family name, or the family name of
    /// `font_file`.
    font: Option<String>,
    font_file: Option<String>,
    radius: Option<f32>,
    glass: Option<f32>,
    font_scale: Option<f32>,
    see_through: Option<bool>,
    window_opacity: Option<f32>,
    blur: Option<Blur>,
    background_image: Option<String>,
    image_dim: Option<f32>,
    image_blur: Option<f32>,
    css: Option<String>,
    icons: BTreeMap<String, String>,
    sounds: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Colors {
    accent: Option<String>,
    accent_2: Option<String>,
    background: Option<String>,
    surface: Option<String>,
    text: Option<String>,
}

/// A theme as the window sees it.
#[derive(Debug, Clone, Serialize)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub folder: PathBuf,
    /// The plugin it came with, if any.
    pub plugin: Option<String>,
    /// Why it can't be used, if it can't.
    pub problem: Option<String>,
    /// Its colors (for a preview swatch): accent, background, surface, text.
    pub swatch: Vec<String>,
    #[serde(skip)]
    pub look: Option<Appearance>,
    #[serde(skip)]
    pub css: Option<PathBuf>,
    #[serde(skip)]
    pub icons: BTreeMap<String, PathBuf>,
    #[serde(skip)]
    pub sounds: BTreeMap<String, PathBuf>,
    #[serde(skip)]
    pub font: Option<String>,
    #[serde(skip)]
    pub font_file: Option<PathBuf>,
}

fn valid_hex(text: &str) -> bool {
    let hex = text.trim().trim_start_matches('#');
    hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

/// Reads one theme folder. Never fails: problems come back in `problem`.
pub fn read(folder: &Path, plugin: Option<&str>) -> Theme {
    let fallback = folder.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut theme = Theme {
        id: fallback.clone(),
        name: fallback.clone(),
        version: String::new(),
        author: String::new(),
        description: String::new(),
        folder: folder.to_path_buf(),
        plugin: plugin.map(str::to_owned),
        problem: None,
        swatch: Vec::new(),
        look: None,
        css: None,
        icons: BTreeMap::new(),
        sounds: BTreeMap::new(),
        font: None,
        font_file: None,
    };
    let manifest = match std::fs::read(folder.join("theme.json")) {
        Err(_) => {
            theme.problem = Some("It has no theme.json.".into());
            return theme;
        }
        Ok(bytes) => match serde_json::from_slice::<Manifest>(&bytes) {
            Ok(m) => m,
            Err(e) => {
                theme.problem = Some(format!("Its theme.json doesn't read ({e})."));
                return theme;
            }
        },
    };
    if !manifest.id.trim().is_empty() {
        theme.id = manifest.id.trim().to_owned();
    }
    // Themes from plugins are told apart from loose ones with the same ID.
    if let Some(plugin) = plugin {
        theme.id = format!("{plugin}/{}", theme.id);
    }
    if !manifest.name.trim().is_empty() {
        theme.name = manifest.name.clone();
    }
    theme.version = manifest.version.clone();
    theme.author = manifest.author.clone();
    theme.description = manifest.description.clone();

    let file = |relative: &str| -> Result<PathBuf, String> {
        let path = folder.join(relative);
        let ok = path.canonicalize().ok().zip(folder.canonicalize().ok()).is_some_and(|(p, f)| p.starts_with(f));
        if ok { Ok(path) } else { Err(format!("{relative} is missing from the theme's folder.")) }
    };
    let mut problems = Vec::new();
    let mut look = Appearance { theme: Some(theme.id.clone()), ..Appearance::default() };
    let c = &manifest.colors;
    for (value, slot) in [
        (&c.accent, &mut look.accent),
        (&c.background, &mut look.background),
        (&c.surface, &mut look.surface),
        (&c.text, &mut look.text),
        (&c.accent_2, &mut look.accent_2),
    ] {
        if let Some(value) = value {
            if valid_hex(value) {
                *slot = value.trim().to_owned();
            } else {
                problems.push(format!("{value} isn't a color like #1A2B3C."));
            }
        }
    }
    if let Some(gradient) = &manifest.gradient {
        if valid_hex(&gradient.from) && valid_hex(&gradient.to) {
            look.gradient = Gradient { enabled: true, ..gradient.clone() };
            look.gradient.opacity = look.gradient.opacity.clamp(0.0, 1.0);
        } else {
            problems.push("The gradient's colors must look like #1A2B3C.".into());
        }
    }
    if let Some(v) = manifest.radius {
        look.radius = v.clamp(0.0, 2.5);
    }
    if let Some(v) = manifest.glass {
        look.glass = v.clamp(0.3, 2.5);
    }
    if let Some(v) = manifest.font_scale {
        look.font_scale = v.clamp(0.8, 1.3);
    }
    if let Some(v) = manifest.see_through {
        look.see_through = v;
    }
    if let Some(v) = manifest.window_opacity {
        look.window_opacity = v.clamp(0.15, 1.0);
    }
    if let Some(v) = manifest.blur {
        look.blur = v;
    }
    if let Some(v) = manifest.image_dim {
        look.image_dim = v.clamp(0.0, 1.0);
    }
    if let Some(v) = manifest.image_blur {
        look.image_blur = v.clamp(0.0, 1.0);
    }
    if let Some(picture) = &manifest.background_image {
        match file(picture) {
            Ok(path) => look.background_image = Some(path),
            Err(e) => problems.push(e),
        }
    }
    if let Some(font_file) = &manifest.font_file {
        match file(font_file) {
            Ok(path) => theme.font_file = Some(path),
            Err(e) => problems.push(e),
        }
    }
    theme.font = manifest.font.clone().filter(|f| !f.trim().is_empty());
    // A bundled font is loaded under its family name; a system font is used
    // by name.
    look.font = theme.font.clone().unwrap_or_default();
    if let Some(css) = &manifest.css {
        match file(css) {
            Ok(path) => theme.css = Some(path),
            Err(e) => problems.push(e),
        }
    }
    for (name, relative) in &manifest.icons {
        match file(relative) {
            Ok(path) => {
                theme.icons.insert(name.clone(), path);
            }
            Err(e) => problems.push(e),
        }
    }
    for (name, relative) in &manifest.sounds {
        match file(relative) {
            Ok(path) => {
                theme.sounds.insert(name.clone(), path);
            }
            Err(e) => problems.push(e),
        }
    }
    theme.swatch = vec![look.accent.clone(), look.background.clone(), look.surface.clone(), look.text.clone()];
    if look.gradient.enabled {
        theme.swatch.push(look.gradient.from.clone());
        theme.swatch.push(look.gradient.to.clone());
    }
    theme.problem = problems.into_iter().next();
    if theme.problem.is_none() {
        theme.look = Some(look);
    }
    theme
}

/// Every folder themes are read from: the themes folder, and in
/// development builds the repository's own.
pub fn dirs() -> Vec<PathBuf> {
    let mut dirs = vec![store::themes_dir()];
    if cfg!(debug_assertions) {
        let repo = super::plugins::repo_root().join("themes");
        if repo.is_dir() {
            dirs.push(repo);
        }
    }
    dirs
}

/// Every theme: the themes folder's (and the repository's, in development
/// builds), then those enabled plugins bring.
pub fn list(plugins: &[Plugin]) -> Vec<Theme> {
    let mut out: Vec<Theme> = Vec::new();
    for dir in dirs() {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        let mut found: Vec<Theme> = entries.flatten().filter(|e| e.path().is_dir()).map(|e| read(&e.path(), None)).collect();
        found.retain(|t| !out.iter().any(|o| o.id == t.id));
        out.extend(found);
    }
    for plugin in plugins.iter().filter(|p| p.enabled) {
        out.extend(plugin.themes.iter().map(|folder| read(folder, Some(&plugin.id))));
    }
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    out
}

pub fn find(id: &str, plugins: &[Plugin]) -> Option<Theme> {
    list(plugins).into_iter().find(|t| t.id == id && t.problem.is_none())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pious-theme-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_a_full_theme() {
        let dir = scratch();
        std::fs::write(dir.join("style.css"), ".sidebar { opacity: .9 }").unwrap();
        std::fs::write(
            dir.join("theme.json"),
            r##"{ "id": "midnight", "name": "Midnight", "colors": { "accent": "#7C5CFF", "background": "#07070C" },
                 "gradient": { "from": "#20104A", "to": "#07070C", "angle": 160, "opacity": 0.8 },
                 "font": "Segoe UI", "radius": 0.5, "css": "style.css" }"##,
        )
        .unwrap();
        let theme = read(&dir, None);
        assert!(theme.problem.is_none(), "{:?}", theme.problem);
        let look = theme.look.unwrap();
        assert_eq!(look.accent, "#7C5CFF");
        assert_eq!(look.text, Appearance::default().text, "unset colors stay Pious's");
        assert!(look.gradient.enabled);
        assert_eq!(look.font, "Segoe UI");
        assert_eq!(look.theme.as_deref(), Some("midnight"));
        assert!(theme.css.is_some());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn shipped_themes_read() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("themes");
        let themes: Vec<Theme> = std::fs::read_dir(&dir).unwrap().flatten().filter(|e| e.path().is_dir()).map(|e| read(&e.path(), None)).collect();
        assert!(themes.len() >= 2);
        for theme in themes {
            assert!(theme.problem.is_none(), "{}: {:?}", theme.name, theme.problem);
            assert!(theme.look.is_some());
        }
    }

    #[test]
    fn broken_themes_are_listed_not_used() {
        let dir = scratch();
        std::fs::write(dir.join("theme.json"), "{ not json").unwrap();
        let theme = read(&dir, None);
        assert!(theme.problem.is_some() && theme.look.is_none());

        std::fs::write(dir.join("theme.json"), r#"{ "colors": { "accent": "purple" } }"#).unwrap();
        assert!(read(&dir, None).problem.unwrap().contains("purple"));

        // Files must stay inside the theme's folder.
        std::fs::write(dir.join("theme.json"), r#"{ "css": "../../secret.css" }"#).unwrap();
        assert!(read(&dir, None).problem.is_some());

        let empty = scratch();
        assert!(read(&empty, None).problem.unwrap().contains("theme.json"));
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_dir_all(empty);
    }
}
