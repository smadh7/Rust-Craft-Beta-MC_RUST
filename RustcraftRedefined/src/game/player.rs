//! The player: movement physics at 20 ticks per second using the vanilla motion constants
//! (gravity 0.08, drag 0.98, jump 0.42, walk speed 0.1, sprint x1.3, ground friction 0.6 * 0.91).

use super::physics::{self, fluid_depth, is_free, sweep};
use crate::util::math::{Aabb, DVec3};
use crate::world::block::{reg, F_CLIMBABLE, F_LAVA, F_WATER};
use crate::world::World;

pub const WIDTH: f64 = 0.6;
pub const HEIGHT: f64 = 1.8;
pub const SNEAK_HEIGHT: f64 = 1.5;
pub const EYE: f64 = 1.62;
pub const SNEAK_EYE: f64 = 1.27;
pub const STEP: f64 = 0.6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameMode {
    Survival,
    Creative,
    Spectator,
}

/// Movement intent for one tick.
#[derive(Clone, Copy, Default, Debug)]
pub struct Controls {
    /// +1 forward, -1 back.
    pub forward: f32,
    /// +1 left, -1 right.
    pub strafe: f32,
    pub jump: bool,
    pub sneak: bool,
    pub sprint: bool,
}

pub struct Player {
    pub pos: DVec3,
    pub prev_pos: DVec3,
    pub vel: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub flying: bool,
    pub sneaking: bool,
    pub sprinting: bool,
    pub in_water: bool,
    pub in_lava: bool,
    pub head_in_water: bool,
    pub on_ladder: bool,
    pub hit_wall: bool,
    pub mode: GameMode,
    pub fall_distance: f32,
    /// Fall distance of the landing that happened this tick (0 otherwise).
    pub landed: f32,
    pub eye_height: f64,
    pub prev_eye_height: f64,
    jump_delay: u8,
    /// Distance walked on the ground, for view bobbing and footsteps.
    pub walk_dist: f32,
    pub prev_walk_dist: f32,
    scratch: Vec<Aabb>,
}

impl Player {
    pub fn new(pos: DVec3) -> Player {
        Player {
            pos,
            prev_pos: pos,
            vel: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            flying: false,
            sneaking: false,
            sprinting: false,
            in_water: false,
            in_lava: false,
            head_in_water: false,
            on_ladder: false,
            hit_wall: false,
            mode: GameMode::Creative,
            fall_distance: 0.0,
            landed: 0.0,
            eye_height: EYE,
            prev_eye_height: EYE,
            jump_delay: 0,
            walk_dist: 0.0,
            prev_walk_dist: 0.0,
            scratch: Vec::new(),
        }
    }

    pub fn height(&self) -> f64 {
        if self.sneaking { SNEAK_HEIGHT } else { HEIGHT }
    }
    pub fn aabb(&self) -> Aabb {
        Aabb::from_feet(self.pos, WIDTH, self.height())
    }
    /// Interpolated eye position for rendering between ticks.
    pub fn eye(&self, alpha: f64) -> DVec3 {
        let p = self.prev_pos.lerp(self.pos, alpha);
        DVec3::new(p.x, p.y + self.prev_eye_height + (self.eye_height - self.prev_eye_height) * alpha, p.z)
    }
    pub fn teleport(&mut self, pos: DVec3) {
        self.pos = pos;
        self.prev_pos = pos;
        self.vel = DVec3::ZERO;
        self.fall_distance = 0.0;
    }

    pub fn turn(&mut self, dx: f32, dy: f32, sensitivity: f32) {
        self.yaw = crate::util::math::wrap_degrees(self.yaw + dx * sensitivity);
        self.pitch = (self.pitch + dy * sensitivity).clamp(-90.0, 90.0);
    }

    fn accelerate(&mut self, accel: f32, strafe: f32, forward: f32) {
        let len2 = strafe * strafe + forward * forward;
        if len2 < 1e-4 {
            return;
        }
        let scale = accel / len2.sqrt().max(1.0);
        let (s, f) = (strafe * scale, forward * scale);
        let (sin, cos) = self.yaw.to_radians().sin_cos();
        self.vel.x += (s * cos - f * sin) as f64;
        self.vel.z += (f * cos + s * sin) as f64;
    }

    /// Shrinks horizontal motion so a sneaking player does not walk off a ledge.
    fn back_off_from_edge(&mut self, world: &World, mut d: DVec3) -> DVec3 {
        let bb = self.aabb();
        let drop = DVec3::new(0.0, -STEP, 0.0);
        let inc = 0.05;
        let free = |dx: f64, dz: f64, scratch: &mut Vec<Aabb>| is_free(world, &bb.offset(DVec3::new(dx, 0.0, dz) + drop), scratch);
        while d.x != 0.0 && free(d.x, 0.0, &mut self.scratch) {
            d.x = if d.x.abs() <= inc { 0.0 } else { d.x - inc * d.x.signum() };
        }
        while d.z != 0.0 && free(0.0, d.z, &mut self.scratch) {
            d.z = if d.z.abs() <= inc { 0.0 } else { d.z - inc * d.z.signum() };
        }
        while d.x != 0.0 && d.z != 0.0 && free(d.x, d.z, &mut self.scratch) {
            d.x = if d.x.abs() <= inc { 0.0 } else { d.x - inc * d.x.signum() };
            d.z = if d.z.abs() <= inc { 0.0 } else { d.z - inc * d.z.signum() };
        }
        d
    }

    fn apply_move(&mut self, world: &World, want: DVec3) {
        if self.mode == GameMode::Spectator {
            self.pos += want;
            self.on_ground = false;
            self.hit_wall = false;
            return;
        }
        let want = if self.sneaking && self.on_ground && !self.flying && want.y <= 0.0 { self.back_off_from_edge(world, want) } else { want };
        let res = sweep(world, &self.aabb(), want, STEP, self.on_ground, &mut self.scratch);
        self.pos += res.delta;
        self.hit_wall = res.hit_x || res.hit_z;
        let was_falling = want.y < 0.0;
        self.on_ground = res.on_ground;
        if res.hit_x {
            self.vel.x = 0.0;
        }
        if res.hit_z {
            self.vel.z = 0.0;
        }
        if res.hit_y {
            self.vel.y = 0.0;
        }
        if self.on_ground {
            if self.fall_distance > 0.0 {
                self.landed = self.fall_distance;
            }
            self.fall_distance = 0.0;
            self.walk_dist += (res.delta.x * res.delta.x + res.delta.z * res.delta.z).sqrt() as f32 * 0.6;
        } else if was_falling && res.delta.y < 0.0 {
            self.fall_distance -= res.delta.y as f32;
        }
    }

    /// Advances the player by one game tick.
    pub fn tick(&mut self, world: &World, c: &Controls) {
        let r = reg();
        self.prev_pos = self.pos;
        self.prev_eye_height = self.eye_height;
        self.prev_walk_dist = self.walk_dist;
        self.landed = 0.0;
        if !world.is_ready(self.pos.x.floor() as i32, self.pos.z.floor() as i32) {
            return; // wait for the terrain under the player
        }
        if self.mode == GameMode::Spectator {
            self.flying = true;
        } else if self.mode == GameMode::Survival {
            self.flying = false;
        }

        // Sneaking: stay crouched while there is no room to stand up.
        let want_sneak = c.sneak && !self.flying;
        if want_sneak {
            self.sneaking = true;
        } else if self.sneaking {
            let standing = Aabb::from_feet(self.pos, WIDTH, HEIGHT);
            if is_free(world, &standing, &mut self.scratch) {
                self.sneaking = false;
            }
        }
        let target_eye = if self.sneaking { SNEAK_EYE } else { EYE };
        self.eye_height += (target_eye - self.eye_height) * 0.5;

        // Fluids and ladders.
        let bb = self.aabb().expand(-0.001, -0.001, -0.001);
        let water = fluid_depth(world, &bb, F_WATER);
        let lava = fluid_depth(world, &bb, F_LAVA);
        self.in_water = water > 0.0 && !self.flying;
        self.in_lava = lava > 0.0 && !self.flying;
        let eye = self.pos + DVec3::new(0.0, self.eye_height, 0.0);
        self.head_in_water = r.has(world.get_at(eye.floor()), F_WATER) && water > self.eye_height - 0.1;
        let feet = world.get_at(self.pos.floor());
        self.on_ladder = r.has(feet, F_CLIMBABLE) && self.mode != GameMode::Spectator;

        let moving_forward = c.forward > 0.0;
        if c.sprint && moving_forward && !self.sneaking && !self.hit_wall {
            self.sprinting = true;
        }
        if !moving_forward || self.sneaking || (self.hit_wall && !self.flying) {
            self.sprinting = false;
        }
        let slow = if self.sneaking { 0.3 } else { 1.0 };
        let (strafe, forward) = (c.strafe * 0.98 * slow, c.forward * 0.98 * slow);
        if self.jump_delay > 0 {
            self.jump_delay -= 1;
        }

        if self.flying {
            let speed = 0.05 * if self.sprinting { 2.0 } else { 1.0 } * if self.mode == GameMode::Spectator { 2.0 } else { 1.0 };
            let lift = (c.jump as i32 - c.sneak as i32) as f64 * speed as f64 * 3.0;
            self.vel.y += lift;
            self.accelerate(speed, strafe, forward);
            let v = self.vel;
            self.apply_move(world, v);
            self.vel.x *= 0.91;
            self.vel.z *= 0.91;
            self.vel.y *= 0.6;
            self.fall_distance = 0.0;
            if self.on_ground && self.mode == GameMode::Creative {
                self.flying = false;
            }
        } else if self.in_water || self.in_lava {
            let y_before = self.pos.y;
            if c.jump {
                self.vel.y += 0.04;
            }
            let drag = if self.in_water { if self.sprinting { 0.9 } else { 0.8 } } else { 0.5 };
            self.accelerate(0.02, strafe, forward);
            let v = self.vel;
            self.apply_move(world, v);
            self.vel.x *= drag;
            self.vel.y *= if self.in_water { 0.8 } else { 0.5 };
            self.vel.z *= drag;
            self.vel.y -= 0.02;
            // Hop out of the fluid when pushing against a bank with free space above it.
            if self.hit_wall {
                let probe = self.aabb().offset(DVec3::new(self.vel.x.signum() * 0.1, 0.6 - self.pos.y + y_before, self.vel.z.signum() * 0.1));
                let (sin, cos) = self.yaw.to_radians().sin_cos();
                let ahead = self.aabb().offset(DVec3::new(-sin as f64 * 0.2, 0.6, cos as f64 * 0.2));
                if is_free(world, &ahead, &mut self.scratch) || is_free(world, &probe, &mut self.scratch) {
                    self.vel.y = 0.3;
                }
            }
            self.fall_distance = 0.0;
        } else {
            let below = world.get((self.pos.x).floor() as i32, (self.pos.y - 0.5).floor() as i32, (self.pos.z).floor() as i32);
            let slip = if self.on_ground { r.block(below).slipperiness } else { 1.0 };
            let friction = if self.on_ground { slip * 0.91 } else { 0.91 };
            let speed = 0.1 * if self.sprinting { 1.3 } else { 1.0 };
            let accel = if self.on_ground { speed * (0.216 / (slip * slip * slip)) } else if self.sprinting { 0.026 } else { 0.02 };
            if c.jump && self.on_ground && self.jump_delay == 0 {
                self.vel.y = 0.42;
                if self.sprinting {
                    let (sin, cos) = self.yaw.to_radians().sin_cos();
                    self.vel.x -= sin as f64 * 0.2;
                    self.vel.z += cos as f64 * 0.2;
                }
                self.jump_delay = 10;
                self.on_ground = false;
            }
            if !c.jump {
                self.jump_delay = 0;
            }
            self.accelerate(accel, strafe, forward);
            if self.on_ladder {
                self.vel.x = self.vel.x.clamp(-0.15, 0.15);
                self.vel.z = self.vel.z.clamp(-0.15, 0.15);
                self.fall_distance = 0.0;
                if self.vel.y < -0.15 {
                    self.vel.y = -0.15;
                }
                if self.sneaking && self.vel.y < 0.0 {
                    self.vel.y = 0.0;
                }
            }
            let v = self.vel;
            self.apply_move(world, v);
            if self.on_ladder && (self.hit_wall || c.jump) {
                self.vel.y = 0.2;
            }
            self.vel.y -= 0.08;
            self.vel.y *= 0.98;
            self.vel.x *= friction as f64;
            self.vel.z *= friction as f64;
        }
        for v in [&mut self.vel.x, &mut self.vel.y, &mut self.vel.z] {
            if v.abs() < 0.003 {
                *v = 0.0;
            }
        }
        // Falling out of the world.
        if self.pos.y < crate::ffi::MIN_Y as f64 - 64.0 && self.mode != GameMode::Survival {
            self.vel.y = self.vel.y.max(0.0);
        }
        let _ = physics::neighbour;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::world::block::AIR;
    use crate::world::chunk::{Chunk, STAGE_LIT};

    /// Flat stone floor with its top surface at y = 61.
    pub fn flat_world() -> World {
        let mut w = World::new(1);
        let stone = reg().k.stone;
        for cz in -2..=2 {
            for cx in -2..=2 {
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
        w
    }

    fn survival(pos: DVec3) -> Player {
        let mut p = Player::new(pos);
        p.mode = GameMode::Survival;
        p
    }

    #[test]
    fn falls_and_lands_on_the_floor() {
        let w = flat_world();
        let mut p = survival(DVec3::new(0.5, 70.0, 0.5));
        let mut landed: f32 = 0.0;
        for _ in 0..60 {
            p.tick(&w, &Controls::default());
            landed = landed.max(p.landed);
        }
        assert!(p.on_ground);
        assert!((p.pos.y - 61.0).abs() < 1e-9, "y = {}", p.pos.y);
        assert!((landed - 9.0).abs() < 0.6, "fall distance {landed}");
    }

    #[test]
    fn walking_and_sprinting_reach_vanilla_speeds() {
        let w = flat_world();
        let run = |sprint: bool| {
            let mut p = survival(DVec3::new(0.5, 61.0, -20.0));
            let c = Controls { forward: 1.0, sprint, ..Default::default() };
            for _ in 0..60 {
                p.tick(&w, &c);
            }
            let z0 = p.pos.z;
            for _ in 0..20 {
                p.tick(&w, &c);
            }
            p.pos.z - z0 // blocks per second
        };
        let walk = run(false);
        let sprint = run(true);
        assert!((walk - 4.317).abs() < 0.05, "walk speed {walk}");
        assert!((sprint - 5.612).abs() < 0.06, "sprint speed {sprint}");
    }

    #[test]
    fn jump_height_is_about_one_and_a_quarter_blocks() {
        let w = flat_world();
        let mut p = survival(DVec3::new(0.5, 61.0, 0.5));
        for _ in 0..3 {
            p.tick(&w, &Controls::default());
        }
        assert!(p.on_ground);
        let mut peak: f64 = 0.0;
        p.tick(&w, &Controls { jump: true, ..Default::default() });
        for _ in 0..20 {
            peak = peak.max(p.pos.y - 61.0);
            p.tick(&w, &Controls::default());
        }
        assert!((peak - 1.2492).abs() < 0.01, "jump peak {peak}");
        assert!(p.on_ground);
    }

    #[test]
    fn steps_up_slabs_but_not_full_blocks_and_sneaking_stops_at_edges() {
        let mut w = flat_world();
        let r = reg();
        let slab = r.parse_state("stone_slab").unwrap();
        for x in -3..=3 {
            w.set_block(x, 61, 5, slab).unwrap();
            w.set_block(x, 61, 9, r.k.stone).unwrap();
        }
        let mut p = survival(DVec3::new(0.5, 61.0, 2.5));
        let c = Controls { forward: 1.0, ..Default::default() };
        for _ in 0..60 {
            p.tick(&w, &c);
        }
        assert!(p.pos.z > 5.0 && p.pos.z < 8.71, "z = {}", p.pos.z);
        assert!((p.pos.z - 8.7).abs() < 0.01, "should be stopped by the full block, z = {}", p.pos.z);
        // Sneak towards a pit: the player must stay on the ledge.
        for z in 12..16 {
            for y in 50..=60 {
                w.set_block(0, y, -z, AIR).unwrap();
            }
        }
        let mut p = survival(DVec3::new(0.5, 61.0, -9.5));
        p.yaw = 180.0;
        let c = Controls { forward: 1.0, sneak: true, ..Default::default() };
        for _ in 0..200 {
            p.tick(&w, &c);
        }
        assert!(p.on_ground && p.pos.y == 61.0, "fell off: {:?}", p.pos);
        assert!(p.pos.z < -11.0 && p.pos.z > -11.4, "z = {}", p.pos.z);
    }

    #[test]
    fn creative_flight_and_water_buoyancy() {
        let mut w = flat_world();
        let r = reg();
        let mut p = Player::new(DVec3::new(0.5, 61.0, 0.5));
        p.flying = true;
        p.on_ground = false;
        for _ in 0..20 {
            p.tick(&w, &Controls { jump: true, ..Default::default() });
        }
        assert!(p.pos.y > 63.0 && p.flying);
        // A pool: the player sinks slowly and can swim up.
        for x in 4..8 {
            for z in 4..8 {
                for y in 56..=60 {
                    w.set_block(x, y, z, r.k.water).unwrap();
                }
            }
        }
        let mut p = survival(DVec3::new(6.0, 60.5, 6.0));
        for _ in 0..10 {
            p.tick(&w, &Controls::default());
        }
        assert!(p.in_water && p.vel.y < 0.0 && p.vel.y > -0.12, "sink speed {}", p.vel.y);
        for _ in 0..40 {
            p.tick(&w, &Controls { jump: true, ..Default::default() });
        }
        assert!(p.pos.y > 59.5, "did not swim up: {}", p.pos.y);
    }
}
