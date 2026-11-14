//! Chunk storage: 16x16 columns split into 24 sections of 16^3 blocks (y = -64..320).

use super::block::{State, AIR};
use crate::ffi::{HEIGHT, MAX_Y, MIN_Y};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

pub const SECTIONS: usize = (HEIGHT / 16) as usize;

/// Generation progress of a chunk.
pub const STAGE_TERRAIN: u8 = 1; // shape, biomes and surface blocks
pub const STAGE_DECORATED: u8 = 2; // ores, trees, plants
pub const STAGE_LIT: u8 = 3; // light computed; ready for meshing and gameplay

pub struct Section {
    pub blocks: [State; 4096],
    /// Sky light in the high nibble, block light in the low nibble.
    pub light: [u8; 4096],
    pub non_air: u16,
}

impl Section {
    fn new(light: u8) -> Box<Section> {
        Box::new(Section { blocks: [AIR; 4096], light: [light; 4096], non_air: 0 })
    }
}

#[inline]
pub fn sindex(x: usize, y: usize, z: usize) -> usize {
    (y * 16 + z) * 16 + x
}

pub struct Chunk {
    pub cx: i32,
    pub cz: i32,
    pub sections: Vec<Option<Box<Section>>>,
    /// Y of the highest non-air block per column (MIN_Y - 1 when the column is empty).
    pub top: [i16; 256],
    pub biome: [u8; 256],
    /// Underground biome per column (0 = none).
    pub cave_biome: [u8; 256],
    /// Estimated terrain height before caves and overhangs, used by surface rules and features.
    pub base_height: [i16; 256],
    pub stage: u8,
    /// One bit per section whose mesh is out of date.
    pub mesh_dirty: u32,
    /// Changed since it was last written to disk.
    pub modified: bool,
}

impl Chunk {
    pub fn new(cx: i32, cz: i32) -> Box<Chunk> {
        Box::new(Chunk {
            cx,
            cz,
            sections: (0..SECTIONS).map(|_| None).collect(),
            top: [(MIN_Y - 1) as i16; 256],
            biome: [0; 256],
            cave_biome: [0; 256],
            base_height: [64; 256],
            stage: 0,
            mesh_dirty: 0,
            modified: false,
        })
    }

    #[inline]
    pub fn get(&self, x: usize, y: i32, z: usize) -> State {
        if !(MIN_Y..MAX_Y).contains(&y) {
            return AIR;
        }
        let yi = (y - MIN_Y) as usize;
        match &self.sections[yi >> 4] {
            Some(s) => s.blocks[sindex(x, yi & 15, z)],
            None => AIR,
        }
    }

    /// Sets a block and returns the previous state.  Light is not updated here.
    pub fn set(&mut self, x: usize, y: i32, z: usize, state: State) -> State {
        if !(MIN_Y..MAX_Y).contains(&y) {
            return AIR;
        }
        let yi = (y - MIN_Y) as usize;
        let si = yi >> 4;
        if self.sections[si].is_none() {
            if state == AIR {
                return AIR;
            }
            self.sections[si] = Some(Section::new(self.default_light(si)));
        }
        let sec = self.sections[si].as_mut().unwrap();
        let i = sindex(x, yi & 15, z);
        let old = sec.blocks[i];
        if old == state {
            return old;
        }
        sec.blocks[i] = state;
        if old == AIR {
            sec.non_air += 1;
        } else if state == AIR {
            sec.non_air -= 1;
        }
        let col = z * 16 + x;
        if state != AIR {
            if y as i16 > self.top[col] {
                self.top[col] = y as i16;
            }
        } else if y as i16 == self.top[col] {
            let mut ny = y - 1;
            while ny >= MIN_Y && self.get(x, ny, z) == AIR {
                ny -= 1;
            }
            self.top[col] = ny as i16;
        }
        self.mesh_dirty |= 1 << si;
        self.modified = true;
        old
    }

    /// Light assumed for a section that has never been allocated: open sky above the terrain,
    /// darkness once the chunk is lit and the section lies under allocated ones.
    fn default_light(&self, si: usize) -> u8 {
        if self.stage >= STAGE_LIT && self.sections[si + 1..].iter().any(|s| s.is_some()) {
            0x00
        } else {
            0xf0
        }
    }

    #[inline]
    pub fn light(&self, x: usize, y: i32, z: usize) -> u8 {
        if y >= MAX_Y {
            return 0xf0;
        }
        if y < MIN_Y {
            return 0;
        }
        let yi = (y - MIN_Y) as usize;
        match &self.sections[yi >> 4] {
            Some(s) => s.light[sindex(x, yi & 15, z)],
            None => 0xf0,
        }
    }

    pub fn set_light(&mut self, x: usize, y: i32, z: usize, value: u8) {
        if !(MIN_Y..MAX_Y).contains(&y) {
            return;
        }
        let yi = (y - MIN_Y) as usize;
        let si = yi >> 4;
        if self.sections[si].is_none() {
            if value == 0xf0 {
                return;
            }
            self.sections[si] = Some(Section::new(0xf0));
        }
        let sec = self.sections[si].as_mut().unwrap();
        let i = sindex(x, yi & 15, z);
        if sec.light[i] != value {
            sec.light[i] = value;
            self.mesh_dirty |= 1 << si;
        }
    }

    /// Allocates every section up to and including the highest one in use, so that explicit
    /// light values can be stored below the terrain surface.
    pub fn allocate_below_top(&mut self) {
        if let Some(top) = self.sections.iter().rposition(|s| s.is_some()) {
            for si in 0..top {
                if self.sections[si].is_none() {
                    self.sections[si] = Some(Section::new(0xf0));
                }
            }
        }
    }

    /// Index of the highest allocated section, if any.
    pub fn top_section(&self) -> Option<usize> {
        self.sections.iter().rposition(|s| s.is_some())
    }

    pub fn section_is_empty(&self, si: usize) -> bool {
        self.sections[si].as_ref().map_or(true, |s| s.non_air == 0)
    }
}

// ------------------------------------------------------------------ save format
const CHUNK_FORMAT: u8 = 1;

impl Chunk {
    /// Serialises the chunk: generation data, then a palette of block-state names and
    /// run-length encoded columns.  Names keep saves valid when the block registry changes.
    pub fn encode(&self) -> Vec<u8> {
        let r = super::block::reg();
        let mut w = crate::util::bin::Writer::new();
        w.u8(CHUNK_FORMAT);
        w.u8(self.stage.min(STAGE_DECORATED));
        w.bytes(&self.biome);
        w.bytes(&self.cave_biome);
        for h in self.base_height {
            w.u16(h as u16);
        }
        let sections = self.top_section().map_or(0, |s| s + 1);
        w.u8(sections as u8);
        let mut palette: Vec<State> = Vec::new();
        let mut lookup: Vec<u32> = vec![u32::MAX; r.states.len()];
        let mut runs: Vec<(u32, u32)> = Vec::new();
        for z in 0..16 {
            for x in 0..16 {
                for si in 0..sections {
                    for ly in 0..16 {
                        let s = match &self.sections[si] {
                            Some(sec) => sec.blocks[sindex(x, ly, z)],
                            None => AIR,
                        };
                        let mut id = lookup[s as usize];
                        if id == u32::MAX {
                            id = palette.len() as u32;
                            lookup[s as usize] = id;
                            palette.push(s);
                        }
                        match runs.last_mut() {
                            Some((last, n)) if *last == id => *n += 1,
                            _ => runs.push((id, 1)),
                        }
                    }
                }
            }
        }
        w.var(palette.len() as u64);
        for s in &palette {
            w.str(&r.describe(*s));
        }
        w.var(runs.len() as u64);
        for (id, n) in runs {
            w.var(id as u64);
            w.var(n as u64);
        }
        w.buf
    }

    /// Rebuilds a chunk from `encode` output.  Returns `None` for corrupt or unknown data.
    pub fn decode(cx: i32, cz: i32, data: &[u8]) -> Option<Box<Chunk>> {
        let r = super::block::reg();
        let mut rd = crate::util::bin::Reader::new(data);
        if rd.u8()? != CHUNK_FORMAT {
            return None;
        }
        let stage = rd.u8()?;
        if !(STAGE_TERRAIN..=STAGE_DECORATED).contains(&stage) {
            return None;
        }
        let mut c = Chunk::new(cx, cz);
        c.biome.copy_from_slice(rd.take(256)?);
        c.cave_biome.copy_from_slice(rd.take(256)?);
        for h in c.base_height.iter_mut() {
            *h = rd.u16()? as i16;
        }
        let sections = rd.u8()? as usize;
        if sections > SECTIONS {
            return None;
        }
        let count = rd.var()? as usize;
        let mut palette = Vec::with_capacity(count.min(4096));
        for _ in 0..count {
            // Blocks that no longer exist turn into air.
            palette.push(r.parse_state(&rd.str()?).unwrap_or(AIR));
        }
        let runs = rd.var()? as usize;
        let per_column = sections * 16;
        let total = per_column * 256;
        let mut i = 0usize;
        for _ in 0..runs {
            let state = *palette.get(rd.var()? as usize)?;
            let n = rd.var()? as usize;
            if n > total - i {
                return None;
            }
            if state != AIR {
                for k in i..i + n {
                    let (col, y) = (k / per_column, k % per_column);
                    c.set(col % 16, MIN_Y + y as i32, col / 16, state);
                }
            }
            i += n;
        }
        if i != total {
            return None;
        }
        c.stage = stage;
        c.modified = false;
        c.mesh_dirty = u32::MAX >> (32 - SECTIONS);
        Some(c)
    }
}

/// Fast hasher for chunk coordinates.
#[derive(Default)]
pub struct CoordHasher(u64);
impl Hasher for CoordHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ *b as u64).wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    fn write_i32(&mut self, v: i32) {
        self.0 = (self.0.rotate_left(29) ^ v as u32 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}
pub type CoordMap<V> = HashMap<(i32, i32), V, BuildHasherDefault<CoordHasher>>;

/// A square window of chunks temporarily taken out of the world for bulk editing
/// (decoration, lighting, mesh input).  Coordinates are world coordinates.
pub struct Region {
    pub cx0: i32,
    pub cz0: i32,
    pub size: i32,
    pub chunks: Vec<Option<Box<Chunk>>>,
}

impl Region {
    #[inline]
    fn slot(&self, x: i32, z: i32) -> Option<usize> {
        let (cx, cz) = ((x >> 4) - self.cx0, (z >> 4) - self.cz0);
        if cx < 0 || cz < 0 || cx >= self.size || cz >= self.size {
            None
        } else {
            Some((cz * self.size + cx) as usize)
        }
    }
    #[inline]
    pub fn chunk(&self, x: i32, z: i32) -> Option<&Chunk> {
        self.slot(x, z).and_then(|i| self.chunks[i].as_deref())
    }
    #[inline]
    pub fn chunk_mut(&mut self, x: i32, z: i32) -> Option<&mut Chunk> {
        match self.slot(x, z) {
            Some(i) => self.chunks[i].as_deref_mut(),
            None => None,
        }
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> State {
        match self.chunk(x, z) {
            Some(c) => c.get((x & 15) as usize, y, (z & 15) as usize),
            None => AIR,
        }
    }
    #[inline]
    pub fn set(&mut self, x: i32, y: i32, z: i32, s: State) {
        if let Some(c) = self.chunk_mut(x, z) {
            c.set((x & 15) as usize, y, (z & 15) as usize, s);
        }
    }
    #[inline]
    pub fn light(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.chunk(x, z) {
            Some(c) => c.light((x & 15) as usize, y, (z & 15) as usize),
            None => 0xf0,
        }
    }
    #[inline]
    pub fn set_light(&mut self, x: i32, y: i32, z: i32, v: u8) {
        if let Some(c) = self.chunk_mut(x, z) {
            c.set_light((x & 15) as usize, y, (z & 15) as usize, v);
        }
    }
    /// Y of the highest non-air block in a column.
    pub fn top(&self, x: i32, z: i32) -> i32 {
        match self.chunk(x, z) {
            Some(c) => c.top[((z & 15) * 16 + (x & 15)) as usize] as i32,
            None => MIN_Y - 1,
        }
    }
    pub fn biome(&self, x: i32, z: i32) -> u8 {
        self.chunk(x, z).map_or(0, |c| c.biome[((z & 15) * 16 + (x & 15)) as usize])
    }
    pub fn cave_biome(&self, x: i32, z: i32) -> u8 {
        self.chunk(x, z).map_or(0, |c| c.cave_biome[((z & 15) * 16 + (x & 15)) as usize])
    }
    pub fn stage(&self, x: i32, z: i32) -> u8 {
        self.chunk(x, z).map_or(0, |c| c.stage)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_and_heightmap() {
        let mut c = Chunk::new(0, 0);
        assert_eq!(c.get(3, 70, 4), AIR);
        assert_eq!(c.set(3, 70, 4, 5), AIR);
        assert_eq!(c.get(3, 70, 4), 5);
        assert_eq!(c.top[4 * 16 + 3], 70);
        c.set(3, 10, 4, 7);
        assert_eq!(c.top[4 * 16 + 3], 70);
        c.set(3, 70, 4, AIR);
        assert_eq!(c.top[4 * 16 + 3], 10);
        c.set(3, 10, 4, AIR);
        assert_eq!(c.top[4 * 16 + 3], (MIN_Y - 1) as i16);
        assert_eq!(c.set(0, MAX_Y, 0, 5), AIR);
        assert_eq!(c.get(0, MIN_Y - 1, 0), AIR);
        assert!(c.section_is_empty(0));
    }

    #[test]
    fn encode_decode_round_trip() {
        let r = crate::world::block::reg();
        let mut c = Chunk::new(3, -2);
        let stairs = r.parse_state("oak_stairs[facing=east,half=top,shape=straight,waterlogged=false]").unwrap();
        for z in 0..16 {
            for x in 0..16 {
                for y in MIN_Y..(40 + (x as i32 + z as i32) % 5) {
                    c.set(x, y, z, if y < 0 { r.k.deepslate } else { r.k.stone });
                }
            }
        }
        c.set(5, 200, 6, stairs);
        c.set(0, MIN_Y, 0, r.k.bedrock);
        c.biome[17] = 9;
        c.cave_biome[3] = 2;
        c.base_height[255] = -12;
        c.stage = STAGE_LIT;
        let data = c.encode();
        assert!(data.len() < 6000, "encoded size {}", data.len());
        let d = Chunk::decode(3, -2, &data).expect("decodes");
        assert_eq!(d.stage, STAGE_DECORATED, "light is recomputed after loading");
        assert!(!d.modified);
        assert_eq!((d.biome[17], d.cave_biome[3], d.base_height[255]), (9, 2, -12));
        assert_eq!(d.top, c.top);
        for z in 0..16 {
            for x in 0..16 {
                for y in MIN_Y..MAX_Y {
                    assert_eq!(d.get(x, y, z), c.get(x, y, z), "at {x} {y} {z}");
                }
            }
        }
        // Corrupt input is rejected rather than crashing.
        for cut in [0, 1, 10, 600, data.len() - 1] {
            assert!(Chunk::decode(0, 0, &data[..cut]).is_none());
        }
        // An empty chunk survives too.
        let mut e = Chunk::new(0, 0);
        e.stage = STAGE_TERRAIN;
        let d = Chunk::decode(0, 0, &e.encode()).unwrap();
        assert!(d.top_section().is_none() && d.stage == STAGE_TERRAIN);
    }

    #[test]
    fn region_addresses_negative_coordinates() {
        let mut r = Region { cx0: -1, cz0: -1, size: 3, chunks: (0..9).map(|i| Some(Chunk::new(i % 3 - 1, i / 3 - 1))).collect() };
        r.set(-1, 5, -1, 9);
        r.set(16, 5, 31, 8);
        assert_eq!(r.get(-1, 5, -1), 9);
        assert_eq!(r.get(16, 5, 31), 8);
        assert_eq!(r.chunks[0].as_ref().unwrap().get(15, 5, 15), 9);
        assert_eq!(r.chunks[8].as_ref().unwrap().get(0, 5, 15), 8);
        assert_eq!(r.get(40, 5, 0), AIR);
        assert_eq!(r.top(-1, -1), 5);
    }
}
