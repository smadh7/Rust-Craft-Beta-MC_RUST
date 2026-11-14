//! Block simulation: fluids that flow, plants that grow and spread, leaves that decay.
//!
//! The world records every block change; each game tick the simulation looks at those positions,
//! wakes the fluids next to them and runs the fluid ticks that have come due.  On top of that a
//! few blocks per chunk section receive a "random tick", which is what makes crops grow.

use super::biome::Tree;
use super::block::*;
use super::chunk::STAGE_LIT;
use super::model::Shape;
use super::{rules, World};
use crate::ffi::{MAX_Y, MIN_Y};
use crate::util::math::{IVec3, DOWN, OPPOSITE, UP};
use crate::util::rng::Rng;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashSet};

const WATER_DELAY: u64 = 5;
const LAVA_DELAY: u64 = 30;
/// Upper bound on scheduled ticks processed per game tick, so a flood cannot stall the game.
const MAX_TICKS_PER_STEP: usize = 1200;
/// Random ticks per 16x16x16 section each game tick.
const RANDOM_TICK_SPEED: u32 = 3;
/// A leaf block further than this from any log decays.
const LEAF_REACH: u8 = 6;

pub struct Sim {
    queue: BinaryHeap<Reverse<(u64, u64, i32, i32, i32)>>,
    queued: HashSet<IVec3>,
    seq: u64,
    rng: Rng,
    /// Per state: reacts to random ticks.
    ticking: Vec<bool>,
    /// Blocks the simulation destroyed (washed away, decayed), for the game to turn into drops.
    pub broken: Vec<(IVec3, State)>,
    /// Places where lava met water this tick.
    pub fizz: Vec<IVec3>,
    /// Sky light is reduced by this much at the moment (0 at noon, 11 at midnight).
    pub sky_darken: u8,
    /// Set to false to freeze growth and decay (fluids still flow).
    pub random_ticks: bool,
}

fn is_log(name: &str) -> bool {
    name.ends_with("_log") || name.ends_with("_wood") || name.ends_with("_stem")
}

fn tree_for(name: &str) -> Option<Tree> {
    Some(match name {
        "oak_sapling" => Tree::Oak,
        "birch_sapling" => Tree::Birch,
        "spruce_sapling" => Tree::Spruce,
        "jungle_sapling" => Tree::Jungle,
        "acacia_sapling" => Tree::Acacia,
        "dark_oak_sapling" => Tree::DarkOak,
        "cherry_sapling" => Tree::Cherry,
        _ => return None,
    })
}

/// A block a fluid may flow into, destroying whatever small thing was there.
fn can_hold(s: State) -> bool {
    if s == AIR {
        return true;
    }
    let r = reg();
    !r.has(s, F_SOLID | F_FLUID | F_WATER) && !matches!(r.block(s).name, "sugar_cane" | "lily_pad")
}

fn same_fluid(s: State, lava: bool) -> bool {
    let r = reg();
    r.has(s, F_FLUID) && r.has(s, F_LAVA) == lava
}

/// Water source blocks and waterlogged blocks both feed neighbouring water.
fn is_source(s: State, lava: bool) -> bool {
    let r = reg();
    if r.has(s, F_FLUID) {
        r.has(s, F_LAVA) == lava && r.prop(s, "level") == Some(0)
    } else {
        !lava && r.has(s, F_WATER)
    }
}

fn level_of(s: State) -> usize {
    let r = reg();
    if r.has(s, F_FLUID) { r.prop(s, "level").unwrap_or(0) } else { 0 }
}

fn fluid_state(lava: bool, level: usize) -> State {
    let r = reg();
    r.with(if lava { r.k.lava } else { r.k.water }, "level", level)
}

impl Sim {
    pub fn new(seed: u64) -> Sim {
        let r = reg();
        let ticking = (0..r.states.len())
            .map(|s| {
                let b = r.block(s as State);
                matches!(b.name, "grass_block" | "mycelium" | "farmland" | "sugar_cane" | "cactus") || matches!(b.shape, Shape::Crop(_)) || b.flags & F_LEAVES != 0 || tree_for(b.name).is_some()
            })
            .collect();
        Sim { queue: BinaryHeap::new(), queued: HashSet::new(), seq: 0, rng: Rng::new(seed ^ 0x51b0_77e5), ticking, broken: Vec::new(), fizz: Vec::new(), sky_darken: 0, random_ticks: true }
    }

    /// Number of scheduled ticks waiting.
    pub fn pending(&self) -> usize {
        self.queue.len()
    }

    /// Positions with a scheduled tick (saved with the world so flows resume after loading).
    pub fn scheduled(&self) -> Vec<IVec3> {
        self.queued.iter().copied().collect()
    }

    pub fn schedule(&mut self, p: IVec3, due: u64) {
        if self.queued.insert(p) {
            self.seq += 1;
            self.queue.push(Reverse((due, self.seq, p.x, p.y, p.z)));
        }
    }

    /// Advances the simulation by one game tick around the given chunk.
    pub fn step(&mut self, world: &mut World, now: u64, centre: (i32, i32), radius: i32) {
        self.absorb_changes(world, now);
        let mut budget = MAX_TICKS_PER_STEP;
        while budget > 0 {
            match self.queue.peek() {
                Some(Reverse((due, ..))) if *due <= now => {}
                _ => break,
            }
            let Some(Reverse((_, _, x, y, z))) = self.queue.pop() else { break };
            let p = IVec3::new(x, y, z);
            self.queued.remove(&p);
            budget -= 1;
            // Fluids at the edge of the loaded world wait: their neighbours are unknown.
            if !(world.is_ready(x - 1, z) && world.is_ready(x + 1, z) && world.is_ready(x, z - 1) && world.is_ready(x, z + 1)) {
                continue;
            }
            let s = world.get_at(p);
            let r = reg();
            if r.has(s, F_FLUID) {
                self.fluid_tick(world, p, s);
            } else if r.has(s, F_WATER) {
                self.spread(world, p, false, 0);
            }
            self.absorb_changes(world, now);
        }
        if self.random_ticks {
            self.run_random_ticks(world, centre, radius);
            self.absorb_changes(world, now);
        }
    }

    /// Looks at every block changed since the last call: neighbours that lost their support
    /// break, and fluids next to the change are scheduled.
    fn absorb_changes(&mut self, world: &mut World, now: u64) {
        let r = reg();
        for _ in 0..8 {
            if world.changed.is_empty() {
                break;
            }
            let changed = std::mem::take(&mut world.changed);
            for p in changed {
                let s = world.get_at(p);
                if s == AIR || r.has(s, F_FLUID) {
                    let popped = rules::notify_neighbours(world, p);
                    self.broken.extend(popped);
                }
                self.wake(world, p, now);
                for d in 0..6 {
                    self.wake(world, p.offset(d), now);
                }
            }
        }
        if world.changed.len() > 100_000 {
            world.changed.clear();
        }
    }

    fn wake(&mut self, world: &mut World, p: IVec3, now: u64) {
        let r = reg();
        let s = world.get_at(p);
        if r.has(s, F_FLUID) {
            let lava = r.has(s, F_LAVA);
            if lava && self.lava_meets_water(world, p, s) {
                return;
            }
            self.schedule(p, now + if lava { LAVA_DELAY } else { WATER_DELAY });
        } else if r.has(s, F_WATER) {
            self.schedule(p, now + WATER_DELAY);
        }
    }

    /// Lava touched by water from above or the side hardens: sources to obsidian, flows to cobblestone.
    fn lava_meets_water(&mut self, world: &mut World, p: IVec3, s: State) -> bool {
        let r = reg();
        if !(1..6).any(|d| r.has(world.get_at(p.offset(d)), F_WATER)) {
            return false;
        }
        let rock = if level_of(s) == 0 { r.k.obsidian } else { r.k.cobblestone };
        if world.set_at(p, rock).is_none() {
            return false;
        }
        self.fizz.push(p);
        true
    }

    // ------------------------------------------------------------ fluids
    fn fluid_tick(&mut self, world: &mut World, p: IVec3, s: State) {
        let lava = reg().has(s, F_LAVA);
        let mut level = level_of(s);
        if level != 0 {
            match self.new_level(world, p, lava) {
                None => {
                    world.set_at(p, AIR);
                    return;
                }
                Some(l) if l != level => {
                    world.set_at(p, fluid_state(lava, l));
                    level = l;
                }
                _ => {}
            }
        }
        self.spread(world, p, lava, level);
    }

    /// The level a non-source fluid block should have, judging by its neighbours.
    fn new_level(&self, world: &World, p: IVec3, lava: bool) -> Option<usize> {
        let r = reg();
        let (mut most, mut sources) = (0i32, 0);
        for d in 2..6 {
            let n = world.get_at(p.offset(d));
            if is_source(n, lava) {
                sources += 1;
                most = 8;
            } else if same_fluid(n, lava) {
                let l = level_of(n) as i32;
                most = most.max(if l >= 8 { 8 } else { 8 - l });
            }
        }
        // Two neighbouring water sources over solid ground make a new source.
        if !lava && sources >= 2 {
            let below = world.get_at(p.offset(DOWN));
            if r.has(below, F_SOLID) || is_source(below, false) {
                return Some(0);
            }
        }
        let above = world.get_at(p.offset(UP));
        if same_fluid(above, lava) || (!lava && r.has(above, F_WATER)) {
            return Some(8);
        }
        let amount = most - if lava { 2 } else { 1 };
        if amount <= 0 { None } else { Some((8 - amount) as usize) }
    }

    fn spread(&mut self, world: &mut World, p: IVec3, lava: bool, level: usize) {
        let r = reg();
        let below_pos = p.offset(DOWN);
        let below = world.get_at(below_pos);
        if below_pos.y >= MIN_Y {
            if can_hold(below) {
                self.flow_into(world, below_pos, below, lava, 8);
                if (2..6).filter(|d| is_source(world.get_at(p.offset(*d)), lava)).count() >= 3 {
                    self.spread_sides(world, p, lava, level);
                }
                return;
            }
            if lava && r.has(below, F_FLUID) && r.has(below, F_WATER) {
                // Lava pouring onto water turns it to stone.
                if world.set_at(below_pos, r.k.stone).is_some() {
                    self.fizz.push(below_pos);
                }
                return;
            }
        }
        let fluid_below = same_fluid(below, lava) || (!lava && r.has(below, F_WATER));
        if level == 0 || !fluid_below {
            self.spread_sides(world, p, lava, level);
        }
    }

    fn spread_sides(&mut self, world: &mut World, p: IVec3, lava: bool, level: usize) {
        let amount = if level >= 8 { 7 } else { 8 - level as i32 - if lava { 2 } else { 1 } };
        if amount <= 0 {
            return;
        }
        for d in self.flow_dirs(world, p, lava) {
            let n = p.offset(d);
            let ns = world.get_at(n);
            if can_hold(ns) {
                self.flow_into(world, n, ns, lava, (8 - amount) as usize);
            }
        }
    }

    fn flow_into(&mut self, world: &mut World, pos: IVec3, old: State, lava: bool, level: usize) {
        if world.set_at(pos, fluid_state(lava, level)).is_none() {
            return;
        }
        // Whatever stood there is washed away (lava simply burns it).
        if old != AIR && !lava {
            self.broken.push((pos, old));
        }
    }

    /// Something a fluid can move through on its way to a drop.
    fn passable(s: State, lava: bool) -> bool {
        can_hold(s) || (same_fluid(s, lava) && level_of(s) != 0)
    }

    fn is_hole(world: &World, below: IVec3, lava: bool) -> bool {
        let s = world.get_at(below);
        below.y >= MIN_Y && (can_hold(s) || same_fluid(s, lava))
    }

    /// Horizontal directions a fluid flows in: towards the nearest drop if there is one within
    /// a few blocks, otherwise every open side.
    fn flow_dirs(&self, world: &World, p: IVec3, lava: bool) -> Vec<usize> {
        let reach = if lava { 2 } else { 4 };
        let mut best = i32::MAX;
        let mut dirs = Vec::with_capacity(4);
        for d in 2..6 {
            let n = p.offset(d);
            if !Self::passable(world.get_at(n), lava) {
                continue;
            }
            let dist = if Self::is_hole(world, n.offset(DOWN), lava) { 0 } else { Self::slope_distance(world, n, 1, OPPOSITE[d], lava, reach) };
            if dist < best {
                dirs.clear();
                best = dist;
            }
            if dist <= best {
                dirs.push(d);
            }
        }
        dirs
    }

    fn slope_distance(world: &World, p: IVec3, depth: i32, from: usize, lava: bool, reach: i32) -> i32 {
        let mut best = 1000;
        for d in 2..6 {
            if d == from {
                continue;
            }
            let n = p.offset(d);
            if !Self::passable(world.get_at(n), lava) {
                continue;
            }
            if Self::is_hole(world, n.offset(DOWN), lava) {
                return depth;
            }
            if depth < reach {
                best = best.min(Self::slope_distance(world, n, depth + 1, OPPOSITE[d], lava, reach));
            }
        }
        best
    }

    // ------------------------------------------------------------ random ticks
    fn run_random_ticks(&mut self, world: &mut World, centre: (i32, i32), radius: i32) {
        let mut hits: Vec<(IVec3, State)> = Vec::new();
        for cz in centre.1 - radius..=centre.1 + radius {
            for cx in centre.0 - radius..=centre.0 + radius {
                let Some(c) = world.chunks.get(&(cx, cz)) else { continue };
                if c.stage < STAGE_LIT {
                    continue;
                }
                for (si, sec) in c.sections.iter().enumerate() {
                    let Some(sec) = sec else { continue };
                    if sec.non_air == 0 {
                        continue;
                    }
                    for _ in 0..RANDOM_TICK_SPEED {
                        let v = self.rng.next_u32() as usize;
                        let (x, y, z) = (v & 15, (v >> 4) & 15, (v >> 8) & 15);
                        let s = sec.blocks[super::chunk::sindex(x, y, z)];
                        if self.ticking[s as usize] {
                            hits.push((IVec3::new(cx * 16 + x as i32, MIN_Y + (si * 16 + y) as i32, cz * 16 + z as i32), s));
                        }
                    }
                }
            }
        }
        for (p, s) in hits {
            if world.get_at(p) == s && world.is_ready(p.x - 8, p.z - 8) && world.is_ready(p.x + 8, p.z + 8) && world.is_ready(p.x - 8, p.z + 8) && world.is_ready(p.x + 8, p.z - 8) {
                self.random_tick(world, p, s);
            }
        }
    }

    /// Brightness the way plants see it: block light, or sky light reduced at night.
    fn brightness(&self, world: &World, p: IVec3) -> u8 {
        let l = world.light(p.x, p.y, p.z);
        (l & 15).max((l >> 4).saturating_sub(self.sky_darken))
    }

    fn random_tick(&mut self, world: &mut World, p: IVec3, s: State) {
        let r = reg();
        let b = r.block(s);
        let up = p.offset(UP);
        let above = world.get_at(up);
        match b.name {
            "grass_block" | "mycelium" => {
                let smothered = |s: State| r.has(s, F_OPAQUE) || r.has(s, F_FLUID);
                if smothered(above) {
                    world.set_at(p, r.k.dirt);
                } else if self.brightness(world, up) >= 9 {
                    for _ in 0..4 {
                        let q = IVec3::new(p.x + self.rng.range(-1, 1), p.y + self.rng.range(-3, 1), p.z + self.rng.range(-1, 1));
                        let q_up = q.offset(UP);
                        if world.get_at(q) == r.k.dirt && !smothered(world.get_at(q_up)) && self.brightness(world, q_up) >= 4 {
                            let snowy = r.block(world.get_at(q_up)).name.starts_with("snow");
                            let spread = r.default_of(b);
                            world.set_at(q, if snowy { r.with(spread, "snowy", 1) } else { spread });
                        }
                    }
                }
            }
            "farmland" => {
                let moisture = r.prop(s, "moisture").unwrap_or(0);
                let wet = (-4..=4).any(|dz| (-4..=4).any(|dx| (0..=1).any(|dy| r.has(world.get(p.x + dx, p.y + dy, p.z + dz), F_WATER))));
                if r.has(above, F_OPAQUE) {
                    world.set_at(p, r.k.dirt);
                } else if wet {
                    if moisture < 7 {
                        world.set_at(p, r.with(s, "moisture", 7));
                    }
                } else if moisture > 0 {
                    world.set_at(p, r.with(s, "moisture", moisture - 1));
                } else if !matches!(r.block(above).shape, Shape::Crop(_)) {
                    world.set_at(p, r.k.dirt);
                }
            }
            "sugar_cane" | "cactus" => {
                // Grows to three blocks tall.
                if above == AIR && self.rng.one_in(16) {
                    let height = (1..3).take_while(|i| r.block(world.get(p.x, p.y - i, p.z)).name == b.name).count();
                    if height < 2 && rules::can_survive(world, up, s) {
                        world.set_at(up, s);
                    }
                }
            }
            _ => {
                if let Shape::Crop(stages) = b.shape {
                    let age = r.prop(s, "age").unwrap_or(0);
                    if age + 1 < stages as usize && self.brightness(world, p) >= 9 {
                        let speed = self.growth_speed(world, p);
                        if self.rng.below((25.0 / speed) as u32 + 1) == 0 {
                            world.set_at(p, r.with(s, "age", age + 1));
                        }
                    }
                } else if let Some(kind) = tree_for(b.name) {
                    if self.brightness(world, up) >= 9 && self.rng.one_in(14) {
                        self.grow_tree(world, p, kind);
                    }
                } else if b.flags & F_LEAVES != 0 && !self.leaf_supported(world, p) {
                    world.set_at(p, AIR);
                    self.broken.push((p, s));
                }
            }
        }
    }

    /// How quickly a crop grows: best on wet farmland surrounded by more wet farmland.
    fn growth_speed(&self, world: &World, p: IVec3) -> f32 {
        let r = reg();
        let mut speed = 1.0;
        for dz in -1..=1 {
            for dx in -1..=1 {
                let soil = world.get(p.x + dx, p.y - 1, p.z + dz);
                if r.block(soil).name == "farmland" {
                    let value = if r.prop(soil, "moisture").unwrap_or(0) > 0 { 3.0 } else { 1.0 };
                    speed += if dx == 0 && dz == 0 { value } else { value / 4.0 };
                }
            }
        }
        speed
    }

    /// True when a log can be reached from the leaf block through at most `LEAF_REACH` leaves.
    fn leaf_supported(&self, world: &World, start: IVec3) -> bool {
        const N: i32 = LEAF_REACH as i32 * 2 + 1;
        let r = reg();
        let mut seen = [false; (N * N * N) as usize];
        let index = |p: IVec3| {
            let (dx, dy, dz) = (p.x - start.x + LEAF_REACH as i32, p.y - start.y + LEAF_REACH as i32, p.z - start.z + LEAF_REACH as i32);
            ((dy * N + dz) * N + dx) as usize
        };
        let mut frontier = vec![start];
        seen[index(start)] = true;
        for _ in 0..LEAF_REACH {
            let mut next = Vec::new();
            for p in frontier {
                for d in 0..6 {
                    let n = p.offset(d);
                    let s = world.get_at(n);
                    if s == AIR {
                        continue;
                    }
                    let b = r.block(s);
                    if is_log(b.name) {
                        return true;
                    }
                    if b.flags & F_LEAVES != 0 && !seen[index(n)] {
                        seen[index(n)] = true;
                        next.push(n);
                    }
                }
            }
            if next.is_empty() {
                return false;
            }
            frontier = next;
        }
        false
    }

    /// Replaces a sapling with a tree when there is room for the trunk.
    pub fn grow_tree(&mut self, world: &mut World, p: IVec3, kind: Tree) -> bool {
        let r = reg();
        let sapling = world.get_at(p);
        let clear = (1..5).all(|i| {
            let s = world.get(p.x, p.y + i, p.z);
            s == AIR || r.has(s, F_LEAVES)
        });
        if !clear || p.y + 40 >= MAX_Y {
            return false;
        }
        // Let the generator build the tree into a scratch copy of the area, note what it
        // changed, undo it, and then apply the changes properly so light and meshes follow.
        let mut region = world.take_region(p.x >> 4, p.z >> 4);
        let (x0, x1, y0, y1, z0, z1) = (p.x - 8, p.x + 8, p.y - 1, p.y + 39, p.z - 8, p.z + 8);
        let mut before = Vec::with_capacity(17 * 17 * 41);
        for y in y0..=y1 {
            for z in z0..=z1 {
                for x in x0..=x1 {
                    before.push(region.get(x, y, z));
                }
            }
        }
        region.set(p.x, p.y, p.z, AIR);
        let mut rng = Rng::new(self.rng.next_u64());
        let grown = world.gen.place_tree(&mut region, &mut rng, kind, p.x, p.y, p.z);
        let mut changes = Vec::new();
        let mut i = 0;
        for y in y0..=y1 {
            for z in z0..=z1 {
                for x in x0..=x1 {
                    let now = region.get(x, y, z);
                    if now != before[i] {
                        changes.push((IVec3::new(x, y, z), now));
                        region.set(x, y, z, before[i]);
                    }
                    i += 1;
                }
            }
        }
        world.put_region(region);
        if !grown {
            return false;
        }
        for (pos, state) in changes {
            world.set_at(pos, state);
        }
        world.get_at(p) != sapling
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::Chunk;

    /// A 3x3 chunk world: stone up to y 60, air above.
    fn flat() -> World {
        let mut w = World::new(1);
        let stone = reg().k.stone;
        for cz in -1..=1 {
            for cx in -1..=1 {
                let mut c = Chunk::new(cx, cz);
                for z in 0..16 {
                    for x in 0..16 {
                        for y in 50..=60 {
                            c.set(x, y, z, stone);
                        }
                    }
                }
                c.stage = STAGE_LIT;
                w.chunks.insert((cx, cz), c);
            }
        }
        w.track_changes = true;
        w
    }

    fn run(sim: &mut Sim, w: &mut World, ticks: u64) {
        for t in 1..=ticks {
            sim.step(w, t, (0, 0), 0);
        }
    }

    fn sim() -> Sim {
        let mut s = Sim::new(1);
        s.random_ticks = false;
        s
    }

    #[test]
    fn water_spreads_seven_blocks_and_recedes() {
        let r = reg();
        let (mut w, mut s) = (flat(), sim());
        w.set_block(8, 61, 8, r.k.water);
        run(&mut s, &mut w, 60);
        for d in 1..=7 {
            let st = w.get(8 + d, 61, 8);
            assert!(r.has(st, F_FLUID), "no water {d} blocks out");
            assert_eq!(r.prop(st, "level"), Some(d as usize));
        }
        assert_eq!(w.get(16, 61, 8), AIR, "water must stop after seven blocks");
        // Diagonal spread follows the taxicab distance.
        assert_eq!(r.prop(w.get(10, 61, 10), "level"), Some(4));
        assert_eq!(w.get(12, 61, 12), AIR);
        // Removing the source dries everything up again.
        w.set_block(8, 61, 8, AIR);
        run(&mut s, &mut w, 120);
        for x in 0..16 {
            for z in 0..16 {
                assert_eq!(w.get(x, 61, z), AIR, "water left at {x} {z}");
            }
        }
        assert_eq!(s.pending(), 0);
    }

    #[test]
    fn water_falls_and_prefers_the_nearest_drop() {
        let r = reg();
        let (mut w, mut s) = (flat(), sim());
        // A hole two blocks east of the source.
        for y in 55..=60 {
            w.set_block(10, y, 8, AIR);
        }
        w.set_block(8, 61, 8, r.k.water);
        run(&mut s, &mut w, 80);
        assert!(r.has(w.get(9, 61, 8), F_FLUID));
        assert_eq!(w.get(7, 61, 8), AIR, "water should only flow towards the hole");
        assert_eq!(w.get(8, 61, 7), AIR);
        // It pours down the shaft as falling water and spreads out at the bottom.
        for y in 55..=60 {
            assert_eq!(r.prop(w.get(10, y, 8), "level"), Some(8), "y {y}");
        }
        assert_eq!(w.get(10, 54, 8), r.k.stone);
    }

    #[test]
    fn two_sources_make_a_third() {
        let r = reg();
        let (mut w, mut s) = (flat(), sim());
        // A one-deep trench three blocks long with sources at both ends.
        for x in 7..=9 {
            w.set_block(x, 60, 8, AIR);
        }
        w.set_block(7, 60, 8, r.k.water);
        w.set_block(9, 60, 8, r.k.water);
        run(&mut s, &mut w, 30);
        assert_eq!(w.get(8, 60, 8), r.k.water, "the middle block should have become a source");
        // Lava never does that.
        let (mut w, mut s) = (flat(), sim());
        for x in 7..=9 {
            w.set_block(x, 60, 8, AIR);
        }
        w.set_block(7, 60, 8, r.k.lava);
        w.set_block(9, 60, 8, r.k.lava);
        run(&mut s, &mut w, 100);
        assert_eq!(r.prop(w.get(8, 60, 8), "level"), Some(2));
    }

    #[test]
    fn lava_and_water_make_stone() {
        let r = reg();
        // Water flowing into a lava source: obsidian.
        let (mut w, mut s) = (flat(), sim());
        w.set_block(8, 60, 8, r.k.lava);
        w.set_block(8, 61, 8, r.k.water);
        run(&mut s, &mut w, 10);
        assert_eq!(w.get(8, 60, 8), r.k.obsidian);
        assert!(!s.fizz.is_empty());
        // Flowing lava touched by water: cobblestone.
        let (mut w, mut s) = (flat(), sim());
        w.set_block(4, 61, 8, r.k.lava);
        run(&mut s, &mut w, 100);
        assert_eq!(r.prop(w.get(6, 61, 8), "level"), Some(4), "lava thins by two per block");
        assert_eq!(w.get(8, 61, 8), AIR, "lava reaches three blocks");
        w.set_block(7, 62, 8, r.k.stone);
        w.set_block(7, 61, 8, r.k.water);
        run(&mut s, &mut w, 20);
        assert_eq!(w.get(6, 61, 8), r.k.cobblestone);
        // Lava falling onto water: stone.
        let (mut w, mut s) = (flat(), sim());
        w.set_block(8, 60, 8, r.k.water);
        w.set_block(8, 63, 8, r.k.lava);
        run(&mut s, &mut w, 200);
        assert_eq!(w.get(8, 60, 8), r.k.stone);
    }

    #[test]
    fn water_washes_plants_away_but_not_solid_things() {
        let r = reg();
        let (mut w, mut s) = (flat(), sim());
        let dirt = r.k.dirt;
        w.set_block(9, 60, 8, dirt);
        w.set_block(9, 61, 8, r.k.short_grass);
        w.set_block(7, 61, 8, r.k.glass);
        w.set_block(8, 61, 8, r.k.water);
        run(&mut s, &mut w, 20);
        assert!(r.has(w.get(9, 61, 8), F_FLUID));
        assert!(s.broken.iter().any(|(p, st)| *p == IVec3::new(9, 61, 8) && *st == r.k.short_grass));
        assert_eq!(w.get(7, 61, 8), r.k.glass);
    }

    #[test]
    fn crops_grow_and_farmland_dries() {
        let r = reg();
        let (mut w, mut s) = (flat(), Sim::new(7));
        let farmland = r.with(r.k.farmland, "moisture", 7);
        let wheat = r.default_state("wheat").unwrap();
        for x in 2..=4 {
            for z in 2..=4 {
                w.set_block(x, 60, z, farmland);
                w.set_block(x, 61, z, wheat);
            }
        }
        // Dry farmland with nothing planted far from the water.
        w.set_block(12, 60, 12, r.k.farmland);
        // Water next to the field keeps it wet.
        w.set_block(1, 60, 3, r.k.water);
        w.changed.clear();
        for _ in 0..40_000 {
            let p = IVec3::new(s.rng.range(2, 4), 61, s.rng.range(2, 4));
            let st = w.get_at(p);
            s.random_tick(&mut w, p, st);
            let f = IVec3::new(p.x, 60, p.z);
            let st = w.get_at(f);
            s.random_tick(&mut w, f, st);
        }
        for x in 2..=4 {
            for z in 2..=4 {
                assert_eq!(r.prop(w.get(x, 61, z), "age"), Some(7), "wheat at {x} {z} is not ripe");
                assert_eq!(r.prop(w.get(x, 60, z), "moisture"), Some(7));
            }
        }
        for _ in 0..20 {
            let st = w.get(12, 60, 12);
            s.random_tick(&mut w, IVec3::new(12, 60, 12), st);
        }
        assert_eq!(w.get(12, 60, 12), r.k.dirt, "dry empty farmland reverts to dirt");
    }

    #[test]
    fn grass_spreads_in_light_and_dies_when_covered() {
        let r = reg();
        let (mut w, mut s) = (flat(), Sim::new(3));
        for x in 4..=12 {
            for z in 4..=12 {
                w.set_block(x, 60, z, r.k.dirt);
            }
        }
        w.set_block(8, 60, 8, r.k.grass_block);
        for _ in 0..3000 {
            for x in 4..=12 {
                for z in 4..=12 {
                    let p = IVec3::new(x, 60, z);
                    let st = w.get_at(p);
                    if st == r.k.grass_block {
                        s.random_tick(&mut w, p, st);
                    }
                }
            }
        }
        let grass = (4..=12).flat_map(|x| (4..=12).map(move |z| (x, z))).filter(|(x, z)| w.get(*x, 60, *z) == r.k.grass_block).count();
        assert_eq!(grass, 81, "grass should have covered all the dirt");
        w.set_block(8, 61, 8, r.k.stone);
        let st = w.get(8, 60, 8);
        s.random_tick(&mut w, IVec3::new(8, 60, 8), st);
        assert_eq!(w.get(8, 60, 8), r.k.dirt);
        // No spreading in the dark.
        s.sky_darken = 11;
        w.set_block(5, 60, 5, r.k.dirt);
        for _ in 0..200 {
            for (x, z) in [(4, 4), (4, 5), (4, 6), (5, 4), (5, 6), (6, 4), (6, 5), (6, 6)] {
                let st = w.get(x, 60, z);
                s.random_tick(&mut w, IVec3::new(x, 60, z), st);
            }
        }
        assert_eq!(w.get(5, 60, 5), r.k.dirt);
    }

    #[test]
    fn saplings_grow_and_orphaned_leaves_decay() {
        let r = reg();
        let (mut w, mut s) = (flat(), Sim::new(5));
        w.set_block(8, 60, 8, r.k.dirt);
        let sapling = r.default_state("oak_sapling").unwrap();
        w.set_block(8, 61, 8, sapling);
        assert!(s.grow_tree(&mut w, IVec3::new(8, 61, 8), Tree::Oak));
        let log = r.default_state("oak_log").unwrap();
        let leaves = r.default_state("oak_leaves").unwrap();
        assert_eq!(r.block(w.get(8, 61, 8)).name, "oak_log");
        let count = |w: &World, st: State| (0..16).flat_map(|x| (0..16).flat_map(move |z| (61..80).map(move |y| (x, y, z)))).filter(|(x, y, z)| r.block(w.get(*x, *y, *z)).name == r.block(st).name).count();
        let (logs, leaf_count) = (count(&w, log), count(&w, leaves));
        assert!(logs >= 4 && leaf_count >= 20, "{logs} logs, {leaf_count} leaves");
        // Light was updated: it is dark under the canopy compared with open sky.
        assert_eq!(w.light(1, 61, 1) >> 4, 15);
        // Every leaf of a fresh tree is supported.
        for x in 0..16 {
            for z in 0..16 {
                for y in 61..80 {
                    let p = IVec3::new(x, y, z);
                    if r.has(w.get_at(p), F_LEAVES) {
                        assert!(s.leaf_supported(&w, p), "leaf at {p:?} is not connected to the trunk");
                    }
                }
            }
        }
        // Cut the trunk: the leaves go, a few at a time.
        for y in 61..80 {
            if r.block(w.get(8, y, 8)).name == "oak_log" {
                w.set_block(8, y, 8, AIR);
            }
        }
        for _ in 0..50 {
            for x in 0..16 {
                for z in 0..16 {
                    for y in 61..80 {
                        let p = IVec3::new(x, y, z);
                        let st = w.get_at(p);
                        if r.has(st, F_LEAVES) {
                            s.random_tick(&mut w, p, st);
                        }
                    }
                }
            }
        }
        assert_eq!(count(&w, leaves), 0);
        assert_eq!(s.broken.len(), leaf_count);
        // A sapling under a ceiling stays a sapling.
        w.set_block(3, 60, 3, r.k.dirt);
        w.set_block(3, 61, 3, sapling);
        w.set_block(3, 63, 3, r.k.stone);
        assert!(!s.grow_tree(&mut w, IVec3::new(3, 61, 3), Tree::Oak));
        assert_eq!(w.get(3, 61, 3), sapling);
    }
}
