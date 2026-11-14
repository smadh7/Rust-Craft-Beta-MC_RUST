//! The voxel world: chunk storage, the generation pipeline and block access.

pub mod biome;
pub mod block;
pub mod blocks;
pub mod chunk;
pub mod gen;
pub mod light;
pub mod mesh;
pub mod model;
pub mod rules;
pub mod sim;

use crate::ffi::{MAX_Y, MIN_Y};
use crate::util::math::IVec3;
use block::{reg, State, AIR};
use chunk::*;
use gen::Generator;
use light::LightEngine;

pub struct World {
    pub seed: u64,
    pub gen: Generator,
    pub chunks: CoordMap<Box<Chunk>>,
    lights: LightEngine,
    /// Chunk offsets sorted by distance, for the streaming loop.
    spiral: Vec<(i32, i32)>,
    spiral_radius: i32,
    cursor: usize,
    cursor_centre: (i32, i32),
    /// Storage key prefix of the save this world belongs to (`None`: nothing is written).
    pub store: Option<String>,
    /// Positions changed through `set_block` since the simulation last looked (only recorded
    /// while `track_changes` is on).
    pub changed: Vec<IVec3>,
    pub track_changes: bool,
    /// Unsupported sand and gravel removed from the world, waiting to become falling entities.
    /// Without `animate_falls` such blocks drop to the ground at once instead.
    pub falls: Vec<(IVec3, State)>,
    pub animate_falls: bool,
}

/// Storage key of a chunk inside a save.
pub fn chunk_key(prefix: &str, cx: i32, cz: i32) -> String {
    format!("{prefix}chunks/c_{cx}_{cz}.rcc")
}

impl World {
    pub fn new(seed: u64) -> World {
        World {
            seed,
            gen: Generator::new(seed),
            chunks: CoordMap::default(),
            lights: LightEngine::default(),
            spiral: Vec::new(),
            spiral_radius: -1,
            cursor: 0,
            cursor_centre: (i32::MAX, i32::MAX),
            store: None,
            changed: Vec::new(),
            track_changes: false,
            falls: Vec::new(),
            animate_falls: false,
        }
    }

    // ------------------------------------------------------------ persistence
    /// A chunk from the save if it was stored there, otherwise freshly generated terrain.
    fn load_or_generate(&mut self, cx: i32, cz: i32) -> Box<Chunk> {
        if let Some(prefix) = &self.store {
            if let Some(data) = crate::platform::storage_read(&chunk_key(prefix, cx, cz)) {
                match Chunk::decode(cx, cz, &data) {
                    Some(c) => return c,
                    None => crate::platform::log(&format!("Chunk {cx} {cz} in the save is damaged; regenerating it")),
                }
            }
        }
        let mut c = self.gen.terrain(cx, cz);
        // Pristine terrain can always be regenerated from the seed, so it is not written out.
        c.modified = false;
        c
    }

    fn write_chunk(prefix: &str, c: &mut Chunk) {
        crate::platform::storage_write(&chunk_key(prefix, c.cx, c.cz), &c.encode());
        c.modified = false;
    }

    /// Writes every chunk that changed since it was last saved.  Returns how many were written.
    pub fn save_modified(&mut self) -> usize {
        let Some(prefix) = self.store.clone() else { return 0 };
        let mut n = 0;
        for c in self.chunks.values_mut() {
            if c.modified && c.stage >= STAGE_TERRAIN {
                Self::write_chunk(&prefix, c);
                n += 1;
            }
        }
        n
    }

    // ------------------------------------------------------------ block access
    #[inline]
    pub fn chunk_at(&self, x: i32, z: i32) -> Option<&Chunk> {
        self.chunks.get(&(x >> 4, z >> 4)).map(|c| &**c)
    }
    #[inline]
    pub fn get(&self, x: i32, y: i32, z: i32) -> State {
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(c) => c.get((x & 15) as usize, y, (z & 15) as usize),
            None => AIR,
        }
    }
    #[inline]
    pub fn get_at(&self, p: IVec3) -> State {
        self.get(p.x, p.y, p.z)
    }
    /// Packed light (sky << 4 | block) at a position; unloaded space counts as open sky.
    pub fn light(&self, x: i32, y: i32, z: i32) -> u8 {
        match self.chunks.get(&(x >> 4, z >> 4)) {
            Some(c) => c.light((x & 15) as usize, y, (z & 15) as usize),
            None => 0xf0,
        }
    }
    /// True when the chunk containing (x, z) is fully generated and lit.
    pub fn is_ready(&self, x: i32, z: i32) -> bool {
        self.chunk_at(x, z).map_or(false, |c| c.stage >= STAGE_LIT)
    }
    pub fn biome(&self, x: i32, z: i32) -> biome::Biome {
        match self.chunk_at(x, z) {
            Some(c) => biome::Biome::from_u8(c.biome[((z & 15) * 16 + (x & 15)) as usize]),
            None => self.gen.biome_at(x, z),
        }
    }
    /// Y of the highest non-air block of a loaded column.
    pub fn top(&self, x: i32, z: i32) -> i32 {
        self.chunk_at(x, z).map_or(MIN_Y - 1, |c| c.top[((z & 15) * 16 + (x & 15)) as usize] as i32)
    }

    pub fn take_region(&mut self, cx: i32, cz: i32) -> Region {
        Region { cx0: cx - 1, cz0: cz - 1, size: 3, chunks: (0..9).map(|i| self.chunks.remove(&(cx - 1 + i % 3, cz - 1 + i / 3))).collect() }
    }
    pub fn put_region(&mut self, r: Region) {
        for (i, c) in r.chunks.into_iter().enumerate() {
            if let Some(c) = c {
                self.chunks.insert((r.cx0 + i as i32 % 3, r.cz0 + i as i32 / 3), c);
            }
        }
    }

    /// Marks the meshes that can see block (x, y, z) as out of date.
    fn mark_dirty(&mut self, x: i32, y: i32, z: i32) {
        let si = (y - MIN_Y) >> 4;
        let ly = (y - MIN_Y) & 15;
        let mut mask = 1u32 << si;
        if ly == 0 && si > 0 {
            mask |= 1 << (si - 1);
        }
        if ly == 15 && si < SECTIONS as i32 - 1 {
            mask |= 1 << (si + 1);
        }
        let (lx, lz) = (x & 15, z & 15);
        for dz in -1..=1 {
            for dx in -1..=1 {
                let hit_x = dx == 0 || (dx < 0 && lx == 0) || (dx > 0 && lx == 15);
                let hit_z = dz == 0 || (dz < 0 && lz == 0) || (dz > 0 && lz == 15);
                if hit_x && hit_z {
                    if let Some(c) = self.chunks.get_mut(&((x >> 4) + dx, (z >> 4) + dz)) {
                        c.mesh_dirty |= mask;
                    }
                }
            }
        }
    }

    /// Changes a block, updates light and schedules remeshing.  Returns the previous state,
    /// or `None` when the position is outside the loaded, lit world.
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, state: State) -> Option<State> {
        if !(MIN_Y..MAX_Y).contains(&y) || !self.is_ready(x, z) {
            return None;
        }
        let old = self.get(x, y, z);
        if old == state {
            return Some(old);
        }
        let r = reg();
        let (od, nd) = (r.data(old), r.data(state));
        let light_changes = od.occlude != nd.occlude || od.emit != nd.emit || od.filter != nd.filter;
        let mut region = self.take_region(x >> 4, z >> 4);
        region.set(x, y, z, state);
        if light_changes {
            self.lights.block_changed(&mut region, x, y, z);
        }
        self.put_region(region);
        self.mark_dirty(x, y, z);
        if self.track_changes {
            self.changed.push(IVec3::new(x, y, z));
        }
        Some(old)
    }
    pub fn set_at(&mut self, p: IVec3, state: State) -> Option<State> {
        self.set_block(p.x, p.y, p.z, state)
    }

    // ------------------------------------------------------------ generation pipeline
    fn stage_of(&self, cx: i32, cz: i32) -> u8 {
        self.chunks.get(&(cx, cz)).map_or(0, |c| c.stage)
    }

    /// Performs one unit of work towards lighting chunk (cx, cz).  Returns true if work was done.
    pub fn advance(&mut self, cx: i32, cz: i32) -> bool {
        if self.stage_of(cx, cz) >= STAGE_LIT {
            return false;
        }
        for nz in cz - 1..=cz + 1 {
            for nx in cx - 1..=cx + 1 {
                if self.stage_of(nx, nz) >= STAGE_DECORATED {
                    continue;
                }
                for mz in nz - 1..=nz + 1 {
                    for mx in nx - 1..=nx + 1 {
                        if self.stage_of(mx, mz) < STAGE_TERRAIN {
                            let chunk = self.load_or_generate(mx, mz);
                            self.chunks.insert((mx, mz), chunk);
                            return true;
                        }
                    }
                }
                let mut region = self.take_region(nx, nz);
                self.gen.decorate(&mut region, nx, nz);
                if let Some(c) = region.chunk_mut(nx * 16, nz * 16) {
                    c.stage = STAGE_DECORATED;
                    c.modified = true;
                }
                self.put_region(region);
                return true;
            }
        }
        let mut region = self.take_region(cx, cz);
        self.lights.light_chunk(&mut region, cx, cz);
        self.put_region(region);
        true
    }

    /// Generates and lights chunk (cx, cz) completely (blocking).
    pub fn ensure_lit(&mut self, cx: i32, cz: i32) {
        while self.advance(cx, cz) {}
    }

    /// Ensures a square of chunks around a block position is ready for gameplay and meshing.
    pub fn ensure_area(&mut self, x: i32, z: i32, radius: i32) {
        let (cx, cz) = (x >> 4, z >> 4);
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                self.ensure_lit(cx + dx, cz + dz);
            }
        }
    }

    /// Streaming step: advances the nearest unfinished chunk within `radius` of the centre chunk.
    /// Returns false when everything in range is lit.
    pub fn pump(&mut self, centre: (i32, i32), radius: i32) -> bool {
        if self.spiral_radius != radius {
            self.spiral.clear();
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    self.spiral.push((dx, dz));
                }
            }
            self.spiral.sort_by_key(|(dx, dz)| dx * dx + dz * dz);
            self.spiral_radius = radius;
            self.cursor = 0;
        }
        if self.cursor_centre != centre {
            self.cursor_centre = centre;
            self.cursor = 0;
        }
        while self.cursor < self.spiral.len() {
            let (dx, dz) = self.spiral[self.cursor];
            if self.advance(centre.0 + dx, centre.1 + dz) {
                return true;
            }
            self.cursor += 1;
        }
        false
    }

    /// Drops chunks farther than `radius` chunks from the centre, saving changed ones first.
    /// Returns how many chunks were unloaded.
    pub fn unload_far(&mut self, centre: (i32, i32), radius: i32) -> usize {
        let far: Vec<(i32, i32)> = self.chunks.keys().copied().filter(|(cx, cz)| (cx - centre.0).abs() > radius || (cz - centre.1).abs() > radius).collect();
        if !far.is_empty() {
            self.cursor = 0;
        }
        let prefix = self.store.clone();
        for k in &far {
            if let Some(mut c) = self.chunks.remove(k) {
                if let (Some(prefix), true) = (&prefix, c.modified && c.stage >= STAGE_TERRAIN) {
                    Self::write_chunk(prefix, &mut c);
                }
            }
        }
        far.len()
    }

    /// First position at or above `y` in a column where a player-sized gap exists over solid ground.
    pub fn find_standing_y(&self, x: i32, z: i32) -> Option<i32> {
        let r = reg();
        let mut y = self.top(x, z).min(MAX_Y - 3);
        while y > MIN_Y {
            let s = self.get(x, y, z);
            if r.has(s, block::F_SOLID) {
                return if r.has(s, block::F_OPAQUE) || !r.collision(s).is_empty() { Some(y + 1) } else { None };
            }
            if r.has(s, block::F_FLUID) {
                return None;
            }
            y -= 1;
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::block::*;
    use super::mesh::Mesher;
    use super::*;
    use crate::ffi::SEA_LEVEL;

    fn sky(w: &World, x: i32, y: i32, z: i32) -> u8 {
        w.light(x, y, z) >> 4
    }
    fn blk(w: &World, x: i32, y: i32, z: i32) -> u8 {
        w.light(x, y, z) & 15
    }

    /// A world with a hand-built stone slab instead of generated terrain, so light tests are exact.
    fn flat_world() -> World {
        let mut w = World::new(5);
        let stone = reg().k.stone;
        for cz in -2..=2 {
            for cx in -2..=2 {
                let mut c = Chunk::new(cx, cz);
                for z in 0..16 {
                    for x in 0..16 {
                        for y in 0..=60 {
                            c.set(x, y, z, stone);
                        }
                    }
                }
                c.stage = STAGE_DECORATED;
                w.chunks.insert((cx, cz), c);
            }
        }
        for cz in -1..=1 {
            for cx in -1..=1 {
                let mut r = w.take_region(cx, cz);
                w.lights.light_chunk(&mut r, cx, cz);
                w.put_region(r);
            }
        }
        w
    }

    #[test]
    fn sky_light_fills_open_air_and_stops_at_ground() {
        let w = flat_world();
        assert_eq!(sky(&w, 3, 61, 3), 15);
        assert_eq!(sky(&w, 3, 200, 3), 15);
        assert_eq!(sky(&w, 3, 60, 3), 0);
        assert_eq!(blk(&w, 3, 61, 3), 0);
    }

    #[test]
    fn torch_light_spreads_and_is_removed() {
        let mut w = flat_world();
        let r = reg();
        // Dig a sealed pocket so only the torch lights it.
        for x in 2..=8 {
            w.set_block(x, 40, 5, AIR).unwrap();
        }
        assert_eq!(w.light(5, 40, 5), 0);
        w.set_block(2, 40, 5, r.k.torch).unwrap();
        assert_eq!(blk(&w, 2, 40, 5), 14);
        assert_eq!(blk(&w, 3, 40, 5), 13);
        assert_eq!(blk(&w, 8, 40, 5), 8);
        assert_eq!(blk(&w, 9, 40, 5), 0, "light must not enter solid stone");
        // A second, weaker emitter must survive removal of the torch.
        w.set_block(8, 40, 5, r.k.magma_block).unwrap();
        w.set_block(2, 40, 5, AIR).unwrap();
        assert_eq!(blk(&w, 2, 40, 5), 0);
        assert_eq!(blk(&w, 7, 40, 5), 2);
        assert_eq!(blk(&w, 5, 40, 5), 0);
    }

    #[test]
    fn digging_a_shaft_lets_sky_light_in_and_covering_removes_it() {
        let mut w = flat_world();
        let r = reg();
        for y in (50..=60).rev() {
            w.set_block(4, y, 4, AIR).unwrap();
        }
        assert_eq!(sky(&w, 4, 50, 4), 15);
        w.set_block(5, 50, 4, AIR).unwrap();
        assert_eq!(sky(&w, 5, 50, 4), 14);
        w.set_block(4, 60, 4, r.k.stone).unwrap();
        assert_eq!(sky(&w, 4, 59, 4), 0);
        assert_eq!(sky(&w, 5, 50, 4), 0);
        // A bottom slab closes its own cell from below but not from above.
        w.set_block(4, 60, 4, AIR).unwrap();
        assert_eq!(sky(&w, 4, 50, 4), 15);
        let slab = r.parse_state("stone_slab[type=bottom]").unwrap();
        w.set_block(4, 60, 4, slab).unwrap();
        assert_eq!(sky(&w, 4, 60, 4), 15);
        assert_eq!(sky(&w, 4, 59, 4), 0);
        // Light crosses chunk borders.
        w.set_block(0, 61, 0, r.k.glowstone).unwrap();
        assert_eq!(blk(&w, -1, 61, 0), 14);
        assert_eq!(blk(&w, -3, 61, -2), 10);
    }

    #[test]
    fn generated_world_lights_and_meshes() {
        let mut w = World::new(1);
        w.ensure_area(8, 8, 1);
        assert!(w.is_ready(8, 8));
        let top = w.top(8, 8);
        assert!(top >= SEA_LEVEL - 1);
        assert_eq!(sky(&w, 8, top + 3, 8), 15);
        assert_eq!(w.light(8, MIN_Y + 1, 8) >> 4, 0);
        let mut m = Mesher::default();
        let mut quads = 0;
        for si in 0..SECTIONS {
            assert!(m.build(&w.chunks, 0, si, 0));
            quads += m.counts[0] + m.counts[1];
            for v in &m.opaque[..m.counts[0] as usize * 4] {
                assert!((-64..=16 * 256 + 64).contains(&(v.x as i32)) && (v.layer as usize) < reg().textures.len());
            }
        }
        assert!(quads > 200, "only {quads} quads");
        // Chunks whose neighbours are missing cannot be meshed yet.
        assert!(!m.build(&w.chunks, 2, 5, 0));
        // Streaming reaches a steady state and unloading removes distant chunks.
        let mut steps = 0;
        while w.pump((0, 0), 2) {
            steps += 1;
            assert!(steps < 2000);
        }
        assert!(w.is_ready(-32, 32));
        let before = w.chunks.len();
        let removed = w.unload_far((0, 0), 1);
        assert_eq!(w.chunks.len(), 9);
        assert_eq!(removed, before - 9);
    }
}
