//! Bootstrapper-style client tweaks: FastFlags (frame rate cap, graphics
//! API, anti-aliasing, textures), a custom font and a mods folder.
//!
//! Tweaks are written into a build's folder right before Pious launches it.
//! Every file Pious replaces is backed up first and listed in a manifest in
//! that folder, so turning tweaks off (or changing them) restores the
//! originals exactly, including files a bootstrapper put there.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::core::fonts;
use crate::core::model::{CursorStyle, EmojiStyle, GraphicsApi, Tweaks};

/// In `presets`, the replacement for Roblox's fonts (not a real path).
const FONT_SLOT: &str = "@font";

const MANIFEST: &str = ".pious-tweaks.json";
const BACKUP: &str = ".pious-backup";

/// A build's own copy of a file, even if Pious replaced it.
pub fn original(build: &Path, relative: &str) -> std::path::PathBuf {
    let backup = build.join(BACKUP).join(relative);
    if backup.is_file() { backup } else { build.join(relative) }
}
const FLAGS: &str = "ClientSettings/ClientAppSettings.json";

#[derive(Default, Serialize, Deserialize)]
struct Manifest {
    /// Files Pious wrote, relative to the build folder, and whether an
    /// original was backed up.
    files: BTreeMap<String, bool>,
}

/// The FastFlags these tweaks set, as Roblox reads them. Only flags on
/// Roblox's allowlist are used: Roblox refuses the rest (the frame rate cap
/// lives in its settings file instead, see [`settings_values`]).
pub fn fast_flags(tweaks: &Tweaks) -> Result<Map<String, Value>, String> {
    let mut flags = Map::new();
    let mut set = |key: &str, value: &str| {
        flags.insert(key.to_owned(), Value::String(value.to_owned()));
    };

    match tweaks.graphics {
        GraphicsApi::Automatic => {}
        GraphicsApi::Direct3D11 => set("FFlagDebugGraphicsPreferD3D11", "True"),
        GraphicsApi::Vulkan => set("FFlagDebugGraphicsPreferVulkan", "True"),
        GraphicsApi::OpenGL => set("FFlagDebugGraphicsPreferOpenGL", "True"),
    }
    if tweaks.msaa_off {
        set("FIntDebugForceMSAASamples", "0");
    } else if tweaks.msaa > 0 {
        set("FIntDebugForceMSAASamples", &tweaks.msaa.to_string());
    }
    if let Some(level) = tweaks.render_quality {
        // The old hidden quality setting: 21 steps, below Roblox's lowest
        // and above its highest slider position.
        set("DFIntDebugFRMQualityLevelOverride", &level.clamp(1, 21).to_string());
    }
    if let Some(quality) = tweaks.texture_quality {
        set("DFFlagTextureQualityOverrideEnabled", "True");
        set("DFIntTextureQualityOverride", &quality.min(3).to_string());
    }
    if tweaks.disable_dpi_scaling {
        set("DFFlagDisableDPIScale", "True");
    }
    if tweaks.alt_enter_fullscreen {
        set("FFlagHandleAltEnterFullscreenManually", "False");
    }
    if tweaks.pause_voxelizer {
        set("DFFlagDebugPauseVoxelizer", "True");
    }
    if tweaks.gray_sky {
        set("FFlagDebugSkyGray", "True");
    }
    if tweaks.still_grass {
        set("FIntGrassMovementReducedMotionFactor", "0");
    }
    if tweaks.no_grass {
        set("FIntFRMMinGrassDistance", "0");
        set("FIntFRMMaxGrassDistance", "0");
        set("FIntRenderGrassDetailStrands", "0");
    }
    if let Some(detail) = tweaks.mesh_detail {
        let distance = [0, 100, 250, 500, 1000][detail.min(4) as usize].to_string();
        for flag in [
            "DFIntCSGLevelOfDetailSwitchingDistance",
            "DFIntCSGLevelOfDetailSwitchingDistanceL12",
            "DFIntCSGLevelOfDetailSwitchingDistanceL23",
            "DFIntCSGLevelOfDetailSwitchingDistanceL34",
        ] {
            set(flag, &distance);
        }
    }

    // The active FastFlag profile goes last, so it can override presets.
    if let Some(profile) = tweaks.active_flags() {
        for (key, value) in &profile.flags {
            flags.insert(key.clone(), Value::String(value.clone()));
        }
    }
    Ok(flags)
}

/// The tweaks that live in Roblox's own settings file
/// (GlobalBasicSettings_13.xml), like Fishstrap's settings editor: the frame
/// rate cap and graphics quality. Set right before a game starts.
pub fn settings_values(tweaks: &Tweaks) -> Vec<(&'static str, crate::core::robloxsettings::Kind, String)> {
    use crate::core::robloxsettings::Kind;
    let mut out = Vec::new();
    if !tweaks.enabled {
        return out;
    }
    // 0 breaks Roblox's renderer; "unlimited" is a cap no screen reaches.
    if let Some(fps) = tweaks.fps_limit {
        out.push(("FramerateCap", Kind::Int, fps.clamp(1, 9999).to_string()));
    }
    if let Some(level) = tweaks.graphics_quality {
        out.push(("SavedQualityLevel", Kind::Token, level.clamp(1, 10).to_string()));
    }
    out
}

/// The values Roblox's settings file had before Pious changed them, so
/// turning a tweak (or all of them) off puts them back.
fn settings_record(data_dir: &Path) -> PathBuf {
    data_dir.join("tweaks-settings.json")
}

type SettingsRecord = BTreeMap<String, (crate::core::robloxsettings::Kind, Option<String>)>;

fn load_record(data_dir: &Path) -> SettingsRecord {
    std::fs::read(settings_record(data_dir)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_record(data_dir: &Path, record: &SettingsRecord) -> Result<(), String> {
    let path = settings_record(data_dir);
    if record.is_empty() {
        let _ = std::fs::remove_file(path);
        return Ok(());
    }
    let json = serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
    let partial = path.with_extension("json.part");
    std::fs::write(&partial, json).and_then(|_| std::fs::rename(&partial, &path)).map_err(|e| format!("Couldn't save the tweaks record ({e})."))
}

/// Puts the tweaks that live in Roblox's settings file in place (right
/// before a game starts), remembering each original value first, and puts
/// back the originals of tweaks that are no longer wanted.
pub fn apply_settings(tweaks: &Tweaks, data_dir: &Path) -> Result<(), String> {
    use crate::core::robloxsettings::{apply_live, live_value};
    let wanted = settings_values(tweaks);
    let mut record = load_record(data_dir);
    for (name, kind, _) in &wanted {
        if !record.contains_key(*name) {
            record.insert((*name).to_owned(), (*kind, live_value(name, *kind)));
        }
    }
    // Saved before anything changes, so a crash can't lose an original.
    save_record(data_dir, &record)?;
    let mut values: Vec<(&str, crate::core::robloxsettings::Kind, String)> = wanted.clone();
    let mut removed = Vec::new();
    let mut done = Vec::new();
    for (name, (kind, original)) in &record {
        if wanted.iter().any(|(n, ..)| n == name) {
            continue;
        }
        match original {
            Some(original) => values.push((name.as_str(), *kind, original.clone())),
            // It wasn't there before: it goes again.
            None => removed.push((name.as_str(), *kind)),
        }
        done.push(name.clone());
    }
    apply_live(&values)?;
    crate::core::robloxsettings::remove_live(&removed)?;
    for name in done {
        record.remove(&name);
    }
    save_record(data_dir, &record)
}

/// Puts back every value of Roblox's settings file that tweaks changed.
/// `keep_record`: a Roblox window is open and will write its own copy back
/// when it closes, so the originals are kept to put back again next time.
pub fn restore_settings(data_dir: &Path, keep_record: bool) -> Result<(), String> {
    let record = load_record(data_dir);
    let values: Vec<(&str, crate::core::robloxsettings::Kind, String)> =
        record.iter().filter_map(|(name, (kind, original))| Some((name.as_str(), *kind, original.clone()?))).collect();
    let added: Vec<(&str, crate::core::robloxsettings::Kind)> =
        record.iter().filter(|(_, (_, original))| original.is_none()).map(|(name, (kind, _))| (name.as_str(), *kind)).collect();
    crate::core::robloxsettings::apply_live(&values)?;
    crate::core::robloxsettings::remove_live(&added)?;
    if !keep_record {
        save_record(data_dir, &SettingsRecord::new())?;
    }
    Ok(())
}

/// Where mod-preset files come from (Fishstrap's open-source resources).
const PRESET_SOURCE: &str = "https://raw.githubusercontent.com/fishstrap/fishstrap/main/Bloxstrap/Resources/Mods";

/// Voidstrap's cursor set (MIT).
const VOIDSTRAP_CURSORS: &str = "https://raw.githubusercontent.com/KloBraticc/Voidstrap/main/src/Voidstrap.App/Resources/Mods/Cursor";
/// Froststrap's cursor set (MPL-2.0).
const FROSTSTRAP_CURSORS: &str = "https://raw.githubusercontent.com/Froststrap/Froststrap/main/Froststrap/Resources/Mods/Cursor";

/// Where a cursor style's pictures come from: (base URL, folder, files
/// besides the pointer and the far pointer).
pub fn cursor_source(style: CursorStyle) -> Option<(String, &'static str, &'static [&'static str])> {
    const DECAL: &[&str] = &["ArrowCursorDecalDrag.png"];
    const IBEAM: &[&str] = &["IBeamCursor.png"];
    let fish = || format!("{PRESET_SOURCE}/Cursor");
    let void = || VOIDSTRAP_CURSORS.to_owned();
    let frost = || FROSTSTRAP_CURSORS.to_owned();
    Some(match style {
        CursorStyle::Default => return None,
        CursorStyle::From2006 => (fish(), "From2006", &[]),
        CursorStyle::From2013 => (fish(), "From2013", &[]),
        CursorStyle::BibataModernIce => (void(), "BibataModernIce", &["ArrowCursorDecalDrag.png", "IBeamCursor.png"]),
        CursorStyle::Clean => (void(), "CleanCursor", DECAL),
        CursorStyle::Dot => (void(), "DotCursor", DECAL),
        CursorStyle::Fps => (void(), "FPSCursor", DECAL),
        CursorStyle::Stoofs => (void(), "StoofsCursor", DECAL),
        CursorStyle::VerySmallWhiteDot => (void(), "VerySmallWhiteDot", DECAL),
        CursorStyle::WhiteDot => (void(), "WhiteDotCursor", DECAL),
        CursorStyle::BlackAndWhiteDot => (frost(), "BlackAndWhiteDot", IBEAM),
        CursorStyle::PurpleCross => (frost(), "PurpleCross", IBEAM),
    })
}

/// Every cursor style with its name, for the picker.
pub const CURSORS: [(CursorStyle, &str); 12] = [
    (CursorStyle::Default, "Roblox's"),
    (CursorStyle::From2013, "2013 (classic)"),
    (CursorStyle::From2006, "2006 (oldest)"),
    (CursorStyle::BibataModernIce, "Bibata Modern Ice"),
    (CursorStyle::Clean, "Clean"),
    (CursorStyle::Dot, "Dot"),
    (CursorStyle::Fps, "FPS crosshair"),
    (CursorStyle::Stoofs, "Stoofs"),
    (CursorStyle::VerySmallWhiteDot, "Tiny white dot"),
    (CursorStyle::WhiteDot, "White dot"),
    (CursorStyle::BlackAndWhiteDot, "Black and white dot"),
    (CursorStyle::PurpleCross, "Purple cross"),
];

/// The mod-preset files these tweaks need: (path in the build, source URL).
fn preset_files(tweaks: &Tweaks) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut add = |target: &str, source: String| files.push((target.to_owned(), source));
    let cursors = "content/textures/Cursors/KeyboardMouse";
    if let Some((base, folder, extra)) = cursor_source(tweaks.cursor) {
        add(&format!("{cursors}/ArrowCursor.png"), format!("{base}/{folder}/ArrowCursor.png"));
        add(&format!("{cursors}/ArrowFarCursor.png"), format!("{base}/{folder}/ArrowFarCursor.png"));
        for file in extra {
            let target = match *file {
                // Dragging decals uses the one in the textures folder itself.
                "ArrowCursorDecalDrag.png" => format!("content/textures/{file}"),
                _ => format!("{cursors}/{file}"),
            };
            add(&target, format!("{base}/{folder}/{file}"));
        }
    }
    if tweaks.old_character_sounds {
        for (target, source) in [
            ("action_footsteps_plastic.mp3", "OldWalk.mp3"),
            ("action_jump.mp3", "OldJump.mp3"),
            ("action_get_up.mp3", "OldGetUp.mp3"),
            ("action_falling.mp3", "Empty.mp3"),
            ("action_jump_land.mp3", "Empty.mp3"),
            ("action_swim.mp3", "Empty.mp3"),
            ("impact_water.mp3", "Empty.mp3"),
        ] {
            add(&format!("content/sounds/{target}"), format!("{PRESET_SOURCE}/Sounds/{source}"));
        }
    }
    if tweaks.old_avatar_background {
        add("ExtraContent/places/Mobile.rbxl", format!("{PRESET_SOURCE}/OldAvatarBackground.rbxl"));
    }
    if let Some(url) = fonts::emoji_url(tweaks.emoji) {
        add("content/fonts/TwemojiMozilla.ttf", url);
    }
    if let Some(url) = tweaks.font_preset.as_deref().and_then(fonts::preset_url) {
        add(FONT_SLOT, url.to_owned());
    }
    files
}

/// Downloads (once, then from `cache`) the mod-preset files these tweaks
/// use. Returns them as (path in the build, contents).
pub async fn preset_mods(tweaks: &Tweaks, cache: &Path, build: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    if !tweaks.enabled {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    // The mod maker's pictures first: a chosen cursor or shift lock
    // (below), plugins and the user's own mods are written after them, so
    // they win.
    if tweaks.ui_mod.enabled {
        let (build, ui, cache) = (build.to_path_buf(), tweaks.ui_mod.clone(), cache.to_path_buf());
        out.extend(tokio::task::spawn_blocking(move || crate::core::modmaker::files(&build, &ui, &cache)).await.map_err(|e| e.to_string())?);
    }
    for (target, url) in preset_files(tweaks) {
        let parts: Vec<&str> = url.rsplit('/').take(2).collect();
        let name = format!("{}-{}", parts[1], parts[0]);
        out.push((target, fonts::cached_download(&url, cache, &name).await?));
    }
    if tweaks.emoji == EmojiStyle::Apple {
        // Built from Roblox's original emoji font, never from a replacement.
        let relative = "content/fonts/TwemojiMozilla.ttf";
        let original = build.join(BACKUP).join(relative);
        let source = if original.is_file() { original } else { build.join(relative) };
        let base = tokio::fs::read(&source).await.map_err(|e| format!("Couldn't read Roblox's emoji font ({e})."))?;
        out.push((relative.to_owned(), fonts::apple_emoji_font(base, cache).await?));
    }
    if let Some(sky) = tweaks.skybox.clone() {
        let (folder, cache) = (tweaks.skybox_folder.clone(), cache.to_path_buf());
        let faces = tokio::task::spawn_blocking(move || crate::core::skybox::files(&sky, folder.as_deref(), &cache))
            .await
            .map_err(|e| e.to_string())??;
        out.extend(faces);
    }
    Ok(out)
}

/// Puts `tweaks` into the build at `build`, first undoing whatever Pious
/// changed there before. With tweaks off, this only restores. `presets`
/// are the mod-preset files from [`preset_mods`]; `plugins` are the client
/// files enabled plugins bring (cursors, sounds…), which win over the
/// presets but not over the user's own mods folder.
pub fn apply(
    build: &Path,
    tweaks: &Tweaks,
    mods_dir: &Path,
    presets: Vec<(String, Vec<u8>)>,
    plugins: Vec<(String, Vec<u8>)>,
) -> Result<(), String> {
    restore(build)?;
    if !tweaks.enabled {
        crate::core::compat::apply(build, &[]);
        crate::core::gpupref::apply(build, None);
        return Ok(());
    }
    crate::core::compat::apply(build, &crate::core::compat::layers(tweaks));
    crate::core::gpupref::apply(build, tweaks.gpu.as_deref());

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();

    let flags = fast_flags(tweaks)?;
    if !flags.is_empty() {
        // Keep flags already in the file (a bootstrapper's), Pious's win.
        let mut merged = std::fs::read(build.join(FLAGS))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Map<String, Value>>(&bytes).ok())
            .unwrap_or_default();
        merged.extend(flags);
        let json = serde_json::to_vec_pretty(&Value::Object(merged)).map_err(|e| e.to_string())?;
        files.push((FLAGS.to_owned(), json));
    }

    // The replacement font: a downloaded preset, one of Roblox's own, or
    // the user's file.
    let (font_slot, presets): (Vec<_>, Vec<_>) = presets.into_iter().partition(|(target, _)| target == FONT_SLOT);
    let font = match (tweaks.font_preset.as_deref(), font_slot.into_iter().next()) {
        (Some(_), Some((_, bytes))) => Some(bytes),
        (Some(preset), None) => match preset.strip_prefix("roblox:") {
            Some(file) => Some(
                std::fs::read(build.join("content/fonts").join(file))
                    .map_err(|e| format!("Roblox's {file} font is missing from this version ({e})."))?,
            ),
            None => None,
        },
        (None, _) => match &tweaks.font {
            Some(font) => {
                Some(std::fs::read(font).map_err(|e| format!("Couldn't read the font {} ({e}).", font.display()))?)
            }
            None => None,
        },
    };
    if let Some(bytes) = font {
        for target in font_files(build) {
            files.push((target, bytes.clone()));
        }
    }

    files.extend(presets);

    // The shift lock cursor: drawn here, or the user's picture.
    match tweaks.shiftlock.as_deref() {
        Some("custom") => {
            if let Some(file) = &tweaks.shiftlock_file {
                let bytes = std::fs::read(file).map_err(|e| format!("Couldn't read the shift lock picture {} ({e}).", file.display()))?;
                files.push((crate::core::crosshair::TARGET.to_owned(), bytes));
            }
        }
        Some(preset) => files.push((
            crate::core::crosshair::TARGET.to_owned(),
            crate::core::crosshair::render(preset, tweaks.shiftlock_color.as_deref()),
        )),
        None => {}
    }

    // Plugins' client files (a cursor pack, a death sound…).
    files.extend(plugins.into_iter().filter(|(relative, _)| safe_relative(relative)));

    // The user's own mods go last, so they win over presets.
    if tweaks.use_mods_folder {
        for relative in walk(mods_dir) {
            let bytes = std::fs::read(mods_dir.join(&relative)).map_err(|e| e.to_string())?;
            files.push((relative, bytes));
        }
    }

    // The manifest is saved before each file is replaced, so even a
    // failure halfway leaves a record that restores every original.
    let mut manifest = Manifest::default();
    for (relative, bytes) in files {
        let target = build.join(&relative);
        let backup = build.join(BACKUP).join(&relative);
        if !manifest.files.contains_key(&relative) {
            let had_original = target.is_file();
            if had_original {
                create_parent(&backup)?;
                std::fs::copy(&target, &backup).map_err(|e| format!("Couldn't back up {relative} ({e})."))?;
            }
            manifest.files.insert(relative.clone(), had_original);
            save_manifest(build, &manifest)?;
        }
        create_parent(&target)?;
        unlock(&target);
        std::fs::write(&target, bytes).map_err(|e| format!("Couldn't write {relative} ({e}). Is that version running?"))?;
        // Read-only, like Fishstrap does, so Roblox doesn't put its own
        // file back over it.
        lock(&target);
    }
    Ok(())
}

fn save_manifest(build: &Path, manifest: &Manifest) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?;
    // Written next to it, then swapped in, so it's never half-written.
    let partial = build.join(format!("{MANIFEST}.part"));
    std::fs::write(&partial, json).map_err(|e| format!("Couldn't save the tweaks record ({e})."))?;
    std::fs::rename(&partial, build.join(MANIFEST)).map_err(|e| format!("Couldn't save the tweaks record ({e})."))
}

/// Undoes every change Pious made to a build.
pub fn restore(build: &Path) -> Result<(), String> {
    let Ok(bytes) = std::fs::read(build.join(MANIFEST)) else {
        return Ok(());
    };
    let manifest: Manifest = match serde_json::from_slice(&bytes) {
        Ok(manifest) => manifest,
        // The record is damaged (say, a crash while writing it): put back
        // every original that was backed up, and keep the backups if any
        // can't be restored.
        Err(_) => {
            let mut files = BTreeMap::new();
            for relative in walk(&build.join(BACKUP)) {
                files.insert(relative, true);
            }
            Manifest { files }
        }
    };
    for (relative, had_original) in &manifest.files {
        let target = build.join(relative);
        let backup = build.join(BACKUP).join(relative);
        if *had_original {
            // A backup that was never finished means the original wasn't
            // touched yet.
            if backup.is_file() {
                unlock(&target);
                std::fs::copy(&backup, &target)
                    .map_err(|e| format!("Couldn't restore {relative} ({e}). Is that version running?"))?;
            }
        } else {
            unlock(&target);
            let _ = std::fs::remove_file(&target);
        }
    }
    let _ = std::fs::remove_dir_all(build.join(BACKUP));
    let _ = std::fs::remove_file(build.join(MANIFEST));
    Ok(())
}

/// Whether Pious has tweaks applied in a build right now.
#[cfg(test)]
pub fn is_applied(build: &Path) -> bool {
    build.join(MANIFEST).is_file()
}

/// The file [`is_applied`] looks for, to check it without waiting on the
/// disk (see [`crate::core::fscache`]).
pub fn applied_marker(build: &Path) -> PathBuf {
    build.join(MANIFEST)
}

/// Roblox's interface fonts, except the emoji font (replacing it would
/// turn every emoji into boxes).
fn font_files(build: &Path) -> Vec<String> {
    let dir = build.join("content").join("fonts");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|name| {
            let lower = name.to_ascii_lowercase();
            (lower.ends_with(".ttf") || lower.ends_with(".otf")) && !lower.contains("emoji")
        })
        .map(|name| format!("content/fonts/{name}"))
        .collect()
}

/// Every file under `dir`, as `/`-separated paths relative to it.
fn walk(dir: &Path) -> Vec<String> {
    fn visit(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, out);
            } else if let Ok(relative) = path.strip_prefix(root) {
                let parts: Vec<String> = relative.iter().map(|p| p.to_string_lossy().into_owned()).collect();
                out.push(parts.join("/"));
            }
        }
    }
    let mut out = Vec::new();
    visit(dir, dir, &mut out);
    out.sort();
    out
}

/// Lets Pious change a file it made read-only.
fn unlock(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        let mut permissions = meta.permissions();
        if permissions.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            let _ = std::fs::set_permissions(path, permissions);
        }
    }
}

fn lock(path: &Path) {
    if let Ok(meta) = std::fs::metadata(path) {
        let mut permissions = meta.permissions();
        permissions.set_readonly(true);
        let _ = std::fs::set_permissions(path, permissions);
    }
}

/// A path that stays inside the build folder (no `..`, no drive or root).
pub fn safe_relative(relative: &str) -> bool {
    let path = Path::new(relative);
    !relative.is_empty() && path.components().all(|c| matches!(c, std::path::Component::Normal(_)))
}

fn create_parent(path: &Path) -> Result<(), String> {
    match path.parent() {
        Some(parent) => std::fs::create_dir_all(parent).map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Where the user keeps files to copy over the client, mirroring its
/// folder layout (e.g. `content/sounds/ouch.ogg`).
pub fn mods_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("Modifications")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("pious-tweaks-{name}-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn builds_fast_flags() {
        let mut tweaks = Tweaks {
            enabled: true,
            fps_limit: Some(240),
            graphics: GraphicsApi::Vulkan,
            custom_flags: r#"{"FFlagExample": "True", "DFIntTaskSchedulerTargetFps": 300}"#.into(),
            ..Tweaks::default()
        };
        tweaks.migrate();
        let flags = fast_flags(&tweaks).unwrap();
        // The profile overrides the preset.
        assert_eq!(flags["DFIntTaskSchedulerTargetFps"], "300");
        assert_eq!(flags["FFlagDebugGraphicsPreferVulkan"], "True");
        assert_eq!(flags["FFlagExample"], "True");
        assert!(tweaks.custom_flags.is_empty());
    }

    #[test]
    fn frame_rate_goes_to_robloxs_settings_not_flags() {
        let tweaks = Tweaks { enabled: true, fps_limit: Some(240), graphics_quality: Some(12), ..Tweaks::default() };
        let flags = fast_flags(&tweaks).unwrap();
        assert!(!flags.contains_key("DFIntTaskSchedulerTargetFps"));
        let values = settings_values(&tweaks);
        assert_eq!(values[0].0, "FramerateCap");
        assert_eq!(values[0].2, "240");
        assert_eq!(values[1].2, "10");
        assert!(settings_values(&Tweaks { enabled: false, ..tweaks }).is_empty());
    }

    #[test]
    fn applies_and_restores_exactly() {
        let build = scratch("build");
        let mods = scratch("mods");
        std::fs::create_dir_all(build.join("content/fonts")).unwrap();
        std::fs::write(build.join("content/fonts/arial.ttf"), b"original").unwrap();
        std::fs::write(build.join("content/fonts/TwemojiMozilla.ttf"), b"emoji").unwrap();
        std::fs::create_dir_all(mods.join("content/sounds")).unwrap();
        std::fs::write(mods.join("content/sounds/ouch.ogg"), b"oof").unwrap();
        let font = mods.join("font.ttf");
        std::fs::write(&font, b"custom").unwrap();

        let tweaks = Tweaks {
            enabled: true,
            msaa: 4,
            font: Some(font),
            use_mods_folder: true,
            ..Tweaks::default()
        };
        // The font file itself sits in the mods folder too; that's fine.
        apply(&build, &tweaks, &mods, Vec::new(), Vec::new()).unwrap();
        assert_eq!(std::fs::read(build.join("content/fonts/arial.ttf")).unwrap(), b"custom");
        assert_eq!(std::fs::read(build.join("content/fonts/TwemojiMozilla.ttf")).unwrap(), b"emoji");
        assert_eq!(std::fs::read(build.join("content/sounds/ouch.ogg")).unwrap(), b"oof");
        assert!(build.join(FLAGS).is_file());

        // Applying again (or with other settings) starts from the originals.
        apply(&build, &tweaks, &mods, vec![("content/textures/x.png".into(), b"preset".to_vec())], Vec::new()).unwrap();
        assert!(build.join("content/textures/x.png").is_file());
        restore(&build).unwrap();
        assert_eq!(std::fs::read(build.join("content/fonts/arial.ttf")).unwrap(), b"original");
        assert!(!build.join("content/sounds/ouch.ogg").exists());
        assert!(!build.join(FLAGS).exists());
        assert!(!is_applied(&build));

        let _ = std::fs::remove_dir_all(build);
        let _ = std::fs::remove_dir_all(mods);
    }

    #[test]
    fn defaults_are_on_and_change_nothing() {
        let tweaks = Tweaks::default();
        assert!(tweaks.enabled, "tweaks are on by default");
        assert!(tweaks.is_neutral());
        assert!(fast_flags(&tweaks).unwrap().is_empty());
        assert!(settings_values(&tweaks).is_empty());
        assert!(preset_files(&tweaks).is_empty());
        assert!(crate::core::compat::layers(&tweaks).is_empty());

        // Applying the defaults to a build writes nothing at all.
        let build = scratch("neutral");
        let mods = scratch("neutral-mods");
        std::fs::create_dir_all(build.join("content/fonts")).unwrap();
        std::fs::write(build.join("content/fonts/arial.ttf"), b"original").unwrap();
        apply(&build, &tweaks, &mods, Vec::new(), Vec::new()).unwrap();
        assert!(!is_applied(&build));
        assert!(!build.join(FLAGS).exists());
        assert_eq!(std::fs::read(build.join("content/fonts/arial.ttf")).unwrap(), b"original");
        let _ = std::fs::remove_dir_all(build);
        let _ = std::fs::remove_dir_all(mods);
    }

    #[test]
    fn reset_brings_every_tweak_back_but_keeps_profiles() {
        let id = uuid::Uuid::new_v4();
        let mut tweaks = Tweaks {
            enabled: true,
            fps_limit: Some(240),
            msaa: 4,
            gray_sky: true,
            shiftlock: Some("cod".into()),
            player_icon: Some("pious".into()),
            window_title: "Hi".into(),
            priority: Some("high".into()),
            flag_profiles: vec![crate::core::model::FlagProfile { id, name: "Mine".into(), flags: [("FFlagX".to_owned(), "True".to_owned())].into() }],
            active_profile: Some(id),
            ..Tweaks::default()
        };
        assert!(!tweaks.is_neutral());
        tweaks.reset();
        assert!(tweaks.is_neutral());
        assert!(tweaks.enabled);
        assert_eq!(tweaks.flag_profiles.len(), 1, "saved profiles are kept");
        assert!(tweaks.active_profile.is_none(), "but not used");
        assert!(fast_flags(&tweaks).unwrap().is_empty());
    }

    #[test]
    fn turning_tweaks_off_writes_nothing_and_restores() {
        let build = scratch("off");
        let mods = scratch("off-mods");
        std::fs::create_dir_all(build.join("content/textures")).unwrap();
        std::fs::write(build.join(crate::core::crosshair::TARGET), b"roblox's").unwrap();
        let on = Tweaks { enabled: true, shiftlock: Some("dot".into()), msaa: 2, ..Tweaks::default() };
        apply(&build, &on, &mods, Vec::new(), vec![("content/sounds/ouch.ogg".into(), b"plugin".to_vec())]).unwrap();
        assert_ne!(std::fs::read(build.join(crate::core::crosshair::TARGET)).unwrap(), b"roblox's");
        assert_eq!(std::fs::read(build.join("content/sounds/ouch.ogg")).unwrap(), b"plugin");

        let off = Tweaks { enabled: false, ..on };
        assert!(settings_values(&off).is_empty());
        apply(&build, &off, &mods, Vec::new(), vec![("content/sounds/ouch.ogg".into(), b"plugin".to_vec())]).unwrap();
        assert_eq!(std::fs::read(build.join(crate::core::crosshair::TARGET)).unwrap(), b"roblox's");
        assert!(!build.join("content/sounds/ouch.ogg").exists());
        assert!(!build.join(FLAGS).exists());
        let _ = std::fs::remove_dir_all(build);
        let _ = std::fs::remove_dir_all(mods);
    }

    #[test]
    fn plugin_files_cant_escape_the_build() {
        assert!(safe_relative("content/sounds/ouch.ogg"));
        assert!(!safe_relative("../../Windows/x.dll"));
        assert!(!safe_relative("C:/Windows/x.dll"));
        assert!(!safe_relative("/etc/x"));
        assert!(!safe_relative(""));
    }

    #[test]
    fn settings_record_round_trip() {
        let dir = scratch("record");
        let mut record = SettingsRecord::new();
        record.insert("FramerateCap".into(), (crate::core::robloxsettings::Kind::Int, Some("60".into())));
        save_record(&dir, &record).unwrap();
        assert_eq!(load_record(&dir).get("FramerateCap").and_then(|(_, v)| v.clone()).as_deref(), Some("60"));
        save_record(&dir, &SettingsRecord::new()).unwrap();
        assert!(!settings_record(&dir).exists(), "an empty record leaves no file");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn restores_originals_from_a_damaged_record() {
        let build = scratch("damaged");
        let mods = scratch("damaged-mods");
        std::fs::create_dir_all(build.join("content/fonts")).unwrap();
        std::fs::write(build.join("content/fonts/arial.ttf"), b"original").unwrap();
        let font = mods.join("font.ttf");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(&font, b"custom").unwrap();
        let tweaks = Tweaks { enabled: true, font: Some(font), ..Tweaks::default() };
        apply(&build, &tweaks, &mods, Vec::new(), Vec::new()).unwrap();
        // A crash halfway through writing the record.
        std::fs::write(build.join(MANIFEST), b"{\"files\": {\"conte").unwrap();
        restore(&build).unwrap();
        assert_eq!(std::fs::read(build.join("content/fonts/arial.ttf")).unwrap(), b"original");
        let _ = std::fs::remove_dir_all(build);
        let _ = std::fs::remove_dir_all(mods);
    }
}
