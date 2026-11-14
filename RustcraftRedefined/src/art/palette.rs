//! Colour tables shared by textures, items and world generation.

use super::img::{hex, Col};

pub const DYES: [(&str, u32); 16] = [
    ("white", 0xE9ECEC),
    ("orange", 0xF07613),
    ("magenta", 0xBD44B3),
    ("light_blue", 0x3AAFD9),
    ("yellow", 0xF8C627),
    ("lime", 0x70B919),
    ("pink", 0xED8DAC),
    ("gray", 0x3E4447),
    ("light_gray", 0x8E8E86),
    ("cyan", 0x158991),
    ("purple", 0x792AAC),
    ("blue", 0x35399D),
    ("brown", 0x724728),
    ("green", 0x546D1B),
    ("red", 0xA12722),
    ("black", 0x141519),
];

pub fn dye(name: &str) -> Option<Col> {
    DYES.iter().find(|(n, _)| *n == name).map(|(_, c)| hex(*c))
}

pub struct Wood {
    pub name: &'static str,
    pub planks: u32,
    pub bark: u32,
    pub inner: u32,
    /// Leaf colour; `None` means the leaves are tinted by the biome foliage colour.
    pub leaves: Option<u32>,
}

pub const WOODS: [Wood; 9] = [
    Wood { name: "oak", planks: 0xB8945F, bark: 0x6B5232, inner: 0xB29157, leaves: None },
    Wood { name: "spruce", planks: 0x82613A, bark: 0x3B2A16, inner: 0x7A5A34, leaves: Some(0x4E7A4E) },
    Wood { name: "birch", planks: 0xD7CB8D, bark: 0xD9D6CF, inner: 0xC8B77A, leaves: Some(0x7FA655) },
    Wood { name: "jungle", planks: 0xB88764, bark: 0x564419, inner: 0xAA7A52, leaves: None },
    Wood { name: "acacia", planks: 0xBA6337, bark: 0x696157, inner: 0xB05D3B, leaves: None },
    Wood { name: "dark_oak", planks: 0x4F3218, bark: 0x3A2A17, inner: 0x4A3018, leaves: None },
    Wood { name: "mangrove", planks: 0x7B3A36, bark: 0x55432A, inner: 0x783732, leaves: None },
    Wood { name: "cherry", planks: 0xE7C0B8, bark: 0x3B1F2B, inner: 0xDDA7A0, leaves: Some(0xF2B5CE) },
    Wood { name: "pale_oak", planks: 0xE3DAD8, bark: 0x5A5450, inner: 0xD9CECC, leaves: Some(0x8C948A) },
];

pub fn wood(name: &str) -> Option<&'static Wood> {
    WOODS.iter().find(|w| w.name == name)
}

/// Mineral families: (name, colour, contrast).  Variants such as `polished_x` or `x_bricks`
/// are derived from these by the texture generator.
pub const MINERALS: [(&str, u32, f32); 40] = [
    ("stone", 0x7F7F7F, 0.22),
    ("granite", 0x9A6B58, 0.30),
    ("diorite", 0xBFBFBF, 0.30),
    ("andesite", 0x888A88, 0.22),
    ("deepslate", 0x505053, 0.26),
    ("tuff", 0x6D6F66, 0.24),
    ("calcite", 0xDFE0DC, 0.12),
    ("dripstone", 0x866B5B, 0.26),
    ("sandstone", 0xDBCFA0, 0.12),
    ("red_sandstone", 0xBA6520, 0.14),
    ("blackstone", 0x2B2730, 0.30),
    ("basalt", 0x4B4B4F, 0.26),
    ("netherrack", 0x70332F, 0.36),
    ("end_stone", 0xDBDE9E, 0.14),
    ("obsidian", 0x15101F, 0.45),
    ("prismarine", 0x63A294, 0.30),
    ("purpur", 0xA97BA9, 0.16),
    ("quartz", 0xEBE6DE, 0.07),
    ("nether", 0x2D1719, 0.30),
    ("mud", 0x8A6952, 0.18),
    ("sulfur", 0xD8C640, 0.22),
    ("cinnabar", 0xA83A30, 0.26),
    ("terracotta", 0x985E44, 0.08),
    ("brick", 0x96614F, 0.20),
    ("moss", 0x596E2D, 0.30),
    ("clay", 0xA0A6B3, 0.10),
    ("amethyst", 0x8662BF, 0.34),
    ("bedrock", 0x565656, 0.60),
    ("smooth_stone", 0x9E9E9E, 0.08),
    ("coal", 0x1B1B1B, 0.40),
    ("iron", 0xD8D8D8, 0.12),
    ("copper", 0xC06B4F, 0.20),
    ("gold", 0xF6D03D, 0.18),
    ("redstone", 0xAA1A0A, 0.22),
    ("lapis", 0x1F4390, 0.22),
    ("diamond", 0x62EAE0, 0.20),
    ("emerald", 0x2BC257, 0.20),
    ("netherite", 0x443F41, 0.20),
    ("resin", 0xD9641C, 0.20),
    ("bone", 0xE0DCC8, 0.10),
];

pub fn mineral(name: &str) -> Option<(Col, f32)> {
    MINERALS.iter().find(|(n, _, _)| *n == name).map(|(_, c, k)| (hex(*c), *k))
}

/// Ore gem colours.
pub const ORES: [(&str, u32); 8] = [
    ("coal", 0x2A2A2A),
    ("iron", 0xD8AF93),
    ("copper", 0xE0734D),
    ("gold", 0xFCEE4B),
    ("redstone", 0xE0100A),
    ("lapis", 0x2351BE),
    ("diamond", 0x5DECF5),
    ("emerald", 0x17DD62),
];

pub fn ore_colour(name: &str) -> Option<Col> {
    ORES.iter().find(|(n, _)| *n == name).map(|(_, c)| hex(*c))
}

/// Small flowers: (name, petal colour, centre colour, height in texels).
pub const FLOWERS: [(&str, u32, u32, i32); 13] = [
    ("dandelion", 0xFFEC4F, 0xF5C400, 7),
    ("poppy", 0xE02B1D, 0x2B1010, 9),
    ("blue_orchid", 0x2FB8F5, 0xE9F7FF, 10),
    ("allium", 0xB878ED, 0xE2C2FA, 12),
    ("azure_bluet", 0xF2F5F7, 0xF3E36A, 7),
    ("red_tulip", 0xD9311F, 0xF0604E, 10),
    ("orange_tulip", 0xF09322, 0xF7B64E, 10),
    ("white_tulip", 0xF0F0F0, 0xFFFFFF, 10),
    ("pink_tulip", 0xF2B4E0, 0xFAD4EE, 10),
    ("oxeye_daisy", 0xF4F4F4, 0xF2D43B, 11),
    ("cornflower", 0x466AEB, 0x2B3FA8, 11),
    ("lily_of_the_valley", 0xFFFFFF, 0xE4EFE4, 11),
    ("torchflower", 0xF2A21B, 0xE0521B, 12),
];
