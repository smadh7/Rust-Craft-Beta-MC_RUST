//! World saves: the level file (player, inventory, block entities, dropped items) and the world list.
//! Chunks are stored separately by `World` under the same key prefix.

use super::entities::{BlockEntity, Furnace, ItemEntity, Vitals};
use super::inventory::Inventory;
use super::mobs::{Mob, KINDS};
use super::player::{GameMode, Player};
use super::Game;
use crate::item::ItemStack;
use crate::platform;
use crate::util::bin::{Reader, Writer};
use crate::util::math::{DVec3, IVec3};
use crate::world::block::reg;
use std::collections::HashMap;

const MAGIC: &[u8; 4] = b"RCLV";
const VERSION: u8 = 1;

#[derive(Clone, Debug, PartialEq)]
pub struct WorldInfo {
    pub id: String,
    pub name: String,
    pub seed: u64,
    pub mode: GameMode,
}

pub fn prefix(id: &str) -> String {
    format!("saves/{id}/")
}
fn level_key(id: &str) -> String {
    format!("saves/{id}/level.dat")
}

fn mode_to_u8(m: GameMode) -> u8 {
    match m {
        GameMode::Survival => 0,
        GameMode::Creative => 1,
        GameMode::Spectator => 2,
    }
}
fn mode_from_u8(v: u8) -> GameMode {
    match v {
        1 => GameMode::Creative,
        2 => GameMode::Spectator,
        _ => GameMode::Survival,
    }
}

fn write_stack(w: &mut Writer, s: &ItemStack) {
    if s.is_empty() {
        w.u8(0);
    } else {
        w.u8(s.count);
        w.str(s.def().name);
        w.u16(s.damage);
    }
}

fn read_stack(r: &mut Reader) -> Option<ItemStack> {
    let count = r.u8()?;
    if count == 0 {
        return Some(ItemStack::EMPTY);
    }
    let name = r.str()?;
    let damage = r.u16()?;
    // Items that no longer exist disappear instead of breaking the save.
    Some(reg().items.id(&name).map_or(ItemStack::EMPTY, |id| ItemStack { id, count, damage }))
}

fn read_header<'a>(id: &str, data: &'a [u8]) -> Option<(WorldInfo, Reader<'a>)> {
    let mut r = Reader::new(data);
    if r.take(4)? != MAGIC || r.u8()? != VERSION {
        return None;
    }
    let name = r.str()?;
    let seed = r.u64()?;
    let mode = mode_from_u8(r.u8()?);
    Some((WorldInfo { id: id.to_string(), name, seed, mode }, r))
}

/// Saved worlds, sorted by name.
pub fn list_worlds() -> Vec<WorldInfo> {
    let mut out = Vec::new();
    for key in platform::storage_list("saves/") {
        let Some(id) = key.strip_prefix("saves/").and_then(|rest| rest.strip_suffix("/level.dat")) else { continue };
        if id.contains('/') {
            continue;
        }
        if let Some((info, _)) = platform::storage_read(&key).as_deref().and_then(|data| read_header(id, data)) {
            out.push(info);
        }
    }
    out.sort_by_key(|w| w.name.to_lowercase());
    out
}

/// Turns a world name into a storage-safe id.
pub fn sanitize(name: &str) -> String {
    let mut base: String = name.trim().chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' }).collect();
    base.truncate(40);
    if base.trim_matches('_').is_empty() {
        base = "World".to_string();
    }
    // Windows refuses a few device names as file names.
    let upper = base.to_ascii_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL") || ((upper.starts_with("COM") || upper.starts_with("LPT")) && upper.len() == 4) {
        base.push('_');
    }
    base
}

/// Picks an unused save id derived from the world name.
pub fn new_id(name: &str) -> String {
    let base = sanitize(name);
    let taken: Vec<String> = list_worlds().into_iter().map(|w| w.id.to_ascii_lowercase()).collect();
    if !taken.contains(&base.to_ascii_lowercase()) {
        return base;
    }
    (2..).map(|n| format!("{base}-{n}")).find(|c| !taken.contains(&c.to_ascii_lowercase())).unwrap()
}

pub fn delete_world(id: &str) {
    platform::storage_remove(&format!("saves/{id}"));
}

/// Everything the level file holds besides the header.
struct Level {
    tick: u64,
    day_time: u64,
    time_frozen: bool,
    pos: DVec3,
    spawn: DVec3,
    yaw: f32,
    pitch: f32,
    flying: bool,
    health: f32,
    food: i32,
    saturation: f32,
    exhaustion: f32,
    air: i32,
    inv: Inventory,
    block_entities: HashMap<IVec3, BlockEntity>,
    drops: Vec<ItemEntity>,
    /// Positions with a pending fluid tick.
    scheduled: Vec<IVec3>,
    mobs: Vec<Mob>,
}

/// Optional sections after the fixed part of the level file.
const SECTION_SCHEDULED: u8 = 1;
const SECTION_MOBS: u8 = 2;

fn read_level(r: &mut Reader) -> Option<Level> {
    let tick = r.u64()?;
    let day_time = r.u64()?;
    let time_frozen = r.u8()? != 0;
    let mut v = [0.0f64; 6];
    for x in v.iter_mut() {
        *x = r.f64()?;
        if !x.is_finite() {
            return None;
        }
    }
    let (yaw, pitch, flying) = (r.f32()?, r.f32()?, r.u8()? != 0);
    let (health, food, saturation, exhaustion, air) = (r.f32()?, r.u8()? as i32, r.f32()?, r.f32()?, r.u16()? as i32);
    let mut inv = Inventory { selected: (r.u8()? as usize).min(8), ..Default::default() };
    for s in inv.slots.iter_mut() {
        *s = read_stack(r)?;
    }
    let mut block_entities = HashMap::new();
    for _ in 0..r.var()? {
        let pos = IVec3::new(r.i32()?, r.i32()?, r.i32()?);
        let be = match r.u8()? {
            0 => {
                let mut items = Box::new([ItemStack::EMPTY; 27]);
                for s in items.iter_mut() {
                    *s = read_stack(r)?;
                }
                BlockEntity::Chest(items)
            }
            1 => BlockEntity::Furnace(Furnace { input: read_stack(r)?, fuel: read_stack(r)?, output: read_stack(r)?, burn: r.u16()?, burn_total: r.u16()?, cook: r.u16()? }),
            _ => return None,
        };
        block_entities.insert(pos, be);
    }
    let mut drops = Vec::new();
    for _ in 0..r.var()? {
        let pos = DVec3::new(r.f64()?, r.f64()?, r.f64()?);
        let stack = read_stack(r)?;
        let age = r.u32()?;
        if !stack.is_empty() {
            drops.push(ItemEntity { pos, prev: pos, vel: DVec3::ZERO, stack, age, delay: 10, on_ground: false });
        }
    }
    // Sections added by later versions follow; unknown ones end the read.
    let mut scheduled = Vec::new();
    let mut mobs = Vec::new();
    let mut rng = crate::util::rng::Rng::new(tick);
    while r.remaining() > 0 {
        match r.u8()? {
            SECTION_MOBS => {
                for _ in 0..r.var()?.min(10_000) {
                    let kind = r.u8()? as usize;
                    let pos = DVec3::new(r.f64()?, r.f64()?, r.f64()?);
                    let (yaw, health, flags, sheared, fuse) = (r.f32()?, r.f32()?, r.u8()?, r.u16()?, r.u16()?);
                    let block = r.str()?;
                    let Some(kind) = KINDS.get(kind) else { continue };
                    if !(pos.x.is_finite() && pos.y.is_finite() && pos.z.is_finite() && health > 0.0) {
                        continue;
                    }
                    let mut m = Mob::new(*kind, pos, &mut rng);
                    m.yaw = yaw;
                    m.prev_yaw = yaw;
                    m.health = health;
                    m.small = flags & 1 != 0;
                    m.persistent = flags & 2 != 0;
                    m.angry = flags & 4 != 0;
                    m.sheared = sheared;
                    m.fuse = fuse as i16;
                    m.block = crate::world::block::reg().default_state(&block).unwrap_or(crate::world::block::AIR);
                    mobs.push(m);
                }
            }
            SECTION_SCHEDULED => {
                for _ in 0..r.var()?.min(1_000_000) {
                    scheduled.push(IVec3::new(r.i32()?, r.i32()?, r.i32()?));
                }
            }
            _ => break,
        }
    }
    Some(Level { tick, day_time, time_frozen, pos: DVec3::new(v[0], v[1], v[2]), spawn: DVec3::new(v[3], v[4], v[5]), yaw, pitch, flying, health, food, saturation, exhaustion, air, inv, block_entities, drops, scheduled, mobs })
}

impl Game {
    /// Writes the level file and every changed chunk.  Worlds without a save slot are skipped.
    pub fn save_world(&mut self) -> bool {
        let Some(id) = self.world_id.clone() else { return false };
        let mut w = Writer::new();
        w.bytes(MAGIC);
        w.u8(VERSION);
        w.str(&self.world_name);
        w.u64(self.world.seed);
        w.u8(mode_to_u8(self.player.mode));
        w.u64(self.tick);
        w.u64(self.day_time);
        w.u8(self.time_frozen as u8);
        let p = &self.player;
        for v in [p.pos.x, p.pos.y, p.pos.z, self.spawn.x, self.spawn.y, self.spawn.z] {
            w.f64(v);
        }
        w.f32(p.yaw);
        w.f32(p.pitch);
        w.u8(p.flying as u8);
        let v = &self.vitals;
        w.f32(v.health);
        w.u8(v.food.clamp(0, 20) as u8);
        w.f32(v.saturation);
        w.f32(v.exhaustion);
        w.u16(v.air.clamp(0, 300) as u16);
        w.u8(self.inv.selected as u8);
        for s in &self.inv.slots {
            write_stack(&mut w, s);
        }
        w.var(self.block_entities.len() as u64);
        for (pos, be) in &self.block_entities {
            w.i32(pos.x);
            w.i32(pos.y);
            w.i32(pos.z);
            match be {
                BlockEntity::Chest(items) => {
                    w.u8(0);
                    for s in items.iter() {
                        write_stack(&mut w, s);
                    }
                }
                BlockEntity::Furnace(f) => {
                    w.u8(1);
                    for s in [&f.input, &f.fuel, &f.output] {
                        write_stack(&mut w, s);
                    }
                    w.u16(f.burn);
                    w.u16(f.burn_total);
                    w.u16(f.cook);
                }
            }
        }
        // Blocks caught in mid-fall (and lit TNT) are kept as items.
        let r = crate::world::block::reg();
        let airborne: Vec<(DVec3, ItemStack, u32)> = self.moving.iter().filter_map(|m| r.items.for_block(r.block(m.state)).map(|id| (m.pos, ItemStack::new(id, 1), 0))).collect();
        w.var((self.drops.len() + airborne.len()) as u64);
        for (pos, stack, age) in self.drops.iter().map(|e| (e.pos, e.stack, e.age)).chain(airborne) {
            w.f64(pos.x);
            w.f64(pos.y);
            w.f64(pos.z);
            write_stack(&mut w, &stack);
            w.u32(age);
        }
        let alive: Vec<&Mob> = self.mobs.iter().filter(|m| m.alive()).collect();
        if !alive.is_empty() {
            w.u8(SECTION_MOBS);
            w.var(alive.len() as u64);
            for m in alive {
                w.u8(KINDS.iter().position(|k| *k == m.kind).unwrap_or(0) as u8);
                w.f64(m.pos.x);
                w.f64(m.pos.y);
                w.f64(m.pos.z);
                w.f32(m.yaw);
                w.f32(m.health);
                w.u8(m.small as u8 | (m.persistent as u8) << 1 | (m.angry as u8) << 2);
                w.u16(m.sheared);
                w.u16(m.fuse.max(0) as u16);
                w.str(if m.block == crate::world::block::AIR { "" } else { r.block(m.block).name });
            }
        }
        let scheduled = self.sim.scheduled();
        if !scheduled.is_empty() {
            w.u8(SECTION_SCHEDULED);
            w.var(scheduled.len() as u64);
            for p in scheduled {
                w.i32(p.x);
                w.i32(p.y);
                w.i32(p.z);
            }
        }
        platform::storage_write(&level_key(&id), &w.buf);
        let chunks = self.world.save_modified();
        platform::log(&format!("Saved world '{}' ({chunks} chunks written)", self.world_name));
        true
    }

    /// Loads a saved world and enters it.  Returns false (leaving the game untouched) when the
    /// save is missing or damaged.
    pub fn load_world(&mut self, id: &str) -> bool {
        let Some(data) = platform::storage_read(&level_key(id)) else { return false };
        let Some((info, mut reader)) = read_header(id, &data) else { return false };
        let Some(level) = read_level(&mut reader) else {
            platform::log(&format!("The save '{id}' is damaged and could not be loaded"));
            return false;
        };
        self.reset_world(info.seed);
        self.world.store = Some(prefix(id));
        self.world_id = Some(id.to_string());
        self.world_name = info.name;
        self.tick = level.tick;
        self.day_time = level.day_time;
        self.time_frozen = level.time_frozen;
        self.player = Player::new(level.pos);
        self.player.mode = info.mode;
        self.player.yaw = level.yaw;
        self.player.pitch = level.pitch.clamp(-90.0, 90.0);
        self.player.flying = level.flying && info.mode != GameMode::Survival;
        self.spawn = level.spawn;
        self.vitals = Vitals { health: level.health.clamp(1.0, 20.0), food: level.food.min(20), saturation: level.saturation, exhaustion: level.exhaustion, air: level.air, ..Default::default() };
        self.inv = level.inv;
        self.block_entities = level.block_entities;
        self.drops = level.drops;
        self.mobs = level.mobs;
        for p in level.scheduled {
            self.sim.schedule(p, self.tick + 5);
        }
        platform::log(&format!("Loaded world '{}' (seed {})", self.world_name, info.seed as i64));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_storage_safe() {
        assert_eq!(sanitize("New World"), "New_World");
        assert_eq!(sanitize("  ../../etc/passwd "), "______etc_passwd");
        assert_eq!(sanitize("///"), "World");
        assert_eq!(sanitize("con"), "con_");
        assert_eq!(sanitize("COM1"), "COM1_");
        assert!(sanitize(&"x".repeat(200)).len() <= 40);
    }

    #[test]
    fn stacks_round_trip() {
        let mut w = Writer::new();
        let pick = ItemStack { damage: 17, ..ItemStack::named("iron_pickaxe", 1) };
        for s in [ItemStack::EMPTY, ItemStack::named("oak_planks", 33), pick] {
            write_stack(&mut w, &s);
        }
        w.u8(4);
        w.str("an_item_from_the_future");
        w.u16(0);
        let mut r = Reader::new(&w.buf);
        assert_eq!(read_stack(&mut r), Some(ItemStack::EMPTY));
        assert_eq!(read_stack(&mut r), Some(ItemStack::named("oak_planks", 33)));
        assert_eq!(read_stack(&mut r), Some(pick));
        assert_eq!(read_stack(&mut r), Some(ItemStack::EMPTY), "unknown items vanish");
        assert_eq!(read_stack(&mut r), None);
    }
}
