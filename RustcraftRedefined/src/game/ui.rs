//! Menus and container screens.  Immediate mode: each screen function handles its input and
//! draws itself in one pass, returning an `Action` for anything that changes the game state.

use super::entities::{BlockEntity, Furnace, COOK_TICKS};
use super::hud::{block_icon, centered, draw_stack, pretty_name};
use super::inventory::{add_to, click_left, click_right, take_output};
use super::player::GameMode;
use super::save::WorldInfo;
use super::Game;
use crate::art::font::*;
use crate::gfx::batch::{rgba, Batch, Rgba, WHITE};
use crate::input::{key, Input};
use crate::item::recipes::{craft, smelt};
use crate::item::{ItemDef, ItemId, ItemStack};
use crate::platform;
use crate::util::math::IVec3;
use crate::world::block::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    None,
    Title,
    Worlds,
    CreateWorld,
    Pause,
    Options,
    Inventory,
    Creative,
    Crafting,
    Furnace(IVec3),
    Chest(IVec3),
    Chat,
    Death,
}

impl Screen {
    /// Screens that show item slots.
    pub fn is_container(self) -> bool {
        matches!(self, Screen::Inventory | Screen::Creative | Screen::Crafting | Screen::Furnace(_) | Screen::Chest(_))
    }
}

/// What a screen asks the game to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    None,
    Open(Screen),
    Close,
    Create,
    Play(usize),
    Delete(usize),
    QuitToTitle,
    Quit,
    Respawn,
    Chat(String),
}

/// Mouse state for one frame in GUI units.
#[derive(Clone, Copy, Default, Debug)]
pub struct Mouse {
    pub x: f32,
    pub y: f32,
    pub left: bool,
    pub right: bool,
    pub middle: bool,
    pub held: bool,
    pub shift: bool,
    pub wheel: f32,
}

pub struct Ui {
    pub mouse: Mouse,
    /// Stack carried by the mouse cursor in container screens.
    pub cursor: ItemStack,
    /// Crafting grid; the 2x2 inventory grid uses the first four cells.
    pub craft: [ItemStack; 9],
    /// Chat input or creative search text.
    pub text: String,
    pub name: String,
    pub seed: String,
    pub focus: usize,
    pub history: Vec<String>,
    pub history_pos: usize,
    pub scroll: usize,
    pub tab: usize,
    pub worlds: Vec<WorldInfo>,
    pub selected_world: Option<usize>,
    pub confirm_delete: bool,
    pub create_mode: GameMode,
    pub options_return: Screen,
    pub drag: Option<usize>,
    /// Set when a screen opens; input is ignored for that frame so the opening key or click
    /// does not act on the new screen as well.
    pub fresh: bool,
    pub esc_guard: f64,
    pub last_click: f64,
}

impl Default for Ui {
    fn default() -> Self {
        Ui {
            mouse: Mouse::default(),
            cursor: ItemStack::EMPTY,
            craft: [ItemStack::EMPTY; 9],
            text: String::new(),
            name: "New World".to_string(),
            seed: String::new(),
            focus: 0,
            history: Vec::new(),
            history_pos: 0,
            scroll: 0,
            tab: 1,
            worlds: Vec::new(),
            selected_world: None,
            confirm_delete: false,
            create_mode: GameMode::Survival,
            options_return: Screen::Title,
            drag: None,
            fresh: false,
            esc_guard: 0.0,
            last_click: -1e9,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlotKind {
    Inv(usize),
    Craft(usize),
    Result,
    Input,
    Fuel,
    Output,
    Chest(usize),
    /// Creative item list entry.
    Palette(ItemId),
}

struct Slot {
    x: f32,
    y: f32,
    kind: SlotKind,
}

const GRAY: Rgba = [0xA0, 0xA0, 0xA0, 0xFF];
const LABEL: Rgba = [0x40, 0x40, 0x40, 0xFF];
const TAB_NAMES: [&str; 7] = ["Search Items", "Building Blocks", "Natural Blocks", "Functional Blocks", "Tools & Combat", "Food & Drinks", "Ingredients"];
const TAB_ICONS: [&str; 7] = ["bookshelf", "bricks", "grass_block", "crafting_table", "iron_pickaxe", "apple", "iron_ingot"];
const PALETTE_ROWS: usize = 5;

// ------------------------------------------------------------------ widgets
fn hit(m: Mouse, x: f32, y: f32, w: f32, h: f32) -> bool {
    m.x >= x && m.x < x + w && m.y >= y && m.y < y + h
}

#[allow(clippy::too_many_arguments)]
fn button(b: &mut Batch, font: &Font, m: Mouse, x: f32, y: f32, w: f32, h: f32, label: &str, enabled: bool) -> bool {
    let hover = enabled && hit(m, x, y, w, h);
    b.nine_slice(x, y, w, h, if hover { R_BUTTON_HOVER } else { R_BUTTON }, 3, if enabled { WHITE } else { rgba(0x707070, 255) });
    let color = if !enabled {
        GRAY
    } else if hover {
        rgba(0xFFFFA0, 255)
    } else {
        rgba(0xE0E0E0, 255)
    };
    b.text_shadow(font, (x + (w - font.width(label)) * 0.5).floor(), (y + (h - 8.0) * 0.5).floor(), label, 1.0, color);
    if hover && m.left {
        crate::audio::play(crate::audio::Sfx::Click, 0.25, 1.0);
    }
    hover && m.left
}

/// Slider with the value shown in its label.  Returns the new position (0..1) while dragged.
#[allow(clippy::too_many_arguments)]
fn slider(b: &mut Batch, font: &Font, m: Mouse, x: f32, y: f32, w: f32, label: &str, t: f32, id: usize, drag: &mut Option<usize>) -> Option<f32> {
    let h = 20.0;
    let hover = hit(m, x, y, w, h);
    if hover && m.left {
        *drag = Some(id);
    }
    if !m.held && *drag == Some(id) {
        *drag = None;
    }
    let active = *drag == Some(id);
    let mut t = t.clamp(0.0, 1.0);
    let mut out = None;
    if active {
        t = ((m.x - x - 4.0) / (w - 8.0)).clamp(0.0, 1.0);
        out = Some(t);
    }
    b.nine_slice(x, y, w, h, R_BUTTON, 3, rgba(0x707070, 255));
    b.nine_slice((x + t * (w - 8.0)).floor(), y, 8.0, h, if hover || active { R_BUTTON_HOVER } else { R_BUTTON }, 3, WHITE);
    b.text_shadow(font, (x + (w - font.width(label)) * 0.5).floor(), y + 6.0, label, 1.0, if hover || active { rgba(0xFFFFA0, 255) } else { rgba(0xE0E0E0, 255) });
    out
}

#[allow(clippy::too_many_arguments)]
fn text_field(b: &mut Batch, font: &Font, x: f32, y: f32, w: f32, h: f32, text: &str, focused: bool, now: f64) {
    b.rect(x - 1.0, y - 1.0, w + 2.0, h + 2.0, if focused { WHITE } else { GRAY });
    b.rect(x, y, w, h, rgba(0x000000, 255));
    // Long text scrolls so that its end stays visible.
    let mut shown = text;
    while font.width(shown) > w - 12.0 && !shown.is_empty() {
        shown = &shown[1..];
    }
    let ty = (y + (h - 8.0) * 0.5).floor();
    let end = b.text_shadow(font, x + 4.0, ty, shown, 1.0, rgba(0xE0E0E0, 255));
    if focused && (now / 300.0) as u64 % 2 == 0 {
        b.text_shadow(font, end, ty, "_", 1.0, WHITE);
    }
}

/// Applies typed characters and backspace to a text field.  Returns true when it changed.
fn edit_text(text: &mut String, input: &Input, max: usize) -> bool {
    let before = text.len();
    let mut changed = false;
    for &c in &input.typed {
        if (' '..='~').contains(&c) && text.len() < max {
            text.push(c);
        }
    }
    for &k in &input.key_events {
        if k == key::BACKSPACE && text.pop().is_some() {
            changed = true;
        }
    }
    changed || text.len() != before
}

fn slot_background(b: &mut Batch, x: f32, y: f32) {
    b.rect(x - 1.0, y - 1.0, 18.0, 18.0, rgba(0x373737, 255));
    b.rect(x, y, 17.0, 17.0, rgba(0xFFFFFF, 255));
    b.rect(x, y, 16.0, 16.0, rgba(0x8B8B8B, 255));
}

fn tooltip(b: &mut Batch, font: &Font, w: f32, h: f32, m: Mouse, text: &str) {
    let tw = font.width(text) + 5.0;
    let x = (m.x + 10.0).min(w - tw - 4.0).max(2.0).floor();
    let y = (m.y - 14.0).clamp(4.0, h - 16.0).floor();
    b.rect(x - 2.0, y - 3.0, tw + 4.0, 14.0, rgba(0x100010, 240));
    b.rect(x - 1.0, y - 2.0, tw + 2.0, 12.0, rgba(0x2D0A63, 255));
    b.rect(x, y - 1.0, tw, 10.0, rgba(0x100010, 255));
    b.text_shadow(font, x + 2.0, y, text, 1.0, WHITE);
}

fn logo(b: &mut Batch, font: &Font, w: f32, y: f32) {
    let scale = if w >= 420.0 { 5.0 } else { 4.0 };
    let title = "RUSTCRAFT";
    let x = ((w - font.width(title) * scale) * 0.5).floor();
    for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (1.0, 1.0), (2.0, 2.0)] {
        b.text(font, x + dx * scale * 0.4, y + dy * scale * 0.4, title, scale, rgba(0x1A1A1E, 255));
    }
    b.text(font, x, y, title, scale, rgba(0xC9C4BC, 255));
    let sub = "REDEFINED";
    b.text_shadow(font, ((w - font.width(sub) * 2.0) * 0.5).floor(), y + scale * 8.0 + 2.0, sub, 2.0, rgba(0xFFD84A, 255));
}

fn dim(b: &mut Batch, w: f32, h: f32) {
    b.rect(0.0, 0.0, w, h, rgba(0x101010, 170));
}

// ------------------------------------------------------------------ creative item list
fn category(def: &ItemDef) -> usize {
    let n = def.name;
    if def.food > 0 {
        return 4;
    }
    if def.tool != Tool::None || n.ends_with("bucket") || n == "flint_and_steel" {
        return 3;
    }
    let Some(bi) = def.block else { return 5 };
    let b = &reg().blocks[bi as usize];
    if def.icon != 0 {
        return 1;
    }
    let has = |keys: &[&str]| keys.iter().any(|k| n.contains(k));
    if has(&["torch", "door", "ladder", "chest", "furnace", "crafting_table", "jukebox", "note_block", "tnt", "spawner", "glowstone", "lamp", "lantern", "bookshelf", "sponge", "slime", "honey", "magma", "potent", "spike"]) {
        return 2;
    }
    let worked = has(&["brick", "polished", "slab", "stairs", "wall", "chiseled", "smooth", "cut_", "planks", "tiles", "pillar", "fence", "glass", "wool", "concrete", "terracotta", "carpet", "stripped"]);
    let natural = b.flags & (F_PLANT | F_LEAVES) != 0
        || has(&["_ore", "_log", "dirt", "sand", "gravel", "grass", "snow", "ice", "clay", "mud", "moss", "stone", "deepslate", "granite", "diorite", "andesite", "tuff", "calcite", "basalt", "netherrack", "obsidian", "sulfur", "cinnabar", "dripstone", "mushroom", "cactus", "vine", "podzol", "mycelium", "coral", "kelp", "melon", "pumpkin", "bedrock", "amethyst", "raw_", "lily", "sapling"]);
    if natural && !worked {
        1
    } else {
        0
    }
}

fn creative_list(tab: usize, query: &str) -> Vec<ItemId> {
    let q = query.to_lowercase();
    reg().items.defs.iter().enumerate().skip(1).filter(|(_, d)| if tab == 0 { q.is_empty() || d.name.replace('_', " ").contains(&q) } else { category(d) == tab - 1 }).map(|(i, _)| i as ItemId).collect()
}

// ------------------------------------------------------------------ slots
fn player_slots(out: &mut Vec<Slot>, x0: f32, y0: f32) {
    for row in 0..3 {
        for col in 0..9 {
            out.push(Slot { x: x0 + col as f32 * 18.0, y: y0 + row as f32 * 18.0, kind: SlotKind::Inv(9 + row * 9 + col) });
        }
    }
    for col in 0..9 {
        out.push(Slot { x: x0 + col as f32 * 18.0, y: y0 + 58.0, kind: SlotKind::Inv(col) });
    }
}

/// Window size and slot positions of the open container screen.
fn layout(screen: Screen, palette: &[ItemId], scroll: usize) -> (f32, f32, Vec<Slot>) {
    let mut s = Vec::with_capacity(64);
    match screen {
        Screen::Inventory => {
            for i in 0..4 {
                s.push(Slot { x: 98.0 + (i % 2) as f32 * 18.0, y: 18.0 + (i / 2) as f32 * 18.0, kind: SlotKind::Craft(i) });
            }
            s.push(Slot { x: 154.0, y: 28.0, kind: SlotKind::Result });
            player_slots(&mut s, 8.0, 84.0);
        }
        Screen::Crafting => {
            for i in 0..9 {
                s.push(Slot { x: 30.0 + (i % 3) as f32 * 18.0, y: 17.0 + (i / 3) as f32 * 18.0, kind: SlotKind::Craft(i) });
            }
            s.push(Slot { x: 124.0, y: 35.0, kind: SlotKind::Result });
            player_slots(&mut s, 8.0, 84.0);
        }
        Screen::Furnace(_) => {
            s.push(Slot { x: 56.0, y: 17.0, kind: SlotKind::Input });
            s.push(Slot { x: 56.0, y: 53.0, kind: SlotKind::Fuel });
            s.push(Slot { x: 116.0, y: 35.0, kind: SlotKind::Output });
            player_slots(&mut s, 8.0, 84.0);
        }
        Screen::Chest(_) => {
            for i in 0..27 {
                s.push(Slot { x: 8.0 + (i % 9) as f32 * 18.0, y: 18.0 + (i / 9) as f32 * 18.0, kind: SlotKind::Chest(i) });
            }
            player_slots(&mut s, 8.0, 84.0);
        }
        Screen::Creative => {
            for (i, id) in palette.iter().skip(scroll * 9).take(PALETTE_ROWS * 9).enumerate() {
                s.push(Slot { x: 8.0 + (i % 9) as f32 * 18.0, y: 40.0 + (i / 9) as f32 * 18.0, kind: SlotKind::Palette(*id) });
            }
            for col in 0..9 {
                s.push(Slot { x: 8.0 + col as f32 * 18.0, y: 142.0, kind: SlotKind::Inv(col) });
            }
            return (194.0, 166.0, s);
        }
        _ => {}
    }
    (176.0, 166.0, s)
}

impl Game {
    fn furnace_mut(&mut self) -> Option<&mut Furnace> {
        let Screen::Furnace(pos) = self.screen else { return None };
        match self.block_entities.get_mut(&pos) {
            Some(BlockEntity::Furnace(f)) => Some(f),
            _ => None,
        }
    }

    fn slot_mut(&mut self, kind: SlotKind) -> Option<&mut ItemStack> {
        match kind {
            SlotKind::Inv(i) => self.inv.slots.get_mut(i),
            SlotKind::Craft(i) => self.ui.craft.get_mut(i),
            SlotKind::Input => self.furnace_mut().map(|f| &mut f.input),
            SlotKind::Fuel => self.furnace_mut().map(|f| &mut f.fuel),
            SlotKind::Output => self.furnace_mut().map(|f| &mut f.output),
            SlotKind::Chest(i) => {
                let Screen::Chest(pos) = self.screen else { return None };
                match self.block_entities.get_mut(&pos) {
                    Some(BlockEntity::Chest(items)) => items.get_mut(i),
                    _ => None,
                }
            }
            SlotKind::Result | SlotKind::Palette(_) => None,
        }
    }

    /// What a slot currently shows.
    pub fn slot_stack(&self, kind: SlotKind) -> ItemStack {
        let furnace = || match self.screen {
            Screen::Furnace(pos) => match self.block_entities.get(&pos) {
                Some(BlockEntity::Furnace(f)) => Some(f),
                _ => None,
            },
            _ => None,
        };
        match kind {
            SlotKind::Inv(i) => self.inv.slots[i],
            SlotKind::Craft(i) => self.ui.craft[i],
            SlotKind::Result => self.craft_result().unwrap_or(ItemStack::EMPTY),
            SlotKind::Input => furnace().map_or(ItemStack::EMPTY, |f| f.input),
            SlotKind::Fuel => furnace().map_or(ItemStack::EMPTY, |f| f.fuel),
            SlotKind::Output => furnace().map_or(ItemStack::EMPTY, |f| f.output),
            SlotKind::Chest(i) => match self.screen {
                Screen::Chest(pos) => match self.block_entities.get(&pos) {
                    Some(BlockEntity::Chest(items)) => items[i],
                    _ => ItemStack::EMPTY,
                },
                _ => ItemStack::EMPTY,
            },
            SlotKind::Palette(id) => ItemStack::new(id, 1),
        }
    }

    fn craft_size(&self) -> usize {
        if self.screen == Screen::Crafting { 3 } else { 2 }
    }

    pub fn craft_result(&self) -> Option<ItemStack> {
        let n = self.craft_size();
        craft(&self.ui.craft[..n * n], n)
    }

    fn consume_craft(&mut self) {
        let n = self.craft_size();
        for s in self.ui.craft[..n * n].iter_mut() {
            s.take(1);
        }
    }

    fn can_fit(&self, stack: ItemStack) -> bool {
        self.inv.clone().add(stack).is_empty()
    }

    /// Mouse click on a slot of the open container screen.
    pub fn click_slot(&mut self, kind: SlotKind, right: bool, shift: bool) {
        let mut cursor = std::mem::take(&mut self.ui.cursor);
        match kind {
            SlotKind::Result => {
                if let Some(out) = self.craft_result() {
                    if shift {
                        // Craft repeatedly straight into the inventory.
                        for _ in 0..64 {
                            match self.craft_result() {
                                Some(o) if o == out && self.can_fit(o) => {
                                    self.inv.add(o);
                                    self.consume_craft();
                                }
                                _ => break,
                            }
                        }
                    } else {
                        let mut o = out;
                        if take_output(&mut o, &mut cursor) {
                            self.consume_craft();
                        }
                    }
                }
            }
            SlotKind::Output => {
                if shift {
                    let taken = self.furnace_mut().map(|f| std::mem::take(&mut f.output)).unwrap_or_default();
                    let rest = self.inv.add(taken);
                    if let Some(f) = self.furnace_mut() {
                        f.output = rest;
                    }
                } else if let Some(slot) = self.slot_mut(kind) {
                    take_output(slot, &mut cursor);
                }
            }
            SlotKind::Palette(id) => {
                let max = reg().items.defs[id as usize].max_stack;
                if shift {
                    cursor = ItemStack::new(id, max);
                } else if cursor.is_empty() {
                    cursor = ItemStack::new(id, 1);
                } else if cursor.id == id && cursor.count < max && max > 1 {
                    cursor.count += 1;
                } else {
                    // Putting a stack back onto the item list deletes it.
                    cursor = ItemStack::EMPTY;
                }
            }
            _ if shift => {
                self.ui.cursor = cursor;
                self.quick_move(kind);
                return;
            }
            // Only things that burn go into the fuel slot.
            SlotKind::Fuel if !cursor.is_empty() && cursor.def().fuel == 0 => {}
            _ => {
                if let Some(slot) = self.slot_mut(kind) {
                    if right {
                        click_right(slot, &mut cursor);
                    } else {
                        click_left(slot, &mut cursor);
                    }
                }
            }
        }
        self.ui.cursor = cursor;
    }

    /// Shift-click: moves a whole stack to the other side of the screen.
    fn quick_move(&mut self, kind: SlotKind) {
        let Some(stack) = self.slot_mut(kind).map(|s| *s) else { return };
        if stack.is_empty() {
            return;
        }
        let swap_sections = |g: &mut Game, i: usize| if i < 9 { g.inv.add_range(stack, 9, 36) } else { g.inv.add_range(stack, 0, 9) };
        let rest = match (self.screen, kind) {
            (Screen::Chest(pos), SlotKind::Inv(_)) => match self.block_entities.get_mut(&pos) {
                Some(BlockEntity::Chest(items)) => add_to(&mut items[..], stack),
                _ => stack,
            },
            (Screen::Furnace(_), SlotKind::Inv(i)) => {
                let (smeltable, burns) = (smelt(stack.id).is_some(), stack.def().fuel > 0);
                let mut rest = stack;
                if let Some(f) = self.furnace_mut() {
                    if smeltable {
                        rest = add_to(std::slice::from_mut(&mut f.input), rest);
                    } else if burns {
                        rest = add_to(std::slice::from_mut(&mut f.fuel), rest);
                    }
                }
                if rest == stack { swap_sections(self, i) } else { rest }
            }
            (Screen::Creative, SlotKind::Inv(_)) => ItemStack::EMPTY,
            (_, SlotKind::Inv(i)) => swap_sections(self, i),
            _ => self.inv.add(stack),
        };
        if let Some(slot) = self.slot_mut(kind) {
            *slot = rest;
        }
    }

    /// Number key pressed while hovering a slot: exchange it with that hotbar slot.
    fn swap_hotbar(&mut self, kind: SlotKind, n: usize) {
        match kind {
            SlotKind::Palette(id) => {
                self.inv.slots[n] = ItemStack::new(id, reg().items.defs[id as usize].max_stack);
                return;
            }
            SlotKind::Result | SlotKind::Output => return,
            SlotKind::Inv(i) if i == n => return,
            _ => {}
        }
        let from_hotbar = self.inv.slots[n];
        if kind == SlotKind::Fuel && !from_hotbar.is_empty() && from_hotbar.def().fuel == 0 {
            return;
        }
        let Some(slot) = self.slot_mut(kind) else { return };
        let other = std::mem::replace(slot, from_hotbar);
        self.inv.slots[n] = other;
    }
}

// ------------------------------------------------------------------ screens
pub fn run(g: &mut Game, w: f32, h: f32) -> Action {
    let fresh = std::mem::replace(&mut g.ui.fresh, false);
    let mut m = g.ui.mouse;
    if fresh {
        m = Mouse { x: m.x, y: m.y, held: m.held, shift: m.shift, ..Mouse::default() };
    }
    let esc = !fresh && g.input.pressed(key::ESCAPE) && platform::now_ms() > g.ui.esc_guard;
    match g.screen {
        Screen::None => Action::None,
        Screen::Title => title(g, w, h, m),
        Screen::Worlds => worlds(g, w, h, m, esc),
        Screen::CreateWorld => create(g, w, h, m, esc, fresh),
        Screen::Pause => pause(g, w, h, m, esc),
        Screen::Options => options(g, w, h, m, esc),
        Screen::Death => death(g, w, h, m),
        Screen::Chat => chat(g, w, h, fresh),
        _ => container(g, w, h, m, esc, fresh),
    }
}

fn title(g: &mut Game, w: f32, h: f32, m: Mouse) -> Action {
    let (b, font) = (&mut g.renderer.batch, &g.renderer.font);
    logo(b, font, w, (h * 0.13).max(6.0).floor());
    let x = (w * 0.5 - 100.0).floor();
    let y = (h / 4.0 + 48.0).floor();
    let mut action = Action::None;
    if button(b, font, m, x, y, 200.0, 20.0, "Singleplayer", true) {
        action = Action::Open(Screen::Worlds);
    }
    button(b, font, m, x, y + 24.0, 200.0, 20.0, "Multiplayer (not available)", false);
    let can_quit = cfg!(not(target_arch = "wasm32"));
    if button(b, font, m, x, y + 60.0, if can_quit { 98.0 } else { 200.0 }, 20.0, "Options...", true) {
        action = Action::Open(Screen::Options);
    }
    if can_quit && button(b, font, m, x + 102.0, y + 60.0, 98.0, 20.0, "Quit Game", true) {
        action = Action::Quit;
    }
    b.text_shadow(font, 2.0, h - 10.0, concat!("Rustcraft Redefined ", env!("CARGO_PKG_VERSION")), 1.0, WHITE);
    let note = "Fan project, not affiliated with Mojang or Microsoft";
    b.text_shadow(font, w - font.width(note) - 2.0, h - 10.0, note, 1.0, WHITE);
    action
}

fn worlds(g: &mut Game, w: f32, h: f32, m: Mouse, esc: bool) -> Action {
    let now = platform::now_ms();
    let mut action = Action::None;
    let n = g.ui.worlds.len();
    let (b, font) = (&mut g.renderer.batch, &g.renderer.font);
    dim(b, w, h);
    centered(b, font, w, 12.0, "Select World", 1.0, WHITE);
    let (lx, lw) = ((w * 0.5 - 135.0).floor(), 270.0);
    let (top, bottom) = (30.0, h - 62.0);
    b.rect(0.0, top, w, bottom - top, rgba(0x000000, 110));
    let rows = (((bottom - top - 4.0) / 36.0).floor() as usize).max(1);
    let max_scroll = n.saturating_sub(rows);
    if m.wheel != 0.0 {
        g.ui.scroll = (g.ui.scroll as i32 + m.wheel.signum() as i32).clamp(0, max_scroll as i32) as usize;
    }
    let scroll = g.ui.scroll.min(max_scroll);
    let grass = reg().k.grass_block;
    for (row, i) in (scroll..n.min(scroll + rows)).enumerate() {
        let info = &g.ui.worlds[i];
        let y = top + 4.0 + row as f32 * 36.0;
        let selected = g.ui.selected_world == Some(i);
        if selected {
            b.rect(lx - 2.0, y - 2.0, lw + 4.0, 36.0, rgba(0x808080, 255));
            b.rect(lx - 1.0, y - 1.0, lw + 2.0, 34.0, rgba(0x000000, 255));
        }
        block_icon(b, lx, y, 32.0, grass);
        b.text_shadow(font, lx + 36.0, y + 1.0, &info.name, 1.0, WHITE);
        b.text_shadow(font, lx + 36.0, y + 12.0, &format!("{} (seed {})", info.id, info.seed as i64), 1.0, rgba(0x808080, 255));
        b.text_shadow(font, lx + 36.0, y + 22.0, &format!("{:?} Mode", info.mode), 1.0, rgba(0x808080, 255));
        if m.left && hit(m, lx, y, lw, 34.0) {
            if selected && now - g.ui.last_click < 400.0 {
                action = Action::Play(i);
            }
            g.ui.selected_world = Some(i);
            g.ui.last_click = now;
            g.ui.confirm_delete = false;
        }
    }
    if n == 0 {
        centered(b, font, w, top + 20.0, "No worlds yet - create one to start playing.", 1.0, GRAY);
    } else if n > rows {
        centered(b, font, w, bottom - 10.0, &format!("{}-{} of {n} (scroll for more)", scroll + 1, (scroll + rows).min(n)), 1.0, GRAY);
    }
    let sel = g.ui.selected_world.filter(|i| *i < n);
    let x = (w * 0.5).floor();
    if button(b, font, m, x - 154.0, h - 52.0, 150.0, 20.0, "Play Selected World", sel.is_some()) {
        action = Action::Play(sel.unwrap_or(0));
    }
    if button(b, font, m, x + 4.0, h - 52.0, 150.0, 20.0, "Create New World", true) {
        action = Action::Open(Screen::CreateWorld);
    }
    let label = if g.ui.confirm_delete { "Click again to delete" } else { "Delete" };
    if button(b, font, m, x - 154.0, h - 28.0, 150.0, 20.0, label, sel.is_some()) {
        if g.ui.confirm_delete {
            action = Action::Delete(sel.unwrap_or(0));
        }
        g.ui.confirm_delete = !g.ui.confirm_delete;
    }
    if button(b, font, m, x + 4.0, h - 28.0, 150.0, 20.0, "Cancel", true) || esc {
        action = Action::Open(Screen::Title);
    }
    action
}

fn create(g: &mut Game, w: f32, h: f32, m: Mouse, esc: bool, fresh: bool) -> Action {
    let now = platform::now_ms();
    let mut enter = false;
    if !fresh {
        if g.input.pressed(key::TAB) {
            g.ui.focus ^= 1;
        }
        let field = if g.ui.focus == 0 { &mut g.ui.name } else { &mut g.ui.seed };
        edit_text(field, &g.input, 32);
        enter = g.input.pressed(key::ENTER);
    }
    let (b, font) = (&mut g.renderer.batch, &g.renderer.font);
    dim(b, w, h);
    centered(b, font, w, 12.0, "Create New World", 1.0, WHITE);
    let x = (w * 0.5 - 100.0).floor();
    let mut y = (h * 0.5 - 76.0).max(28.0).floor();
    b.text_shadow(font, x, y, "World Name", 1.0, GRAY);
    text_field(b, font, x, y + 11.0, 200.0, 18.0, &g.ui.name, g.ui.focus == 0, now);
    if m.left && hit(m, x, y + 11.0, 200.0, 18.0) {
        g.ui.focus = 0;
    }
    y += 38.0;
    b.text_shadow(font, x, y, "Seed for the world generator", 1.0, GRAY);
    text_field(b, font, x, y + 11.0, 200.0, 18.0, &g.ui.seed, g.ui.focus == 1, now);
    if m.left && hit(m, x, y + 11.0, 200.0, 18.0) {
        g.ui.focus = 1;
    }
    b.text_shadow(font, x, y + 33.0, "Leave blank for a random seed", 1.0, GRAY);
    y += 50.0;
    if button(b, font, m, x, y, 200.0, 20.0, &format!("Game Mode: {:?}", g.ui.create_mode), true) {
        g.ui.create_mode = if g.ui.create_mode == GameMode::Survival { GameMode::Creative } else { GameMode::Survival };
    }
    let x2 = (w * 0.5).floor();
    let mut action = Action::None;
    if button(b, font, m, x2 - 154.0, h - 28.0, 150.0, 20.0, "Create New World", true) || enter {
        action = Action::Create;
    }
    if button(b, font, m, x2 + 4.0, h - 28.0, 150.0, 20.0, "Cancel", true) || esc {
        action = Action::Open(Screen::Worlds);
    }
    action
}

fn pause(g: &mut Game, w: f32, h: f32, m: Mouse, esc: bool) -> Action {
    let (b, font) = (&mut g.renderer.batch, &g.renderer.font);
    b.rect(0.0, 0.0, w, h, rgba(0x101010, 140));
    centered(b, font, w, (h / 4.0 - 16.0).max(8.0).floor(), "Game Menu", 1.0, WHITE);
    let x = (w * 0.5 - 100.0).floor();
    let y = (h / 4.0 + 8.0).floor();
    let mut action = Action::None;
    if button(b, font, m, x, y, 200.0, 20.0, "Back to Game", true) || esc {
        action = Action::Close;
    }
    if button(b, font, m, x, y + 24.0, 200.0, 20.0, "Options...", true) {
        action = Action::Open(Screen::Options);
    }
    let label = if g.world_id.is_some() { "Save and Quit to Title" } else { "Quit to Title" };
    if button(b, font, m, x, y + 48.0, 200.0, 20.0, label, true) {
        action = Action::QuitToTitle;
    }
    if g.world_id.is_none() {
        centered(b, font, w, y + 76.0, "This world was started with /newworld and is not saved.", 1.0, GRAY);
    }
    action
}

fn options(g: &mut Game, w: f32, h: f32, m: Mouse, esc: bool) -> Action {
    let s = &mut g.settings;
    let drag = &mut g.ui.drag;
    let (b, font) = (&mut g.renderer.batch, &g.renderer.font);
    dim(b, w, h);
    centered(b, font, w, 12.0, "Options", 1.0, WHITE);
    let (x1, x2) = ((w * 0.5 - 155.0).floor(), (w * 0.5 + 5.0).floor());
    let y = (h / 6.0).max(30.0).floor();
    let fov = match s.fov as i32 {
        70 => "FOV: Normal".to_string(),
        110 => "FOV: Quake Pro".to_string(),
        v => format!("FOV: {v}"),
    };
    if let Some(t) = slider(b, font, m, x1, y, 150.0, &fov, (s.fov - 30.0) / 80.0, 0, drag) {
        s.fov = (30.0 + t * 80.0).round();
    }
    if let Some(t) = slider(b, font, m, x2, y, 150.0, &format!("Render Distance: {} chunks", s.render_distance), (s.render_distance - 2) as f32 / 22.0, 1, drag) {
        s.render_distance = 2 + (t * 22.0).round() as i32;
    }
    if let Some(t) = slider(b, font, m, x1, y + 24.0, 150.0, &format!("Sensitivity: {}%", (s.sensitivity / 0.15 * 100.0).round() as i32), s.sensitivity / 0.3, 2, drag) {
        s.sensitivity = (t * 0.3).max(0.0075);
    }
    let bright = match (s.gamma * 100.0).round() as i32 {
        0 => "Brightness: Moody".to_string(),
        100 => "Brightness: Bright".to_string(),
        v => format!("Brightness: {v}%"),
    };
    if let Some(t) = slider(b, font, m, x2, y + 24.0, 150.0, &bright, s.gamma, 3, drag) {
        s.gamma = (t * 100.0).round() / 100.0;
    }
    let scale = if s.gui_scale < 1.0 { "GUI Scale: Auto".to_string() } else { format!("GUI Scale: {}", s.gui_scale as i32) };
    if button(b, font, m, x1, y + 48.0, 150.0, 20.0, &scale, true) {
        s.gui_scale = ((s.gui_scale as i32 + 1) % 5) as f32;
    }
    if button(b, font, m, x2, y + 48.0, 150.0, 20.0, if s.bobbing { "View Bobbing: ON" } else { "View Bobbing: OFF" }, true) {
        s.bobbing = !s.bobbing;
    }
    if button(b, font, m, x1, y + 72.0, 150.0, 20.0, if s.peaceful { "Difficulty: Peaceful" } else { "Difficulty: Normal" }, true) {
        s.peaceful = !s.peaceful;
    }
    let sound = if s.volume < 0.005 { "Sound: OFF".to_string() } else { format!("Sound: {}%", (s.volume * 100.0).round() as i32) };
    if let Some(t) = slider(b, font, m, x2, y + 72.0, 150.0, &sound, s.volume, 4, drag) {
        s.volume = (t * 20.0).round() / 20.0;
        crate::audio::set_volume(s.volume);
    }
    if button(b, font, m, (w * 0.5 - 100.0).floor(), h - 28.0, 200.0, 20.0, "Done", true) || esc {
        return Action::Open(g.ui.options_return);
    }
    Action::None
}

fn death(g: &mut Game, w: f32, h: f32, m: Mouse) -> Action {
    let (b, font) = (&mut g.renderer.batch, &g.renderer.font);
    b.rect(0.0, 0.0, w, h, rgba(0x600000, 120));
    centered(b, font, w, (h / 4.0).floor(), "You Died!", 2.0, WHITE);
    centered(b, font, w, (h / 4.0 + 28.0).floor(), &format!("Player {}", g.vitals.death_message), 1.0, WHITE);
    let x = (w * 0.5 - 100.0).floor();
    let y = (h / 4.0 + 72.0).floor();
    if button(b, font, m, x, y, 200.0, 20.0, "Respawn", true) {
        return Action::Respawn;
    }
    if button(b, font, m, x, y + 24.0, 200.0, 20.0, "Title Screen", true) {
        return Action::QuitToTitle;
    }
    Action::None
}

fn chat(g: &mut Game, w: f32, h: f32, fresh: bool) -> Action {
    if !fresh {
        edit_text(&mut g.ui.text, &g.input, 256);
        for &k in &g.input.key_events {
            // Arrow keys walk through previously sent lines.
            if k == key::UP && g.ui.history_pos > 0 {
                g.ui.history_pos -= 1;
                g.ui.text = g.ui.history[g.ui.history_pos].clone();
            } else if k == key::DOWN && g.ui.history_pos < g.ui.history.len() {
                g.ui.history_pos += 1;
                g.ui.text = g.ui.history.get(g.ui.history_pos).cloned().unwrap_or_default();
            }
        }
        if g.input.pressed(key::ESCAPE) {
            return Action::Close;
        }
        if g.input.pressed(key::ENTER) {
            return Action::Chat(g.ui.text.trim().to_string());
        }
    }
    let (b, font) = (&mut g.renderer.batch, &g.renderer.font);
    b.rect(2.0, h - 14.0, w - 4.0, 12.0, rgba(0x000000, 128));
    let end = b.text_shadow(font, 4.0, h - 12.0, &g.ui.text, 1.0, WHITE);
    if (platform::now_ms() / 300.0) as u64 % 2 == 0 {
        b.text_shadow(font, end, h - 12.0, "_", 1.0, WHITE);
    }
    Action::None
}

fn container(g: &mut Game, w: f32, h: f32, m: Mouse, esc: bool, fresh: bool) -> Action {
    let creative = g.screen == Screen::Creative;
    let searching = creative && g.ui.tab == 0;
    let palette = if creative { creative_list(g.ui.tab, &g.ui.text) } else { Vec::new() };
    let max_scroll = palette.len().div_ceil(9).saturating_sub(PALETTE_ROWS);
    g.ui.scroll = g.ui.scroll.min(max_scroll);
    let (pw, ph, slots) = layout(g.screen, &palette, g.ui.scroll);
    let (px, py) = (((w - pw) * 0.5).floor(), ((h - ph) * 0.5).floor());
    let hovered = slots.iter().find(|s| hit(m, px + s.x - 1.0, py + s.y - 1.0, 18.0, 18.0)).map(|s| s.kind);
    let bar = (px + 174.0, py + 40.0, 12.0, PALETTE_ROWS as f32 * 18.0 - 2.0);

    // ---- input
    if !fresh {
        if esc || (!searching && g.input.pressed(key::E)) {
            return Action::Close;
        }
        if searching && edit_text(&mut g.ui.text, &g.input, 24) {
            g.ui.scroll = 0;
        }
        if creative {
            if m.wheel != 0.0 {
                g.ui.scroll = (g.ui.scroll as i32 + m.wheel.signum() as i32).clamp(0, max_scroll as i32) as usize;
            }
            for t in 0..TAB_NAMES.len() {
                if m.left && hit(m, px + 6.0 + t as f32 * 26.0, py + 4.0, 24.0, 20.0) && g.ui.tab != t {
                    g.ui.tab = t;
                    g.ui.scroll = 0;
                    g.ui.text.clear();
                    return Action::None;
                }
            }
            if m.left && hit(m, bar.0, bar.1, bar.2, bar.3) {
                g.ui.drag = Some(100);
            }
            if !m.held {
                g.ui.drag = None;
            }
            if g.ui.drag == Some(100) && max_scroll > 0 {
                g.ui.scroll = (((m.y - bar.1 - 7.0) / (bar.3 - 15.0)).clamp(0.0, 1.0) * max_scroll as f32).round() as usize;
            }
        }
        match hovered {
            Some(kind) => {
                if m.left || m.right {
                    g.click_slot(kind, m.right, m.shift);
                } else if m.middle && g.player.mode == GameMode::Creative && g.ui.cursor.is_empty() {
                    let s = g.slot_stack(kind);
                    if !s.is_empty() {
                        g.ui.cursor = ItemStack { count: s.max(), ..s };
                    }
                } else if !searching {
                    for n in 0..9 {
                        if g.input.pressed(key::N1 + n) {
                            g.swap_hotbar(kind, n as usize);
                        }
                    }
                    if g.input.pressed(key::Q) && g.ui.cursor.is_empty() {
                        let all = g.input.down(key::CONTROL);
                        let thrown = g.slot_mut(kind).map(|s| {
                            let n = if all { s.count } else { 1 };
                            s.take(n)
                        });
                        if let Some(s) = thrown {
                            g.throw_item(s);
                        }
                    }
                }
            }
            None => {
                if (m.left || m.right) && !hit(m, px, py, pw, ph) && !g.ui.cursor.is_empty() {
                    // Clicking outside the window throws the carried stack (right click: one item).
                    let n = if m.right { 1 } else { g.ui.cursor.count };
                    let s = g.ui.cursor.take(n);
                    g.throw_item(s);
                }
            }
        }
    }

    // ---- gather what the slots show before borrowing the renderer
    let (_, _, slots) = layout(g.screen, &palette, g.ui.scroll);
    let stacks: Vec<ItemStack> = slots.iter().map(|s| g.slot_stack(s.kind)).collect();
    let furnace = match g.screen {
        Screen::Furnace(pos) => match g.block_entities.get(&pos) {
            Some(BlockEntity::Furnace(f)) => Some((f.burn, f.burn_total, f.cook)),
            _ => None,
        },
        _ => None,
    };
    let cursor = g.ui.cursor;
    let screen = g.screen;
    let tab = g.ui.tab;
    let scroll = g.ui.scroll;
    let now = platform::now_ms();
    let (b, font) = (&mut g.renderer.batch, &g.renderer.font);
    dim(b, w, h);
    b.nine_slice(px, py, pw, ph, R_PANEL, 4, WHITE);
    let mut tip: Option<String> = None;
    match screen {
        Screen::Inventory => {
            b.text(font, px + 97.0, py + 7.0, "Crafting", 1.0, LABEL);
            b.sprite(px + 135.0, py + 29.0, 16.0, 13.0, R_ARROW, rgba(0x8B8B8B, 255));
        }
        Screen::Crafting => {
            b.text(font, px + 29.0, py + 6.0, "Crafting", 1.0, LABEL);
            b.icon(px + 90.0, py + 35.0, R_ARROW, rgba(0x8B8B8B, 255));
        }
        Screen::Furnace(_) => {
            b.text(font, px + 8.0, py + 6.0, "Furnace", 1.0, LABEL);
            let (burn, total, cook) = furnace.unwrap_or((0, 0, 0));
            b.icon(px + 57.0, py + 36.0, R_FLAME, rgba(0x8B8B8B, 255));
            if burn > 0 && total > 0 {
                let lit = ((burn as f32 / total as f32) * 14.0).ceil().clamp(1.0, 14.0);
                b.sprite(px + 57.0, py + 36.0 + 14.0 - lit, 14.0, lit, [R_FLAME[0], R_FLAME[1] + 14 - lit as u16, 14, lit as u16], WHITE);
            }
            b.icon(px + 80.0, py + 35.0, R_ARROW, rgba(0x8B8B8B, 255));
            let done = (cook as f32 / COOK_TICKS as f32 * 22.0).floor().clamp(0.0, 22.0);
            if done > 0.0 {
                b.sprite(px + 80.0, py + 35.0, done, 15.0, [R_ARROW[0], R_ARROW[1], done as u16, 15], WHITE);
            }
        }
        Screen::Chest(_) => {
            b.text(font, px + 8.0, py + 6.0, "Chest", 1.0, LABEL);
        }
        Screen::Creative => {
            for (t, icon) in TAB_ICONS.iter().enumerate() {
                let (tx, ty) = (px + 6.0 + t as f32 * 26.0, py + 4.0);
                let over = hit(m, tx, ty, 24.0, 20.0);
                b.rect(tx, ty, 24.0, 20.0, if t == tab { rgba(0xFFFFFF, 255) } else if over { rgba(0xA8A8A8, 255) } else { rgba(0x8B8B8B, 255) });
                b.rect(tx + 1.0, ty + 1.0, 22.0, 18.0, if t == tab { rgba(0xC6C6C6, 255) } else { rgba(0x9A9A9A, 255) });
                draw_stack(b, font, tx + 4.0, ty + 2.0, ItemStack::named(icon, 1));
                if over {
                    tip = Some(TAB_NAMES[t].to_string());
                }
            }
            if tab == 0 {
                text_field(b, font, px + 9.0, py + 27.0, 160.0, 10.0, &g.ui.text, true, now);
            } else {
                b.text(font, px + 8.0, py + 28.0, TAB_NAMES[tab], 1.0, LABEL);
            }
            b.rect(bar.0, bar.1, bar.2, bar.3, rgba(0x555555, 255));
            b.rect(bar.0 + 1.0, bar.1 + 1.0, bar.2 - 2.0, bar.3 - 2.0, rgba(0x8B8B8B, 255));
            let t = if max_scroll == 0 { 0.0 } else { scroll as f32 / max_scroll as f32 };
            b.nine_slice(bar.0, (bar.1 + t * (bar.3 - 15.0)).floor(), 12.0, 15.0, if max_scroll == 0 { R_BUTTON } else { R_BUTTON_HOVER }, 3, if max_scroll == 0 { GRAY } else { WHITE });
        }
        _ => {}
    }
    if screen != Screen::Creative {
        b.text(font, px + 8.0, py + ph - 94.0, "Inventory", 1.0, LABEL);
    }
    for (slot, stack) in slots.iter().zip(&stacks) {
        let (sx, sy) = (px + slot.x, py + slot.y);
        slot_background(b, sx, sy);
        draw_stack(b, font, sx, sy, *stack);
        if hovered == Some(slot.kind) {
            b.rect(sx, sy, 16.0, 16.0, rgba(0xFFFFFF, 110));
            if !stack.is_empty() && cursor.is_empty() {
                tip = Some(pretty_name(stack.def().name));
            }
        }
    }
    if let Some(text) = tip {
        tooltip(b, font, w, h, m, &text);
    }
    draw_stack(b, font, (m.x - 8.0).floor(), (m.y - 8.0).floor(), cursor);
    Action::None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_fit_their_windows() {
        let pos = IVec3::new(0, 0, 0);
        let palette: Vec<ItemId> = (1..200).collect();
        for screen in [Screen::Inventory, Screen::Crafting, Screen::Furnace(pos), Screen::Chest(pos), Screen::Creative] {
            let (pw, ph, slots) = layout(screen, &palette, 2);
            assert!(!slots.is_empty());
            for (i, a) in slots.iter().enumerate() {
                assert!(a.x >= 7.0 && a.y >= 7.0 && a.x + 16.0 <= pw - 6.0 && a.y + 16.0 <= ph - 7.0, "{screen:?} slot {i} outside the window");
                for b in &slots[i + 1..] {
                    assert!((a.x - b.x).abs() >= 18.0 || (a.y - b.y).abs() >= 18.0, "{screen:?} slots overlap");
                    assert_ne!(a.kind, b.kind);
                }
            }
        }
        // Every inventory slot is reachable from the normal screens.
        let (_, _, slots) = layout(Screen::Inventory, &[], 0);
        for i in 0..36 {
            assert!(slots.iter().any(|s| s.kind == SlotKind::Inv(i)));
        }
    }

    #[test]
    fn creative_tabs_cover_every_item() {
        let total = reg().items.defs.len() - 1;
        assert_eq!(creative_list(0, "").len(), total);
        let by_tab: usize = (1..TAB_NAMES.len()).map(|t| creative_list(t, "").len()).sum();
        assert_eq!(by_tab, total, "each item belongs to exactly one category tab");
        for t in 1..TAB_NAMES.len() {
            assert!(!creative_list(t, "").is_empty(), "tab {} is empty", TAB_NAMES[t]);
        }
        let hits = creative_list(0, "oak pl");
        assert!(hits.iter().any(|id| reg().items.defs[*id as usize].name == "oak_planks"));
        assert!(creative_list(0, "zzzz").is_empty());
        for icon in TAB_ICONS {
            assert!(!ItemStack::named(icon, 1).is_empty(), "tab icon {icon} is not an item");
        }
    }

    #[test]
    fn text_editing() {
        let mut input = Input::default();
        let mut text = String::from("ab");
        input.typed = vec!['c', '\u{7f}', 'd'];
        assert!(edit_text(&mut text, &input, 8));
        assert_eq!(text, "abcd");
        input.typed.clear();
        input.key_events = vec![key::BACKSPACE, key::BACKSPACE];
        assert!(edit_text(&mut text, &input, 8));
        assert_eq!(text, "ab");
        input.key_events.clear();
        assert!(!edit_text(&mut text, &input, 8));
        input.typed = vec!['x'; 20];
        edit_text(&mut text, &input, 8);
        assert_eq!(text.len(), 8);
    }
}
