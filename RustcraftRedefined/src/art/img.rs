//! A tiny software canvas plus the procedural patterns every texture in the game is built from.
//! Nothing here is loaded from disk: all art is generated at start-up from these recipes.

use crate::util::rng::{hash64, Rng};

pub type Col = [u8; 4];
pub const CLEAR: Col = [0, 0, 0, 0];
pub const WHITE: Col = [255, 255, 255, 255];
pub const BLACK: Col = [0, 0, 0, 255];

#[inline]
pub const fn hex(v: u32) -> Col {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8, 255]
}
#[inline]
pub fn hexa(v: u32, a: u8) -> Col {
    [(v >> 16) as u8, (v >> 8) as u8, v as u8, a]
}
/// Multiplies the colour channels by `f`.
#[inline]
pub fn shade(c: Col, f: f32) -> Col {
    let m = |v: u8| (v as f32 * f).round().clamp(0.0, 255.0) as u8;
    [m(c[0]), m(c[1]), m(c[2]), c[3]]
}
#[inline]
pub fn mix(a: Col, b: Col, t: f32) -> Col {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), m(a[3], b[3])]
}
/// Shifts towards white (t > 0) or black (t < 0).
pub fn tone(c: Col, t: f32) -> Col {
    if t >= 0.0 {
        mix(c, [255, 255, 255, c[3]], t)
    } else {
        mix(c, [0, 0, 0, c[3]], -t)
    }
}
pub fn luma(c: Col) -> f32 {
    (c[0] as f32 * 0.299 + c[1] as f32 * 0.587 + c[2] as f32 * 0.114) / 255.0
}

#[derive(Clone)]
pub struct Img {
    pub w: usize,
    pub h: usize,
    pub px: Vec<Col>,
}

impl Img {
    pub fn new(w: usize, h: usize) -> Img {
        Img { w, h, px: vec![CLEAR; w * h] }
    }
    pub fn tile() -> Img {
        Img::new(16, 16)
    }
    pub fn filled(c: Col) -> Img {
        let mut i = Img::tile();
        i.fill(c);
        i
    }
    pub fn fill(&mut self, c: Col) {
        self.px.iter_mut().for_each(|p| *p = c);
    }
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, c: Col) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[y as usize * self.w + x as usize] = c;
        }
    }
    /// Sets a pixel with wrap-around, so patterns tile seamlessly.
    #[inline]
    pub fn setw(&mut self, x: i32, y: i32, c: Col) {
        let (w, h) = (self.w as i32, self.h as i32);
        self.px[(y.rem_euclid(h) * w + x.rem_euclid(w)) as usize] = c;
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32) -> Col {
        let (w, h) = (self.w as i32, self.h as i32);
        self.px[(y.rem_euclid(h) * w + x.rem_euclid(w)) as usize]
    }
    pub fn rect(&mut self, x0: i32, y0: i32, w: i32, h: i32, c: Col) {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                self.set(x, y, c);
            }
        }
    }
    pub fn outline(&mut self, x0: i32, y0: i32, w: i32, h: i32, c: Col) {
        for x in x0..x0 + w {
            self.set(x, y0, c);
            self.set(x, y0 + h - 1, c);
        }
        for y in y0..y0 + h {
            self.set(x0, y, c);
            self.set(x0 + w - 1, y, c);
        }
    }
    /// Applies `f(x, y, old) -> new` to every pixel.
    pub fn map(&mut self, mut f: impl FnMut(i32, i32, Col) -> Col) {
        for y in 0..self.h {
            for x in 0..self.w {
                let i = y * self.w + x;
                self.px[i] = f(x as i32, y as i32, self.px[i]);
            }
        }
    }
    /// Alpha-blends `src` over this image.
    pub fn overlay(&mut self, src: &Img) {
        for (d, s) in self.px.iter_mut().zip(&src.px) {
            if s[3] == 255 {
                *d = *s;
            } else if s[3] > 0 {
                let t = s[3] as f32 / 255.0;
                let a = d[3].max(s[3]);
                *d = mix(*d, [s[0], s[1], s[2], 255], t);
                d[3] = a;
            }
        }
    }
    pub fn blit(&mut self, src: &Img, dx: i32, dy: i32) {
        for y in 0..src.h as i32 {
            for x in 0..src.w as i32 {
                let c = src.px[y as usize * src.w + x as usize];
                if c[3] > 0 {
                    self.set(dx + x, dy + y, c);
                }
            }
        }
    }
    pub fn rotated_cw(&self) -> Img {
        let mut o = Img::new(self.h, self.w);
        for y in 0..self.h {
            for x in 0..self.w {
                o.px[x * o.w + (self.h - 1 - y)] = self.px[y * self.w + x];
            }
        }
        o
    }
    pub fn flipped_v(&self) -> Img {
        let mut o = self.clone();
        for y in 0..self.h {
            for x in 0..self.w {
                o.px[y * self.w + x] = self.px[(self.h - 1 - y) * self.w + x];
            }
        }
        o
    }
    pub fn bytes(&self) -> Vec<u8> {
        self.px.iter().flat_map(|c| c.iter().copied()).collect()
    }
    /// Halves the resolution; colour is weighted by alpha so cut-out textures get no dark fringe.
    pub fn half(&self) -> Img {
        let (w, h) = ((self.w / 2).max(1), (self.h / 2).max(1));
        let mut o = Img::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let (mut r, mut g, mut b, mut a) = (0u32, 0u32, 0u32, 0u32);
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    let c = self.get((x * 2 + dx) as i32, (y * 2 + dy) as i32);
                    r += c[0] as u32 * c[3] as u32;
                    g += c[1] as u32 * c[3] as u32;
                    b += c[2] as u32 * c[3] as u32;
                    a += c[3] as u32;
                }
                o.px[y * w + x] = if a == 0 { CLEAR } else { [(r / a) as u8, (g / a) as u8, (b / a) as u8, (a / 4) as u8] };
            }
        }
        o
    }
}

/// Deterministic per-texel random value in 0..1.
#[inline]
pub fn rnd(seed: u64, x: i32, y: i32) -> f32 {
    (hash64(seed, x, y, 0) >> 40) as f32 * (1.0 / 16_777_216.0)
}

/// Smooth value noise that repeats every `period` cells; `cell` is the cell size in texels.
pub fn vnoise(seed: u64, x: f32, y: f32, cell: f32, period: i32) -> f32 {
    let (fx, fy) = (x / cell, y / cell);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (tx, ty) = (fx - x0, fy - y0);
    let (sx, sy) = (tx * tx * (3.0 - 2.0 * tx), ty * ty * (3.0 - 2.0 * ty));
    let at = |ix: i32, iy: i32| rnd(seed, ix.rem_euclid(period), iy.rem_euclid(period));
    let (ix, iy) = (x0 as i32, y0 as i32);
    let a = at(ix, iy) + (at(ix + 1, iy) - at(ix, iy)) * sx;
    let b = at(ix, iy + 1) + (at(ix + 1, iy + 1) - at(ix, iy + 1)) * sx;
    a + (b - a) * sy
}

/// Quantises a brightness offset into a few steps for a pixel-art look.
#[inline]
fn steps(v: f32, n: f32) -> f32 {
    (v * n).round() / n
}

/// Blotchy mineral surface: stone, dirt, sand, ore bases.
pub fn rock(base: Col, contrast: f32, grain: f32, seed: u64) -> Img {
    let mut img = Img::tile();
    img.map(|x, y, _| {
        let big = vnoise(seed, x as f32, y as f32, 4.0, 4) - 0.5;
        let mid = vnoise(seed ^ 77, x as f32, y as f32, 2.0, 8) - 0.5;
        let fine = rnd(seed ^ 991, x, y) - 0.5;
        let v = big * 0.9 + mid * 0.7 + fine * grain;
        tone(base, steps(v * contrast, 12.0))
    });
    img
}

/// Fine random speckle: sand, concrete powder, snow.
pub fn speckle(base: Col, contrast: f32, seed: u64) -> Img {
    let mut img = Img::tile();
    img.map(|x, y, _| tone(base, steps((rnd(seed, x, y) - 0.5) * contrast, 10.0)));
    img
}

/// Rounded cobbles separated by dark seams.
pub fn cobble(base: Col, contrast: f32, cells: u32, seed: u64) -> Img {
    let mut rng = Rng::new(seed);
    let pts: Vec<(f32, f32, f32)> = (0..cells).map(|_| (rng.f32() * 16.0, rng.f32() * 16.0, rng.f32() - 0.5)).collect();
    let mut img = Img::tile();
    img.map(|x, y, _| {
        let (mut d1, mut d2, mut tint) = (99.0f32, 99.0f32, 0.0);
        for &(px, py, t) in &pts {
            let dx = ((x as f32 + 0.5 - px).abs()).min(16.0 - (x as f32 + 0.5 - px).abs());
            let dy = ((y as f32 + 0.5 - py).abs()).min(16.0 - (y as f32 + 0.5 - py).abs());
            let d = (dx * dx + dy * dy * 1.3).sqrt();
            if d < d1 {
                d2 = d1;
                d1 = d;
                tint = t;
            } else if d < d2 {
                d2 = d;
            }
        }
        let edge = d2 - d1;
        let mut v = tint * 0.5 + (rnd(seed ^ 5, x, y) - 0.5) * 0.25;
        if edge < 0.9 {
            v -= 0.75;
        } else if edge < 1.8 {
            v += 0.25;
        }
        tone(base, steps(v * contrast, 10.0))
    });
    img
}

/// Rectangular bricks. `bw`/`bh` include the one texel mortar line; alternate rows are offset by half.
pub fn bricks(brick: Col, mortar: Col, bw: i32, bh: i32, contrast: f32, seed: u64) -> Img {
    let mut img = Img::tile();
    img.map(|x, y, _| {
        let row = y.div_euclid(bh);
        let xo = (x + if row % 2 == 1 { bw / 2 } else { 0 }).rem_euclid(16);
        let (lx, ly) = (xo.rem_euclid(bw), y.rem_euclid(bh));
        if lx == bw - 1 || ly == bh - 1 {
            return tone(mortar, (rnd(seed ^ 3, x, y) - 0.5) * 0.12);
        }
        let id = hash64(seed, xo.div_euclid(bw), row, 1);
        let mut v = ((id & 255) as f32 / 255.0 - 0.5) * 0.6 + (rnd(seed ^ 9, x, y) - 0.5) * 0.35;
        if ly == 0 || lx == 0 {
            v += 0.3;
        }
        if ly == bh - 2 || lx == bw - 2 {
            v -= 0.3;
        }
        tone(brick, steps(v * contrast, 10.0))
    });
    img
}

/// Four horizontal boards with seams, end joints and grain.
pub fn planks(base: Col, seed: u64) -> Img {
    let mut img = Img::tile();
    img.map(|x, y, _| {
        let board = y / 4;
        let joint = (hash64(seed, board, 0, 7) % 16) as i32;
        let grain = vnoise(seed ^ (board as u64 * 131), x as f32, y as f32 * 4.0, 5.0, 16) - 0.5;
        let mut v = grain * 0.32 + (((hash64(seed, board, 1, 7) & 255) as f32 / 255.0) - 0.5) * 0.16 + (rnd(seed, x, y) - 0.5) * 0.06;
        if y % 4 == 3 {
            v -= 0.34;
        } else if x == joint {
            v -= 0.26;
        } else if y % 4 == 0 {
            v += 0.07;
        }
        tone(base, steps(v, 14.0))
    });
    img
}

/// Vertical bark streaks.
pub fn bark(base: Col, contrast: f32, seed: u64) -> Img {
    let mut img = Img::tile();
    img.map(|x, y, _| {
        let col = ((hash64(seed, x, 0, 3) & 255) as f32 / 255.0) - 0.5;
        let streak = vnoise(seed ^ 21, x as f32 * 3.0, y as f32, 6.0, 8) - 0.5;
        tone(base, steps((col * 0.55 + streak * 0.6 + (rnd(seed, x, y) - 0.5) * 0.2) * contrast, 10.0))
    });
    img
}

/// End grain: bark border around concentric rings.
pub fn rings(bark_col: Col, wood: Col, seed: u64) -> Img {
    let mut img = Img::tile();
    img.map(|x, y, _| {
        let d = (x.min(15 - x)).min(y.min(15 - y));
        if d == 0 {
            return tone(bark_col, (rnd(seed, x, y) - 0.5) * 0.3);
        }
        let ring = if d % 2 == 0 { -0.16 } else { 0.06 };
        tone(wood, ring + (rnd(seed ^ 8, x, y) - 0.5) * 0.08)
    });
    img
}

/// Foliage with see-through gaps.  Pass a grey `base` for biome-tinted leaves.
pub fn leaves(base: Col, gaps: f32, seed: u64) -> Img {
    let mut img = Img::tile();
    img.map(|x, y, _| {
        let n = vnoise(seed, x as f32, y as f32, 2.0, 8) * 0.6 + rnd(seed ^ 4, x, y) * 0.4;
        if n < gaps {
            return CLEAR;
        }
        let v = (rnd(seed ^ 12, x, y) - 0.5) * 0.5 + (n - 0.5) * 0.5;
        tone(base, steps(v, 8.0))
    });
    img
}

/// Scatters small gem clusters over a base texture (ores).
pub fn ore(mut img: Img, gem: Col, clusters: u32, seed: u64) -> Img {
    let mut rng = Rng::new(seed ^ 0x0e5);
    for _ in 0..clusters {
        let (cx, cy) = (rng.range(1, 14), rng.range(1, 14));
        let n = rng.range(2, 4);
        for i in 0..n {
            let (x, y) = (cx + rng.range(-1, 1), cy + rng.range(-1, 1));
            let c = match i {
                0 => tone(gem, 0.35),
                1 => gem,
                _ => tone(gem, -0.25),
            };
            img.setw(x, y, c);
            let below = img.get(x, y + 1);
            if below[..3] != c[..3] && rng.chance(0.5) {
                img.setw(x, y + 1, tone(below, -0.3));
            }
        }
    }
    img
}

/// Adds a lighter top-left and darker bottom-right bevel (polished stone, metal blocks).
pub fn bevel(img: &mut Img, amount: f32) {
    let (w, h) = (img.w as i32, img.h as i32);
    img.map(|x, y, c| {
        if x == 0 || y == 0 {
            tone(c, amount)
        } else if x == w - 1 || y == h - 1 {
            tone(c, -amount)
        } else {
            c
        }
    });
}

/// Grey scale copy of an image scaled by `gain` (used to prepare tintable textures).
pub fn grayscale(img: &Img, gain: f32) -> Img {
    let mut o = img.clone();
    o.map(|_, _, c| {
        let v = (luma(c) * gain * 255.0).clamp(0.0, 255.0) as u8;
        [v, v, v, c[3]]
    });
    o
}

/// Draws a sprite from rows of characters using a palette of (char, colour) pairs; '.' is transparent.
pub fn sprite(rows: &[&str], palette: &[(char, Col)]) -> Img {
    let h = rows.len();
    let w = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
    let mut img = Img::new(w.max(1), h.max(1));
    for (y, row) in rows.iter().enumerate() {
        for (x, ch) in row.chars().enumerate() {
            if let Some((_, c)) = palette.iter().find(|(p, _)| *p == ch) {
                img.px[y * img.w + x] = *c;
            }
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_are_deterministic_and_tile_sized() {
        let a = rock(hex(0x808080), 0.3, 0.4, 5);
        let b = rock(hex(0x808080), 0.3, 0.4, 5);
        assert_eq!(a.px, b.px);
        assert_eq!(a.px.len(), 256);
        assert!(a.px.iter().all(|p| p[3] == 255));
        let l = leaves(hex(0x9a9a9a), 0.3, 9);
        let holes = l.px.iter().filter(|p| p[3] == 0).count();
        assert!(holes > 20 && holes < 150, "{holes}");
    }

    #[test]
    fn half_ignores_transparent_texels() {
        let mut i = Img::new(2, 2);
        i.px[0] = [200, 100, 50, 255];
        let h = i.half();
        assert_eq!(h.px[0], [200, 100, 50, 63]);
    }
}
