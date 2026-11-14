//! Geysers: Potent Sulfur sitting on magma or lava under one to four blocks of water throws
//! the water (and anything in it) into the air.  Over magma it erupts now and then; over lava
//! it never stops.

use super::entities::Particle;
use super::player::GameMode;
use super::Game;
use crate::util::math::{DVec3, IVec3};
use crate::world::block::*;

pub struct Geyser {
    /// The Potent Sulfur block.
    pub pos: IVec3,
    /// Water source blocks above it (1 to 4).
    pub depth: i32,
    pub continuous: bool,
    /// Ticks the current eruption has left.
    pub erupting: u16,
}

/// How far above the water surface an eruption still lifts things.
const PLUME: f64 = 7.0;

impl Game {
    /// Looks for geysers around the player.  Running eruptions carry over.
    fn scan_geysers(&mut self) {
        let r = reg();
        let p = self.player.pos.floor();
        let mut found = Vec::new();
        for y in p.y - 14..=p.y + 8 {
            for z in p.z - 16..=p.z + 16 {
                for x in p.x - 16..=p.x + 16 {
                    if self.world.get(x, y, z) != r.k.potent_sulfur {
                        continue;
                    }
                    let below = self.world.get(x, y - 1, z);
                    let (magma, lava) = (below == r.k.magma_block, r.has(below, F_LAVA));
                    if !magma && !lava {
                        continue;
                    }
                    let depth = (1..=5).take_while(|i| self.world.get(x, y + i, z) == r.k.water).count() as i32;
                    if !(1..=4).contains(&depth) {
                        continue;
                    }
                    let pos = IVec3::new(x, y, z);
                    let erupting = self.geysers.iter().find(|g| g.pos == pos).map_or(0, |g| g.erupting);
                    found.push(Geyser { pos, depth, continuous: lava, erupting });
                }
            }
        }
        self.geysers = found;
    }

    pub fn tick_geysers(&mut self) {
        if self.tick % 20 == 0 {
            self.scan_geysers();
        }
        for i in 0..self.geysers.len() {
            let g = &mut self.geysers[i];
            if g.continuous {
                g.erupting = 2;
            } else if g.erupting > 0 {
                g.erupting -= 1;
            } else if self.rng.one_in(160) {
                g.erupting = 50 + self.rng.below(50) as u16;
            }
            let (pos, depth, erupting) = (g.pos, g.depth, g.erupting > 0);
            let centre = DVec3::new(pos.x as f64 + 0.5, pos.y as f64 + 1.0, pos.z as f64 + 0.5);
            let surface = centre.y + depth as f64;
            if !erupting {
                // Bubbles drift up through the pool between eruptions.
                if self.rng.one_in(6) && self.particles.len() < 3000 {
                    let at = centre + DVec3::new(self.rng.f64() * 0.6 - 0.3, 0.1, self.rng.f64() * 0.6 - 0.3);
                    self.particles.push(Particle { pos: at, prev: at, vel: DVec3::new(0.0, 0.06, 0.0), life: (depth * 14) as u16, layer: DROP_LAYER, uv: [0.0, 0.0, 1.0], color: [225, 240, 255, 255], size: 0.04, gravity: 0.0, grow: 1.0 });
                }
                continue;
            }
            // The plume: spray thrown up from the surface and steam above it.
            if self.particles.len() < 3000 {
                for _ in 0..3 {
                    let at = DVec3::new(centre.x + self.rng.f64() * 0.5 - 0.25, surface, centre.z + self.rng.f64() * 0.5 - 0.25);
                    let vel = DVec3::new(self.rng.f64() * 0.08 - 0.04, 0.55 + self.rng.f64() * 0.4, self.rng.f64() * 0.08 - 0.04);
                    self.particles.push(Particle { pos: at, prev: at, vel, life: 26 + self.rng.below(14) as u16, layer: DROP_LAYER, uv: [0.0, 0.0, 1.0], color: [190, 220, 255, 255], size: 0.07 + self.rng.f32() * 0.05, gravity: 0.045, grow: 1.0 });
                }
            }
            if self.tick % 6 == 0 {
                crate::audio::play_at(crate::audio::Sfx::Geyser, DVec3::new(centre.x, surface, centre.z), self.player.pos, 0.5, 1.0);
            }
            if self.tick % 3 == 0 {
                self.puffs(DVec3::new(centre.x, surface + 1.0, centre.z), 1, 0.5, 0.05, [235, 240, 245, 255], 0.3);
            }
            // Everything in the column is thrown upwards.
            let inside = |p: DVec3| (p.x - centre.x).abs() < 0.9 && (p.z - centre.z).abs() < 0.9 && p.y >= centre.y - 0.5 && p.y < surface + PLUME;
            if inside(self.player.pos) && self.player.mode != GameMode::Spectator && !self.player.flying {
                self.player.vel.y = (self.player.vel.y + 0.14).min(0.9);
                self.player.fall_distance = 0.0;
                self.player.on_ground = false;
            }
            for m in self.mobs.iter_mut().filter(|m| inside(m.pos)) {
                m.vel.y = (m.vel.y + 0.14).min(0.9);
                m.on_ground = false;
            }
            for e in self.drops.iter_mut().filter(|e| inside(e.pos)) {
                e.vel.y = (e.vel.y + 0.1).min(0.7);
            }
        }
    }
}
