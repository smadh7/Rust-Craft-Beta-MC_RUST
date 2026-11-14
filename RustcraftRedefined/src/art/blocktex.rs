//! Procedural block textures.  `generate` maps a texture name to a 16x16 image by parsing the
//! name into a material plus a style, so whole block families share a few recipes.

use super::img::*;
use super::palette::*;
use crate::util::rng::{hash_str, Rng};

/// Number of animation frames for fluids.
pub const FLUID_FRAMES: u32 = 32;

const GREEN: Col = hex(0x4F8A2C);
const STEM: Col = hex(0x3F7A28);

pub fn missing() -> Img {
    let mut i = Img::tile();
    i.map(|x, y, _| if (x / 8 + y / 8) % 2 == 0 { hex(0xE040E0) } else { hex(0x202020) });
    i
}

/// Animated fluid frame; `t` is the frame index.
pub fn fluid(name: &str, t: u32) -> Option<Img> {
    let ph = t as f32 / FLUID_FRAMES as f32 * std::f32::consts::TAU;
    let flow = name.ends_with("_flow");
    let mut img = Img::tile();
    if name.starts_with("water") {
        img.map(|x, y, _| {
            let yy = if flow { y as f32 - t as f32 * 0.5 } else { y as f32 };
            let a = vnoise(11, x as f32 + ph.cos() * 2.0, yy + ph.sin() * 2.0, 4.0, 4);
            let b = vnoise(29, x as f32 - ph.sin() * 2.0, yy * if flow { 0.5 } else { 1.0 } + ph.cos() * 2.0, 8.0, 2);
            let v = (a * 0.6 + b * 0.4 - 0.5) * 0.5;
            let c = tone(hex(0xC8D2E4), v.clamp(-0.3, 0.3));
            [c[0], c[1], c[2], (176.0 + v * 80.0) as u8]
        });
    } else if name.starts_with("lava") {
        img.map(|x, y, _| {
            let yy = if flow { y as f32 - t as f32 * 0.25 } else { y as f32 };
            let a = vnoise(41, x as f32 + ph.cos() * 3.0, yy + ph.sin() * 3.0, 4.0, 4);
            let b = vnoise(53, x as f32 - ph.sin() * 1.5, yy + ph.cos() * 1.5, 2.0, 8);
            let v = a * 0.7 + b * 0.3;
            if v > 0.62 {
                mix(hex(0xF8B030), hex(0xFFF08A), ((v - 0.62) * 5.0).min(1.0))
            } else if v > 0.42 {
                mix(hex(0xD8581A), hex(0xF8A030), (v - 0.42) * 5.0)
            } else {
                mix(hex(0x9C2A10), hex(0xD8581A), (v / 0.42).powi(2))
            }
        });
    } else {
        return None;
    }
    Some(img)
}

fn stone_bricks(base: Col, k: f32, seed: u64) -> Img {
    bricks(base, tone(base, -0.42), 8, 8, k.max(0.16) * 1.1, seed)
}

fn cracks(img: &mut Img, seed: u64) {
    let mut rng = Rng::new(seed ^ 0xc4);
    for _ in 0..3 {
        let (mut x, mut y) = (rng.range(1, 14), rng.range(0, 6));
        for _ in 0..rng.range(5, 9) {
            let c = img.get(x, y);
            img.setw(x, y, tone(c, -0.45));
            y += 1;
            x += rng.range(-1, 1);
        }
    }
}

fn moss_over(img: &mut Img, seed: u64, amount: f32) {
    img.map(|x, y, c| {
        let n = vnoise(seed ^ 0x66, x as f32, y as f32, 3.0, 16) * 0.7 + rnd(seed, x, y) * 0.3;
        if n > 1.0 - amount {
            tone(hex(0x5A7A2E), (rnd(seed ^ 2, x, y) - 0.5) * 0.4)
        } else {
            c
        }
    });
}

fn chiseled(base: Col, k: f32, seed: u64) -> Img {
    let mut img = rock(base, k * 0.5, 0.2, seed);
    img.outline(0, 0, 16, 16, tone(base, -0.3));
    img.outline(2, 2, 12, 12, tone(base, -0.38));
    img.outline(3, 3, 10, 10, tone(base, 0.22));
    for i in 0..4 {
        img.set(5 + i, 5 + i, tone(base, -0.4));
        img.set(10 - i, 5 + i, tone(base, -0.4));
        img.set(6 + i, 5 + i, tone(base, 0.2));
    }
    img
}

/// Mineral families: `stone`, `polished_granite`, `deepslate_tiles`, `chiseled_sulfur`, ...
fn mineral_texture(name: &str, seed: u64) -> Option<Img> {
    let direct = |n: &str| mineral(n);
    if let Some((c, k)) = direct(name) {
        return Some(match name {
            "deepslate" => {
                let mut i = rock(c, k, 0.3, seed);
                i.map(|x, y, p| tone(p, (vnoise(seed, x as f32, y as f32 * 4.0, 8.0, 8) - 0.5) * 0.28));
                i
            }
            "bedrock" => rock(c, k, 0.8, seed),
            "smooth_stone" => {
                let mut i = rock(c, k, 0.3, seed);
                i.outline(0, 0, 16, 16, tone(c, -0.22));
                i
            }
            "obsidian" => {
                let mut i = rock(c, k, 0.5, seed);
                i.map(|x, y, p| if rnd(seed ^ 6, x, y) > 0.9 { hex(0x3B2754) } else { p });
                i
            }
            "amethyst" | "netherrack" | "granite" | "diorite" | "prismarine" => rock(c, k, 1.0, seed),
            "sulfur" => {
                let mut i = rock(c, k, 0.5, seed);
                i.map(|x, y, p| {
                    let n = vnoise(seed ^ 0x51, x as f32 + y as f32 * 0.5, y as f32, 4.0, 4);
                    if n > 0.68 { tone(hex(0xF2EE9A), (rnd(seed, x, y) - 0.5) * 0.2) } else if n < 0.25 { tone(hex(0xA88F22), 0.0) } else { p }
                });
                i
            }
            "cinnabar" => {
                let mut i = rock(c, k, 0.6, seed);
                i.map(|x, y, p| {
                    let n = vnoise(seed ^ 0x52, x as f32, y as f32 + x as f32 * 0.7, 3.0, 16);
                    if n > 0.72 { hex(0xD8605A) } else if n < 0.2 { hex(0x6E1E22) } else { p }
                });
                i
            }
            "coal" | "iron" | "gold" | "diamond" | "emerald" | "lapis" | "redstone" | "netherite" | "copper" | "resin" => {
                let mut i = rock(c, k, 0.3, seed);
                bevel(&mut i, 0.22);
                i
            }
            _ => rock(c, k, 0.45, seed),
        });
    }
    let (style, base) = if let Some(b) = name.strip_prefix("polished_") {
        ("polished", b)
    } else if let Some(b) = name.strip_prefix("chiseled_") {
        ("chiseled", b)
    } else if let Some(b) = name.strip_prefix("cracked_") {
        ("cracked", b)
    } else if let Some(b) = name.strip_prefix("mossy_") {
        ("mossy", b)
    } else if let Some(b) = name.strip_prefix("smooth_") {
        ("smooth", b)
    } else if let Some(b) = name.strip_prefix("cut_") {
        ("cut", b)
    } else if let Some(b) = name.strip_prefix("cobbled_") {
        ("cobbled", b)
    } else {
        ("", name)
    };
    let (shape, base) = if let Some(b) = base.strip_suffix("_bricks") {
        ("bricks", b)
    } else if let Some(b) = base.strip_suffix("_tiles") {
        ("tiles", b)
    } else if let Some(b) = base.strip_suffix("_pillar_top") {
        ("pillar_top", b)
    } else if let Some(b) = base.strip_suffix("_pillar") {
        ("pillar", b)
    } else if let Some(b) = base.strip_suffix("_top") {
        ("top", b)
    } else if let Some(b) = base.strip_suffix("_bottom") {
        ("bottom", b)
    } else if let Some(b) = base.strip_suffix("_block") {
        ("", b)
    } else {
        ("", base)
    };
    let base = if base == "cobblestone" { "stone" } else { base };
    let (c, k) = direct(base)?;
    let style = if name.contains("cobblestone") { "cobbled" } else { style };
    let mut img = match (style, shape) {
        (_, "bricks") => stone_bricks(c, k, seed),
        (_, "tiles") => bricks(c, tone(c, -0.45), 8, 4, k * 1.1, seed),
        (_, "pillar") => {
            let mut i = rock(c, k * 0.6, 0.2, seed);
            i.map(|x, _, p| if x % 4 == 1 { tone(p, -0.2) } else if x % 4 == 2 { tone(p, 0.12) } else { p });
            i
        }
        (_, "pillar_top") => {
            let mut i = rock(c, k * 0.6, 0.2, seed);
            i.outline(1, 1, 14, 14, tone(c, -0.2));
            i.outline(4, 4, 8, 8, tone(c, -0.12));
            i
        }
        ("cobbled", _) => cobble(c, (k * 2.2).min(0.6), 11, seed),
        ("polished", _) => {
            let mut i = rock(c, k * 0.55, 0.2, seed);
            bevel(&mut i, 0.2);
            i
        }
        ("chiseled", _) => chiseled(c, k, seed),
        ("smooth", _) | (_, "top") | (_, "bottom") => rock(c, k * 0.4, 0.3, seed),
        ("cut", _) => {
            let mut i = rock(c, k * 0.4, 0.3, seed);
            i.outline(0, 0, 16, 16, tone(c, -0.14));
            i.rect(0, 0, 16, 1, tone(c, 0.12));
            i
        }
        ("", "") if name.ends_with("_block") => {
            let mut i = rock(c, k, 0.3, seed);
            bevel(&mut i, 0.22);
            i
        }
        ("", "") if name.starts_with("sandstone") || name.starts_with("red_sandstone") => rock(c, k, 0.4, seed),
        ("cracked", _) | ("mossy", _) => rock(c, k, 0.45, seed),
        _ => return None,
    };
    if base == "sandstone" || base == "red_sandstone" {
        if shape == "" && style == "" {
            for y in [3, 8, 12] {
                img.map(|x, yy, p| if yy == y + ((hash_str(name) >> (x / 5)) & 1) as i32 { tone(p, -0.12) } else { p });
            }
            img.rect(0, 0, 16, 1, tone(c, -0.14));
        }
    }
    match style {
        "cracked" => cracks(&mut img, seed),
        "mossy" => moss_over(&mut img, seed, 0.3),
        _ => {}
    }
    Some(img)
}

fn wood_texture(name: &str, seed: u64) -> Option<Img> {
    let stripped = name.starts_with("stripped_");
    let rest = name.strip_prefix("stripped_").unwrap_or(name);
    for w in WOODS.iter() {
        let Some(part) = rest.strip_prefix(w.name).and_then(|s| s.strip_prefix('_')) else { continue };
        let (planks_c, bark_c, inner) = (hex(w.planks), hex(w.bark), hex(w.inner));
        return Some(match part {
            "planks" => planks(planks_c, seed),
            "log" if stripped => bark(inner, 0.45, seed),
            "log" => {
                let mut i = bark(bark_c, if w.name == "birch" { 0.25 } else { 1.0 }, seed);
                if w.name == "birch" {
                    let mut rng = Rng::new(seed);
                    for _ in 0..9 {
                        let (x, y, len) = (rng.range(0, 15), rng.range(0, 15), rng.range(2, 5));
                        for dx in 0..len {
                            i.setw(x + dx, y, hex(0x2F2B28));
                        }
                    }
                }
                i
            }
            "log_top" => rings(if stripped { inner } else { bark_c }, inner, seed),
            "leaves" => match w.leaves {
                None => leaves(hex(0xB4B4B4), 0.30, seed),
                Some(c) => {
                    let mut i = leaves(hex(c), 0.30, seed);
                    if w.name == "cherry" {
                        i.map(|x, y, p| if p[3] > 0 && rnd(seed ^ 3, x, y) > 0.86 { hex(0xFBE3EC) } else { p });
                    }
                    i
                }
            },
            "sapling" => {
                let leaf = w.leaves.map(hex).unwrap_or(GREEN);
                let mut i = Img::tile();
                i.rect(7, 9, 2, 7, bark_c);
                let mut rng = Rng::new(seed);
                for y in 2..11 {
                    let half = 1 + (5 - (y - 6i32).abs()).max(0);
                    for x in 8 - half..8 + half {
                        if rng.chance(0.78) {
                            i.set(x, y, tone(leaf, (rng.f32() - 0.5) * 0.4));
                        }
                    }
                }
                i
            }
            "door_top" | "door_bottom" | "trapdoor" => {
                let mut i = planks(planks_c, seed);
                i.outline(0, 0, 16, 16, tone(planks_c, -0.38));
                if part == "door_top" {
                    i.rect(3, 3, 4, 5, CLEAR);
                    i.rect(9, 3, 4, 5, CLEAR);
                    i.outline(2, 2, 6, 7, tone(planks_c, -0.3));
                    i.outline(8, 2, 6, 7, tone(planks_c, -0.3));
                    i.rect(1, 12, 2, 2, hex(0x3A3A3A));
                } else if part == "trapdoor" {
                    for (x, y) in [(3, 3), (9, 3), (3, 9), (9, 9)] {
                        i.rect(x, y, 4, 4, CLEAR);
                        i.outline(x - 1, y - 1, 6, 6, tone(planks_c, -0.3));
                    }
                } else {
                    i.outline(2, 3, 12, 10, tone(planks_c, -0.25));
                }
                i
            }
            _ => return None,
        });
    }
    None
}

fn blades(seed: u64, count: i32, min_h: i32, max_h: i32, col: Col) -> Img {
    let mut img = Img::tile();
    let mut rng = Rng::new(seed);
    for _ in 0..count {
        let mut x = rng.range(1, 14) as f32;
        let h = rng.range(min_h, max_h);
        let lean = (rng.f32() - 0.5) * 0.7;
        let c = tone(col, (rng.f32() - 0.5) * 0.45);
        for i in 0..h {
            img.set(x.round() as i32, 15 - i, tone(c, i as f32 / h as f32 * 0.25));
            x += lean * (i as f32 / h as f32);
        }
    }
    img
}

fn flower(petal: Col, centre: Col, h: i32, seed: u64) -> Img {
    let mut img = Img::tile();
    let top = 15 - h;
    for y in top + 2..16 {
        img.set(7 + ((y / 4) % 2), y, STEM);
    }
    img.set(6, 12, tone(STEM, 0.15));
    img.set(5, 11, tone(STEM, 0.15));
    img.set(9, 13, tone(STEM, -0.1));
    img.set(10, 12, tone(STEM, -0.1));
    let mut rng = Rng::new(seed);
    for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, -1), (0, 1), (1, 1), (-1, -1), (1, -1), (-1, 1), (2, 0), (-2, 0)] {
        if dx == 0 && dy == 0 {
            img.set(7, top + 2, centre);
        } else if rng.chance(0.82) {
            img.set(7 + dx, top + 2 + dy, tone(petal, (rng.f32() - 0.5) * 0.3));
        }
    }
    img
}

fn crop(stage: i32, stages: i32, ripe: Col, seed: u64) -> Img {
    let t = stage as f32 / (stages - 1) as f32;
    let h = 3 + (t * 11.0) as i32;
    let col = mix(hex(0x3F9A2C), ripe, (t * 1.3 - 0.3).clamp(0.0, 1.0));
    let mut img = Img::tile();
    let mut rng = Rng::new(seed);
    for sx in [2, 5, 8, 11, 14] {
        let hh = (h + rng.range(-1, 1)).clamp(2, 15);
        for i in 0..hh {
            let x = sx + if i > hh / 2 && rng.chance(0.3) { rng.range(-1, 1) } else { 0 };
            img.set(x, 15 - i, tone(col, (rng.f32() - 0.5) * 0.3));
        }
        if t > 0.8 {
            img.rect(sx - 1, 15 - hh, 2, 3, tone(ripe, 0.12));
        }
    }
    img
}

fn framed(base: Img, frame: Col) -> Img {
    let mut i = base;
    i.outline(0, 0, 16, 16, frame);
    i
}

pub fn generate(name: &str) -> Option<Img> {
    if let Some(item) = name.strip_prefix("item/") {
        return super::itemtex::generate(item);
    }
    if let Some(mob) = name.strip_prefix("mob/") {
        return super::mobtex::generate(mob);
    }
    let seed = hash_str(name);
    if let Some(i) = fluid(name, 0) {
        return Some(i);
    }
    let dirt = || rock(hex(0x866043), 0.30, 0.9, hash_str("dirt"));
    let stone = || rock(hex(0x7F7F7F), 0.22, 0.45, hash_str("stone"));
    let img = match name {
        "dirt" => dirt(),
        "coarse_dirt" => cobble(hex(0x775539), 0.34, 14, seed),
        "rooted_dirt" => {
            let mut i = dirt();
            i.overlay(&blades(seed, 7, 4, 9, hex(0xB89A78)).flipped_v());
            i
        }
        "mud" => rock(hex(0x3C3A3D), 0.2, 0.5, seed),
        "grass_block_top" | "moss_block" | "mycelium_top" | "podzol_top" => {
            let base = match name {
                "grass_block_top" => hex(0x969696),
                "moss_block" => hex(0x5B7A2D),
                "mycelium_top" => hex(0x6F6369),
                _ => hex(0x5B3F18),
            };
            rock(base, 0.34, 1.0, seed)
        }
        "grass_block_side" => dirt(),
        "grass_block_side_overlay" | "grass_block_snow" | "mycelium_side" | "podzol_side" => {
            let (col, back) = match name {
                "grass_block_side_overlay" => (hex(0x969696), None),
                "grass_block_snow" => (hex(0xF4FAFA), Some(dirt())),
                "mycelium_side" => (hex(0x6F6369), Some(dirt())),
                _ => (hex(0x5B3F18), Some(dirt())),
            };
            let mut i = back.unwrap_or_else(Img::tile);
            for x in 0..16 {
                let depth = 2 + (rnd(seed, x, 0) * 3.2) as i32;
                for y in 0..depth {
                    i.set(x, y, tone(col, (rnd(seed, x, y + 1) - 0.5) * 0.34));
                }
            }
            i
        }
        "sand" => speckle(hex(0xDBCFA3), 0.16, seed),
        "red_sand" => speckle(hex(0xBE6621), 0.16, seed),
        "gravel" => cobble(hex(0x84807E), 0.5, 26, seed),
        "clay" => rock(hex(0xA0A6B3), 0.1, 0.4, seed),
        "snow" => speckle(hex(0xF4FAFA), 0.05, seed),
        "ice" => {
            let mut i = rock(hex(0x91B7FD), 0.14, 0.2, seed);
            i.map(|x, y, c| {
                let streak = (x + y * 2) % 11 == 0 || (x * 3 - y) % 13 == 0;
                let c = if streak { tone(c, 0.3) } else { c };
                [c[0], c[1], c[2], 200]
            });
            i
        }
        "packed_ice" => rock(hex(0x8DB4FA), 0.14, 0.3, seed),
        "blue_ice" => rock(hex(0x74A8FD), 0.12, 0.3, seed),
        "glass" => {
            let mut i = Img::tile();
            i.outline(0, 0, 16, 16, hex(0xD6ECF2));
            for (x, y) in [(3, 2), (4, 3), (5, 4), (11, 10), (12, 11), (10, 3), (3, 12)] {
                i.set(x, y, hexa(0xFFFFFF, 220));
            }
            i
        }
        "tinted_glass" => {
            let mut i = Img::filled(hexa(0x2C2630, 200));
            i.outline(0, 0, 16, 16, hex(0x1F1A22));
            i
        }
        "bricks" => bricks(hex(0x96614F), hex(0xA9A49B), 8, 4, 0.22, seed),
        "nether_bricks" => bricks(hex(0x2D1719), hex(0x1A0D0F), 8, 4, 0.3, seed),
        "red_nether_bricks" => bricks(hex(0x4A0D10), hex(0x2A0709), 8, 4, 0.3, seed),
        "mud_bricks" => bricks(hex(0x8A6952), hex(0x6B5140), 8, 8, 0.2, seed),
        "mossy_cobblestone" => {
            let mut i = cobble(hex(0x7A7A7A), 0.5, 11, hash_str("cobblestone"));
            moss_over(&mut i, seed, 0.32);
            i
        }
        "soul_sand" => rock(hex(0x513E33), 0.4, 1.0, seed),
        "soul_soil" => rock(hex(0x4B3A2E), 0.22, 0.6, seed),
        "glowstone" => {
            let mut i = rock(hex(0xAB8354), 0.4, 0.8, seed);
            i.map(|x, y, c| if vnoise(seed, x as f32, y as f32, 3.0, 16) > 0.58 { tone(hex(0xF9D49C), rnd(seed, x, y) * 0.5) } else { c });
            i
        }
        "sea_lantern" => {
            let mut i = rock(hex(0xACC8BE), 0.2, 0.4, seed);
            i.outline(0, 0, 16, 16, hex(0x7FA79B));
            i.rect(4, 4, 8, 8, hex(0xE6F3EC));
            i
        }
        "shroomlight" => rock(hex(0xF09A45), 0.3, 0.8, seed),
        "magma" => {
            let mut i = rock(hex(0x5A2A1C), 0.4, 0.8, seed);
            i.map(|x, y, c| {
                let n = (vnoise(seed, x as f32, y as f32, 4.0, 4) - 0.5).abs();
                if n < 0.05 { hex(0xF8B030) } else if n < 0.1 { hex(0xD8581A) } else { c }
            });
            i
        }
        "sculk" => {
            let mut i = rock(hex(0x0D1C22), 0.5, 0.8, seed);
            i.map(|x, y, c| if rnd(seed ^ 9, x, y) > 0.93 { hex(0x2FB6C2) } else { c });
            i
        }
        "potent_sulfur" => {
            let mut i = mineral_texture("sulfur", hash_str("sulfur")).unwrap();
            i.map(|x, y, c| {
                let d = (((x - 8) * (x - 8) + (y - 8) * (y - 8)) as f32).sqrt() + rnd(seed, x, y) * 1.5;
                if d < 2.5 { hex(0x4A3A12) } else if d < 4.5 { hex(0xF6F2A8) } else if (d as i32) == 6 { tone(c, -0.25) } else { c }
            });
            i
        }
        "sulfur_spike" => {
            let mut i = Img::tile();
            for y in 0..16 {
                let half = 1 + y / 3;
                for x in 8 - half..8 + half {
                    i.set(x, y, tone(hex(0xD8C640), (rnd(seed, x, y) - 0.5) * 0.4 - if x == 8 - half { 0.2 } else { 0.0 }));
                }
            }
            i
        }
        "pointed_dripstone" => {
            let mut i = Img::tile();
            for y in 0..16 {
                let half = 1 + y / 3;
                for x in 8 - half..8 + half {
                    i.set(x, y, tone(hex(0x866B5B), (rnd(seed, x, y) - 0.5) * 0.4));
                }
            }
            i
        }
        "bookshelf" => {
            let mut i = planks(hex(0xB8945F), seed);
            let mut rng = Rng::new(seed);
            for shelf in 0..2 {
                let y0 = 1 + shelf * 8;
                i.rect(1, y0, 14, 6, hex(0x3A2A18));
                let mut x = 1;
                while x < 15 {
                    let w = rng.range(1, 2);
                    let col = *rng.pick(&[0x9A2A22, 0x2F6B3A, 0x2A4A8A, 0xB89A3A, 0x6A3A7A, 0x8A5A2A]);
                    let h = rng.range(4, 6);
                    i.rect(x, y0 + 6 - h, w.min(15 - x), h, tone(hex(col), (rng.f32() - 0.5) * 0.2));
                    x += w;
                }
            }
            i
        }
        "crafting_table_top" => {
            let mut i = planks(hex(0xB8945F), seed);
            i.outline(0, 0, 16, 16, hex(0x5A4020));
            for k in [5, 10] {
                i.rect(k, 1, 1, 14, hex(0x7A5A30));
                i.rect(1, k, 14, 1, hex(0x7A5A30));
            }
            i
        }
        "crafting_table_side" | "crafting_table_front" => {
            let mut i = planks(hex(0xB8945F), seed);
            i.rect(0, 0, 16, 3, hex(0x6B4E28));
            i.rect(0, 3, 16, 1, hex(0x4A3418));
            if name.ends_with("front") {
                i.rect(3, 6, 2, 6, hex(0x8C8C8C));
                i.rect(2, 5, 4, 2, hex(0x6A6A6A));
                i.rect(10, 5, 1, 8, hex(0x6B4E28));
                i.rect(9, 5, 4, 2, hex(0xA0A0A0));
            } else {
                i.rect(4, 6, 8, 1, hex(0x8C8C8C));
                i.rect(4, 7, 1, 5, hex(0x6A6A6A));
                i.rect(11, 7, 1, 5, hex(0x6A6A6A));
            }
            i
        }
        "furnace_top" | "furnace_side" => framed(cobble(hex(0x7A7A7A), 0.4, 9, seed), hex(0x4A4A4A)),
        "furnace_front" | "furnace_front_on" => {
            let mut i = framed(cobble(hex(0x7A7A7A), 0.4, 9, hash_str("furnace_side")), hex(0x4A4A4A));
            i.rect(3, 3, 10, 4, hex(0x2A2A2A));
            i.rect(3, 9, 10, 5, hex(0x1A1A1A));
            if name.ends_with("_on") {
                i.rect(4, 11, 8, 3, hex(0xF08A1A));
                i.rect(5, 10, 2, 1, hex(0xFFD84A));
                i.rect(9, 10, 2, 1, hex(0xFFD84A));
                i.rect(6, 12, 4, 2, hex(0xFFE88A));
            }
            i
        }
        "chest_top" | "chest_side" | "chest_front" => {
            let mut i = planks(hex(0xA87830), seed);
            i.outline(0, 0, 16, 16, hex(0x3A2810));
            if name != "chest_top" {
                i.rect(0, 5, 16, 1, hex(0x3A2810));
            }
            if name == "chest_front" {
                i.rect(7, 4, 2, 4, hex(0xC8C8C8));
                i.rect(7, 7, 2, 1, hex(0x6A6A6A));
            }
            i
        }
        "tnt_side" => {
            let mut i = Img::tile();
            i.map(|x, _, _| tone(hex(0xC83A2A), if x % 4 == 3 { -0.3 } else { ((x % 4) as f32 - 1.0) * 0.06 }));
            i.rect(0, 5, 16, 6, hex(0xE8E4DC));
            for (x, w) in [(2, 3), (7, 3), (12, 3)] {
                i.rect(x, 6, w, 1, hex(0x202020));
                i.rect(x + 1, 6, 1, 4, hex(0x202020));
            }
            i.rect(7, 6, 1, 4, hex(0x202020));
            i.rect(9, 6, 1, 4, hex(0x202020));
            i.set(8, 7, hex(0x202020));
            i
        }
        "tnt_top" | "tnt_bottom" => {
            let mut i = Img::tile();
            i.map(|x, y, _| {
                let (cx, cy) = (x % 4, y % 4);
                if cx == 3 || cy == 3 { hex(0x8A2A20) } else { hex(0xC83A2A) }
            });
            if name == "tnt_top" {
                i.rect(6, 6, 4, 4, hex(0x3A3A3A));
                i.rect(7, 7, 2, 2, hex(0xD8D0C0));
            }
            i
        }
        "pumpkin_side" | "pumpkin_top" | "carved_pumpkin" | "jack_o_lantern" => {
            let mut i = Img::tile();
            i.map(|x, y, _| tone(hex(0xD8801A), if x % 4 == 0 { -0.28 } else { (rnd(seed, x, y) - 0.5) * 0.16 }));
            if name == "pumpkin_top" {
                i.rect(6, 6, 4, 4, hex(0x5A7A2A));
            } else if name != "pumpkin_side" {
                let glow = if name == "jack_o_lantern" { hex(0xFFE060) } else { hex(0x3A1E08) };
                for (x, y, w, h) in [(3, 4, 3, 3), (10, 4, 3, 3), (4, 10, 8, 2), (3, 9, 2, 2), (11, 9, 2, 2)] {
                    i.rect(x, y, w, h, glow);
                }
            }
            i
        }
        "melon_side" | "melon_top" => {
            let mut i = Img::tile();
            i.map(|x, y, _| tone(hex(0x6FA02A), if (x + (y / 5) % 2) % 4 == 0 { -0.3 } else { (rnd(seed, x, y) - 0.5) * 0.2 }));
            i
        }
        "hay_block_side" | "hay_block_top" => {
            let mut i = bark(hex(0xC8A21E), 0.6, seed);
            if name.ends_with("side") {
                i.rect(0, 3, 16, 2, hex(0x9A2E22));
                i.rect(0, 11, 16, 2, hex(0x9A2E22));
            } else {
                i = rings(hex(0xA8861A), hex(0xC8A21E), seed);
            }
            i
        }
        "farmland" | "farmland_moist" => {
            let base = if name == "farmland" { hex(0x7A5234) } else { hex(0x4A2E1C) };
            let mut i = rock(base, 0.2, 0.6, seed);
            i.map(|_, y, c| if y % 4 == 0 { tone(c, -0.25) } else { c });
            i
        }
        "dirt_path_top" => rock(hex(0x94793F), 0.22, 0.8, seed),
        "dirt_path_side" => {
            let mut i = dirt();
            i.rect(0, 0, 16, 2, hex(0x94793F));
            i
        }
        "cactus_side" | "cactus_top" | "cactus_bottom" => {
            let mut i = Img::tile();
            let top = name != "cactus_side";
            i.map(|x, y, _| {
                let rib = if top { (x.min(15 - x)).min(y.min(15 - y)) % 3 == 0 } else { x % 3 == 1 };
                tone(hex(if name == "cactus_bottom" { 0xC8C08A } else { 0x4F8A2C }), if rib { -0.25 } else { (rnd(seed, x, y) - 0.5) * 0.14 })
            });
            if !top {
                for (x, y) in [(2, 2), (8, 5), (13, 9), (5, 12), (11, 14)] {
                    i.set(x, y, hex(0xE8E8D0));
                }
            }
            i
        }
        "sugar_cane" => {
            let mut i = Img::tile();
            for (x, shade_v) in [(3, 0.0), (8, 0.12), (12, -0.1)] {
                for y in 0..16 {
                    i.set(x, y, tone(hex(0x8FC06A), shade_v));
                    i.set(x + 1, y, tone(hex(0x6FA04A), shade_v));
                    if (y + x) % 5 == 0 {
                        i.set(x, y, tone(hex(0x4F7A2A), shade_v));
                        i.set(x + 1, y, tone(hex(0x4F7A2A), shade_v));
                    }
                }
            }
            i
        }
        "short_grass" => blades(seed, 13, 5, 11, hex(0xB0B0B0)),
        "tall_grass_bottom" => blades(seed, 15, 13, 16, hex(0xB0B0B0)),
        "tall_grass_top" => blades(seed, 12, 6, 13, hex(0xB0B0B0)),
        "fern" => {
            let mut i = blades(seed, 5, 9, 14, hex(0xA0A0A0));
            let src = i.clone();
            i.map(|x, y, c| if c[3] == 0 && y % 2 == 0 && (src.get(x - 1, y)[3] > 0 || src.get(x + 1, y)[3] > 0) && y < 14 { hex(0xB8B8B8) } else { c });
            i
        }
        "dead_bush" => {
            let mut i = blades(seed, 6, 6, 12, hex(0x8A5A2A));
            i.rect(7, 11, 2, 5, hex(0x6B4420));
            i
        }
        "seagrass" => blades(seed, 9, 7, 14, hex(0x3E9A3A)),
        "kelp" => {
            let mut i = Img::tile();
            for y in 0..16 {
                let x = 7 + ((y as f32 * 0.8).sin() * 2.0) as i32;
                i.rect(x, y, 3, 1, tone(hex(0x4F8A2C), (rnd(seed, x, y) - 0.5) * 0.3));
            }
            i
        }
        "vine" | "glow_lichen" => {
            let mut i = leaves(if name == "vine" { hex(0xB4B4B4) } else { hex(0x6FA8A0) }, 0.55, seed);
            i.map(|x, y, c| if c[3] > 0 && rnd(seed ^ 5, x / 2, y) > 0.8 { CLEAR } else { c });
            i
        }
        "lily_pad" => {
            let mut i = Img::tile();
            i.map(|x, y, _| {
                let d = (((x - 8) * (x - 8) + (y - 8) * (y - 8)) as f32).sqrt();
                if d < 7.0 && !(x >= 8 && (y - 8).abs() <= (x - 8) / 3) { tone(hex(0xA8A8A8), (rnd(seed, x, y) - 0.5) * 0.25) } else { CLEAR }
            });
            i
        }
        "brown_mushroom" | "red_mushroom" => {
            let cap = if name.starts_with("red") { hex(0xD82A22) } else { hex(0x9A7458) };
            let mut i = Img::tile();
            i.rect(7, 11, 2, 5, hex(0xD8D0C0));
            for y in 7..11 {
                let half = 1 + (y - 6);
                i.rect(8 - half, y, half * 2, 1, tone(cap, (10 - y) as f32 * 0.06));
            }
            if name.starts_with("red") {
                i.set(6, 9, WHITE);
                i.set(9, 8, WHITE);
                i.set(11, 10, WHITE);
            }
            i
        }
        "sunflower_bottom" | "sunflower_top" => {
            let mut i = Img::tile();
            i.rect(7, 0, 2, 16, STEM);
            if name.ends_with("top") {
                i.map(|x, y, c| {
                    let d = (((x - 8) * (x - 8) + (y - 6) * (y - 6)) as f32).sqrt();
                    if d < 2.5 { hex(0x5A3A18) } else if d < 5.5 { tone(hex(0xF8D030), (rnd(seed, x, y) - 0.5) * 0.3) } else { c }
                });
            } else {
                i.rect(4, 6, 3, 2, tone(STEM, 0.15));
                i.rect(9, 10, 4, 2, tone(STEM, -0.1));
            }
            i
        }
        "torch" | "soul_torch" | "redstone_torch" => {
            let mut i = Img::tile();
            i.rect(7, 6, 2, 10, hex(0x8A6A3A));
            i.rect(7, 6, 1, 10, hex(0x6B4E28));
            let (hot, mid) = match name {
                "torch" => (hex(0xFFF2A0), hex(0xF8A020)),
                "soul_torch" => (hex(0xC8FBFF), hex(0x38C8E0)),
                _ => (hex(0xFF7060), hex(0xD81810)),
            };
            i.rect(7, 6, 2, 2, mid);
            i.rect(7, 6, 2, 1, hot);
            i
        }
        "ladder" => {
            let mut i = Img::tile();
            i.rect(2, 0, 2, 16, hex(0x8A6A3A));
            i.rect(12, 0, 2, 16, hex(0x8A6A3A));
            for y in [2, 6, 10, 14] {
                i.rect(2, y, 12, 2, hex(0xA88248));
                i.rect(2, y + 1, 12, 1, hex(0x6B4E28));
            }
            i
        }
        "iron_bars" => {
            let mut i = Img::tile();
            for x in [1, 5, 9, 13] {
                i.rect(x, 0, 2, 16, hex(0x8C8C8C));
                i.rect(x + 1, 0, 1, 16, hex(0x5A5A5A));
            }
            i.rect(0, 1, 16, 1, hex(0x6E6E6E));
            i.rect(0, 14, 16, 1, hex(0x6E6E6E));
            i
        }
        "iron_door_top" | "iron_door_bottom" | "iron_trapdoor" => {
            let mut i = rock(hex(0xC8C8C8), 0.08, 0.3, seed);
            i.outline(0, 0, 16, 16, hex(0x7A7A7A));
            if name != "iron_door_bottom" {
                i.rect(3, 3, 4, 4, CLEAR);
                i.rect(9, 3, 4, 4, CLEAR);
            }
            i
        }
        "bed_top" | "bed_side" | "bed_pillow" => {
            let mut i = Img::filled(hex(if name == "bed_pillow" { 0xE8E8E8 } else { 0xA12722 }));
            i.map(|x, y, c| tone(c, (rnd(seed, x, y) - 0.5) * 0.1));
            if name == "bed_side" {
                i.rect(0, 10, 16, 6, hex(0xB8945F));
            }
            i
        }
        "ancient_debris_side" | "ancient_debris_top" => bark(hex(0x5E4138), 0.8, seed),
        "nether_quartz_ore" => ore(rock(hex(0x70332F), 0.36, 1.0, hash_str("netherrack")), hex(0xEBE6DE), 6, seed),
        "nether_gold_ore" => ore(rock(hex(0x70332F), 0.36, 1.0, hash_str("netherrack")), hex(0xFCEE4B), 6, seed),
        "spawner" => {
            let mut i = Img::tile();
            i.map(|x, y, _| if x % 4 == 0 || y % 4 == 0 { hex(0x1F2A33) } else { CLEAR });
            i
        }
        "slime_block" => Img::filled(hexa(0x73C262, 190)),
        "honey_block" => Img::filled(hexa(0xF4A51C, 210)),
        "honeycomb_block" => bricks(hex(0xE89A1A), hex(0xB86A0A), 4, 4, 0.2, seed),
        "sponge" => {
            let mut i = rock(hex(0xC8C04A), 0.3, 0.8, seed);
            i.map(|x, y, c| if rnd(seed ^ 7, x, y) > 0.8 { hex(0x8A8228) } else { c });
            i
        }
        "note_block" | "jukebox_side" => framed(planks(hex(0x6A4428), seed), hex(0x3A2412)),
        "jukebox_top" => {
            let mut i = framed(planks(hex(0x6A4428), seed), hex(0x3A2412));
            i.rect(5, 5, 6, 6, hex(0x1A1A1A));
            i.rect(7, 7, 2, 2, hex(0xC8A21E));
            i
        }
        "cave_vines" => {
            let mut i = blades(seed, 6, 9, 15, hex(0x5E8A2E)).flipped_v();
            for (x, y) in [(4, 5), (9, 9), (12, 4), (6, 12)] {
                i.rect(x, y, 2, 2, hex(0xF2A21B));
                i.set(x, y, hex(0xFFD86A));
            }
            i
        }
        "mushroom_stem" => rock(hex(0xD8D0C0), 0.12, 0.5, seed),
        "brown_mushroom_block" => rock(hex(0x9A7458), 0.14, 0.5, seed),
        "red_mushroom_block" => {
            let mut i = rock(hex(0xC82A22), 0.14, 0.5, seed);
            for (x, y) in [(2, 3), (9, 2), (5, 9), (12, 11)] {
                i.rect(x, y, 3, 3, hex(0xE8E4DC));
            }
            i
        }
        "packed_mud" => rock(hex(0x8E6A4F), 0.14, 0.6, seed),
        "debug" => missing(),
        _ => {
            if let Some(c) = name.strip_suffix("_wool").and_then(dye) {
                let mut i = rock(c, 0.16, 0.9, seed);
                i.map(|x, y, p| if (x + y * 3) % 4 == 0 { tone(p, -0.08) } else { p });
                return Some(i);
            }
            if let Some(c) = name.strip_suffix("_concrete_powder").and_then(dye) {
                return Some(speckle(c, 0.22, seed));
            }
            if let Some(c) = name.strip_suffix("_concrete").and_then(dye) {
                return Some(speckle(c, 0.04, seed));
            }
            if let Some(c) = name.strip_suffix("_terracotta").and_then(dye) {
                return Some(rock(mix(c, hex(0x985E44), 0.55), 0.08, 0.3, seed));
            }
            if let Some(c) = name.strip_suffix("_stained_glass").and_then(dye) {
                let mut i = Img::filled([c[0], c[1], c[2], 120]);
                i.outline(0, 0, 16, 16, [c[0], c[1], c[2], 200]);
                return Some(i);
            }
            if let Some((_, petal, centre, h)) = FLOWERS.iter().find(|f| f.0 == name) {
                return Some(flower(hex(*petal), hex(*centre), *h, seed));
            }
            if name == "particle_puff" || name == "particle_drop" {
                // Soft round blob (smoke) and a small droplet, both white so they can be tinted.
                let mut i = Img::tile();
                let radius = if name == "particle_puff" { 7.5f32 } else { 3.5 };
                for y in 0..16 {
                    for x in 0..16 {
                        let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
                        let d = (dx * dx + dy * dy).sqrt();
                        if d <= radius {
                            let shade = 255 - ((d / radius) * 60.0) as u8 - if (x + y * 3) % 5 == 0 { 20 } else { 0 };
                            i.set(x, y, [shade, shade, shade, 255]);
                        }
                    }
                }
                return Some(i);
            }
            if let Some(stage) = name.strip_prefix("destroy_stage_").and_then(|n| n.parse::<i32>().ok()) {
                // Cracks spread from the centre; each stage adds more and longer fractures.
                let mut i = Img::tile();
                let mut rng = Rng::new(0xC4AC);
                for walk in 0..(3 + stage * 2) {
                    let (mut x, mut y) = (8.0f32 + rng.range(-2, 2) as f32, 8.0f32 + rng.range(-2, 2) as f32);
                    let angle = rng.f32() * std::f32::consts::TAU;
                    let (mut dx, mut dy) = (angle.cos(), angle.sin());
                    let len = 2 + stage + (walk % 3);
                    for _ in 0..len {
                        i.set(x as i32, y as i32, hexa(0x101010, 170));
                        x += dx;
                        y += dy;
                        let turn = (rng.f32() - 0.5) * 0.9;
                        (dx, dy) = (dx * turn.cos() - dy * turn.sin(), dx * turn.sin() + dy * turn.cos());
                    }
                }
                return Some(i);
            }
            if let Some(rest) = name.strip_prefix("wheat_stage") {
                return Some(crop(rest.parse().ok()?, 8, hex(0xD8B43A), seed));
            }
            for (prefix, stages, ripe) in [("carrots_stage", 4, 0x5FAF2F), ("potatoes_stage", 4, 0x4F9F3F), ("beetroots_stage", 4, 0x6FAF4F)] {
                if let Some(rest) = name.strip_prefix(prefix) {
                    return Some(crop(rest.parse().ok()?, stages, hex(ripe), seed));
                }
            }
            if let Some(base) = name.strip_suffix("_ore") {
                let (host, gem) = match base.strip_prefix("deepslate_") {
                    Some(g) => ("deepslate", g),
                    None => ("stone", base),
                };
                let gem = ore_colour(gem)?;
                let host_img = if host == "stone" { stone() } else { mineral_texture("deepslate", hash_str("deepslate"))? };
                return Some(ore(host_img, gem, 7, seed));
            }
            if let Some(metal) = name.strip_prefix("raw_").and_then(|n| n.strip_suffix("_block")) {
                let c = ore_colour(metal)?;
                return Some(cobble(tone(c, -0.15), 0.4, 12, seed));
            }
            return wood_texture(name, seed).or_else(|| mineral_texture(name, seed));
        }
    };
    Some(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_resolve() {
        for name in [
            "stone", "cobblestone", "polished_granite", "stone_bricks", "cracked_stone_bricks", "mossy_stone_bricks", "chiseled_stone_bricks",
            "deepslate_tiles", "cobbled_deepslate", "sandstone", "sandstone_top", "cut_sandstone", "quartz_pillar", "quartz_pillar_top",
            "oak_planks", "stripped_birch_log", "cherry_leaves", "dark_oak_log_top", "spruce_sapling", "oak_door_top", "red_wool",
            "light_blue_concrete", "lime_stained_glass", "black_terracotta", "diamond_ore", "deepslate_redstone_ore", "raw_iron_block",
            "gold_block", "poppy", "wheat_stage7", "water_still", "lava_flow", "sulfur", "polished_sulfur", "sulfur_bricks",
            "chiseled_cinnabar", "cinnabar_bricks", "potent_sulfur", "torch", "grass_block_side_overlay", "amethyst_block", "smooth_stone",
        ] {
            assert!(generate(name).is_some(), "no recipe for {name}");
        }
        assert!(generate("definitely_not_a_texture").is_none());
    }

    #[test]
    fn fluid_frames_animate() {
        let a = fluid("water_still", 0).unwrap();
        let b = fluid("water_still", 8).unwrap();
        assert_ne!(a.px, b.px);
        assert!(a.px.iter().all(|p| p[3] > 100 && p[3] < 255));
    }
}
