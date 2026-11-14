//! Overworld generation: the native kernel shapes the terrain; this module assigns biomes,
//! builds the surface and places ores, cave features, trees and plants.

use super::biome::{pick_cave, pick_surface, Biome, Tree};
use super::block::*;
use super::chunk::{Chunk, Region, STAGE_TERRAIN};
use crate::ffi::*;
use crate::util::rng::Rng;

macro_rules! gen_blocks {
    ($($f:ident),* $(,)?) => {
        struct GenBlocks { $($f: State),* }
        impl GenBlocks {
            fn new() -> GenBlocks {
                let r = reg();
                GenBlocks { $($f: r.default_state(stringify!($f)).unwrap_or_else(|| panic!("missing block {}", stringify!($f)))),* }
            }
        }
    };
}
gen_blocks!(
    stone, deepslate, bedrock, water, lava, dirt, grass_block, sand, red_sand, gravel, sandstone, clay, mud,
    coarse_dirt, podzol, mycelium, snow_block, snow, ice, packed_ice, calcite, terracotta, orange_terracotta, white_terracotta,
    yellow_terracotta, red_terracotta, brown_terracotta, granite, diorite, andesite, tuff,
    oak_log, oak_leaves, birch_log, birch_leaves, spruce_log, spruce_leaves, jungle_log, jungle_leaves, acacia_log, acacia_leaves,
    dark_oak_log, dark_oak_leaves, cherry_log, cherry_leaves, mushroom_stem, brown_mushroom_block, red_mushroom_block,
    short_grass, fern, tall_grass, dead_bush, cactus, sugar_cane, lily_pad, seagrass, kelp, sunflower, brown_mushroom, red_mushroom,
    pumpkin, melon, dandelion, poppy, blue_orchid, allium, azure_bluet, red_tulip, orange_tulip, white_tulip, pink_tulip,
    oxeye_daisy, cornflower, lily_of_the_valley, moss_block, moss_carpet, cave_vines, dripstone_block, pointed_dripstone, sculk,
    sulfur, cinnabar, potent_sulfur, sulfur_spike,
);

/// Ore distribution: name, veins per chunk, vein size, lowest Y, highest Y, peak Y (None = uniform).
const ORES: &[(&str, u32, u32, i32, i32, Option<i32>)] = &[
    ("coal", 16, 14, 0, 190, Some(96)),
    ("iron", 12, 9, -56, 72, Some(16)),
    ("iron", 6, 9, 80, 300, Some(232)),
    ("copper", 12, 10, -16, 112, Some(48)),
    ("gold", 4, 9, -64, 32, Some(-16)),
    ("redstone", 7, 8, -64, 15, Some(-60)),
    ("lapis", 3, 7, -64, 64, Some(0)),
    ("diamond", 6, 6, -64, 16, Some(-60)),
];

pub struct Generator {
    pub seed: u64,
    kernel: TerrainKernel,
    surface_noise: Fbm,
    geo_noise: Fbm,
    patch_noise: Fbm,
    b: GenBlocks,
    ores: Vec<(State, State, u32, u32, i32, i32, Option<i32>)>,
}

#[derive(Clone, Copy, PartialEq)]
enum Above {
    Air,
    Water,
    Solid,
}

impl Generator {
    pub fn new(seed: u64) -> Generator {
        let r = reg();
        let ores = ORES
            .iter()
            .map(|&(n, count, size, lo, hi, peak)| {
                (r.default_state(&format!("{n}_ore")).unwrap(), r.default_state(&format!("deepslate_{n}_ore")).unwrap(), count, size, lo, hi, peak)
            })
            .collect();
        Generator {
            seed,
            kernel: TerrainKernel::new(seed),
            surface_noise: Fbm::new(seed ^ 0x5u64, 3, 1.0 / 28.0, 2.6),
            geo_noise: Fbm::new(seed ^ 0x6e0, 2, 1.0 / 260.0, 2.4),
            patch_noise: Fbm::new(seed ^ 0x9a7c, 2, 1.0 / 40.0, 2.4),
            b: GenBlocks::new(),
            ores,
        }
    }

    pub fn column(&self, x: i32, z: i32) -> VcColumn {
        self.kernel.column(x, z)
    }

    /// Biome at a block column without generating the chunk (used for spawn search and maps).
    pub fn biome_at(&self, x: i32, z: i32) -> Biome {
        let c = self.kernel.column(x, z);
        pick_surface(&c, c.base_height.floor() as i32, SEA_LEVEL)
    }

    // ------------------------------------------------------------ stage 1: terrain
    pub fn terrain(&self, cx: i32, cz: i32) -> Box<Chunk> {
        let mut mats = vec![0u8; 16 * 16 * HEIGHT as usize];
        let mut cols = [VcColumn::default(); 256];
        self.kernel.chunk(cx, cz, &mut mats, &mut cols);
        let mut ch = Chunk::new(cx, cz);
        let b = &self.b;
        for (i, c) in cols.iter().enumerate() {
            let (wx, wz) = (cx * 16 + (i % 16) as i32, cz * 16 + (i / 16) as i32);
            let est = c.base_height.floor() as i32;
            ch.base_height[i] = est as i16;
            ch.biome[i] = pick_surface(c, est, SEA_LEVEL) as u8;
            let geo = self.geo_noise.sample2(wx as f64, wz as f64) as f32;
            ch.cave_biome[i] = pick_cave(c, geo).map_or(0, |b| b as u8);
        }
        let table = [AIR, b.stone, b.deepslate, b.water, b.lava, b.bedrock];
        for yi in 0..HEIGHT as usize {
            let layer = &mats[yi * 256..yi * 256 + 256];
            if layer.iter().all(|m| *m == MAT_AIR) {
                continue;
            }
            let y = MIN_Y + yi as i32;
            for (i, m) in layer.iter().enumerate() {
                if *m != MAT_AIR {
                    ch.set(i % 16, y, i / 16, table[*m as usize]);
                }
            }
        }
        for z in 0..16 {
            for x in 0..16 {
                let i = z * 16 + x;
                let h = |dx: i32, dz: i32| cols[((z as i32 + dz).clamp(0, 15) * 16 + (x as i32 + dx).clamp(0, 15)) as usize].surface;
                let slope = (h(1, 0) - h(-1, 0)).abs().max((h(0, 1) - h(0, -1)).abs());
                let biome = Biome::from_u8(ch.biome[i]);
                self.surface_column(&mut ch, x, z, cx * 16 + x as i32, cz * 16 + z as i32, biome, slope);
            }
        }
        ch.stage = STAGE_TERRAIN;
        ch.modified = false;
        ch
    }

    fn badlands_band(&self, y: i32, wobble: i32) -> State {
        let b = &self.b;
        const BANDS: [u8; 24] = [0, 0, 1, 0, 2, 2, 0, 3, 0, 0, 4, 1, 0, 0, 5, 0, 2, 0, 0, 1, 1, 0, 3, 0];
        match BANDS[(y + wobble).rem_euclid(24) as usize] {
            1 => b.orange_terracotta,
            2 => b.yellow_terracotta,
            3 => b.white_terracotta,
            4 => b.red_terracotta,
            5 => b.brown_terracotta,
            _ => b.terracotta,
        }
    }

    fn surface_column(&self, ch: &mut Chunk, x: usize, z: usize, wx: i32, wz: i32, biome: Biome, slope: i32) {
        use Biome::*;
        let b = &self.b;
        let i = z * 16 + x;
        let top = ch.top[i] as i32;
        let base = ch.base_height[i] as i32;
        let n = self.surface_noise.sample2(wx as f64, wz as f64) as f32;
        let filler_depth = 3 + (n * 1.6) as i32;
        let floor = (base - 14).min(top - 6).max(MIN_Y + 6);
        let mut above = Above::Air;
        let mut run = 0;
        let (mut cur_top, mut cur_fill, mut cur_deep) = (b.stone, b.stone, b.stone);
        let mut y = top;
        while y >= floor {
            let s = ch.get(x, y, z);
            if s == AIR {
                above = Above::Air;
                run = 0;
            } else if s == b.water {
                above = Above::Water;
                run = 0;
            } else if s == b.stone || s == b.deepslate {
                if run == 0 {
                    let wet = above == Above::Water;
                    let steep = slope >= 3;
                    (cur_top, cur_fill, cur_deep) = if above == Above::Solid {
                        (s, s, s)
                    } else if wet {
                        let t = match biome {
                            WarmOcean | LukewarmOcean | Beach | SnowyBeach | Desert => b.sand,
                            MangroveSwamp => b.mud,
                            Swamp => if n < -0.3 { b.clay } else { b.dirt },
                            River | FrozenRiver => if n > 0.35 { b.gravel } else if n < -0.5 { b.clay } else { b.sand },
                            Ocean | DeepOcean | ColdOcean | FrozenOcean => if n > 0.05 { b.gravel } else if n < -0.62 { b.clay } else { b.sand },
                            Badlands => b.red_sand,
                            _ => if y >= SEA_LEVEL - 2 { b.sand } else { b.dirt },
                        };
                        (t, t, s)
                    } else {
                        match biome {
                            Desert => (b.sand, b.sand, b.sandstone),
                            Beach | SnowyBeach => (b.sand, b.sand, b.sandstone),
                            Badlands => {
                                if y > 70 + (n * 4.0) as i32 {
                                    let band = self.badlands_band(y, (n * 3.0) as i32);
                                    (band, b.orange_terracotta, b.terracotta)
                                } else {
                                    (b.red_sand, b.orange_terracotta, b.terracotta)
                                }
                            }
                            MushroomFields => (b.mycelium, b.dirt, s),
                            MangroveSwamp => (b.mud, b.mud, s),
                            StonyShore => if n > 0.3 { (b.gravel, b.gravel, s) } else { (b.stone, b.stone, s) },
                            StonyPeaks => if n > 0.35 { (b.calcite, b.calcite, s) } else { (b.stone, b.stone, s) },
                            JaggedPeaks => if steep { (b.stone, b.stone, s) } else { (b.snow_block, b.stone, s) },
                            FrozenPeaks => if steep || n > 0.3 { (b.packed_ice, b.packed_ice, s) } else { (b.snow_block, b.snow_block, s) },
                            SnowySlopes => if steep { (b.stone, b.stone, s) } else { (b.snow_block, b.snow_block, s) },
                            Grove | IceSpikes => (b.snow_block, b.dirt, s),
                            WindsweptHills => if n > 0.4 || steep { (b.stone, b.stone, s) } else if n < -0.45 { (b.gravel, b.gravel, s) } else { (b.grass_block, b.dirt, s) },
                            Taiga | SnowyTaiga => if n < -0.5 { (b.podzol, b.dirt, s) } else if n > 0.62 { (b.coarse_dirt, b.dirt, s) } else { (b.grass_block, b.dirt, s) },
                            Savanna => if n > 0.6 { (b.coarse_dirt, b.dirt, s) } else { (b.grass_block, b.dirt, s) },
                            Meadow | CherryGrove if steep && y > 110 => (b.stone, b.stone, s),
                            _ if biome.is_ocean() || biome == River || biome == FrozenRiver => {
                                if y <= SEA_LEVEL + 2 { (b.sand, b.sand, s) } else { (b.grass_block, b.dirt, s) }
                            }
                            _ => (b.grass_block, b.dirt, s),
                        }
                    };
                }
                let place = if run == 0 {
                    cur_top
                } else if run <= filler_depth {
                    cur_fill
                } else if run <= filler_depth + 4 && (cur_deep == b.sandstone || cur_deep == b.terracotta) {
                    cur_deep
                } else {
                    s
                };
                // Badlands keep their striped look all the way down the cliff face.
                let place = if biome == Badlands && run > 0 && run < 40 && y > 62 && place != b.red_sand { self.badlands_band(y, (n * 3.0) as i32) } else { place };
                if place != s {
                    ch.set(x, y, z, place);
                }
                run += 1;
                above = Above::Solid;
            } else {
                run += 1;
                above = Above::Solid;
            }
            y -= 1;
        }
    }

    // ------------------------------------------------------------ stage 2: decoration
    /// Places features that originate in chunk (cx, cz).  The region must hold the 3x3 neighbourhood.
    pub fn decorate(&self, r: &mut Region, cx: i32, cz: i32) {
        let mut rng = Rng::at(self.seed, cx, cz, 0xdec0);
        self.place_ores(r, &mut rng, cx, cz);
        self.decorate_caves(r, &mut rng, cx, cz);
        self.place_trees(r, &mut rng, cx, cz);
        self.place_plants(r, &mut rng, cx, cz);
        self.freeze(r, cx, cz);
    }

    fn blob(&self, r: &mut Region, rng: &mut Rng, x: i32, y: i32, z: i32, size: u32, swap: &dyn Fn(State) -> Option<State>) {
        let angle = rng.f32() * std::f32::consts::PI;
        let len = size as f32 / 8.0;
        let (x0, x1) = (x as f32 + angle.sin() * len, x as f32 - angle.sin() * len);
        let (z0, z1) = (z as f32 + angle.cos() * len, z as f32 - angle.cos() * len);
        let (y0, y1) = (y as f32 + rng.range(-2, 2) as f32, y as f32 + rng.range(-2, 2) as f32);
        for i in 0..size {
            let t = i as f32 / size as f32;
            let (cx, cy, cz) = (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, z0 + (z1 - z0) * t);
            let rad = (((std::f32::consts::PI * t).sin() + 1.0) * rng.f32() * size as f32 / 32.0 + 1.0) * 0.5;
            let ir = rad.ceil() as i32;
            for dy in -ir..=ir {
                for dz in -ir..=ir {
                    for dx in -ir..=ir {
                        let (bx, by, bz) = ((cx + dx as f32).floor() as i32, (cy + dy as f32).floor() as i32, (cz + dz as f32).floor() as i32);
                        let d2 = (bx as f32 + 0.5 - cx).powi(2) + (by as f32 + 0.5 - cy).powi(2) + (bz as f32 + 0.5 - cz).powi(2);
                        if d2 < rad * rad {
                            if let Some(new) = swap(r.get(bx, by, bz)) {
                                r.set(bx, by, bz, new);
                            }
                        }
                    }
                }
            }
        }
    }

    fn place_ores(&self, r: &mut Region, rng: &mut Rng, cx: i32, cz: i32) {
        let b = &self.b;
        let (stone, deepslate) = (b.stone, b.deepslate);
        let pos = |rng: &mut Rng, lo: i32, hi: i32, peak: Option<i32>| {
            let y = match peak {
                Some(p) => {
                    let half = (p - lo).max(hi - p) as f32;
                    (rng.triangle(p as f32, half)).round() as i32
                }
                None => rng.range(lo, hi),
            };
            (cx * 16 + rng.range(0, 15), y, cz * 16 + rng.range(0, 15))
        };
        // Stone variety and soil pockets.
        for (state, count, size, lo, hi) in [(b.dirt, 6, 30, 0, 160), (b.gravel, 6, 30, -64, 160), (b.granite, 2, 50, 0, 128), (b.diorite, 2, 50, 0, 128), (b.andesite, 2, 50, 0, 128), (b.tuff, 2, 50, -64, 0)] {
            for _ in 0..count {
                let (x, y, z) = pos(rng, lo, hi, None);
                self.blob(r, rng, x, y, z, size, &|s| if s == stone || (s == deepslate && state == b.tuff) { Some(state) } else { None });
            }
        }
        for &(ore, deep, count, size, lo, hi, peak) in &self.ores {
            for _ in 0..count {
                let (x, y, z) = pos(rng, lo, hi, peak);
                if y < lo || y > hi {
                    continue;
                }
                self.blob(r, rng, x, y, z, size, &|s| if s == stone { Some(ore) } else if s == deepslate { Some(deep) } else { None });
            }
        }
        // Emeralds: single blocks under mountain biomes.
        let centre = Biome::from_u8(r.biome(cx * 16 + 8, cz * 16 + 8));
        if matches!(centre, Biome::JaggedPeaks | Biome::FrozenPeaks | Biome::StonyPeaks | Biome::Meadow | Biome::Grove | Biome::SnowySlopes | Biome::WindsweptHills) {
            let em = reg().default_state("emerald_ore").unwrap();
            for _ in 0..40 {
                let (x, y, z) = pos(rng, -16, 320, Some(200));
                if r.get(x, y, z) == stone {
                    r.set(x, y, z, em);
                }
            }
        }
    }

    fn spike(&self, r: &mut Region, rng: &mut Rng, x: i32, y: i32, z: i32, up: bool, block: State) {
        let reg = reg();
        let len = rng.range(1, 4);
        let step = if up { 1 } else { -1 };
        let mut free = 0;
        while free < len && r.get(x, y + free * step, z) == AIR {
            free += 1;
        }
        // Leave a gap so stalactites and stalagmites do not seal the cave.
        if r.get(x, y + free * step, z) != AIR && free > 1 {
            free -= 1;
        }
        for i in 0..free {
            let thickness = match free - 1 - i {
                0 => 1,
                1 => 2,
                _ => if i == 0 { 4 } else { 3 },
            };
            let s = reg.with(reg.with(block, "vertical_direction", if up { 0 } else { 1 }), "thickness", thickness);
            r.set(x, y + i * step, z, s);
        }
    }

    fn decorate_caves(&self, r: &mut Region, rng: &mut Rng, cx: i32, cz: i32) {
        let b = &self.b;
        let reg = reg();
        let rock = |s: State| s == b.stone || s == b.deepslate || s == b.granite || s == b.diorite || s == b.andesite || s == b.tuff;
        for lz in 0..16 {
            for lx in 0..16 {
                let (x, z) = (cx * 16 + lx, cz * 16 + lz);
                let cave = r.cave_biome(x, z);
                if cave == 0 {
                    continue;
                }
                let biome = Biome::from_u8(cave);
                let base = r.chunk(x, z).map_or(64, |c| c.base_height[(lz * 16 + lx) as usize] as i32);
                let ceiling_limit = (base - 10).min(52);
                let patch = self.patch_noise.sample(x as f64, 0.0, z as f64) as f32;
                let mut y = MIN_Y + 6;
                while y < ceiling_limit {
                    let here = r.get(x, y, z);
                    if here != AIR {
                        y += 1;
                        continue;
                    }
                    let below = r.get(x, y - 1, z);
                    let above = r.get(x, y + 1, z);
                    match biome {
                        Biome::LushCaves => {
                            if rock(below) && patch > -0.3 {
                                r.set(x, y - 1, z, b.moss_block);
                                if rng.chance(0.25) {
                                    r.set(x, y, z, if rng.chance(0.5) { b.moss_carpet } else { b.short_grass });
                                }
                            }
                            if rock(above) && patch > -0.2 {
                                r.set(x, y + 1, z, b.moss_block);
                                if rng.chance(0.09) && r.get(x, y, z) == AIR {
                                    r.set(x, y, z, b.cave_vines);
                                }
                            }
                        }
                        Biome::DripstoneCaves => {
                            if rock(below) && patch > 0.0 {
                                r.set(x, y - 1, z, b.dripstone_block);
                                if rng.chance(0.12) {
                                    self.spike(r, rng, x, y, z, true, b.pointed_dripstone);
                                }
                            }
                            if rock(above) && patch > -0.1 {
                                r.set(x, y + 1, z, b.dripstone_block);
                                if rng.chance(0.16) {
                                    self.spike(r, rng, x, y, z, false, b.pointed_dripstone);
                                }
                            }
                        }
                        Biome::DeepDark => {
                            if y < 0 && rock(below) && patch > -0.1 {
                                r.set(x, y - 1, z, b.sculk);
                            }
                        }
                        Biome::SulfurCaves => {
                            let mineral = if patch > 0.38 { b.cinnabar } else { b.sulfur };
                            if rock(below) && patch > -0.35 {
                                r.set(x, y - 1, z, mineral);
                                if mineral == b.sulfur && rng.chance(0.07) {
                                    self.spike(r, rng, x, y, z, true, b.sulfur_spike);
                                } else if mineral == b.sulfur && rng.chance(0.012) && lx > 2 && lx < 13 && lz > 2 && lz < 13 {
                                    self.sulfur_spring(r, rng, x, y - 1, z);
                                }
                            }
                            if rock(above) && patch > -0.35 {
                                r.set(x, y + 1, z, mineral);
                                if mineral == b.sulfur && rng.chance(0.09) {
                                    self.spike(r, rng, x, y, z, false, b.sulfur_spike);
                                }
                            }
                            for d in 2..6 {
                                let (dx, _, dz) = crate::util::math::DIRS[d];
                                if rock(r.get(x + dx, y, z + dz)) && patch > -0.2 {
                                    r.set(x + dx, y, z + dz, mineral);
                                }
                            }
                        }
                        _ => {}
                    }
                    y += 1;
                }
            }
        }
        let _ = reg;
    }

    /// A small sulfur-lined pool with potent sulfur at its bottom: the source of a geyser.
    fn sulfur_spring(&self, r: &mut Region, rng: &mut Rng, x: i32, floor_y: i32, z: i32) {
        let b = &self.b;
        let rad = rng.range(2, 3);
        for dz in -rad - 1..=rad + 1 {
            for dx in -rad - 1..=rad + 1 {
                let d2 = dx * dx + dz * dz;
                if d2 > (rad + 1) * (rad + 1) {
                    continue;
                }
                let inside = d2 <= rad * rad;
                let depth = if inside { if d2 <= 1 { 2 } else { 1 } } else { 0 };
                // Never open the pool sideways into empty space.
                if r.get(x + dx, floor_y, z + dz) == AIR {
                    continue;
                }
                for dy in 0..=depth + 1 {
                    let (bx, by, bz) = (x + dx, floor_y - dy, z + dz);
                    if dy < depth {
                        r.set(bx, by, bz, b.water);
                    } else if dy == depth {
                        r.set(bx, by, bz, if dx == 0 && dz == 0 { b.potent_sulfur } else { b.sulfur });
                    } else if r.get(bx, by, bz) == AIR {
                        r.set(bx, by, bz, b.sulfur);
                    }
                }
                if !inside {
                    r.set(x + dx, floor_y, z + dz, b.sulfur);
                }
            }
        }
    }

    /// Y of the first free block above the ground at a column, if the ground is exposed soil.
    fn ground(&self, r: &Region, x: i32, z: i32) -> Option<(i32, State)> {
        let reg = reg();
        let mut y = r.top(x, z);
        for _ in 0..40 {
            if y <= MIN_Y {
                return None;
            }
            let s = r.get(x, y, z);
            if s == AIR || reg.has(s, F_LEAVES | F_PLANT) || reg.block(s).name.ends_with("_log") {
                y -= 1;
                continue;
            }
            return if r.get(x, y + 1, z) == AIR || reg.has(r.get(x, y + 1, z), F_PLANT) { Some((y + 1, s)) } else { None };
        }
        None
    }

    fn is_soil(&self, s: State) -> bool {
        let b = &self.b;
        s == b.grass_block || s == b.dirt || s == b.podzol || s == b.coarse_dirt || s == b.mycelium || s == b.mud || s == b.moss_block
    }

    fn leaf(&self, r: &mut Region, x: i32, y: i32, z: i32, leaves: State) {
        let s = r.get(x, y, z);
        if s == AIR || reg().has(s, F_PLANT | F_REPLACEABLE) && !reg().has(s, F_FLUID) {
            r.set(x, y, z, leaves);
        }
    }

    /// Square canopy layer; corners are dropped when `corner` returns false.
    fn canopy(&self, r: &mut Region, rng: &mut Rng, x: i32, y: i32, z: i32, rad: i32, corner_chance: f32, leaves: State) {
        for dz in -rad..=rad {
            for dx in -rad..=rad {
                if dx.abs() == rad && dz.abs() == rad && rad > 0 && !rng.chance(corner_chance) {
                    continue;
                }
                self.leaf(r, x + dx, y, z + dz, leaves);
            }
        }
    }

    pub fn place_tree(&self, r: &mut Region, rng: &mut Rng, kind: Tree, x: i32, y: i32, z: i32) -> bool {
        let b = &self.b;
        let reg = reg();
        let ground = r.get(x, y - 1, z);
        if !self.is_soil(ground) || (kind == Tree::BigMushroom && ground != b.mycelium && ground != b.podzol && ground != b.grass_block) {
            return false;
        }
        let (log, leaves, height) = match kind {
            Tree::Oak | Tree::SwampOak => (b.oak_log, b.oak_leaves, rng.range(4, 6)),
            Tree::Birch => (b.birch_log, b.birch_leaves, rng.range(5, 7)),
            Tree::Spruce => (b.spruce_log, b.spruce_leaves, rng.range(6, 10)),
            Tree::Jungle => (b.jungle_log, b.jungle_leaves, rng.range(7, 12)),
            Tree::Acacia => (b.acacia_log, b.acacia_leaves, rng.range(5, 7)),
            Tree::DarkOak => (b.dark_oak_log, b.dark_oak_leaves, rng.range(6, 8)),
            Tree::Cherry => (b.cherry_log, b.cherry_leaves, rng.range(5, 7)),
            Tree::BigMushroom => (b.mushroom_stem, if rng.chance(0.5) { b.red_mushroom_block } else { b.brown_mushroom_block }, rng.range(4, 6)),
        };
        if y + height + 3 >= MAX_Y {
            return false;
        }
        for dy in 0..height + 1 {
            let s = r.get(x, y + dy, z);
            if s != AIR && !reg.has(s, F_LEAVES | F_PLANT) {
                return false;
            }
        }
        if ground == b.grass_block || ground == b.podzol || ground == b.mycelium {
            r.set(x, y - 1, z, b.dirt);
        }
        let top = y + height;
        match kind {
            Tree::Oak | Tree::Birch | Tree::Jungle | Tree::SwampOak => {
                let wide = if kind == Tree::SwampOak { 3 } else { 2 };
                for (i, ly) in (top - 3..=top).enumerate() {
                    let rad = if i >= 2 { 1 } else { wide };
                    let corners = if i == 3 { 0.0 } else if i == 2 { 0.4 } else { 0.6 };
                    self.canopy(r, rng, x, ly, z, rad, corners, leaves);
                }
                if kind == Tree::Jungle && height > 8 {
                    self.canopy(r, rng, x, top - 4, z, 3, 0.2, leaves);
                }
                for dy in 0..height {
                    r.set(x, y + dy, z, log);
                }
            }
            Tree::Spruce => {
                self.leaf(r, x, top + 1, z, leaves);
                let mut rad = 1;
                let mut grow = true;
                for ly in (y + 2..=top).rev() {
                    self.canopy(r, rng, x, ly, z, rad, 0.0, leaves);
                    if grow {
                        rad += 1;
                        if rad > 2 + (height > 7) as i32 {
                            grow = false;
                            rad = 1;
                        }
                    } else {
                        grow = true;
                        rad += 1;
                    }
                    rad = rad.min(3);
                }
                for dy in 0..height {
                    r.set(x, y + dy, z, log);
                }
            }
            Tree::Acacia => {
                let (dx, _, dz) = crate::util::math::DIRS[2 + rng.below(4) as usize];
                let bend = height - rng.range(2, 3);
                let (mut tx, mut tz) = (x, z);
                for dy in 0..height {
                    if dy >= bend {
                        tx += dx;
                        tz += dz;
                    }
                    r.set(tx, y + dy, tz, log);
                }
                self.canopy(r, rng, tx, top - 1, tz, 3, 0.0, leaves);
                self.canopy(r, rng, tx, top, tz, 2, 0.3, leaves);
                r.set(tx, top - 1, tz, log);
            }
            Tree::DarkOak => {
                for (ox, oz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    if self.is_soil(r.get(x + ox, y - 1, z + oz)) {
                        r.set(x + ox, y - 1, z + oz, b.dirt);
                    }
                }
                for (i, ly) in (top - 2..=top + 1).enumerate() {
                    let rad = [3, 4, 3, 2][i];
                    for dz in -rad..=rad + 1 {
                        for dx in -rad..=rad + 1 {
                            let edge = (dx == -rad || dx == rad + 1) && (dz == -rad || dz == rad + 1);
                            if !edge || rng.chance(0.25) {
                                self.leaf(r, x + dx, ly, z + dz, leaves);
                            }
                        }
                    }
                }
                for dy in 0..height {
                    for (ox, oz) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        r.set(x + ox, y + dy, z + oz, log);
                    }
                }
            }
            Tree::Cherry => {
                let (cy, rad) = (top, 3);
                for dy in -2..=2 {
                    for dz in -rad - 1..=rad + 1 {
                        for dx in -rad - 1..=rad + 1 {
                            let d = (dx * dx + dz * dz) as f32 + (dy * dy) as f32 * 2.4;
                            if d < (rad as f32 + 0.8).powi(2) - rng.f32() * 3.0 {
                                self.leaf(r, x + dx, cy + dy, z + dz, leaves);
                            }
                        }
                    }
                }
                for dy in 0..height {
                    r.set(x, y + dy, z, log);
                }
                let (dx, _, dz) = crate::util::math::DIRS[2 + rng.below(4) as usize];
                let axis = if dx != 0 { 0 } else { 2 };
                let side = reg.with(log, "axis", axis);
                r.set(x + dx, y + height - 2, z + dz, side);
                r.set(x + dx * 2, y + height - 1, z + dz * 2, log);
            }
            Tree::BigMushroom => {
                let red = leaves == b.red_mushroom_block;
                if red {
                    for (i, ly) in (top - 2..=top).enumerate() {
                        let rad: i32 = if i == 2 { 1 } else { 2 };
                        for dz in -rad..=rad {
                            for dx in -rad..=rad {
                                let rim = dx.abs() == rad || dz.abs() == rad;
                                if (i == 2 || rim) && !(i < 2 && dx.abs() == rad && dz.abs() == rad) {
                                    self.leaf(r, x + dx, ly, z + dz, leaves);
                                }
                            }
                        }
                    }
                } else {
                    for dz in -3i32..=3 {
                        for dx in -3i32..=3 {
                            if !(dx.abs() == 3 && dz.abs() == 3) {
                                self.leaf(r, x + dx, top, z + dz, leaves);
                            }
                        }
                    }
                }
                for dy in 0..height {
                    r.set(x, y + dy, z, log);
                }
            }
        }
        true
    }

    fn place_trees(&self, r: &mut Region, rng: &mut Rng, cx: i32, cz: i32) {
        let centre = Biome::from_u8(r.biome(cx * 16 + 8, cz * 16 + 8)).info();
        let density = centre.trees * (0.75 + 0.5 * self.patch_noise.sample2((cx * 16) as f64 * 0.37, (cz * 16) as f64 * 0.37) as f32).max(0.2);
        let count = density.floor() as i32 + rng.chance(density.fract()) as i32;
        for _ in 0..count {
            let (x, z) = (cx * 16 + rng.range(0, 15), cz * 16 + rng.range(0, 15));
            let info = Biome::from_u8(r.biome(x, z)).info();
            if info.trees <= 0.0 {
                continue;
            }
            if let Some((y, ground)) = self.ground(r, x, z) {
                if self.is_soil(ground) && y > SEA_LEVEL {
                    let kind = *rng.pick(info.kinds);
                    self.place_tree(r, rng, kind, x, y, z);
                }
            }
        }
    }

    fn place_plants(&self, r: &mut Region, rng: &mut Rng, cx: i32, cz: i32) {
        use Biome::*;
        let b = &self.b;
        let reg = reg();
        for lz in 0..16 {
            for lx in 0..16 {
                let (x, z) = (cx * 16 + lx, cz * 16 + lz);
                let biome = Biome::from_u8(r.biome(x, z));
                let info = biome.info();
                let top = r.top(x, z);
                let top_block = r.get(x, top, z);
                // Water plants.
                if top_block == b.water {
                    if biome == Swamp && rng.chance(0.04) && top == SEA_LEVEL - 1 {
                        r.set(x, top + 1, z, b.lily_pad);
                    }
                    let mut fy = top;
                    while fy > MIN_Y && r.get(x, fy, z) == b.water {
                        fy -= 1;
                    }
                    let depth = top - fy;
                    let floor = r.get(x, fy, z);
                    if depth >= 2 && (floor == b.sand || floor == b.gravel || floor == b.dirt || floor == b.clay) && !info.frozen {
                        let patch = self.patch_noise.sample2(x as f64 * 1.7, z as f64 * 1.7) as f32;
                        if matches!(biome, Ocean | ColdOcean | DeepOcean) && patch > 0.35 && depth > 5 && rng.chance(0.3) {
                            for i in 1..=rng.range(2, depth - 2) {
                                r.set(x, fy + i, z, b.kelp);
                            }
                        } else if rng.chance(if biome == WarmOcean || biome == LukewarmOcean { 0.2 } else { 0.06 }) {
                            r.set(x, fy + 1, z, b.seagrass);
                        }
                    }
                    continue;
                }
                let Some((y, ground)) = self.ground(r, x, z) else { continue };
                if r.get(x, y, z) != AIR {
                    continue;
                }
                let free2 = r.get(x, y + 1, z) == AIR;
                if ground == b.sand || ground == b.red_sand {
                    if matches!(biome, Desert | Badlands) {
                        if rng.chance(0.004) {
                            let clear = (2..6).all(|d| {
                                let (dx, _, dz) = crate::util::math::DIRS[d];
                                r.get(x + dx, y, z + dz) == AIR && r.get(x + dx, y + 1, z + dz) == AIR
                            });
                            if clear {
                                for i in 0..rng.range(1, 3) {
                                    r.set(x, y + i, z, b.cactus);
                                }
                            }
                        } else if rng.chance(0.006) {
                            r.set(x, y, z, b.dead_bush);
                        }
                    }
                }
                let soil = self.is_soil(ground);
                if (soil || ground == b.sand) && y <= SEA_LEVEL + 2 && rng.chance(0.12) {
                    let wet = (2..6).any(|d| {
                        let (dx, _, dz) = crate::util::math::DIRS[d];
                        reg.has(r.get(x + dx, y - 1, z + dz), F_WATER)
                    });
                    if wet && !info.frozen {
                        for i in 0..rng.range(1, 3) {
                            if r.get(x, y + i, z) == AIR {
                                r.set(x, y + i, z, b.sugar_cane);
                            }
                        }
                        continue;
                    }
                }
                if !soil || ground == b.mud {
                    continue;
                }
                let patch = self.patch_noise.sample2(x as f64, z as f64) as f32;
                if ground == b.mycelium || biome == DarkForest {
                    if rng.chance(if ground == b.mycelium { 0.03 } else { 0.008 }) {
                        r.set(x, y, z, if rng.chance(0.5) { b.red_mushroom } else { b.brown_mushroom });
                        continue;
                    }
                }
                if ground != b.grass_block {
                    continue;
                }
                if rng.chance(info.flowers * (1.0 + patch).max(0.0)) {
                    if biome == SunflowerPlains && free2 && rng.chance(0.5) {
                        r.set(x, y, z, b.sunflower);
                        r.set(x, y + 1, z, reg.with(b.sunflower, "half", 1));
                        continue;
                    }
                    let choices: &[State] = match biome {
                        Swamp => &[b.blue_orchid],
                        Plains | SunflowerPlains => if patch > 0.2 { &[b.red_tulip, b.orange_tulip, b.white_tulip, b.pink_tulip] } else { &[b.dandelion, b.poppy, b.azure_bluet, b.oxeye_daisy, b.cornflower] },
                        FlowerForest | Meadow => &[b.dandelion, b.poppy, b.allium, b.azure_bluet, b.red_tulip, b.orange_tulip, b.white_tulip, b.pink_tulip, b.oxeye_daisy, b.cornflower, b.lily_of_the_valley],
                        BirchForest | DarkForest | Forest => &[b.dandelion, b.poppy, b.lily_of_the_valley],
                        _ => &[b.dandelion, b.poppy],
                    };
                    let idx = (((patch + 1.0) * 2.5) as usize + rng.below(2) as usize) % choices.len();
                    r.set(x, y, z, choices[idx]);
                } else if rng.chance(info.grass_cover) {
                    let fernish = matches!(biome, Taiga | SnowyTaiga | Jungle) && rng.chance(0.5);
                    if !fernish && free2 && rng.chance(if biome == Savanna || biome == Plains { 0.12 } else { 0.04 }) {
                        r.set(x, y, z, b.tall_grass);
                        r.set(x, y + 1, z, reg.with(b.tall_grass, "half", 1));
                    } else {
                        r.set(x, y, z, if fernish { b.fern } else { b.short_grass });
                    }
                } else if rng.chance(0.0004) && !info.frozen {
                    r.set(x, y, z, if biome == Jungle { b.melon } else { b.pumpkin });
                }
            }
        }
    }

    /// Snow cover and ice for frozen biomes, applied to the columns of one chunk.
    fn freeze(&self, r: &mut Region, cx: i32, cz: i32) {
        let b = &self.b;
        let reg = reg();
        for lz in 0..16 {
            for lx in 0..16 {
                let (x, z) = (cx * 16 + lx, cz * 16 + lz);
                let biome = Biome::from_u8(r.biome(x, z));
                let top = r.top(x, z);
                let high = top > 190 + (self.surface_noise.sample2(x as f64, z as f64) * 8.0) as i32;
                if !(biome.info().frozen || high) || top + 1 >= MAX_Y {
                    continue;
                }
                let s = r.get(x, top, z);
                if s == b.water {
                    let n = self.surface_noise.sample2(x as f64 * 0.6, z as f64 * 0.6);
                    if biome != Biome::FrozenOcean || n > -0.15 {
                        r.set(x, top, z, b.ice);
                    }
                } else if reg.has(s, F_OPAQUE | F_LEAVES) && s != b.snow_block && s != b.packed_ice && s != b.ice {
                    r.set(x, top + 1, z, b.snow);
                    if s == b.grass_block {
                        r.set(x, top, z, reg.with(s, "snowy", 1));
                    }
                } else if reg.has(s, F_PLANT) && reg.has(s, F_REPLACEABLE) && !reg.has(s, F_WATER) {
                    // Short plants are buried by the snow.
                    let below = r.get(x, top - 1, z);
                    if reg.has(below, F_OPAQUE) {
                        r.set(x, top, z, b.snow);
                        if reg.block(below).name == "grass_block" {
                            r.set(x, top - 1, z, reg.with(b.grass_block, "snowy", 1));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(gen: &Generator, cx: i32, cz: i32) -> Region {
        Region { cx0: cx - 1, cz0: cz - 1, size: 3, chunks: (0..9).map(|i| Some(gen.terrain(cx - 1 + i % 3, cz - 1 + i / 3))).collect() }
    }

    #[test]
    fn terrain_is_deterministic_and_layered() {
        let gen = Generator::new(1234);
        let a = gen.terrain(3, -2);
        let c = gen.terrain(3, -2);
        let reg = reg();
        for y in MIN_Y..MAX_Y {
            for i in [0usize, 37, 120, 255] {
                assert_eq!(a.get(i % 16, y, i / 16), c.get(i % 16, y, i / 16));
            }
        }
        for i in 0..256 {
            assert_eq!(a.get(i % 16, MIN_Y, i / 16), reg.k.bedrock);
            assert!(a.top[i] as i32 >= SEA_LEVEL - 1, "column {i} ends below sea level without water");
        }
        assert_eq!(a.stage, STAGE_TERRAIN);
    }

    #[test]
    fn decoration_stays_inside_the_region_and_adds_ores() {
        let gen = Generator::new(77);
        let reg = reg();
        let mut found_ore = false;
        let mut found_plant = false;
        for (cx, cz) in [(0, 0), (40, 13), (-25, 60), (9, -70)] {
            let mut r = region(&gen, cx, cz);
            gen.decorate(&mut r, cx, cz);
            for ch in r.chunks.iter().flatten() {
                for y in MIN_Y..200 {
                    for i in (0..256).step_by(3) {
                        let s = ch.get(i % 16, y, i / 16);
                        let name = reg.block(s).name;
                        found_ore |= name.ends_with("_ore");
                        found_plant |= reg.has(s, F_PLANT | F_LEAVES);
                    }
                }
            }
        }
        assert!(found_ore, "no ore generated");
        assert!(found_plant, "no vegetation generated");
    }
}
