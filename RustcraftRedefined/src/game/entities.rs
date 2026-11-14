//! Dropped items, block entities (chests and furnaces) and the player's survival stats.

use super::physics::{fluid_depth, sweep};
use super::player::Player;
use crate::item::recipes::smelt;
use crate::item::ItemStack;
use crate::util::math::{Aabb, DVec3, IVec3};
use crate::util::rng::Rng;
use crate::world::block::{reg, State, F_WATER};
use crate::world::World;

// ------------------------------------------------------------------ dropped items
pub struct ItemEntity {
    pub pos: DVec3,
    pub prev: DVec3,
    pub vel: DVec3,
    pub stack: ItemStack,
    pub age: u32,
    /// Ticks before the player may pick the item up.
    pub delay: u16,
    pub on_ground: bool,
}

impl ItemEntity {
    /// Item popping out of a broken block, with a small random toss.
    pub fn from_block(pos: IVec3, stack: ItemStack, rng: &mut Rng) -> ItemEntity {
        let p = DVec3::new(pos.x as f64 + 0.25 + rng.f64() * 0.5, pos.y as f64 + 0.25 + rng.f64() * 0.5, pos.z as f64 + 0.25 + rng.f64() * 0.5);
        ItemEntity { pos: p, prev: p, vel: DVec3::new(rng.f64() * 0.2 - 0.1, 0.2, rng.f64() * 0.2 - 0.1), stack, age: 0, delay: 10, on_ground: false }
    }
    /// Item thrown by the player in the direction they are looking.
    pub fn thrown(eye: DVec3, dir: DVec3, stack: ItemStack) -> ItemEntity {
        let p = eye + DVec3::new(0.0, -0.3, 0.0);
        ItemEntity { pos: p, prev: p, vel: dir * 0.3 + DVec3::new(0.0, 0.1, 0.0), stack, age: 0, delay: 40, on_ground: false }
    }
    pub fn aabb(&self) -> Aabb {
        Aabb::from_feet(self.pos, 0.25, 0.25)
    }
    /// Advances one tick; returns false when the item should be removed.
    pub fn tick(&mut self, world: &World, scratch: &mut Vec<Aabb>) -> bool {
        self.prev = self.pos;
        self.age += 1;
        if self.delay > 0 {
            self.delay -= 1;
        }
        if !world.is_ready(self.pos.x.floor() as i32, self.pos.z.floor() as i32) {
            return self.age < 6000;
        }
        let bb = self.aabb();
        if fluid_depth(world, &bb, F_WATER) > 0.1 {
            // Float towards the surface.
            self.vel.y = (self.vel.y + 0.02).min(0.06);
            self.vel.x *= 0.95;
            self.vel.z *= 0.95;
        } else {
            self.vel.y -= 0.04;
        }
        let res = sweep(world, &bb, self.vel, 0.0, self.on_ground, scratch);
        self.pos += res.delta;
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
        let friction = if self.on_ground {
            let below = world.get(self.pos.x.floor() as i32, (self.pos.y - 0.3).floor() as i32, self.pos.z.floor() as i32);
            reg().block(below).slipperiness as f64 * 0.98
        } else {
            0.98
        };
        self.vel.x *= friction;
        self.vel.z *= friction;
        self.vel.y *= 0.98;
        self.age < 6000 && self.pos.y > crate::ffi::MIN_Y as f64 - 64.0
    }
}

/// Combines nearby stacks of the same item so piles do not grow without bound.
pub fn merge_items(items: &mut Vec<ItemEntity>) {
    for i in 0..items.len() {
        if items[i].stack.is_empty() {
            continue;
        }
        for j in i + 1..items.len() {
            let (a, b) = (items[i].stack, items[j].stack);
            if b.is_empty() || !a.stacks_with(&b) || (items[i].pos - items[j].pos).length_sq() > 0.6 {
                continue;
            }
            let n = (a.max() - a.count).min(b.count);
            items[i].stack.count += n;
            items[j].stack.take(n);
            items[i].delay = items[i].delay.max(items[j].delay);
        }
    }
    items.retain(|e| !e.stack.is_empty());
}

// ------------------------------------------------------------------ moving blocks
/// A block in motion: falling sand or primed TNT.
pub struct MovingBlock {
    /// Centre of the bottom face.
    pub pos: DVec3,
    pub prev: DVec3,
    pub vel: DVec3,
    pub state: State,
    /// Ticks until primed TNT explodes; negative for falling blocks.
    pub fuse: i32,
    pub age: u32,
    pub on_ground: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MovingEvent {
    None,
    /// A falling block came to rest.
    Landed,
    Explode,
    /// Fell out of the world or got stuck.
    Lost,
}

impl MovingBlock {
    pub fn falling(pos: IVec3, state: State) -> MovingBlock {
        let p = DVec3::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5);
        MovingBlock { pos: p, prev: p, vel: DVec3::ZERO, state, fuse: -1, age: 0, on_ground: false }
    }
    pub fn tnt(pos: IVec3, state: State, fuse: i32, rng: &mut Rng) -> MovingBlock {
        let p = DVec3::new(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5);
        let a = rng.f64() * std::f64::consts::TAU;
        MovingBlock { pos: p, prev: p, vel: DVec3::new(-a.sin() * 0.02, 0.2, -a.cos() * 0.02), state, fuse: fuse.max(1), age: 0, on_ground: false }
    }
    pub fn aabb(&self) -> Aabb {
        Aabb::from_feet(self.pos, 0.98, 0.98)
    }
    pub fn tick(&mut self, world: &World, scratch: &mut Vec<Aabb>) -> MovingEvent {
        self.prev = self.pos;
        self.age += 1;
        if !world.is_ready(self.pos.x.floor() as i32, self.pos.z.floor() as i32) {
            return if self.age > 1200 { MovingEvent::Lost } else { MovingEvent::None };
        }
        self.vel.y -= 0.04;
        let res = sweep(world, &self.aabb(), self.vel, 0.0, self.on_ground, scratch);
        self.pos += res.delta;
        self.on_ground = res.on_ground;
        if res.hit_x {
            self.vel.x = 0.0;
        }
        if res.hit_z {
            self.vel.z = 0.0;
        }
        self.vel = self.vel * 0.98;
        if self.on_ground {
            self.vel.x *= 0.7;
            self.vel.z *= 0.7;
            self.vel.y *= -0.5;
        }
        if self.fuse >= 0 {
            self.fuse -= 1;
            if self.fuse <= 0 {
                return MovingEvent::Explode;
            }
        } else if self.on_ground {
            return MovingEvent::Landed;
        } else if self.age > 600 || self.pos.y < crate::ffi::MIN_Y as f64 - 16.0 {
            return MovingEvent::Lost;
        }
        MovingEvent::None
    }
}

// ------------------------------------------------------------------ particles
/// A short-lived camera-facing sprite: block fragments, smoke, droplets.
#[derive(Clone, Copy)]
pub struct Particle {
    pub pos: DVec3,
    pub prev: DVec3,
    pub vel: DVec3,
    pub life: u16,
    pub layer: u16,
    /// Sub-rectangle of the texture layer: u, v, size.
    pub uv: [f32; 3],
    pub color: [u8; 4],
    pub size: f32,
    pub gravity: f32,
    /// Scales the size each tick (smoke grows, fragments stay the same).
    pub grow: f32,
}

impl Particle {
    /// Advances one tick; returns false when the particle has expired.
    pub fn tick(&mut self, world: &World) -> bool {
        self.prev = self.pos;
        if self.life == 0 {
            return false;
        }
        self.life -= 1;
        self.vel.y -= self.gravity as f64;
        let next = self.pos + self.vel;
        let r = reg();
        // Particles stop at full blocks; anything finer is not worth the cost.
        if r.has(world.get_at(next.floor()), crate::world::block::F_OPAQUE) {
            if r.has(world.get(self.pos.x.floor() as i32, next.y.floor() as i32, self.pos.z.floor() as i32), crate::world::block::F_OPAQUE) {
                self.vel.y = 0.0;
                self.vel.x *= 0.7;
                self.vel.z *= 0.7;
            } else {
                self.vel.x = 0.0;
                self.vel.z = 0.0;
            }
        } else {
            self.pos = next;
        }
        self.vel = self.vel * 0.98;
        self.size *= self.grow;
        true
    }
}

// ------------------------------------------------------------------ block entities
#[derive(Clone, Default)]
pub struct Furnace {
    pub input: ItemStack,
    pub fuel: ItemStack,
    pub output: ItemStack,
    /// Remaining burn ticks of the current fuel item and its total, for the flame icon.
    pub burn: u16,
    pub burn_total: u16,
    /// Cooking progress, 0..200.
    pub cook: u16,
}

pub const COOK_TICKS: u16 = 200;

impl Furnace {
    pub fn lit(&self) -> bool {
        self.burn > 0
    }
    fn can_smelt(&self) -> Option<ItemStack> {
        let result = smelt(self.input.id)?;
        if self.input.is_empty() {
            return None;
        }
        if self.output.is_empty() || (self.output.stacks_with(&result) && self.output.count + result.count <= self.output.max()) {
            Some(result)
        } else {
            None
        }
    }
    /// One game tick.  Returns true when the lit state changed.
    pub fn tick(&mut self) -> bool {
        let was_lit = self.lit();
        if self.burn > 0 {
            self.burn -= 1;
        }
        let result = self.can_smelt();
        if self.burn == 0 && result.is_some() && !self.fuel.is_empty() {
            let def = self.fuel.def();
            if def.fuel > 0 {
                self.burn = def.fuel;
                self.burn_total = def.fuel;
                let was_lava = def.name == "lava_bucket";
                self.fuel.take(1);
                if was_lava && self.fuel.is_empty() {
                    self.fuel = ItemStack::named("bucket", 1);
                }
            }
        }
        match (self.lit(), result) {
            (true, Some(result)) => {
                self.cook += 1;
                if self.cook >= COOK_TICKS {
                    self.cook = 0;
                    if self.output.is_empty() {
                        self.output = result;
                    } else {
                        self.output.count += result.count;
                    }
                    self.input.take(1);
                }
            }
            (false, _) if self.cook > 0 => self.cook = self.cook.saturating_sub(2),
            (true, None) => self.cook = 0,
            _ => {}
        }
        was_lit != self.lit()
    }
}

#[derive(Clone)]
pub enum BlockEntity {
    Chest(Box<[ItemStack; 27]>),
    Furnace(Furnace),
}

impl BlockEntity {
    /// Everything stored inside, for dropping when the block is broken.
    pub fn contents(&self) -> Vec<ItemStack> {
        match self {
            BlockEntity::Chest(items) => items.iter().copied().filter(|s| !s.is_empty()).collect(),
            BlockEntity::Furnace(f) => [f.input, f.fuel, f.output].into_iter().filter(|s| !s.is_empty()).collect(),
        }
    }
}

// ------------------------------------------------------------------ survival stats
pub struct Vitals {
    pub health: f32,
    pub food: i32,
    pub saturation: f32,
    pub exhaustion: f32,
    pub air: i32,
    /// Counts down after taking damage; drives the red flash and brief invulnerability.
    pub hurt_time: u8,
    pub food_timer: u32,
    pub dead: bool,
    pub death_message: String,
    pub contact_timer: u8,
}

impl Default for Vitals {
    fn default() -> Self {
        Vitals { health: 20.0, food: 20, saturation: 5.0, exhaustion: 0.0, air: 300, hurt_time: 0, food_timer: 0, dead: false, death_message: String::new(), contact_timer: 0 }
    }
}

impl Vitals {
    /// Applies damage; `message` describes the death it would cause.
    pub fn damage(&mut self, amount: f32, message: &str) {
        if self.dead || amount <= 0.0 || self.hurt_time > 10 {
            return;
        }
        self.health -= amount;
        self.hurt_time = 20;
        self.exhaustion += 0.1;
        if self.health <= 0.0 {
            self.health = 0.0;
            self.dead = true;
            self.death_message = message.to_string();
        }
    }
    pub fn heal(&mut self, amount: f32) {
        if !self.dead {
            self.health = (self.health + amount).min(20.0);
        }
    }
    pub fn eat(&mut self, food: u8, saturation: f32) {
        self.food = (self.food + food as i32).min(20);
        self.saturation = (self.saturation + saturation).min(self.food as f32);
    }

    /// One survival tick: hunger, regeneration, drowning, lava, the void and fall damage.
    pub fn tick(&mut self, player: &Player, world: &World) {
        let r = reg();
        if self.hurt_time > 0 {
            self.hurt_time -= 1;
        }
        if self.dead {
            return;
        }
        // Exhaustion from movement.
        let moved = player.pos - player.prev_pos;
        let horizontal = (moved.x * moved.x + moved.z * moved.z).sqrt() as f32;
        if player.in_water {
            self.exhaustion += 0.01 * (horizontal + moved.y.abs() as f32);
        } else if player.on_ground && player.sprinting {
            self.exhaustion += 0.1 * horizontal;
        }
        if player.vel.y > 0.4 && !player.on_ground && player.prev_pos.y == player.pos.y.min(player.prev_pos.y) && moved.y > 0.41 {
            self.exhaustion += if player.sprinting { 0.2 } else { 0.05 };
        }
        // Hunger.
        if self.exhaustion > 4.0 {
            self.exhaustion -= 4.0;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else {
                self.food = (self.food - 1).max(0);
            }
        }
        if self.saturation > 0.0 && self.health < 20.0 && self.food >= 20 {
            self.food_timer += 1;
            if self.food_timer >= 10 {
                let spend = self.saturation.min(6.0);
                self.heal(spend / 6.0);
                self.exhaustion += spend;
                self.food_timer = 0;
            }
        } else if self.food >= 18 && self.health < 20.0 {
            self.food_timer += 1;
            if self.food_timer >= 80 {
                self.heal(1.0);
                self.exhaustion += 6.0;
                self.food_timer = 0;
            }
        } else if self.food <= 0 {
            self.food_timer += 1;
            if self.food_timer >= 80 {
                if self.health > 1.0 {
                    self.hurt_time = 0;
                    self.damage(1.0, "starved to death");
                }
                self.food_timer = 0;
            }
        } else {
            self.food_timer = 0;
        }
        // Air.
        if player.head_in_water {
            self.air -= 1;
            if self.air <= -20 {
                self.air = 0;
                self.hurt_time = 0;
                self.damage(2.0, "drowned");
            }
        } else {
            self.air = (self.air + 4).min(300);
        }
        // Contact damage: lava, cactus, magma.
        self.contact_timer = self.contact_timer.saturating_sub(1);
        if self.contact_timer == 0 {
            let bb = player.aabb();
            let mut hurt: Option<(f32, &str)> = None;
            if player.in_lava {
                hurt = Some((4.0, "tried to swim in lava"));
            }
            let touch = bb.expand(0.01, 0.01, 0.01);
            for y in touch.min.y.floor() as i32..=touch.max.y.floor() as i32 {
                for z in touch.min.z.floor() as i32..=touch.max.z.floor() as i32 {
                    for x in touch.min.x.floor() as i32..=touch.max.x.floor() as i32 {
                        let name = r.block(world.get(x, y, z)).name;
                        if name == "cactus" && hurt.is_none() {
                            hurt = Some((1.0, "was pricked to death"));
                        } else if name == "magma_block" && player.on_ground && !player.sneaking && y < bb.min.y.floor() as i32 + 1 && hurt.is_none() {
                            hurt = Some((1.0, "discovered the floor was lava"));
                        }
                    }
                }
            }
            if let Some((amount, msg)) = hurt {
                self.hurt_time = 0;
                self.damage(amount, msg);
                self.contact_timer = 10;
            }
        }
        if player.pos.y < crate::ffi::MIN_Y as f64 - 64.0 {
            self.hurt_time = 0;
            self.damage(4.0, "fell out of the world");
        }
        if player.landed > 3.0 {
            let amount = (player.landed - 3.0).ceil();
            self.hurt_time = 0;
            self.damage(amount, if amount >= 5.0 { "fell from a high place" } else { "hit the ground too hard" });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn furnace_smelts_with_fuel() {
        let mut f = Furnace { input: ItemStack::named("raw_iron", 2), fuel: ItemStack::named("coal", 1), ..Default::default() };
        assert!(f.tick(), "furnace should light");
        assert_eq!(f.burn_total, 1600);
        assert!(f.fuel.is_empty());
        for _ in 0..199 {
            f.tick();
        }
        assert_eq!(f.output, ItemStack::named("iron_ingot", 1));
        assert_eq!(f.input.count, 1);
        for _ in 0..200 {
            f.tick();
        }
        assert_eq!(f.output.count, 2);
        assert!(f.input.is_empty());
        // Nothing left to smelt: the fuel keeps burning down, then the furnace goes out.
        let mut changed = false;
        for _ in 0..1300 {
            changed |= f.tick();
        }
        assert!(changed && !f.lit());
        // Unsmeltable input never lights the furnace.
        let mut g = Furnace { input: ItemStack::named("dirt", 1), fuel: ItemStack::named("coal", 1), ..Default::default() };
        assert!(!g.tick() && !g.lit() && g.fuel.count == 1);
    }

    #[test]
    fn hunger_and_damage() {
        let mut v = Vitals::default();
        v.damage(5.0, "test");
        assert_eq!(v.health, 15.0);
        v.damage(5.0, "test");
        assert_eq!(v.health, 15.0, "invulnerable right after a hit");
        v.hurt_time = 0;
        v.damage(100.0, "was tested");
        assert!(v.dead && v.death_message == "was tested");
        let mut v = Vitals { food: 10, saturation: 0.0, ..Default::default() };
        v.eat(8, 12.8);
        assert_eq!(v.food, 18);
        assert_eq!(v.saturation, 12.8);
        v.eat(8, 12.8);
        assert_eq!((v.food, v.saturation), (20, 20.0));
    }

    #[test]
    fn items_merge() {
        let p = DVec3::new(0.0, 0.0, 0.0);
        let mk = |n| ItemEntity { pos: p, prev: p, vel: p, stack: ItemStack::named("dirt", n), age: 0, delay: 0, on_ground: true };
        let mut items = vec![mk(40), mk(40), mk(3)];
        merge_items(&mut items);
        let total: u32 = items.iter().map(|e| e.stack.count as u32).sum();
        assert_eq!(total, 83);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].stack.count, 64);
    }
}
