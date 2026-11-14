//! Mobs: farm animals, monsters and the Sulfur Cube.
//!
//! Every mob is a box moved by the same swept collision as the player.  Behaviour is kept
//! simple: animals wander, panic and follow food; monsters walk straight at the player; the
//! Sulfur Cube hops about until it is fed a block, after which it becomes a ball whose physics
//! depend on that block.

use super::entities::ItemEntity;
use super::physics::{fluid_depth, sweep};
use super::player::GameMode;
use super::Game;
use crate::ffi::MIN_Y;
use crate::gfx::batch::{Batch, Rgba};
use crate::item::{ItemDef, ItemStack};
use crate::util::math::{wrap_degrees, Aabb, DVec3, IVec3, Vec3};
use crate::util::rng::Rng;
use crate::world::biome::Biome;
use crate::world::block::*;
use crate::world::model::Shape;
use crate::world::World;
use std::sync::OnceLock;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Pig,
    Cow,
    Sheep,
    Chicken,
    Zombie,
    Creeper,
    Spider,
    SulfurCube,
}

pub const KINDS: [Kind; 8] = [Kind::Pig, Kind::Cow, Kind::Sheep, Kind::Chicken, Kind::Zombie, Kind::Creeper, Kind::Spider, Kind::SulfurCube];

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Pig => "pig",
            Kind::Cow => "cow",
            Kind::Sheep => "sheep",
            Kind::Chicken => "chicken",
            Kind::Zombie => "zombie",
            Kind::Creeper => "creeper",
            Kind::Spider => "spider",
            Kind::SulfurCube => "sulfur_cube",
        }
    }
    pub fn from_name(name: &str) -> Option<Kind> {
        KINDS.iter().copied().find(|k| k.name() == name.trim_start_matches("minecraft:"))
    }
    pub fn hostile(self) -> bool {
        matches!(self, Kind::Zombie | Kind::Creeper | Kind::Spider)
    }
    /// Width, height, health and walking speed (blocks per tick).
    fn stats(self) -> (f64, f64, f32, f64) {
        match self {
            Kind::Pig => (0.9, 0.9, 10.0, 0.135),
            Kind::Cow => (0.9, 1.4, 10.0, 0.086),
            Kind::Sheep => (0.9, 1.3, 8.0, 0.114),
            Kind::Chicken => (0.4, 0.7, 4.0, 0.135),
            Kind::Zombie => (0.6, 1.95, 20.0, 0.114),
            Kind::Creeper => (0.6, 1.7, 20.0, 0.135),
            Kind::Spider => (1.4, 0.9, 16.0, 0.194),
            Kind::SulfurCube => (1.04, 1.04, 8.0, 0.0),
        }
    }
    /// Item that makes the animal follow the player.
    fn lure(self) -> &'static str {
        match self {
            Kind::Pig => "carrot",
            Kind::Cow | Kind::Sheep => "wheat",
            Kind::Chicken => "wheat_seeds",
            _ => "",
        }
    }
}

// ------------------------------------------------------------------ sulfur cube archetypes
/// How a Sulfur Cube behaves as a ball, decided by the block it absorbed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Archetype {
    Regular,
    Bouncy,
    SlowBouncy,
    SlowFlat,
    FastFlat,
    Light,
    FastSliding,
    SlowSliding,
    HighResistance,
    Sticky,
    Explosive,
    Hot,
}

pub struct BallPhysics {
    /// Speed gained from a punch.
    pub speed: f64,
    /// Share of the velocity kept when bouncing off something.
    pub bounce: f64,
    /// Horizontal velocity kept per tick while touching the ground.
    pub friction: f64,
    /// Velocity kept per tick in the air.
    pub drag: f64,
    pub buoyant: bool,
}

impl Archetype {
    pub fn of(block: &BlockDef) -> Archetype {
        let n = block.name;
        let has = |keys: &[&str]| keys.iter().any(|k| n.contains(k));
        if n == "tnt" {
            Archetype::Explosive
        } else if n == "magma_block" {
            Archetype::Hot
        } else if has(&["honeycomb", "honey_block", "slime_block"]) {
            Archetype::Sticky
        } else if has(&["soul_sand", "soul_soil"]) {
            Archetype::HighResistance
        } else if has(&["mushroom", "mycelium"]) {
            Archetype::SlowSliding
        } else if n.ends_with("ice") || n == "snow_block" {
            Archetype::FastSliding
        } else if n.ends_with("_wool") {
            Archetype::Light
        } else if has(&["coral", "sponge", "froglight", "hay_block", "melon", "pumpkin"]) || n.starts_with("moss_") || n.ends_with("_leaves") {
            Archetype::FastFlat
        } else if has(&["iron_block", "gold_block", "copper_block", "netherite_block", "raw_iron", "raw_gold", "raw_copper", "diamond_block", "emerald_block", "lapis_block", "redstone_block"]) {
            Archetype::SlowFlat
        } else if block.sound == Sound::Wood {
            Archetype::Bouncy
        } else if matches!(n, "dirt" | "coarse_dirt" | "rooted_dirt" | "mud" | "packed_mud" | "sand" | "red_sand" | "gravel" | "clay" | "coal_block" | "grass_block" | "podzol") || n.ends_with("concrete_powder") {
            Archetype::Regular
        } else if block.tool == Tool::Pickaxe {
            Archetype::SlowBouncy
        } else {
            Archetype::Regular
        }
    }

    pub fn physics(self) -> BallPhysics {
        let (speed, bounce, friction, drag, buoyant) = match self {
            Archetype::Regular | Archetype::Explosive | Archetype::Hot => (0.6, 0.5, 0.80, 0.99, true),
            Archetype::Bouncy => (0.9, 0.8, 0.80, 0.99, true),
            Archetype::SlowBouncy => (0.4, 0.8, 0.80, 0.97, false),
            Archetype::SlowFlat => (0.4, 0.2, 0.80, 0.97, false),
            Archetype::FastFlat => (0.9, 0.2, 0.80, 0.99, false),
            Archetype::Light => (0.4, 0.8, 0.80, 0.93, true),
            Archetype::FastSliding => (0.9, 0.0, 0.98, 0.99, false),
            Archetype::SlowSliding => (0.4, 0.0, 0.98, 0.99, false),
            Archetype::HighResistance => (0.2, 0.2, 0.50, 0.99, false),
            Archetype::Sticky => (0.6, 0.0, 0.10, 0.99, false),
        };
        BallPhysics { speed, bounce, friction, drag, buoyant }
    }

    pub fn label(self) -> &'static str {
        match self {
            Archetype::Regular => "Regular",
            Archetype::Bouncy => "Bouncy",
            Archetype::SlowBouncy => "Slow Bouncy",
            Archetype::SlowFlat => "Slow Flat",
            Archetype::FastFlat => "Fast Flat",
            Archetype::Light => "Light",
            Archetype::FastSliding => "Fast Sliding",
            Archetype::SlowSliding => "Slow Sliding",
            Archetype::HighResistance => "High Resistance",
            Archetype::Sticky => "Sticky",
            Archetype::Explosive => "Explosive",
            Archetype::Hot => "Hot",
        }
    }
}

/// The block state a Sulfur Cube takes in when given this item: full cubes only.
pub fn absorbable(def: &ItemDef) -> Option<State> {
    let r = reg();
    let b = &r.blocks[def.block? as usize];
    let state = r.default_of(b);
    (def.icon == 0 && matches!(b.shape, Shape::Cube | Shape::Pillar | Shape::Grass) && r.has(state, F_SOLID) && r.collision(state).len() == 1).then_some(state)
}

// ------------------------------------------------------------------ the mob itself
pub struct Mob {
    pub kind: Kind,
    /// Centre of the bottom of the hitbox.
    pub pos: DVec3,
    pub prev: DVec3,
    pub vel: DVec3,
    pub yaw: f32,
    pub prev_yaw: f32,
    pub on_ground: bool,
    pub health: f32,
    /// Ticks of the red flash after being hit; no further damage until it passes.
    pub hurt: u8,
    /// Counts up once health is gone; the mob is removed at 20.
    pub dying: u8,
    pub age: u32,
    pub swing: f32,
    pub prev_swing: f32,
    pub swing_amount: f32,
    goal: Option<(f64, f64)>,
    idle: u16,
    stuck: u16,
    panic: u16,
    cooldown: u8,
    bumped: bool,
    /// Creeper: swelling 0..30.  Sulfur Cube holding TNT: ticks until it explodes (0 = not lit).
    pub fuse: i16,
    /// Small Sulfur Cube (left behind when a large one is killed).
    pub small: bool,
    /// Block a Sulfur Cube has absorbed (AIR = none).
    pub block: State,
    /// Never despawns (cubes from a bucket or carrying a block).
    pub persistent: bool,
    pub angry: bool,
    /// Sheep: ticks until the wool has grown back.
    pub sheared: u16,
    fall: f32,
    pub squish: f32,
    pub prev_squish: f32,
    jump_delay: u16,
    /// Killed by its own explosion: no drops, no splitting.
    pub spent: bool,
}

/// What the mob wants the game to do after its tick.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    None,
    /// Hit the player for this much damage.
    Attack(f32),
    Explode(f32),
    Remove,
}

/// What a mob can perceive of the player this tick.
pub struct Sense {
    pub player: DVec3,
    pub player_bb: Aabb,
    /// The player can be attacked (survival mode, alive, not peaceful).
    pub targetable: bool,
    pub held: ItemStack,
    pub sky_darken: u8,
}

fn turn_towards(yaw: f32, target: f32, max: f32) -> f32 {
    yaw + wrap_degrees(target - yaw).clamp(-max, max)
}

impl Mob {
    pub fn new(kind: Kind, pos: DVec3, rng: &mut Rng) -> Mob {
        let yaw = rng.f32() * 360.0;
        Mob {
            kind,
            pos,
            prev: pos,
            vel: DVec3::ZERO,
            yaw,
            prev_yaw: yaw,
            on_ground: false,
            health: kind.stats().2,
            hurt: 0,
            dying: 0,
            age: 0,
            swing: 0.0,
            prev_swing: 0.0,
            swing_amount: 0.0,
            goal: None,
            idle: rng.below(60) as u16,
            stuck: 0,
            panic: 0,
            cooldown: 0,
            bumped: false,
            fuse: 0,
            small: false,
            block: AIR,
            persistent: false,
            angry: false,
            sheared: 0,
            fall: 0.0,
            squish: 0.0,
            prev_squish: 0.0,
            jump_delay: 10 + rng.below(30) as u16,
            spent: false,
        }
    }

    pub fn small_cube(pos: DVec3, rng: &mut Rng) -> Mob {
        Mob { small: true, health: 4.0, ..Mob::new(Kind::SulfurCube, pos, rng) }
    }

    pub fn size(&self) -> (f64, f64) {
        let (w, h, _, _) = self.kind.stats();
        if self.small { (w * 0.5, h * 0.5) } else { (w, h) }
    }

    pub fn aabb(&self) -> Aabb {
        let (w, h) = self.size();
        Aabb::from_feet(self.pos, w, h)
    }

    pub fn archetype(&self) -> Option<Archetype> {
        (self.block != AIR).then(|| Archetype::of(reg().block(self.block)))
    }

    pub fn alive(&self) -> bool {
        self.dying == 0
    }

    /// Applies damage with knockback away from `from`.  Returns false when nothing happened.
    pub fn damage(&mut self, amount: f32, from: Option<DVec3>) -> bool {
        if self.dying > 0 || self.hurt > 0 || amount <= 0.0 {
            return false;
        }
        self.health -= amount;
        self.hurt = 10;
        if let Some(from) = from {
            let (dx, dz) = (self.pos.x - from.x, self.pos.z - from.z);
            let len = (dx * dx + dz * dz).sqrt().max(1e-3);
            self.vel.x = self.vel.x * 0.5 + dx / len * 0.4;
            self.vel.z = self.vel.z * 0.5 + dz / len * 0.4;
            if self.on_ground {
                self.vel.y = 0.36;
            }
            if !self.kind.hostile() {
                // Run the other way for a few seconds.
                self.panic = 100;
                self.goal = Some((self.pos.x + dx / len * 8.0, self.pos.z + dz / len * 8.0));
                self.idle = 0;
            }
        }
        self.angry = true;
        if self.health <= 0.0 {
            self.health = 0.0;
            self.dying = 1;
        }
        true
    }

    /// One game tick of thinking and moving.
    pub fn tick(&mut self, world: &World, s: &Sense, rng: &mut Rng, scratch: &mut Vec<Aabb>) -> Action {
        self.prev = self.pos;
        self.prev_yaw = self.yaw;
        self.prev_swing = self.swing;
        self.prev_squish = self.squish;
        self.age += 1;
        self.hurt = self.hurt.saturating_sub(1);
        self.cooldown = self.cooldown.saturating_sub(1);
        self.sheared = self.sheared.saturating_sub(1);
        if !world.is_ready(self.pos.x.floor() as i32, self.pos.z.floor() as i32) {
            return Action::None;
        }
        if self.pos.y < MIN_Y as f64 - 32.0 {
            self.spent = true;
            return Action::Remove;
        }
        if self.dying > 0 {
            self.dying += 1;
            self.walk(world, None, 1.0, scratch);
            return if self.dying > 20 { Action::Remove } else { Action::None };
        }
        if self.kind == Kind::SulfurCube {
            return if self.block != AIR { self.tick_ball(world, scratch) } else { self.tick_cube(world, s, rng, scratch) };
        }

        let mut action = Action::None;
        let mut wish: Option<(f64, f64)> = None;
        let mut pace = 1.0;
        let to = s.player - self.pos;
        let dist = to.length();
        let towards = {
            let len = (to.x * to.x + to.z * to.z).sqrt().max(1e-6);
            (to.x / len, to.z / len)
        };
        let light = world.light(self.pos.x.floor() as i32, (self.pos.y + 0.5).floor() as i32, self.pos.z.floor() as i32);
        let bright = (light & 15).max((light >> 4).saturating_sub(s.sky_darken));
        let hunting = self.kind.hostile() && s.targetable && dist < 16.0 && (self.kind != Kind::Spider || self.angry || bright < 10);
        if hunting {
            self.goal = None;
            self.idle = 0;
            match self.kind {
                Kind::Creeper => {
                    if dist < 3.0 {
                        self.fuse += 1;
                        if self.fuse >= 30 {
                            self.spent = true;
                            return Action::Explode(3.0);
                        }
                    } else {
                        self.fuse = (self.fuse - 1).max(0);
                        wish = Some(towards);
                    }
                }
                _ => {
                    wish = Some(towards);
                    if self.cooldown == 0 && self.aabb().expand(0.4, 0.2, 0.4).intersects(&s.player_bb) {
                        self.cooldown = 20;
                        action = Action::Attack(if self.kind == Kind::Zombie { 3.0 } else { 2.0 });
                    }
                    // Spiders pounce from a couple of blocks away.
                    if self.kind == Kind::Spider && self.on_ground && (2.0..4.0).contains(&dist) && rng.one_in(12) {
                        self.vel.x += towards.0 * 0.4;
                        self.vel.z += towards.1 * 0.4;
                        self.vel.y = 0.4;
                        self.on_ground = false;
                    }
                }
            }
        } else {
            if self.kind == Kind::Creeper {
                self.fuse = (self.fuse - 1).max(0);
            }
            let lured = !self.kind.lure().is_empty() && !s.held.is_empty() && s.held.def().name == self.kind.lure() && dist < 8.0;
            if self.panic > 0 {
                self.panic -= 1;
                pace = 1.6;
                if self.goal.is_none() {
                    let a = rng.f64() * std::f64::consts::TAU;
                    self.goal = Some((self.pos.x + a.cos() * 6.0, self.pos.z + a.sin() * 6.0));
                }
            } else if lured {
                self.goal = None;
                if dist > 2.5 {
                    wish = Some(towards);
                    pace = 1.2;
                } else {
                    self.yaw = turn_towards(self.yaw, (-towards.0).atan2(towards.1).to_degrees() as f32, 20.0);
                }
            }
            if wish.is_none() && !lured {
                if self.idle > 0 {
                    self.idle -= 1;
                } else if let Some((gx, gz)) = self.goal {
                    let (dx, dz) = (gx - self.pos.x, gz - self.pos.z);
                    let len = (dx * dx + dz * dz).sqrt();
                    self.stuck += 1;
                    if len < 0.7 || self.stuck > 120 {
                        self.goal = None;
                        self.stuck = 0;
                        self.idle = if self.panic > 0 { 0 } else { 40 + rng.below(140) as u16 };
                    } else {
                        wish = Some((dx / len, dz / len));
                    }
                } else {
                    let a = rng.f64() * std::f64::consts::TAU;
                    let d = 3.0 + rng.f64() * 6.0;
                    self.goal = Some((self.pos.x + a.cos() * d, self.pos.z + a.sin() * d));
                }
            }
            // Animals do not walk off cliffs or into water and lava.
            if let Some((wx, wz)) = wish {
                if self.on_ground && !self.safe_ahead(world, wx, wz) {
                    wish = None;
                    self.goal = None;
                    self.idle = 10;
                }
            }
        }
        // Zombies burn in direct sunlight.
        if self.kind == Kind::Zombie && s.sky_darken < 4 && light >> 4 == 15 && self.age % 20 == 0 && fluid_depth(world, &self.aabb(), F_WATER) < 0.1 {
            self.damage(1.0, None);
            self.hurt = 0;
        }
        if self.kind == Kind::Spider && wish.is_some() && self.bumped {
            // Spiders climb what they run into.
            self.vel.y = 0.2;
        }
        self.walk(world, wish, pace, scratch);
        if self.health <= 0.0 && self.dying == 0 {
            self.dying = 1;
        }
        action
    }

    /// Is the block one step ahead somewhere an animal would walk?
    fn safe_ahead(&self, world: &World, wx: f64, wz: f64) -> bool {
        let r = reg();
        let (w, _) = self.size();
        let (x, z) = ((self.pos.x + wx * (w * 0.5 + 0.6)).floor() as i32, (self.pos.z + wz * (w * 0.5 + 0.6)).floor() as i32);
        let y = (self.pos.y + 0.5).floor() as i32;
        for dy in 0..=4 {
            let s = world.get(x, y - dy, z);
            if r.has(s, F_FLUID) {
                return false;
            }
            if r.has(s, F_SOLID) {
                return !matches!(r.block(s).name, "cactus" | "magma_block");
            }
        }
        false
    }

    /// Walking physics shared by everything with legs: accelerate, collide, fall, swim.
    fn walk(&mut self, world: &World, wish: Option<(f64, f64)>, pace: f64, scratch: &mut Vec<Aabb>) {
        let r = reg();
        let (w, h) = self.size();
        let speed = self.kind.stats().3 * pace;
        let bb = Aabb::from_feet(self.pos, w, h);
        let swimming = fluid_depth(world, &bb, F_WATER) > h * 0.5;
        if let Some((wx, wz)) = wish {
            let accel = if self.on_ground || swimming { speed * 0.454 } else { 0.02 };
            self.vel.x += wx * accel;
            self.vel.z += wz * accel;
            self.yaw = turn_towards(self.yaw, (-wx).atan2(wz).to_degrees() as f32, 20.0);
            if self.bumped && self.on_ground {
                self.vel.y = 0.42;
            }
        }
        let res = sweep(world, &bb, self.vel, 0.6, self.on_ground, scratch);
        self.pos += res.delta;
        self.bumped = (res.hit_x || res.hit_z) && wish.is_some();
        if res.hit_x {
            self.vel.x = 0.0;
        }
        if res.hit_z {
            self.vel.z = 0.0;
        }
        if res.hit_y {
            self.vel.y = 0.0;
        }
        self.on_ground = res.on_ground;
        if self.on_ground {
            if self.fall > 3.5 && self.kind != Kind::Chicken && self.dying == 0 {
                self.health -= (self.fall - 3.0).floor();
                self.hurt = 10;
            }
            self.fall = 0.0;
        } else if res.delta.y < 0.0 {
            self.fall -= res.delta.y as f32;
        }
        if swimming {
            self.vel = self.vel * 0.8;
            self.vel.y += 0.03;
            self.fall = 0.0;
        } else {
            self.vel.y = (self.vel.y - 0.08) * 0.98;
            // Chickens flap and fall slowly.
            if self.kind == Kind::Chicken && self.vel.y < -0.1 {
                self.vel.y = -0.1;
            }
            let below = world.get(self.pos.x.floor() as i32, (self.pos.y - 0.2).floor() as i32, self.pos.z.floor() as i32);
            let f = if self.on_ground { r.block(below).slipperiness as f64 * 0.91 } else { 0.91 };
            self.vel.x *= f;
            self.vel.z *= f;
        }
        if fluid_depth(world, &bb, F_LAVA) > 0.0 && self.age % 10 == 0 && self.dying == 0 {
            self.health -= 4.0;
            self.hurt = 10;
        }
        let moved = (res.delta.x * res.delta.x + res.delta.z * res.delta.z).sqrt() as f32;
        self.swing_amount += ((moved * 4.0).min(1.0) - self.swing_amount) * 0.4;
        self.swing += self.swing_amount;
    }

    /// A Sulfur Cube without a block: hops around, towards food and friends with blocks.
    fn tick_cube(&mut self, world: &World, s: &Sense, rng: &mut Rng, scratch: &mut Vec<Aabb>) -> Action {
        let was_on_ground = self.on_ground;
        let to = s.player - self.pos;
        let wants_player = !self.small && to.length() < 10.0 && !s.held.is_empty() && absorbable(s.held.def()).is_some();
        if self.on_ground {
            if self.jump_delay > 0 {
                self.jump_delay -= 1;
            } else {
                let target = if wants_player { Some((s.player.x, s.player.z)) } else { self.goal };
                self.jump_delay = if target.is_some() { 6 + rng.below(8) as u16 } else { 15 + rng.below(40) as u16 };
                match target {
                    Some((gx, gz)) => self.yaw = (-(gx - self.pos.x)).atan2(gz - self.pos.z).to_degrees() as f32,
                    None if rng.one_in(2) => self.yaw += rng.f32() * 180.0 - 90.0,
                    None => {}
                }
                let (sin, cos) = (self.yaw as f64).to_radians().sin_cos();
                let hop = if self.small { 0.18 } else { 0.26 };
                self.vel = DVec3::new(-sin * hop, if self.small { 0.42 } else { 0.5 }, cos * hop);
                self.on_ground = false;
                self.squish = 0.35;
            }
        }
        self.walk(world, None, 1.0, scratch);
        if self.on_ground && !was_on_ground {
            self.squish = -0.4;
        }
        self.squish *= 0.7;
        if self.health <= 0.0 && self.dying == 0 {
            self.dying = 1;
        }
        Action::None
    }

    /// A Sulfur Cube with a block inside: a ball that bounces, slides and floats by archetype.
    fn tick_ball(&mut self, world: &World, scratch: &mut Vec<Aabb>) -> Action {
        let Some(kind) = self.archetype() else { return Action::None };
        let ph = kind.physics();
        let (w, h) = self.size();
        let bb = Aabb::from_feet(self.pos, w, h);
        let depth = fluid_depth(world, &bb, F_WATER);
        if depth > h * 0.5 {
            self.vel = self.vel * 0.85;
            self.vel.y += if ph.buoyant { 0.05 } else { -0.02 };
        } else {
            self.vel.y -= 0.08;
        }
        let before = self.vel;
        let res = sweep(world, &bb, self.vel, 0.0, false, scratch);
        self.pos += res.delta;
        if res.hit_x {
            self.vel.x = -before.x * ph.bounce;
        }
        if res.hit_z {
            self.vel.z = -before.z * ph.bounce;
        }
        self.on_ground = false;
        if res.hit_y {
            let rebound = -before.y * ph.bounce;
            if before.y < 0.0 {
                // Small rebounds die out so the ball comes to rest.
                self.vel.y = if rebound > 0.16 { rebound } else { 0.0 };
                self.on_ground = self.vel.y == 0.0;
                if rebound > 0.16 {
                    self.squish = -0.3;
                }
                self.vel.x *= ph.friction;
                self.vel.z *= ph.friction;
            } else {
                self.vel.y = rebound;
            }
        }
        self.vel = self.vel * ph.drag;
        if self.vel.x.abs() < 0.003 {
            self.vel.x = 0.0;
        }
        if self.vel.z.abs() < 0.003 {
            self.vel.z = 0.0;
        }
        self.squish *= 0.7;
        let moved = (res.delta.x * res.delta.x + res.delta.z * res.delta.z).sqrt() as f32;
        // A rolling ball turns to face the way it travels.
        if moved > 0.02 {
            self.yaw = turn_towards(self.yaw, (-(res.delta.x)).atan2(res.delta.z).to_degrees() as f32, 12.0);
        }
        if self.fuse > 0 {
            self.fuse -= 1;
            if self.fuse == 0 {
                self.spent = true;
                return Action::Explode(4.0);
            }
        }
        if fluid_depth(world, &bb, F_LAVA) > 0.0 && self.age % 10 == 0 {
            self.health -= 4.0;
            if self.health <= 0.0 {
                self.dying = 1;
            }
        }
        Action::None
    }

    /// Items left behind on death.
    pub fn loot(&self, rng: &mut Rng) -> Vec<ItemStack> {
        let mut out = Vec::new();
        let mut give = |name: &str, lo: i32, hi: i32| {
            let n = rng.range(lo, hi);
            if n > 0 {
                out.push(ItemStack::named(name, n as u8));
            }
        };
        match self.kind {
            Kind::Pig => give("porkchop", 1, 3),
            Kind::Cow => {
                give("beef", 1, 3);
                give("leather", 0, 2);
            }
            Kind::Sheep => {
                give("mutton", 1, 2);
                if self.sheared == 0 {
                    give("white_wool", 1, 1);
                }
            }
            Kind::Chicken => {
                give("chicken", 1, 1);
                give("feather", 0, 2);
            }
            Kind::Zombie => give("rotten_flesh", 0, 2),
            Kind::Creeper => give("gunpowder", 0, 2),
            Kind::Spider => give("string", 0, 2),
            Kind::SulfurCube => {}
        }
        if self.block != AIR {
            let r = reg();
            if let Some(id) = r.items.for_block(r.block(self.block)) {
                out.push(ItemStack::new(id, 1));
            }
        }
        out
    }
}

// ------------------------------------------------------------------ models
/// Texture layers of the mob skins.
pub struct Skins {
    layers: Vec<u16>,
}

impl Skins {
    fn get(&self, name: &str) -> u16 {
        crate::art::mobtex::NAMES.iter().position(|n| *n == name).map_or(0, |i| self.layers[i])
    }
}

pub fn skins() -> &'static Skins {
    static SKINS: OnceLock<Skins> = OnceLock::new();
    SKINS.get_or_init(|| Skins { layers: crate::art::mobtex::NAMES.iter().map(|n| reg().texture_index(&format!("mob/{n}")).unwrap_or(0)).collect() })
}

/// Emits the boxes of one mob.  Model space is in pixels (1/16 block): x to the mob's left,
/// y up from the feet, z forwards.
struct Model<'a> {
    b: &'a mut Batch,
    origin: Vec3,
    sin: f32,
    cos: f32,
    light: [f32; 3],
    tint: [f32; 3],
    alpha: u8,
    scale: f32,
    /// Stretch applied to the height (and inversely to the width) for squishy things.
    squish: f32,
    /// Sideways roll in radians as the mob falls over dead.
    roll: f32,
}

impl Model<'_> {
    /// A box from `min` with `size`, turned by `rx` around the X axis through `pivot`.
    /// `tex` gives the layers for top and bottom, the sides, and the front.
    fn cube(&mut self, min: [f32; 3], size: [f32; 3], pivot: [f32; 3], rx: f32, tex: [u16; 3]) {
        self.cube_rz(min, size, pivot, rx, 0.0, tex);
    }

    fn cube_rz(&mut self, min: [f32; 3], size: [f32; 3], pivot: [f32; 3], rx: f32, rz: f32, tex: [u16; 3]) {
        let max = [min[0] + size[0], min[1] + size[1], min[2] + size[2]];
        let (sx, cx) = rx.sin_cos();
        let (sz, cz) = rz.sin_cos();
        let (sr, cr) = self.roll.sin_cos();
        let (scale, squish, origin, sin, cos) = (self.scale, self.squish, self.origin, self.sin, self.cos);
        let place = |p: [f32; 3]| -> [f32; 3] {
            // Limb rotation about the pivot.
            let (x, mut y, mut z) = (p[0] - pivot[0], p[1] - pivot[1], p[2] - pivot[2]);
            (y, z) = (y * cx - z * sx, y * sx + z * cx);
            let (x, y) = (x * cz - y * sz, x * sz + y * cz);
            let (mut x, mut y, z) = (x + pivot[0], y + pivot[1], z + pivot[2]);
            // Death roll about the feet, squish, then into the world.
            (x, y) = (x * cr - y * sr, x * sr + y * cr);
            let k = scale / 16.0;
            let (x, y, z) = (x * k * (1.0 - squish * 0.5), y * k * (1.0 + squish), z * k * (1.0 - squish * 0.5));
            [origin.x + x * cos - z * sin, origin.y + y, origin.z + x * sin + z * cos]
        };
        let c = |ix: usize, iy: usize, iz: usize| [if ix == 0 { min[0] } else { max[0] }, if iy == 0 { min[1] } else { max[1] }, if iz == 0 { min[2] } else { max[2] }];
        // Faces: corners counter-clockwise seen from outside, starting bottom-left.
        let faces: [([[f32; 3]; 4], u16, f32); 6] = [
            ([c(0, 0, 0), c(1, 0, 0), c(1, 0, 1), c(0, 0, 1)], tex[0], 0.5),
            ([c(0, 1, 1), c(1, 1, 1), c(1, 1, 0), c(0, 1, 0)], tex[0], 1.0),
            ([c(0, 0, 0), c(0, 1, 0), c(1, 1, 0), c(1, 0, 0)], tex[1], 0.7),
            ([c(1, 0, 1), c(0, 0, 1), c(0, 1, 1), c(1, 1, 1)], tex[2], 0.85),
            ([c(0, 0, 1), c(0, 0, 0), c(0, 1, 0), c(0, 1, 1)], tex[1], 0.65),
            ([c(1, 0, 0), c(1, 0, 1), c(1, 1, 1), c(1, 1, 0)], tex[1], 0.65),
        ];
        for (f, (corners, layer, shade)) in faces.into_iter().enumerate() {
            let color: Rgba = [
                (255.0 * self.light[0] * self.tint[0] * shade).min(255.0) as u8,
                (255.0 * self.light[1] * self.tint[1] * shade).min(255.0) as u8,
                (255.0 * self.light[2] * self.tint[2] * shade).min(255.0) as u8,
                self.alpha,
            ];
            // The back face (f == 2) lists its corners in a different order; match the UVs.
            let uv = if f == 2 { [[1.0, 1.0], [1.0, 0.0], [0.0, 0.0], [0.0, 1.0]] } else { [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]] };
            self.b.quad(corners.map(place), uv, layer as f32, color);
        }
    }

    /// Four legs under a body, swinging in diagonal pairs.
    fn legs(&mut self, x: f32, z_front: f32, z_back: f32, width: f32, length: f32, swing: f32, tex: u16) {
        for (i, (lx, lz)) in [(-x, z_front), (x - width, z_front), (-x, z_back), (x - width, z_back)].into_iter().enumerate() {
            let phase = if i == 0 || i == 3 { swing } else { -swing };
            self.cube([lx, 0.0, lz], [width, length, width], [lx + width * 0.5, length, lz + width * 0.5], phase, [tex; 3]);
        }
    }
}

impl Mob {
    /// Draws the mob.  `rel` is the interpolated position relative to the camera.  The Sulfur
    /// Cube's jelly is see-through and is drawn in the `translucent` pass; everything else
    /// in the opaque one.
    pub fn draw(&self, b: &mut Batch, rel: Vec3, alpha: f32, light: [f32; 3], translucent: bool) {
        let sk = skins();
        let yaw = (self.prev_yaw + wrap_degrees(self.yaw - self.prev_yaw) * alpha).to_radians();
        let hurt = self.hurt > 0 || self.dying > 0;
        let flash = self.kind == Kind::Creeper && self.fuse > 0 && (self.fuse / 3) % 2 == 0 || self.kind == Kind::SulfurCube && self.fuse > 0 && (self.fuse / 4) % 2 == 0;
        let swell = if self.kind == Kind::Creeper { 1.0 + self.fuse as f32 / 30.0 * 0.25 } else { 1.0 };
        let tint = if flash { [1.8, 1.8, 1.8] } else if hurt { [1.0, 0.45, 0.45] } else { [1.0, 1.0, 1.0] };
        let mut m = Model {
            b,
            origin: rel,
            sin: yaw.sin(),
            cos: yaw.cos(),
            light: if flash { [1.0, 1.0, 1.0] } else { light },
            tint,
            alpha: 255,
            scale: swell,
            squish: 0.0,
            roll: if self.dying > 0 { ((self.dying as f32 + alpha) / 20.0).min(1.0) * std::f32::consts::FRAC_PI_2 } else { 0.0 },
        };
        let swing_pos = self.prev_swing + (self.swing - self.prev_swing) * alpha;
        let swing = (swing_pos * 0.6662).cos() * 1.2 * self.swing_amount;
        // Only the cube has a translucent part.
        if translucent && self.kind != Kind::SulfurCube {
            return;
        }
        match self.kind {
            Kind::Pig => {
                let (skin, face) = (sk.get("pig"), sk.get("pig_face"));
                m.cube([-5.0, 6.0, -8.0], [10.0, 8.0, 16.0], [0.0; 3], 0.0, [skin; 3]);
                m.cube([-4.0, 8.0, 6.0], [8.0, 8.0, 8.0], [0.0; 3], 0.0, [skin, skin, face]);
                m.legs(5.0, 3.0, -7.0, 4.0, 6.0, swing, skin);
            }
            Kind::Cow => {
                let (skin, face) = (sk.get("cow"), sk.get("cow_face"));
                m.cube([-6.0, 12.0, -9.0], [12.0, 10.0, 18.0], [0.0; 3], 0.0, [skin; 3]);
                m.cube([-4.0, 16.0, 8.0], [8.0, 8.0, 6.0], [0.0; 3], 0.0, [skin, skin, face]);
                m.legs(6.0, 4.0, -8.0, 4.0, 12.0, swing, skin);
            }
            Kind::Sheep => {
                let (wool, skin, face) = (if self.sheared > 0 { sk.get("sheep_skin") } else { sk.get("sheep_wool") }, sk.get("sheep_skin"), sk.get("sheep_face"));
                let fat = if self.sheared > 0 { 0.0 } else { 1.5 };
                m.cube([-4.0 - fat, 12.0, -8.0], [8.0 + fat * 2.0, 8.0 + fat, 16.0], [0.0; 3], 0.0, [wool; 3]);
                m.cube([-3.0, 16.0, 7.0], [6.0, 6.0, 7.0], [0.0; 3], 0.0, [skin, skin, face]);
                m.legs(4.0, 3.0, -7.0, 4.0, 12.0, swing, skin);
            }
            Kind::Chicken => {
                let (body, face, beak) = (sk.get("chicken"), sk.get("chicken_face"), sk.get("chicken_beak"));
                m.cube([-3.0, 5.0, -4.0], [6.0, 6.0, 8.0], [0.0; 3], 0.0, [body; 3]);
                m.cube([-2.0, 9.0, 3.0], [4.0, 6.0, 3.0], [0.0; 3], 0.0, [body, body, face]);
                m.cube([-2.0, 11.0, 6.0], [4.0, 2.0, 2.0], [0.0; 3], 0.0, [beak; 3]);
                // Wings flap while airborne.
                let flap = if self.on_ground { 0.0 } else { ((self.age as f32 + alpha) * 1.2).sin() * 0.8 + 0.8 };
                m.cube_rz([-4.0, 7.0, -3.0], [1.0, 4.0, 6.0], [-3.0, 11.0, 0.0], 0.0, flap, [body; 3]);
                m.cube_rz([3.0, 7.0, -3.0], [1.0, 4.0, 6.0], [3.0, 11.0, 0.0], 0.0, -flap, [body; 3]);
                for (i, x) in [-2.0f32, 1.0].into_iter().enumerate() {
                    m.cube([x, 0.0, 0.0], [1.0, 5.0, 1.0], [x + 0.5, 5.0, 0.5], if i == 0 { swing } else { -swing }, [beak; 3]);
                }
            }
            Kind::Zombie => {
                let (skin, face, shirt, pants) = (sk.get("zombie_skin"), sk.get("zombie_face"), sk.get("zombie_shirt"), sk.get("zombie_pants"));
                m.cube([-4.0, 24.0, -4.0], [8.0, 8.0, 8.0], [0.0; 3], 0.0, [skin, skin, face]);
                m.cube([-4.0, 12.0, -2.0], [8.0, 12.0, 4.0], [0.0; 3], 0.0, [shirt; 3]);
                // Arms held out in front.
                for x in [-8.0f32, 4.0] {
                    m.cube([x, 12.0, -2.0], [4.0, 12.0, 4.0], [x + 2.0, 22.0, 0.0], -1.45 + swing * 0.15, [skin, shirt, skin]);
                }
                for (i, x) in [-4.0f32, 0.0].into_iter().enumerate() {
                    m.cube([x, 0.0, -2.0], [4.0, 12.0, 4.0], [x + 2.0, 12.0, 0.0], if i == 0 { swing } else { -swing }, [pants; 3]);
                }
            }
            Kind::Creeper => {
                let (skin, face) = (sk.get("creeper"), sk.get("creeper_face"));
                m.cube([-4.0, 18.0, -4.0], [8.0, 8.0, 8.0], [0.0; 3], 0.0, [skin, skin, face]);
                m.cube([-4.0, 6.0, -2.0], [8.0, 12.0, 4.0], [0.0; 3], 0.0, [skin; 3]);
                m.legs(4.0, 2.0, -6.0, 4.0, 6.0, swing, skin);
            }
            Kind::Spider => {
                let (skin, face) = (sk.get("spider"), sk.get("spider_face"));
                m.cube([-4.0, 5.0, 3.0], [8.0, 8.0, 8.0], [0.0; 3], 0.0, [skin, skin, face]);
                m.cube([-3.0, 6.0, -3.0], [6.0, 6.0, 6.0], [0.0; 3], 0.0, [skin; 3]);
                m.cube([-5.0, 5.0, -15.0], [10.0, 8.0, 12.0], [0.0; 3], 0.0, [skin; 3]);
                // Eight legs angled down from the body, scuttling in alternation.
                for i in 0..4 {
                    let z = 3.0 - i as f32 * 2.0;
                    let wave = (swing_pos * 1.3 + i as f32 * 1.6).sin() * 0.25 * self.swing_amount;
                    m.cube_rz([3.0, 8.0, z - 1.0], [15.0, 2.0, 2.0], [3.0, 9.0, z], 0.0, -0.55 + wave, [skin; 3]);
                    m.cube_rz([-18.0, 8.0, z - 1.0], [15.0, 2.0, 2.0], [-3.0, 9.0, z], 0.0, 0.55 - wave, [skin; 3]);
                }
            }
            Kind::SulfurCube => {
                let size = if self.small { 8.0 } else { 16.6 };
                m.squish = self.prev_squish + (self.squish - self.prev_squish) * alpha;
                let half = size * 0.5;
                if translucent {
                    m.alpha = 255;
                    let (jelly, face) = (sk.get("sulfur_cube"), sk.get("sulfur_cube_face"));
                    m.cube([-half, 0.0, -half], [size, size, size], [0.0; 3], 0.0, [jelly, jelly, face]);
                } else if self.block != AIR {
                    // The absorbed block sits inside the jelly.
                    let r = reg();
                    let layers = r.texsets[r.mesh_states[self.block as usize].texset as usize].layer;
                    let blk = r.block(self.block);
                    if blk.tint != TINT_NONE && blk.shape != Shape::Grass {
                        let t = super::hud::icon_tint(blk);
                        m.tint = [m.tint[0] * t[0] as f32 / 255.0, m.tint[1] * t[1] as f32 / 255.0, m.tint[2] * t[2] as f32 / 255.0];
                    }
                    let inner = size * 0.62;
                    let lo = (size - inner) * 0.5;
                    m.cube([-inner * 0.5, lo, -inner * 0.5], [inner, inner, inner], [0.0; 3], 0.0, [layers[1], layers[2], layers[2]]);
                } else {
                    let core = sk.get("sulfur_cube_core");
                    let inner = size * 0.4;
                    let lo = (size - inner) * 0.5;
                    m.cube([-inner * 0.5, lo, -inner * 0.5], [inner, inner, inner], [0.0; 3], 0.0, [core; 3]);
                }
            }
        }
    }
}

// ------------------------------------------------------------------ the game's side
const HOSTILE_CAP: usize = 14;
const PASSIVE_CAP: usize = 10;
const CUBE_CAP: usize = 5;
const MAX_MOBS: usize = 120;

impl Game {
    /// Mob nearest along the player's line of sight within `reach`: index and distance.
    pub fn mob_in_sight(&self, eye: DVec3, dir: DVec3, reach: f64) -> Option<(usize, f64)> {
        let mut best: Option<(usize, f64)> = None;
        for (i, m) in self.mobs.iter().enumerate() {
            if !m.alive() {
                continue;
            }
            if let Some((t, _)) = m.aabb().raycast(eye, dir, reach) {
                if best.map_or(true, |(_, bt)| t < bt) {
                    best = Some((i, t));
                }
            }
        }
        best
    }

    /// Left click on a mob.
    pub fn attack_mob(&mut self, index: usize) {
        let held = self.inv.held();
        let def = held.def();
        let damage = if held.is_empty() { 1.0 } else { def.attack };
        let eye = self.player.eye(1.0);
        let look = crate::util::math::look_dir(self.player.yaw, self.player.pitch).to_f64();
        self.swing = 6;
        let m = &mut self.mobs[index];
        if let Some(kind) = m.archetype() {
            // A cube holding a block is a ball: it takes no damage and flies off instead.
            let ph = kind.physics();
            let (_, h) = m.size();
            let away = (m.pos + DVec3::new(0.0, h * 0.5, 0.0) - eye).normalized();
            let dir = (look * 0.7 + away * 0.3 + DVec3::new(0.0, 0.25, 0.0)).normalized();
            let power = ph.speed * (0.7 + damage as f64 * 0.12).min(2.0) * if self.player.sprinting { 1.3 } else { 1.0 };
            m.vel += dir * power;
            m.on_ground = false;
            m.squish = 0.3;
            m.hurt = 3;
            crate::audio::play(crate::audio::Sfx::Boing, 0.6, 1.0);
            return;
        }
        if m.damage(damage, Some(self.player.pos)) {
            let pos = m.pos;
            crate::audio::play_at(crate::audio::Sfx::Thud, pos, self.player.pos, 0.6, 1.2);
            self.puffs(pos + DVec3::new(0.0, 0.8, 0.0), 4, 0.5, 0.06, [200, 40, 40, 255], 0.08);
            if self.player.mode == GameMode::Survival && def.durability > 0 {
                self.inv.held_mut().wear(if def.tool == Tool::Sword { 1 } else { 2 });
            }
            self.vitals.exhaustion += 0.1;
        }
    }

    /// Right click on a mob.  Returns true when the click did something.
    pub fn interact_mob(&mut self, index: usize) -> bool {
        let held = self.inv.held();
        let def = held.def();
        let survival = self.player.mode == GameMode::Survival;
        let kind = self.mobs[index].kind;
        match kind {
            Kind::SulfurCube => {
                let m = &mut self.mobs[index];
                if m.block == AIR {
                    if m.small {
                        // Slime balls grow a small cube back to full size.
                        if def.name == "slime_ball" {
                            m.small = false;
                            m.health = 8.0;
                            m.squish = 0.5;
                            m.pos.y += 0.1;
                            if survival {
                                self.inv.consume_held();
                            }
                            return true;
                        }
                        return false;
                    }
                    if def.name == "bucket" {
                        self.mobs.swap_remove(index);
                        if survival {
                            self.replace_held_one(ItemStack::named("sulfur_cube_bucket", 1));
                        } else {
                            let rest = self.inv.add(ItemStack::named("sulfur_cube_bucket", 1));
                            self.throw_item(rest);
                        }
                        return true;
                    }
                    if let Some(state) = (!held.is_empty()).then(|| absorbable(def)).flatten() {
                        m.block = state;
                        m.persistent = true;
                        m.squish = 0.4;
                        m.vel = DVec3::ZERO;
                        m.fuse = 0;
                        if survival {
                            self.inv.consume_held();
                        }
                        let label = Archetype::of(reg().block(state)).label();
                        self.say(format!("The Sulfur Cube absorbed the block ({label})"));
                        return true;
                    }
                    return false;
                }
                // Already holding a block.
                if def.tool == Tool::Shears && m.fuse == 0 {
                    let state = std::mem::replace(&mut m.block, AIR);
                    let pos = m.pos;
                    m.squish = -0.3;
                    let r = reg();
                    if let Some(id) = r.items.for_block(r.block(state)) {
                        let mut drop = ItemEntity::from_block(pos.floor(), ItemStack::new(id, 1), &mut self.rng);
                        drop.pos = pos + DVec3::new(0.0, 1.1, 0.0);
                        drop.prev = drop.pos;
                        drop.delay = 30;
                        self.drops.push(drop);
                    }
                    if survival {
                        self.inv.held_mut().wear(1);
                    }
                    return true;
                }
                if def.name == "flint_and_steel" && m.archetype() == Some(Archetype::Explosive) && m.fuse == 0 {
                    // Six seconds, as when lit by fire.
                    m.fuse = 120;
                    if survival {
                        self.inv.held_mut().wear(1);
                    }
                    return true;
                }
                false
            }
            Kind::Sheep if def.tool == Tool::Shears && self.mobs[index].sheared == 0 => {
                self.mobs[index].sheared = 2400;
                let pos = self.mobs[index].pos;
                let n = self.rng.range(1, 3) as u8;
                let mut drop = ItemEntity::from_block(pos.floor(), ItemStack::named("white_wool", n), &mut self.rng);
                drop.pos = pos + DVec3::new(0.0, 1.0, 0.0);
                drop.prev = drop.pos;
                self.drops.push(drop);
                if survival {
                    self.inv.held_mut().wear(1);
                }
                true
            }
            Kind::Cow if def.name == "bucket" => {
                if survival {
                    self.replace_held_one(ItemStack::named("milk_bucket", 1));
                } else {
                    let rest = self.inv.add(ItemStack::named("milk_bucket", 1));
                    self.throw_item(rest);
                }
                true
            }
            _ => false,
        }
    }

    pub fn spawn_mob(&mut self, kind: Kind, pos: DVec3) -> Option<&mut Mob> {
        if self.mobs.len() >= MAX_MOBS {
            return None;
        }
        let m = Mob::new(kind, pos, &mut self.rng);
        self.mobs.push(m);
        self.mobs.last_mut()
    }

    /// A place to stand at (x, z) at or below `y`: solid floor, two blocks of room.
    fn standing_spot(&self, x: i32, mut y: i32, z: i32, max_drop: i32) -> Option<(i32, State)> {
        let r = reg();
        for _ in 0..=max_drop {
            let floor = self.world.get(x, y - 1, z);
            let free = |s: State| !r.has(s, F_SOLID | F_FLUID);
            if r.has(floor, F_OPAQUE) && free(self.world.get(x, y, z)) && free(self.world.get(x, y + 1, z)) {
                return Some((y, floor));
            }
            y -= 1;
            if y <= MIN_Y {
                break;
            }
        }
        None
    }

    fn natural_spawning(&mut self) {
        let r = reg();
        let p = self.player.pos;
        let near = |m: &Mob| (m.pos - p).length_sq() < 128.0 * 128.0;
        let hostile = self.mobs.iter().filter(|m| m.kind.hostile() && near(m)).count();
        let passive = self.mobs.iter().filter(|m| !m.kind.hostile() && m.kind != Kind::SulfurCube && near(m)).count();
        let cubes = self.mobs.iter().filter(|m| m.kind == Kind::SulfurCube && near(m)).count();
        let darken = self.sky_darken();
        // Monsters and cubes: a few random spots each tick, anywhere from bedrock to the surface.
        for _ in 0..3 {
            let a = self.rng.f64() * std::f64::consts::TAU;
            let d = 24.0 + self.rng.f64() * 40.0;
            let (x, z) = ((p.x + a.cos() * d).floor() as i32, (p.z + a.sin() * d).floor() as i32);
            if !self.world.is_ready(x, z) {
                continue;
            }
            let top = self.world.top(x, z);
            let y0 = self.rng.range(MIN_Y + 4, top + 2);
            let Some((y, floor)) = self.standing_spot(x, y0, z, 10) else { continue };
            let pos = DVec3::new(x as f64 + 0.5, y as f64, z as f64 + 0.5);
            if (pos - p).length_sq() < 24.0 * 24.0 {
                continue;
            }
            let light = self.world.light(x, y, z);
            let floor_name = r.block(floor).name;
            let in_sulfur_cave = y < top - 4 && self.world.chunk_at(x, z).map_or(false, |c| c.cave_biome[((z & 15) * 16 + (x & 15)) as usize] == Biome::SulfurCaves as u8);
            if in_sulfur_cave && (floor_name.contains("sulfur") || floor_name.contains("cinnabar")) {
                if cubes < CUBE_CAP && self.rng.one_in(4) {
                    self.spawn_mob(Kind::SulfurCube, pos);
                }
                continue;
            }
            if self.settings.peaceful || hostile >= HOSTILE_CAP {
                continue;
            }
            // Monsters need darkness: no block light and little sky light.
            if light & 15 == 0 && (light >> 4).saturating_sub(darken) <= self.rng.below(8) as u8 {
                let kind = *self.rng.pick(&[Kind::Zombie, Kind::Zombie, Kind::Spider, Kind::Creeper]);
                let (w, h, _, _) = kind.stats();
                let mut scratch = std::mem::take(&mut self.scratch);
                let fits = super::physics::is_free(&self.world, &Aabb::from_feet(pos, w, h), &mut scratch);
                self.scratch = scratch;
                if fits {
                    self.spawn_mob(kind, pos);
                }
            }
        }
        // Animals: now and then a small herd on sunlit grass.
        if self.tick % 20 == 0 && passive < PASSIVE_CAP {
            let a = self.rng.f64() * std::f64::consts::TAU;
            let d = 20.0 + self.rng.f64() * 44.0;
            let (x, z) = ((p.x + a.cos() * d).floor() as i32, (p.z + a.sin() * d).floor() as i32);
            if !self.world.is_ready(x, z) {
                return;
            }
            let kind = *self.rng.pick(&[Kind::Pig, Kind::Cow, Kind::Sheep, Kind::Chicken]);
            let herd = self.rng.range(2, 4);
            for _ in 0..herd {
                let (hx, hz) = (x + self.rng.range(-3, 3), z + self.rng.range(-3, 3));
                let y = self.world.top(hx, hz) + 1;
                let floor = self.world.get(hx, y - 1, hz);
                if r.block(floor).name == "grass_block" && self.world.light(hx, y, hz) >> 4 >= 9 && self.world.is_ready(hx, hz) {
                    self.spawn_mob(kind, DVec3::new(hx as f64 + 0.5, y as f64, hz as f64 + 0.5));
                }
            }
        }
    }

    /// Thinking, moving, fighting, spawning and despawning.
    pub fn tick_mobs(&mut self) {
        let survival = self.player.mode == GameMode::Survival;
        let sense = Sense {
            player: self.player.pos,
            player_bb: self.player.aabb(),
            targetable: survival && !self.vitals.dead && !self.settings.peaceful,
            held: self.inv.held(),
            sky_darken: self.sky_darken(),
        };
        // Cubes without a block look for block items lying around and swallow them.
        for i in 0..self.mobs.len() {
            let m = &self.mobs[i];
            if m.kind != Kind::SulfurCube || m.block != AIR || m.small || !m.alive() {
                continue;
            }
            let found = self.drops.iter().enumerate().filter(|(_, e)| e.delay == 0 && absorbable(e.stack.def()).is_some()).map(|(j, e)| (j, (e.pos - m.pos).length_sq())).filter(|(_, d)| *d < 64.0).min_by(|a, b| a.1.total_cmp(&b.1));
            let m = &mut self.mobs[i];
            m.goal = None;
            if let Some((j, d)) = found {
                let e = &mut self.drops[j];
                if d < 1.2 {
                    if let Some(state) = absorbable(e.stack.def()) {
                        e.stack.take(1);
                        m.block = state;
                        m.persistent = true;
                        m.squish = 0.4;
                        m.vel = DVec3::ZERO;
                    }
                } else {
                    m.goal = Some((e.pos.x, e.pos.z));
                }
            }
        }
        self.drops.retain(|e| !e.stack.is_empty());

        let mut scratch = std::mem::take(&mut self.scratch);
        let mut actions: Vec<(usize, Action)> = Vec::new();
        for (i, m) in self.mobs.iter_mut().enumerate() {
            let a = m.tick(&self.world, &sense, &mut self.rng, &mut scratch);
            if a != Action::None {
                actions.push((i, a));
            }
        }
        self.scratch = scratch;

        // Bodies push each other apart, and the player shoves whatever they walk into.
        let player_bb = self.player.aabb();
        for i in 0..self.mobs.len() {
            let (a_pos, a_w) = (self.mobs[i].pos, self.mobs[i].size().0);
            if !self.mobs[i].alive() {
                continue;
            }
            for j in i + 1..self.mobs.len() {
                let (b_pos, b_w) = (self.mobs[j].pos, self.mobs[j].size().0);
                let (dx, dz) = (b_pos.x - a_pos.x, b_pos.z - a_pos.z);
                let reach = (a_w + b_w) * 0.5;
                if dx.abs() < reach && dz.abs() < reach && (a_pos.y - b_pos.y).abs() < 1.0 && self.mobs[j].alive() {
                    let len = (dx * dx + dz * dz).sqrt().max(0.01);
                    let push = 0.03;
                    self.mobs[i].vel.x -= dx / len * push;
                    self.mobs[i].vel.z -= dz / len * push;
                    self.mobs[j].vel.x += dx / len * push;
                    self.mobs[j].vel.z += dz / len * push;
                }
            }
            let m = &mut self.mobs[i];
            if self.player.mode != GameMode::Spectator && m.aabb().intersects(&player_bb) {
                let (dx, dz) = (m.pos.x - self.player.pos.x, m.pos.z - self.player.pos.z);
                let len = (dx * dx + dz * dz).sqrt().max(0.01);
                let shove = if m.block != AIR { 0.06 } else { 0.04 };
                m.vel.x += dx / len * shove;
                m.vel.z += dz / len * shove;
                // A cube that swallowed magma burns whoever touches it.
                if m.archetype() == Some(Archetype::Hot) && survival {
                    self.vitals.damage(1.0, "was scorched by a Sulfur Cube");
                }
            }
        }

        // Highest index first so removals do not disturb the rest.
        for (i, action) in actions.into_iter().rev() {
            match action {
                Action::Attack(amount) => {
                    if survival && !self.vitals.dead {
                        let m = &self.mobs[i];
                        let (dx, dz) = (self.player.pos.x - m.pos.x, self.player.pos.z - m.pos.z);
                        let len = (dx * dx + dz * dz).sqrt().max(0.01);
                        let name = super::hud::pretty_name(m.kind.name());
                        let before = self.vitals.health;
                        self.vitals.damage(amount, &format!("was slain by {name}"));
                        if self.vitals.health < before {
                            self.player.vel.x += dx / len * 0.4;
                            self.player.vel.z += dz / len * 0.4;
                            if self.player.on_ground {
                                self.player.vel.y = 0.3;
                            }
                        }
                    }
                }
                Action::Explode(power) => {
                    let m = self.mobs.swap_remove(i);
                    let (_, h) = m.size();
                    let centre = m.pos + DVec3::new(0.0, h * 0.5, 0.0);
                    self.explode(centre, power, m.kind == Kind::SulfurCube);
                    if self.vitals.dead && self.vitals.death_message == "blew up" {
                        self.vitals.death_message = format!("was blown up by {}", super::hud::pretty_name(m.kind.name()));
                    }
                }
                Action::Remove => {
                    let m = self.mobs.swap_remove(i);
                    if m.spent {
                        continue;
                    }
                    for stack in m.loot(&mut self.rng) {
                        let mut drop = ItemEntity::from_block(m.pos.floor(), stack, &mut self.rng);
                        drop.pos = m.pos + DVec3::new(0.0, 0.3, 0.0);
                        drop.prev = drop.pos;
                        self.drops.push(drop);
                    }
                    self.puffs(m.pos + DVec3::new(0.0, 0.5, 0.0), 10, 0.8, 0.05, [240, 240, 240, 255], 0.12);
                    // A large Sulfur Cube splits in two.
                    if m.kind == Kind::SulfurCube && !m.small {
                        for side in [-0.3, 0.3] {
                            let mut small = Mob::small_cube(m.pos + DVec3::new(side, 0.1, side), &mut self.rng);
                            small.vel = DVec3::new(side * 0.4, 0.3, -side * 0.4);
                            self.mobs.push(small);
                        }
                    }
                }
                Action::None => {}
            }
        }
        self.on_death();

        // Monsters far from the player fade away; so does everything in peaceful mode.
        let p = self.player.pos;
        let peaceful = self.settings.peaceful;
        let rng = &mut self.rng;
        self.mobs.retain(|m| {
            let d = (m.pos - p).length_sq();
            if m.persistent {
                return d < 300.0 * 300.0;
            }
            let wild = m.kind.hostile() || m.kind == Kind::SulfurCube;
            if m.kind.hostile() && peaceful {
                return false;
            }
            d < 128.0 * 128.0 && !(wild && d > 40.0 * 40.0 && rng.one_in(800))
        });
        if !self.at_title {
            self.natural_spawning();
        }
    }

    /// Explosion effects on mobs: damage, knockback, and lighting any TNT a cube carries.
    pub fn blast_mobs(&mut self, centre: DVec3, power: f32) {
        let reach = power as f64 * 2.0;
        for i in 0..self.mobs.len() {
            let bb = self.mobs[i].aabb();
            let d = bb.center() - centre;
            let dist = d.length();
            if dist >= reach {
                continue;
            }
            let impact = (1.0 - dist / reach) * self.exposure(centre, &bb);
            if impact <= 0.0 {
                continue;
            }
            let fuse = 15 + self.rng.below(46) as i16;
            let m = &mut self.mobs[i];
            m.vel += d * (impact / dist.max(0.1));
            if m.archetype() == Some(Archetype::Explosive) {
                if m.fuse == 0 {
                    m.fuse = fuse;
                }
            } else if m.block == AIR {
                m.hurt = 0;
                m.damage(((impact * impact + impact) / 2.0 * 7.0 * reach + 1.0).floor() as f32, None);
            }
        }
    }

    /// Draws every mob in view for one pass.
    pub fn draw_mobs(&mut self, eye: DVec3, alpha: f64, env: &crate::gfx::Environment, translucent: bool) {
        for i in 0..self.mobs.len() {
            let m = &self.mobs[i];
            if translucent && m.kind != Kind::SulfurCube {
                continue;
            }
            let p = m.prev.lerp(m.pos, alpha);
            let rel = p - eye;
            if rel.length_sq() > 80.0 * 80.0 {
                continue;
            }
            let light = self.light_at(p + DVec3::new(0.0, m.size().1 * 0.6, 0.0), env);
            let rel = Vec3::new(rel.x as f32, rel.y as f32, rel.z as f32);
            let m = &self.mobs[i];
            m.draw(&mut self.renderer.batch, rel, alpha as f32, light, translucent);
        }
    }

    /// The place a bucketed Sulfur Cube is released at.
    pub fn release_cube(&mut self, pos: IVec3) {
        let at = DVec3::new(pos.x as f64 + 0.5, pos.y as f64 + 0.05, pos.z as f64 + 0.5);
        if let Some(m) = self.spawn_mob(Kind::SulfurCube, at) {
            m.persistent = true;
            m.squish = 0.4;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::player::tests::flat_world;

    fn sense(player: DVec3, targetable: bool) -> Sense {
        Sense { player, player_bb: Aabb::from_feet(player, 0.6, 1.8), targetable, held: ItemStack::EMPTY, sky_darken: 0 }
    }

    fn run(m: &mut Mob, w: &World, s: &Sense, ticks: usize) -> Vec<Action> {
        let mut rng = Rng::new(9);
        let mut scratch = Vec::new();
        (0..ticks).map(|_| m.tick(w, s, &mut rng, &mut scratch)).filter(|a| *a != Action::None).collect()
    }

    #[test]
    fn archetypes_follow_the_absorbed_block() {
        let r = reg();
        let of = |n: &str| Archetype::of(r.block_named(n).unwrap_or_else(|| panic!("no block {n}")));
        assert_eq!(of("dirt"), Archetype::Regular);
        assert_eq!(of("oak_planks"), Archetype::Bouncy);
        assert_eq!(of("oak_log"), Archetype::Bouncy);
        assert_eq!(of("stone"), Archetype::SlowBouncy);
        assert_eq!(of("iron_ore"), Archetype::SlowBouncy);
        assert_eq!(of("iron_block"), Archetype::SlowFlat);
        assert_eq!(of("white_wool"), Archetype::Light);
        assert_eq!(of("packed_ice"), Archetype::FastSliding);
        assert_eq!(of("snow_block"), Archetype::FastSliding);
        assert_eq!(of("mycelium"), Archetype::SlowSliding);
        assert_eq!(of("soul_sand"), Archetype::HighResistance);
        assert_eq!(of("honeycomb_block"), Archetype::Sticky);
        assert_eq!(of("tnt"), Archetype::Explosive);
        assert_eq!(of("magma_block"), Archetype::Hot);
        // Only full blocks can be swallowed.
        assert!(absorbable(ItemStack::named("stone", 1).def()).is_some());
        assert!(absorbable(ItemStack::named("oak_slab", 1).def()).is_none());
        assert!(absorbable(ItemStack::named("torch", 1).def()).is_none());
        assert!(absorbable(ItemStack::named("stick", 1).def()).is_none());
    }

    #[test]
    fn a_zombie_walks_to_the_player_and_attacks() {
        let w = flat_world();
        let mut rng = Rng::new(1);
        let mut z = Mob::new(Kind::Zombie, DVec3::new(0.5, 61.0, 0.5), &mut rng);
        let s = sense(DVec3::new(8.5, 61.0, 0.5), true);
        let actions = run(&mut z, &w, &s, 200);
        assert!(z.pos.x > 7.0, "zombie stopped at {:?}", z.pos);
        assert!(actions.iter().any(|a| matches!(a, Action::Attack(d) if *d == 3.0)));
        // About 2.3 blocks per second on flat ground.
        let mut z = Mob::new(Kind::Zombie, DVec3::new(0.5, 61.0, 0.5), &mut rng);
        let far = sense(DVec3::new(15.5, 61.0, 0.5), true);
        run(&mut z, &w, &far, 20);
        let x0 = z.pos.x;
        run(&mut z, &w, &far, 40);
        let speed = (z.pos.x - x0) / 2.0;
        assert!((speed - 2.28).abs() < 0.2, "zombie speed {speed} blocks/s");
        // It leaves a creative player alone.
        let mut z = Mob::new(Kind::Zombie, DVec3::new(0.5, 61.0, 0.5), &mut rng);
        let actions = run(&mut z, &w, &sense(DVec3::new(1.2, 61.0, 0.5), false), 100);
        assert!(actions.is_empty());
    }

    #[test]
    fn a_creeper_swells_then_explodes_and_calms_down_when_left() {
        let w = flat_world();
        let mut rng = Rng::new(2);
        let mut c = Mob::new(Kind::Creeper, DVec3::new(0.5, 61.0, 0.5), &mut rng);
        let actions = run(&mut c, &w, &sense(DVec3::new(2.5, 61.0, 0.5), true), 30);
        assert_eq!(actions, vec![Action::Explode(3.0)]);
        let mut c = Mob::new(Kind::Creeper, DVec3::new(0.5, 61.0, 0.5), &mut rng);
        run(&mut c, &w, &sense(DVec3::new(2.5, 61.0, 0.5), true), 15);
        assert!(c.fuse >= 10);
        let actions = run(&mut c, &w, &sense(DVec3::new(2.5, 61.0, 0.5), false), 40);
        assert!(actions.is_empty() && c.fuse == 0);
    }

    #[test]
    fn animals_take_damage_flee_and_drop_loot() {
        let w = flat_world();
        let mut rng = Rng::new(3);
        let mut pig = Mob::new(Kind::Pig, DVec3::new(0.5, 61.0, 0.5), &mut rng);
        let s = sense(DVec3::new(-1.0, 61.0, 0.5), true);
        run(&mut pig, &w, &s, 5);
        assert!(pig.damage(4.0, Some(s.player)));
        assert!(!pig.damage(4.0, Some(s.player)), "no second hit during the flash");
        run(&mut pig, &w, &s, 20);
        assert!(pig.pos.x > 5.0, "the pig should have run away, it is at {:?}", pig.pos);
        assert!(pig.damage(10.0, None));
        let actions = run(&mut pig, &w, &s, 20);
        assert_eq!(actions, vec![Action::Remove]);
        let loot = pig.loot(&mut rng);
        assert_eq!(loot.len(), 1);
        assert_eq!(loot[0].def().name, "porkchop");
        // Falling hurts, except for chickens.
        let mut cow = Mob::new(Kind::Cow, DVec3::new(4.5, 71.0, 4.5), &mut rng);
        let mut hen = Mob::new(Kind::Chicken, DVec3::new(6.5, 71.0, 6.5), &mut rng);
        cow.idle = 500;
        hen.idle = 500;
        run(&mut cow, &w, &s, 60);
        run(&mut hen, &w, &s, 200);
        assert!(cow.on_ground && (cow.health - 3.0).abs() < 1.01, "cow health {}", cow.health);
        assert!(hen.on_ground && hen.health == 4.0);
    }

    #[test]
    fn sulfur_cube_balls_bounce_and_slide_by_archetype() {
        let w = flat_world();
        let r = reg();
        let mut rng = Rng::new(4);
        let s = sense(DVec3::new(40.0, 61.0, 40.0), false);
        // Drop from six blocks: wood bounces high, ice does not bounce at all.
        let peak_after_drop = |block: &str, rng: &mut Rng| {
            let mut m = Mob::new(Kind::SulfurCube, DVec3::new(0.5, 67.0, 0.5), rng);
            m.block = r.default_state(block).unwrap();
            let mut scratch = Vec::new();
            let (mut landed, mut peak) = (false, 61.0f64);
            for _ in 0..200 {
                m.tick(&w, &s, rng, &mut scratch);
                if m.pos.y <= 61.001 {
                    landed = true;
                }
                if landed {
                    peak = peak.max(m.pos.y);
                }
            }
            assert!((m.pos.y - 61.0).abs() < 1e-6 && m.vel.y == 0.0, "{block} ball never came to rest");
            peak - 61.0
        };
        let (wood, stone_like, ice, wool) = (peak_after_drop("oak_planks", &mut rng), peak_after_drop("iron_block", &mut rng), peak_after_drop("packed_ice", &mut rng), peak_after_drop("white_wool", &mut rng));
        assert!(wood > 2.0, "wood bounce {wood}");
        assert!(stone_like < 0.6 && stone_like < wood, "metal bounce {stone_like}");
        assert!(ice < 1e-6, "ice bounce {ice}");
        assert!(wool > 0.5 && wool < wood, "wool is slowed by the air: {wool}");
        // Push along the ground: ice slides far, honeycomb stops dead.
        let slide = |block: &str, rng: &mut Rng| {
            let mut m = Mob::new(Kind::SulfurCube, DVec3::new(0.5, 61.0, 0.5), rng);
            m.block = r.default_state(block).unwrap();
            m.vel = DVec3::new(0.5, 0.0, 0.0);
            let mut scratch = Vec::new();
            for _ in 0..300 {
                m.tick(&w, &s, rng, &mut scratch);
            }
            m.pos.x - 0.5
        };
        let (ice, dirt, honey) = (slide("packed_ice", &mut rng), slide("dirt", &mut rng), slide("honeycomb_block", &mut rng));
        assert!(ice > 10.0, "ice slid {ice}");
        assert!(dirt > 1.0 && dirt < ice / 3.0, "dirt slid {dirt}");
        assert!(honey < 0.7, "honeycomb slid {honey}");
        // A ball takes no damage path through `tick`, and a cube without a block hops.
        let mut free = Mob::new(Kind::SulfurCube, DVec3::new(0.5, 61.0, 0.5), &mut rng);
        let start = free.pos;
        let mut scratch = Vec::new();
        let mut highest = 61.0f64;
        for _ in 0..400 {
            free.tick(&w, &s, &mut rng, &mut scratch);
            highest = highest.max(free.pos.y);
        }
        assert!(highest > 61.8, "the cube never hopped");
        assert!((free.pos - start).horizontal_len() > 1.0, "the cube never went anywhere");
        // TNT inside: the fuse runs down and it goes off without splitting.
        let mut bomb = Mob::new(Kind::SulfurCube, DVec3::new(0.5, 61.0, 0.5), &mut rng);
        bomb.block = r.k.tnt;
        bomb.fuse = 20;
        let actions = run(&mut bomb, &w, &s, 30);
        assert_eq!(actions.first(), Some(&Action::Explode(4.0)));
        assert!(bomb.spent);
    }
}
