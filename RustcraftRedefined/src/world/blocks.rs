//! The block list.  Names and gameplay numbers follow the vanilla game where they are public
//! facts (identifiers, hardness, blast resistance, light levels); all art is procedural.

use super::block::*;
use super::model::Shape;
use crate::art::palette::{DYES, FLOWERS, WOODS};

fn cube(name: &str) -> BlockDef {
    BlockDef::new(name, Shape::Cube)
}

/// Name stem used for derived blocks: `stone_bricks` -> `stone_brick`, `oak_planks` -> `oak`.
fn stem(base: &str) -> String {
    if let Some(s) = base.strip_suffix("_planks") {
        s.to_string()
    } else if let Some(s) = base.strip_suffix("bricks") {
        format!("{s}brick")
    } else if let Some(s) = base.strip_suffix("tiles") {
        format!("{s}tile")
    } else if base == "quartz_block" || base == "purpur_block" {
        base.trim_end_matches("_block").to_string()
    } else {
        base.to_string()
    }
}

/// Adds stairs and a slab (and optionally a wall) made from an existing block.
fn family(r: &mut Registry, base: &str, wall: bool) {
    let src = r.block_named(base).unwrap_or_else(|| panic!("family base {base} missing")).clone();
    let s = stem(base);
    let derive = |name: String, shape: Shape| {
        let mut d = BlockDef::new(&name, shape);
        d.tex = src.tex.clone();
        d.hardness = src.hardness;
        d.resistance = src.resistance;
        d.tool = src.tool;
        d.tier = src.tier;
        d.needs_tool = src.needs_tool;
        d.sound = src.sound;
        d.flags = src.flags & F_FLAMMABLE;
        d
    };
    r.add(derive(format!("{s}_stairs"), Shape::Stairs));
    r.add(derive(format!("{s}_slab"), Shape::Slab));
    if wall {
        r.add(derive(format!("{s}_wall"), Shape::Wall));
    }
}

pub fn register_all(r: &mut Registry) {
    use Sound::*;
    r.add(BlockDef::new("air", Shape::Air).hard(0.0, 0.0).flag(F_REPLACEABLE).drops(Drops::Nothing).no_item().clear().tex(&[]));

    // ------------------------------------------------------------ stone and minerals
    r.add(cube("stone").hard(1.5, 6.0).pick(1).drop_item("cobblestone", 1, 1));
    for n in ["granite", "polished_granite", "diorite", "polished_diorite", "andesite", "polished_andesite"] {
        r.add(cube(n).hard(1.5, 6.0).pick(1));
    }
    r.add(BlockDef::new("deepslate", Shape::Pillar).tex(&["deepslate_top", "deepslate"]).hard(3.0, 6.0).pick(1).drop_item("cobbled_deepslate", 1, 1));
    for n in ["cobbled_deepslate", "polished_deepslate", "deepslate_bricks", "cracked_deepslate_bricks", "deepslate_tiles", "chiseled_deepslate"] {
        r.add(cube(n).hard(3.5, 6.0).pick(1));
    }
    r.add(cube("tuff").hard(1.5, 6.0).pick(1));
    for n in ["polished_tuff", "tuff_bricks", "chiseled_tuff"] {
        r.add(cube(n).hard(1.5, 6.0).pick(1));
    }
    r.add(cube("calcite").hard(0.75, 0.75).pick(1));
    r.add(cube("dripstone_block").hard(1.5, 1.0).pick(1));
    r.add(BlockDef::new("pointed_dripstone", Shape::Spike).hard(1.5, 3.0).tool(Tool::Pickaxe).clear());
    r.add(cube("cobblestone").hard(2.0, 6.0).pick(1));
    r.add(cube("mossy_cobblestone").hard(2.0, 6.0).pick(1));
    r.add(cube("smooth_stone").hard(2.0, 6.0).pick(1));
    for n in ["stone_bricks", "mossy_stone_bricks", "cracked_stone_bricks", "chiseled_stone_bricks"] {
        r.add(cube(n).hard(1.5, 6.0).pick(1));
    }
    r.add(cube("bricks").hard(2.0, 6.0).pick(1));
    r.add(cube("mud_bricks").hard(1.5, 3.0).pick(1));
    r.add(cube("bedrock").hard(-1.0, 3_600_000.0).drops(Drops::Nothing));
    r.add(cube("obsidian").hard(50.0, 1200.0).pick(4));
    r.add(cube("crying_obsidian").tex(&["obsidian"]).hard(50.0, 1200.0).pick(4).light(10));
    for (n, top) in [("sandstone", "sandstone_top"), ("red_sandstone", "red_sandstone_top")] {
        r.add(cube(n).tsb(top, n, top).hard(0.8, 0.8).pick(1));
        let cut = format!("cut_{n}");
        r.add(cube(&cut).tsb(top, &cut, top).hard(0.8, 0.8).pick(1));
        let chis = format!("chiseled_{n}");
        r.add(cube(&chis).tsb(top, &chis, top).hard(0.8, 0.8).pick(1));
        r.add(cube(&format!("smooth_{n}")).tex(&[top]).hard(2.0, 6.0).pick(1));
    }
    r.add(cube("netherrack").hard(0.4, 0.4).pick(1));
    r.add(cube("nether_bricks").hard(2.0, 6.0).pick(1));
    r.add(cube("red_nether_bricks").hard(2.0, 6.0).pick(1));
    r.add(cube("soul_sand").hard(0.5, 0.5).tool(Tool::Shovel).sound(Sand).slip(0.6));
    r.add(cube("soul_soil").hard(0.5, 0.5).tool(Tool::Shovel).sound(Sand));
    r.add(BlockDef::new("basalt", Shape::Pillar).tex(&["basalt_top", "basalt"]).hard(1.25, 4.2).pick(1));
    r.add(cube("smooth_basalt").hard(1.25, 4.2).pick(1));
    for n in ["blackstone", "polished_blackstone", "polished_blackstone_bricks"] {
        r.add(cube(n).hard(1.5, 6.0).pick(1));
    }
    r.add(cube("end_stone").hard(3.0, 9.0).pick(1));
    r.add(cube("end_stone_bricks").hard(3.0, 9.0).pick(1));
    r.add(cube("purpur_block").hard(1.5, 6.0).pick(1));
    r.add(cube("prismarine").hard(1.5, 6.0).pick(1));
    r.add(cube("prismarine_bricks").hard(1.5, 6.0).pick(1));
    r.add(cube("quartz_block").tsb("quartz_top", "quartz_block", "quartz_top").hard(0.8, 0.8).pick(1));
    r.add(cube("quartz_bricks").hard(0.8, 0.8).pick(1));
    r.add(BlockDef::new("quartz_pillar", Shape::Pillar).tex(&["quartz_pillar_top", "quartz_pillar"]).hard(0.8, 0.8).pick(1));
    r.add(cube("amethyst_block").hard(1.5, 1.5).pick(1).sound(Glass));
    r.add(cube("glowstone").hard(0.3, 0.3).light(15).sound(Glass).drop_item("glowstone_dust", 2, 4));
    r.add(cube("sea_lantern").hard(0.3, 0.3).light(15).sound(Glass));
    r.add(cube("shroomlight").hard(1.0, 1.0).light(15).tool(Tool::Hoe));
    r.add(cube("magma_block").tex(&["magma"]).hard(0.5, 0.5).pick(1).light(3));
    r.add(cube("sculk").hard(0.2, 0.2).tool(Tool::Hoe).drops(Drops::Nothing));
    r.add(cube("moss_block").hard(0.1, 0.1).tool(Tool::Hoe).sound(Grass));
    r.add(cube("clay").hard(0.6, 0.6).tool(Tool::Shovel).sound(Gravel).drop_item("clay_ball", 4, 4));
    r.add(cube("terracotta").hard(1.25, 4.2).pick(1));

    // 26.2 "Chaos Cubed": sulfur and cinnabar.  Hardness values are this project's estimates.
    for base in ["sulfur", "cinnabar"] {
        for n in [base.to_string(), format!("polished_{base}"), format!("{base}_bricks")] {
            r.add(cube(&n).hard(1.5, 6.0).pick(1));
            family(r, &n, true);
        }
        r.add(cube(&format!("chiseled_{base}")).hard(1.5, 6.0).pick(1));
    }
    r.add(cube("potent_sulfur").hard(1.5, 6.0).pick(1));
    r.add(BlockDef::new("sulfur_spike", Shape::Spike).hard(1.5, 3.0).tool(Tool::Pickaxe).clear());

    // ------------------------------------------------------------ ores and metal blocks
    for (ore, tier, item, min, max) in [
        ("coal", 1, "coal", 1, 1),
        ("iron", 2, "raw_iron", 1, 1),
        ("copper", 2, "raw_copper", 2, 5),
        ("gold", 3, "raw_gold", 1, 1),
        ("redstone", 3, "redstone", 4, 5),
        ("lapis", 2, "lapis_lazuli", 4, 9),
        ("diamond", 3, "diamond", 1, 1),
        ("emerald", 3, "emerald", 1, 1),
    ] {
        r.add(cube(&format!("{ore}_ore")).hard(3.0, 3.0).pick(tier).drop_item(item, min, max));
        r.add(cube(&format!("deepslate_{ore}_ore")).hard(4.5, 3.0).pick(tier).drop_item(item, min, max));
    }
    r.add(cube("nether_quartz_ore").hard(3.0, 3.0).pick(1).drop_item("quartz", 1, 1));
    r.add(cube("nether_gold_ore").hard(3.0, 3.0).pick(1).drop_item("gold_nugget", 2, 6));
    r.add(BlockDef::new("ancient_debris", Shape::Pillar).tex(&["ancient_debris_top", "ancient_debris_side"]).hard(30.0, 1200.0).pick(4));
    for (n, tier, h) in [
        ("coal_block", 1, 5.0),
        ("iron_block", 2, 5.0),
        ("copper_block", 2, 3.0),
        ("gold_block", 3, 3.0),
        ("redstone_block", 1, 5.0),
        ("lapis_block", 2, 3.0),
        ("diamond_block", 3, 5.0),
        ("emerald_block", 3, 5.0),
        ("netherite_block", 4, 50.0),
        ("raw_iron_block", 2, 5.0),
        ("raw_copper_block", 2, 5.0),
        ("raw_gold_block", 3, 5.0),
    ] {
        r.add(cube(n).hard(h, if n == "netherite_block" { 1200.0 } else { 6.0 }).pick(tier).sound(Metal));
    }

    // ------------------------------------------------------------ soil and loose blocks
    r.add(BlockDef::new("grass_block", Shape::Grass)
        .tex(&["dirt", "grass_block_top", "grass_block_side", "grass_block_side", "grass_block_side", "grass_block_side", "grass_block_side_overlay", "grass_block_snow"])
        .hard(0.6, 0.6).tool(Tool::Shovel).sound(Grass).tint(TINT_GRASS).drop_item("dirt", 1, 1));
    r.add(cube("dirt").hard(0.5, 0.5).tool(Tool::Shovel).sound(Gravel));
    r.add(cube("coarse_dirt").hard(0.5, 0.5).tool(Tool::Shovel).sound(Gravel));
    r.add(cube("rooted_dirt").hard(0.5, 0.5).tool(Tool::Shovel).sound(Gravel));
    r.add(cube("podzol").tsb("podzol_top", "podzol_side", "dirt").hard(0.5, 0.5).tool(Tool::Shovel).sound(Gravel).drop_item("dirt", 1, 1));
    r.add(cube("mycelium").tsb("mycelium_top", "mycelium_side", "dirt").hard(0.6, 0.6).tool(Tool::Shovel).sound(Grass).drop_item("dirt", 1, 1));
    r.add(cube("mud").hard(0.5, 0.5).tool(Tool::Shovel).sound(Wet));
    r.add(BlockDef::new("farmland", Shape::Farmland).tex(&["dirt", "farmland", "farmland_moist"]).hard(0.6, 0.6).tool(Tool::Shovel).sound(Gravel).drop_item("dirt", 1, 1));
    r.add(BlockDef::new("dirt_path", Shape::Low(15)).tsb("dirt_path_top", "dirt_path_side", "dirt").hard(0.65, 0.65).tool(Tool::Shovel).sound(Grass).drop_item("dirt", 1, 1));
    r.add(cube("sand").hard(0.5, 0.5).tool(Tool::Shovel).sound(Sand).flag(F_GRAVITY));
    r.add(cube("red_sand").hard(0.5, 0.5).tool(Tool::Shovel).sound(Sand).flag(F_GRAVITY));
    r.add(cube("gravel").hard(0.6, 0.6).tool(Tool::Shovel).sound(Gravel).flag(F_GRAVITY));
    r.add(cube("snow_block").tex(&["snow"]).hard(0.2, 0.2).tool(Tool::Shovel).needs_tool().sound(Snow).drop_item("snowball", 4, 4));
    r.add(BlockDef::new("snow", Shape::Layers).hard(0.1, 0.1).tool(Tool::Shovel).needs_tool().sound(Snow).clear().flag(F_REPLACEABLE).drop_item("snowball", 1, 1));
    r.add(cube("ice").hard(0.5, 0.5).tool(Tool::Pickaxe).sound(Glass).translucent().cull_same().slip(0.98).drops(Drops::Nothing));
    r.add(cube("packed_ice").hard(0.5, 0.5).tool(Tool::Pickaxe).sound(Glass).slip(0.98));
    r.add(cube("blue_ice").hard(2.8, 2.8).tool(Tool::Pickaxe).sound(Glass).slip(0.989));

    // ------------------------------------------------------------ fluids
    r.add(BlockDef::new("water", Shape::Fluid).tex(&["water_still"]).hard(100.0, 100.0).translucent().tint(TINT_WATER).drops(Drops::Nothing).no_item().sound(Wet));
    r.add(BlockDef::new("lava", Shape::Fluid).tex(&["lava_still"]).hard(100.0, 100.0).clear().flag(F_LAVA).light(15).drops(Drops::Nothing).no_item().sound(Wet));

    // ------------------------------------------------------------ wood
    for w in WOODS.iter() {
        let n = w.name;
        let wood = |d: BlockDef| d.hard(2.0, 2.0).tool(Tool::Axe).sound(Wood).flag(F_FLAMMABLE);
        r.add(wood(BlockDef::new(&format!("{n}_log"), Shape::Pillar).tex(&[&format!("{n}_log_top"), &format!("{n}_log")])));
        r.add(wood(BlockDef::new(&format!("stripped_{n}_log"), Shape::Pillar).tex(&[&format!("stripped_{n}_log_top"), &format!("stripped_{n}_log")])));
        r.add(wood(BlockDef::new(&format!("{n}_wood"), Shape::Pillar).tex(&[&format!("{n}_log"), &format!("{n}_log")])));
        r.add(wood(cube(&format!("{n}_planks"))).hard(2.0, 3.0));
        family(r, &format!("{n}_planks"), false);
        let mut leaves = cube(&format!("{n}_leaves")).hard(0.2, 0.2).tool(Tool::Shears).sound(Grass).clear().flag(F_FLAMMABLE | F_LEAVES).drops(Drops::Shears);
        if w.leaves.is_none() {
            leaves = leaves.tint(TINT_FOLIAGE);
        }
        r.add(leaves);
        r.add(BlockDef::new(&format!("{n}_sapling"), Shape::Cross).plant());
        r.add(wood(BlockDef::new(&format!("{n}_fence"), Shape::Fence).tex(&[&format!("{n}_planks")])).hard(2.0, 3.0).clear());
        r.add(wood(BlockDef::new(&format!("{n}_door"), Shape::Door).tex(&[&format!("{n}_door_bottom"), &format!("{n}_door_top")])).hard(3.0, 3.0).clear());
        r.add(wood(BlockDef::new(&format!("{n}_trapdoor"), Shape::Trapdoor)).hard(3.0, 3.0).clear());
    }

    // ------------------------------------------------------------ derived stairs, slabs, walls
    for (base, wall) in [
        ("stone", false), ("cobblestone", true), ("mossy_cobblestone", true), ("stone_bricks", true), ("mossy_stone_bricks", true),
        ("granite", true), ("polished_granite", false), ("diorite", true), ("polished_diorite", false), ("andesite", true),
        ("polished_andesite", false), ("cobbled_deepslate", true), ("polished_deepslate", true), ("deepslate_bricks", true),
        ("deepslate_tiles", true), ("tuff", true), ("polished_tuff", true), ("tuff_bricks", true), ("bricks", true), ("mud_bricks", true),
        ("sandstone", true), ("red_sandstone", true), ("nether_bricks", true), ("blackstone", true), ("polished_blackstone", true),
        ("end_stone_bricks", true), ("purpur_block", false), ("prismarine", true), ("quartz_block", false), ("smooth_stone", false),
    ] {
        family(r, base, wall);
    }

    // ------------------------------------------------------------ coloured blocks
    for (c, _) in DYES.iter() {
        r.add(cube(&format!("{c}_wool")).hard(0.8, 0.8).tool(Tool::Shears).sound(Wool).flag(F_FLAMMABLE));
        r.add(BlockDef::new(&format!("{c}_carpet"), Shape::Low(1)).tex(&[&format!("{c}_wool")]).hard(0.1, 0.1).sound(Wool).flag(F_FLAMMABLE).clear());
        r.add(cube(&format!("{c}_concrete")).hard(1.8, 1.8).pick(1));
        r.add(cube(&format!("{c}_concrete_powder")).hard(0.5, 0.5).tool(Tool::Shovel).sound(Sand).flag(F_GRAVITY));
        r.add(cube(&format!("{c}_terracotta")).hard(1.25, 4.2).pick(1));
        r.add(cube(&format!("{c}_stained_glass")).hard(0.3, 0.3).sound(Glass).translucent().cull_same().drops(Drops::Nothing));
    }
    r.add(cube("glass").hard(0.3, 0.3).sound(Glass).clear().cull_same().drops(Drops::Nothing));
    r.add(cube("tinted_glass").hard(0.3, 0.3).sound(Glass).translucent().cull_same());
    r.add(BlockDef::new("glass_pane", Shape::Pane).tex(&["glass"]).hard(0.3, 0.3).sound(Glass).clear().drops(Drops::Nothing));
    r.add(BlockDef::new("iron_bars", Shape::Pane).hard(5.0, 6.0).pick(1).sound(Metal).clear());

    // ------------------------------------------------------------ plants
    for n in ["short_grass", "fern"] {
        r.add(BlockDef::new(n, Shape::Cross).plant().tint(TINT_GRASS).flag(F_REPLACEABLE | F_FLAMMABLE).drops(Drops::Shears));
    }
    r.add(BlockDef::new("tall_grass", Shape::DoublePlant).tex(&["tall_grass_bottom", "tall_grass_top"]).plant().tint(TINT_GRASS).flag(F_REPLACEABLE | F_FLAMMABLE).drops(Drops::Shears));
    r.add(BlockDef::new("sunflower", Shape::DoublePlant).tex(&["sunflower_bottom", "sunflower_top"]).plant());
    r.add(BlockDef::new("dead_bush", Shape::Cross).plant().flag(F_REPLACEABLE | F_FLAMMABLE).drop_item("stick", 0, 2));
    for f in FLOWERS.iter() {
        r.add(BlockDef::new(f.0, Shape::Cross).plant());
    }
    r.add(BlockDef::new("brown_mushroom", Shape::Cross).plant().light(1));
    r.add(BlockDef::new("red_mushroom", Shape::Cross).plant());
    r.add(BlockDef::new("sugar_cane", Shape::Cross).plant());
    r.add(BlockDef::new("seagrass", Shape::Cross).plant().flag(F_REPLACEABLE | F_WATER).drops(Drops::Shears));
    r.add(BlockDef::new("kelp", Shape::Cross).plant().flag(F_WATER));
    r.add(BlockDef::new("cactus", Shape::Cactus).tex(&["cactus_bottom", "cactus_top", "cactus_side", "cactus_side", "cactus_side", "cactus_side"]).hard(0.4, 0.4).sound(Wool).clear());
    r.add(BlockDef::new("lily_pad", Shape::LilyPad).hard(0.0, 0.0).sound(Grass).clear().tint(TINT_FOLIAGE));
    r.add(BlockDef::new("vine", Shape::Ladder).hard(0.2, 0.2).tool(Tool::Shears).sound(Grass).clear().tint(TINT_FOLIAGE).flag(F_CLIMBABLE | F_FLAMMABLE | F_REPLACEABLE).drops(Drops::Shears));
    r.add(BlockDef::new("glow_lichen", Shape::Ladder).hard(0.2, 0.2).tool(Tool::Shears).sound(Grass).clear().light(7).flag(F_REPLACEABLE).drops(Drops::Shears));
    r.add(BlockDef::new("cave_vines", Shape::Cross).plant().light(14).drop_item("glow_berries", 1, 1));
    for n in ["mushroom_stem", "brown_mushroom_block", "red_mushroom_block"] {
        r.add(cube(n).hard(0.2, 0.2).tool(Tool::Axe).sound(Wood).drops(Drops::Nothing));
    }
    r.add(cube("packed_mud").hard(1.0, 3.0).sound(Gravel));
    r.add(BlockDef::new("moss_carpet", Shape::Low(1)).tex(&["moss_block"]).hard(0.1, 0.1).sound(Grass).clear());
    r.add(BlockDef::new("wheat", Shape::Crop(8)).tex(&["wheat_stage0", "wheat_stage1", "wheat_stage2", "wheat_stage3", "wheat_stage4", "wheat_stage5", "wheat_stage6", "wheat_stage7"]).plant().no_item().drop_item("wheat_seeds", 1, 1));
    for n in ["carrots", "potatoes", "beetroots"] {
        let names: Vec<String> = (0..4).map(|i| format!("{n}_stage{i}")).collect();
        let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
        r.add(BlockDef::new(n, Shape::Crop(4)).tex(&refs).plant().no_item().drops(Drops::Nothing));
    }
    r.add(cube("pumpkin").tsb("pumpkin_top", "pumpkin_side", "pumpkin_top").hard(1.0, 1.0).tool(Tool::Axe).sound(Wood));
    r.add(BlockDef::new("carved_pumpkin", Shape::Facing).tex(&["pumpkin_top", "pumpkin_side", "carved_pumpkin"]).hard(1.0, 1.0).tool(Tool::Axe).sound(Wood));
    r.add(BlockDef::new("jack_o_lantern", Shape::Facing).tex(&["pumpkin_top", "pumpkin_side", "jack_o_lantern"]).hard(1.0, 1.0).tool(Tool::Axe).sound(Wood).light(15));
    r.add(cube("melon").tsb("melon_top", "melon_side", "melon_top").hard(1.0, 1.0).tool(Tool::Axe).sound(Wood).drop_item("melon_slice", 3, 7));
    r.add(BlockDef::new("hay_block", Shape::Pillar).tex(&["hay_block_top", "hay_block_side"]).hard(0.5, 0.5).tool(Tool::Hoe).sound(Grass).flag(F_FLAMMABLE));

    // ------------------------------------------------------------ utility
    r.add(cube("crafting_table").tex(&["oak_planks", "crafting_table_top", "crafting_table_front", "crafting_table_side", "crafting_table_side", "crafting_table_front"]).hard(2.5, 2.5).tool(Tool::Axe).sound(Wood).flag(F_FLAMMABLE));
    r.add(BlockDef::new("furnace", Shape::FacingLit).tex(&["furnace_top", "furnace_side", "furnace_front", "furnace_front_on"]).hard(3.5, 3.5).pick(1).light(13));
    r.add(BlockDef::new("chest", Shape::Chest).tex(&["chest_top", "chest_side", "chest_front"]).hard(2.5, 2.5).tool(Tool::Axe).sound(Wood).clear());
    r.add(cube("bookshelf").tsb("oak_planks", "bookshelf", "oak_planks").hard(1.5, 1.5).tool(Tool::Axe).sound(Wood).flag(F_FLAMMABLE).drop_item("book", 3, 3));
    r.add(cube("tnt").tsb("tnt_top", "tnt_side", "tnt_bottom").hard(0.0, 0.0).sound(Grass).flag(F_FLAMMABLE));
    r.add(BlockDef::new("torch", Shape::Torch).hard(0.0, 0.0).sound(Wood).light(14).clear());
    r.add(BlockDef::new("soul_torch", Shape::Torch).hard(0.0, 0.0).sound(Wood).light(10).clear());
    r.add(BlockDef::new("ladder", Shape::Ladder).hard(0.4, 0.4).tool(Tool::Axe).sound(Wood).clear().flag(F_CLIMBABLE));
    r.add(BlockDef::new("iron_door", Shape::Door).tex(&["iron_door_bottom", "iron_door_top"]).hard(5.0, 5.0).pick(1).sound(Metal).clear());
    r.add(BlockDef::new("iron_trapdoor", Shape::Trapdoor).hard(5.0, 5.0).pick(1).sound(Metal).clear());
    r.add(cube("sponge").hard(0.6, 0.6).tool(Tool::Hoe).sound(Grass));
    r.add(cube("slime_block").hard(0.0, 0.0).sound(Wet).translucent().cull_same().slip(0.8));
    r.add(cube("honey_block").hard(0.0, 0.0).sound(Wet).translucent().cull_same());
    r.add(cube("honeycomb_block").hard(0.6, 0.6).sound(Stone));
    r.add(cube("note_block").hard(0.8, 0.8).tool(Tool::Axe).sound(Wood).flag(F_FLAMMABLE));
    r.add(cube("jukebox").tsb("jukebox_top", "jukebox_side", "jukebox_side").hard(2.0, 6.0).tool(Tool::Axe).sound(Wood));
    r.add(cube("spawner").hard(5.0, 5.0).pick(1).sound(Metal).clear().drops(Drops::Nothing));
}

#[cfg(test)]
mod tests {
    use super::super::block::*;
    use crate::art::blocktex;

    #[test]
    fn registry_is_consistent() {
        let r = reg();
        assert_eq!(r.k.air, AIR);
        assert!(r.blocks.len() > 400, "{} blocks", r.blocks.len());
        assert!(r.states.len() < u16::MAX as usize);
        assert_eq!(r.states.len(), r.mesh_states.len());
        for (i, b) in r.blocks.iter().enumerate() {
            assert!(b.count >= 1, "{}", b.name);
            for s in b.first..b.first + b.count {
                assert_eq!(r.data(s).block as usize, i);
            }
        }
        assert!(r.has(r.k.stone, F_OPAQUE | F_SOLID));
        assert!(!r.has(r.k.water, F_SOLID) && r.has(r.k.water, F_WATER | F_FLUID));
        assert!(r.has(r.k.lava, F_LAVA) && r.data(r.k.lava).emit == 15);
        assert_eq!(r.data(r.k.stone).occlude, 0x3f);
        assert_eq!(r.data(r.k.glass).occlude, 0);
        assert_eq!(r.data(r.k.torch).emit, 14);
    }

    #[test]
    fn every_texture_has_a_recipe() {
        let r = reg();
        let missing: Vec<&String> = r.textures.iter().filter(|n| blocktex::generate(n).is_none()).collect();
        assert!(missing.is_empty(), "textures without a recipe: {missing:?}");
    }

    #[test]
    fn state_strings_round_trip() {
        let r = reg();
        let s = r.parse_state("oak_stairs[facing=east,half=top,shape=inner_left]").unwrap();
        assert_eq!(r.describe(s), "oak_stairs[facing=east,half=top,shape=inner_left,waterlogged=false]");
        assert_eq!(r.parse_state(&r.describe(s)), Some(s));
        assert_eq!(r.parse_state("minecraft:stone"), Some(r.k.stone));
        assert!(r.parse_state("stone[foo=bar]").is_none());
        let lit = r.parse_state("furnace[lit=true]").unwrap();
        assert_eq!(r.data(lit).emit, 13);
        assert_eq!(r.data(r.k.furnace).emit, 0);
        let slab = r.parse_state("oak_slab[type=bottom,waterlogged=true]").unwrap();
        assert!(r.has(slab, F_WATER));
        assert_eq!(r.data(r.parse_state("oak_slab[type=double]").unwrap()).occlude, 0x3f);
    }
}
