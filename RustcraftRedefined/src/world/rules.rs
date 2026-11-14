//! Block behaviour that depends on neighbours: placement orientation, connections
//! (fences, walls, panes, stair corners), support checks and gravity.

use super::block::*;
use super::model::{facing_dir, facing_value, Shape};
use super::World;
use crate::ffi::MIN_Y;
use crate::util::math::{DVec3, IVec3, DOWN, EAST, NORTH, OPPOSITE, ROTATE_CCW, SOUTH, UP, WEST};
use std::collections::VecDeque;

/// Everything known about a block placement attempt.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    /// Cell the new block goes into.
    pub pos: IVec3,
    /// Face of the existing block that was clicked.
    pub face: usize,
    /// Exact point that was clicked, in world coordinates.
    pub hit: DVec3,
    pub yaw: f32,
    pub pitch: f32,
}

/// Horizontal direction a player with this yaw is looking at.
pub fn facing_from_yaw(yaw: f32) -> usize {
    match ((yaw / 90.0 + 0.5).floor() as i32).rem_euclid(4) {
        0 => SOUTH,
        1 => WEST,
        2 => NORTH,
        _ => EAST,
    }
}

fn is_soil(name: &str) -> bool {
    matches!(name, "grass_block" | "dirt" | "coarse_dirt" | "podzol" | "rooted_dirt" | "moss_block" | "mud" | "mycelium" | "farmland")
}

fn connects(r: &Registry, me: Shape, other: State) -> bool {
    let b = r.block(other);
    if r.has(other, F_OPAQUE) && !r.has(other, F_LEAVES) {
        return true;
    }
    match me {
        Shape::Fence => b.shape == Shape::Fence && (b.sound == Sound::Wood) == true,
        Shape::Wall | Shape::Pane => matches!(b.shape, Shape::Wall | Shape::Pane),
        _ => false,
    }
}

fn stairs_shape(world: &World, r: &Registry, pos: IVec3, state: State) -> usize {
    let facing = facing_dir(r.prop(state, "facing").unwrap_or(0));
    let half = r.prop(state, "half");
    let stair_at = |p: IVec3| {
        let s = world.get_at(p);
        if r.block(s).shape == Shape::Stairs && r.prop(s, "half") == half { Some(facing_dir(r.prop(s, "facing").unwrap_or(0))) } else { None }
    };
    let free = |dir: usize| stair_at(pos.offset(dir)) != Some(facing);
    if let Some(bf) = stair_at(pos.offset(facing)) {
        if bf / 2 != facing / 2 && free(OPPOSITE[bf]) {
            return if bf == ROTATE_CCW[facing] { 3 } else { 4 };
        }
    }
    if let Some(ff) = stair_at(pos.offset(OPPOSITE[facing])) {
        if ff / 2 != facing / 2 && free(ff) {
            return if ff == ROTATE_CCW[facing] { 1 } else { 2 };
        }
    }
    0
}

/// True when the block can stay where it is.
pub fn can_survive(world: &World, pos: IVec3, state: State) -> bool {
    let r = reg();
    let b = r.block(state);
    let below = world.get_at(pos.offset(DOWN));
    let below_name = r.block(below).name;
    let sturdy_below = r.has(below, F_STURDY_TOP) || matches!(r.block(below).shape, Shape::Fence | Shape::Wall);
    match b.shape {
        Shape::Cross | Shape::Crop(_) if r.has(state, F_PLANT) => match b.name {
            "dead_bush" => is_soil(below_name) || below_name.ends_with("sand") || below_name.ends_with("terracotta"),
            "sugar_cane" => {
                below == state
                    || ((is_soil(below_name) || below_name.ends_with("sand"))
                        && (2..6).any(|d| {
                            let n = world.get_at(pos.offset(DOWN).offset(d));
                            r.has(n, F_WATER) || r.block(n).name == "ice"
                        }))
            }
            "brown_mushroom" | "red_mushroom" => r.has(below, F_OPAQUE),
            "seagrass" => r.has(below, F_OPAQUE),
            "kelp" => r.has(below, F_OPAQUE) || below == state,
            "cave_vines" => {
                let above = world.get_at(pos.offset(UP));
                r.has(above, F_OPAQUE) || above == state
            }
            "wheat" | "carrots" | "potatoes" | "beetroots" => below_name == "farmland",
            _ => is_soil(below_name),
        },
        Shape::DoublePlant | Shape::Door => {
            let upper = r.prop(state, "half") == Some(1);
            if upper {
                r.block(below).name == b.name && r.prop(below, "half") == Some(0)
            } else {
                let above = world.get_at(pos.offset(UP));
                let has_top = r.block(above).name == b.name && r.prop(above, "half") == Some(1);
                has_top && if b.shape == Shape::Door { sturdy_below } else { is_soil(below_name) }
            }
        }
        Shape::Cactus => (below == state || below_name.ends_with("sand")) && (2..6).all(|d| !r.has(world.get_at(pos.offset(d)), F_SOLID)),
        Shape::LilyPad => r.has(below, F_WATER) || below_name == "ice",
        Shape::Torch => match r.prop(state, "facing").unwrap_or(0) {
            0 => sturdy_below,
            f => r.has(world.get_at(pos.offset(OPPOSITE[1 + f])), F_OPAQUE),
        },
        Shape::Ladder => r.has(world.get_at(pos.offset(OPPOSITE[facing_dir(r.prop(state, "facing").unwrap_or(0))])), F_OPAQUE),
        Shape::Layers => sturdy_below && !below_name.ends_with("ice"),
        Shape::Low(1) => below != AIR,
        Shape::Spike => {
            if r.prop(state, "vertical_direction") == Some(0) {
                r.has(below, F_STURDY_TOP) || (r.block(below).name == b.name && r.prop(below, "vertical_direction") == Some(0))
            } else {
                let above = world.get_at(pos.offset(UP));
                r.has(above, F_OPAQUE) || (r.block(above).name == b.name && r.prop(above, "vertical_direction") == Some(1))
            }
        }
        _ => true,
    }
}

/// Recomputes the neighbour-dependent properties of a block.  Returns AIR when it cannot stay.
pub fn update_shape(world: &World, pos: IVec3, state: State) -> State {
    let r = reg();
    if state == AIR {
        return AIR;
    }
    if !can_survive(world, pos, state) {
        // Waterlogged plants leave their water behind.
        return if r.has(state, F_WATER) && !r.has(state, F_FLUID) { r.k.water } else { AIR };
    }
    let b = r.block(state);
    match b.shape {
        Shape::Fence | Shape::Wall | Shape::Pane => {
            let mut s = state;
            let mut n = [false; 4];
            for (i, (name, dir)) in [("north", NORTH), ("east", EAST), ("south", SOUTH), ("west", WEST)].iter().enumerate() {
                n[i] = connects(r, b.shape, world.get_at(pos.offset(*dir)));
                s = r.with(s, name, n[i] as usize);
            }
            if b.shape == Shape::Wall {
                let straight = (n[0] && n[2] && !n[1] && !n[3]) || (n[1] && n[3] && !n[0] && !n[2]);
                let covered = world.get_at(pos.offset(UP)) != AIR;
                s = r.with(s, "up", (!straight || covered) as usize);
            }
            s
        }
        Shape::Stairs => r.with(state, "shape", stairs_shape(world, r, pos, state)),
        Shape::Grass => {
            let above = r.block(world.get_at(pos.offset(UP))).name;
            r.with(state, "snowy", (above == "snow" || above == "snow_block") as usize)
        }
        Shape::Spike => {
            // Thickness follows the distance to the tip of the column.
            let up = r.prop(state, "vertical_direction") == Some(0);
            let step = if up { UP } else { DOWN };
            let same = |p: IVec3| {
                let s = world.get_at(p);
                r.block(s).name == b.name && r.prop(s, "vertical_direction") == r.prop(state, "vertical_direction")
            };
            let mut ahead = 0;
            let mut p = pos.offset(step);
            while ahead < 3 && same(p) {
                ahead += 1;
                p = p.offset(step);
            }
            let behind = same(pos.offset(OPPOSITE[step]));
            let thickness = match ahead {
                0 => 1,
                1 => 2,
                _ => if behind { 3 } else { 4 },
            };
            r.with(state, "thickness", thickness)
        }
        _ => state,
    }
}

/// State a block takes when a player places it, or `None` if it cannot be placed there.
pub fn placement_state(world: &World, block: &BlockDef, p: &Placement) -> Option<State> {
    let r = reg();
    let base = r.default_of(block);
    let existing = world.get_at(p.pos);
    let facing = facing_from_yaw(p.yaw);
    let fy = p.hit.y - p.hit.y.floor();
    let upper_half = p.face == DOWN || (p.face != UP && fy > 0.5);
    let flooded = r.has(existing, F_WATER) && r.has(existing, F_FLUID) && r.local(existing) == 0;
    let water = |s: State| if flooded && block.shape.get(0, "waterlogged").is_some() { r.with(s, "waterlogged", 1) } else { s };
    let state = match block.shape {
        Shape::Air | Shape::Fluid => return None,
        Shape::Pillar => r.with(base, "axis", [1, 1, 2, 2, 0, 0][p.face]),
        Shape::Facing | Shape::FacingLit | Shape::Chest => r.with(base, "facing", facing_value(OPPOSITE[facing])),
        Shape::Stairs => water(r.with(r.with(base, "facing", facing_value(facing)), "half", upper_half as usize)),
        Shape::Slab => {
            // Completing a half slab turns it into a double slab.
            if r.block(existing).name == block.name && r.prop(existing, "type").map_or(false, |t| t < 2) {
                return Some(r.with(r.with(existing, "type", 2), "waterlogged", 0));
            }
            water(r.with(base, "type", upper_half as usize))
        }
        Shape::Torch => {
            if p.face == DOWN {
                return None;
            }
            let wall = if p.face == UP { 0 } else { p.face - 1 };
            let s = r.with(base, "facing", wall);
            if can_survive(world, p.pos, s) {
                s
            } else {
                // Fall back to any wall or the floor that can hold the torch.
                (0..5).map(|f| r.with(base, "facing", f)).find(|s| can_survive(world, p.pos, *s))?
            }
        }
        Shape::Ladder => {
            if p.face < 2 {
                (0..4).map(|f| r.with(base, "facing", f)).find(|s| can_survive(world, p.pos, *s))?
            } else {
                r.with(base, "facing", facing_value(p.face))
            }
        }
        Shape::Door => {
            let hinge_right = {
                // Hinge on the side where the click landed, seen from the player.
                let (fx, fz) = (p.hit.x - p.hit.x.floor(), p.hit.z - p.hit.z.floor());
                match facing {
                    NORTH => fx > 0.5,
                    SOUTH => fx < 0.5,
                    WEST => fz < 0.5,
                    _ => fz > 0.5,
                }
            };
            r.with(r.with(base, "facing", facing_value(facing)), "hinge", hinge_right as usize)
        }
        Shape::Trapdoor => water(r.with(r.with(base, "facing", facing_value(OPPOSITE[facing])), "half", upper_half as usize)),
        Shape::Layers => {
            if r.block(existing).name == block.name {
                let n = r.prop(existing, "layers").unwrap_or(0);
                return if n < 7 { Some(r.with(existing, "layers", n + 1)) } else { None };
            }
            base
        }
        Shape::Spike => r.with(base, "vertical_direction", (p.face == DOWN || (p.face != UP && !r.has(world.get_at(p.pos.offset(DOWN)), F_STURDY_TOP))) as usize),
        _ => water(base),
    };
    // Two-block-tall things need room above.
    if matches!(block.shape, Shape::Door | Shape::DoublePlant) {
        let above = world.get_at(p.pos.offset(UP));
        if !r.has(above, F_REPLACEABLE) || p.pos.y + 1 >= crate::ffi::MAX_Y {
            return None;
        }
        return Some(state);
    }
    let shaped = update_shape_unchecked(world, p.pos, state);
    if can_survive(world, p.pos, shaped) { Some(shaped) } else { None }
}

fn update_shape_unchecked(world: &World, pos: IVec3, state: State) -> State {
    let s = update_shape(world, pos, state);
    if s == AIR || (s == reg().k.water && state != s) { state } else { s }
}

/// Places a block as a player would: sets it (and its second half), then updates the neighbours.
pub fn place(world: &mut World, block: &BlockDef, p: &Placement) -> bool {
    let r = reg();
    let Some(state) = placement_state(world, block, p) else { return false };
    if world.set_at(p.pos, state).is_none() {
        return false;
    }
    if matches!(block.shape, Shape::Door | Shape::DoublePlant) {
        world.set_at(p.pos.offset(UP), r.with(state, "half", 1));
    }
    notify_neighbours(world, p.pos);
    true
}

/// Removes a block (leaving water where it was waterlogged) and updates the neighbours.
/// Returns the blocks that lost their support and broke as a consequence.
pub fn remove(world: &mut World, pos: IVec3) -> Vec<(IVec3, State)> {
    let r = reg();
    let old = world.get_at(pos);
    let left = if r.has(old, F_WATER) && !r.has(old, F_FLUID) { r.k.water } else { AIR };
    world.set_at(pos, left);
    notify_neighbours(world, pos)
}

/// Right-click behaviour of a block.  Returns true when the click was consumed.
pub fn interact(world: &mut World, pos: IVec3) -> bool {
    let r = reg();
    let s = world.get_at(pos);
    let b = r.block(s);
    match b.shape {
        Shape::Door | Shape::Trapdoor if b.sound == Sound::Wood => {
            let open = 1 - r.prop(s, "open").unwrap_or(0);
            world.set_at(pos, r.with(s, "open", open));
            if b.shape == Shape::Door {
                let other = pos.offset(if r.prop(s, "half") == Some(0) { UP } else { DOWN });
                let o = world.get_at(other);
                if r.block(o).name == b.name {
                    world.set_at(other, r.with(o, "open", open));
                }
            }
            true
        }
        _ => false,
    }
}

/// Lets unsupported sand and gravel drop to the ground.
fn settle_gravity(world: &mut World, pos: IVec3, queue: &mut VecDeque<IVec3>) {
    let r = reg();
    let s = world.get_at(pos);
    if !r.has(s, F_GRAVITY) {
        return;
    }
    if world.animate_falls && world.falls.len() < 400 {
        let below = world.get(pos.x, pos.y - 1, pos.z);
        if pos.y > MIN_Y && (below == AIR || (r.has(below, F_REPLACEABLE) || !r.has(below, F_SOLID)) && world.is_ready(pos.x, pos.z)) && world.set_at(pos, AIR).is_some() {
            world.falls.push((pos, s));
            queue.push_back(pos);
        }
        return;
    }
    let mut y = pos.y;
    while y > MIN_Y && r.has(world.get(pos.x, y - 1, pos.z), F_REPLACEABLE) {
        y -= 1;
    }
    if y != pos.y {
        world.set_at(pos, AIR);
        world.set_block(pos.x, y, pos.z, s);
        queue.push_back(pos);
        queue.push_back(IVec3::new(pos.x, y, pos.z));
    }
}

/// Propagates shape updates outwards from a changed block.
/// Returns the position and former state of every block that broke because it lost its support.
pub fn notify_neighbours(world: &mut World, origin: IVec3) -> Vec<(IVec3, State)> {
    let mut popped = Vec::new();
    let mut queue: VecDeque<IVec3> = VecDeque::new();
    queue.push_back(origin);
    let mut budget = 4096;
    while let Some(p) = queue.pop_front() {
        budget -= 1;
        if budget == 0 {
            break;
        }
        for d in 0..6 {
            let n = p.offset(d);
            let s = world.get_at(n);
            if s == AIR {
                continue;
            }
            let new = update_shape(world, n, s);
            if new != s {
                if world.set_at(n, new).is_some() && (new == AIR || reg().has(new, F_FLUID)) {
                    popped.push((n, s));
                    queue.push_back(n);
                }
            } else {
                settle_gravity(world, n, &mut queue);
            }
        }
        settle_gravity(world, p, &mut queue);
    }
    popped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::chunk::{Chunk, STAGE_LIT};

    fn flat_world() -> World {
        let mut w = World::new(1);
        let stone = reg().k.stone;
        for cz in -1..=1 {
            for cx in -1..=1 {
                let mut c = Chunk::new(cx, cz);
                for z in 0..16 {
                    for x in 0..16 {
                        for y in 58..=60 {
                            c.set(x, y, z, stone);
                        }
                    }
                }
                c.stage = STAGE_LIT;
                w.chunks.insert((cx, cz), c);
            }
        }
        w
    }

    fn put(w: &mut World, name: &str, x: i32, y: i32, z: i32, face: usize, yaw: f32) -> bool {
        let p = Placement { pos: IVec3::new(x, y, z), face, hit: DVec3::new(x as f64 + 0.5, y as f64 + 0.2, z as f64 + 0.5), yaw, pitch: 0.0 };
        place(w, reg().block_named(name).unwrap(), &p)
    }

    #[test]
    fn yaw_maps_to_compass() {
        assert_eq!(facing_from_yaw(0.0), SOUTH);
        assert_eq!(facing_from_yaw(90.0), WEST);
        assert_eq!(facing_from_yaw(-180.0), NORTH);
        assert_eq!(facing_from_yaw(-90.0), EAST);
        assert_eq!(facing_from_yaw(44.0), SOUTH);
    }

    #[test]
    fn fences_connect_and_disconnect() {
        let mut w = flat_world();
        let r = reg();
        assert!(put(&mut w, "oak_fence", 2, 61, 2, UP, 0.0));
        assert!(put(&mut w, "oak_fence", 3, 61, 2, UP, 0.0));
        assert_eq!(r.describe(w.get(2, 61, 2)), "oak_fence[north=false,east=true,south=false,west=false,waterlogged=false]");
        assert_eq!(r.prop(w.get(3, 61, 2), "west"), Some(1));
        remove(&mut w, IVec3::new(3, 61, 2));
        assert_eq!(r.prop(w.get(2, 61, 2), "east"), Some(0));
        // Walls lose their post in a straight run.
        for x in 5..8 {
            assert!(put(&mut w, "cobblestone_wall", x, 61, 2, UP, 0.0));
        }
        assert_eq!(r.prop(w.get(6, 61, 2), "up"), Some(0));
        assert_eq!(r.prop(w.get(5, 61, 2), "up"), Some(1));
    }

    #[test]
    fn stairs_form_corners() {
        let mut w = flat_world();
        let r = reg();
        // Facing north (player looks north), then a stair facing east placed behind it.
        assert!(put(&mut w, "oak_stairs", 4, 61, 4, UP, 180.0));
        assert_eq!(r.prop(w.get(4, 61, 4), "facing"), Some(0));
        assert!(put(&mut w, "oak_stairs", 4, 61, 3, UP, -90.0));
        assert_eq!(r.prop(w.get(4, 61, 4), "shape"), Some(4), "{}", r.describe(w.get(4, 61, 4)));
        remove(&mut w, IVec3::new(4, 61, 3));
        assert_eq!(r.prop(w.get(4, 61, 4), "shape"), Some(0));
    }

    #[test]
    fn unsupported_blocks_pop_off_and_sand_falls() {
        let mut w = flat_world();
        let r = reg();
        assert!(put(&mut w, "torch", 1, 61, 1, UP, 0.0));
        assert!(!put(&mut w, "torch", 1, 63, 1, DOWN, 0.0), "torches do not hang from ceilings");
        assert!(put(&mut w, "oak_door", 6, 61, 6, UP, 0.0));
        assert_eq!(r.prop(w.get(6, 62, 6), "half"), Some(1));
        assert!(interact(&mut w, IVec3::new(6, 62, 6)));
        assert_eq!(r.prop(w.get(6, 61, 6), "open"), Some(1));
        remove(&mut w, IVec3::new(1, 60, 1));
        assert_eq!(w.get(1, 61, 1), AIR, "torch should break with its support");
        remove(&mut w, IVec3::new(6, 61, 6));
        assert_eq!(w.get(6, 62, 6), AIR, "upper door half should go with the lower");
        // Sand stacked on a block falls when that block is removed.
        w.set_block(9, 61, 9, r.k.stone);
        w.set_block(9, 62, 9, r.k.sand);
        w.set_block(9, 63, 9, r.k.sand);
        remove(&mut w, IVec3::new(9, 61, 9));
        assert_eq!(w.get(9, 61, 9), r.k.sand);
        assert_eq!(w.get(9, 62, 9), r.k.sand);
        assert_eq!(w.get(9, 63, 9), AIR);
        // Slabs merge into a double slab; logs take their axis from the clicked face.
        assert!(put(&mut w, "stone_slab", 11, 61, 11, UP, 0.0));
        assert!(put(&mut w, "stone_slab", 11, 61, 11, UP, 0.0));
        assert_eq!(r.prop(w.get(11, 61, 11), "type"), Some(2));
        assert!(put(&mut w, "oak_log", 12, 61, 12, EAST, 0.0));
        assert_eq!(r.prop(w.get(12, 61, 12), "axis"), Some(0));
    }
}
