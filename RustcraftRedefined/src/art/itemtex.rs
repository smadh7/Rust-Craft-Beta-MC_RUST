//! Procedural item icons.  Shapes are drawn from a few primitives and coloured per material,
//! so tools, ingots, gems and food share a handful of recipes.

use super::img::*;
use super::palette::{dye, ore_colour};
use crate::util::rng::Rng;

/// Tool materials: name prefix and head colour.
pub const TOOL_MATERIALS: [(&str, u32); 6] = [("wooden", 0x9A7B4A), ("stone", 0x8C8C8C), ("iron", 0xDADADA), ("golden", 0xF2D43A), ("diamond", 0x4FE0D6), ("netherite", 0x4A4347)];

const STICK: Col = hex(0x7A5A30);
const OUTLINE: Col = hex(0x1E1A16);

fn line(img: &mut Img, x0: i32, y0: i32, x1: i32, y1: i32, c: Col) {
    let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for i in 0..=n {
        img.set(x0 + (x1 - x0) * i / n, y0 + (y1 - y0) * i / n, c);
    }
}

/// Darkens the lower-right side of every shape and adds a dark outline around it.
fn finish(mut img: Img) -> Img {
    let src = img.clone();
    let solid = |x: i32, y: i32| x >= 0 && y >= 0 && x < 16 && y < 16 && src.px[(y * 16 + x) as usize][3] > 0;
    img.map(|x, y, c| {
        if c[3] > 0 {
            if !solid(x + 1, y + 1) || !solid(x, y + 1) { tone(c, -0.22) } else if !solid(x - 1, y - 1) { tone(c, 0.2) } else { c }
        } else if solid(x + 1, y) || solid(x - 1, y) || solid(x, y + 1) || solid(x, y - 1) {
            OUTLINE
        } else {
            c
        }
    });
    img
}

fn tool(kind: &str, head: Col) -> Img {
    let mut i = Img::tile();
    if kind == "sword" {
        line(&mut i, 3, 12, 2, 13, STICK);
        line(&mut i, 3, 10, 5, 12, tone(head, -0.35));
        for k in 0..2 {
            line(&mut i, 5 + k, 10, 12 + k, 3, head);
        }
        line(&mut i, 6, 10, 13, 3, tone(head, 0.3));
        i.set(13, 2, head);
        return finish(i);
    }
    line(&mut i, 3, 13, 10, 6, STICK);
    line(&mut i, 4, 13, 11, 6, tone(STICK, -0.25));
    match kind {
        "pickaxe" => {
            for (x, y) in [(5, 3), (6, 2), (7, 2), (8, 2), (9, 2), (10, 3), (11, 4), (12, 5), (13, 6), (13, 7), (13, 8), (13, 9), (12, 10), (10, 4), (11, 5), (9, 3), (12, 6)] {
                i.set(x, y, head);
            }
        }
        "axe" => {
            i.rect(8, 2, 4, 5, head);
            i.rect(7, 3, 2, 4, head);
            i.rect(11, 4, 2, 4, tone(head, 0.2));
            i.set(6, 4, head);
        }
        "shovel" => {
            i.rect(10, 2, 4, 4, head);
            i.set(9, 3, head);
            i.set(12, 6, head);
            i.set(13, 2, CLEAR);
        }
        _ => {
            // hoe
            i.rect(7, 2, 6, 2, head);
            i.rect(11, 4, 2, 2, head);
        }
    }
    finish(i)
}

fn blob(col: Col, pts: &[(i32, i32, i32, i32)], seed: u64) -> Img {
    let mut i = Img::tile();
    for &(x, y, w, h) in pts {
        i.rect(x, y, w, h, col);
    }
    i.map(|x, y, c| if c[3] > 0 { tone(c, (rnd(seed, x, y) - 0.5) * 0.2) } else { c });
    finish(i)
}

fn ingot(col: Col) -> Img {
    let mut i = Img::tile();
    for y in 0..5 {
        i.rect(2 + y / 2, 6 + y, 11, 1, if y < 2 { tone(col, 0.3) } else { col });
    }
    finish(i)
}

fn gem(col: Col) -> Img {
    let mut i = Img::tile();
    for y in 0..10i32 {
        let half = if y < 3 { 2 + y } else { (9 - y).max(0) * 5 / 7 + 1 };
        i.rect(8 - half, 3 + y, half * 2, 1, if y < 3 { tone(col, 0.35) } else { col });
    }
    i.set(6, 5, WHITE);
    finish(i)
}

fn lump(col: Col, seed: u64) -> Img {
    blob(col, &[(4, 5, 8, 7), (6, 3, 5, 3), (3, 7, 2, 4), (11, 6, 2, 4)], seed)
}

fn dust(col: Col, seed: u64) -> Img {
    let mut i = Img::tile();
    let mut rng = Rng::new(seed);
    for y in 8..14 {
        let half = y - 6;
        for x in 8 - half..8 + half {
            if rng.chance(0.75) {
                i.set(x, y, tone(col, (rng.f32() - 0.5) * 0.5));
            }
        }
    }
    i
}

fn ball(col: Col, seed: u64) -> Img {
    let mut i = Img::tile();
    i.map(|x, y, _| {
        let d = (((x - 8) * (x - 8) + (y - 8) * (y - 8)) as f32).sqrt();
        if d < 5.2 { tone(col, 0.25 - d * 0.07 + (rnd(seed, x, y) - 0.5) * 0.1) } else { CLEAR }
    });
    finish(i)
}

fn bucket(content: Option<Col>) -> Img {
    let mut i = Img::tile();
    let metal = hex(0xB8B8C0);
    for y in 0..9 {
        let inset = y / 3;
        i.rect(3 + inset, 5 + y, 10 - inset * 2, 1, if y == 0 { tone(metal, 0.3) } else { metal });
    }
    if let Some(c) = content {
        i.rect(4, 5, 8, 2, c);
    } else {
        i.rect(4, 5, 8, 1, hex(0x5A5A62));
    }
    line(&mut i, 3, 4, 8, 2, tone(metal, -0.3));
    line(&mut i, 8, 2, 12, 4, tone(metal, -0.3));
    finish(i)
}

/// Icon for a plain item, or `None` if the name has no recipe.
pub fn generate(name: &str) -> Option<Img> {
    let seed = crate::util::rng::hash_str(name);
    for (m, c) in TOOL_MATERIALS {
        if let Some(kind) = name.strip_prefix(m).and_then(|s| s.strip_prefix('_')) {
            if matches!(kind, "pickaxe" | "axe" | "shovel" | "hoe" | "sword") {
                return Some(tool(kind, hex(c)));
            }
        }
    }
    if let Some(c) = name.strip_suffix("_dye").and_then(dye) {
        return Some(blob(c, &[(5, 6, 6, 5), (6, 4, 4, 2), (4, 8, 2, 2)], seed));
    }
    if let Some(metal) = name.strip_prefix("raw_") {
        if let Some(c) = ore_colour(metal) {
            return Some(lump(c, seed));
        }
    }
    let img = match name {
        "stick" => {
            let mut i = Img::tile();
            line(&mut i, 4, 12, 11, 4, STICK);
            line(&mut i, 5, 12, 12, 4, tone(STICK, -0.25));
            finish(i)
        }
        "coal" => lump(hex(0x2A2A2A), seed),
        "charcoal" => lump(hex(0x4A3A2E), seed),
        "flint" => blob(hex(0x3A3A3E), &[(6, 3, 4, 3), (5, 5, 6, 5), (7, 10, 3, 3)], seed),
        "iron_ingot" => ingot(hex(0xDADADA)),
        "copper_ingot" => ingot(hex(0xD9764F)),
        "gold_ingot" => ingot(hex(0xF8D840)),
        "netherite_ingot" => ingot(hex(0x4A4347)),
        "brick" => ingot(hex(0xA85A48)),
        "nether_brick" => ingot(hex(0x3A1E22)),
        "diamond" => gem(hex(0x5DECF5)),
        "emerald" => gem(hex(0x2BD86A)),
        "lapis_lazuli" => lump(hex(0x2351BE), seed),
        "quartz" => gem(hex(0xEBE6DE)),
        "amethyst_shard" => gem(hex(0x9A6AD8)),
        "redstone" => dust(hex(0xE0100A), seed),
        "glowstone_dust" => dust(hex(0xF8D060), seed),
        "gunpowder" => dust(hex(0x6A6A6A), seed),
        "sugar" => dust(hex(0xF4F4F4), seed),
        "bone_meal" => dust(hex(0xE8E4D0), seed),
        "sulfur_dust" => dust(hex(0xE2D048), seed),
        "gold_nugget" => blob(hex(0xF8D840), &[(6, 7, 4, 3), (7, 6, 2, 1)], seed),
        "iron_nugget" => blob(hex(0xDADADA), &[(6, 7, 4, 3), (7, 6, 2, 1)], seed),
        "clay_ball" => ball(hex(0xA0A6B3), seed),
        "snowball" => ball(hex(0xF4FAFA), seed),
        "slime_ball" => ball(hex(0x73C262), seed),
        "egg" => ball(hex(0xE8DCC0), seed),
        "apple" => {
            let mut i = ball(hex(0xD82A22), seed);
            i.rect(8, 1, 1, 3, STICK);
            i.rect(9, 2, 2, 1, hex(0x4F8A2C));
            i
        }
        "golden_apple" => {
            let mut i = ball(hex(0xF8D840), seed);
            i.rect(8, 1, 1, 3, STICK);
            i
        }
        "melon_slice" => blob(hex(0xE04A3A), &[(3, 7, 10, 3), (5, 10, 6, 2)], seed),
        "sweet_berries" | "glow_berries" => {
            let c = if name == "glow_berries" { hex(0xF2A21B) } else { hex(0xC81E2A) };
            blob(c, &[(4, 6, 3, 3), (8, 8, 3, 3), (9, 4, 3, 3)], seed)
        }
        "bread" => blob(hex(0xB8742A), &[(2, 7, 12, 4), (3, 6, 10, 1)], seed),
        "cookie" => ball(hex(0xC8843A), seed),
        "wheat" => {
            let mut i = Img::tile();
            for k in 0..4 {
                line(&mut i, 4 + k * 2, 13, 6 + k * 2, 3, hex(0xD8B43A));
            }
            i.rect(4, 9, 9, 1, STICK);
            finish(i)
        }
        "wheat_seeds" | "beetroot_seeds" | "melon_seeds" | "pumpkin_seeds" => {
            let c = if name == "wheat_seeds" { hex(0x5FA02A) } else { hex(0xC8B88A) };
            let mut i = Img::tile();
            for (x, y) in [(5, 6), (9, 5), (7, 9), (11, 9), (4, 11), (9, 12)] {
                i.rect(x, y, 2, 2, c);
            }
            finish(i)
        }
        "carrot" => blob(hex(0xF08A1A), &[(8, 4, 3, 3), (6, 6, 3, 3), (4, 9, 3, 3)], seed),
        "potato" | "baked_potato" => blob(hex(if name == "potato" { 0xC8A45A } else { 0xD8B46A }), &[(4, 6, 8, 5), (5, 5, 6, 1)], seed),
        "beetroot" => ball(hex(0x9A2A3A), seed),
        "sugar_cane_item" => blob(hex(0x8FC06A), &[(7, 2, 2, 12)], seed),
        "porkchop" | "beef" | "mutton" | "chicken" | "rabbit" | "cod" | "salmon" => {
            let c = match name {
                "chicken" => 0xE8B8A8,
                "cod" => 0xB8A888,
                "salmon" => 0xD86A4A,
                _ => 0xD84A4A,
            };
            blob(hex(c), &[(4, 5, 8, 6), (3, 7, 10, 3), (11, 10, 2, 2)], seed)
        }
        "cooked_porkchop" | "cooked_beef" | "cooked_mutton" | "cooked_chicken" | "cooked_rabbit" | "cooked_cod" | "cooked_salmon" => blob(hex(0x9A5A2A), &[(4, 5, 8, 6), (3, 7, 10, 3), (11, 10, 2, 2)], seed),
        "rotten_flesh" => blob(hex(0x8A5A3A), &[(4, 5, 8, 6), (3, 7, 10, 3)], seed),
        "bone" => {
            let mut i = Img::tile();
            line(&mut i, 4, 12, 11, 5, hex(0xE8E4D0));
            line(&mut i, 5, 12, 12, 5, hex(0xE8E4D0));
            i.rect(3, 11, 2, 3, hex(0xE8E4D0));
            i.rect(11, 3, 3, 2, hex(0xE8E4D0));
            finish(i)
        }
        "string" => {
            let mut i = Img::tile();
            line(&mut i, 3, 4, 8, 8, hex(0xE8E8E8));
            line(&mut i, 8, 8, 5, 12, hex(0xE8E8E8));
            line(&mut i, 8, 8, 12, 11, hex(0xE8E8E8));
            i
        }
        "feather" => {
            let mut i = Img::tile();
            line(&mut i, 3, 13, 12, 3, hex(0xF0F0F0));
            line(&mut i, 5, 12, 12, 5, hex(0xD8D8D8));
            line(&mut i, 4, 11, 10, 4, hex(0xFFFFFF));
            finish(i)
        }
        "leather" => blob(hex(0xA85A2A), &[(4, 4, 8, 8), (3, 6, 10, 4)], seed),
        "paper" => blob(hex(0xF0F0E8), &[(4, 3, 8, 10)], seed),
        "book" => {
            let mut i = blob(hex(0x8A3A22), &[(4, 3, 9, 10)], seed);
            i.rect(5, 12, 7, 1, hex(0xF0F0E8));
            i
        }
        "bowl" => blob(hex(0x7A5A30), &[(3, 7, 10, 2), (4, 9, 8, 2), (6, 11, 4, 1)], seed),
        "shears" => {
            let mut i = Img::tile();
            line(&mut i, 4, 3, 10, 10, hex(0xDADADA));
            line(&mut i, 11, 3, 5, 10, hex(0xDADADA));
            i.outline(3, 10, 4, 4, hex(0x8A3A22));
            i.outline(9, 10, 4, 4, hex(0x8A3A22));
            finish(i)
        }
        "flint_and_steel" => {
            let mut i = blob(hex(0x3A3A3E), &[(8, 7, 4, 5)], seed);
            i.rect(3, 3, 5, 2, hex(0xDADADA));
            i.rect(3, 5, 2, 5, hex(0xDADADA));
            i
        }
        "bucket" => bucket(None),
        "water_bucket" => bucket(Some(hex(0x3F76E4))),
        "lava_bucket" => bucket(Some(hex(0xF08A1A))),
        "milk_bucket" => bucket(Some(hex(0xF8F8F8))),
        "sulfur_cube_bucket" => bucket(Some(hex(0xE2D048))),
        _ => return None,
    };
    Some(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_exist_and_are_partly_transparent() {
        for name in ["iron_pickaxe", "wooden_sword", "diamond", "bread", "bucket", "stick", "raw_iron", "red_dye", "golden_hoe", "netherite_axe"] {
            let img = generate(name).unwrap_or_else(|| panic!("no icon for {name}"));
            let opaque = img.px.iter().filter(|p| p[3] > 0).count();
            assert!(opaque > 12 && opaque < 230, "{name}: {opaque} opaque texels");
        }
        assert!(generate("no_such_item").is_none());
    }
}
