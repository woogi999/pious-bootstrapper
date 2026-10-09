//! Auto arrange: lays Roblox windows out across the screens, so many
//! instances can be watched at once. Pure layout math here (tested); the
//! service finds the windows and moves them.

/// A rectangle on the desktop, in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Rows × columns for `n` windows in an area, picked so each window is as
/// close to 16:9 as possible.
fn grid(n: usize, area: Rect) -> (usize, usize) {
    let mut best = (1, n.max(1));
    let mut best_score = f64::MAX;
    for rows in 1..=n.max(1) {
        let cols = n.div_ceil(rows);
        let w = area.width as f64 / cols as f64;
        let h = area.height as f64 / rows as f64;
        // How far from 16:9, plus a little for empty cells.
        let score = ((w / h) / (16.0 / 9.0)).ln().abs() + (rows * cols - n) as f64 * 0.05;
        if score < best_score {
            best_score = score;
            best = (rows, cols);
        }
    }
    best
}

/// Splits `n` windows over the screens, the biggest screens taking more.
fn share(n: usize, screens: &[Rect]) -> Vec<usize> {
    let total: f64 = screens.iter().map(|s| s.width as f64 * s.height as f64).sum();
    let mut counts: Vec<usize> = screens.iter().map(|s| ((s.width as f64 * s.height as f64) / total * n as f64).floor() as usize).collect();
    // Hand out what rounding left, biggest screens first.
    let mut order: Vec<usize> = (0..screens.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(screens[i].width as i64 * screens[i].height as i64));
    let mut left = n - counts.iter().sum::<usize>();
    for &i in order.iter().cycle() {
        if left == 0 {
            break;
        }
        counts[i] += 1;
        left -= 1;
    }
    counts
}

/// Where each of `n` windows goes on `screens` (their work areas, without
/// the taskbar), in the given layout. Every rectangle lies inside a screen.
pub fn layout(n: usize, screens: &[Rect], kind: &str) -> Vec<Rect> {
    if n == 0 || screens.is_empty() {
        return Vec::new();
    }
    let gap = 4;
    let mut out = Vec::with_capacity(n);
    let counts = if kind == "cascade" { vec![n] } else { share(n, screens) };
    for (screen, &count) in screens.iter().zip(&counts) {
        if count == 0 {
            continue;
        }
        let (rows, cols) = match kind {
            "columns" => (1, count),
            "rows" => (count, 1),
            "cascade" => {
                // Each window 70% of the screen, stepped down and right.
                let (w, h) = (screen.width * 7 / 10, screen.height * 7 / 10);
                let step = ((screen.width - w).min(screen.height - h) / count.max(1) as i32).clamp(0, 40);
                for i in 0..count as i32 {
                    out.push(Rect { x: screen.x + i * step, y: screen.y + i * step, width: w, height: h });
                }
                continue;
            }
            _ => grid(count, *screen),
        };
        let (cell_w, cell_h) = (screen.width / cols as i32, screen.height / rows as i32);
        for i in 0..count {
            let (row, col) = ((i / cols) as i32, (i % cols) as i32);
            out.push(Rect {
                x: screen.x + col * cell_w + gap / 2,
                y: screen.y + row * cell_h + gap / 2,
                width: (cell_w - gap).max(1),
                height: (cell_h - gap).max(1),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect { x: 0, y: 0, width: 1920, height: 1040 };

    fn inside(r: &Rect, s: &Rect) -> bool {
        r.x >= s.x && r.y >= s.y && r.x + r.width <= s.x + s.width && r.y + r.height <= s.y + s.height
    }

    #[test]
    fn grid_layouts_fit_on_screen() {
        for n in 1..=12 {
            let rects = layout(n, &[SCREEN], "grid");
            assert_eq!(rects.len(), n);
            assert!(rects.iter().all(|r| inside(r, &SCREEN)), "{n}: {rects:?}");
            // No two overlap.
            for (i, a) in rects.iter().enumerate() {
                for b in &rects[i + 1..] {
                    let apart = a.x + a.width <= b.x || b.x + b.width <= a.x || a.y + a.height <= b.y || b.y + b.height <= a.y;
                    assert!(apart, "{a:?} {b:?}");
                }
            }
        }
        assert_eq!(layout(4, &[SCREEN], "grid").iter().map(|r| r.y).collect::<std::collections::BTreeSet<_>>().len(), 2, "four make a 2×2");
    }

    #[test]
    fn spreads_over_monitors() {
        let second = Rect { x: 1920, y: 0, width: 1920, height: 1040 };
        let rects = layout(4, &[SCREEN, second], "grid");
        assert_eq!(rects.iter().filter(|r| inside(r, &SCREEN)).count(), 2);
        assert_eq!(rects.iter().filter(|r| inside(r, &second)).count(), 2);
        // A screen to the left (negative coordinates) works too.
        let left = Rect { x: -1280, y: 100, width: 1280, height: 984 };
        assert!(layout(3, &[left], "columns").iter().all(|r| inside(r, &left)));
    }

    #[test]
    fn other_layouts() {
        assert!(layout(3, &[SCREEN], "rows").iter().all(|r| r.width > 1900));
        assert!(layout(5, &[SCREEN], "cascade").iter().all(|r| inside(r, &SCREEN)));
        assert!(layout(0, &[SCREEN], "grid").is_empty());
        assert!(layout(2, &[], "grid").is_empty());
    }
}
