//! The game: owns the world, the player and the renderer, runs the 20 Hz simulation and draws frames.

pub mod entities;
pub mod geyser;
pub mod hud;
pub mod inventory;
pub mod mobs;
pub mod physics;
pub mod player;
pub mod save;
pub mod ui;

use crate::audio::{self, Sfx};
use crate::ffi::{MAX_Y, MIN_Y, SEA_LEVEL};
use crate::gfx::batch::{rgba, Batch, Rgba, WHITE};
use crate::gfx::{gl, Camera, Environment, Renderer};
use crate::input::{key, Event, Input, MOUSE_LEFT, MOUSE_MIDDLE, MOUSE_RIGHT};
use crate::item::{self, ItemDef, ItemStack};
use crate::platform;
use crate::util::math::{look_dir, Aabb, DVec3, IVec3, Vec3, UP};
use crate::util::rng::{hash_str, Rng};
use crate::world::block::*;
use crate::world::model::{Box6, Shape};
use crate::world::rules::{self, Placement};
use crate::world::sim::Sim;
use crate::world::World;
use entities::{merge_items, BlockEntity, Furnace, ItemEntity, MovingBlock, MovingEvent, Particle, Vitals};
use inventory::Inventory;
use mobs::{Kind, Mob};
use physics::{raycast, Hit};
use player::{Controls, GameMode, Player};
use std::collections::{HashMap, HashSet};
use ui::{Action, Mouse, Screen, Ui};

pub const TICK_MS: f64 = 50.0;
pub const DAY_TICKS: u64 = 24000;
/// The world is written to storage this often (vanilla saves every five minutes).
const AUTOSAVE_TICKS: u64 = 6000;
const MAX_DROPS: usize = 1500;

pub struct Settings {
    pub render_distance: i32,
    pub fov: f32,
    pub sensitivity: f32,
    /// 0 = automatic.
    pub gui_scale: f32,
    pub gamma: f32,
    pub bobbing: bool,
    /// Peaceful difficulty: no monsters.
    pub peaceful: bool,
    /// Master volume, 0 to 1.
    pub volume: f32,
}

impl Settings {
    fn load() -> Settings {
        let mut s = Settings { render_distance: 8, fov: 70.0, sensitivity: 0.15, gui_scale: 0.0, gamma: 0.5, bobbing: true, peaceful: false, volume: 1.0 };
        if let Some(data) = platform::storage_read("options.txt") {
            for line in String::from_utf8_lossy(&data).lines() {
                let Some((k, v)) = line.split_once(':') else { continue };
                let Ok(v) = v.trim().parse::<f32>() else { continue };
                if !v.is_finite() {
                    continue;
                }
                match k.trim() {
                    "fov" => s.fov = v.clamp(30.0, 110.0),
                    "renderDistance" => s.render_distance = (v as i32).clamp(2, 32),
                    "mouseSensitivity" => s.sensitivity = v.clamp(0.0075, 0.3),
                    "guiScale" => s.gui_scale = v.clamp(0.0, 4.0).floor(),
                    "gamma" => s.gamma = v.clamp(0.0, 1.0),
                    "bobView" => s.bobbing = v != 0.0,
                    "difficulty" => s.peaceful = v == 0.0,
                    "soundVolume" => s.volume = v.clamp(0.0, 1.0),
                    _ => {}
                }
            }
        }
        s
    }
    fn save(&self) {
        let text = format!("fov:{}\nrenderDistance:{}\nmouseSensitivity:{}\nguiScale:{}\ngamma:{}\nbobView:{}\ndifficulty:{}\nsoundVolume:{}\n", self.fov, self.render_distance, self.sensitivity, self.gui_scale, self.gamma, self.bobbing as u8, if self.peaceful { 0 } else { 2 }, self.volume);
        platform::storage_write("options.txt", text.as_bytes());
    }
}

/// Progress on the block currently being mined.
#[derive(Clone, Copy, Debug)]
pub struct Mining {
    pub pos: IVec3,
    pub ticks: u32,
    pub total: u32,
}

pub struct Game {
    pub world: World,
    pub renderer: Renderer,
    pub player: Player,
    pub input: Input,
    pub settings: Settings,
    pub tick: u64,
    pub day_time: u64,
    pub time_frozen: bool,
    accumulator: f64,
    last_frame: f64,
    pub grabbed: bool,
    pub inv: Inventory,
    pub vitals: Vitals,
    pub drops: Vec<ItemEntity>,
    pub block_entities: HashMap<IVec3, BlockEntity>,
    pub sim: Sim,
    /// Falling blocks and primed TNT.
    pub moving: Vec<MovingBlock>,
    pub particles: Vec<Particle>,
    pub mobs: Vec<Mob>,
    pub geysers: Vec<geyser::Geyser>,
    /// Mob under the crosshair, if one is nearer than the targeted block.
    pub target_mob: Option<usize>,
    pub screen: Screen,
    pub ui: Ui,
    /// True while the title screen and its sub-menus are shown over the backdrop world.
    pub at_title: bool,
    pub mining: Option<Mining>,
    pub spawn: DVec3,
    pub world_id: Option<String>,
    pub world_name: String,
    pub rng: Rng,
    pub target: Option<Hit>,
    pub show_debug: bool,
    pub hide_hud: bool,
    break_cooldown: u8,
    place_cooldown: u8,
    eating: u8,
    /// Clicks that happened since the last tick, so a quick click between two ticks still counts.
    clicked: [bool; 2],
    /// Ticks left of the arm swing animation.
    pub swing: u8,
    bob: f32,
    prev_bob: f32,
    last_space_tick: u64,
    last_forward_tick: u64,
    last_save_tick: u64,
    fov_scale: f32,
    fps: f32,
    frame_count: u32,
    fps_timer: f64,
    pub messages: Vec<(String, f64)>,
    pub loading: bool,
    frame_ms: f32,
    /// Time the selected hotbar slot last changed (for the item name popup).
    held_changed: f64,
    /// Distance walked since the last footstep sound.
    step_dist: f32,
    last_health: f32,
    scratch: Vec<Aabb>,
}

/// Parses a seed the way players expect: numbers are used directly, other text is hashed.
pub fn parse_seed(text: &str) -> u64 {
    let t = text.trim();
    t.parse::<i64>().map(|v| v as u64).unwrap_or_else(|_| hash_str(t))
}

/// Adds a textured box to a world-space batch.  `c` is the centre relative to the camera and
/// `yaw` a rotation about the vertical axis in radians.
fn draw_cube(b: &mut Batch, c: Vec3, half: f32, yaw: f32, layers: Option<&[u16]>, tints: [Rgba; 2], light: [f32; 3]) {
    const FACES: [[[f32; 3]; 4]; 6] = [
        [[-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [1.0, -1.0, 1.0], [-1.0, -1.0, 1.0]],
        [[-1.0, 1.0, 1.0], [1.0, 1.0, 1.0], [1.0, 1.0, -1.0], [-1.0, 1.0, -1.0]],
        [[1.0, -1.0, -1.0], [-1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, -1.0]],
        [[-1.0, -1.0, 1.0], [1.0, -1.0, 1.0], [1.0, 1.0, 1.0], [-1.0, 1.0, 1.0]],
        [[-1.0, -1.0, -1.0], [-1.0, -1.0, 1.0], [-1.0, 1.0, 1.0], [-1.0, 1.0, -1.0]],
        [[1.0, -1.0, 1.0], [1.0, -1.0, -1.0], [1.0, 1.0, -1.0], [1.0, 1.0, 1.0]],
    ];
    const SHADE: [f32; 6] = [0.5, 1.0, 0.8, 0.8, 0.6, 0.6];
    let (sin, cos) = yaw.sin_cos();
    for (f, corners) in FACES.iter().enumerate() {
        let tint = tints[(f != 1) as usize];
        let color = [(tint[0] as f32 * light[0] * SHADE[f]) as u8, (tint[1] as f32 * light[1] * SHADE[f]) as u8, (tint[2] as f32 * light[2] * SHADE[f]) as u8, 255];
        let pos = corners.map(|p| [c.x + (p[0] * cos - p[2] * sin) * half, c.y + p[1] * half, c.z + (p[0] * sin + p[2] * cos) * half]);
        b.quad(pos, [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]], layers.map_or(crate::gfx::batch::LAYER_NONE, |l| l[f] as f32), color);
    }
}

/// Adds an upright textured square (flat items) to a world-space batch.
fn draw_sprite(b: &mut Batch, c: Vec3, half: f32, yaw: f32, layer: u16, tint: Rgba, light: [f32; 3]) {
    let (sin, cos) = yaw.sin_cos();
    let color = [(tint[0] as f32 * light[0]) as u8, (tint[1] as f32 * light[1]) as u8, (tint[2] as f32 * light[2]) as u8, 255];
    let p = |x: f32, y: f32| [c.x + x * cos * half, c.y + y * half, c.z + x * sin * half];
    b.quad([p(-1.0, -1.0), p(1.0, -1.0), p(1.0, 1.0), p(-1.0, 1.0)], [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]], layer as f32, color);
}

/// Adds the six faces of a block-local box, textured in block space (for the breaking overlay).
fn draw_box_faces(b: &mut Batch, origin: Vec3, bx: &Box6, grow: f32, layer: f32, color: Rgba) {
    let lo = [bx[0] - grow, bx[1] - grow, bx[2] - grow];
    let hi = [bx[3] + grow, bx[4] + grow, bx[5] + grow];
    for (axis, ua, va) in [(1usize, 0usize, 2usize), (2, 0, 1), (0, 2, 1)] {
        for side in [lo[axis], hi[axis]] {
            let corner = |u: f32, v: f32| {
                let mut p = [0.0f32; 3];
                p[axis] = side;
                p[ua] = u;
                p[va] = v;
                p
            };
            let ps = [corner(lo[ua], lo[va]), corner(hi[ua], lo[va]), corner(hi[ua], hi[va]), corner(lo[ua], hi[va])];
            b.quad(ps.map(|p| [origin.x + p[0], origin.y + p[1], origin.z + p[2]]), ps.map(|p| [p[ua], 1.0 - p[va]]), layer, color);
        }
    }
}

impl Game {
    pub fn new(width: i32, height: i32) -> Game {
        let seed = (platform::now_ms() * 1000.0) as u64 ^ 0x5eed_c0de;
        let mut game = Game {
            world: World::new(seed),
            renderer: Renderer::new(width, height),
            player: Player::new(DVec3::new(0.5, 100.0, 0.5)),
            input: Input::default(),
            settings: Settings::load(),
            tick: 0,
            day_time: 1000,
            time_frozen: false,
            accumulator: 0.0,
            last_frame: -1.0,
            grabbed: false,
            inv: Inventory::default(),
            vitals: Vitals::default(),
            drops: Vec::new(),
            block_entities: HashMap::new(),
            sim: Sim::new(seed),
            moving: Vec::new(),
            particles: Vec::new(),
            mobs: Vec::new(),
            geysers: Vec::new(),
            target_mob: None,
            screen: Screen::Title,
            ui: Ui::default(),
            at_title: true,
            mining: None,
            spawn: DVec3::ZERO,
            world_id: None,
            world_name: String::new(),
            rng: Rng::new(seed ^ 0x1234_5678_9abc),
            target: None,
            show_debug: false,
            hide_hud: false,
            break_cooldown: 0,
            place_cooldown: 0,
            eating: 0,
            clicked: [false; 2],
            swing: 0,
            bob: 0.0,
            prev_bob: 0.0,
            last_space_tick: 0,
            last_forward_tick: 0,
            last_save_tick: 0,
            fov_scale: 1.0,
            fps: 0.0,
            frame_count: 0,
            fps_timer: 0.0,
            messages: Vec::new(),
            loading: true,
            frame_ms: 0.0,
            held_changed: -1e9,
            step_dist: 0.0,
            last_health: 20.0,
            scratch: Vec::new(),
        };
        audio::init();
        audio::set_volume(game.settings.volume);
        game.enter_title();
        game
    }

    fn random_seed(&mut self) -> u64 {
        self.rng.next_u64() ^ (platform::now_ms() * 1000.0) as u64
    }

    /// Throws away the current world and everything in it (without saving).
    fn reset_world(&mut self, seed: u64) {
        self.renderer.chunks.clear();
        self.world = World::new(seed);
        self.world.track_changes = true;
        self.world.animate_falls = true;
        self.sim = Sim::new(seed);
        self.moving.clear();
        self.particles.clear();
        self.mobs.clear();
        self.geysers.clear();
        self.target_mob = None;
        self.drops.clear();
        self.block_entities.clear();
        self.inv = Inventory::default();
        self.vitals = Vitals::default();
        self.ui.cursor = ItemStack::EMPTY;
        self.ui.craft = [ItemStack::EMPTY; 9];
        self.mining = None;
        self.target = None;
        self.messages.clear();
        self.world_id = None;
        self.world_name.clear();
        self.tick = 0;
        self.last_save_tick = 0;
        self.day_time = 1000;
        self.time_frozen = false;
        self.accumulator = 0.0;
        self.loading = true;
    }

    /// Replaces the world with a fresh one and puts the player at its spawn point.
    pub fn new_world(&mut self, seed: u64) {
        self.reset_world(seed);
        let (sx, sz) = self.find_spawn();
        self.world.ensure_area(sx, sz, 1);
        let (sx, y, sz) = self.settle_spawn(sx, sz);
        self.spawn = DVec3::new(sx as f64 + 0.5, y as f64, sz as f64 + 0.5);
        self.player = Player::new(self.spawn);
        platform::log(&format!("World seed {}: spawn at {sx} {y} {sz}", seed as i64));
    }

    /// Creates the world described on the "Create New World" screen and starts playing it.
    fn create_world(&mut self) {
        let name = match self.ui.name.trim() {
            "" => "New World".to_string(),
            n => n.to_string(),
        };
        let seed = if self.ui.seed.trim().is_empty() { self.random_seed() } else { parse_seed(&self.ui.seed) };
        let id = save::new_id(&name);
        self.new_world(seed);
        self.player.mode = self.ui.create_mode;
        self.world_name = name;
        self.world.store = Some(save::prefix(&id));
        self.world_id = Some(id);
        self.at_title = false;
        self.screen = Screen::None;
        platform::set_cursor_grab(true);
        self.save_world();
    }

    /// Saves the current world and shows the title screen over a backdrop world.
    fn enter_title(&mut self) {
        self.save_world();
        let seed = self.random_seed();
        self.new_world(seed);
        self.day_time = 5200;
        self.at_title = true;
        self.screen = Screen::Title;
        self.ui.fresh = true;
        self.grabbed = false;
        platform::set_cursor_grab(false);
    }

    /// Picks the exact spawn block near a candidate column: open ground rather than a tree top.
    fn settle_spawn(&self, sx: i32, sz: i32) -> (i32, i32, i32) {
        let r = reg();
        let mut offsets: Vec<(i32, i32)> = (-12..=12).flat_map(|dz| (-12..=12).map(move |dx| (dx, dz))).collect();
        offsets.sort_by_key(|(dx, dz)| dx * dx + dz * dz);
        for (dx, dz) in offsets {
            let (x, z) = (sx + dx, sz + dz);
            let Some(y) = self.world.find_standing_y(x, z) else { continue };
            let ground = self.world.get(x, y - 1, z);
            let open = (0..2).all(|i| self.world.get(x, y + i, z) == AIR || r.has(self.world.get(x, y + i, z), F_REPLACEABLE) && !r.has(self.world.get(x, y + i, z), F_FLUID));
            if r.has(ground, F_OPAQUE) && !r.has(ground, F_LEAVES) && open && self.world.light(x, y + 1, z) >> 4 == 15 {
                return (x, y, z);
            }
        }
        (sx, self.world.find_standing_y(sx, sz).unwrap_or(self.world.top(sx, sz) + 1), sz)
    }

    /// Looks for dry land near the origin, spiralling outwards.
    fn find_spawn(&self) -> (i32, i32) {
        let mut best = (0, 0);
        'search: for ring in 0..160 {
            let r = ring * 24;
            let steps = (ring * 8).max(1);
            for i in 0..steps {
                let a = i as f32 / steps as f32 * std::f32::consts::TAU;
                let (x, z) = ((a.cos() * r as f32) as i32, (a.sin() * r as f32) as i32);
                let c = self.world.gen.column(x, z);
                let b = self.world.gen.biome_at(x, z);
                if c.base_height > SEA_LEVEL as f32 + 1.5 && c.base_height < 110.0 && !b.is_ocean() && b.name() != "river" && b.name() != "frozen_river" {
                    best = (x, z);
                    break 'search;
                }
            }
        }
        best
    }

    pub fn say(&mut self, text: impl Into<String>) {
        let text = text.into();
        platform::log(&text);
        self.messages.push((text, platform::now_ms()));
        if self.messages.len() > 100 {
            self.messages.remove(0);
        }
    }

    fn in_play(&self) -> bool {
        self.screen == Screen::None && !self.at_title
    }

    pub fn handle(&mut self, ev: Event) {
        match ev {
            Event::Resize(w, h) => self.renderer.resize(w, h),
            Event::Grab(on) => {
                if !on && self.grabbed && self.in_play() && !self.loading {
                    // The platform took the cursor away (Escape in a browser): show the game menu.
                    self.open_screen(Screen::Pause);
                    self.ui.esc_guard = platform::now_ms() + 250.0;
                }
                self.grabbed = on;
            }
            Event::Focus(false) => {
                if self.in_play() && self.grabbed && !self.loading {
                    self.open_screen(Screen::Pause);
                }
                self.grabbed = false;
            }
            _ => {}
        }
        self.input.handle(&ev);
    }

    /// Saves the open world and the options without leaving the game.
    pub fn persist(&mut self) {
        self.save_world();
        self.settings.save();
        self.last_save_tick = self.tick;
    }

    /// Called once when the game is closing; saves anything that must persist.
    pub fn shutdown(&mut self) {
        self.persist();
        platform::log("Shutting down");
    }

    /// Numeric status values for tooling (see `rc_query` in the web back end).
    pub fn query(&self, what: u32) -> f64 {
        match what {
            0 => self.loading as u32 as f64,
            1 => self.fps as f64,
            2 => self.player.pos.x,
            3 => self.player.pos.y,
            4 => self.player.pos.z,
            5 => self.renderer.chunks.stats.chunks as f64,
            6 => self.world.chunks.len() as f64,
            7 => {
                // Chunks inside the render distance whose meshes are still out of date.
                let (cx, cz) = ((self.player.pos.x.floor() as i32) >> 4, (self.player.pos.z.floor() as i32) >> 4);
                let rd = self.settings.render_distance;
                let mut pending = 0;
                for dz in -rd..=rd {
                    for dx in -rd..=rd {
                        match self.world.chunks.get(&(cx + dx, cz + dz)) {
                            Some(c) if c.stage >= crate::world::chunk::STAGE_LIT && c.mesh_dirty == 0 => {}
                            _ => pending += 1,
                        }
                    }
                }
                pending as f64
            }
            8 => match self.screen {
                Screen::None => 0.0,
                Screen::Title => 1.0,
                Screen::Worlds => 2.0,
                Screen::CreateWorld => 3.0,
                Screen::Pause => 4.0,
                Screen::Options => 5.0,
                Screen::Inventory => 6.0,
                Screen::Creative => 7.0,
                Screen::Crafting => 8.0,
                Screen::Furnace(_) => 9.0,
                Screen::Chest(_) => 10.0,
                Screen::Chat => 11.0,
                Screen::Death => 12.0,
            },
            9 => self.vitals.health as f64,
            10 => self.vitals.food as f64,
            11 => self.drops.len() as f64,
            12 => self.inv.slots.iter().map(|s| s.count as u32).sum::<u32>() as f64,
            13 => self.inv.held().id as f64,
            14 => self.inv.held().count as f64,
            15 => self.tick as f64,
            16 => self.grabbed as u32 as f64,
            17 => self.moving.len() as f64,
            18 => self.particles.len() as f64,
            19 => self.sim.pending() as f64,
            20 => self.mobs.len() as f64,
            21 => self.mobs.iter().filter(|m| m.kind.hostile()).count() as f64,
            22 => self.mobs.first().map_or(-1.0, |m| m.health as f64),
            23 => self.mobs.first().map_or(-1.0, |m| (m.pos - self.player.pos).length()),
            24 => self.geysers.len() as f64,
            25 => self.geysers.iter().filter(|g| g.erupting > 0).count() as f64,
            _ => -1.0,
        }
    }

    pub fn gui_scale(&self) -> f32 {
        let fit = ((self.renderer.width as f32 / 320.0).min(self.renderer.height as f32 / 240.0)).floor().clamp(1.0, 4.0);
        if self.settings.gui_scale >= 1.0 { self.settings.gui_scale.min(fit) } else { fit }
    }

    // ------------------------------------------------------------ screens
    pub fn open_screen(&mut self, screen: Screen) {
        if screen == Screen::Options && self.screen != Screen::Options {
            self.ui.options_return = self.screen;
        }
        if self.screen == Screen::Options && screen != Screen::Options {
            self.settings.save();
        }
        match screen {
            Screen::Worlds => {
                self.ui.worlds = save::list_worlds();
                self.ui.selected_world = if self.ui.worlds.is_empty() { None } else { Some(0) };
                self.ui.scroll = 0;
                self.ui.confirm_delete = false;
            }
            Screen::CreateWorld => {
                self.ui.name = "New World".to_string();
                self.ui.seed.clear();
                self.ui.focus = 0;
            }
            Screen::Chat => self.ui.history_pos = self.ui.history.len(),
            Screen::Creative => {
                self.ui.text.clear();
                self.ui.scroll = 0;
            }
            _ => {}
        }
        self.screen = screen;
        self.ui.fresh = true;
        self.ui.drag = None;
        self.mining = None;
        self.eating = 0;
        self.clicked = [false; 2];
        if screen != Screen::None {
            self.grabbed = false;
            platform::set_cursor_grab(false);
        }
    }

    /// Returns to the game, putting carried and crafting-grid items back into the inventory.
    pub fn close_screen(&mut self) {
        if self.screen.is_container() {
            let mut back: Vec<ItemStack> = self.ui.craft.iter_mut().map(std::mem::take).collect();
            back.push(std::mem::take(&mut self.ui.cursor));
            for s in back {
                let rest = self.inv.add(s);
                self.throw_item(rest);
            }
        }
        self.screen = Screen::None;
        self.ui.text.clear();
        self.held_changed = -1e9;
        platform::set_cursor_grab(true);
    }

    fn perform(&mut self, action: Action) {
        match action {
            Action::None => {}
            Action::Open(screen) => self.open_screen(screen),
            Action::Close => self.close_screen(),
            Action::Create => self.create_world(),
            Action::Play(i) => {
                if let Some(info) = self.ui.worlds.get(i).cloned() {
                    if self.load_world(&info.id) {
                        self.at_title = false;
                        self.screen = Screen::None;
                        platform::set_cursor_grab(true);
                    } else {
                        platform::log(&format!("Could not load world '{}'", info.name));
                    }
                }
            }
            Action::Delete(i) => {
                if let Some(info) = self.ui.worlds.get(i).cloned() {
                    save::delete_world(&info.id);
                    platform::log(&format!("Deleted world '{}'", info.name));
                }
                self.open_screen(Screen::Worlds);
            }
            Action::QuitToTitle => self.enter_title(),
            Action::Quit => {
                self.shutdown();
                platform::request_quit();
            }
            Action::Respawn => self.respawn(),
            Action::Chat(text) => {
                self.close_screen();
                if !text.is_empty() {
                    if self.ui.history.last() != Some(&text) {
                        self.ui.history.push(text.clone());
                    }
                    match text.strip_prefix('/') {
                        Some(command) => self.run_command(command),
                        None => self.say(format!("<Player> {text}")),
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ commands
    /// Executes a chat command given without the leading slash.
    pub fn run_command(&mut self, line: &str) {
        let r = reg();
        let args: Vec<&str> = line.split_whitespace().collect();
        let num = |i: usize| args.get(i).and_then(|s| s.parse::<f64>().ok());
        match args.first().copied().unwrap_or("") {
            "tp" | "teleport" => match (num(1), num(2), num(3)) {
                (Some(x), Some(y), Some(z)) => {
                    self.player.teleport(DVec3::new(x, y, z));
                    self.loading = true;
                    self.say(format!("Teleported to {x:.1} {y:.1} {z:.1}"));
                }
                (Some(x), Some(z), None) => {
                    self.world.ensure_area(x.floor() as i32, z.floor() as i32, 1);
                    let y = self.world.find_standing_y(x.floor() as i32, z.floor() as i32).unwrap_or(self.world.top(x.floor() as i32, z.floor() as i32) + 1);
                    self.player.teleport(DVec3::new(x, y as f64, z));
                    self.loading = true;
                    self.say(format!("Teleported to {x:.1} {y} {z:.1}"));
                }
                _ => self.say("Usage: /tp <x> <y> <z>  or  /tp <x> <z>"),
            },
            "look" => {
                if let (Some(yaw), Some(pitch)) = (num(1), num(2)) {
                    self.player.yaw = yaw as f32;
                    self.player.pitch = (pitch as f32).clamp(-90.0, 90.0);
                }
            }
            "time" => {
                let value = match args.get(2).copied().or(args.get(1).copied()) {
                    Some("day") => Some(1000),
                    Some("noon") => Some(6000),
                    Some("sunset") => Some(12000),
                    Some("night") => Some(13000),
                    Some("midnight") => Some(18000),
                    Some("sunrise") => Some(23000),
                    Some(n) => n.parse::<u64>().ok(),
                    None => None,
                };
                match (args.get(1).copied(), value) {
                    (Some("freeze"), _) => {
                        self.time_frozen = !self.time_frozen;
                        self.say(format!("Daylight cycle {}", if self.time_frozen { "frozen" } else { "running" }));
                    }
                    (Some("add"), Some(v)) => self.day_time += v,
                    (_, Some(v)) => {
                        self.day_time = self.day_time / DAY_TICKS * DAY_TICKS + v % DAY_TICKS;
                        self.say(format!("Set the time to {v}"));
                    }
                    _ => self.say(format!("The time is {}", self.day_time % DAY_TICKS)),
                }
            }
            "gamemode" => {
                let mode = match args.get(1).copied() {
                    Some("survival") | Some("s") | Some("0") => Some(GameMode::Survival),
                    Some("creative") | Some("c") | Some("1") => Some(GameMode::Creative),
                    Some("spectator") | Some("sp") | Some("3") => Some(GameMode::Spectator),
                    _ => None,
                };
                match mode {
                    Some(m) => {
                        self.player.mode = m;
                        self.mining = None;
                        if m != GameMode::Creative && m != GameMode::Spectator {
                            self.player.flying = false;
                        }
                        self.say(format!("Set own game mode to {m:?}"));
                    }
                    None => self.say("Usage: /gamemode <survival|creative|spectator>"),
                }
            }
            "seed" => self.say(format!("Seed: [{}]", self.world.seed as i64)),
            "newworld" => {
                // Developer shortcut: a throw-away world that is never saved.
                let seed = args.get(1).map(|s| parse_seed(s)).unwrap_or(self.world.seed.wrapping_add(1));
                self.new_world(seed);
                self.player.mode = match args.get(2).copied() {
                    Some("survival") | Some("s") => GameMode::Survival,
                    _ => GameMode::Creative,
                };
                self.at_title = false;
                self.screen = Screen::None;
                platform::set_cursor_grab(true);
            }
            "give" => {
                // /give [@s] <item> [count]
                let i = if args.get(1).map_or(false, |a| a.starts_with('@') || *a == "Player") { 2 } else { 1 };
                match args.get(i).and_then(|n| r.items.id(n)) {
                    Some(id) => {
                        let max = r.items.defs[id as usize].max_stack.max(1) as u32;
                        let count = num(i + 1).unwrap_or(1.0).clamp(1.0, 2304.0) as u32;
                        let mut left = count;
                        while left > 0 {
                            let n = left.min(max);
                            let rest = self.inv.add(ItemStack::new(id, n as u8));
                            self.throw_item(rest);
                            left -= n;
                        }
                        self.say(format!("Gave {count} [{}] to Player", hud::pretty_name(r.items.defs[id as usize].name)));
                    }
                    None => self.say("Usage: /give <item> [count]"),
                }
            }
            "clear" => {
                let n: u32 = self.inv.slots.iter().map(|s| s.count as u32).sum();
                self.inv.clear();
                self.say(if n > 0 { format!("Removed {n} item(s) from player Player") } else { "No items were found on player Player".to_string() });
            }
            "kill" if args.get(1).map_or(false, |a| a.starts_with("@e")) => {
                let n = self.mobs.len();
                self.mobs.clear();
                self.say(format!("Killed {n} entities"));
            }
            "summon" => match args.get(1).and_then(|n| Kind::from_name(n)) {
                Some(kind) => {
                    let pos = match (num(2), num(3), num(4)) {
                        (Some(x), Some(y), Some(z)) => DVec3::new(x, y, z),
                        _ => {
                            let (sin, cos) = (self.player.yaw as f64).to_radians().sin_cos();
                            self.player.pos + DVec3::new(-sin * 2.5, 0.2, cos * 2.5)
                        }
                    };
                    let persistent = kind == Kind::SulfurCube;
                    if let Some(m) = self.spawn_mob(kind, pos) {
                        m.persistent = persistent;
                    }
                    self.say(format!("Summoned new {}", hud::pretty_name(kind.name())));
                }
                None => self.say("Usage: /summon <pig|cow|sheep|chicken|zombie|creeper|spider|sulfur_cube> [x y z]"),
            },
            "difficulty" => {
                match args.get(1).copied() {
                    Some("peaceful") | Some("0") => self.settings.peaceful = true,
                    Some("easy") | Some("normal") | Some("hard") | Some("1") | Some("2") | Some("3") => self.settings.peaceful = false,
                    _ => {}
                }
                self.say(format!("The difficulty is {}", if self.settings.peaceful { "Peaceful" } else { "Normal" }));
            }
            "kill" => {
                self.vitals.hurt_time = 0;
                self.vitals.damage(1000.0, "fell out of the world");
                self.on_death();
            }
            "spawnpoint" => {
                self.spawn = self.player.pos;
                self.say(format!("Set spawn point to {:.0}, {:.0}, {:.0}", self.spawn.x, self.spawn.y, self.spawn.z));
            }
            "save-all" | "save" => {
                let text = if self.save_world() { "Saved the game" } else { "This world has no save slot (it was started with /newworld)" };
                self.say(text);
            }
            "help" | "?" => {
                for line in ["/gamemode <survival|creative|spectator>   /give <item> [count]   /clear   /kill", "/tp <x> <y> <z>   /time set <day|noon|night|ticks>   /seed   /spawnpoint", "/setblock <x> <y> <z> <block>   /fill <x1> <y1> <z1> <x2> <y2> <z2> <block>   /save-all", "/summon <mob> [x y z]   /kill @e   /difficulty <peaceful|normal>"] {
                    self.say(line);
                }
            }
            "rd" | "renderdistance" => {
                if let Some(v) = num(1) {
                    self.settings.render_distance = (v as i32).clamp(2, 32);
                    self.say(format!("Render distance: {} chunks", self.settings.render_distance));
                }
            }
            "fov" => {
                if let Some(v) = num(1) {
                    self.settings.fov = (v as f32).clamp(30.0, 110.0);
                }
            }
            "hud" => self.hide_hud = !self.hide_hud,
            "debug" => self.show_debug = !self.show_debug,
            "fly" => {
                self.player.flying = !self.player.flying;
                self.player.on_ground = false;
            }
            "setblock" => match (num(1), num(2), num(3), args.get(4).and_then(|s| r.parse_state(s))) {
                (Some(x), Some(y), Some(z), Some(state)) => {
                    let p = IVec3::new(x.floor() as i32, y.floor() as i32, z.floor() as i32);
                    if self.world.set_at(p, state).is_some() {
                        rules::notify_neighbours(&mut self.world, p);
                        self.say(format!("Changed the block at {} {} {}", p.x, p.y, p.z));
                    } else {
                        self.say("That position is not loaded");
                    }
                }
                _ => self.say("Usage: /setblock <x> <y> <z> <block[state]>"),
            },
            "fill" => {
                let c: Vec<i32> = (1..7).filter_map(|i| num(i).map(|v| v.floor() as i32)).collect();
                match (c.len(), args.get(7).and_then(|s| r.parse_state(s))) {
                    (6, Some(state)) => {
                        let (x0, x1, y0, y1, z0, z1) = (c[0].min(c[3]), c[0].max(c[3]), c[1].min(c[4]), c[1].max(c[4]), c[2].min(c[5]), c[2].max(c[5]));
                        let volume = (x1 - x0 + 1) as i64 * (y1 - y0 + 1) as i64 * (z1 - z0 + 1) as i64;
                        if volume > 32768 {
                            self.say(format!("Too many blocks in the specified area (maximum 32768, specified {volume})"));
                        } else {
                            let mut n = 0;
                            for y in y0..=y1 {
                                for z in z0..=z1 {
                                    for x in x0..=x1 {
                                        if self.world.get(x, y, z) != state && self.world.set_block(x, y, z, state).is_some() {
                                            n += 1;
                                        }
                                    }
                                }
                            }
                            self.say(format!("Successfully filled {n} blocks"));
                        }
                    }
                    _ => self.say("Usage: /fill <x1> <y1> <z1> <x2> <y2> <z2> <block[state]>"),
                }
            }
            "hotbar" => {
                // /hotbar <slot 1-9> <item>: fills a hotbar slot (testing aid)
                match (num(1), args.get(2).and_then(|s| r.items.id(s))) {
                    (Some(slot), Some(id)) if (1.0..=9.0).contains(&slot) => self.inv.slots[slot as usize - 1] = ItemStack::new(id, r.items.defs[id as usize].max_stack),
                    _ => self.say("Usage: /hotbar <1-9> <item>"),
                }
            }
            "" => {}
            other => self.say(format!("Unknown command: {other}")),
        }
    }

    // ------------------------------------------------------------ simulation
    fn controls(&self) -> Controls {
        let i = &self.input;
        if !self.grabbed || !self.in_play() || self.vitals.dead {
            return Controls::default();
        }
        Controls {
            forward: i.down(key::W) as i32 as f32 - i.down(key::S) as i32 as f32,
            strafe: i.down(key::A) as i32 as f32 - i.down(key::D) as i32 as f32,
            jump: i.down(key::SPACE),
            sneak: i.down(key::SHIFT),
            sprint: i.down(key::CONTROL) || self.player.sprinting,
        }
    }

    fn reach(&self) -> f64 {
        if self.player.mode == GameMode::Creative { 5.0 } else { 4.5 }
    }

    fn update_target(&mut self, alpha: f64) {
        self.target_mob = None;
        if self.at_title || self.vitals.dead {
            self.target = None;
            return;
        }
        let eye = self.player.eye(alpha);
        let dir = look_dir(self.player.yaw, self.player.pitch).to_f64();
        let hit = raycast(&self.world, eye, dir, self.reach(), false);
        // A mob in front of the block takes the click instead.
        let block_dist = hit.map_or(self.reach(), |h| (h.point - eye).length());
        let mob_reach = if self.player.mode == GameMode::Creative { 5.0 } else { 3.0 };
        self.target_mob = self.mob_in_sight(eye, dir, block_dist.min(mob_reach)).map(|(i, _)| i);
        self.target = if self.target_mob.is_some() { None } else { hit };
    }

    pub fn spawn_drop(&mut self, pos: IVec3, stack: ItemStack) {
        if !stack.is_empty() && self.drops.len() < MAX_DROPS {
            let e = ItemEntity::from_block(pos, stack, &mut self.rng);
            self.drops.push(e);
        }
    }

    /// Tosses a stack out in front of the player.
    pub fn throw_item(&mut self, stack: ItemStack) {
        if !stack.is_empty() && self.drops.len() < MAX_DROPS {
            let dir = look_dir(self.player.yaw, self.player.pitch).to_f64();
            self.drops.push(ItemEntity::thrown(self.player.eye(1.0), dir, stack));
        }
    }

    /// Removes a block the way a player does: drops, tool wear and anything stored inside.
    fn break_block(&mut self, hit: Hit) {
        let r = reg();
        let survival = self.player.mode == GameMode::Survival;
        let block = r.block(hit.state);
        if block.hardness < 0.0 && survival {
            return;
        }
        let held_def = self.inv.held().def();
        self.block_particles(hit.pos, hit.state, 24);
        audio::play(audio::for_block(block.sound), 0.9, 0.8);
        let popped = rules::remove(&mut self.world, hit.pos);
        if let Some(be) = self.block_entities.remove(&hit.pos) {
            for s in be.contents() {
                self.spawn_drop(hit.pos, s);
            }
        }
        if survival {
            for s in item::block_drops(hit.state, held_def, &mut self.rng) {
                self.spawn_drop(hit.pos, s);
            }
            if held_def.durability > 0 && block.hardness > 0.0 {
                self.inv.held_mut().wear(if held_def.tool == Tool::Sword { 2 } else { 1 });
            }
            self.vitals.exhaustion += 0.005;
            // Ice melts into water when something is underneath it.
            if block.name == "ice" && held_def.tool != Tool::None {
                let below = self.world.get_at(hit.pos.add(0, -1, 0));
                if r.has(below, F_SOLID) || r.has(below, F_FLUID) {
                    self.world.set_at(hit.pos, r.k.water);
                }
            }
        }
        // Whatever was attached to the block falls off as an item, in every game mode.
        let hand = &r.items.defs[0];
        for (pos, state) in popped {
            if let Some(be) = self.block_entities.remove(&pos) {
                for s in be.contents() {
                    self.spawn_drop(pos, s);
                }
            }
            for s in item::block_drops(state, hand, &mut self.rng) {
                self.spawn_drop(pos, s);
            }
        }
        self.swing = 6;
    }

    /// Block a tool turns the targeted block into (tilling, path making, log stripping).
    fn tool_conversion(&self, def: &ItemDef, hit: &Hit) -> Option<State> {
        let r = reg();
        let name = r.block(hit.state).name;
        let open_above = self.world.get_at(hit.pos.offset(UP)) == AIR;
        match def.tool {
            Tool::Hoe if hit.face != 0 && open_above && matches!(name, "grass_block" | "dirt" | "dirt_path") => r.default_state("farmland"),
            Tool::Hoe if name == "coarse_dirt" => r.default_state("dirt"),
            Tool::Shovel if hit.face != 0 && open_above && matches!(name, "grass_block" | "dirt" | "coarse_dirt" | "podzol" | "mycelium" | "rooted_dirt") => r.default_state("dirt_path"),
            Tool::Axe if name.ends_with("_log") && !name.starts_with("stripped_") => {
                let stripped = r.default_state(&format!("stripped_{name}"))?;
                Some(r.with(stripped, "axis", r.prop(hit.state, "axis").unwrap_or(0)))
            }
            _ => None,
        }
    }

    /// Bone meal on a crop advances it a few stages.
    fn apply_bone_meal(&mut self, pos: IVec3) -> bool {
        let r = reg();
        let s = self.world.get_at(pos);
        if let Shape::Crop(stages) = r.block(s).shape {
            let age = r.prop(s, "age").unwrap_or(0);
            if age + 1 < stages as usize {
                let grown = (age + self.rng.range(2, 5) as usize).min(stages as usize - 1);
                self.world.set_at(pos, r.with(s, "age", grown));
                return true;
            }
        }
        false
    }

    /// Swaps one item of the held stack for another item (filling a bucket).
    fn replace_held_one(&mut self, new: ItemStack) {
        if self.inv.held().count <= 1 {
            *self.inv.held_mut() = new;
        } else {
            self.inv.consume_held();
            let rest = self.inv.add(new);
            self.throw_item(rest);
        }
    }

    /// Right click.  Returns true when something happened, so the caller applies the use delay.
    fn use_item(&mut self) -> bool {
        let r = reg();
        let held = self.inv.held();
        let def = held.def();
        let survival = self.player.mode == GameMode::Survival;
        let sneak = self.input.down(key::SHIFT);
        if let Some(hit) = self.target {
            if !sneak || held.is_empty() {
                match r.block(hit.state).name {
                    "crafting_table" => {
                        self.open_screen(Screen::Crafting);
                        return true;
                    }
                    "furnace" => {
                        self.block_entities.entry(hit.pos).or_insert_with(|| BlockEntity::Furnace(Furnace::default()));
                        self.open_screen(Screen::Furnace(hit.pos));
                        return true;
                    }
                    "chest" => {
                        self.block_entities.entry(hit.pos).or_insert_with(|| BlockEntity::Chest(Box::new([ItemStack::EMPTY; 27])));
                        self.open_screen(Screen::Chest(hit.pos));
                        return true;
                    }
                    _ => {}
                }
                if rules::interact(&mut self.world, hit.pos) {
                    self.swing = 6;
                    return true;
                }
            }
        }
        if held.is_empty() {
            return false;
        }
        // Buckets act on fluids, which the ordinary target ray passes through.
        match def.name {
            "sulfur_cube_bucket" => {
                let Some(hit) = self.target else { return false };
                let pos = if r.has(hit.state, F_REPLACEABLE) { hit.pos } else { physics::neighbour(hit.pos, hit.face) };
                self.release_cube(pos);
                if survival {
                    *self.inv.held_mut() = ItemStack::named("bucket", 1);
                }
                self.swing = 6;
                return true;
            }
            "bucket" => {
                let eye = self.player.eye(1.0);
                let dir = look_dir(self.player.yaw, self.player.pitch).to_f64();
                let Some(hit) = raycast(&self.world, eye, dir, self.reach(), true) else { return false };
                if !r.has(hit.state, F_FLUID) || r.prop(hit.state, "level") != Some(0) {
                    return false;
                }
                let filled = if r.block(hit.state).name == "water" { "water_bucket" } else { "lava_bucket" };
                self.world.set_at(hit.pos, AIR);
                rules::notify_neighbours(&mut self.world, hit.pos);
                if survival {
                    self.replace_held_one(ItemStack::named(filled, 1));
                }
                self.swing = 6;
                return true;
            }
            "water_bucket" | "lava_bucket" => {
                let Some(hit) = self.target else { return false };
                let pos = if r.has(hit.state, F_REPLACEABLE) { hit.pos } else { physics::neighbour(hit.pos, hit.face) };
                let existing = self.world.get_at(pos);
                if !(MIN_Y..MAX_Y).contains(&pos.y) || !(existing == AIR || r.has(existing, F_REPLACEABLE)) {
                    return false;
                }
                let fluid = if def.name == "water_bucket" { r.k.water } else { r.default_state("lava").unwrap_or(AIR) };
                if self.world.set_at(pos, fluid).is_none() {
                    return false;
                }
                rules::notify_neighbours(&mut self.world, pos);
                if survival {
                    *self.inv.held_mut() = ItemStack::named("bucket", 1);
                }
                self.swing = 6;
                return true;
            }
            _ => {}
        }
        let Some(hit) = self.target else { return false };
        if let Some(new_state) = self.tool_conversion(def, &hit) {
            self.world.set_at(hit.pos, new_state);
            rules::notify_neighbours(&mut self.world, hit.pos);
            if survival {
                self.inv.held_mut().wear(1);
            }
            self.swing = 6;
            return true;
        }
        if def.name == "flint_and_steel" {
            if r.block(hit.state).name != "tnt" {
                return false;
            }
            self.world.set_at(hit.pos, AIR);
            let tnt = MovingBlock::tnt(hit.pos, hit.state, 80, &mut self.rng);
            self.moving.push(tnt);
            if survival {
                self.inv.held_mut().wear(1);
            }
            self.swing = 6;
            return true;
        }
        if def.name == "bone_meal" {
            if self.apply_bone_meal(hit.pos) {
                if survival {
                    self.inv.consume_held();
                }
                self.swing = 6;
                return true;
            }
            return false;
        }
        let Some(bi) = def.block else { return false };
        let block = &r.blocks[bi as usize];
        let same_slab = block.name == r.block(hit.state).name && ((hit.face == 1 && r.prop(hit.state, "type") == Some(0)) || (hit.face == 0 && r.prop(hit.state, "type") == Some(1)));
        let same_snow = block.name == "snow" && r.block(hit.state).name == "snow" && hit.face == 1;
        let pos = if r.has(hit.state, F_REPLACEABLE) || same_slab || same_snow { hit.pos } else { physics::neighbour(hit.pos, hit.face) };
        if !(MIN_Y..MAX_Y).contains(&pos.y) {
            return false;
        }
        let existing = self.world.get_at(pos);
        let merge = r.block(existing).name == block.name && (r.prop(existing, "type").map_or(false, |t| t < 2) || block.name == "snow");
        if !r.has(existing, F_REPLACEABLE) && !merge {
            return false;
        }
        let placement = Placement { pos, face: hit.face, hit: hit.point, yaw: self.player.yaw, pitch: self.player.pitch };
        let Some(state) = rules::placement_state(&self.world, block, &placement) else { return false };
        // Do not place a solid block inside the player.
        let me = self.player.aabb();
        let blocked = r.collision(state).iter().any(|b| Aabb::new(pos.x as f64 + b[0] as f64, pos.y as f64 + b[1] as f64, pos.z as f64 + b[2] as f64, pos.x as f64 + b[3] as f64, pos.y as f64 + b[4] as f64, pos.z as f64 + b[5] as f64).intersects(&me));
        if blocked {
            return false;
        }
        if !rules::place(&mut self.world, block, &placement) {
            return false;
        }
        audio::play(audio::for_block(block.sound), 0.8, 0.8);
        if survival {
            self.inv.consume_held();
        }
        self.swing = 6;
        true
    }

    fn pick_block(&mut self) {
        let Some(hit) = self.target else { return };
        let r = reg();
        let Some(id) = r.items.for_block(r.block(hit.state)) else { return };
        if let Some(i) = self.inv.slots[..9].iter().position(|s| s.id == id) {
            self.inv.selected = i;
        } else if self.player.mode == GameMode::Creative {
            let slot = if self.inv.held().is_empty() { self.inv.selected } else { self.inv.slots[..9].iter().position(|s| s.is_empty()).unwrap_or(self.inv.selected) };
            self.inv.slots[slot] = ItemStack::new(id, 1);
            self.inv.selected = slot;
        } else if let Some(i) = self.inv.slots.iter().position(|s| s.id == id) {
            self.inv.slots.swap(i, self.inv.selected);
        }
        self.held_changed = platform::now_ms();
    }

    /// The player died: everything carried spills onto the ground.
    fn on_death(&mut self) {
        if !self.vitals.dead || self.screen == Screen::Death {
            return;
        }
        let pos = self.player.pos + DVec3::new(0.0, 0.5, 0.0);
        let mut stacks: Vec<ItemStack> = self.inv.slots.iter().copied().collect();
        stacks.extend(self.ui.craft.iter_mut().map(std::mem::take));
        stacks.push(std::mem::take(&mut self.ui.cursor));
        self.inv.clear();
        for s in stacks {
            if !s.is_empty() && self.drops.len() < MAX_DROPS {
                let vel = DVec3::new(self.rng.f64() * 0.3 - 0.15, 0.2 + self.rng.f64() * 0.1, self.rng.f64() * 0.3 - 0.15);
                self.drops.push(ItemEntity { pos, prev: pos, vel, stack: s, age: 0, delay: 40, on_ground: false });
            }
        }
        let message = format!("Player {}", self.vitals.death_message);
        self.say(message);
        self.open_screen(Screen::Death);
    }

    fn respawn(&mut self) {
        let (sx, sz) = (self.spawn.x.floor() as i32, self.spawn.z.floor() as i32);
        self.world.ensure_area(sx, sz, 1);
        // The spawn point may have been built over or dug out since it was set.
        let y = self.world.find_standing_y(sx, sz).map_or(self.spawn.y, |y| y as f64);
        let mode = self.player.mode;
        self.player = Player::new(DVec3::new(self.spawn.x, y, self.spawn.z));
        self.player.mode = mode;
        self.vitals = Vitals::default();
        self.loading = true;
        self.screen = Screen::None;
        platform::set_cursor_grab(true);
    }

    fn tick_drops(&mut self) {
        let mut scratch = std::mem::take(&mut self.scratch);
        let world = &self.world;
        self.drops.retain_mut(|e| e.tick(world, &mut scratch));
        self.scratch = scratch;
        if self.tick % 20 == 0 {
            merge_items(&mut self.drops);
        }
        if self.vitals.dead || self.player.mode == GameMode::Spectator {
            return;
        }
        let reach = self.player.aabb().expand(1.0, 0.5, 1.0);
        let mut picked = false;
        for e in self.drops.iter_mut() {
            if e.delay == 0 && reach.intersects(&e.aabb()) {
                let rest = self.inv.add(e.stack);
                picked |= rest != e.stack;
                e.stack = rest;
            }
        }
        if picked {
            self.drops.retain(|e| !e.stack.is_empty());
            let pitch = 1.3 + self.rng.f32() * 1.2;
            audio::play(Sfx::Pop, 0.25, pitch);
        }
    }

    /// How much darker the sky is than at noon, in light levels (0 by day, 11 at midnight).
    pub fn sky_darken(&self) -> u8 {
        let c = (self.celestial_angle(0.0) * std::f32::consts::TAU).cos();
        ((1.0 - (c * 2.0 + 0.5).clamp(0.0, 1.0)) * 11.0) as u8
    }

    /// Items a block leaves behind when something other than the player destroys it.
    fn drop_loot(&mut self, pos: IVec3, state: State) {
        if let Some(be) = self.block_entities.remove(&pos) {
            for s in be.contents() {
                self.spawn_drop(pos, s);
            }
        }
        for s in item::block_loot(state, false, &mut self.rng) {
            self.spawn_drop(pos, s);
        }
    }

    /// Fragments flying off a block, textured like it.
    pub fn block_particles(&mut self, pos: IVec3, state: State, count: usize) {
        let r = reg();
        let b = r.block(state);
        if state == AIR || r.has(state, F_FLUID) || self.particles.len() > 3000 {
            return;
        }
        let layer = r.texsets[r.mesh_states[state as usize].texset as usize].layer[2];
        let tint = if b.tint != TINT_NONE && b.shape != Shape::Grass { hud::icon_tint(b) } else { WHITE };
        for _ in 0..count {
            let p = DVec3::new(pos.x as f64 + self.rng.f64(), pos.y as f64 + self.rng.f64(), pos.z as f64 + self.rng.f64());
            let vel = DVec3::new(self.rng.f64() * 0.16 - 0.08, self.rng.f64() * 0.16, self.rng.f64() * 0.16 - 0.08);
            let uv = [self.rng.below(13) as f32 / 16.0, self.rng.below(13) as f32 / 16.0, 0.1875];
            self.particles.push(Particle { pos: p, prev: p, vel, life: 8 + self.rng.below(22) as u16, layer, uv, color: tint, size: 0.05 + self.rng.f32() * 0.05, gravity: 0.04, grow: 1.0 });
        }
    }

    /// Puffs of smoke (or steam, or spray) drifting away from a point.
    pub fn puffs(&mut self, at: DVec3, count: usize, spread: f64, speed: f64, color: Rgba, size: f32) {
        if self.particles.len() > 3000 {
            return;
        }
        for _ in 0..count {
            let p = at + DVec3::new((self.rng.f64() - 0.5) * spread, (self.rng.f64() - 0.5) * spread, (self.rng.f64() - 0.5) * spread);
            let vel = DVec3::new((self.rng.f64() - 0.5) * speed, self.rng.f64() * speed * 0.6 + 0.01, (self.rng.f64() - 0.5) * speed);
            let shade = 0.75 + self.rng.f32() * 0.25;
            let c = [(color[0] as f32 * shade) as u8, (color[1] as f32 * shade) as u8, (color[2] as f32 * shade) as u8, color[3]];
            self.particles.push(Particle { pos: p, prev: p, vel, life: 12 + self.rng.below(24) as u16, layer: PUFF_LAYER, uv: [0.0, 0.0, 1.0], color: c, size: size * (0.6 + self.rng.f32() * 0.8), gravity: -0.002, grow: 1.02 });
        }
    }

    /// Share of an explosion that reaches a box: the fraction of sample points with a clear line.
    fn exposure(&self, centre: DVec3, bb: &Aabb) -> f64 {
        let (mut clear, mut total) = (0, 0);
        for iy in 0..=2 {
            for iz in 0..=2 {
                for ix in 0..=2 {
                    let p = DVec3::new(bb.min.x + (bb.max.x - bb.min.x) * ix as f64 / 2.0, bb.min.y + (bb.max.y - bb.min.y) * iy as f64 / 2.0, bb.min.z + (bb.max.z - bb.min.z) * iz as f64 / 2.0);
                    let d = centre - p;
                    let len = d.length();
                    total += 1;
                    if len < 1e-6 || raycast(&self.world, p, d * (1.0 / len), len, false).is_none() {
                        clear += 1;
                    }
                }
            }
        }
        clear as f64 / total as f64
    }

    /// Blows a hole in the world.  `drop_all` makes every destroyed block leave its item
    /// (TNT); otherwise only about one in `power` does.
    pub fn explode(&mut self, centre: DVec3, power: f32, drop_all: bool) {
        let r = reg();
        let mut hit: Vec<IVec3> = Vec::new();
        let mut seen: HashSet<IVec3> = HashSet::new();
        for i in 0..16 {
            for j in 0..16 {
                for k in 0..16 {
                    if !(i == 0 || i == 15 || j == 0 || j == 15 || k == 0 || k == 15) {
                        continue;
                    }
                    let d = DVec3::new(i as f64 / 7.5 - 1.0, j as f64 / 7.5 - 1.0, k as f64 / 7.5 - 1.0).normalized();
                    let mut strength = power * (0.7 + self.rng.f32() * 0.6);
                    let mut p = centre;
                    while strength > 0.0 {
                        let bp = p.floor();
                        let s = self.world.get_at(bp);
                        if s != AIR {
                            strength -= (r.block(s).resistance + 0.3) * 0.3;
                            if strength > 0.0 && seen.insert(bp) {
                                hit.push(bp);
                            }
                        }
                        p += d * 0.3;
                        strength -= 0.225;
                    }
                }
            }
        }
        // Living things.
        let reach = power as f64 * 2.0;
        if self.player.mode == GameMode::Survival && !self.vitals.dead {
            let bb = self.player.aabb();
            let dist = (bb.center() - centre).length();
            if dist < reach {
                let impact = (1.0 - dist / reach) * self.exposure(centre, &bb);
                let damage = ((impact * impact + impact) / 2.0 * 7.0 * reach + 1.0).floor();
                if impact > 0.0 {
                    self.vitals.damage(damage as f32, "blew up");
                    let push = (self.player.eye(1.0) - centre).normalized() * impact;
                    self.player.vel += push;
                }
            }
        }
        self.blast_mobs(centre, power);
        for e in self.drops.iter_mut() {
            let d = e.pos - centre;
            let dist = d.length();
            if dist < reach && dist > 1e-3 {
                e.vel += d * ((1.0 - dist / reach) * 0.6 / dist);
            }
        }
        // Blocks.
        for bp in hit {
            let s = self.world.get_at(bp);
            if s == AIR || r.has(s, F_FLUID) {
                continue;
            }
            let left = if r.has(s, F_WATER) { r.k.water } else { AIR };
            if self.world.set_at(bp, left).is_none() {
                continue;
            }
            if r.block(s).name == "tnt" {
                // Chain reaction: neighbouring TNT is lit with a short fuse.
                let fuse = self.rng.range(10, 30);
                let tnt = MovingBlock::tnt(bp, s, fuse, &mut self.rng);
                self.moving.push(tnt);
                continue;
            }
            if let Some(be) = self.block_entities.remove(&bp) {
                for st in be.contents() {
                    self.spawn_drop(bp, st);
                }
            }
            if drop_all || self.rng.chance(1.0 / power) {
                for st in item::block_loot(s, false, &mut self.rng) {
                    self.spawn_drop(bp, st);
                }
            }
            if self.rng.one_in(6) {
                self.block_particles(bp, s, 3);
            }
        }
        let pitch = 0.8 + self.rng.f32() * 0.3;
        audio::play_at(Sfx::Explode, centre, self.player.pos, 1.0, pitch);
        self.puffs(centre, 40, power as f64, 0.35, [230, 230, 230, 255], 0.5);
        self.puffs(centre, 12, 1.0, 0.1, [90, 90, 90, 255], 0.9);
    }

    /// A falling block came to rest: it becomes a block again, or an item if it cannot.
    fn land_block(&mut self, m: MovingBlock) {
        let r = reg();
        let bp = IVec3::new(m.pos.x.floor() as i32, (m.pos.y + 0.01).floor() as i32, m.pos.z.floor() as i32);
        let there = self.world.get_at(bp);
        if (there == AIR || r.has(there, F_REPLACEABLE)) && self.world.set_at(bp, m.state).is_some() {
            for (pos, state) in rules::notify_neighbours(&mut self.world, bp) {
                self.drop_loot(pos, state);
            }
        } else {
            self.drop_loot(bp, m.state);
        }
    }

    /// Fluids, growth and decay; falling blocks; TNT; particles.
    fn tick_simulation(&mut self) {
        let centre = ((self.player.pos.x.floor() as i32) >> 4, (self.player.pos.z.floor() as i32) >> 4);
        self.sim.sky_darken = self.sky_darken();
        let radius = self.settings.render_distance.min(5);
        self.sim.step(&mut self.world, self.tick, centre, radius);
        for (pos, state) in std::mem::take(&mut self.sim.broken) {
            self.drop_loot(pos, state);
        }
        for p in std::mem::take(&mut self.sim.fizz) {
            audio::play_at(Sfx::Fizz, p.center(), self.player.pos, 0.5, 1.6);
            self.puffs(p.center() + DVec3::new(0.0, 0.5, 0.0), 8, 0.8, 0.05, [200, 200, 200, 255], 0.25);
        }
        for (pos, state) in std::mem::take(&mut self.world.falls) {
            self.moving.push(MovingBlock::falling(pos, state));
        }
        let mut scratch = std::mem::take(&mut self.scratch);
        let mut events: Vec<(usize, MovingEvent)> = Vec::new();
        for (i, m) in self.moving.iter_mut().enumerate() {
            let e = m.tick(&self.world, &mut scratch);
            if e != MovingEvent::None {
                events.push((i, e));
            }
        }
        self.scratch = scratch;
        // Highest index first, so swap_remove never disturbs an entry still to be handled.
        for (i, e) in events.into_iter().rev() {
            let m = self.moving.swap_remove(i);
            match e {
                MovingEvent::Landed => self.land_block(m),
                MovingEvent::Explode => self.explode(m.pos + DVec3::new(0.0, 0.49, 0.0), 4.0, true),
                MovingEvent::Lost | MovingEvent::None => self.drop_loot(m.pos.floor(), m.state),
            }
        }
        // Lit fuses smoke.
        if self.tick % 2 == 0 {
            let fuses: Vec<DVec3> = self.moving.iter().filter(|m| m.fuse >= 0).map(|m| m.pos + DVec3::new(0.0, 1.05, 0.0)).collect();
            for p in fuses {
                self.puffs(p, 1, 0.1, 0.02, [120, 120, 120, 255], 0.12);
            }
        }
        let world = &self.world;
        self.particles.retain_mut(|p| p.tick(world));
        self.on_death();
    }

    fn tick_block_entities(&mut self) {
        let r = reg();
        let mut changed: Vec<(IVec3, bool)> = Vec::new();
        for (pos, be) in self.block_entities.iter_mut() {
            if let BlockEntity::Furnace(f) = be {
                if f.tick() {
                    changed.push((*pos, f.lit()));
                }
            }
        }
        for (pos, lit) in changed {
            let s = self.world.get_at(pos);
            if r.block(s).name == "furnace" {
                self.world.set_at(pos, r.with(s, "lit", lit as usize));
            }
        }
        // Containers whose block disappeared some other way (commands) spill their contents.
        if self.tick % 100 == 0 {
            let orphans: Vec<IVec3> = self
                .block_entities
                .iter()
                .filter(|(pos, be)| {
                    let name = r.block(self.world.get_at(**pos)).name;
                    self.world.is_ready(pos.x, pos.z) && name != if matches!(be, BlockEntity::Chest(_)) { "chest" } else { "furnace" }
                })
                .map(|(pos, _)| *pos)
                .collect();
            for pos in orphans {
                if let Some(be) = self.block_entities.remove(&pos) {
                    for s in be.contents() {
                        self.spawn_drop(pos, s);
                    }
                }
            }
        }
    }

    fn game_tick(&mut self) {
        let r = reg();
        self.tick += 1;
        if !self.time_frozen {
            self.day_time += 1;
        }
        let survival = self.player.mode == GameMode::Survival;
        let c = self.controls();
        if !self.vitals.dead {
            self.player.tick(&self.world, &c);
        }
        if survival {
            self.vitals.tick(&self.player, &self.world);
            self.on_death();
        } else {
            self.vitals.hurt_time = 0;
            self.vitals.air = 300;
        }
        // View bobbing follows the walking speed.
        self.prev_bob = self.bob;
        let step = self.player.pos - self.player.prev_pos;
        let target = if self.player.on_ground && !self.vitals.dead { ((step.x * step.x + step.z * step.z).sqrt() as f32).min(0.1) } else { 0.0 };
        self.bob += (target - self.bob) * 0.4;

        self.update_target(1.0);
        self.break_cooldown = self.break_cooldown.saturating_sub(1);
        self.place_cooldown = self.place_cooldown.saturating_sub(1);
        self.swing = self.swing.saturating_sub(1);
        let acting = self.grabbed && self.in_play() && !self.vitals.dead && self.player.mode != GameMode::Spectator;
        let clicked = std::mem::take(&mut self.clicked);
        let (left, mut right) = (acting && (self.input.mouse_down[MOUSE_LEFT] || clicked[0]), acting && (self.input.mouse_down[MOUSE_RIGHT] || clicked[1]));
        // Clicks on a mob: left attacks, right interacts.
        if let Some(i) = self.target_mob.filter(|i| acting && *i < self.mobs.len()) {
            if clicked[0] {
                self.attack_mob(i);
            } else if clicked[1] && self.place_cooldown == 0 && self.interact_mob(i) {
                self.place_cooldown = 4;
                self.swing = 6;
                right = false;
            }
            self.update_target(1.0);
        }

        // Left button: mine.
        match (left, self.target) {
            (true, Some(hit)) => {
                if self.swing == 0 {
                    self.swing = 6;
                }
                if self.break_cooldown > 0 {
                    self.mining = None;
                } else if !survival {
                    self.break_block(hit);
                    self.break_cooldown = 5;
                    self.update_target(1.0);
                } else {
                    let block = r.block(hit.state);
                    match item::break_ticks(self.inv.held().def(), block, self.player.on_ground, self.player.head_in_water) {
                        None => self.mining = None,
                        Some(total) => {
                            let done = match self.mining {
                                Some(m) if m.pos == hit.pos => m.ticks + 1,
                                _ => 1,
                            };
                            if done >= total {
                                self.break_block(hit);
                                self.mining = None;
                                self.break_cooldown = if total == 0 { 0 } else { 5 };
                                self.update_target(1.0);
                            } else {
                                if done % 4 == 1 {
                                    audio::play(audio::for_block(block.sound), 0.2, 0.5);
                                }
                                self.mining = Some(Mining { pos: hit.pos, ticks: done, total });
                            }
                        }
                    }
                }
            }
            _ => {
                self.mining = None;
                if !left {
                    self.break_cooldown = 0;
                }
            }
        }

        // Right button: use the held item, or eat it.
        if right {
            let mut used = false;
            if self.place_cooldown == 0 {
                used = self.use_item();
                if used {
                    self.place_cooldown = 4;
                    self.update_target(1.0);
                }
            }
            let held = self.inv.held();
            let def = held.def();
            let edible = !held.is_empty() && def.food > 0 && survival && (self.vitals.food < 20 || def.name == "golden_apple");
            if !used && self.place_cooldown == 0 && edible && self.screen == Screen::None {
                self.eating += 1;
                if self.eating >= 32 {
                    self.vitals.eat(def.food, def.saturation);
                    if def.name == "golden_apple" {
                        self.vitals.heal(4.0);
                    }
                    self.inv.consume_held();
                    self.eating = 0;
                }
            } else {
                self.eating = 0;
            }
        } else {
            self.place_cooldown = 0;
            self.eating = 0;
        }

        // Footsteps, eating and the sound of getting hurt.
        if self.player.on_ground && !self.vitals.dead {
            self.step_dist += (step.x * step.x + step.z * step.z).sqrt() as f32;
            if self.step_dist > 2.2 {
                self.step_dist = 0.0;
                let below = self.world.get(self.player.pos.x.floor() as i32, (self.player.pos.y - 0.2).floor() as i32, self.player.pos.z.floor() as i32);
                if below != AIR {
                    audio::play(audio::for_block(r.block(below).sound), 0.12, 1.0);
                }
            }
        }
        if self.eating > 0 && self.eating % 4 == 1 {
            audio::play(Sfx::Eat, 0.3, 0.9 + self.rng.f32() * 0.3);
        }
        if self.vitals.health < self.last_health - 0.01 {
            audio::play(Sfx::Hurt, 0.7, 0.9 + self.rng.f32() * 0.2);
        }
        self.last_health = self.vitals.health;
        self.tick_block_entities();
        self.tick_simulation();
        self.tick_mobs();
        self.tick_geysers();
        self.tick_drops();
        self.renderer.animate(self.tick);
        if self.world_id.is_some() && self.tick - self.last_save_tick >= AUTOSAVE_TICKS {
            self.last_save_tick = self.tick;
            self.save_world();
        }
    }

    /// Per-frame input that should not wait for the next tick.
    fn frame_input(&mut self) {
        let now = platform::now_ms();
        let pressed = |g: &Game, k: u32| g.input.pressed(k);
        if self.in_play() {
            if self.grabbed {
                let (dx, dy) = self.input.look;
                self.player.turn(dx, dy, self.settings.sensitivity);
                let before = self.inv.selected;
                for n in 0..9 {
                    if pressed(self, key::N1 + n) {
                        self.inv.selected = n as usize;
                    }
                }
                let wheel = self.input.wheel;
                if wheel != 0.0 {
                    self.inv.selected = (self.inv.selected + if wheel > 0.0 { 1 } else { 8 }) % 9;
                }
                if self.input.mouse_pressed[MOUSE_MIDDLE] {
                    self.pick_block();
                }
                self.clicked[0] |= self.input.mouse_pressed[MOUSE_LEFT];
                self.clicked[1] |= self.input.mouse_pressed[MOUSE_RIGHT];
                if before != self.inv.selected {
                    self.held_changed = now;
                    self.mining = None;
                    self.eating = 0;
                }
                // Double-tap space toggles flight; double-tap forward starts a sprint.
                if pressed(self, key::SPACE) {
                    if self.player.mode == GameMode::Creative && self.tick.saturating_sub(self.last_space_tick) < 7 {
                        self.player.flying = !self.player.flying;
                        self.player.on_ground = false;
                        self.last_space_tick = 0;
                    } else {
                        self.last_space_tick = self.tick;
                    }
                }
                if pressed(self, key::W) {
                    if self.tick.saturating_sub(self.last_forward_tick) < 7 {
                        self.player.sprinting = true;
                    }
                    self.last_forward_tick = self.tick;
                }
                if pressed(self, key::Q) {
                    let n = if self.input.down(key::CONTROL) { self.inv.held().count } else { 1 };
                    let s = self.inv.held_mut().take(n);
                    self.throw_item(s);
                }
                if pressed(self, key::E) && self.player.mode != GameMode::Spectator {
                    self.open_screen(if self.player.mode == GameMode::Creative { Screen::Creative } else { Screen::Inventory });
                } else if pressed(self, key::T) || pressed(self, key::ENTER) {
                    self.ui.text.clear();
                    self.open_screen(Screen::Chat);
                } else if pressed(self, key::SLASH) {
                    self.ui.text = "/".to_string();
                    self.open_screen(Screen::Chat);
                } else if pressed(self, key::ESCAPE) {
                    self.open_screen(Screen::Pause);
                }
            } else if self.input.mouse_pressed[MOUSE_LEFT] {
                platform::set_cursor_grab(true);
            } else if pressed(self, key::ESCAPE) && now > self.ui.esc_guard {
                self.open_screen(Screen::Pause);
            }
        }
        if pressed(self, key::F1) {
            self.hide_hud = !self.hide_hud;
        }
        if pressed(self, key::F3) {
            self.show_debug = !self.show_debug;
        }
        if pressed(self, key::F2) {
            let px = self.renderer.capture();
            let png = crate::util::png::encode_rgba(self.renderer.width as u32, self.renderer.height as u32, &px);
            let name = format!("screenshot-{}.png", (platform::now_ms() as u64) / 100);
            platform::export_file(&name, &png);
            self.say(format!("Saved screenshot as {name}"));
        }
    }

    // ------------------------------------------------------------ atmosphere
    /// Position of the sun in its cycle: 0 noon, 0.25 sunset, 0.5 midnight, 0.75 sunrise.
    pub fn celestial_angle(&self, alpha: f64) -> f32 {
        let t = (((self.day_time % DAY_TICKS) as f64 + if self.time_frozen { 0.0 } else { alpha }) / DAY_TICKS as f64 - 0.25).rem_euclid(1.0);
        let eased = 0.5 - (t * std::f64::consts::PI).cos() / 2.0;
        (t + (eased - t) / 3.0) as f32
    }

    fn environment(&self, alpha: f64, eye: DVec3) -> Environment {
        let r = reg();
        let a = self.celestial_angle(alpha);
        let c = (a * std::f32::consts::TAU).cos();
        let day = (c * 2.0 + 0.5).clamp(0.0, 1.0);
        let biome = self.world.biome(eye.x.floor() as i32, eye.z.floor() as i32).info();
        let hex = |v: u32| [((v >> 16) & 255) as f32 / 255.0, ((v >> 8) & 255) as f32 / 255.0, (v & 255) as f32 / 255.0];
        let sky_base = hex(biome.sky);
        let mut sky = [sky_base[0] * day, sky_base[1] * day, sky_base[2] * day];
        let mut fog = [0.753 * (day * 0.94 + 0.06), 0.847 * (day * 0.94 + 0.06), 1.0 * (day * 0.91 + 0.09)];
        let sunset = (1.0 - (c.abs() / 0.4)).clamp(0.0, 1.0).powi(2);
        fog = [fog[0] + (1.0 - fog[0]) * sunset * 0.25, fog[1] + (0.55 - fog[1]) * sunset * 0.12, fog[2] * (1.0 - sunset * 0.18)];
        let light = (c * 2.0 + 0.2).clamp(0.0, 1.0);
        let l = 0.2 + light * 0.8;
        let sky_light = [l * (0.72 + 0.28 * light), l * (0.78 + 0.22 * light), l];
        let rd = self.settings.render_distance as f32 * 16.0;
        let (mut fog_start, mut fog_end) = (rd * 0.72, rd * 0.98);
        let at_eye = self.world.get_at(eye.floor());
        if r.has(at_eye, F_WATER) {
            let w = hex(biome.water);
            let depth_light = (self.world.light(eye.x.floor() as i32, eye.y.floor() as i32, eye.z.floor() as i32) >> 4) as f32 / 15.0 * l;
            fog = [w[0] * 0.25 * depth_light, w[1] * 0.45 * depth_light, w[2] * 0.75 * depth_light];
            sky = fog;
            fog_start = -8.0;
            fog_end = 46.0;
        } else if r.has(at_eye, F_LAVA) {
            fog = [0.6, 0.1, 0.0];
            sky = fog;
            fog_start = 0.0;
            fog_end = 2.5;
        }
        Environment { sky, fog, sky_light, celestial: a, stars: ((1.0 - (c * 2.0 + 0.25)).clamp(0.0, 1.0)).powi(2) * 0.5, fog_start, fog_end, sunset, gamma: self.settings.gamma }
    }

    /// Brightness of the light at a point as an RGB factor (matches the terrain shader).
    fn light_at(&self, p: DVec3, env: &Environment) -> [f32; 3] {
        let l = self.world.light(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        let curve = |v: u8| {
            let v = v as f32 / 15.0;
            v / (4.0 - 3.0 * v)
        };
        let (sky, block) = (curve(l >> 4), curve(l & 15));
        let warm = [1.0, 0.87, 0.70];
        let mut c = [0.0f32; 3];
        for i in 0..3 {
            let v = (sky * env.sky_light[i]).max(block * warm[i]);
            c[i] = v + (1.0 - v) * (0.035 + env.gamma * 0.2 * (1.0 - v));
        }
        c
    }

    /// Dropped items: little spinning cubes for blocks, upright sprites for everything else.
    fn draw_drops(&mut self, eye: DVec3, alpha: f64, env: &Environment) {
        let r = reg();
        for i in 0..self.drops.len() {
            let e = &self.drops[i];
            let p = e.prev.lerp(e.pos, alpha);
            let rel = p - eye;
            if rel.length_sq() > 48.0 * 48.0 {
                continue;
            }
            let t = e.age as f32 + alpha as f32;
            let lift = (t / 10.0).sin() * 0.1 + 0.1;
            let yaw = t / 20.0;
            let light = self.light_at(p + DVec3::new(0.0, 0.25, 0.0), env);
            let def = e.stack.def();
            let copies = match e.stack.count {
                0..=1 => 1,
                2..=16 => 2,
                17..=32 => 3,
                33..=48 => 4,
                _ => 5,
            };
            let b = &mut self.renderer.batch;
            let cube = def.block.filter(|_| def.icon == 0).map(|bi| &r.blocks[bi as usize]).filter(|blk| hud::icon_is_cube(blk));
            for k in 0..copies {
                let o = k as f32;
                let c = Vec3::new(rel.x as f32 + o * 0.06 * yaw.cos(), rel.y as f32 + lift + o * 0.04, rel.z as f32 + o * 0.06 * yaw.sin());
                match cube {
                    Some(blk) => {
                        let state = r.default_of(blk);
                        let layers = r.texsets[r.mesh_states[state as usize].texset as usize].layer;
                        let tint = hud::icon_tint(blk);
                        let top = if blk.shape == Shape::Grass || blk.tint != TINT_NONE { tint } else { WHITE };
                        let side = if blk.shape == Shape::Grass { WHITE } else { top };
                        draw_cube(b, Vec3::new(c.x, c.y + 0.125, c.z), 0.125, yaw, Some(&layers), [top, side], light);
                    }
                    None => {
                        let (layer, tint) = match def.block {
                            Some(bi) if def.icon == 0 => {
                                let blk = &r.blocks[bi as usize];
                                let state = r.default_of(blk);
                                (r.texsets[r.mesh_states[state as usize].texset as usize].layer[0], hud::icon_tint(blk))
                            }
                            _ => (def.icon, WHITE),
                        };
                        draw_sprite(b, Vec3::new(c.x, c.y + 0.25, c.z), 0.25, yaw, layer, tint, light);
                    }
                }
            }
        }
    }

    /// Falling blocks and primed TNT (which flashes white as the fuse burns down).
    fn draw_moving(&mut self, eye: DVec3, alpha: f64, env: &Environment) {
        let r = reg();
        for i in 0..self.moving.len() {
            let m = &self.moving[i];
            let p = m.prev.lerp(m.pos, alpha);
            let rel = p - eye;
            if rel.length_sq() > 96.0 * 96.0 {
                continue;
            }
            let light = self.light_at(p + DVec3::new(0.0, 0.5, 0.0), env);
            let blk = r.block(m.state);
            let layers = r.texsets[r.mesh_states[m.state as usize].texset as usize].layer;
            let flash = m.fuse >= 0 && (m.fuse / 5) % 2 == 0;
            let tint = if blk.tint != TINT_NONE { hud::icon_tint(blk) } else { WHITE };
            // Primed TNT swells just before it goes off.
            let swell = if m.fuse >= 0 && m.fuse < 10 { 1.0 + (10 - m.fuse) as f32 * 0.03 } else { 1.0 };
            let c = Vec3::new(rel.x as f32, rel.y as f32 + 0.49, rel.z as f32);
            let b = &mut self.renderer.batch;
            if flash {
                draw_cube(b, c, 0.49 * swell, 0.0, None, [WHITE, WHITE], [1.0, 1.0, 1.0]);
            } else {
                draw_cube(b, c, 0.49 * swell, 0.0, Some(&layers), [tint, tint], light);
            }
        }
    }

    fn draw_particles(&mut self, eye: DVec3, alpha: f64, env: &Environment, yaw: f32, pitch: f32) {
        if self.particles.is_empty() {
            return;
        }
        let look = look_dir(yaw, pitch);
        let right = look.cross(Vec3::new(0.0, 1.0, 0.0)).normalized();
        let up = right.cross(look).normalized();
        for i in 0..self.particles.len() {
            let p = self.particles[i];
            let at = p.prev.lerp(p.pos, alpha);
            let rel = at - eye;
            if rel.length_sq() > 64.0 * 64.0 {
                continue;
            }
            let light = self.light_at(at, env);
            let c = Vec3::new(rel.x as f32, rel.y as f32, rel.z as f32);
            let (rx, ux) = (right * p.size, up * p.size);
            let color = [(p.color[0] as f32 * light[0]) as u8, (p.color[1] as f32 * light[1]) as u8, (p.color[2] as f32 * light[2]) as u8, p.color[3]];
            let corner = |v: Vec3| [v.x, v.y, v.z];
            let [u, v, sz] = p.uv;
            self.renderer.batch.quad([corner(c - rx - ux), corner(c + rx - ux), corner(c + rx + ux), corner(c - rx + ux)], [[u, v + sz], [u + sz, v + sz], [u + sz, v], [u, v]], p.layer as f32, color);
        }
    }

    /// The held item at the lower right of the view, dipping when the arm swings.
    fn draw_hand(&mut self, eye: DVec3, alpha: f64, env: &Environment, yaw: f32, pitch: f32) {
        let r = reg();
        let held = self.inv.held();
        if held.is_empty() {
            return;
        }
        let def = held.def();
        let look = look_dir(yaw, pitch);
        let right = look.cross(Vec3::new(0.0, 1.0, 0.0)).normalized();
        let up = right.cross(look).normalized();
        let t = if self.swing > 0 { ((6.0 - self.swing as f32 + alpha as f32) / 6.0).clamp(0.0, 1.0) } else { 0.0 };
        let dip = (t * std::f32::consts::PI).sin();
        let eat = if self.eating > 0 { ((self.eating as f32 + alpha as f32) * 0.9).sin() * 0.03 + 0.12 } else { 0.0 };
        let c = look * (0.62 + dip * 0.12) + right * (0.44 - eat * 2.0 - dip * 0.1) - up * (0.40 + dip * 0.14 - eat);
        let light = self.light_at(eye, env);
        let b = &mut self.renderer.batch;
        let cube = def.block.filter(|_| def.icon == 0).map(|bi| &r.blocks[bi as usize]).filter(|blk| hud::icon_is_cube(blk));
        match cube {
            Some(blk) => {
                let state = r.default_of(blk);
                let layers = r.texsets[r.mesh_states[state as usize].texset as usize].layer;
                let tint = hud::icon_tint(blk);
                let top = if blk.shape == Shape::Grass || blk.tint != TINT_NONE { tint } else { WHITE };
                let side = if blk.shape == Shape::Grass { WHITE } else { top };
                draw_cube(b, c, 0.13, yaw.to_radians() + 0.75 + dip * 0.6, Some(&layers), [top, side], light);
            }
            None => {
                let (layer, tint) = match def.block {
                    Some(bi) if def.icon == 0 => {
                        let blk = &r.blocks[bi as usize];
                        (r.texsets[r.mesh_states[r.default_of(blk) as usize].texset as usize].layer[0], hud::icon_tint(blk))
                    }
                    _ => (def.icon, WHITE),
                };
                // A flat sprite turned a little towards the middle of the view.
                let across = (right * 0.85 + look * 0.5).normalized() * 0.2;
                let along = (up * 0.9 - look * (0.25 + dip * 0.5)).normalized() * 0.2;
                let color = [(tint[0] as f32 * light[0]) as u8, (tint[1] as f32 * light[1]) as u8, (tint[2] as f32 * light[2]) as u8, 255];
                let p = |v: Vec3| [v.x, v.y, v.z];
                b.quad([p(c - across - along), p(c + across - along), p(c + across + along), p(c - across + along)], [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]], layer as f32, color);
            }
        }
    }

    // ------------------------------------------------------------ frame
    pub fn frame(&mut self, now_ms: f64) {
        let frame_start = platform::now_ms();
        if self.last_frame < 0.0 {
            self.last_frame = now_ms;
        }
        let dt = (now_ms - self.last_frame).clamp(0.0, 250.0);
        self.last_frame = now_ms;
        self.frame_count += 1;
        self.fps_timer += dt;
        if self.fps_timer >= 500.0 {
            self.fps = self.frame_count as f32 * 1000.0 / self.fps_timer as f32;
            self.frame_count = 0;
            self.fps_timer = 0.0;
        }
        // Menus own the cursor.
        if self.screen != Screen::None && self.grabbed {
            self.grabbed = false;
            platform::set_cursor_grab(false);
        }
        let scale = self.gui_scale();
        let i = &self.input;
        self.ui.mouse = Mouse {
            x: i.cursor.0 / scale,
            y: i.cursor.1 / scale,
            left: i.mouse_pressed[MOUSE_LEFT],
            right: i.mouse_pressed[MOUSE_RIGHT],
            middle: i.mouse_pressed[MOUSE_MIDDLE],
            held: i.mouse_down[MOUSE_LEFT],
            shift: i.down(key::SHIFT),
            wheel: i.wheel,
        };
        self.frame_input();

        // Streaming: generate and light chunks around the player within a time budget.
        let centre = ((self.player.pos.x.floor() as i32) >> 4, (self.player.pos.z.floor() as i32) >> 4);
        let rd = self.settings.render_distance;
        let budget = if self.loading { 40.0 } else { 7.0 };
        while platform::now_ms() - frame_start < budget {
            if !self.world.pump(centre, rd + 1) {
                break;
            }
        }
        if self.frame_count % 64 == 0 {
            self.world.unload_far(centre, rd + 6);
            self.renderer.chunks.retain_loaded(&self.world);
        }
        let mesh_budget = budget + if self.loading { 30.0 } else { 6.0 };
        self.renderer.chunks.update(&mut self.world, centre, rd, || platform::now_ms() - frame_start < mesh_budget);
        if self.loading {
            let near_ready = (-1..=1).all(|dz| (-1..=1).all(|dx| self.world.chunks.get(&(centre.0 + dx, centre.1 + dz)).map_or(false, |c| c.stage >= crate::world::chunk::STAGE_LIT && c.mesh_dirty == 0)));
            if near_ready {
                self.loading = false;
            }
        }

        // The simulation stands still behind the title screen and the game menu.
        let running = !self.loading && !self.at_title && !matches!(self.screen, Screen::Pause | Screen::Options);
        if running {
            self.accumulator += dt;
            let mut steps = 0;
            while self.accumulator >= TICK_MS && steps < 10 {
                self.game_tick();
                self.accumulator -= TICK_MS;
                steps += 1;
            }
            if steps == 10 {
                self.accumulator = 0.0;
            }
        } else if self.at_title {
            self.renderer.animate((now_ms / TICK_MS) as u64);
        }
        let alpha = if running { (self.accumulator / TICK_MS).clamp(0.0, 1.0) } else { 1.0 };
        self.update_target(alpha);
        self.render(alpha);
        self.input.end_frame();
        self.frame_ms += ((platform::now_ms() - frame_start) as f32 - self.frame_ms) * 0.1;
    }

    fn render(&mut self, alpha: f64) {
        let r = reg();
        let (eye, yaw, pitch) = if self.at_title {
            // Backdrop: a slow turn above the spawn point.
            let p = self.player.pos;
            let top = self.world.top(p.x.floor() as i32, p.z.floor() as i32) as f64;
            (DVec3::new(p.x, (p.y + 9.0).max(top + 5.0), p.z), (platform::now_ms() * 0.004 % 360.0) as f32, 14.0)
        } else {
            let (mut eye, yaw, mut pitch) = (self.player.eye(alpha), self.player.yaw, self.player.pitch);
            if self.settings.bobbing && !self.player.flying {
                let a = alpha as f32;
                let walk = (self.player.prev_walk_dist + (self.player.walk_dist - self.player.prev_walk_dist) * a) * std::f32::consts::PI;
                let bob = self.prev_bob + (self.bob - self.prev_bob) * a;
                let (sin, cos) = yaw.to_radians().sin_cos();
                let side = (walk.sin() * bob * 0.5) as f64;
                eye.x += cos as f64 * side;
                eye.z += sin as f64 * side;
                eye.y -= (walk.cos() * bob).abs() as f64;
                pitch += (walk.cos() - 0.2).abs() * bob * 5.0;
            }
            (eye, yaw, pitch.clamp(-90.0, 90.0))
        };
        let env = self.environment(alpha, eye);
        let target_fov = if self.player.sprinting { 1.15 } else { 1.0 } * if self.player.flying { 1.1 } else { 1.0 };
        self.fov_scale += (target_fov - self.fov_scale) * 0.2;
        let fov = if self.at_title { 70.0 } else { (self.settings.fov * self.fov_scale).min(140.0) };
        let cam = Camera { pos: eye, yaw, pitch, fov };
        let rd = self.settings.render_distance;
        self.renderer.draw_world(&mut self.world, &cam, &env, rd);
        gl::disable(gl::CULL_FACE);

        if !self.at_title {
            self.draw_drops(eye, alpha, &env);
            self.draw_moving(eye, alpha, &env);
            self.draw_mobs(eye, alpha, &env, false);
            self.draw_particles(eye, alpha, &env, yaw, pitch);
            self.renderer.flush_world(gl::TRIANGLES, false);
            if let (Some(hit), false) = (self.target, self.hide_hud || self.screen != Screen::None) {
                let origin = Vec3::new((hit.pos.x as f64 - eye.x) as f32, (hit.pos.y as f64 - eye.y) as f32, (hit.pos.z as f64 - eye.z) as f32);
                // Cracks spreading over the block being mined.
                if let Some(m) = self.mining.filter(|m| m.pos == hit.pos && m.total > 0) {
                    let stage = ((m.ticks * 10) / m.total).min(9) as u16;
                    for bx in r.outline(hit.state) {
                        draw_box_faces(&mut self.renderer.batch, origin, bx, 0.003, (DESTROY_LAYER + stage) as f32, WHITE);
                    }
                    self.renderer.flush_world(gl::TRIANGLES, true);
                }
                // Outline of the targeted block.
                let color = rgba(0x000000, 110);
                let e = 0.002;
                for b in r.outline(hit.state) {
                    let lo = Vec3::new(origin.x + b[0] - e, origin.y + b[1] - e, origin.z + b[2] - e);
                    let hi = Vec3::new(origin.x + b[3] + e, origin.y + b[4] + e, origin.z + b[5] + e);
                    let c = |i: usize| Vec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
                    for (a, b) in [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)] {
                        self.renderer.batch.line(c(a), c(b), color);
                    }
                }
                self.renderer.flush_world(gl::LINES, true);
            }
        }
        if !self.at_title {
            // The jelly of Sulfur Cubes is see-through.
            self.draw_mobs(eye, alpha, &env, true);
            self.renderer.flush_world(gl::TRIANGLES, true);
        }
        self.renderer.draw_translucent(&cam, &env, rd);
        if !self.at_title && !self.hide_hud && self.player.mode != GameMode::Spectator && !self.vitals.dead {
            // The hand is drawn over everything else in the world.
            gl::disable(gl::CULL_FACE);
            gl::depth_mask(true);
            gl::clear(gl::DEPTH_BUFFER_BIT);
            self.draw_hand(eye, alpha, &env, yaw, pitch);
            self.renderer.flush_world(gl::TRIANGLES, false);
        }

        let scale = self.gui_scale();
        let (w, h) = self.renderer.begin_ui(scale);
        if !self.at_title {
            hud::draw(self, w, h);
        }
        let action = ui::run(self, w, h);
        self.renderer.end_ui();
        self.perform(action);
    }
}
