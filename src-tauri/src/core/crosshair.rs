//! Shift lock cursors: the crosshair Roblox shows while shift lock is on
//! (`content/textures/MouseLockedCursor.png`). The presets are drawn here,
//! in the spirit of other games' crosshairs, so nothing is downloaded.

use image::{ImageFormat, Rgba, RgbaImage};

/// Where Roblox reads the shift lock cursor from.
pub const TARGET: &str = "content/textures/MouseLockedCursor.png";

/// The presets: ID, name.
pub const PRESETS: [(&str, &str); 9] = [
    ("minecraft", "Blocky plus (Minecraft)"),
    ("cod", "Gap lines (Call of Duty)"),
    ("battlefield", "Dot and ticks (Battlefield)"),
    ("csgo", "Classic green (CS:GO)"),
    ("valorant", "Dot and lines (Valorant)"),
    ("fortnite", "Ring and ticks (Fortnite)"),
    ("classic", "Ring and dot (old Roblox)"),
    ("dot", "Just a dot"),
    ("circle", "Hollow circle"),
];

/// The color a preset is drawn in unless another is picked.
fn default_color(preset: &str) -> [u8; 3] {
    match preset {
        "csgo" => [50, 255, 50],
        "valorant" => [0, 255, 220],
        _ => [255, 255, 255],
    }
}

const SIZE: u32 = 64;
/// Drawn this many times bigger, then shrunk: smooth edges.
const SCALE: u32 = 4;

/// Shapes in a 64×64 space, centered on (32, 32).
enum Shape {
    /// A rectangle: left, top, width, height.
    Rect(f32, f32, f32, f32),
    /// A ring: radius, thickness.
    Ring(f32, f32),
    /// A dot: radius.
    Dot(f32),
}

fn shapes(preset: &str) -> Vec<Shape> {
    use Shape::*;
    let c = 32.0;
    // Four lines around the middle: length, thickness, gap from the middle.
    let cross = |len: f32, thick: f32, gap: f32| {
        vec![
            Rect(c - thick / 2.0, c - gap - len, thick, len),
            Rect(c - thick / 2.0, c + gap, thick, len),
            Rect(c - gap - len, c - thick / 2.0, len, thick),
            Rect(c + gap, c - thick / 2.0, len, thick),
        ]
    };
    match preset {
        "minecraft" => vec![Rect(c - 2.0, c - 11.0, 4.0, 22.0), Rect(c - 11.0, c - 2.0, 22.0, 4.0)],
        "cod" => cross(9.0, 2.5, 6.0),
        "battlefield" => vec![
            Dot(2.2),
            Rect(c - 14.0, c - 1.0, 8.0, 2.0),
            Rect(c + 6.0, c - 1.0, 8.0, 2.0),
            Rect(c - 1.0, c + 6.0, 2.0, 8.0),
        ],
        "csgo" => cross(8.0, 2.0, 3.0),
        "valorant" => {
            let mut s = cross(6.0, 2.0, 5.0);
            s.push(Dot(1.6));
            s
        }
        "fortnite" => {
            let mut s = vec![Ring(9.0, 1.6)];
            s.extend(cross(5.0, 2.0, 10.5));
            s
        }
        "classic" => vec![Ring(10.0, 2.0), Dot(2.0)],
        "dot" => vec![Dot(3.0)],
        "circle" => vec![Ring(7.0, 2.0)],
        _ => cross(8.0, 2.0, 4.0),
    }
}

fn inside(shape: &Shape, x: f32, y: f32) -> bool {
    match *shape {
        Shape::Rect(l, t, w, h) => x >= l && x < l + w && y >= t && y < t + h,
        Shape::Ring(r, thick) => {
            let d = ((x - 32.0).powi(2) + (y - 32.0).powi(2)).sqrt();
            d >= r - thick / 2.0 && d <= r + thick / 2.0
        }
        Shape::Dot(r) => ((x - 32.0).powi(2) + (y - 32.0).powi(2)).sqrt() <= r,
    }
}

/// Parses `#RRGGBB`.
fn parse(color: &str) -> Option<[u8; 3]> {
    let hex = color.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// Draws a preset as a PNG, in `color` (`#RRGGBB`) or its own. A thin dark
/// outline keeps it visible on bright scenes.
pub fn render(preset: &str, color: Option<&str>) -> Vec<u8> {
    let [r, g, b] = color.and_then(parse).unwrap_or_else(|| default_color(preset));
    let shapes = shapes(preset);
    let big = SIZE * SCALE;
    // Coverage of the shape and of its outline, at the big size.
    let mut fill = vec![false; (big * big) as usize];
    for py in 0..big {
        for px in 0..big {
            let (x, y) = ((px as f32 + 0.5) / SCALE as f32, (py as f32 + 0.5) / SCALE as f32);
            fill[(py * big + px) as usize] = shapes.iter().any(|s| inside(s, x, y));
        }
    }
    let outline = (1.2 * SCALE as f32) as i32;
    let near = |px: i32, py: i32| {
        for dy in -outline..=outline {
            for dx in -outline..=outline {
                if dx * dx + dy * dy > outline * outline {
                    continue;
                }
                let (x, y) = (px + dx, py + dy);
                if x >= 0 && y >= 0 && x < big as i32 && y < big as i32 && fill[(y as u32 * big + x as u32) as usize] {
                    return true;
                }
            }
        }
        false
    };
    let mut image = RgbaImage::new(SIZE, SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let (mut solid, mut edge) = (0u32, 0u32);
            for sy in 0..SCALE {
                for sx in 0..SCALE {
                    let (px, py) = (x * SCALE + sx, y * SCALE + sy);
                    if fill[(py * big + px) as usize] {
                        solid += 1;
                    } else if near(px as i32, py as i32) {
                        edge += 1;
                    }
                }
            }
            let samples = (SCALE * SCALE) as f32;
            let a_fill = solid as f32 / samples;
            let a_edge = edge as f32 / samples * 0.55;
            let alpha = (a_fill + a_edge).min(1.0);
            if alpha <= 0.0 {
                continue;
            }
            // Mixed with the dark outline by how much of each covers it.
            let k = a_fill / alpha;
            let mix = |c: u8| (c as f32 * k) as u8;
            image.put_pixel(x, y, Rgba([mix(r), mix(g), mix(b), (alpha * 255.0) as u8]));
        }
    }
    let mut out = std::io::Cursor::new(Vec::new());
    let _ = image.write_to(&mut out, ImageFormat::Png);
    out.into_inner()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_every_preset() {
        for (id, _) in PRESETS {
            let png = render(id, None);
            let image = image::load_from_memory(&png).unwrap().to_rgba8();
            assert_eq!(image.dimensions(), (64, 64));
            // Something is drawn, and the corners stay clear.
            assert!(image.pixels().any(|p| p.0[3] > 200), "{id} is empty");
            assert_eq!(image.get_pixel(0, 0).0[3], 0);
        }
    }
}
