//! Procedural textures for mobs: one 16x16 tile per surface kind (hide, wool, face ...).
//! Faces are drawn on an 8x8 grid of 2x2 texel "pixels" so they read at a distance.

use super::img::{hex, hexa, shade, speckle, Col, Img};
use crate::util::rng::hash_str;

pub const NAMES: [&str; 22] = [
    "pig", "pig_face", "cow", "cow_face", "sheep_wool", "sheep_skin", "sheep_face", "chicken", "chicken_face", "chicken_beak", "zombie_skin", "zombie_face", "zombie_shirt",
    "zombie_pants", "creeper", "creeper_face", "spider", "spider_face", "sulfur_cube", "sulfur_cube_face", "sulfur_cube_core", "white",
];

/// Sets one face pixel (2x2 texels).
fn px(img: &mut Img, x: i32, y: i32, c: Col) {
    img.rect(x * 2, y * 2, 2, 2, c);
}

fn eyes(img: &mut Img, y: i32, white: Col, pupil: Col) {
    for (x, c) in [(0, white), (1, pupil), (6, pupil), (7, white)] {
        px(img, x, y, c);
    }
}

pub fn generate(name: &str) -> Option<Img> {
    let seed = hash_str(name);
    let white = hex(0xF4F4F4);
    let black = hex(0x141414);
    Some(match name {
        "pig" => speckle(hex(0xEFA3A0), 0.10, seed),
        "pig_face" => {
            let mut i = speckle(hex(0xEFA3A0), 0.08, seed);
            eyes(&mut i, 3, white, black);
            i.rect(4, 8, 8, 6, hex(0xF7BDB8));
            px(&mut i, 2, 5, hex(0xB56B68));
            px(&mut i, 5, 5, hex(0xB56B68));
            i
        }
        "cow" => {
            // Dark hide with pale patches.
            let mut i = speckle(hex(0x4A3526), 0.12, seed);
            for (x, y, w, h) in [(1, 2, 6, 5), (9, 9, 6, 5), (11, 1, 4, 3), (2, 11, 4, 4)] {
                i.rect(x, y, w, h, hex(0xE8E2D8));
            }
            i
        }
        "cow_face" => {
            let mut i = speckle(hex(0x4A3526), 0.10, seed);
            i.rect(6, 0, 4, 10, hex(0xE8E2D8));
            eyes(&mut i, 3, white, black);
            i.rect(4, 10, 8, 6, hex(0xD9C7BC));
            px(&mut i, 2, 6, hex(0x3A2A22));
            px(&mut i, 5, 6, hex(0x3A2A22));
            i
        }
        "sheep_wool" => speckle(hex(0xECECEC), 0.07, seed),
        "sheep_skin" => speckle(hex(0xD9B99B), 0.08, seed),
        "sheep_face" => {
            let mut i = speckle(hex(0xD9B99B), 0.06, seed);
            i.rect(0, 0, 16, 4, hex(0xECECEC));
            eyes(&mut i, 3, white, black);
            i.rect(6, 10, 4, 2, hex(0xE79A9A));
            i
        }
        "chicken" => speckle(hex(0xF2F2F2), 0.06, seed),
        "chicken_face" => {
            let mut i = speckle(hex(0xF2F2F2), 0.05, seed);
            px(&mut i, 1, 2, black);
            px(&mut i, 6, 2, black);
            i.rect(6, 12, 4, 4, hex(0xC8322B));
            i
        }
        "chicken_beak" => speckle(hex(0xE0A32A), 0.08, seed),
        "zombie_skin" => speckle(hex(0x4F7F45), 0.14, seed),
        "zombie_face" => {
            let mut i = speckle(hex(0x4F7F45), 0.12, seed);
            i.rect(0, 0, 16, 2, hex(0x2F4F2C));
            for x in [1, 2, 5, 6] {
                px(&mut i, x, 4, black);
            }
            i.rect(4, 12, 8, 2, hex(0x2F4F2C));
            i
        }
        "zombie_shirt" => speckle(hex(0x2B8E93), 0.12, seed),
        "zombie_pants" => speckle(hex(0x3F3A9B), 0.12, seed),
        "creeper" => {
            let mut i = speckle(hex(0x52B040), 0.30, seed);
            i.map(|x, y, c| if (x * 7 + y * 13) % 11 == 0 { shade(c, 1.35) } else { c });
            i
        }
        "creeper_face" => {
            let mut i = speckle(hex(0x52B040), 0.26, seed);
            for (x, y) in [(1, 2), (2, 2), (1, 3), (2, 3), (5, 2), (6, 2), (5, 3), (6, 3), (3, 4), (4, 4), (2, 5), (3, 5), (4, 5), (5, 5), (2, 6), (3, 6), (4, 6), (5, 6), (2, 7), (5, 7)] {
                px(&mut i, x, y, black);
            }
            i
        }
        "spider" => speckle(hex(0x3B322C), 0.18, seed),
        "spider_face" => {
            let mut i = speckle(hex(0x3B322C), 0.14, seed);
            for (x, y) in [(1, 3), (2, 3), (5, 3), (6, 3), (1, 4), (2, 4), (5, 4), (6, 4), (3, 2), (4, 2), (0, 1), (7, 1)] {
                px(&mut i, x, y, hex(0xC8201E));
            }
            i
        }
        // The cube's jelly lets light through; the core is the denser middle.
        "sulfur_cube" => {
            let mut i = speckle(hex(0xE9D34A), 0.10, seed);
            i.map(|x, y, c| {
                let edge = x == 0 || y == 0 || x == 15 || y == 15;
                let c = if edge { shade(c, 0.8) } else { c };
                [c[0], c[1], c[2], if edge { 215 } else { 150 }]
            });
            i
        }
        "sulfur_cube_face" => {
            let mut i = speckle(hex(0xE9D34A), 0.10, seed);
            i.map(|x, y, c| {
                let edge = x == 0 || y == 0 || x == 15 || y == 15;
                [c[0], c[1], c[2], if edge { 215 } else { 150 }]
            });
            let dark = hexa(0x5A4A12, 240);
            for (x, y) in [(1, 2), (2, 2), (1, 3), (2, 3), (5, 2), (6, 2), (5, 3), (6, 3), (3, 5), (4, 5)] {
                px(&mut i, x, y, dark);
            }
            i
        }
        "sulfur_cube_core" => speckle(hex(0xC9A92C), 0.16, seed),
        "white" => Img::filled(hex(0xFFFFFF)),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mob_texture_exists() {
        for n in NAMES {
            let img = generate(n).unwrap_or_else(|| panic!("no texture for {n}"));
            assert_eq!(img.bytes().len(), 16 * 16 * 4);
        }
        assert!(generate("unicorn").is_none());
    }
}
