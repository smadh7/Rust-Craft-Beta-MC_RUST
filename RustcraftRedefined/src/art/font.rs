//! Bitmap font and the GUI atlas.  Glyphs are a 5x8 column-encoded ASCII set: bit 0 is the top
//! row, bit 6 the baseline and bit 7 a descender row used by g, j, p, q and y.  Widths are derived
//! from the used columns so text is proportional.

use super::img::{hex, hexa, Col, Img, WHITE};

const GLYPHS: [[u8; 5]; 95] = [
    [0x00, 0x00, 0x00, 0x00, 0x00], [0x00, 0x00, 0x5F, 0x00, 0x00], [0x00, 0x07, 0x00, 0x07, 0x00], [0x14, 0x7F, 0x14, 0x7F, 0x14],
    [0x24, 0x2A, 0x7F, 0x2A, 0x12], [0x23, 0x13, 0x08, 0x64, 0x62], [0x36, 0x49, 0x55, 0x22, 0x50], [0x00, 0x05, 0x03, 0x00, 0x00],
    [0x00, 0x1C, 0x22, 0x41, 0x00], [0x00, 0x41, 0x22, 0x1C, 0x00], [0x14, 0x08, 0x3E, 0x08, 0x14], [0x08, 0x08, 0x3E, 0x08, 0x08],
    [0x00, 0x50, 0x30, 0x00, 0x00], [0x08, 0x08, 0x08, 0x08, 0x08], [0x00, 0x60, 0x60, 0x00, 0x00], [0x20, 0x10, 0x08, 0x04, 0x02],
    [0x3E, 0x51, 0x49, 0x45, 0x3E], [0x00, 0x42, 0x7F, 0x40, 0x00], [0x42, 0x61, 0x51, 0x49, 0x46], [0x21, 0x41, 0x45, 0x4B, 0x31],
    [0x18, 0x14, 0x12, 0x7F, 0x10], [0x27, 0x45, 0x45, 0x45, 0x39], [0x3C, 0x4A, 0x49, 0x49, 0x30], [0x01, 0x71, 0x09, 0x05, 0x03],
    [0x36, 0x49, 0x49, 0x49, 0x36], [0x06, 0x49, 0x49, 0x29, 0x1E], [0x00, 0x36, 0x36, 0x00, 0x00], [0x00, 0x56, 0x36, 0x00, 0x00],
    [0x08, 0x14, 0x22, 0x41, 0x00], [0x14, 0x14, 0x14, 0x14, 0x14], [0x00, 0x41, 0x22, 0x14, 0x08], [0x02, 0x01, 0x51, 0x09, 0x06],
    [0x32, 0x49, 0x79, 0x41, 0x3E], [0x7E, 0x11, 0x11, 0x11, 0x7E], [0x7F, 0x49, 0x49, 0x49, 0x36], [0x3E, 0x41, 0x41, 0x41, 0x22],
    [0x7F, 0x41, 0x41, 0x22, 0x1C], [0x7F, 0x49, 0x49, 0x49, 0x41], [0x7F, 0x09, 0x09, 0x09, 0x01], [0x3E, 0x41, 0x49, 0x49, 0x7A],
    [0x7F, 0x08, 0x08, 0x08, 0x7F], [0x00, 0x41, 0x7F, 0x41, 0x00], [0x20, 0x40, 0x41, 0x3F, 0x01], [0x7F, 0x08, 0x14, 0x22, 0x41],
    [0x7F, 0x40, 0x40, 0x40, 0x40], [0x7F, 0x02, 0x0C, 0x02, 0x7F], [0x7F, 0x04, 0x08, 0x10, 0x7F], [0x3E, 0x41, 0x41, 0x41, 0x3E],
    [0x7F, 0x09, 0x09, 0x09, 0x06], [0x3E, 0x41, 0x51, 0x21, 0x5E], [0x7F, 0x09, 0x19, 0x29, 0x46], [0x46, 0x49, 0x49, 0x49, 0x31],
    [0x01, 0x01, 0x7F, 0x01, 0x01], [0x3F, 0x40, 0x40, 0x40, 0x3F], [0x1F, 0x20, 0x40, 0x20, 0x1F], [0x3F, 0x40, 0x38, 0x40, 0x3F],
    [0x63, 0x14, 0x08, 0x14, 0x63], [0x07, 0x08, 0x70, 0x08, 0x07], [0x61, 0x51, 0x49, 0x45, 0x43], [0x00, 0x7F, 0x41, 0x41, 0x00],
    [0x02, 0x04, 0x08, 0x10, 0x20], [0x00, 0x41, 0x41, 0x7F, 0x00], [0x04, 0x02, 0x01, 0x02, 0x04], [0x40, 0x40, 0x40, 0x40, 0x40],
    [0x00, 0x01, 0x02, 0x04, 0x00], [0x20, 0x54, 0x54, 0x54, 0x78], [0x7F, 0x48, 0x44, 0x44, 0x38], [0x38, 0x44, 0x44, 0x44, 0x20],
    [0x38, 0x44, 0x44, 0x48, 0x7F], [0x38, 0x54, 0x54, 0x54, 0x18], [0x08, 0x7E, 0x09, 0x01, 0x02], [0x98, 0xA4, 0xA4, 0xA4, 0x7C],
    [0x7F, 0x08, 0x04, 0x04, 0x78], [0x00, 0x44, 0x7D, 0x40, 0x00], [0x00, 0x80, 0x80, 0x7D, 0x00], [0x7F, 0x10, 0x28, 0x44, 0x00],
    [0x00, 0x41, 0x7F, 0x40, 0x00], [0x7C, 0x04, 0x18, 0x04, 0x78], [0x7C, 0x08, 0x04, 0x04, 0x78], [0x38, 0x44, 0x44, 0x44, 0x38],
    [0xFC, 0x44, 0x44, 0x44, 0x38], [0x38, 0x44, 0x44, 0x44, 0xFC], [0x7C, 0x08, 0x04, 0x04, 0x08], [0x48, 0x54, 0x54, 0x54, 0x20],
    [0x04, 0x3F, 0x44, 0x40, 0x20], [0x3C, 0x40, 0x40, 0x20, 0x7C], [0x1C, 0x20, 0x40, 0x20, 0x1C], [0x3C, 0x40, 0x30, 0x40, 0x3C],
    [0x44, 0x28, 0x10, 0x28, 0x44], [0x9C, 0xA0, 0xA0, 0xA0, 0x7C], [0x44, 0x64, 0x54, 0x4C, 0x44], [0x00, 0x08, 0x36, 0x41, 0x00],
    [0x00, 0x00, 0x7F, 0x00, 0x00], [0x00, 0x41, 0x36, 0x08, 0x00], [0x08, 0x04, 0x08, 0x10, 0x08],
];

pub const ATLAS_SIZE: usize = 256;
/// Glyph cells are 8x8 texels, 16 per row, starting at the atlas origin.
pub const GLYPH_CELL: usize = 8;
pub const LINE_HEIGHT: f32 = 9.0;

/// Sub-rectangle of the GUI atlas in texels: x, y, w, h.
pub type Rect = [u16; 4];
pub const R_WHITE: Rect = [250, 250, 2, 2];
pub const R_SLOT: Rect = [0, 64, 20, 20];
pub const R_SLOT_SELECTED: Rect = [24, 64, 24, 24];
pub const R_HEART_EMPTY: Rect = [0, 96, 9, 9];
pub const R_HEART_FULL: Rect = [10, 96, 9, 9];
pub const R_HEART_HALF: Rect = [20, 96, 9, 9];
pub const R_FOOD_EMPTY: Rect = [30, 96, 9, 9];
pub const R_FOOD_FULL: Rect = [40, 96, 9, 9];
pub const R_FOOD_HALF: Rect = [50, 96, 9, 9];
pub const R_BUBBLE: Rect = [60, 96, 9, 9];
pub const R_ARMOR_FULL: Rect = [70, 96, 9, 9];
pub const R_CROSSHAIR: Rect = [0, 112, 9, 9];
pub const R_BUTTON: Rect = [0, 128, 200, 20];
pub const R_BUTTON_HOVER: Rect = [0, 150, 200, 20];
pub const R_PANEL: Rect = [0, 176, 24, 24];
pub const R_SUN: Rect = [64, 200, 32, 32];
pub const R_MOON: Rect = [100, 200, 32, 32];
pub const R_ARROW: Rect = [48, 112, 22, 15];
pub const R_FLAME: Rect = [72, 112, 14, 14];

#[derive(Clone, Copy, Default)]
pub struct Glyph {
    /// First used column and advance width in texels.
    pub start: u8,
    pub width: u8,
}

pub struct Font {
    pub glyphs: [Glyph; 95],
}

impl Font {
    pub fn new() -> Font {
        let mut glyphs = [Glyph::default(); 95];
        for (i, g) in GLYPHS.iter().enumerate() {
            let first = g.iter().position(|c| *c != 0);
            let last = g.iter().rposition(|c| *c != 0);
            glyphs[i] = match (first, last) {
                (Some(a), Some(b)) => Glyph { start: a as u8, width: (b - a + 2) as u8 },
                _ => Glyph { start: 0, width: 4 },
            };
        }
        Font { glyphs }
    }
    #[inline]
    pub fn glyph(&self, ch: char) -> (usize, Glyph) {
        let i = if (' '..='~').contains(&ch) { ch as usize - 32 } else { '?' as usize - 32 };
        (i, self.glyphs[i])
    }
    /// Width of a string in unscaled texels.
    pub fn width(&self, text: &str) -> f32 {
        text.chars().map(|c| self.glyph(c).1.width as f32).sum()
    }
}

impl Default for Font {
    fn default() -> Self {
        Font::new()
    }
}

fn heart(img: &mut Img, ox: i32, oy: i32, fill: Option<Col>, half: bool) {
    const SHAPE: [&str; 9] = [".##.##...", "#########", "#########", "#########", ".#######.", "..#####..", "...###...", "....#....", "........."];
    for (y, row) in SHAPE.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch != '#' {
                continue;
            }
            let edge = y == 0 || x == 0 || x == 8 || SHAPE.get(y + 1).map_or(true, |r| r.as_bytes()[x] != b'#') || row.as_bytes().get(x + 1) != Some(&b'#') || (x > 0 && row.as_bytes()[x - 1] != b'#') || (y > 0 && SHAPE[y - 1].as_bytes()[x] != b'#');
            let c = match fill {
                Some(c) if !edge && (!half || x < 5) => if x < 3 && y < 3 { super::img::tone(c, 0.35) } else { c },
                _ if edge => hex(0x1A1A1A),
                _ => hex(0x4A3A3A),
            };
            img.set(ox + x as i32, oy + y as i32, c);
        }
    }
}

fn drumstick(img: &mut Img, ox: i32, oy: i32, fill: Option<Col>, half: bool) {
    const SHAPE: [&str; 9] = ["....###..", "...#####.", "..######.", "..######.", "..#####..", ".####....", "##.......", "##.......", "........."];
    for (y, row) in SHAPE.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if ch == '#' {
                let bone = y >= 6;
                let c = match fill {
                    Some(_) if bone => hex(0xE8E0D0),
                    Some(c) if !half || x < 5 => c,
                    _ => hex(0x3A2E26),
                };
                img.set(ox + x as i32, oy + y as i32, c);
            }
        }
    }
}

fn panel(img: &mut Img, r: Rect, face: Col, light: Col, dark: Col, border: Col) {
    let [x, y, w, h] = r.map(|v| v as i32);
    img.rect(x, y, w, h, face);
    img.rect(x, y, w, 1, light);
    img.rect(x, y, 1, h, light);
    img.rect(x, y + h - 1, w, 1, dark);
    img.rect(x + w - 1, y, 1, h, dark);
    img.outline(x - 1, y - 1, w + 2, h + 2, border);
}

/// Builds the GUI atlas: glyphs, widgets and celestial sprites.
pub fn build_atlas() -> Img {
    let mut img = Img::new(ATLAS_SIZE, ATLAS_SIZE);
    for (i, g) in GLYPHS.iter().enumerate() {
        let (cx, cy) = ((i % 16) * GLYPH_CELL, (i / 16) * GLYPH_CELL);
        for (col, bits) in g.iter().enumerate() {
            for row in 0..8 {
                if bits & (1 << row) != 0 {
                    img.set((cx + col) as i32, (cy + row) as i32, WHITE);
                }
            }
        }
    }
    img.rect(R_WHITE[0] as i32 - 2, R_WHITE[1] as i32 - 2, 6, 6, WHITE);
    // Hotbar slot and selection frame.
    let [sx, sy, sw, sh] = R_SLOT.map(|v| v as i32);
    img.rect(sx, sy, sw, sh, hexa(0x202028, 150));
    img.outline(sx, sy, sw, sh, hexa(0x8A8A92, 230));
    img.outline(sx + 1, sy + 1, sw - 2, sh - 2, hexa(0x101014, 200));
    let [qx, qy, qw, qh] = R_SLOT_SELECTED.map(|v| v as i32);
    img.outline(qx, qy, qw, qh, hex(0xF4F4F4));
    img.outline(qx + 1, qy + 1, qw - 2, qh - 2, hex(0xF4F4F4));
    img.outline(qx + 2, qy + 2, qw - 4, qh - 4, hex(0x30303A));
    // Status icons.
    heart(&mut img, R_HEART_EMPTY[0] as i32, R_HEART_EMPTY[1] as i32, None, false);
    heart(&mut img, R_HEART_FULL[0] as i32, R_HEART_FULL[1] as i32, Some(hex(0xE0242A)), false);
    heart(&mut img, R_HEART_HALF[0] as i32, R_HEART_HALF[1] as i32, Some(hex(0xE0242A)), true);
    drumstick(&mut img, R_FOOD_EMPTY[0] as i32, R_FOOD_EMPTY[1] as i32, None, false);
    drumstick(&mut img, R_FOOD_FULL[0] as i32, R_FOOD_FULL[1] as i32, Some(hex(0xB8742A)), false);
    drumstick(&mut img, R_FOOD_HALF[0] as i32, R_FOOD_HALF[1] as i32, Some(hex(0xB8742A)), true);
    let [bx, by, ..] = R_BUBBLE.map(|v| v as i32);
    for (x, y) in [(3, 1), (4, 1), (5, 1), (2, 2), (6, 2), (1, 3), (7, 3), (1, 4), (7, 4), (1, 5), (7, 5), (2, 6), (6, 6), (3, 7), (4, 7), (5, 7)] {
        img.set(bx + x, by + y, hex(0x8FD0FF));
    }
    img.set(bx + 3, by + 3, WHITE);
    let [ax, ay, ..] = R_ARMOR_FULL.map(|v| v as i32);
    for y in 0..8 {
        let inset = if y < 5 { 0 } else { y - 4 };
        img.rect(ax + 1 + inset, ay + y, 7 - inset * 2, 1, if y == 0 { hex(0xE8E8F0) } else { hex(0xA8A8B4) });
    }
    let [cx, cy, ..] = R_CROSSHAIR.map(|v| v as i32);
    img.rect(cx + 4, cy, 1, 9, WHITE);
    img.rect(cx, cy + 4, 9, 1, WHITE);
    // Buttons and window panel.
    panel(&mut img, R_BUTTON, hex(0x6E6E76), hex(0xA8A8B0), hex(0x48484E), hex(0x101014));
    panel(&mut img, R_BUTTON_HOVER, hex(0x7C86B8), hex(0xBCC6F4), hex(0x525A86), hex(0xF4F4F4));
    panel(&mut img, R_PANEL, hex(0xC6C6C6), hex(0xFFFFFF), hex(0x555555), hex(0x101014));
    // Sun: a soft-edged square with a brighter core.  Moon: pale disc with a few craters.
    let [ux, uy, uw, _] = R_SUN.map(|v| v as i32);
    for y in 0..uw {
        for x in 0..uw {
            let d = ((x - uw / 2).abs()).max((y - uw / 2).abs());
            if d < 12 {
                img.set(ux + x, uy + y, if d < 9 { hex(0xFFF7D0) } else { hex(0xFFE27A) });
            } else if d < 15 {
                img.set(ux + x, uy + y, hexa(0xFFD860, (255 - (d - 11) * 70) as u8));
            }
        }
    }
    let [mx, my, mw, _] = R_MOON.map(|v| v as i32);
    for y in 0..mw {
        for x in 0..mw {
            let d2 = (x - mw / 2) * (x - mw / 2) + (y - mw / 2) * (y - mw / 2);
            if d2 < 144 {
                let crater = [(11, 12, 9), (20, 18, 12), (15, 22, 5)].iter().any(|&(cx, cy, r2)| (x - cx) * (x - cx) + (y - cy) * (y - cy) < r2);
                img.set(mx + x, my + y, if crater { hex(0xB4BCCC) } else { hex(0xE4E8F2) });
            }
        }
    }
    // Furnace arrow and flame.
    let [rx, ry, ..] = R_ARROW.map(|v| v as i32);
    img.rect(rx, ry + 5, 14, 5, WHITE);
    for i in 0..8 {
        img.rect(rx + 14 + i, ry + i, 1, 15 - i * 2, WHITE);
    }
    let [fx, fy, ..] = R_FLAME.map(|v| v as i32);
    for y in 0..14 {
        let half = (1 + y / 2).min(6);
        img.rect(fx + 7 - half, fy + y, half * 2, 1, if y > 8 { hex(0xFFE27A) } else { hex(0xF08A1A) });
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyph_metrics() {
        let f = Font::new();
        assert_eq!(f.glyph(' ').1.width, 4);
        assert!(f.glyph('i').1.width < f.glyph('W').1.width);
        assert_eq!(f.glyph('W').1.width, 6);
        assert_eq!(f.glyph('\u{1F600}').0, '?' as usize - 32);
        assert!(f.width("Hello") > 20.0);
        let atlas = build_atlas();
        assert_eq!(atlas.px.len(), ATLAS_SIZE * ATLAS_SIZE);
        // The solid white texel used for flat colours must be opaque white.
        assert_eq!(atlas.get(R_WHITE[0] as i32, R_WHITE[1] as i32), WHITE);
    }
}
