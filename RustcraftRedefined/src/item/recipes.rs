//! Crafting and smelting recipes, written in a compact table form and matched against a grid.

use super::{ItemId, ItemStack};
use crate::art::palette::{DYES, WOODS};
use crate::world::block::reg;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq)]
pub enum Ingredient {
    Empty,
    Item(ItemId),
    /// Named group such as `#planks`.
    Tag(&'static str),
}

impl Ingredient {
    pub fn matches(&self, id: ItemId) -> bool {
        match self {
            Ingredient::Empty => id == 0,
            Ingredient::Item(i) => *i == id,
            Ingredient::Tag(tag) => id != 0 && in_tag(tag, reg().items.get(id).name),
        }
    }
}

/// Item groups used by recipes, defined by name.
pub fn in_tag(tag: &str, name: &str) -> bool {
    match tag {
        "#planks" => name.ends_with("_planks"),
        "#logs" => name.ends_with("_log") || name.ends_with("_wood"),
        "#stone_tool" => matches!(name, "cobblestone" | "cobbled_deepslate" | "blackstone"),
        "#wool" => name.ends_with("_wool"),
        "#coal" => matches!(name, "coal" | "charcoal"),
        "#sand" => matches!(name, "sand" | "red_sand"),
        "#wooden_slab" => name.ends_with("_slab") && WOODS.iter().any(|w| name.starts_with(w.name)),
        _ => false,
    }
}

#[derive(Clone, Debug)]
pub struct Recipe {
    pub out: ItemStack,
    pub width: usize,
    pub height: usize,
    /// Row-major cells for shaped recipes; an unordered list for shapeless ones.
    pub cells: Vec<Ingredient>,
    pub shapeless: bool,
}

pub struct Recipes {
    pub crafting: Vec<Recipe>,
    /// Input ingredient and result.
    pub smelting: Vec<(Ingredient, ItemStack)>,
}

struct Builder {
    list: Vec<Recipe>,
    smelting: Vec<(Ingredient, ItemStack)>,
}

fn ingredient(name: &str) -> Option<Ingredient> {
    if name.is_empty() {
        Some(Ingredient::Empty)
    } else if name.starts_with('#') {
        Some(Ingredient::Tag(crate::world::block::leak(name)))
    } else {
        reg().items.id(name).map(Ingredient::Item)
    }
}

impl Builder {
    /// Shaped recipe: rows of characters, each mapped to an item or tag; a space is an empty cell.
    fn shaped(&mut self, out: &str, count: u8, rows: &[&str], keys: &[(char, &str)]) {
        let Some(id) = reg().items.id(out) else { return };
        let width = rows.iter().map(|r| r.chars().count()).max().unwrap_or(0);
        let mut cells = Vec::new();
        for row in rows {
            let chars: Vec<char> = row.chars().collect();
            for x in 0..width {
                let ch = chars.get(x).copied().unwrap_or(' ');
                if ch == ' ' {
                    cells.push(Ingredient::Empty);
                    continue;
                }
                let Some(name) = keys.iter().find(|(k, _)| *k == ch).map(|(_, n)| *n) else { return };
                let Some(ing) = ingredient(name) else { return };
                cells.push(ing);
            }
        }
        self.list.push(Recipe { out: ItemStack::new(id, count), width, height: rows.len(), cells, shapeless: false });
    }
    fn shapeless(&mut self, out: &str, count: u8, inputs: &[&str]) {
        let Some(id) = reg().items.id(out) else { return };
        let cells: Option<Vec<Ingredient>> = inputs.iter().map(|n| ingredient(n)).collect();
        if let Some(cells) = cells {
            self.list.push(Recipe { out: ItemStack::new(id, count), width: 0, height: 0, cells, shapeless: true });
        }
    }
    fn square(&mut self, out: &str, count: u8, input: &str) {
        self.shaped(out, count, &["##", "##"], &[('#', input)]);
    }
    /// Nine items make a block and the block gives nine items back.
    fn compress(&mut self, block: &str, item: &str) {
        self.shaped(block, 1, &["###", "###", "###"], &[('#', item)]);
        self.shapeless(item, 9, &[block]);
    }
    fn smelt(&mut self, input: &str, out: &str) {
        if let (Some(i), Some(o)) = (ingredient(input), reg().items.id(out)) {
            self.smelting.push((i, ItemStack::new(o, 1)));
        }
    }
}

fn build() -> Recipes {
    let r = reg();
    let mut b = Builder { list: Vec::new(), smelting: Vec::new() };
    // ------------------------------------------------------------ wood
    for w in WOODS.iter() {
        let n = w.name;
        let planks = format!("{n}_planks");
        for src in [format!("{n}_log"), format!("stripped_{n}_log"), format!("{n}_wood")] {
            b.shapeless(&planks, 4, &[&src]);
        }
        b.shaped(&format!("{n}_wood"), 3, &["##", "##"], &[('#', &format!("{n}_log"))]);
        b.shaped(&format!("{n}_fence"), 3, &["#|#", "#|#"], &[('#', &planks), ('|', "stick")]);
        b.shaped(&format!("{n}_door"), 3, &["##", "##", "##"], &[('#', &planks)]);
        b.shaped(&format!("{n}_trapdoor"), 2, &["###", "###"], &[('#', &planks)]);
    }
    b.shaped("stick", 4, &["#", "#"], &[('#', "#planks")]);
    b.square("crafting_table", 1, "#planks");
    b.shaped("chest", 1, &["###", "# #", "###"], &[('#', "#planks")]);
    b.shaped("furnace", 1, &["###", "# #", "###"], &[('#', "#stone_tool")]);
    b.shaped("torch", 4, &["c", "|"], &[('c', "#coal"), ('|', "stick")]);
    b.shaped("soul_torch", 4, &["c", "|", "s"], &[('c', "#coal"), ('|', "stick"), ('s', "soul_sand")]);
    b.shaped("ladder", 3, &["| |", "|||", "| |"], &[('|', "stick")]);
    b.shaped("bookshelf", 1, &["###", "BBB", "###"], &[('#', "#planks"), ('B', "book")]);
    b.shaped("note_block", 1, &["###", "#r#", "###"], &[('#', "#planks"), ('r', "redstone")]);
    b.shaped("jukebox", 1, &["###", "#d#", "###"], &[('#', "#planks"), ('d', "diamond")]);
    b.shaped("bowl", 4, &["# #", " # "], &[('#', "#planks")]);
    // ------------------------------------------------------------ tools
    for (prefix, material) in [("wooden", "#planks"), ("stone", "#stone_tool"), ("iron", "iron_ingot"), ("golden", "gold_ingot"), ("diamond", "diamond")] {
        let keys = [('#', material), ('|', "stick")];
        b.shaped(&format!("{prefix}_pickaxe"), 1, &["###", " | ", " | "], &keys);
        b.shaped(&format!("{prefix}_axe"), 1, &["##", "#|", " |"], &keys);
        b.shaped(&format!("{prefix}_shovel"), 1, &["#", "|", "|"], &keys);
        b.shaped(&format!("{prefix}_hoe"), 1, &["##", " |", " |"], &keys);
        b.shaped(&format!("{prefix}_sword"), 1, &["#", "#", "|"], &keys);
    }
    b.shaped("shears", 1, &[" #", "# "], &[('#', "iron_ingot")]);
    b.shapeless("flint_and_steel", 1, &["iron_ingot", "flint"]);
    b.shaped("bucket", 1, &["# #", " # "], &[('#', "iron_ingot")]);
    // ------------------------------------------------------------ food and materials
    b.shaped("bread", 1, &["###"], &[('#', "wheat")]);
    b.shaped("cookie", 8, &["#c#"], &[('#', "wheat"), ('c', "brown_dye")]);
    b.shaped("paper", 3, &["###"], &[('#', "sugar_cane")]);
    b.shapeless("sugar", 1, &["sugar_cane"]);
    b.shapeless("book", 1, &["paper", "paper", "paper", "leather"]);
    b.shapeless("bone_meal", 3, &["bone"]);
    b.shaped("golden_apple", 1, &["###", "#a#", "###"], &[('#', "gold_ingot"), ('a', "apple")]);
    for (block, item) in [("coal_block", "coal"), ("iron_block", "iron_ingot"), ("copper_block", "copper_ingot"), ("gold_block", "gold_ingot"), ("redstone_block", "redstone"), ("lapis_block", "lapis_lazuli"), ("diamond_block", "diamond"), ("emerald_block", "emerald"), ("raw_iron_block", "raw_iron"), ("raw_copper_block", "raw_copper"), ("raw_gold_block", "raw_gold"), ("iron_ingot", "iron_nugget"), ("gold_ingot", "gold_nugget"), ("hay_block", "wheat"), ("melon", "melon_slice")] {
        b.compress(block, item);
    }
    // ------------------------------------------------------------ building blocks
    for (out, count, input) in [
        ("snow_block", 1, "snowball"), ("clay", 1, "clay_ball"), ("bricks", 1, "brick"), ("nether_bricks", 1, "nether_brick"), ("glowstone", 1, "glowstone_dust"),
        ("sandstone", 1, "sand"), ("red_sandstone", 1, "red_sand"), ("stone_bricks", 4, "stone"), ("polished_granite", 4, "granite"), ("polished_diorite", 4, "diorite"),
        ("polished_andesite", 4, "andesite"), ("polished_deepslate", 4, "cobbled_deepslate"), ("deepslate_bricks", 4, "polished_deepslate"), ("deepslate_tiles", 4, "deepslate_bricks"),
        ("polished_tuff", 4, "tuff"), ("tuff_bricks", 4, "polished_tuff"), ("polished_blackstone", 4, "blackstone"), ("polished_blackstone_bricks", 4, "polished_blackstone"),
        ("quartz_block", 1, "quartz"), ("quartz_bricks", 4, "quartz_block"), ("mud_bricks", 4, "packed_mud"), ("end_stone_bricks", 4, "end_stone"), ("cut_sandstone", 4, "sandstone"),
        ("cut_red_sandstone", 4, "red_sandstone"), ("prismarine_bricks", 1, "prismarine"), ("iron_trapdoor", 1, "iron_ingot"), ("amethyst_block", 1, "amethyst_shard"),
        // 26.2: four spikes make sulfur; polished and brick variants follow the usual 2x2 pattern.
        ("sulfur", 1, "sulfur_spike"), ("polished_sulfur", 4, "sulfur"), ("sulfur_bricks", 4, "polished_sulfur"), ("polished_cinnabar", 4, "cinnabar"), ("cinnabar_bricks", 4, "polished_cinnabar"),
    ] {
        b.square(out, count, input);
    }
    b.shaped("potent_sulfur", 1, &["###", "###", "###"], &[('#', "sulfur")]);
    for base in ["sulfur", "cinnabar", "stone_brick", "deepslate", "tuff"] {
        let slab = if base == "deepslate" { "cobbled_deepslate_slab".to_string() } else { format!("{base}_slab") };
        let out = if base == "stone_brick" { "chiseled_stone_bricks".to_string() } else { format!("chiseled_{base}") };
        b.shaped(&out, 1, &["#", "#"], &[('#', &slab)]);
    }
    b.shaped("chiseled_sandstone", 1, &["#", "#"], &[('#', "sandstone_slab")]);
    b.shaped("quartz_pillar", 2, &["#", "#"], &[('#', "quartz_block")]);
    b.shapeless("mossy_cobblestone", 1, &["cobblestone", "vine"]);
    b.shapeless("mossy_stone_bricks", 1, &["stone_bricks", "vine"]);
    b.shaped("coarse_dirt", 4, &["dg", "gd"], &[('d', "dirt"), ('g', "gravel")]);
    b.shapeless("packed_mud", 1, &["mud", "wheat"]);
    b.shaped("glass_pane", 16, &["###", "###"], &[('#', "glass")]);
    b.shaped("iron_bars", 16, &["###", "###"], &[('#', "iron_ingot")]);
    b.shaped("iron_door", 3, &["##", "##", "##"], &[('#', "iron_ingot")]);
    b.shaped("tnt", 1, &["gsg", "sgs", "gsg"], &[('g', "gunpowder"), ('s', "#sand")]);
    b.shaped("jack_o_lantern", 1, &["p", "t"], &[('p', "carved_pumpkin"), ('t', "torch")]);
    b.shapeless("pumpkin_seeds", 4, &["pumpkin"]);
    b.shapeless("melon_seeds", 1, &["melon_slice"]);
    b.shaped("moss_carpet", 3, &["##"], &[('#', "moss_block")]);
    b.shaped("honeycomb_block", 1, &["##", "##"], &[('#', "slime_ball")]);
    // Stairs, slabs and walls for every block family.
    let names: Vec<&'static str> = r.blocks.iter().map(|b| b.name).collect();
    for name in &names {
        let (stem, kind) = if let Some(s) = name.strip_suffix("_stairs") {
            (s, 0)
        } else if let Some(s) = name.strip_suffix("_slab") {
            (s, 1)
        } else if let Some(s) = name.strip_suffix("_wall") {
            (s, 2)
        } else {
            continue;
        };
        let candidates = [stem.to_string(), format!("{stem}s"), format!("{stem}_planks"), format!("{stem}_block")];
        let Some(base) = candidates.iter().find(|c| r.items.id(c).is_some() && r.block_named(c).is_some()) else { continue };
        match kind {
            0 => b.shaped(name, 4, &["#  ", "## ", "###"], &[('#', base)]),
            1 => b.shaped(name, 6, &["###"], &[('#', base)]),
            _ => b.shaped(name, 6, &["###", "###"], &[('#', base)]),
        }
    }
    // ------------------------------------------------------------ colours
    for (c, _) in DYES.iter() {
        let dye = format!("{c}_dye");
        b.shapeless(&format!("{c}_wool"), 1, &["#wool", &dye]);
        b.shaped(&format!("{c}_carpet"), 3, &["##"], &[('#', &format!("{c}_wool"))]);
        b.shapeless(&format!("{c}_concrete_powder"), 8, &[&dye, "#sand", "#sand", "#sand", "#sand", "gravel", "gravel", "gravel", "gravel"]);
        b.shaped(&format!("{c}_terracotta"), 8, &["###", "#d#", "###"], &[('#', "terracotta"), ('d', &dye)]);
        b.shaped(&format!("{c}_stained_glass"), 8, &["###", "#d#", "###"], &[('#', "glass"), ('d', &dye)]);
    }
    b.shaped("white_wool", 1, &["##", "##"], &[('#', "string")]);
    for (flower, dye) in [
        ("dandelion", "yellow"), ("sunflower", "yellow"), ("poppy", "red"), ("red_tulip", "red"), ("beetroot", "red"), ("blue_orchid", "light_blue"), ("allium", "magenta"),
        ("azure_bluet", "light_gray"), ("oxeye_daisy", "light_gray"), ("white_tulip", "light_gray"), ("orange_tulip", "orange"), ("torchflower", "orange"), ("pink_tulip", "pink"),
        ("cornflower", "blue"), ("lapis_lazuli", "blue"), ("lily_of_the_valley", "white"), ("bone_meal", "white"), ("coal", "black"), ("charcoal", "black"),
    ] {
        b.shapeless(&format!("{dye}_dye"), 1, &[flower]);
    }
    for (a, c, out) in [("red", "yellow", "orange"), ("blue", "green", "cyan"), ("red", "blue", "purple"), ("red", "white", "pink"), ("green", "white", "lime"), ("black", "white", "gray"), ("blue", "white", "light_blue")] {
        b.shapeless(&format!("{out}_dye"), 2, &[&format!("{a}_dye"), &format!("{c}_dye")]);
    }
    // ------------------------------------------------------------ smelting
    for (input, out) in [
        ("raw_iron", "iron_ingot"), ("raw_copper", "copper_ingot"), ("raw_gold", "gold_ingot"), ("iron_ore", "iron_ingot"), ("deepslate_iron_ore", "iron_ingot"),
        ("copper_ore", "copper_ingot"), ("deepslate_copper_ore", "copper_ingot"), ("gold_ore", "gold_ingot"), ("deepslate_gold_ore", "gold_ingot"), ("nether_gold_ore", "gold_ingot"),
        ("coal_ore", "coal"), ("deepslate_coal_ore", "coal"), ("diamond_ore", "diamond"), ("deepslate_diamond_ore", "diamond"), ("emerald_ore", "emerald"),
        ("deepslate_emerald_ore", "emerald"), ("lapis_ore", "lapis_lazuli"), ("deepslate_lapis_ore", "lapis_lazuli"), ("redstone_ore", "redstone"), ("deepslate_redstone_ore", "redstone"),
        ("nether_quartz_ore", "quartz"), ("#sand", "glass"), ("cobblestone", "stone"), ("stone", "smooth_stone"), ("cobbled_deepslate", "deepslate"), ("#logs", "charcoal"),
        ("clay_ball", "brick"), ("clay", "terracotta"), ("netherrack", "nether_brick"), ("cactus", "green_dye"), ("stone_bricks", "cracked_stone_bricks"),
        ("deepslate_bricks", "cracked_deepslate_bricks"), ("sandstone", "smooth_sandstone"), ("red_sandstone", "smooth_red_sandstone"), ("basalt", "smooth_basalt"),
        ("porkchop", "cooked_porkchop"), ("beef", "cooked_beef"), ("mutton", "cooked_mutton"), ("chicken", "cooked_chicken"), ("rabbit", "cooked_rabbit"), ("cod", "cooked_cod"),
        ("salmon", "cooked_salmon"), ("potato", "baked_potato"),
    ] {
        b.smelt(input, out);
    }
    Recipes { crafting: b.list, smelting: b.smelting }
}

static RECIPES: OnceLock<Recipes> = OnceLock::new();

pub fn recipes() -> &'static Recipes {
    RECIPES.get_or_init(build)
}

/// Finds the result for a square crafting grid of side `size` (2 or 3), row-major.
pub fn craft(grid: &[ItemStack], size: usize) -> Option<ItemStack> {
    let id = |x: usize, y: usize| grid[y * size + x].id;
    let filled: Vec<(usize, usize)> = (0..size * size).filter(|i| !grid[*i].is_empty()).map(|i| (i % size, i / size)).collect();
    if filled.is_empty() {
        return None;
    }
    let (x0, x1) = (filled.iter().map(|p| p.0).min().unwrap(), filled.iter().map(|p| p.0).max().unwrap());
    let (y0, y1) = (filled.iter().map(|p| p.1).min().unwrap(), filled.iter().map(|p| p.1).max().unwrap());
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    'recipes: for rec in &recipes().crafting {
        if rec.shapeless {
            if rec.cells.len() != filled.len() {
                continue;
            }
            let mut used = vec![false; rec.cells.len()];
            for &(x, y) in &filled {
                match (0..rec.cells.len()).find(|i| !used[*i] && rec.cells[*i].matches(id(x, y))) {
                    Some(i) => used[i] = true,
                    None => continue 'recipes,
                }
            }
            return Some(rec.out);
        }
        if rec.width != w || rec.height != h {
            continue;
        }
        for mirror in [false, true] {
            let ok = (0..h).all(|y| (0..w).all(|x| rec.cells[y * w + if mirror { w - 1 - x } else { x }].matches(id(x0 + x, y0 + y))));
            if ok {
                return Some(rec.out);
            }
        }
    }
    None
}

/// Smelting result for an input item.
pub fn smelt(input: ItemId) -> Option<ItemStack> {
    recipes().smelting.iter().find(|(i, _)| i.matches(input)).map(|(_, out)| *out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid3(rows: [[&str; 3]; 3]) -> Vec<ItemStack> {
        rows.iter().flat_map(|r| r.iter().map(|n| if n.is_empty() { ItemStack::EMPTY } else { ItemStack::named(n, 1) })).collect()
    }
    fn named(n: &str, c: u8) -> Option<ItemStack> {
        Some(ItemStack::named(n, c))
    }

    #[test]
    fn basic_recipes() {
        assert!(recipes().crafting.len() > 400, "{} recipes", recipes().crafting.len());
        assert_eq!(craft(&grid3([["oak_log", "", ""], ["", "", ""], ["", "", ""]]), 3), named("oak_planks", 4));
        assert_eq!(craft(&grid3([["", "", ""], ["", "birch_planks", ""], ["", "oak_planks", ""]]), 3), named("stick", 4));
        assert_eq!(craft(&grid3([["oak_planks", "oak_planks", ""], ["oak_planks", "oak_planks", ""], ["", "", ""]]), 3), named("crafting_table", 1));
        let pick = [["cobblestone", "cobblestone", "cobblestone"], ["", "stick", ""], ["", "stick", ""]];
        assert_eq!(craft(&grid3(pick), 3), named("stone_pickaxe", 1));
        // Mirrored axe, and position inside the grid does not matter.
        assert_eq!(craft(&grid3([["", "iron_ingot", "iron_ingot"], ["", "stick", "iron_ingot"], ["", "stick", ""]]), 3), named("iron_axe", 1));
        assert_eq!(craft(&grid3([["", "", ""], ["", "coal", ""], ["", "stick", ""]]), 3), named("torch", 4));
        assert_eq!(craft(&grid3([["", "", ""], ["", "stick", ""], ["", "coal", ""]]), 3), None);
        assert_eq!(craft(&grid3([["oak_planks", "", ""], ["oak_planks", "oak_planks", ""], ["oak_planks", "oak_planks", "oak_planks"]]), 3), named("oak_stairs", 4));
        assert_eq!(craft(&grid3([["", "", ""], ["", "", ""], ["stone_bricks", "stone_bricks", "stone_bricks"]]), 3), named("stone_brick_slab", 6));
        assert_eq!(craft(&grid3([["red_dye", "white_wool", ""], ["", "", ""], ["", "", ""]]), 3), named("red_wool", 1));
        assert_eq!(craft(&grid3([["", "", ""], ["", "", ""], ["", "", ""]]), 3), None);
        // 2x2 grid.
        let g: Vec<ItemStack> = ["oak_planks", "oak_planks", "oak_planks", "oak_planks"].iter().map(|n| ItemStack::named(n, 1)).collect();
        assert_eq!(craft(&g, 2), named("crafting_table", 1));
    }

    #[test]
    fn chaos_cubed_recipes_and_smelting() {
        let spikes = [["sulfur_spike", "sulfur_spike", ""], ["sulfur_spike", "sulfur_spike", ""], ["", "", ""]];
        assert_eq!(craft(&grid3(spikes), 3), named("sulfur", 1));
        assert_eq!(craft(&grid3([["sulfur"; 3]; 3]), 3), named("potent_sulfur", 1));
        assert_eq!(craft(&grid3([["cinnabar_bricks", "", ""], ["cinnabar_bricks", "cinnabar_bricks", ""], ["cinnabar_bricks"; 3]]), 3), named("cinnabar_brick_stairs", 4));
        let id = |n: &str| reg().items.id(n).unwrap();
        assert_eq!(smelt(id("raw_iron")), named("iron_ingot", 1));
        assert_eq!(smelt(id("sand")), named("glass", 1));
        assert_eq!(smelt(id("spruce_log")), named("charcoal", 1));
        assert_eq!(smelt(id("beef")), named("cooked_beef", 1));
        assert_eq!(smelt(id("dirt")), None);
    }
}
