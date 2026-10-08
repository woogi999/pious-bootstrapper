//! Ready-made fonts and emoji sets for the font tweaks.
//!
//! Fonts come from three places: Roblox's own (already in every build, so
//! nothing is downloaded), game fonts (Minecraft, Pokémon) and Google
//! Fonts. Emoji sets are fonts that replace Roblox's TwemojiMozilla.ttf.
//! Roblox draws COLRv0 and SVG emoji fonts, not bitmap ones, so Apple's
//! emoji are turned into an SVG emoji font on this PC from Apple's pictures
//! (see [`apple_emoji_font`]).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use super::model::EmojiStyle;

/// A ready-made font: (ID, name, group, download URL). Roblox's own fonts
/// have no URL; their ID is `roblox:<file in content/fonts>`.
pub const PRESETS: &[(&str, &str, &str, &str)] = &[
    ("roblox:BuilderSans-Bold.otf", "Builder Sans (Roblox today)", "Roblox", ""),
    ("roblox:BuilderSans-Medium.otf", "Builder Sans Medium", "Roblox", ""),
    ("roblox:SourceSansPro-Regular.ttf", "Source Sans Pro (classic Roblox)", "Roblox", ""),
    ("roblox:SourceSansPro-Bold.ttf", "Source Sans Pro Bold", "Roblox", ""),
    ("roblox:Montserrat-Bold.ttf", "Gotham style (Montserrat)", "Roblox", ""),
    ("roblox:Arimo-Regular.ttf", "Arial (Arimo)", "Roblox", ""),
    ("roblox:FredokaOne-Regular.ttf", "Fredoka One", "Roblox", ""),
    ("roblox:LuckiestGuy-Regular.ttf", "Luckiest Guy", "Roblox", ""),
    ("roblox:Bangers-Regular.ttf", "Bangers (comic)", "Roblox", ""),
    ("roblox:PressStart2P-Regular.ttf", "Press Start 2P (arcade)", "Roblox", ""),
    ("roblox:ComicNeue-Angular-Bold.ttf", "Comic Neue", "Roblox", ""),
    ("roblox:Oswald-Bold.ttf", "Oswald (tall)", "Roblox", ""),
    ("roblox:RobotoMono-Regular.ttf", "Roboto Mono", "Roblox", ""),
    ("roblox:SpecialElite-Regular.ttf", "Typewriter (Special Elite)", "Roblox", ""),
    ("roblox:Michroma-Regular.ttf", "Sci-fi (Michroma)", "Roblox", ""),
    ("roblox:Creepster-Regular.ttf", "Creepster (spooky)", "Roblox", ""),
    ("roblox:PermanentMarker-Regular.ttf", "Permanent Marker", "Roblox", ""),
    ("roblox:IndieFlower-Regular.ttf", "Handwriting (Indie Flower)", "Roblox", ""),
    ("roblox:GrenzeGotisch-Regular.ttf", "Gothic (Grenze)", "Roblox", ""),
    (
        "minecraft",
        "Minecraft",
        "Games",
        "https://cdn.jsdelivr.net/gh/idreesinc/minecraft-font/Minecraft.otf",
    ),
    (
        "monocraft",
        "Monocraft (Minecraft, monospaced)",
        "Games",
        "https://cdn.jsdelivr.net/gh/IdreesInc/Monocraft/dist/Monocraft-otf/Monocraft.otf",
    ),
    (
        "pokemon-ds",
        "Pokémon DS (Gen 4–5)",
        "Games",
        "https://cdn.jsdelivr.net/gh/Maruno17/pokemon-essentials/Fonts/power%20clear.ttf",
    ),
    (
        "pokemon-gba",
        "Pokémon GBA (Gen 3)",
        "Games",
        "https://cdn.jsdelivr.net/gh/Maruno17/pokemon-essentials/Fonts/power%20green.ttf",
    ),
    (
        "pokemon-gb",
        "Pokémon Game Boy (Gen 1)",
        "Games",
        "https://cdn.jsdelivr.net/gh/Maruno17/pokemon-essentials/Fonts/power%20red%20and%20blue.ttf",
    ),
    (
        "silkscreen",
        "Silkscreen (pixel)",
        "Fun",
        "https://raw.githubusercontent.com/google/fonts/main/ofl/silkscreen/Silkscreen-Regular.ttf",
    ),
    ("vt323", "VT323 (terminal)", "Fun", "https://raw.githubusercontent.com/google/fonts/main/ofl/vt323/VT323-Regular.ttf"),
    (
        "jersey10",
        "Jersey 10 (sports pixel)",
        "Fun",
        "https://raw.githubusercontent.com/google/fonts/main/ofl/jersey10/Jersey10-Regular.ttf",
    ),
    ("pacifico", "Pacifico (script)", "Fun", "https://raw.githubusercontent.com/google/fonts/main/ofl/pacifico/Pacifico-Regular.ttf"),
    (
        "bubblegum",
        "Bubblegum Sans",
        "Fun",
        "https://raw.githubusercontent.com/google/fonts/main/ofl/bubblegumsans/BubblegumSans-Regular.ttf",
    ),
    (
        "rubikmono",
        "Rubik Mono One (chunky)",
        "Fun",
        "https://raw.githubusercontent.com/google/fonts/main/ofl/rubikmonoone/RubikMonoOne-Regular.ttf",
    ),
];

#[derive(Debug, Clone, Serialize)]
pub struct PresetInfo {
    pub id: &'static str,
    pub label: &'static str,
    pub group: &'static str,
}

pub fn presets() -> Vec<PresetInfo> {
    PRESETS.iter().map(|(id, label, group, _)| PresetInfo { id, label, group }).collect()
}

/// The download URL of a preset font (none for Roblox's own fonts).
pub fn preset_url(id: &str) -> Option<&'static str> {
    PRESETS.iter().find(|(p, ..)| *p == id).map(|(.., url)| *url).filter(|u| !u.is_empty())
}

const EMOJI_FONTS: &str = "https://cdn.jsdelivr.net/gh/niksavc/rbxcustom-fontemojis";

/// Where an emoji set's font comes from. `None` for the default and for
/// Apple's (built locally).
pub fn emoji_url(style: EmojiStyle) -> Option<String> {
    let file = match style {
        EmojiStyle::Default | EmojiStyle::Apple => return None,
        EmojiStyle::Windows11Fluent => "Win1123H2SegoeUIEmoji.ttf",
        EmojiStyle::Windows11 => "Win1122H2SegoeUIEmoji.ttf",
        EmojiStyle::Windows11Original => "Win11SegoeUIEmoji.ttf",
        EmojiStyle::Windows10 => "Win10April2018SegoeUIEmoji.ttf",
        EmojiStyle::Windows10Anniversary => "Win10AUpdSegoeUIEmoji.ttf",
        EmojiStyle::Windows8 => "Win8.1SegoeUIEmoji.ttf",
        EmojiStyle::TwemojiSvg => "TwitterColorEmoji-SVGinOT.ttf",
        EmojiStyle::EmojiOne => "EmojiOneMozilla.ttf",
        EmojiStyle::Catmoji => "Catmoji.ttf",
        EmojiStyle::NotoMono => "NotoEmoji-16.0.ttf",
        EmojiStyle::OpenMojiMono => "OpenMoji-black-glyf-16.0.ttf",
        EmojiStyle::Mona12 => "Mona12ColorEmoji-17.0.ttf",
        EmojiStyle::Docomo => "og-dcm-emoji.ttf",
    };
    Some(format!("{EMOJI_FONTS}/{file}"))
}

/// Downloads `url` once, then reads it from `cache`.
pub async fn cached_download(url: &str, cache: &Path, name: &str) -> Result<Vec<u8>, String> {
    let path = cache.join(name);
    if let Ok(bytes) = tokio::fs::read(&path).await {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }
    let response = super::roblox::http()
        .get(url)
        .timeout(std::time::Duration::from_secs(300))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|e| format!("Couldn't download {name} ({e})."))?;
    let bytes = response.bytes().await.map_err(|e| e.to_string())?.to_vec();
    let _ = tokio::fs::create_dir_all(cache).await;
    let _ = tokio::fs::write(&path, &bytes).await;
    Ok(bytes)
}

// ── Apple emoji ──────────────────────────────────────────────────────────

const EMOJI_DATA: &str = "https://cdn.jsdelivr.net/npm/emoji-datasource@15.1.2/emoji.json";
const APPLE_SHEET: &str = "https://cdn.jsdelivr.net/npm/emoji-datasource-apple@15.1.2/img/apple/sheets/64.png";
/// The sheet's cells: 64px pictures with a 1px border.
const CELL: u32 = 66;

/// Builds an emoji font that looks like Apple's: Roblox's own emoji font
/// (`base`) with every emoji it has redrawn from Apple's pictures, stored
/// as SVG glyphs. Cached after the first build.
pub async fn apple_emoji_font(base: Vec<u8>, cache: &Path) -> Result<Vec<u8>, String> {
    let built = cache.join("AppleEmoji-built.ttf");
    if let Ok(bytes) = tokio::fs::read(&built).await {
        if bytes.len() > 1_000_000 {
            return Ok(bytes);
        }
    }
    let data = cached_download(EMOJI_DATA, cache, "emoji-datasource-15.1.2.json").await?;
    let sheet = cached_download(APPLE_SHEET, cache, "emoji-apple-sheet-64.png").await?;
    let font = tokio::task::spawn_blocking(move || build_svg_font(&base, &data, &sheet))
        .await
        .map_err(|e| e.to_string())??;
    let _ = tokio::fs::write(&built, &font).await;
    Ok(font)
}

fn build_svg_font(base: &[u8], data: &[u8], sheet: &[u8]) -> Result<Vec<u8>, String> {
    use base64::Engine;

    let entries: Vec<serde_json::Value> = serde_json::from_slice(data).map_err(|e| format!("Bad emoji list ({e})."))?;
    let sheet = image::load_from_memory(sheet).map_err(|e| format!("Bad emoji pictures ({e})."))?.to_rgba8();
    let face = rustybuzz::Face::from_slice(base, 0).ok_or("Roblox's emoji font couldn't be read.")?;

    // Each emoji sequence → the glyph Roblox's font draws for it.
    let mut pictures: BTreeMap<u16, (u32, u32)> = BTreeMap::new();
    let mut add = |unified: &str, x: u64, y: u64| {
        let text: String = unified
            .split('-')
            .filter_map(|h| u32::from_str_radix(h, 16).ok())
            .filter_map(char::from_u32)
            .collect();
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(&text);
        let shaped = rustybuzz::shape(&face, &[], buffer);
        let glyphs: Vec<u16> = shaped.glyph_infos().iter().map(|g| g.glyph_id as u16).filter(|g| *g != 0).collect();
        // A sequence that shapes to one glyph, plus an optional VS16.
        if let Some(&glyph) = glyphs.first() {
            if glyphs.len() == 1 || (glyphs.len() == 2 && text.ends_with('\u{FE0F}')) {
                pictures.entry(glyph).or_insert((x as u32, y as u32));
            }
        }
    };
    for entry in &entries {
        let has = |e: &serde_json::Value| e["has_img_apple"].as_bool().unwrap_or(false);
        let pos = |e: &serde_json::Value| (e["sheet_x"].as_u64().unwrap_or(0), e["sheet_y"].as_u64().unwrap_or(0));
        if has(entry) {
            let (x, y) = pos(entry);
            for key in ["unified", "non_qualified"] {
                if let Some(u) = entry[key].as_str() {
                    add(u, x, y);
                }
            }
        }
        if let Some(skins) = entry["skin_variations"].as_object() {
            for skin in skins.values() {
                if has(skin) {
                    let (x, y) = pos(skin);
                    if let Some(u) = skin["unified"].as_str() {
                        add(u, x, y);
                    }
                }
            }
        }
    }
    if pictures.len() < 500 {
        return Err("Roblox's emoji font didn't match Apple's emoji.".into());
    }

    // One SVG document per glyph, sized to the glyph Roblox already has.
    let ttf = ttf_parser::Face::parse(base, 0).map_err(|e| e.to_string())?;
    let mut docs: Vec<(u16, Vec<u8>)> = Vec::with_capacity(pictures.len());
    for (glyph, (sx, sy)) in pictures {
        let cell = image::imageops::crop_imm(&sheet, sx * CELL + 1, sy * CELL + 1, 64, 64).to_image();
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(cell)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
        let bbox = ttf.glyph_bounding_box(ttf_parser::GlyphId(glyph));
        let advance = ttf.glyph_hor_advance(ttf_parser::GlyphId(glyph)).unwrap_or(ttf.units_per_em()) as i32;
        let (x, top, size) = match bbox {
            Some(b) => {
                let w = (b.x_max - b.x_min) as i32;
                let h = (b.y_max - b.y_min) as i32;
                let size = w.max(h);
                (b.x_min as i32 + (w - size) / 2, b.y_max as i32 + (size - h) / 2, size)
            }
            None => (0, ttf.ascender() as i32, advance),
        };
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><g id="glyph{glyph}"><image x="{x}" y="{}" width="{size}" height="{size}" xlink:href="data:image/png;base64,{b64}"/></g></svg>"#,
            -top
        );
        docs.push((glyph, svg.into_bytes()));
    }

    // The SVG table.
    let mut list = Vec::new();
    list.extend((docs.len() as u16).to_be_bytes());
    let mut offset = 2 + docs.len() as u32 * 12;
    for (glyph, doc) in &docs {
        list.extend(glyph.to_be_bytes());
        list.extend(glyph.to_be_bytes());
        list.extend(offset.to_be_bytes());
        list.extend((doc.len() as u32).to_be_bytes());
        offset += doc.len() as u32;
    }
    for (_, doc) in &docs {
        list.extend(doc);
    }
    let mut svg_table = Vec::with_capacity(10 + list.len());
    svg_table.extend(0u16.to_be_bytes());
    svg_table.extend(10u32.to_be_bytes());
    svg_table.extend(0u32.to_be_bytes());
    svg_table.extend(list);

    // Roblox prefers color layers when a font has them, so they go; the
    // signature would no longer match, so it goes too.
    rebuild_font(base, &[*b"COLR", *b"CPAL", *b"DSIG"], (*b"SVG ", svg_table))
}

/// Rewrites an OpenType font without `drop`'s tables and with `extra`.
fn rebuild_font(base: &[u8], drop: &[[u8; 4]], extra: ([u8; 4], Vec<u8>)) -> Result<Vec<u8>, String> {
    let be16 = |o: usize| -> Result<u16, String> {
        base.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]])).ok_or_else(|| "Bad font".to_owned())
    };
    let be32 = |o: usize| -> Result<u32, String> {
        base.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]])).ok_or_else(|| "Bad font".to_owned())
    };
    let count = be16(4)? as usize;
    let mut tables: Vec<([u8; 4], Vec<u8>)> = Vec::new();
    for i in 0..count {
        let record = 12 + i * 16;
        let tag: [u8; 4] = base.get(record..record + 4).ok_or("Bad font")?.try_into().unwrap();
        if drop.contains(&tag) || tag == extra.0 {
            continue;
        }
        let offset = be32(record + 8)? as usize;
        let length = be32(record + 12)? as usize;
        let data = base.get(offset..offset + length).ok_or("Bad font")?.to_vec();
        tables.push((tag, data));
    }
    tables.push(extra);
    tables.sort_by(|a, b| a.0.cmp(&b.0));

    let checksum = |data: &[u8]| -> u32 {
        data.chunks(4).fold(0u32, |sum, c| {
            let mut word = [0u8; 4];
            word[..c.len()].copy_from_slice(c);
            sum.wrapping_add(u32::from_be_bytes(word))
        })
    };

    let n = tables.len() as u16;
    let mut power = 1u16;
    let mut log = 0u16;
    while power * 2 <= n {
        power *= 2;
        log += 1;
    }
    let mut out = Vec::new();
    out.extend(base[0..4].iter());
    out.extend(n.to_be_bytes());
    out.extend((power * 16).to_be_bytes());
    out.extend(log.to_be_bytes());
    out.extend((n * 16 - power * 16).to_be_bytes());
    let mut offset = 12 + tables.len() * 16;
    let mut head_at = None;
    for (tag, data) in &tables {
        if tag == b"head" {
            head_at = Some(offset);
        }
        let mut data = data.clone();
        if tag == b"head" && data.len() >= 12 {
            data[8..12].copy_from_slice(&[0, 0, 0, 0]);
        }
        out.extend(tag);
        out.extend(checksum(&data).to_be_bytes());
        out.extend((offset as u32).to_be_bytes());
        out.extend((data.len() as u32).to_be_bytes());
        offset += data.len().div_ceil(4) * 4;
    }
    for (tag, data) in &tables {
        let start = out.len();
        out.extend(data);
        if tag == b"head" && data.len() >= 12 {
            out[start + 8..start + 12].copy_from_slice(&[0, 0, 0, 0]);
        }
        while out.len() % 4 != 0 {
            out.push(0);
        }
    }
    if let Some(head) = head_at {
        let adjust = 0xB1B0_AFBAu32.wrapping_sub(checksum(&out));
        out[head + 8..head + 12].copy_from_slice(&adjust.to_be_bytes());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_emoji_style_but_two_downloads() {
        assert!(emoji_url(EmojiStyle::Default).is_none());
        assert!(emoji_url(EmojiStyle::Apple).is_none());
        assert!(emoji_url(EmojiStyle::Catmoji).unwrap().ends_with("Catmoji.ttf"));
    }

    /// Builds the Apple emoji font from a real Roblox emoji font:
    /// `PIOUS_EMOJI_BASE=<path to TwemojiMozilla.ttf> cargo test apple -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn builds_apple_emoji() {
        let base = std::fs::read(std::env::var("PIOUS_EMOJI_BASE").unwrap()).unwrap();
        let cache = std::env::temp_dir().join("pious-emoji-test");
        let _ = std::fs::remove_file(cache.join("AppleEmoji-built.ttf"));
        let font = apple_emoji_font(base, &cache).await.unwrap();
        let face = ttf_parser::Face::parse(&font, 0).unwrap();
        assert!(face.tables().svg.is_some());
        assert!(face.tables().colr.is_none());
        println!("built {} bytes, {} glyphs", font.len(), face.number_of_glyphs());
    }

    #[test]
    fn presets_are_unique() {
        let mut ids: Vec<_> = PRESETS.iter().map(|p| p.0).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), PRESETS.len());
    }
}
