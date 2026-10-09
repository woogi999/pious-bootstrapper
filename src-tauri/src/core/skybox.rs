//! Custom skies. Roblox's default sky is six pictures in
//! `PlatformContent/pc/textures/sky/sky512_{bk,dn,ft,lf,rt,up}.tex`, which
//! are DDS files (DXT1 with mipmaps). The presets are drawn here; a custom
//! sky is a folder with six pictures (or ready-made .tex/.dds files). Only
//! games that don't set their own sky show it.

use std::path::{Path, PathBuf};

use image::{imageops::FilterType, RgbaImage};

/// The six faces, as Roblox names them.
pub const FACES: [&str; 6] = ["bk", "dn", "ft", "lf", "rt", "up"];

/// Where a face goes in a build.
pub fn target(face: &str) -> String {
    format!("PlatformContent/pc/textures/sky/sky512_{face}.tex")
}

/// The presets: ID, name.
pub const PRESETS: [(&str, &str); 7] = [
    ("clear", "Clear day"),
    ("sunset", "Sunset"),
    ("dusk", "Dusk"),
    ("night", "Starry night"),
    ("overcast", "Overcast"),
    ("pastel", "Pastel"),
    ("void", "Void"),
];

/// Bump when the drawings change, so cached faces are redrawn.
const DRAWING: u32 = 1;
const SIZE: u32 = 512;

/// Colors from the ground (elevation -1) to straight up (+1), and whether
/// there are stars.
fn palette(id: &str) -> Option<(&'static [(f32, [u8; 3])], bool)> {
    Some(match id {
        "clear" => (&[(-1.0, [86, 96, 104]), (0.0, [205, 226, 245]), (0.25, [120, 175, 235]), (1.0, [48, 110, 210])], false),
        "sunset" => (&[(-1.0, [48, 30, 40]), (0.0, [255, 150, 70]), (0.12, [240, 96, 96]), (0.4, [128, 64, 140]), (1.0, [34, 32, 92])], false),
        "dusk" => (&[(-1.0, [20, 16, 34]), (0.0, [150, 98, 170]), (0.3, [72, 56, 140]), (1.0, [18, 18, 58])], true),
        "night" => (&[(-1.0, [6, 7, 14]), (0.0, [26, 34, 70]), (0.35, [10, 14, 36]), (1.0, [3, 4, 12])], true),
        "overcast" => (&[(-1.0, [70, 72, 76]), (0.0, [178, 182, 188]), (1.0, [130, 136, 146])], false),
        "pastel" => (&[(-1.0, [200, 190, 230]), (0.0, [255, 214, 232]), (0.35, [205, 210, 255]), (1.0, [160, 205, 255])], false),
        "void" => (&[(-1.0, [0, 0, 0]), (1.0, [0, 0, 0])], false),
        _ => return None,
    })
}

/// The direction a face's pixel looks in (x right, y up, z ahead).
fn direction(face: &str, u: f32, v: f32) -> [f32; 3] {
    // u, v in -1..1, v down the picture.
    match face {
        "up" => [u, 1.0, v],
        "dn" => [u, -1.0, -v],
        _ => [u, -v, 1.0],
    }
}

fn shade(stops: &[(f32, [u8; 3])], e: f32) -> [u8; 3] {
    let e = e.clamp(-1.0, 1.0);
    let mut lower = stops[0];
    for &stop in stops {
        if stop.0 <= e {
            lower = stop;
        } else {
            let t = ((e - lower.0) / (stop.0 - lower.0)).clamp(0.0, 1.0);
            let t = t * t * (3.0 - 2.0 * t);
            let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
            return [mix(lower.1[0], stop.1[0]), mix(lower.1[1], stop.1[1]), mix(lower.1[2], stop.1[2])];
        }
    }
    lower.1
}

/// A small, fast hash for star placement (same sky every time).
fn hash(face: usize, x: u32, y: u32) -> u32 {
    let mut h = (face as u32).wrapping_mul(0x9E37_79B9) ^ x.wrapping_mul(0x85EB_CA6B) ^ y.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h
}

/// One face of a preset, `size` pixels square.
pub fn draw(id: &str, face: &str, size: u32) -> Option<RgbaImage> {
    let (stops, stars) = palette(id)?;
    let index = FACES.iter().position(|f| *f == face)?;
    let mut picture = RgbaImage::new(size, size);
    for (x, y, pixel) in picture.enumerate_pixels_mut() {
        let u = (x as f32 + 0.5) / size as f32 * 2.0 - 1.0;
        let v = (y as f32 + 0.5) / size as f32 * 2.0 - 1.0;
        let d = direction(face, u, v);
        let elevation = d[1] / (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let mut c = shade(stops, elevation);
        if stars && elevation > 0.02 {
            let h = hash(index, x, y);
            if h % 1400 == 0 {
                let glow = 150 + (h >> 24) as u16 % 106;
                let fade = ((elevation - 0.02) * 6.0).min(1.0);
                for channel in &mut c {
                    *channel = (*channel as f32 + (glow as f32 - *channel as f32).max(0.0) * fade) as u8;
                }
            }
        }
        *pixel = image::Rgba([c[0], c[1], c[2], 255]);
    }
    Some(picture)
}

/// A picture as a DDS file Roblox reads: DXT1 with every mipmap.
pub fn encode(picture: &RgbaImage) -> Vec<u8> {
    let (width, height) = picture.dimensions();
    let mut levels = vec![picture.clone()];
    while let Some(last) = levels.last() {
        let (w, h) = last.dimensions();
        if w == 1 && h == 1 {
            break;
        }
        let next = image::imageops::resize(last, (w / 2).max(1), (h / 2).max(1), FilterType::Triangle);
        levels.push(next);
    }
    let format = texpresso::Format::Bc1;
    let top = format.compressed_size(width as usize, height as usize);

    let mut out = Vec::with_capacity(128 + top * 2);
    let put = |v: u32, out: &mut Vec<u8>| out.extend_from_slice(&v.to_le_bytes());
    out.extend_from_slice(b"DDS ");
    put(124, &mut out);
    // CAPS | HEIGHT | WIDTH | PIXELFORMAT | MIPMAPCOUNT | LINEARSIZE
    put(0x000A_1007, &mut out);
    put(height, &mut out);
    put(width, &mut out);
    put(top as u32, &mut out);
    put(0, &mut out); // depth
    put(levels.len() as u32, &mut out);
    for _ in 0..11 {
        put(0, &mut out);
    }
    // Pixel format: FOURCC "DXT1".
    put(32, &mut out);
    put(0x4, &mut out);
    out.extend_from_slice(b"DXT1");
    for _ in 0..5 {
        put(0, &mut out);
    }
    // COMPLEX | TEXTURE | MIPMAP
    put(0x0040_1008, &mut out);
    for _ in 0..4 {
        put(0, &mut out);
    }
    for level in &levels {
        let (w, h) = level.dimensions();
        let mut block = vec![0u8; format.compressed_size(w as usize, h as usize)];
        format.compress(level.as_raw(), w as usize, h as usize, texpresso::Params::default(), &mut block);
        out.extend_from_slice(&block);
    }
    out
}

/// The six files for `tweaks.skybox` as (target, bytes), or none for
/// Roblox's own sky. Presets are drawn once and kept in `cache`.
pub fn files(id: &str, folder: Option<&Path>, cache: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    if id == "custom" {
        let folder = folder.ok_or("Choose the folder with your sky's pictures.")?;
        return custom(folder);
    }
    if palette(id).is_none() {
        return Ok(Vec::new());
    }
    let dir = cache.join("skybox");
    let _ = std::fs::create_dir_all(&dir);
    let mut out = Vec::new();
    for face in FACES {
        let cached = dir.join(format!("{id}-{DRAWING}-{face}.tex"));
        let bytes = match std::fs::read(&cached) {
            Ok(bytes) if bytes.starts_with(b"DDS ") => bytes,
            _ => {
                let bytes = encode(&draw(id, face, SIZE).ok_or("Unknown sky.")?);
                let _ = std::fs::write(&cached, &bytes);
                bytes
            }
        };
        out.push((target(face), bytes));
    }
    Ok(out)
}

/// Which face a file in a custom sky folder is, by its name: Roblox's
/// (`sky512_bk`, `bk`) or plain words (`back`, `top`…).
fn face_of(path: &Path) -> Option<&'static str> {
    let stem = path.file_stem()?.to_str()?.to_ascii_lowercase();
    let words: Vec<&str> = stem.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let last = *words.last()?;
    let by_word = |w: &str| -> Option<&'static str> {
        Some(match w {
            "bk" | "back" | "posz" | "pz" => "bk",
            "ft" | "front" | "negz" | "nz" => "ft",
            "lf" | "left" | "negx" | "nx" => "lf",
            "rt" | "right" | "posx" | "px" => "rt",
            "up" | "top" | "posy" | "py" => "up",
            "dn" | "down" | "bottom" | "negy" | "ny" => "dn",
            _ => return None,
        })
    };
    by_word(last).or_else(|| words.iter().rev().find_map(|w| by_word(w)))
}

fn custom(folder: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let entries = std::fs::read_dir(folder).map_err(|e| format!("Couldn't open the sky folder {} ({e}).", folder.display()))?;
    let mut found: std::collections::BTreeMap<&str, PathBuf> = Default::default();
    for path in entries.flatten().map(|e| e.path()).filter(|p| p.is_file()) {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "tex" | "dds") {
            continue;
        }
        if let Some(face) = face_of(&path) {
            found.entry(face).or_insert(path);
        }
    }
    let missing: Vec<&str> = FACES.iter().copied().filter(|f| !found.contains_key(f)).collect();
    if !missing.is_empty() {
        return Err(format!(
            "The sky folder needs six pictures named for their side (back, front, left, right, up, down, or bk, ft, lf, rt, up, dn). Missing: {}.",
            missing.join(", ")
        ));
    }
    let mut out = Vec::new();
    for face in FACES {
        let path = &found[face];
        let raw = std::fs::read(path).map_err(|e| format!("Couldn't read {} ({e}).", path.display()))?;
        let bytes = if raw.starts_with(b"DDS ") {
            raw
        } else {
            let picture = image::load_from_memory(&raw).map_err(|e| format!("{} isn't a picture Pious can read ({e}).", path.display()))?.to_rgba8();
            // Square, a power of two, at most 1024 (like Roblox's own).
            let side = picture.width().min(picture.height()).clamp(4, 1024);
            let side = 1u32 << (31 - side.leading_zeros());
            encode(&image::imageops::resize(&picture, side, side, FilterType::Lanczos3))
        };
        out.push((target(face), bytes));
    }
    Ok(out)
}

/// A preset's picture for the settings: its front face, small.
pub fn preview_png(id: &str) -> Option<Vec<u8>> {
    let picture = draw(id, "ft", 96)?;
    let mut png = Vec::new();
    image::DynamicImage::ImageRgba8(picture).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    Some(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_dds_like_roblox() {
        let bytes = encode(&draw("sunset", "ft", 64).unwrap());
        assert_eq!(&bytes[0..4], b"DDS ");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 124);
        assert_eq!(&bytes[84..88], b"DXT1");
        // 64 → 1: seven levels, each at least one 8-byte block.
        assert_eq!(u32::from_le_bytes(bytes[28..32].try_into().unwrap()), 7);
        let blocks: usize = [64usize, 32, 16, 8, 4, 2, 1].iter().map(|s| (s.div_ceil(4)).pow(2) * 8).sum();
        assert_eq!(bytes.len(), 128 + blocks);
    }

    #[test]
    fn every_preset_draws_every_face() {
        for (id, _) in PRESETS {
            for face in FACES {
                assert!(draw(id, face, 8).is_some(), "{id} {face}");
            }
            assert!(preview_png(id).is_some());
        }
        assert!(draw("nope", "ft", 8).is_none());
    }

    #[test]
    fn the_sky_is_lighter_above_the_horizon_by_day() {
        let face = draw("clear", "ft", 32).unwrap();
        let top = face.get_pixel(16, 0).0;
        let below = face.get_pixel(16, 31).0;
        assert!(top[2] > below[2]);
    }

    #[test]
    fn names_custom_faces() {
        for (name, face) in [("sky512_bk.tex", "bk"), ("Top.png", "up"), ("my sky - left.jpg", "lf"), ("bottom.png", "dn"), ("posx.png", "rt")] {
            assert_eq!(face_of(Path::new(name)), Some(face), "{name}");
        }
        assert_eq!(face_of(Path::new("readme.png")), None);
    }

    #[test]
    fn custom_folder_needs_all_six() {
        let dir = std::env::temp_dir().join(format!("pious-sky-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let picture = draw("night", "ft", 16).unwrap();
        for face in ["back", "front", "left", "right", "top"] {
            picture.save(dir.join(format!("{face}.png"))).unwrap();
        }
        assert!(files("custom", Some(&dir), &dir).unwrap_err().contains("dn"));
        picture.save(dir.join("down.png")).unwrap();
        let out = files("custom", Some(&dir), &dir).unwrap();
        assert_eq!(out.len(), 6);
        assert!(out.iter().all(|(target, bytes)| target.starts_with("PlatformContent/pc/textures/sky/sky512_") && bytes.starts_with(b"DDS ")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
