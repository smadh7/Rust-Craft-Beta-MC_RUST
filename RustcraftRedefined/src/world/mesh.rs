//! Glue between chunk storage and the native section mesher.

use super::biome::Biome;
use super::block::reg;
use super::chunk::{Chunk, CoordMap, SECTIONS, STAGE_LIT};
use crate::ffi::*;

const PAD: usize = 18;

pub struct Mesher {
    blocks: Vec<u16>,
    light: Vec<u8>,
    tints: [Vec<u32>; 3],
    tint_key: Option<(i32, i32)>,
    pub opaque: Vec<VcVertex>,
    pub translucent: Vec<VcVertex>,
    /// Quad counts of the last build.
    pub counts: [u32; 2],
}

impl Default for Mesher {
    fn default() -> Self {
        Mesher {
            blocks: vec![0; PAD * PAD * PAD],
            light: vec![0; PAD * PAD * PAD],
            tints: [vec![0; PAD * PAD], vec![0; PAD * PAD], vec![0; PAD * PAD]],
            tint_key: None,
            opaque: vec![VcVertex::default(); 4 * 4096],
            translucent: vec![VcVertex::default(); 4 * 1024],
            counts: [0, 0],
        }
    }
}

/// True when chunk (cx, cz) and its eight neighbours are lit, so its sections can be meshed.
pub fn meshable(chunks: &CoordMap<Box<Chunk>>, cx: i32, cz: i32) -> bool {
    (-1..=1).all(|dz| (-1..=1).all(|dx| chunks.get(&(cx + dx, cz + dz)).map_or(false, |c| c.stage >= STAGE_LIT)))
}

impl Mesher {
    /// Biome colours blended over a 5x5 neighbourhood for the 18x18 columns around a chunk.
    fn build_tints(&mut self, chunks: &CoordMap<Box<Chunk>>, cx: i32, cz: i32) {
        const N: usize = PAD + 4;
        let mut src = [[0u32; N * N]; 3];
        for gz in 0..N {
            for gx in 0..N {
                let (x, z) = (cx * 16 + gx as i32 - 3, cz * 16 + gz as i32 - 3);
                let biome = chunks.get(&(x >> 4, z >> 4)).map_or(Biome::Plains, |c| Biome::from_u8(c.biome[((z & 15) * 16 + (x & 15)) as usize]));
                let info = biome.info();
                src[0][gz * N + gx] = info.grass;
                src[1][gz * N + gx] = info.foliage;
                src[2][gz * N + gx] = info.water;
            }
        }
        for k in 0..3 {
            for pz in 0..PAD {
                for px in 0..PAD {
                    let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                    for dz in 0..5 {
                        for dx in 0..5 {
                            let c = src[k][(pz + dz) * N + px + dx];
                            r += (c >> 16) & 255;
                            g += (c >> 8) & 255;
                            b += c & 255;
                        }
                    }
                    self.tints[k][pz * PAD + px] = ((r / 25) << 16) | ((g / 25) << 8) | (b / 25);
                }
            }
        }
        self.tint_key = Some((cx, cz));
    }

    /// Meshes one section.  Returns false if the chunk neighbourhood is not ready.
    /// On success the vertices are in `opaque[..counts[0] * 4]` and `translucent[..counts[1] * 4]`.
    pub fn build(&mut self, chunks: &CoordMap<Box<Chunk>>, cx: i32, si: usize, cz: i32) -> bool {
        let mut near: [Option<&Chunk>; 9] = [None; 9];
        for dz in 0..3 {
            for dx in 0..3 {
                match chunks.get(&(cx + dx as i32 - 1, cz + dz as i32 - 1)) {
                    Some(c) if c.stage >= STAGE_LIT => near[dz * 3 + dx] = Some(c),
                    _ => return false,
                }
            }
        }
        self.counts = [0, 0];
        let centre = near[4].unwrap();
        if si >= SECTIONS || centre.section_is_empty(si) {
            return true;
        }
        if self.tint_key != Some((cx, cz)) {
            self.build_tints(chunks, cx, cz);
        }
        let y0 = MIN_Y + si as i32 * 16;
        for py in 0..PAD {
            let y = y0 + py as i32 - 1;
            for pz in 0..PAD {
                let lz = pz as i32 - 1;
                let (cz_i, z) = if lz < 0 { (0, 15) } else if lz > 15 { (2, 0) } else { (1, lz as usize) };
                for px in 0..PAD {
                    let lx = px as i32 - 1;
                    let (cx_i, x) = if lx < 0 { (0, 15) } else if lx > 15 { (2, 0) } else { (1, lx as usize) };
                    let c = near[cz_i * 3 + cx_i].unwrap();
                    let i = (py * PAD + pz) * PAD + px;
                    self.blocks[i] = c.get(x, y, z);
                    self.light[i] = c.light(x, y, z);
                }
            }
        }
        let tables = reg().mesh_tables();
        let input = VcMeshInput {
            blocks: self.blocks.as_ptr(),
            light: self.light.as_ptr(),
            tint_grass: self.tints[0].as_ptr(),
            tint_foliage: self.tints[1].as_ptr(),
            tint_water: self.tints[2].as_ptr(),
            origin_x: cx * 16,
            origin_y: y0,
            origin_z: cz * 16,
        };
        loop {
            // SAFETY: all pointers refer to live buffers of the sizes the mesher expects.
            let ok = unsafe {
                vc_mesh_section(
                    &tables,
                    &input,
                    self.opaque.as_mut_ptr(),
                    (self.opaque.len() / 4) as u32,
                    self.translucent.as_mut_ptr(),
                    (self.translucent.len() / 4) as u32,
                    self.counts.as_mut_ptr(),
                )
            };
            if ok != 0 {
                return true;
            }
            self.opaque.resize((self.counts[0] as usize + 256) * 4, VcVertex::default());
            self.translucent.resize((self.counts[1] as usize + 256) * 4, VcVertex::default());
        }
    }
}
