//! Items: block items plus tools, materials and food.  Numbers (durability, mining speed,
//! food values, burn times) follow the published vanilla values.

pub mod recipes;

use crate::art::itemtex::TOOL_MATERIALS;
use crate::art::palette::DYES;
use crate::util::rng::Rng;
use crate::world::block::*;
use crate::world::model::Shape;
use std::collections::HashMap;

pub type ItemId = u16;

#[derive(Clone)]
pub struct ItemDef {
    pub name: &'static str,
    /// Block placed when the item is used on a surface.
    pub block: Option<u16>,
    pub max_stack: u8,
    /// Texture layer of the flat icon (0 for items drawn as their block).
    pub icon: u16,
    pub tool: Tool,
    pub tier: u8,
    pub speed: f32,
    pub durability: u16,
    pub attack: f32,
    pub food: u8,
    pub saturation: f32,
    /// Furnace burn time in ticks.
    pub fuel: u16,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct ItemStack {
    pub id: ItemId,
    pub count: u8,
    pub damage: u16,
}

impl ItemStack {
    pub const EMPTY: ItemStack = ItemStack { id: 0, count: 0, damage: 0 };
    pub fn new(id: ItemId, count: u8) -> ItemStack {
        if id == 0 || count == 0 { ItemStack::EMPTY } else { ItemStack { id, count, damage: 0 } }
    }
    pub fn named(name: &str, count: u8) -> ItemStack {
        ItemStack::new(reg().items.id(name).unwrap_or(0), count)
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.id == 0 || self.count == 0
    }
    pub fn def(&self) -> &'static ItemDef {
        &reg().items.defs[self.id as usize]
    }
    pub fn max(&self) -> u8 {
        self.def().max_stack
    }
    /// True when `other` can be merged into this stack.
    pub fn stacks_with(&self, other: &ItemStack) -> bool {
        self.id == other.id && self.damage == other.damage && self.max() > 1
    }
    /// Removes `n` items; the stack becomes empty when it runs out.
    pub fn take(&mut self, n: u8) -> ItemStack {
        let n = n.min(self.count);
        let out = ItemStack { id: self.id, count: n, damage: self.damage };
        self.count -= n;
        if self.count == 0 {
            *self = ItemStack::EMPTY;
        }
        if n == 0 { ItemStack::EMPTY } else { out }
    }
    /// Uses up durability; returns true when the tool breaks.
    pub fn wear(&mut self, amount: u16) -> bool {
        let max = self.def().durability;
        if max == 0 {
            return false;
        }
        self.damage += amount;
        if self.damage >= max {
            *self = ItemStack::EMPTY;
            true
        } else {
            false
        }
    }
}

#[derive(Default)]
pub struct Items {
    pub defs: Vec<ItemDef>,
    by_name: HashMap<&'static str, ItemId>,
}

impl Items {
    pub fn id(&self, name: &str) -> Option<ItemId> {
        self.by_name.get(name.strip_prefix("minecraft:").unwrap_or(name)).copied()
    }
    pub fn get(&self, id: ItemId) -> &ItemDef {
        &self.defs[id as usize]
    }
    /// Item that places the given block, if any.
    pub fn for_block(&self, block: &BlockDef) -> Option<ItemId> {
        let name = match block.name {
            "wheat" => "wheat_seeds",
            "carrots" => "carrot",
            "potatoes" => "potato",
            "beetroots" => "beetroot_seeds",
            n => n,
        };
        self.id(name)
    }
    fn add(&mut self, def: ItemDef) -> ItemId {
        assert!(!self.by_name.contains_key(def.name), "duplicate item {}", def.name);
        let id = self.defs.len() as ItemId;
        self.by_name.insert(def.name, id);
        self.defs.push(def);
        id
    }
}

fn base(name: &str) -> ItemDef {
    ItemDef { name: leak(name), block: None, max_stack: 64, icon: 0, tool: Tool::None, tier: 0, speed: 1.0, durability: 0, attack: 1.0, food: 0, saturation: 0.0, fuel: 0 }
}

fn block_fuel(b: &BlockDef) -> u16 {
    let n = b.name;
    if n == "coal_block" {
        16000
    } else if n.ends_with("_sapling") || n.ends_with("_wool") || n == "dead_bush" {
        100
    } else if n.ends_with("_carpet") {
        67
    } else if n.ends_with("_slab") && b.sound == Sound::Wood {
        150
    } else if n.ends_with("_door") && b.sound == Sound::Wood {
        200
    } else if b.sound == Sound::Wood && b.flags & F_FLAMMABLE != 0 || matches!(n, "chest" | "ladder" | "crafting_table" | "bookshelf" | "note_block" | "jukebox") {
        300
    } else {
        0
    }
}

/// Builds the item table.  Called once while the block registry is being created.
pub fn register_all(r: &mut Registry) {
    let mut items = Items::default();
    items.add(ItemDef { max_stack: 0, ..base("air") });
    let block_items: Vec<(usize, &'static str, u16)> = r.blocks.iter().enumerate().filter(|(_, b)| !b.no_item).map(|(i, b)| (i, b.name, block_fuel(b))).collect();
    for (i, name, fuel) in block_items {
        items.add(ItemDef { block: Some(i as u16), fuel, ..base(name) });
    }
    let block_index = |r: &Registry, n: &str| r.block_index(n);
    let mut plain = |r: &mut Registry, def: ItemDef| {
        let icon = r.texture(&format!("item/{}", def.name));
        items.add(ItemDef { icon, ..def })
    };
    // Tools: (tier, speed, durability, attack bonus) per material.
    let stats: [(u8, f32, u16, f32); 6] = [(1, 2.0, 59, 0.0), (2, 4.0, 131, 1.0), (3, 6.0, 250, 2.0), (1, 12.0, 32, 0.0), (4, 8.0, 1561, 3.0), (5, 9.0, 2031, 4.0)];
    for ((material, _), (tier, speed, durability, bonus)) in TOOL_MATERIALS.iter().zip(stats) {
        let fuel = if *material == "wooden" { 200 } else { 0 };
        for (kind, tool, attack) in [("pickaxe", Tool::Pickaxe, 2.0 + bonus), ("axe", Tool::Axe, 7.0 + bonus.min(2.0)), ("shovel", Tool::Shovel, 2.5 + bonus), ("hoe", Tool::Hoe, 1.0), ("sword", Tool::Sword, 4.0 + bonus)] {
            plain(r, ItemDef { max_stack: 1, tool, tier, speed, durability, attack, fuel, ..base(&format!("{material}_{kind}")) });
        }
    }
    plain(r, ItemDef { max_stack: 1, tool: Tool::Shears, speed: 5.0, durability: 238, ..base("shears") });
    plain(r, ItemDef { max_stack: 1, durability: 64, ..base("flint_and_steel") });
    plain(r, ItemDef { max_stack: 16, ..base("bucket") });
    for (n, fuel) in [("water_bucket", 0), ("lava_bucket", 20000), ("milk_bucket", 0), ("sulfur_cube_bucket", 0)] {
        plain(r, ItemDef { max_stack: 1, fuel, ..base(n) });
    }
    for (n, fuel) in [("stick", 100), ("coal", 1600), ("charcoal", 1600), ("bowl", 100)] {
        plain(r, ItemDef { fuel, ..base(n) });
    }
    for n in [
        "flint", "iron_ingot", "copper_ingot", "gold_ingot", "netherite_ingot", "brick", "nether_brick", "diamond", "emerald", "lapis_lazuli",
        "quartz", "amethyst_shard", "redstone", "glowstone_dust", "gunpowder", "sugar", "bone_meal", "sulfur_dust", "gold_nugget", "iron_nugget",
        "clay_ball", "slime_ball", "raw_iron", "raw_copper", "raw_gold", "bone", "string", "feather", "leather", "paper", "book", "wheat",
        "melon_seeds", "pumpkin_seeds",
    ] {
        plain(r, base(n));
    }
    for n in ["snowball", "egg"] {
        plain(r, ItemDef { max_stack: 16, ..base(n) });
    }
    for (c, _) in DYES.iter() {
        plain(r, base(&format!("{c}_dye")));
    }
    for (n, block) in [("wheat_seeds", "wheat"), ("beetroot_seeds", "beetroots")] {
        let b = block_index(r, block);
        plain(r, ItemDef { block: b, ..base(n) });
    }
    for (n, food, saturation, block) in [
        ("apple", 4, 2.4, ""), ("golden_apple", 4, 9.6, ""), ("melon_slice", 2, 1.2, ""), ("sweet_berries", 2, 0.4, ""), ("glow_berries", 2, 0.4, "cave_vines"),
        ("bread", 5, 6.0, ""), ("cookie", 2, 0.4, ""), ("carrot", 3, 3.6, "carrots"), ("potato", 1, 0.6, "potatoes"), ("baked_potato", 5, 6.0, ""),
        ("beetroot", 1, 1.2, ""), ("porkchop", 3, 1.8, ""), ("cooked_porkchop", 8, 12.8, ""), ("beef", 3, 1.8, ""), ("cooked_beef", 8, 12.8, ""),
        ("mutton", 2, 1.2, ""), ("cooked_mutton", 6, 9.6, ""), ("chicken", 2, 1.2, ""), ("cooked_chicken", 6, 7.2, ""), ("rabbit", 3, 1.8, ""),
        ("cooked_rabbit", 5, 6.0, ""), ("cod", 2, 0.4, ""), ("cooked_cod", 5, 6.0, ""), ("salmon", 2, 0.4, ""), ("cooked_salmon", 6, 9.6, ""),
        ("rotten_flesh", 4, 0.8, ""),
    ] {
        let b = if block.is_empty() { None } else { block_index(r, block) };
        plain(r, ItemDef { food, saturation, block: b, ..base(n) });
    }
    r.items = items;
}

/// Mining speed of a held item against a block, and whether the block will drop anything.
pub fn mining(held: &ItemDef, block: &BlockDef) -> (f32, bool) {
    let matches = held.tool != Tool::None && held.tool == block.tool;
    let sword_on_soft = held.tool == Tool::Sword && (block.flags & F_LEAVES != 0 || block.name == "cobweb");
    let speed = if matches { held.speed } else if sword_on_soft { 1.5 } else { 1.0 };
    let harvest = !block.needs_tool || (matches && held.tier >= block.tier);
    (speed, harvest)
}

/// Ticks needed to break a block; 0 means instantly.
pub fn break_ticks(held: &ItemDef, block: &BlockDef, on_ground: bool, in_water: bool) -> Option<u32> {
    if block.hardness < 0.0 {
        return None;
    }
    if block.hardness == 0.0 {
        return Some(0);
    }
    let (mut speed, harvest) = mining(held, block);
    if in_water {
        speed /= 5.0;
    }
    if !on_ground {
        speed /= 5.0;
    }
    let per_tick = speed / block.hardness / if harvest { 30.0 } else { 100.0 };
    Some(if per_tick >= 1.0 { 0 } else { (1.0 / per_tick).ceil() as u32 })
}

/// Items produced by breaking a block state with the given held item.
pub fn block_drops(state: State, held: &ItemDef, rng: &mut Rng) -> Vec<ItemStack> {
    let r = reg();
    let b = r.block(state);
    let (_, harvest) = mining(held, b);
    if !harvest {
        return Vec::new();
    }
    // Snow layers only yield snowballs to a shovel; water and explosions leave nothing.
    if b.shape == Shape::Layers {
        return vec![ItemStack::named("snowball", r.prop(state, "layers").unwrap_or(0) as u8 + 1)];
    }
    block_loot(state, held.tool == Tool::Shears, rng)
}

/// What a block leaves behind when it is destroyed, regardless of tool requirements
/// (explosions, water, lost support).
pub fn block_loot(state: State, shears: bool, rng: &mut Rng) -> Vec<ItemStack> {
    let r = reg();
    let b = r.block(state);
    let mut out = Vec::new();
    let me = |n: u8| r.items.for_block(b).map(|id| ItemStack::new(id, n));
    let mut item = |name: &str, n: u8| {
        if n > 0 {
            out.push(ItemStack::named(name, n));
        }
    };
    let prop = |name: &str| r.prop(state, name).unwrap_or(0);
    // Second halves of two-block-tall things drop nothing themselves.
    if matches!(b.shape, Shape::Door | Shape::DoublePlant) && prop("half") == 1 {
        return out;
    }
    match b.shape {
        Shape::Crop(stages) => {
            let ripe = prop("age") as u8 == stages - 1;
            match b.name {
                "wheat" => {
                    item("wheat_seeds", if ripe { rng.range(1, 3) as u8 } else { 1 });
                    item("wheat", ripe as u8);
                }
                "carrots" => item("carrot", if ripe { rng.range(2, 5) as u8 } else { 1 }),
                "potatoes" => item("potato", if ripe { rng.range(2, 5) as u8 } else { 1 }),
                _ => {
                    item("beetroot_seeds", if ripe { rng.range(1, 3) as u8 } else { 1 });
                    item("beetroot", ripe as u8);
                }
            }
            return out;
        }
        Shape::Slab if prop("type") == 2 => {
            out.extend(me(2));
            return out;
        }
        Shape::Layers => return out,
        _ => {}
    }
    match b.drops {
        Drops::Itself => {
            if b.name == "gravel" && rng.chance(0.1) {
                item("flint", 1);
            } else {
                out.extend(me(1));
            }
        }
        Drops::Nothing => {}
        Drops::Item(name, min, max) => item(name, rng.range(min as i32, max as i32) as u8),
        Drops::Shears => {
            if shears {
                out.extend(me(1));
            } else if b.flags & F_LEAVES != 0 {
                if rng.chance(0.05) {
                    item(&b.name.replace("_leaves", "_sapling"), 1);
                }
                if rng.chance(0.02) {
                    item("stick", rng.range(1, 2) as u8);
                }
                if (b.name == "oak_leaves" || b.name == "dark_oak_leaves") && rng.chance(0.005) {
                    item("apple", 1);
                }
            } else if matches!(b.name, "short_grass" | "fern" | "tall_grass") && rng.chance(0.125) {
                item("wheat_seeds", 1);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::art::blocktex;

    #[test]
    fn items_cover_blocks_and_drops() {
        let r = reg();
        assert!(r.items.defs.len() > 500, "{} items", r.items.defs.len());
        // Everything a block can drop must exist as an item.
        for b in &r.blocks {
            if let Drops::Item(name, _, _) = b.drops {
                assert!(r.items.id(name).is_some(), "{} drops unknown item {name}", b.name);
            }
        }
        // Plain items have icons with recipes.
        for d in r.items.defs.iter().filter(|d| d.icon != 0) {
            let tex = &r.textures[d.icon as usize];
            assert!(blocktex::generate(tex).is_some(), "no icon recipe for {tex}");
        }
        let stone = r.items.id("stone").unwrap();
        assert_eq!(r.items.get(stone).block, r.block_index("stone"));
        assert_eq!(r.items.get(r.items.id("wheat_seeds").unwrap()).block, r.block_index("wheat"));
        assert!(r.items.id("water").is_none(), "fluids have no item form");
    }

    #[test]
    fn mining_times_match_vanilla() {
        let r = reg();
        let hand = r.items.get(0);
        let item = |n: &str| r.items.get(r.items.id(n).unwrap());
        let block = |n: &str| r.block_named(n).unwrap();
        // Stone: 7.5 s by hand (no drop), 1.15 s wooden, 0.4 s iron, 0.3 s diamond pickaxe.
        assert_eq!(break_ticks(hand, block("stone"), true, false), Some(150));
        assert_eq!(break_ticks(item("wooden_pickaxe"), block("stone"), true, false), Some(23));
        assert_eq!(break_ticks(item("iron_pickaxe"), block("stone"), true, false), Some(8));
        assert_eq!(break_ticks(item("diamond_pickaxe"), block("stone"), true, false), Some(6));
        // Dirt: 0.75 s by hand; oak log 3 s by hand, 1.5 s wooden axe.
        assert_eq!(break_ticks(hand, block("dirt"), true, false), Some(15));
        assert_eq!(break_ticks(hand, block("oak_log"), true, false), Some(60));
        assert_eq!(break_ticks(item("wooden_axe"), block("oak_log"), true, false), Some(30));
        // Obsidian needs a diamond pickaxe: 9.4 s; bedrock cannot be broken; torches break instantly.
        assert_eq!(break_ticks(item("diamond_pickaxe"), block("obsidian"), true, false), Some(188));
        assert_eq!(break_ticks(item("diamond_pickaxe"), block("bedrock"), true, false), None);
        assert_eq!(break_ticks(hand, block("torch"), true, false), Some(0));
        assert_eq!(break_ticks(hand, block("dirt"), false, true), Some(375));
    }

    #[test]
    fn drops_depend_on_the_tool() {
        let r = reg();
        let mut rng = Rng::new(3);
        let hand = r.items.get(0);
        let item = |n: &str| r.items.get(r.items.id(n).unwrap());
        assert!(block_drops(r.k.stone, hand, &mut rng).is_empty());
        assert_eq!(block_drops(r.k.stone, item("wooden_pickaxe"), &mut rng), vec![ItemStack::named("cobblestone", 1)]);
        let iron = r.default_state("iron_ore").unwrap();
        assert!(block_drops(iron, item("wooden_pickaxe"), &mut rng).is_empty());
        assert_eq!(block_drops(iron, item("stone_pickaxe"), &mut rng), vec![ItemStack::named("raw_iron", 1)]);
        assert_eq!(block_drops(r.k.grass_block, hand, &mut rng), vec![ItemStack::named("dirt", 1)]);
        assert_eq!(block_drops(r.k.glass, hand, &mut rng), vec![]);
        let leaves = r.default_state("oak_leaves").unwrap();
        assert_eq!(block_drops(leaves, item("shears"), &mut rng), vec![ItemStack::named("oak_leaves", 1)]);
        let double = r.parse_state("oak_slab[type=double]").unwrap();
        assert_eq!(block_drops(double, hand, &mut rng), vec![ItemStack::named("oak_slab", 2)]);
        let upper = r.parse_state("oak_door[half=upper]").unwrap();
        assert!(block_drops(upper, hand, &mut rng).is_empty());
        let ripe = r.parse_state("wheat[age=7]").unwrap();
        let d = block_drops(ripe, hand, &mut rng);
        assert!(d.iter().any(|s| s.id == r.items.id("wheat").unwrap()));
        let mut s = ItemStack::named("wooden_pickaxe", 1);
        assert!(!s.wear(58));
        assert!(s.wear(1) && s.is_empty());
    }
}
