//! Block geometry: every shape is described as quads for the mesher plus boxes for collision.

use crate::ffi::{VcQuad, DIR_NONE, Q_DOUBLE, Q_INSET, Q_NOSHADE, Q_TINT};
use crate::util::math::{DOWN, EAST, NORTH, OPPOSITE, ROTATE_CCW, ROTATE_CW, SOUTH, UP, WEST};

/// Axis-aligned box in block-local coordinates (0..1): x0, y0, z0, x1, y1, z1.
pub type Box6 = [f32; 6];
pub const FULL: Box6 = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];

/// Box given in sixteenths, the unit all models are authored in.
pub fn px(x0: f32, y0: f32, z0: f32, x1: f32, y1: f32, z1: f32) -> Box6 {
    [x0 / 16.0, y0 / 16.0, z0 / 16.0, x1 / 16.0, y1 / 16.0, z1 / 16.0]
}

/// Rotates a box around the block's vertical axis by `turns` quarter turns clockwise (seen from above).
pub fn rot_y(b: Box6, turns: usize) -> Box6 {
    let mut b = b;
    for _ in 0..turns % 4 {
        let (x0, z0, x1, z1) = (1.0 - b[5], b[0], 1.0 - b[2], b[3]);
        b = [x0, b[1], z0, x1, b[4], z1];
    }
    b
}

/// Quarter turns needed to rotate a model authored facing north so it faces `dir`.
pub fn turns_from_north(dir: usize) -> usize {
    match dir {
        EAST => 1,
        SOUTH => 2,
        WEST => 3,
        _ => 0,
    }
}

/// Per-face options for `Quads::cuboid`.
#[derive(Clone, Copy)]
pub struct Face {
    pub slot: u8,
    /// Quarter turns of texture rotation.
    pub rot: u8,
    pub flags: u8,
    /// Explicit texture rectangle (u0, v0, u1, v1) instead of the position-derived one.
    pub uv: Option<[f32; 4]>,
}

impl Face {
    pub fn slot(slot: u8) -> Face {
        Face { slot, rot: 0, flags: Q_TINT, uv: None }
    }
}

#[derive(Default)]
pub struct Quads(pub Vec<VcQuad>);

impl Quads {
    /// Adds the faces of a box.  `face(dir)` returns `None` to leave a face out.
    pub fn cuboid(&mut self, b: Box6, face: impl Fn(usize) -> Option<Face>) {
        let [x0, y0, z0, x1, y1, z1] = b;
        for dir in 0..6 {
            let Some(f) = face(dir) else { continue };
            let pos: [[f32; 3]; 4] = match dir {
                DOWN => [[x0, y0, z1], [x0, y0, z0], [x1, y0, z0], [x1, y0, z1]],
                UP => [[x0, y1, z0], [x0, y1, z1], [x1, y1, z1], [x1, y1, z0]],
                NORTH => [[x1, y0, z0], [x0, y0, z0], [x0, y1, z0], [x1, y1, z0]],
                SOUTH => [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]],
                WEST => [[x0, y0, z0], [x0, y0, z1], [x0, y1, z1], [x0, y1, z0]],
                _ => [[x1, y0, z1], [x1, y0, z0], [x1, y1, z0], [x1, y1, z1]],
            };
            let mut uv = [[0.0f32; 2]; 4];
            for (i, p) in pos.iter().enumerate() {
                let (u, v) = match dir {
                    DOWN => (p[0], 1.0 - p[2]),
                    UP => (p[0], p[2]),
                    NORTH => (1.0 - p[0], 1.0 - p[1]),
                    SOUTH => (p[0], 1.0 - p[1]),
                    WEST => (p[2], 1.0 - p[1]),
                    _ => (1.0 - p[2], 1.0 - p[1]),
                };
                uv[i] = [u, v];
            }
            if let Some([u0, v0, u1, v1]) = f.uv {
                // Normalise the position-derived coordinates to the face, then map into the rectangle.
                let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                for t in &uv {
                    for k in 0..2 {
                        lo[k] = lo[k].min(t[k]);
                        hi[k] = hi[k].max(t[k]);
                    }
                }
                for t in uv.iter_mut() {
                    let (a, c) = ((t[0] - lo[0]) / (hi[0] - lo[0]).max(1e-6), (t[1] - lo[1]) / (hi[1] - lo[1]).max(1e-6));
                    *t = [u0 + (u1 - u0) * a, v0 + (v1 - v0) * c];
                }
            }
            for _ in 0..f.rot % 4 {
                for t in uv.iter_mut() {
                    *t = [1.0 - t[1], t[0]];
                }
            }
            let plane = match dir {
                DOWN => y0,
                UP => y1,
                NORTH => z0,
                SOUTH => z1,
                WEST => x0,
                _ => x1,
            };
            let on_boundary = if dir % 2 == 0 { plane <= 0.0001 } else { plane >= 0.9999 };
            self.0.push(VcQuad {
                pos,
                uv,
                slot: f.slot,
                cull: if on_boundary { dir as u8 } else { DIR_NONE },
                dir: dir as u8,
                flags: f.flags | if on_boundary { 0 } else { Q_INSET },
            });
        }
    }

    /// Box with one texture slot per direction.
    pub fn simple(&mut self, b: Box6) {
        self.cuboid(b, |d| Some(Face::slot(d as u8)));
    }

    /// Box where every face uses the same slot.
    pub fn uniform(&mut self, b: Box6, slot: u8) {
        self.cuboid(b, |_| Some(Face::slot(slot)));
    }

    /// Free-standing double-sided quad lit from the block's own cell.
    pub fn plane(&mut self, pos: [[f32; 3]; 4], slot: u8, flip_v: bool) {
        let (top, bottom) = if flip_v { (1.0, 0.0) } else { (0.0, 1.0) };
        self.0.push(VcQuad {
            pos,
            uv: [[0.0, bottom], [1.0, bottom], [1.0, top], [0.0, top]],
            slot,
            cull: DIR_NONE,
            dir: UP as u8,
            flags: Q_TINT | Q_DOUBLE,
        });
    }

    /// Two diagonal planes (flowers, saplings, speleothems).
    pub fn cross(&mut self, slot: u8, flip_v: bool) {
        let (a, b) = (0.1, 0.9);
        self.plane([[a, 0.0, a], [b, 0.0, b], [b, 1.0, b], [a, 1.0, a]], slot, flip_v);
        self.plane([[a, 0.0, b], [b, 0.0, a], [b, 1.0, a], [a, 1.0, b]], slot, flip_v);
    }

    /// Four planes in a # layout (crops).
    pub fn hash(&mut self, slot: u8) {
        let (a, b) = (0.25, 0.75);
        self.plane([[a, 0.0, 0.0], [a, 0.0, 1.0], [a, 1.0, 1.0], [a, 1.0, 0.0]], slot, false);
        self.plane([[b, 0.0, 0.0], [b, 0.0, 1.0], [b, 1.0, 1.0], [b, 1.0, 0.0]], slot, false);
        self.plane([[0.0, 0.0, a], [1.0, 0.0, a], [1.0, 1.0, a], [0.0, 1.0, a]], slot, false);
        self.plane([[0.0, 0.0, b], [1.0, 0.0, b], [1.0, 1.0, b], [0.0, 1.0, b]], slot, false);
    }
}

/// True when the union of `boxes` covers the whole block face in direction `dir`.
pub fn covers_face(boxes: &[Box6], dir: usize) -> bool {
    let axis = [1, 1, 2, 2, 0, 0][dir];
    let (ua, va) = [(0, 2), (0, 2), (0, 1), (0, 1), (2, 1), (2, 1)][dir];
    let positive = dir % 2 == 1;
    for cv in 0..16 {
        for cu in 0..16 {
            let (u, v) = ((cu as f32 + 0.5) / 16.0, (cv as f32 + 0.5) / 16.0);
            let hit = boxes.iter().any(|b| {
                let touches = if positive { b[axis + 3] >= 0.9999 } else { b[axis] <= 0.0001 };
                touches && u > b[ua] && u < b[ua + 3] && v > b[va] && v < b[va + 3]
            });
            if !hit {
                return false;
            }
        }
    }
    true
}

/// Geometric kind of a block.  Each kind defines its state properties and its model.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Shape {
    Air,
    Cube,
    /// Cube with a tinted top and tinted side overlay; property `snowy`.
    Grass,
    Pillar,
    Facing,
    FacingLit,
    Cross,
    DoublePlant,
    Slab,
    Stairs,
    Fluid,
    Torch,
    Layers,
    /// Box of the given height in sixteenths (dirt path, carpets, plates).
    Low(u8),
    Farmland,
    /// Planted crop with the given number of growth stages.
    Crop(u8),
    Fence,
    Wall,
    Pane,
    Door,
    Trapdoor,
    Ladder,
    LilyPad,
    Spike,
    Chest,
    Cactus,
}

pub struct Prop {
    pub name: &'static str,
    pub values: &'static [&'static str],
}

const BOOL: &[&str] = &["false", "true"];
const FACING4: &[&str] = &["north", "south", "west", "east"];
const N16: &[&str] = &["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15"];
const P_FACING: Prop = Prop { name: "facing", values: FACING4 };
const P_WATER: Prop = Prop { name: "waterlogged", values: BOOL };
const P_HALF: Prop = Prop { name: "half", values: &["bottom", "top"] };
const P_OPEN: Prop = Prop { name: "open", values: BOOL };
const P_NORTH: Prop = Prop { name: "north", values: BOOL };
const P_EAST: Prop = Prop { name: "east", values: BOOL };
const P_SOUTH: Prop = Prop { name: "south", values: BOOL };
const P_WEST: Prop = Prop { name: "west", values: BOOL };

impl Shape {
    /// State properties, most significant first.
    pub fn props(self) -> &'static [Prop] {
        match self {
            Shape::Grass => &[Prop { name: "snowy", values: BOOL }],
            Shape::Pillar => &[Prop { name: "axis", values: &["x", "y", "z"] }],
            Shape::Facing | Shape::Chest | Shape::Ladder => &[P_FACING],
            Shape::FacingLit => &[P_FACING, Prop { name: "lit", values: BOOL }],
            Shape::DoublePlant => &[Prop { name: "half", values: &["lower", "upper"] }],
            Shape::Slab => &[Prop { name: "type", values: &["bottom", "top", "double"] }, P_WATER],
            Shape::Stairs => &[
                P_FACING,
                P_HALF,
                Prop { name: "shape", values: &["straight", "inner_left", "inner_right", "outer_left", "outer_right"] },
                P_WATER,
            ],
            Shape::Fluid => &[Prop { name: "level", values: N16 }],
            Shape::Torch => &[Prop { name: "facing", values: &["up", "north", "south", "west", "east"] }],
            Shape::Layers => &[Prop { name: "layers", values: &["1", "2", "3", "4", "5", "6", "7", "8"] }],
            Shape::Farmland => &[Prop { name: "moisture", values: &["0", "1", "2", "3", "4", "5", "6", "7"] }],
            Shape::Crop(8) => &[Prop { name: "age", values: &["0", "1", "2", "3", "4", "5", "6", "7"] }],
            Shape::Crop(_) => &[Prop { name: "age", values: &["0", "1", "2", "3"] }],
            Shape::Fence | Shape::Pane => &[P_NORTH, P_EAST, P_SOUTH, P_WEST, P_WATER],
            Shape::Wall => &[Prop { name: "up", values: BOOL }, P_NORTH, P_EAST, P_SOUTH, P_WEST, P_WATER],
            Shape::Door => &[P_FACING, Prop { name: "half", values: &["lower", "upper"] }, Prop { name: "hinge", values: &["left", "right"] }, P_OPEN],
            Shape::Trapdoor => &[P_FACING, P_HALF, P_OPEN, P_WATER],
            Shape::Spike => &[
                Prop { name: "vertical_direction", values: &["up", "down"] },
                Prop { name: "thickness", values: &["tip_merge", "tip", "frustum", "middle", "base"] },
                P_WATER,
            ],
            _ => &[],
        }
    }

    pub fn state_count(self) -> u16 {
        self.props().iter().map(|p| p.values.len() as u16).product()
    }

    /// Value index of property `name` in local state `local`.
    pub fn get(self, local: u16, name: &str) -> Option<usize> {
        let mut stride = self.state_count() as usize;
        let mut rest = local as usize;
        for p in self.props() {
            stride /= p.values.len();
            let v = rest / stride;
            rest %= stride;
            if p.name == name {
                return Some(v);
            }
        }
        None
    }

    /// Local state with property `name` set to value index `value`.
    pub fn with(self, local: u16, name: &str, value: usize) -> u16 {
        let mut stride = self.state_count() as usize;
        let mut rest = local as usize;
        for p in self.props() {
            stride /= p.values.len();
            let v = rest / stride;
            rest %= stride;
            if p.name == name && value < p.values.len() {
                return (local as usize - v * stride + value * stride) as u16;
            }
        }
        local
    }

    pub fn flag(self, local: u16, name: &str) -> bool {
        self.get(local, name) == Some(1)
    }
}

/// Direction index for a value of the four-way `facing` property.
#[inline]
pub fn facing_dir(value: usize) -> usize {
    2 + (value & 3)
}
#[inline]
pub fn facing_value(dir: usize) -> usize {
    dir.max(2) - 2
}

/// Everything derived from a (shape, local state) pair.
pub struct Built {
    pub quads: Vec<VcQuad>,
    pub collision: Vec<Box6>,
    /// Selection outline; falls back to the collision boxes when empty.
    pub outline: Vec<Box6>,
    pub waterlogged: bool,
}

fn stair_boxes(facing: usize, top: bool, shape: usize) -> Vec<Box6> {
    let turns = turns_from_north(facing);
    let mut v = vec![if top { px(0.0, 8.0, 0.0, 16.0, 16.0, 16.0) } else { px(0.0, 0.0, 0.0, 16.0, 8.0, 16.0) }];
    let (y0, y1) = if top { (0.0, 8.0) } else { (8.0, 16.0) };
    let mut part = |x0: f32, z0: f32, x1: f32, z1: f32| v.push(rot_y(px(x0, y0, z0, x1, y1, z1), turns));
    match shape {
        0 => part(0.0, 0.0, 16.0, 8.0),
        1 => {
            part(0.0, 0.0, 16.0, 8.0);
            part(0.0, 8.0, 8.0, 16.0);
        }
        2 => {
            part(0.0, 0.0, 16.0, 8.0);
            part(8.0, 8.0, 16.0, 16.0);
        }
        3 => part(0.0, 0.0, 8.0, 8.0),
        _ => part(8.0, 0.0, 16.0, 8.0),
    }
    v
}

/// Side of the block a door leaf occupies.
pub fn door_side(facing: usize, open: bool, hinge_right: bool) -> usize {
    if !open {
        OPPOSITE[facing]
    } else if hinge_right {
        ROTATE_CCW[facing]
    } else {
        ROTATE_CW[facing]
    }
}

/// Thin box hugging one side of the block.
pub fn side_slab(side: usize, thickness: f32, y0: f32, y1: f32) -> Box6 {
    let t = thickness;
    match side {
        NORTH => px(0.0, y0, 0.0, 16.0, y1, t),
        SOUTH => px(0.0, y0, 16.0 - t, 16.0, y1, 16.0),
        WEST => px(0.0, y0, 0.0, t, y1, 16.0),
        _ => px(16.0 - t, y0, 0.0, 16.0, y1, 16.0),
    }
}

pub fn build(shape: Shape, local: u16) -> Built {
    let mut q = Quads::default();
    let mut collision: Vec<Box6> = Vec::new();
    let mut outline: Vec<Box6> = Vec::new();
    let g = |name: &str| shape.get(local, name).unwrap_or(0);
    let waterlogged = shape.flag(local, "waterlogged");
    let connect = |q: &mut Quads, col: &mut Vec<Box6>, arms: &[Box6], tall: f32| {
        for (i, name) in ["north", "east", "south", "west"].iter().enumerate() {
            if shape.flag(local, name) {
                for (n, arm) in arms.iter().enumerate() {
                    let b = rot_y(*arm, i);
                    q.uniform(b, 0);
                    if n == 0 {
                        let mut c = rot_y(arms[arms.len() - 1], i);
                        c[1] = 0.0;
                        c[4] = tall;
                        col.push(c);
                    }
                }
            }
        }
    };
    match shape {
        Shape::Air | Shape::Fluid => {}
        Shape::Cube | Shape::Facing | Shape::FacingLit => {
            q.simple(FULL);
            collision.push(FULL);
        }
        Shape::Grass => {
            let snowy = g("snowy") == 1;
            q.cuboid(FULL, |d| {
                Some(match d {
                    UP => Face::slot(1),
                    DOWN => Face { flags: 0, ..Face::slot(0) },
                    _ => Face { flags: 0, ..Face::slot(if snowy { 7 } else { d as u8 }) },
                })
            });
            if !snowy {
                q.cuboid(FULL, |d| if d >= 2 { Some(Face::slot(6)) } else { None });
            }
            collision.push(FULL);
        }
        Shape::Pillar => {
            let axis = g("axis");
            q.cuboid(FULL, |d| {
                let rot = match (axis, d) {
                    (0, DOWN) | (0, UP) | (0, NORTH) | (0, SOUTH) | (2, WEST) | (2, EAST) => 1,
                    _ => 0,
                };
                Some(Face { rot, ..Face::slot(d as u8) })
            });
            collision.push(FULL);
        }
        Shape::Cross | Shape::DoublePlant => {
            q.cross(0, false);
            outline.push(px(2.0, 0.0, 2.0, 14.0, 13.0, 14.0));
        }
        Shape::Slab => {
            let b = match g("type") {
                0 => px(0.0, 0.0, 0.0, 16.0, 8.0, 16.0),
                1 => px(0.0, 8.0, 0.0, 16.0, 16.0, 16.0),
                _ => FULL,
            };
            q.simple(b);
            collision.push(b);
        }
        Shape::Stairs => {
            collision = stair_boxes(facing_dir(g("facing")), g("half") == 1, g("shape"));
            for b in &collision {
                q.simple(*b);
            }
        }
        Shape::Torch => {
            let f = g("facing");
            let stick = px(7.0, 0.0, 7.0, 9.0, 10.0, 9.0);
            let before = q.0.len();
            q.cuboid(stick, |d| match d {
                DOWN => None,
                UP => Some(Face { uv: Some([7.0 / 16.0, 6.0 / 16.0, 9.0 / 16.0, 8.0 / 16.0]), flags: Q_NOSHADE, ..Face::slot(0) }),
                _ => Some(Face { flags: Q_NOSHADE, ..Face::slot(0) }),
            });
            if f == 0 {
                outline.push(px(6.0, 0.0, 6.0, 10.0, 10.0, 10.0));
            } else {
                // Lean the stick against the wall behind it (the wall is opposite to `facing`).
                let dir = 1 + f; // 2..5
                let back = OPPOSITE[dir];
                let (ax, sign) = match back {
                    NORTH => (2, -1.0),
                    SOUTH => (2, 1.0),
                    WEST => (0, -1.0),
                    _ => (0, 1.0),
                };
                for quad in q.0[before..].iter_mut() {
                    quad.cull = DIR_NONE;
                    for p in quad.pos.iter_mut() {
                        let lean = (10.0 / 16.0 - p[1]) * 0.4 + 2.0 / 16.0;
                        p[ax] += sign * lean;
                        p[1] += 3.0 / 16.0;
                    }
                }
                outline.push(match back {
                    NORTH => px(5.5, 3.0, 0.0, 10.5, 13.0, 5.0),
                    SOUTH => px(5.5, 3.0, 11.0, 10.5, 13.0, 16.0),
                    WEST => px(0.0, 3.0, 5.5, 5.0, 13.0, 10.5),
                    _ => px(11.0, 3.0, 5.5, 16.0, 13.0, 10.5),
                });
            }
        }
        Shape::Layers => {
            let n = g("layers") as f32 + 1.0;
            let b = px(0.0, 0.0, 0.0, 16.0, n * 2.0, 16.0);
            q.simple(b);
            outline.push(b);
            if n > 1.0 {
                collision.push(px(0.0, 0.0, 0.0, 16.0, (n - 1.0) * 2.0, 16.0));
            }
        }
        Shape::Low(h) => {
            let b = px(0.0, 0.0, 0.0, 16.0, h as f32, 16.0);
            q.simple(b);
            if h >= 2 {
                collision.push(b);
            } else {
                outline.push(b);
            }
        }
        Shape::Farmland => {
            let b = px(0.0, 0.0, 0.0, 16.0, 15.0, 16.0);
            q.simple(b);
            collision.push(b);
        }
        Shape::Crop(_) => {
            q.hash(0);
            outline.push(px(0.0, 0.0, 0.0, 16.0, 4.0 + 1.5 * g("age") as f32, 16.0));
        }
        Shape::Fence => {
            let post = px(6.0, 0.0, 6.0, 10.0, 16.0, 10.0);
            q.uniform(post, 0);
            collision.push([post[0], 0.0, post[2], post[3], 1.5, post[5]]);
            connect(&mut q, &mut collision, &[px(7.0, 12.0, 0.0, 9.0, 15.0, 6.0), px(7.0, 6.0, 0.0, 9.0, 9.0, 6.0), px(6.0, 0.0, 0.0, 10.0, 16.0, 6.0)][..2], 1.5);
            outline.push(post);
        }
        Shape::Wall => {
            if g("up") == 1 {
                let post = px(4.0, 0.0, 4.0, 12.0, 16.0, 12.0);
                q.uniform(post, 0);
                collision.push([post[0], 0.0, post[2], post[3], 1.5, post[5]]);
            }
            connect(&mut q, &mut collision, &[px(5.0, 0.0, 0.0, 11.0, 14.0, 8.0)], 1.5);
            outline.push(px(4.0, 0.0, 4.0, 12.0, 16.0, 12.0));
        }
        Shape::Pane => {
            let post = px(7.0, 0.0, 7.0, 9.0, 16.0, 9.0);
            q.uniform(post, 0);
            collision.push(post);
            connect(&mut q, &mut collision, &[px(7.0, 0.0, 0.0, 9.0, 16.0, 7.0)], 1.0);
        }
        Shape::Door => {
            let side = door_side(facing_dir(g("facing")), g("open") == 1, g("hinge") == 1);
            let b = side_slab(side, 3.0, 0.0, 16.0);
            q.uniform(b, 0);
            collision.push(b);
        }
        Shape::Trapdoor => {
            let b = if g("open") == 1 {
                side_slab(OPPOSITE[facing_dir(g("facing"))], 3.0, 0.0, 16.0)
            } else if g("half") == 1 {
                px(0.0, 13.0, 0.0, 16.0, 16.0, 16.0)
            } else {
                px(0.0, 0.0, 0.0, 16.0, 3.0, 16.0)
            };
            q.uniform(b, 0);
            collision.push(b);
        }
        Shape::Ladder => {
            let facing = facing_dir(g("facing"));
            let back = OPPOSITE[facing];
            let b = side_slab(back, 1.0, 0.0, 16.0);
            q.cuboid(b, |d| if d == facing { Some(Face { flags: Q_DOUBLE, ..Face::slot(0) }) } else { None });
            collision.push(side_slab(back, 3.0, 0.0, 16.0));
        }
        Shape::LilyPad => {
            let y = 1.0 / 64.0;
            q.plane([[0.0, y, 1.0], [1.0, y, 1.0], [1.0, y, 0.0], [0.0, y, 0.0]], 0, false);
            collision.push(px(1.0, 0.0, 1.0, 15.0, 1.5, 15.0));
        }
        Shape::Spike => {
            q.cross(0, g("vertical_direction") == 1);
            let w = [3.0, 3.0, 4.0, 5.0, 6.0][g("thickness")];
            collision.push(px(8.0 - w, 0.0, 8.0 - w, 8.0 + w, 16.0, 8.0 + w));
        }
        Shape::Chest => {
            let b = px(1.0, 0.0, 1.0, 15.0, 14.0, 15.0);
            q.simple(b);
            collision.push(b);
        }
        Shape::Cactus => {
            let b = px(1.0, 0.0, 1.0, 15.0, 16.0, 15.0);
            q.cuboid(b, |d| Some(Face { uv: Some([0.0, 0.0, 1.0, 1.0]), ..Face::slot(d as u8) }));
            collision.push(b);
        }
    }
    Built { quads: q.0, collision, outline, waterlogged }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_round_trip() {
        let s = Shape::Stairs;
        assert_eq!(s.state_count(), 80);
        for local in 0..s.state_count() {
            for p in s.props() {
                let v = s.get(local, p.name).unwrap();
                assert_eq!(s.with(local, p.name, v), local);
                let other = s.with(local, p.name, (v + 1) % p.values.len());
                assert_ne!(other, local);
                assert_eq!(s.get(other, p.name), Some((v + 1) % p.values.len()));
            }
        }
        assert_eq!(Shape::Cube.state_count(), 1);
        assert_eq!(Shape::Wall.state_count(), 64);
    }

    #[test]
    fn cube_faces_wind_outwards() {
        let b = build(Shape::Cube, 0);
        assert_eq!(b.quads.len(), 6);
        for quad in &b.quads {
            let p = quad.pos;
            let e1 = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
            let e2 = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
            let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
            let d = crate::util::math::DIRS[quad.dir as usize];
            assert!(n[0] * d.0 as f32 + n[1] * d.1 as f32 + n[2] * d.2 as f32 > 0.9, "face {} winds inwards", quad.dir);
            assert_eq!(quad.cull, quad.dir);
        }
    }

    #[test]
    fn rotation_and_coverage() {
        // A box on the north side ends up on the east side after one clockwise turn.
        let east = rot_y(px(0.0, 0.0, 0.0, 16.0, 16.0, 8.0), 1);
        assert_eq!(east, px(8.0, 0.0, 0.0, 16.0, 16.0, 16.0));
        assert_eq!(rot_y(east, 3), px(0.0, 0.0, 0.0, 16.0, 16.0, 8.0));
        let stairs = build(Shape::Stairs, 0); // facing north, bottom, straight
        assert!(covers_face(&stairs.collision, DOWN));
        assert!(covers_face(&stairs.collision, NORTH));
        assert!(!covers_face(&stairs.collision, SOUTH));
        assert!(!covers_face(&stairs.collision, UP));
        let slab = build(Shape::Slab, 0);
        assert!(covers_face(&slab.collision, DOWN) && !covers_face(&slab.collision, UP));
    }

    #[test]
    fn doors_swing_about_their_hinge() {
        assert_eq!(door_side(EAST, false, false), WEST);
        assert_eq!(door_side(EAST, true, true), NORTH);
        assert_eq!(door_side(EAST, true, false), SOUTH);
    }
}
