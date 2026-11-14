//! Heads-up display: crosshair, hotbar, status bars, chat and the debug overlay.

use super::player::GameMode;
use super::ui::Screen;
use super::Game;
use crate::art::font::*;
use crate::gfx::batch::{rgba, Batch, Rgba, WHITE};
use crate::item::ItemStack;
use crate::platform;
use crate::util::math::wrap_degrees;
use crate::world::block::*;
use crate::world::model::Shape;

fn shade(c: Rgba, f: f32) -> Rgba {
    [(c[0] as f32 * f) as u8, (c[1] as f32 * f) as u8, (c[2] as f32 * f) as u8, c[3]]
}

/// Colour a block is tinted with when shown outside the world (inventory, dropped items).
pub fn icon_tint(b: &BlockDef) -> Rgba {
    match b.tint {
        TINT_GRASS => rgba(0x7DBD52, 255),
        TINT_FOLIAGE => rgba(0x64AA38, 255),
        TINT_WATER => rgba(0x3F76E4, 255),
        _ if b.fixed_tint != 0 => rgba(b.fixed_tint, 255),
        _ => WHITE,
    }
}

/// True for blocks drawn as a little cube in the inventory; the rest use a flat sprite.
pub fn icon_is_cube(b: &BlockDef) -> bool {
    matches!(b.shape, Shape::Cube | Shape::Grass | Shape::Pillar | Shape::Facing | Shape::FacingLit | Shape::Slab | Shape::Stairs | Shape::Farmland | Shape::Low(_) | Shape::Chest | Shape::Cactus | Shape::Layers)
}

/// Draws a block as a small isometric cube (or as a flat sprite for non-cubic shapes).
pub fn block_icon(batch: &mut Batch, x: f32, y: f32, size: f32, state: State) {
    let r = reg();
    if state == AIR {
        return;
    }
    let b = r.block(state);
    let ms = &r.mesh_states[state as usize];
    let layers = r.texsets[ms.texset as usize].layer;
    let tint = icon_tint(b);
    if !icon_is_cube(b) {
        batch.tile(x, y, size, layers[0], tint);
        return;
    }
    let s = size / 16.0;
    let height = match b.shape {
        Shape::Slab => 8.0,
        Shape::Low(h) => h as f32,
        Shape::Layers => 2.0,
        _ => 16.0,
    };
    let drop = (16.0 - height) * 0.5 * s;
    let p = |px: f32, py: f32| [x + px * s, y + py * s, 0.0];
    let q = |px: f32, py: f32| [x + px * s, y + py * s + drop, 0.0];
    let side_v = height / 16.0;
    let grass = b.shape == Shape::Grass;
    let top_tint = if grass || b.tint != TINT_NONE { tint } else { WHITE };
    let side_tint = if grass { WHITE } else { top_tint };
    // Top, then the south (left) and east (right) faces.
    batch.quad([q(0.0, 4.0), q(8.0, 8.0), q(16.0, 4.0), q(8.0, 0.0)], [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]], layers[1] as f32, top_tint);
    let left = [q(0.0, 4.0), p(0.0, 12.0), p(8.0, 16.0), q(8.0, 8.0)];
    let right = [q(8.0, 8.0), p(8.0, 16.0), p(16.0, 12.0), q(16.0, 4.0)];
    let uv = [[0.0, 1.0 - side_v], [0.0, 1.0], [1.0, 1.0], [1.0, 1.0 - side_v]];
    batch.quad(left, uv, layers[3] as f32, shade(side_tint, 0.8));
    batch.quad(right, uv, layers[5] as f32, shade(side_tint, 0.6));
    if grass {
        batch.quad(left, uv, layers[6] as f32, shade(tint, 0.8));
        batch.quad(right, uv, layers[6] as f32, shade(tint, 0.6));
    }
}

/// Draws an item stack in a 16x16 slot: icon, durability bar and count.
pub fn draw_stack(b: &mut Batch, font: &Font, x: f32, y: f32, s: ItemStack) {
    if s.is_empty() {
        return;
    }
    let r = reg();
    let def = s.def();
    match def.block {
        Some(bi) if def.icon == 0 => block_icon(b, x, y, 16.0, r.default_of(&r.blocks[bi as usize])),
        _ => b.tile(x, y, 16.0, def.icon, WHITE),
    }
    if def.durability > 0 && s.damage > 0 {
        let f = 1.0 - s.damage as f32 / def.durability as f32;
        b.rect(x + 2.0, y + 13.0, 13.0, 2.0, rgba(0x000000, 255));
        b.rect(x + 2.0, y + 13.0, (13.0 * f).ceil(), 1.0, [(255.0 * (1.0 - f)) as u8, (255.0 * f) as u8, 0, 255]);
    }
    if s.count > 1 {
        let t = s.count.to_string();
        b.text_shadow(font, x + 17.0 - font.width(&t), y + 9.0, &t, 1.0, WHITE);
    }
}

/// `oak_planks` -> `Oak Planks`.
pub fn pretty_name(id: &str) -> String {
    id.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn centered(b: &mut Batch, font: &Font, w: f32, y: f32, text: &str, scale: f32, color: Rgba) {
    b.text_shadow(font, ((w - font.width(text) * scale) * 0.5).floor(), y, text, scale, color);
}

pub fn draw(g: &mut Game, w: f32, h: f32) {
    let now = platform::now_ms();
    let r = reg();
    let font = &g.renderer.font;
    let b = &mut g.renderer.batch;
    if g.loading {
        b.rect(0.0, 0.0, w, h, rgba(0x15120F, 255));
        centered(b, font, w, h * 0.5 - 4.0, "Loading terrain...", 1.0, WHITE);
        return;
    }
    if g.hide_hud {
        return;
    }
    let survival = g.player.mode == GameMode::Survival;
    let spectator = g.player.mode == GameMode::Spectator;

    // Damage flash and the underwater tint come first so the widgets stay readable.
    if g.vitals.hurt_time > 0 && survival {
        b.rect(0.0, 0.0, w, h, rgba(0xB00000, (g.vitals.hurt_time as u32 * 5).min(90) as u8));
    }
    if g.screen == Screen::None {
        b.icon((w * 0.5 - 4.5).floor(), (h * 0.5 - 4.5).floor(), R_CROSSHAIR, rgba(0xFFFFFF, 200));
    }

    let x0 = ((w - 182.0) * 0.5).floor();
    let y0 = h - 22.0;
    if !spectator {
        // Hotbar.
        b.rect(x0, y0, 182.0, 22.0, rgba(0x000000, 90));
        for i in 0..9 {
            let sx = x0 + 1.0 + i as f32 * 20.0;
            b.icon(sx, y0 + 1.0, R_SLOT, WHITE);
            draw_stack(b, font, sx + 2.0, y0 + 3.0, g.inv.slots[i]);
        }
        b.icon(x0 - 1.0 + g.inv.selected as f32 * 20.0, y0 - 1.0, R_SLOT_SELECTED, WHITE);
    }

    if survival {
        // Hearts shake when health is low and blink right after a hit.
        let health = g.vitals.health.ceil() as i32;
        let blink = g.vitals.hurt_time > 10 && (g.vitals.hurt_time / 3) % 2 == 0;
        let y = h - 39.0;
        for i in 0..10 {
            let jitter = if health <= 4 { ((g.tick as i32 * 7 + i * 13) % 3 - 1) as f32 } else { 0.0 };
            let x = x0 + i as f32 * 8.0;
            b.icon(x, y + jitter, R_HEART_EMPTY, if blink { rgba(0xFFFFFF, 255) } else { rgba(0xB0B0B0, 255) });
            if i * 2 + 1 < health {
                b.icon(x, y + jitter, R_HEART_FULL, WHITE);
            } else if i * 2 + 1 == health {
                b.icon(x, y + jitter, R_HEART_HALF, WHITE);
            }
        }
        let food = g.vitals.food;
        for i in 0..10 {
            let jitter = if g.vitals.saturation <= 0.0 && g.tick % (food as u64 * 3 + 1) == 0 { ((g.tick as i32 + i * 5) % 3 - 1) as f32 } else { 0.0 };
            let x = x0 + 182.0 - 9.0 - i as f32 * 8.0;
            b.icon(x, y + jitter, R_FOOD_EMPTY, WHITE);
            if i * 2 + 1 < food {
                b.icon(x, y + jitter, R_FOOD_FULL, WHITE);
            } else if i * 2 + 1 == food {
                b.icon(x, y + jitter, R_FOOD_HALF, WHITE);
            }
        }
        if g.vitals.air < 300 {
            let bubbles = (((g.vitals.air - 2) as f32 * 10.0 / 300.0).ceil() as i32).clamp(0, 10);
            for i in 0..bubbles {
                b.icon(x0 + 182.0 - 9.0 - i as f32 * 8.0, y - 10.0, R_BUBBLE, WHITE);
            }
        }
    }

    // Name of the held item, shown briefly after switching slots.
    let held = g.inv.held();
    let since = now - g.held_changed;
    if !held.is_empty() && since < 2000.0 && g.screen == Screen::None && !spectator {
        let name = pretty_name(held.def().name);
        let alpha = (((2000.0 - since) / 500.0).min(1.0) * 255.0) as u8;
        let y = if survival { h - 59.0 } else { h - 45.0 };
        b.text_shadow(font, ((w - font.width(&name)) * 0.5).floor(), y, &name, 1.0, [255, 255, 255, alpha]);
    }

    // Chat: recent lines fade out; with the chat box open the whole backlog is shown.
    let chatting = g.screen == Screen::Chat;
    let mut y = h - if chatting { 28.0 } else { 48.0 };
    for (text, at) in g.messages.iter().rev().take(if chatting { 14 } else { 8 }) {
        let age = (now - at) / 1000.0;
        if age > 10.0 && !chatting {
            break;
        }
        let alpha = if chatting { 255 } else { ((10.0 - age).min(1.0) * 255.0) as u8 };
        b.rect(2.0, y - 1.0, font.width(text).max(160.0) + 4.0, LINE_HEIGHT, rgba(0x000000, alpha / 2));
        b.text_shadow(font, 4.0, y, text, 1.0, [255, 255, 255, alpha]);
        y -= LINE_HEIGHT;
    }

    if g.show_debug {
        let p = g.player.pos;
        let (bx, by, bz) = (p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        let yaw = wrap_degrees(g.player.yaw);
        let facing = match ((yaw / 90.0 + 0.5).floor() as i32).rem_euclid(4) {
            0 => "south (Towards positive Z)",
            1 => "west (Towards negative X)",
            2 => "north (Towards negative Z)",
            _ => "east (Towards positive X)",
        };
        let light = g.world.light(bx, by, bz);
        let stats = g.renderer.chunks.stats;
        let mut left = vec![
            format!("Rustcraft Redefined {} (native core v{})", env!("CARGO_PKG_VERSION"), unsafe { crate::ffi::vc_core_version() }),
            format!("{:.0} fps, {:.1} ms/frame", g.fps, g.frame_ms),
            format!("C: {} sections, {} quads, {} chunks meshed, {} loaded", stats.sections_drawn, stats.quads_drawn, stats.chunks, g.world.chunks.len()),
            format!("E: {} items, {} block entities", g.drops.len(), g.block_entities.len()),
            String::new(),
            format!("XYZ: {:.3} / {:.5} / {:.3}", p.x, p.y, p.z),
            format!("Block: {bx} {by} {bz}   Chunk: {} {} {}", bx >> 4, by >> 4, bz >> 4),
            format!("Facing: {facing} ({:.1} / {:.1})", yaw, g.player.pitch),
            format!("Light: sky {} block {}", light >> 4, light & 15),
            format!("Biome: {}", g.world.biome(bx, bz).name()),
            format!("Day {} time {}   Seed {}", g.day_time / super::DAY_TICKS, g.day_time % super::DAY_TICKS, g.world.seed as i64),
            format!("Mode: {:?}{}{}", g.player.mode, if g.player.flying { " flying" } else { "" }, if g.player.on_ground { " on ground" } else { "" }),
            format!("Health {:.1}  Food {}  Saturation {:.1}  Air {}", g.vitals.health, g.vitals.food, g.vitals.saturation, g.vitals.air),
        ];
        if let Some(hit) = g.target {
            left.push(String::new());
            left.push(format!("Targeted block: {} {} {}", hit.pos.x, hit.pos.y, hit.pos.z));
            left.push(r.describe(hit.state));
        }
        let mut y = 2.0;
        for line in &left {
            if !line.is_empty() {
                b.rect(1.0, y - 1.0, font.width(line) + 3.0, LINE_HEIGHT, rgba(0x505050, 120));
                b.text(font, 2.0, y, line, 1.0, rgba(0xE0E0E0, 255));
            }
            y += LINE_HEIGHT;
        }
    }
    if g.screen == Screen::None && !g.grabbed {
        b.rect(0.0, 0.0, w, h, rgba(0x000000, 90));
        centered(b, font, w, (h * 0.5 - 30.0).floor(), "Click to resume", 2.0, WHITE);
    }
}
