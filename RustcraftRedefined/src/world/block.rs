//! Block registry: block definitions, their states, and the tables shared with the native mesher.

use super::model::{self, covers_face, Box6, Shape};
use crate::ffi::*;
use std::collections::HashMap;
use std::sync::OnceLock;

pub type State = u16;
pub const AIR: State = 0;
/// Texture layer of the first block-breaking overlay stage.
pub const DESTROY_LAYER: u16 = 4;
/// Texture layers of the smoke puff and droplet particles.
pub const PUFF_LAYER: u16 = DESTROY_LAYER + 10;
pub const DROP_LAYER: u16 = DESTROY_LAYER + 11;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    None,
    Pickaxe,
    Axe,
    Shovel,
    Hoe,
    Shears,
    Sword,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sound {
    Stone,
    Wood,
    Grass,
    Gravel,
    Sand,
    Snow,
    Glass,
    Wool,
    Metal,
    Wet,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drops {
    Itself,
    Nothing,
    /// Item name with a count range.
    Item(&'static str, u8, u8),
    /// Only with the right tool enchant-free rule: drops itself with shears, otherwise nothing.
    Shears,
}

// State flags (Rust side).
pub const F_SOLID: u16 = 1; // has collision boxes
pub const F_OPAQUE: u16 = 2; // full opaque cube
pub const F_REPLACEABLE: u16 = 4; // placing a block here overwrites it
pub const F_WATER: u16 = 8; // water fluid or waterlogged
pub const F_LAVA: u16 = 16;
pub const F_FLUID: u16 = 32; // the block itself is a fluid
pub const F_CLIMBABLE: u16 = 64;
pub const F_PLANT: u16 = 128; // needs soil below, breaks instantly
pub const F_FLAMMABLE: u16 = 256;
pub const F_GRAVITY: u16 = 512; // falls when unsupported
pub const F_STURDY_TOP: u16 = 1024; // can support torches, plants, rails on top
pub const F_LEAVES: u16 = 2048;

/// Tint kinds understood by the mesher.
pub const TINT_NONE: u8 = 0;
pub const TINT_GRASS: u8 = 1;
pub const TINT_FOLIAGE: u8 = 2;
pub const TINT_WATER: u8 = 3;

#[derive(Clone)]
pub struct BlockDef {
    pub name: &'static str,
    pub shape: Shape,
    /// Texture names; their meaning depends on the shape (see `Registry::texset_for`).
    pub tex: Vec<String>,
    pub hardness: f32,
    pub resistance: f32,
    pub tool: Tool,
    /// Minimum tool tier needed for drops: 0 = any, 1 wood, 2 stone, 3 iron, 4 diamond.
    pub tier: u8,
    pub needs_tool: bool,
    pub light: u8,
    pub sound: Sound,
    pub drops: Drops,
    pub opaque: bool,
    pub translucent: bool,
    pub cull_same: bool,
    pub tint: u8,
    pub fixed_tint: u32,
    pub offset: bool,
    pub flags: u16,
    pub slipperiness: f32,
    pub no_item: bool,
    pub first: State,
    pub count: u16,
}

impl BlockDef {
    pub fn new(name: &str, shape: Shape) -> BlockDef {
        let opaque = matches!(shape, Shape::Cube | Shape::Grass | Shape::Pillar | Shape::Facing | Shape::FacingLit | Shape::Slab | Shape::Stairs | Shape::Farmland | Shape::Low(_) | Shape::Wall);
        BlockDef {
            name: leak(name),
            shape,
            tex: vec![name.to_string()],
            hardness: 1.0,
            resistance: 1.0,
            tool: Tool::None,
            tier: 0,
            needs_tool: false,
            light: 0,
            sound: Sound::Stone,
            drops: Drops::Itself,
            opaque,
            translucent: false,
            cull_same: false,
            tint: TINT_NONE,
            fixed_tint: 0,
            offset: false,
            flags: 0,
            slipperiness: 0.6,
            no_item: false,
            first: 0,
            count: 0,
        }
    }
    pub fn tex(mut self, names: &[&str]) -> Self {
        self.tex = names.iter().map(|s| s.to_string()).collect();
        self
    }
    /// Top, side, bottom.
    pub fn tsb(self, top: &str, side: &str, bottom: &str) -> Self {
        self.tex(&[bottom, top, side, side, side, side])
    }
    pub fn hard(mut self, hardness: f32, resistance: f32) -> Self {
        self.hardness = hardness;
        self.resistance = resistance;
        self
    }
    pub fn pick(mut self, tier: u8) -> Self {
        self.tool = Tool::Pickaxe;
        self.needs_tool = true;
        self.tier = tier;
        self
    }
    pub fn tool(mut self, tool: Tool) -> Self {
        self.tool = tool;
        self
    }
    /// Drops nothing unless mined with its preferred kind of tool (any tier).
    pub fn needs_tool(mut self) -> Self {
        self.needs_tool = true;
        self
    }
    pub fn sound(mut self, s: Sound) -> Self {
        self.sound = s;
        self
    }
    pub fn light(mut self, l: u8) -> Self {
        self.light = l;
        self
    }
    pub fn drops(mut self, d: Drops) -> Self {
        self.drops = d;
        self
    }
    pub fn drop_item(self, item: &'static str, min: u8, max: u8) -> Self {
        self.drops(Drops::Item(item, min, max))
    }
    pub fn clear(mut self) -> Self {
        self.opaque = false;
        self
    }
    pub fn translucent(mut self) -> Self {
        self.opaque = false;
        self.translucent = true;
        self
    }
    pub fn cull_same(mut self) -> Self {
        self.cull_same = true;
        self
    }
    pub fn tint(mut self, t: u8) -> Self {
        self.tint = t;
        self
    }
    pub fn fixed_tint(mut self, rgb: u32) -> Self {
        self.fixed_tint = rgb;
        self
    }
    pub fn flag(mut self, f: u16) -> Self {
        self.flags |= f;
        self
    }
    pub fn plant(mut self) -> Self {
        self.hardness = 0.0;
        self.resistance = 0.0;
        self.sound = Sound::Grass;
        self.opaque = false;
        self.offset = matches!(self.shape, Shape::Cross | Shape::DoublePlant);
        self.flags |= F_PLANT;
        self
    }
    pub fn slip(mut self, s: f32) -> Self {
        self.slipperiness = s;
        self
    }
    pub fn no_item(mut self) -> Self {
        self.no_item = true;
        self
    }
}

pub fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

#[derive(Clone, Copy, Default)]
pub struct StateData {
    pub block: u16,
    pub flags: u16,
    pub emit: u8,
    /// Extra light attenuation when light enters this cell (0 for air, 1 for water and leaves).
    pub filter: u8,
    /// Bit d set: the face in direction d blocks light and hides neighbouring faces.
    pub occlude: u8,
    pub box_start: u32,
    pub box_count: u8,
    pub outline_start: u32,
    pub outline_count: u8,
}

macro_rules! known_blocks {
    ($($name:ident),* $(,)?) => {
        /// Default states of blocks the engine refers to directly.
        #[derive(Default)]
        pub struct Known { $(pub $name: State),* }
        impl Known {
            fn resolve(reg: &Registry) -> Known {
                Known { $($name: reg.default_state(stringify!($name)).unwrap_or_else(|| panic!("block {} is not registered", stringify!($name)))),* }
            }
        }
    };
}
known_blocks!(
    air, stone, deepslate, bedrock, water, lava, dirt, grass_block, sand, red_sand, gravel, sandstone, red_sandstone, snow, snow_block, ice,
    packed_ice, clay, coarse_dirt, podzol, mycelium, mud, terracotta, obsidian, cobblestone, mossy_cobblestone, farmland, dirt_path,
    short_grass, fern, dead_bush, tall_grass, cactus, sugar_cane, lily_pad, vine, brown_mushroom, red_mushroom, pumpkin, melon,
    torch, glass, tnt, crafting_table, furnace, chest, moss_block, calcite, tuff, dripstone_block, pointed_dripstone, magma_block,
    sulfur, cinnabar, potent_sulfur, sulfur_spike, glowstone, amethyst_block, sculk, andesite, diorite, granite,
    moss_carpet, cave_vines, seagrass, kelp, sunflower, orange_terracotta, packed_mud,
);

pub struct Registry {
    pub blocks: Vec<BlockDef>,
    pub states: Vec<StateData>,
    pub boxes: Vec<Box6>,
    pub textures: Vec<String>,
    by_name: HashMap<&'static str, u16>,
    tex_index: HashMap<String, u16>,
    // Mesher tables.
    pub mesh_states: Vec<VcStateInfo>,
    pub geoms: Vec<VcGeom>,
    pub quads: Vec<VcQuad>,
    pub texsets: Vec<VcTexSet>,
    pub fixed_tints: Vec<u32>,
    geom_cache: HashMap<(Shape, u16), (u16, u32, u8, u32, u8, u8, bool)>,
    texset_cache: HashMap<VcTexSet, u16>,
    pub k: Known,
    pub items: crate::item::Items,
}

impl Registry {
    fn empty() -> Registry {
        let mut r = Registry {
            blocks: Vec::new(),
            states: Vec::new(),
            boxes: Vec::new(),
            textures: Vec::new(),
            by_name: HashMap::new(),
            tex_index: HashMap::new(),
            mesh_states: Vec::new(),
            geoms: vec![VcGeom::default()],
            quads: Vec::new(),
            texsets: Vec::new(),
            fixed_tints: Vec::new(),
            geom_cache: HashMap::new(),
            texset_cache: HashMap::new(),
            k: Known::default(),
            items: crate::item::Items::default(),
        };
        // Fluids are textured by the mesher itself, so their layers are reserved first.
        for n in ["water_still", "water_flow", "lava_still", "lava_flow"] {
            r.texture(n);
        }
        // Block-breaking overlay stages occupy the next ten layers.
        for i in 0..10 {
            r.texture(&format!("destroy_stage_{i}"));
        }
        r.texture("particle_puff");
        r.texture("particle_drop");
        for name in crate::art::mobtex::NAMES {
            r.texture(&format!("mob/{name}"));
        }
        r
    }

    /// Layer index of a texture name, allocating it on first use.
    pub fn texture(&mut self, name: &str) -> u16 {
        if let Some(&i) = self.tex_index.get(name) {
            return i;
        }
        let i = self.textures.len() as u16;
        self.textures.push(name.to_string());
        self.tex_index.insert(name.to_string(), i);
        i
    }

    pub fn texture_index(&self, name: &str) -> Option<u16> {
        self.tex_index.get(name).copied()
    }

    fn texset(&mut self, names: [&str; 8]) -> u16 {
        let mut set = VcTexSet::default();
        for (i, n) in names.iter().enumerate() {
            set.layer[i] = if n.is_empty() { 0 } else { self.texture(n) };
        }
        if let Some(&i) = self.texset_cache.get(&set) {
            return i;
        }
        let i = self.texsets.len() as u16;
        self.texsets.push(set);
        self.texset_cache.insert(set, i);
        i
    }

    /// Texture names per mesher slot for one state of a block.
    fn texset_for(def: &BlockDef, local: u16) -> [String; 8] {
        let t = |i: usize| def.tex.get(i).or_else(|| def.tex.last()).cloned().unwrap_or_default();
        let mut out: [String; 8] = Default::default();
        let s = def.shape;
        let g = |name: &str| s.get(local, name).unwrap_or(0);
        match s {
            Shape::Pillar => {
                // tex: [end, side]
                let axis = g("axis");
                for d in 0..6 {
                    out[d] = if d / 2 == [2, 0, 1][axis] { t(0) } else { t(1) };
                }
            }
            Shape::Facing | Shape::FacingLit | Shape::Chest => {
                // tex: [top, side, front, front_lit]
                let facing = model::facing_dir(g("facing"));
                let lit = s == Shape::FacingLit && g("lit") == 1;
                for d in 0..6 {
                    out[d] = if d < 2 { t(0) } else if d == facing { t(if lit { 3 } else { 2 }) } else { t(1) };
                }
            }
            Shape::DoublePlant => out[0] = t(g("half")),
            Shape::Door => out[0] = t(g("half")),
            Shape::Crop(_) => out[0] = t(g("age")),
            Shape::Farmland => {
                // tex: [side, dry top, moist top]
                for d in 0..6 {
                    out[d] = if d == 1 { t(if g("moisture") == 7 { 2 } else { 1 }) } else { t(0) };
                }
            }
            _ => {
                if def.tex.len() == 1 {
                    for o in out.iter_mut().take(6) {
                        *o = t(0);
                    }
                } else {
                    for (d, o) in out.iter_mut().enumerate() {
                        *o = def.tex.get(d).cloned().unwrap_or_default();
                    }
                }
            }
        }
        out
    }

    pub fn add(&mut self, mut def: BlockDef) -> u16 {
        assert!(!self.by_name.contains_key(def.name), "duplicate block {}", def.name);
        let index = self.blocks.len() as u16;
        def.first = self.states.len() as State;
        def.count = def.shape.state_count();
        let tint = if def.fixed_tint != 0 {
            let i = self.fixed_tints.iter().position(|c| *c == def.fixed_tint).unwrap_or_else(|| {
                self.fixed_tints.push(def.fixed_tint);
                self.fixed_tints.len() - 1
            });
            4 + i as u8
        } else {
            def.tint
        };
        for local in 0..def.count {
            let key = (def.shape, local);
            if !self.geom_cache.contains_key(&key) {
                let built = model::build(def.shape, local);
                let geom = if built.quads.is_empty() {
                    0
                } else {
                    self.geoms.push(VcGeom { first_quad: self.quads.len() as u32, quad_count: built.quads.len() as u32 });
                    self.quads.extend_from_slice(&built.quads);
                    (self.geoms.len() - 1) as u16
                };
                let box_start = self.boxes.len() as u32;
                self.boxes.extend_from_slice(&built.collision);
                let outline_start = self.boxes.len() as u32;
                self.boxes.extend_from_slice(&built.outline);
                let mut occ = 0u8;
                for d in 0..6 {
                    if covers_face(&built.collision, d) {
                        occ |= 1 << d;
                    }
                }
                self.geom_cache.insert(key, (geom, box_start, built.collision.len() as u8, outline_start, built.outline.len() as u8, occ, built.waterlogged));
            }
            let (geom, box_start, box_count, outline_start, outline_count, occ, waterlogged) = self.geom_cache[&key];
            let names = Self::texset_for(&def, local);
            let refs: [&str; 8] = std::array::from_fn(|i| names[i].as_str());
            let texset = self.texset(refs);
            let full = occ == 0x3f && def.opaque;
            let is_fluid = def.shape == Shape::Fluid;
            let lava = def.flags & F_LAVA != 0;
            let mut flags = def.flags;
            if box_count > 0 {
                flags |= F_SOLID;
            }
            if full {
                flags |= F_OPAQUE | F_STURDY_TOP;
            }
            if def.opaque && occ & 2 != 0 {
                flags |= F_STURDY_TOP;
            }
            if is_fluid {
                flags |= F_FLUID | F_REPLACEABLE | if lava { F_LAVA } else { F_WATER };
            }
            if waterlogged {
                flags |= F_WATER;
            }
            let lit = def.shape == Shape::FacingLit && def.shape.flag(local, "lit");
            let emit = if def.shape == Shape::FacingLit { if lit { def.light } else { 0 } } else { def.light };
            let filter = if full { 15 } else if is_fluid || waterlogged || (def.shape == Shape::Cube && !def.opaque && !def.cull_same) { 1 } else { 0 };
            self.states.push(StateData {
                block: index,
                flags,
                emit,
                filter,
                occlude: if def.opaque { occ } else { 0 },
                box_start,
                box_count,
                outline_start: if outline_count > 0 { outline_start } else { box_start },
                outline_count: if outline_count > 0 { outline_count } else { box_count },
            });
            let mut mflags = 0u8;
            if def.translucent {
                mflags |= S_TRANSLUCENT;
            }
            if full {
                mflags |= S_AO;
            }
            if def.cull_same {
                mflags |= S_CULL_SAME;
            }
            if def.offset {
                mflags |= S_OFFSET;
            }
            if lava {
                mflags |= S_LAVA;
            }
            if waterlogged || (!is_fluid && def.flags & F_WATER != 0) {
                mflags |= S_WATERLOGGED;
            }
            if def.light >= 14 && !def.opaque && def.shape == Shape::Torch {
                mflags |= S_FULLBRIGHT;
            }
            self.mesh_states.push(VcStateInfo {
                geom,
                texset,
                flags: mflags,
                occlude: if def.opaque { occ } else { 0 },
                tint,
                fluid: if is_fluid { 1 + local as u8 } else { 0 },
            });
        }
        self.by_name.insert(def.name, index);
        self.blocks.push(def);
        index
    }

    pub fn block_index(&self, name: &str) -> Option<u16> {
        self.by_name.get(name.strip_prefix("minecraft:").unwrap_or(name)).copied()
    }
    pub fn block_named(&self, name: &str) -> Option<&BlockDef> {
        self.block_index(name).map(|i| &self.blocks[i as usize])
    }
    pub fn default_state(&self, name: &str) -> Option<State> {
        self.block_named(name).map(|b| self.default_of(b))
    }
    /// Default state: the first one, except where another value is the natural resting state.
    pub fn default_of(&self, b: &BlockDef) -> State {
        let s = b.shape;
        let local = match s {
            Shape::Pillar => s.with(0, "axis", 1),
            Shape::Wall => s.with(0, "up", 1),
            _ => 0,
        };
        b.first + local
    }
    #[inline]
    pub fn data(&self, s: State) -> &StateData {
        &self.states[s as usize]
    }
    #[inline]
    pub fn block(&self, s: State) -> &BlockDef {
        &self.blocks[self.states[s as usize].block as usize]
    }
    #[inline]
    pub fn has(&self, s: State, flag: u16) -> bool {
        self.states[s as usize].flags & flag != 0
    }
    #[inline]
    pub fn local(&self, s: State) -> u16 {
        s - self.block(s).first
    }
    pub fn prop(&self, s: State, name: &str) -> Option<usize> {
        let b = self.block(s);
        b.shape.get(s - b.first, name)
    }
    pub fn with(&self, s: State, name: &str, value: usize) -> State {
        let b = self.block(s);
        b.first + b.shape.with(s - b.first, name, value)
    }
    pub fn collision(&self, s: State) -> &[Box6] {
        let d = &self.states[s as usize];
        &self.boxes[d.box_start as usize..d.box_start as usize + d.box_count as usize]
    }
    pub fn outline(&self, s: State) -> &[Box6] {
        let d = &self.states[s as usize];
        &self.boxes[d.outline_start as usize..d.outline_start as usize + d.outline_count as usize]
    }
    /// Human readable state, e.g. `oak_stairs[facing=north,half=bottom]`.
    pub fn describe(&self, s: State) -> String {
        let b = self.block(s);
        let props = b.shape.props();
        if props.is_empty() {
            return b.name.to_string();
        }
        let parts: Vec<String> = props.iter().map(|p| format!("{}={}", p.name, p.values[b.shape.get(s - b.first, p.name).unwrap_or(0)])).collect();
        format!("{}[{}]", b.name, parts.join(","))
    }
    /// Parses `name` or `name[prop=value,...]`.
    pub fn parse_state(&self, text: &str) -> Option<State> {
        let (name, rest) = match text.find('[') {
            Some(i) => (&text[..i], text[i + 1..].trim_end_matches(']')),
            None => (text, ""),
        };
        let b = self.block_named(name.trim())?;
        let mut s = self.default_of(b);
        for pair in rest.split(',').filter(|p| !p.trim().is_empty()) {
            let (k, v) = pair.split_once('=')?;
            let p = b.shape.props().iter().find(|p| p.name == k.trim())?;
            let value = p.values.iter().position(|x| *x == v.trim())?;
            s = self.with(s, p.name, value);
        }
        Some(s)
    }
    pub fn mesh_tables(&self) -> VcMeshTables {
        VcMeshTables {
            states: self.mesh_states.as_ptr(),
            geoms: self.geoms.as_ptr(),
            quads: self.quads.as_ptr(),
            texsets: self.texsets.as_ptr(),
            fixed_tints: self.fixed_tints.as_ptr(),
            num_states: self.mesh_states.len() as u32,
            water_still: 0,
            water_flow: 1,
            lava_still: 2,
            lava_flow: 3,
        }
    }
}

// SAFETY: the registry is immutable after construction; the raw pointers handed to the mesher
// point into its own vectors.
unsafe impl Sync for Registry {}
unsafe impl Send for Registry {}

static REGISTRY: OnceLock<Registry> = OnceLock::new();

/// The global block registry (built on first use).
pub fn reg() -> &'static Registry {
    REGISTRY.get_or_init(|| {
        let mut r = Registry::empty();
        super::blocks::register_all(&mut r);
        r.k = Known::resolve(&r);
        crate::item::register_all(&mut r);
        r
    })
}
