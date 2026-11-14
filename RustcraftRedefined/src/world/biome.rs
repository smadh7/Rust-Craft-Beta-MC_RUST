//! Biomes: climate-driven selection plus the per-biome data used by generation and rendering.
//! The selection table is this project's own approximation of a multi-noise biome layout.

use crate::ffi::VcColumn;

macro_rules! biomes {
    ($($variant:ident = $name:literal),* $(,)?) => {
        #[repr(u8)]
        #[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
        pub enum Biome { $($variant),* }
        pub const ALL_BIOMES: &[Biome] = &[$(Biome::$variant),*];
        impl Biome {
            pub fn name(self) -> &'static str { match self { $(Biome::$variant => $name),* } }
        }
    };
}
biomes!(
    Ocean = "ocean", DeepOcean = "deep_ocean", ColdOcean = "cold_ocean", FrozenOcean = "frozen_ocean",
    LukewarmOcean = "lukewarm_ocean", WarmOcean = "warm_ocean", River = "river", FrozenRiver = "frozen_river",
    Beach = "beach", SnowyBeach = "snowy_beach", StonyShore = "stony_shore", Plains = "plains",
    SunflowerPlains = "sunflower_plains", Forest = "forest", FlowerForest = "flower_forest", BirchForest = "birch_forest",
    DarkForest = "dark_forest", Taiga = "taiga", SnowyTaiga = "snowy_taiga", SnowyPlains = "snowy_plains",
    IceSpikes = "ice_spikes", Desert = "desert", Savanna = "savanna", Jungle = "jungle", Badlands = "badlands",
    Swamp = "swamp", MangroveSwamp = "mangrove_swamp", Meadow = "meadow", CherryGrove = "cherry_grove", Grove = "grove",
    SnowySlopes = "snowy_slopes", JaggedPeaks = "jagged_peaks", FrozenPeaks = "frozen_peaks", StonyPeaks = "stony_peaks",
    WindsweptHills = "windswept_hills", MushroomFields = "mushroom_fields", LushCaves = "lush_caves",
    DripstoneCaves = "dripstone_caves", DeepDark = "deep_dark", SulfurCaves = "sulfur_caves",
);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tree {
    Oak,
    Birch,
    Spruce,
    Jungle,
    Acacia,
    DarkOak,
    Cherry,
    SwampOak,
    BigMushroom,
}

#[derive(Clone, Copy)]
pub struct BiomeInfo {
    pub grass: u32,
    pub foliage: u32,
    pub water: u32,
    pub sky: u32,
    /// Snow falls and water freezes.
    pub frozen: bool,
    /// No rain or snow at all.
    pub dry: bool,
    /// Average trees per chunk and the kinds to choose from.
    pub trees: f32,
    pub kinds: &'static [Tree],
    /// Chance per surface block of ground cover and of a flower.
    pub grass_cover: f32,
    pub flowers: f32,
}

const DEFAULT: BiomeInfo = BiomeInfo {
    grass: 0x7DBD52,
    foliage: 0x64AA38,
    water: 0x3F76E4,
    sky: 0x7BA4FF,
    frozen: false,
    dry: false,
    trees: 0.0,
    kinds: &[Tree::Oak],
    grass_cover: 0.0,
    flowers: 0.0,
};

impl Biome {
    pub fn from_u8(v: u8) -> Biome {
        ALL_BIOMES.get(v as usize).copied().unwrap_or(Biome::Plains)
    }
    pub fn from_name(name: &str) -> Option<Biome> {
        ALL_BIOMES.iter().copied().find(|b| b.name() == name)
    }
    pub fn is_ocean(self) -> bool {
        (self as u8) <= Biome::WarmOcean as u8
    }
    pub fn is_cave(self) -> bool {
        (self as u8) >= Biome::LushCaves as u8
    }

    pub fn info(self) -> BiomeInfo {
        use Biome::*;
        let d = DEFAULT;
        match self {
            Ocean | DeepOcean | River | Beach | StonyShore => BiomeInfo { grass: 0x8EB971, foliage: 0x71A74D, ..d },
            ColdOcean => BiomeInfo { water: 0x3D57D6, grass: 0x8EB971, ..d },
            FrozenOcean | FrozenRiver | SnowyBeach => BiomeInfo { water: 0x3938C9, grass: 0x80B497, foliage: 0x60A17B, sky: 0x7FA1FF, frozen: true, ..d },
            LukewarmOcean => BiomeInfo { water: 0x45ADF2, ..d },
            WarmOcean => BiomeInfo { water: 0x43D5EE, ..d },
            Plains => BiomeInfo { trees: 0.06, grass_cover: 0.34, flowers: 0.012, ..d },
            SunflowerPlains => BiomeInfo { trees: 0.05, grass_cover: 0.34, flowers: 0.03, ..d },
            Forest => BiomeInfo { grass: 0x72B348, foliage: 0x58A62E, trees: 9.0, kinds: &[Tree::Oak, Tree::Oak, Tree::Oak, Tree::Birch], grass_cover: 0.12, flowers: 0.006, ..d },
            FlowerForest => BiomeInfo { grass: 0x72B348, foliage: 0x58A62E, trees: 5.0, kinds: &[Tree::Oak, Tree::Birch], grass_cover: 0.15, flowers: 0.12, ..d },
            BirchForest => BiomeInfo { grass: 0x84BA5A, foliage: 0x6CA840, trees: 9.0, kinds: &[Tree::Birch], grass_cover: 0.12, flowers: 0.006, ..d },
            DarkForest => BiomeInfo { grass: 0x4F8E33, foliage: 0x3F7F26, trees: 22.0, kinds: &[Tree::DarkOak, Tree::DarkOak, Tree::DarkOak, Tree::Oak, Tree::BigMushroom], grass_cover: 0.06, flowers: 0.002, ..d },
            Taiga => BiomeInfo { grass: 0x7FA873, foliage: 0x64935C, sky: 0x7FA7FF, trees: 9.0, kinds: &[Tree::Spruce], grass_cover: 0.16, flowers: 0.002, ..d },
            SnowyTaiga => BiomeInfo { grass: 0x80B497, foliage: 0x60A17B, water: 0x3D57D6, sky: 0x83A1FF, frozen: true, trees: 8.0, kinds: &[Tree::Spruce], grass_cover: 0.04, ..d },
            SnowyPlains => BiomeInfo { grass: 0x80B497, foliage: 0x60A17B, water: 0x3D57D6, sky: 0x83A1FF, frozen: true, trees: 0.12, kinds: &[Tree::Spruce], grass_cover: 0.02, ..d },
            IceSpikes => BiomeInfo { grass: 0x80B497, foliage: 0x60A17B, water: 0x3D57D6, sky: 0x83A1FF, frozen: true, ..d },
            Desert => BiomeInfo { grass: 0xBFB755, foliage: 0xAEA42A, sky: 0x6EB1FF, dry: true, ..d },
            Savanna => BiomeInfo { grass: 0xBFB755, foliage: 0xAEA42A, sky: 0x6EB1FF, dry: true, trees: 0.9, kinds: &[Tree::Acacia, Tree::Acacia, Tree::Oak], grass_cover: 0.5, flowers: 0.003, ..d },
            Jungle => BiomeInfo { grass: 0x4FC93A, foliage: 0x2FBB0B, sky: 0x77A8FF, trees: 26.0, kinds: &[Tree::Jungle, Tree::Jungle, Tree::Oak], grass_cover: 0.45, flowers: 0.01, ..d },
            Badlands => BiomeInfo { grass: 0x90814D, foliage: 0x9E814D, sky: 0x6EB1FF, dry: true, trees: 0.0, ..d },
            Swamp => BiomeInfo { grass: 0x6A7039, foliage: 0x6A7039, water: 0x617B64, sky: 0x78A7FF, trees: 2.2, kinds: &[Tree::SwampOak], grass_cover: 0.22, flowers: 0.01, ..d },
            MangroveSwamp => BiomeInfo { grass: 0x6A7039, foliage: 0x8DB127, water: 0x3A7A6A, sky: 0x78A7FF, trees: 6.0, kinds: &[Tree::SwampOak, Tree::Jungle], grass_cover: 0.1, ..d },
            Meadow => BiomeInfo { grass: 0x83BB6D, foliage: 0x63A948, water: 0x0E4ECF, trees: 0.03, kinds: &[Tree::Oak, Tree::Birch], grass_cover: 0.55, flowers: 0.16, ..d },
            CherryGrove => BiomeInfo { grass: 0xB6DB61, foliage: 0xB6DB61, water: 0x5DB7EF, trees: 2.6, kinds: &[Tree::Cherry], grass_cover: 0.3, flowers: 0.05, ..d },
            Grove => BiomeInfo { grass: 0x80B497, foliage: 0x60A17B, sky: 0x83A1FF, frozen: true, trees: 6.0, kinds: &[Tree::Spruce], ..d },
            SnowySlopes | JaggedPeaks | FrozenPeaks => BiomeInfo { grass: 0x80B497, foliage: 0x60A17B, sky: 0x85A0FF, frozen: true, ..d },
            StonyPeaks => BiomeInfo { grass: 0x9ABE4B, foliage: 0x82AC1E, sky: 0x76A8FF, ..d },
            WindsweptHills => BiomeInfo { grass: 0x8AB689, foliage: 0x6DA36B, sky: 0x7DA3FF, trees: 0.4, kinds: &[Tree::Spruce, Tree::Oak], grass_cover: 0.1, flowers: 0.002, ..d },
            MushroomFields => BiomeInfo { grass: 0x55C93F, foliage: 0x2BBB0F, sky: 0x77A8FF, trees: 0.5, kinds: &[Tree::BigMushroom], ..d },
            LushCaves | DripstoneCaves | DeepDark | SulfurCaves => d,
        }
    }
}

/// Surface biome for a column.  `surface` is the terrain height estimate in world Y.
pub fn pick_surface(c: &VcColumn, surface: i32, sea: i32) -> Biome {
    use Biome::*;
    let (t, h, cont, e, w, pv) = (c.temperature, c.humidity, c.continentalness, c.erosion, c.weirdness, c.pv);
    let band = if t < -0.45 { 0 } else if t < -0.15 { 1 } else if t < 0.2 { 2 } else if t < 0.55 { 3 } else { 4 };
    if cont < -1.05 {
        return MushroomFields;
    }
    if cont < -0.19 || surface < sea - 6 {
        if cont >= -0.105 && surface >= sea - 8 {
            return if band == 0 { FrozenRiver } else { River };
        }
        return match band {
            0 => FrozenOcean,
            1 => ColdOcean,
            2 => if cont < -0.455 { DeepOcean } else { Ocean },
            3 => LukewarmOcean,
            _ => WarmOcean,
        };
    }
    if pv < -0.85 && surface <= sea + 1 {
        return if band == 0 { FrozenRiver } else { River };
    }
    if cont < -0.105 && surface <= sea + 3 {
        return if e < -0.375 { StonyShore } else if band == 0 { SnowyBeach } else { Beach };
    }
    if e < -0.375 && cont > 0.03 {
        if pv > 0.56 && surface > 120 {
            return match band {
                0 | 1 => if w < 0.0 { FrozenPeaks } else { JaggedPeaks },
                2 => JaggedPeaks,
                _ => StonyPeaks,
            };
        }
        if pv > 0.1 && surface > 96 {
            return match band {
                0 | 1 => if h < -0.1 { SnowySlopes } else { Grove },
                2 => if w > 0.2 && h > -0.1 { CherryGrove } else { Meadow },
                3 => Meadow,
                _ => Badlands,
            };
        }
    }
    if e > 0.55 && band >= 2 && surface <= sea + 4 {
        return if band >= 3 { MangroveSwamp } else { Swamp };
    }
    if e > 0.45 && e < 0.55 && cont > 0.0 {
        return if band <= 2 { WindsweptHills } else { Savanna };
    }
    let hb = if h < -0.35 { 0 } else if h < -0.1 { 1 } else if h < 0.1 { 2 } else if h < 0.3 { 3 } else { 4 };
    match (band, hb) {
        (0, 0) => if w > 0.25 { IceSpikes } else { SnowyPlains },
        (0, 1) | (0, 2) => SnowyPlains,
        (0, _) => SnowyTaiga,
        (1, 0) | (1, 1) => Plains,
        (1, 2) => Forest,
        (1, _) => Taiga,
        (2, 0) => if w > 0.0 { FlowerForest } else { SunflowerPlains },
        (2, 1) => Plains,
        (2, 2) => Forest,
        (2, 3) => BirchForest,
        (2, _) => DarkForest,
        (3, 0) | (3, 1) => Savanna,
        (3, 2) => Forest,
        (3, _) => Jungle,
        (_, 0) | (_, 1) | (_, 2) => if w > 0.25 || e < -0.25 { Badlands } else { Desert },
        _ => if w > 0.3 { Badlands } else { Desert },
    }
}

/// Underground biome for a column; `geo` is the sulfur region noise.
pub fn pick_cave(c: &VcColumn, geo: f32) -> Option<Biome> {
    if geo > 0.42 {
        Some(Biome::SulfurCaves)
    } else if c.humidity > 0.48 {
        Some(Biome::LushCaves)
    } else if c.continentalness > 0.55 {
        Some(Biome::DripstoneCaves)
    } else if c.erosion < -0.55 && c.continentalness > 0.1 {
        Some(Biome::DeepDark)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for (i, b) in ALL_BIOMES.iter().enumerate() {
            assert_eq!(*b as usize, i);
            assert_eq!(Biome::from_u8(i as u8), *b);
            assert_eq!(Biome::from_name(b.name()), Some(*b));
        }
        assert!(Biome::DeepOcean.is_ocean() && !Biome::River.is_ocean());
        assert!(Biome::SulfurCaves.is_cave() && !Biome::MushroomFields.is_cave());
    }

    #[test]
    fn climate_extremes_pick_expected_biomes() {
        let col = |t: f32, h: f32, c: f32, e: f32, w: f32| VcColumn { temperature: t, humidity: h, continentalness: c, erosion: e, weirdness: w, pv: 0.0, base_height: 70.0, surface: 70 };
        assert_eq!(pick_surface(&col(0.9, -0.5, 0.3, 0.2, 0.0), 70, 63), Biome::Desert);
        assert_eq!(pick_surface(&col(-0.9, 0.0, 0.3, 0.2, 0.0), 70, 63), Biome::SnowyPlains);
        assert_eq!(pick_surface(&col(0.0, 0.0, -0.6, 0.0, 0.0), 30, 63), Biome::DeepOcean);
        assert_eq!(pick_surface(&col(0.4, 0.5, 0.3, 0.2, 0.0), 70, 63), Biome::Jungle);
        assert_eq!(pick_surface(&col(0.0, 0.0, -1.2, 0.0, 0.0), 66, 63), Biome::MushroomFields);
        assert_eq!(pick_cave(&col(0.0, 0.0, 0.0, 0.0, 0.0), 0.6), Some(Biome::SulfurCaves));
        assert_eq!(pick_cave(&col(0.0, 0.0, 0.0, 0.0, 0.0), 0.0), None);
    }
}
