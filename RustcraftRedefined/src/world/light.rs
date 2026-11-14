//! Sky and block light: flood fill with face-aware occlusion, plus incremental updates.
//!
//! Light moves from cell A to its neighbour B in direction d unless A's face d or B's opposite
//! face is a full opaque square.  That single rule covers cubes, slabs and stairs alike.

use super::block::{reg, Registry, State};
use super::chunk::{Region, STAGE_LIT};
use crate::ffi::{MAX_Y, MIN_Y};
use crate::util::math::{DIRS, DOWN, OPPOSITE, UP};
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Sky,
    Block,
}

type Node = (i32, i32, i32, u8);

#[derive(Default)]
pub struct LightEngine {
    queue: VecDeque<Node>,
    removal: VecDeque<Node>,
}

#[inline]
fn level(v: u8, ch: Channel) -> u8 {
    match ch {
        Channel::Sky => v >> 4,
        Channel::Block => v & 15,
    }
}
#[inline]
fn with_level(v: u8, ch: Channel, l: u8) -> u8 {
    match ch {
        Channel::Sky => (v & 0x0f) | (l << 4),
        Channel::Block => (v & 0xf0) | l,
    }
}
/// Light leaves `from` through face `dir` unless that face is opaque (emitters always shine) and
/// enters `to` unless its facing side is opaque.
#[inline]
fn passable(reg: &Registry, from: State, to: State, dir: usize) -> bool {
    let f = &reg.states[from as usize];
    (f.occlude & (1 << dir) == 0 || f.emit > 0) && reg.states[to as usize].occlude & (1 << OPPOSITE[dir]) == 0
}

impl LightEngine {
    /// Spreads light outwards from the queued cells.
    fn propagate(&mut self, r: &mut Region, ch: Channel) {
        let reg = reg();
        while let Some((x, y, z, _)) = self.queue.pop_front() {
            let here = r.get(x, y, z);
            let l = level(r.light(x, y, z), ch);
            if l == 0 {
                continue;
            }
            for (d, &(dx, dy, dz)) in DIRS.iter().enumerate() {
                let (nx, ny, nz) = (x + dx, y + dy, z + dz);
                if !(MIN_Y..MAX_Y).contains(&ny) || r.stage(nx, nz) < STAGE_LIT {
                    continue;
                }
                let there = r.get(nx, ny, nz);
                if !passable(reg, here, there, d) {
                    continue;
                }
                let filter = reg.states[there as usize].filter;
                let new = if ch == Channel::Sky && d == DOWN && l == 15 && filter == 0 { 15 } else { l.saturating_sub(filter.max(1)) };
                let cur = r.light(nx, ny, nz);
                if new > level(cur, ch) {
                    r.set_light(nx, ny, nz, with_level(cur, ch, new));
                    self.queue.push_back((nx, ny, nz, new));
                }
            }
        }
    }

    /// Computes light for chunk (cx, cz).  The region must contain its 3x3 neighbourhood, fully decorated.
    pub fn light_chunk(&mut self, r: &mut Region, cx: i32, cz: i32) {
        let reg = reg();
        let (bx, bz) = (cx * 16, cz * 16);
        let mut sky_seeds: Vec<Node> = Vec::new();
        let mut block_seeds: Vec<Node> = Vec::new();
        let mut low = [MAX_Y; 256];
        let top_y;
        {
            let c = r.chunk_mut(bx, bz).expect("chunk to light is missing");
            c.allocate_below_top();
            top_y = c.top_section().map_or(MIN_Y, |s| MIN_Y + (s as i32 + 1) * 16);
            for lz in 0..16usize {
                for lx in 0..16usize {
                    let mut sky = 15u8;
                    let mut above: State = 0;
                    for y in (MIN_Y..top_y).rev() {
                        let s = c.get(lx, y, lz);
                        let d = &reg.states[s as usize];
                        if sky > 0 {
                            if !passable(reg, above, s, DOWN) {
                                sky = 0;
                            } else {
                                sky = sky.saturating_sub(d.filter);
                            }
                            if sky == 15 {
                                low[lz * 16 + lx] = y;
                            } else if sky > 0 {
                                sky_seeds.push((bx + lx as i32, y, bz + lz as i32, sky));
                            }
                        }
                        c.set_light(lx, y, lz, (sky << 4) | d.emit);
                        if d.emit > 0 {
                            block_seeds.push((bx + lx as i32, y, bz + lz as i32, d.emit));
                        }
                        above = s;
                    }
                }
            }
            c.stage = STAGE_LIT;
        }
        // Fully lit cells next to columns that may be darker.
        for lz in 0..16i32 {
            for lx in 0..16i32 {
                let from = low[(lz * 16 + lx) as usize];
                let mut to = from;
                for d in 2..6 {
                    to = to.max(r.top(bx + lx + DIRS[d].0, bz + lz + DIRS[d].2) + 1);
                }
                for y in from..to.min(top_y) {
                    sky_seeds.push((bx + lx, y, bz + lz, 15));
                }
            }
        }
        // Pull light in from neighbours that were lit earlier.
        for d in 2..6 {
            let (dx, _, dz) = DIRS[d];
            if r.stage(bx + dx * 16, bz + dz * 16) < STAGE_LIT {
                continue;
            }
            for i in 0..16 {
                let (x, z) = match d {
                    2 => (bx + i, bz - 1),
                    3 => (bx + i, bz + 16),
                    4 => (bx - 1, bz + i),
                    _ => (bx + 16, bz + i),
                };
                let limit = top_y.max(r.top(x, z) + 2).min(MAX_Y);
                for y in MIN_Y..limit {
                    let v = r.light(x, y, z);
                    if v >> 4 > 1 {
                        sky_seeds.push((x, y, z, v >> 4));
                    }
                    if v & 15 > 1 {
                        block_seeds.push((x, y, z, v & 15));
                    }
                }
            }
        }
        self.queue.clear();
        self.queue.extend(sky_seeds);
        self.propagate(r, Channel::Sky);
        self.queue.extend(block_seeds);
        self.propagate(r, Channel::Block);
    }

    /// Restores the light a cell has on its own: block emission, or sky entering the top of the world.
    fn reseed(&mut self, r: &mut Region, x: i32, y: i32, z: i32, ch: Channel) {
        let d = &reg().states[r.get(x, y, z) as usize];
        let own = match ch {
            Channel::Block => d.emit,
            Channel::Sky if y == MAX_Y - 1 && d.occlude & (1 << UP) == 0 => 15u8.saturating_sub(d.filter),
            Channel::Sky => 0,
        };
        if own > 0 {
            let v = r.light(x, y, z);
            r.set_light(x, y, z, with_level(v, ch, own));
            self.queue.push_back((x, y, z, own));
        }
    }

    /// Removes light that depended on (x, y, z) and re-spreads from the remaining sources.
    fn relight(&mut self, r: &mut Region, x: i32, y: i32, z: i32, ch: Channel) {
        let cur = r.light(x, y, z);
        let old = level(cur, ch);
        self.queue.clear();
        self.removal.clear();
        r.set_light(x, y, z, with_level(cur, ch, 0));
        self.removal.push_back((x, y, z, old));
        self.reseed(r, x, y, z, ch);
        while let Some((px, py, pz, pl)) = self.removal.pop_front() {
            for (d, &(dx, dy, dz)) in DIRS.iter().enumerate() {
                let (nx, ny, nz) = (px + dx, py + dy, pz + dz);
                if !(MIN_Y..MAX_Y).contains(&ny) || r.stage(nx, nz) < STAGE_LIT {
                    continue;
                }
                let nv = r.light(nx, ny, nz);
                let nl = level(nv, ch);
                if nl == 0 {
                    continue;
                }
                if nl < pl || (ch == Channel::Sky && d == DOWN && pl == 15 && nl == 15) {
                    r.set_light(nx, ny, nz, with_level(nv, ch, 0));
                    self.removal.push_back((nx, ny, nz, nl));
                    self.reseed(r, nx, ny, nz, ch);
                } else {
                    self.queue.push_back((nx, ny, nz, nl));
                }
            }
        }
        self.propagate(r, ch);
    }

    /// Call after the block at (x, y, z) changed.  The region must cover the 3x3 chunks around it.
    pub fn block_changed(&mut self, r: &mut Region, x: i32, y: i32, z: i32) {
        self.relight(r, x, y, z, Channel::Sky);
        self.relight(r, x, y, z, Channel::Block);
    }
}
