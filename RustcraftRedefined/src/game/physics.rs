//! Collision against block shapes, swept movement with step-up, and voxel ray casting.

use crate::util::math::{Aabb, DVec3, IVec3, DIRS};
use crate::world::block::{reg, State, F_FLUID, F_SOLID};
use crate::world::World;

/// Collects the collision boxes of all blocks that may touch `region`.
pub fn block_boxes(world: &World, region: &Aabb, out: &mut Vec<Aabb>) {
    out.clear();
    let r = reg();
    let (x0, x1) = ((region.min.x - 1e-7).floor() as i32, (region.max.x + 1e-7).floor() as i32);
    // Tall collision shapes (fences, walls) reach half a block above their cell.
    let (y0, y1) = ((region.min.y - 0.5 - 1e-7).floor() as i32, (region.max.y + 1e-7).floor() as i32);
    let (z0, z1) = ((region.min.z - 1e-7).floor() as i32, (region.max.z + 1e-7).floor() as i32);
    for y in y0..=y1 {
        for z in z0..=z1 {
            for x in x0..=x1 {
                let s = world.get(x, y, z);
                if !r.has(s, F_SOLID) {
                    continue;
                }
                for b in r.collision(s) {
                    out.push(Aabb::new(x as f64 + b[0] as f64, y as f64 + b[1] as f64, z as f64 + b[2] as f64, x as f64 + b[3] as f64, y as f64 + b[4] as f64, z as f64 + b[5] as f64));
                }
            }
        }
    }
}

fn collide(boxes: &[Aabb], bb: &Aabb, d: DVec3) -> DVec3 {
    let mut b = *bb;
    let (mut dx, mut dy, mut dz) = (d.x, d.y, d.z);
    if dy != 0.0 {
        for c in boxes {
            dy = c.clip_axis(&b, 1, dy);
        }
        b = b.offset(DVec3::new(0.0, dy, 0.0));
    }
    let x_first = dx.abs() >= dz.abs();
    if x_first && dx != 0.0 {
        for c in boxes {
            dx = c.clip_axis(&b, 0, dx);
        }
        b = b.offset(DVec3::new(dx, 0.0, 0.0));
    }
    if dz != 0.0 {
        for c in boxes {
            dz = c.clip_axis(&b, 2, dz);
        }
        b = b.offset(DVec3::new(0.0, 0.0, dz));
    }
    if !x_first && dx != 0.0 {
        for c in boxes {
            dx = c.clip_axis(&b, 0, dx);
        }
    }
    DVec3::new(dx, dy, dz)
}

/// Result of a swept move.
#[derive(Clone, Copy, Debug, Default)]
pub struct MoveResult {
    pub delta: DVec3,
    pub hit_x: bool,
    pub hit_y: bool,
    pub hit_z: bool,
    pub on_ground: bool,
}

/// Moves a box through the world, sliding along obstacles and stepping up ledges of at most `step`.
pub fn sweep(world: &World, bb: &Aabb, want: DVec3, step: f64, was_on_ground: bool, scratch: &mut Vec<Aabb>) -> MoveResult {
    let reach = bb.stretch(want).stretch(DVec3::new(0.0, step, 0.0)).stretch(DVec3::new(0.0, -step, 0.0));
    block_boxes(world, &reach, scratch);
    let mut moved = collide(scratch, bb, want);
    let hit_x = moved.x != want.x;
    let hit_z = moved.z != want.z;
    let hit_y = moved.y != want.y;
    let grounded = was_on_ground || (hit_y && want.y < 0.0);
    if step > 0.0 && grounded && (hit_x || hit_z) {
        let mut up = collide(scratch, bb, DVec3::new(want.x, step, want.z));
        let lift = collide(scratch, &bb.stretch(DVec3::new(want.x, 0.0, want.z)), DVec3::new(0.0, step, 0.0));
        if lift.y < step {
            let alt = collide(scratch, &bb.offset(lift), DVec3::new(want.x, 0.0, want.z)) + lift;
            if alt.x * alt.x + alt.z * alt.z > up.x * up.x + up.z * up.z {
                up = alt;
            }
        }
        if up.x * up.x + up.z * up.z > moved.x * moved.x + moved.z * moved.z {
            let settle = collide(scratch, &bb.offset(up), DVec3::new(0.0, -up.y + want.y.min(0.0), 0.0));
            moved = up + settle;
        }
    }
    MoveResult { delta: moved, hit_x: moved.x != want.x, hit_y: moved.y != want.y, hit_z: moved.z != want.z, on_ground: (moved.y != want.y || hit_y) && want.y < 0.0 }
}

/// True when `bb` overlaps no block collision box.
pub fn is_free(world: &World, bb: &Aabb, scratch: &mut Vec<Aabb>) -> bool {
    block_boxes(world, bb, scratch);
    !scratch.iter().any(|b| b.intersects(bb))
}

/// Highest fluid surface overlapping the box, as (state flags matched, depth of the box under the surface).
pub fn fluid_depth(world: &World, bb: &Aabb, flag: u16) -> f64 {
    let r = reg();
    let mut depth: f64 = 0.0;
    for y in bb.min.y.floor() as i32..=bb.max.y.floor() as i32 {
        for z in bb.min.z.floor() as i32..=bb.max.z.floor() as i32 {
            for x in bb.min.x.floor() as i32..=bb.max.x.floor() as i32 {
                let s = world.get(x, y, z);
                if !r.has(s, flag) {
                    continue;
                }
                let above_same = r.has(world.get(x, y + 1, z), flag);
                let h = if above_same || !r.has(s, F_FLUID) {
                    1.0
                } else {
                    let level = r.local(s) as f64;
                    if level >= 8.0 { 1.0 } else { (8.0 - level) / 9.0 }
                };
                let surface = y as f64 + h;
                if surface > bb.min.y {
                    depth = depth.max(surface - bb.min.y);
                }
            }
        }
    }
    depth
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub pos: IVec3,
    /// Face of the block that was hit (direction index pointing out of the block).
    pub face: usize,
    pub point: DVec3,
    pub state: State,
    pub distance: f64,
}

/// Casts a ray through the voxel grid and returns the first block whose outline it hits.
pub fn raycast(world: &World, origin: DVec3, dir: DVec3, max_dist: f64, hit_fluids: bool) -> Option<Hit> {
    let r = reg();
    let dir = dir.normalized();
    let mut cell = origin.floor();
    let step = IVec3::new(if dir.x > 0.0 { 1 } else { -1 }, if dir.y > 0.0 { 1 } else { -1 }, if dir.z > 0.0 { 1 } else { -1 });
    let inv = |d: f64| if d.abs() < 1e-12 { f64::INFINITY } else { 1.0 / d.abs() };
    let t_delta = DVec3::new(inv(dir.x), inv(dir.y), inv(dir.z));
    let first = |o: f64, c: i32, d: f64, td: f64| if td.is_infinite() { f64::INFINITY } else if d > 0.0 { (c as f64 + 1.0 - o) * td } else { (o - c as f64) * td };
    let mut t_max = DVec3::new(first(origin.x, cell.x, dir.x, t_delta.x), first(origin.y, cell.y, dir.y, t_delta.y), first(origin.z, cell.z, dir.z, t_delta.z));
    let mut travelled = 0.0;
    let mut entry_face = 1usize;
    for _ in 0..512 {
        if travelled > max_dist {
            return None;
        }
        let s = world.get_at(cell);
        if s != 0 && (hit_fluids || !r.has(s, F_FLUID)) {
            let boxes = r.outline(s);
            if r.has(s, F_FLUID) {
                return Some(Hit { pos: cell, face: entry_face, point: origin + dir * travelled, state: s, distance: travelled });
            }
            let mut best: Option<(f64, usize)> = None;
            for b in boxes {
                let bb = Aabb::new(cell.x as f64 + b[0] as f64, cell.y as f64 + b[1] as f64, cell.z as f64 + b[2] as f64, cell.x as f64 + b[3] as f64, cell.y as f64 + b[4] as f64, cell.z as f64 + b[5] as f64);
                if let Some((t, face)) = bb.raycast(origin, dir, max_dist) {
                    if best.map_or(true, |(bt, _)| t < bt) {
                        best = Some((t, if t <= 1e-9 { entry_face } else { face }));
                    }
                }
            }
            if let Some((t, face)) = best {
                return Some(Hit { pos: cell, face, point: origin + dir * t, state: s, distance: t });
            }
        }
        if t_max.x < t_max.y && t_max.x < t_max.z {
            travelled = t_max.x;
            t_max.x += t_delta.x;
            cell.x += step.x;
            entry_face = if step.x > 0 { 4 } else { 5 };
        } else if t_max.y < t_max.z {
            travelled = t_max.y;
            t_max.y += t_delta.y;
            cell.y += step.y;
            entry_face = if step.y > 0 { 0 } else { 1 };
        } else {
            travelled = t_max.z;
            t_max.z += t_delta.z;
            cell.z += step.z;
            entry_face = if step.z > 0 { 2 } else { 3 };
        }
    }
    None
}

/// Block position adjacent to `pos` across `face`.
pub fn neighbour(pos: IVec3, face: usize) -> IVec3 {
    let d = DIRS[face];
    IVec3::new(pos.x + d.0, pos.y + d.1, pos.z + d.2)
}
