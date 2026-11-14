//! GPU meshes for chunk sections and the passes that draw them.

use super::gl;
use crate::ffi::{VcVertex, MIN_Y};
use crate::util::as_bytes;
use crate::util::math::{DVec3, Frustum, Vec3};
use crate::world::chunk::{CoordMap, SECTIONS};
use crate::world::mesh::{meshable, Mesher};
use crate::world::World;

#[derive(Clone, Copy, Default)]
struct SectionMesh {
    vao: u32,
    vbo: u32,
    quads: u32,
}

struct ChunkMesh {
    /// [opaque, translucent] per section.
    sections: Vec<[SectionMesh; 2]>,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct ChunkStats {
    pub chunks: usize,
    pub sections_drawn: u32,
    pub quads_drawn: u32,
    pub meshed_this_frame: u32,
}

pub struct ChunkRenderer {
    meshes: CoordMap<ChunkMesh>,
    mesher: Mesher,
    index_buffer: u32,
    index_quads: u32,
    order: Vec<(i32, i32)>,
    pub stats: ChunkStats,
}

impl ChunkRenderer {
    pub fn new() -> ChunkRenderer {
        ChunkRenderer { meshes: CoordMap::default(), mesher: Mesher::default(), index_buffer: gl::create_buffer(), index_quads: 0, order: Vec::new(), stats: ChunkStats::default() }
    }

    /// Makes sure the shared quad index buffer covers `quads` quads.  A vertex array must be bound.
    fn ensure_indices(&mut self, quads: u32) {
        gl::bind_buffer(gl::ELEMENT_ARRAY_BUFFER, self.index_buffer);
        if quads <= self.index_quads {
            return;
        }
        let n = quads.max(16384).next_power_of_two();
        let mut idx: Vec<u32> = Vec::with_capacity(n as usize * 6);
        for q in 0..n {
            let b = q * 4;
            idx.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        gl::buffer_data(gl::ELEMENT_ARRAY_BUFFER, as_bytes(&idx), gl::STATIC_DRAW);
        self.index_quads = n;
    }

    fn upload(&mut self, mesh: &mut SectionMesh, verts: &[VcVertex], quads: u32) {
        if quads == 0 {
            if mesh.vao != 0 {
                gl::delete_vertex_array(mesh.vao);
                gl::delete_buffer(mesh.vbo);
                *mesh = SectionMesh::default();
            }
            return;
        }
        let fresh = mesh.vao == 0;
        if fresh {
            mesh.vao = gl::create_vertex_array();
            mesh.vbo = gl::create_buffer();
        }
        gl::bind_vertex_array(mesh.vao);
        gl::bind_buffer(gl::ARRAY_BUFFER, mesh.vbo);
        gl::buffer_data(gl::ARRAY_BUFFER, as_bytes(&verts[..quads as usize * 4]), gl::STATIC_DRAW);
        if fresh {
            for i in 0..4 {
                gl::enable_vertex_attrib_array(i);
            }
            gl::vertex_attrib_pointer(0, 4, gl::SHORT, false, 20, 0);
            gl::vertex_attrib_pointer(1, 2, gl::SHORT, false, 20, 8);
            gl::vertex_attrib_pointer(2, 4, gl::UNSIGNED_BYTE, true, 20, 12);
            gl::vertex_attrib_pointer(3, 4, gl::UNSIGNED_BYTE, true, 20, 16);
        }
        self.ensure_indices(quads);
        mesh.quads = quads;
    }

    /// Rebuilds out-of-date section meshes near the centre chunk, closest first.
    /// `keep_going` is polled between sections so the caller can enforce a time budget.
    pub fn update(&mut self, world: &mut World, centre: (i32, i32), radius: i32, mut keep_going: impl FnMut() -> bool) {
        self.stats.meshed_this_frame = 0;
        self.order.clear();
        for (&(cx, cz), c) in world.chunks.iter() {
            if c.mesh_dirty != 0 && (cx - centre.0).abs() <= radius && (cz - centre.1).abs() <= radius {
                self.order.push((cx, cz));
            }
        }
        self.order.sort_by_key(|(cx, cz)| (cx - centre.0).pow(2) + (cz - centre.1).pow(2));
        let order = std::mem::take(&mut self.order);
        'outer: for &(cx, cz) in &order {
            if !meshable(&world.chunks, cx, cz) {
                continue;
            }
            let mut dirty = world.chunks[&(cx, cz)].mesh_dirty;
            let mut entry = self.meshes.remove(&(cx, cz)).unwrap_or_else(|| ChunkMesh { sections: vec![[SectionMesh::default(); 2]; SECTIONS] });
            let mut finished = true;
            for si in 0..SECTIONS {
                if dirty & (1 << si) == 0 {
                    continue;
                }
                if !keep_going() && self.stats.meshed_this_frame > 0 {
                    finished = false;
                    break;
                }
                self.mesher.build(&world.chunks, cx, si, cz);
                let counts = self.mesher.counts;
                let opaque = std::mem::take(&mut self.mesher.opaque);
                let translucent = std::mem::take(&mut self.mesher.translucent);
                self.upload(&mut entry.sections[si][0], &opaque, counts[0]);
                self.upload(&mut entry.sections[si][1], &translucent, counts[1]);
                self.mesher.opaque = opaque;
                self.mesher.translucent = translucent;
                dirty &= !(1 << si);
                self.stats.meshed_this_frame += 1;
            }
            gl::bind_vertex_array(0);
            self.meshes.insert((cx, cz), entry);
            if let Some(c) = world.chunks.get_mut(&(cx, cz)) {
                c.mesh_dirty = dirty;
            }
            if !finished {
                break 'outer;
            }
        }
        self.order = order;
        self.stats.chunks = self.meshes.len();
    }

    /// Frees meshes of chunks that are no longer loaded.
    pub fn retain_loaded(&mut self, world: &World) {
        let gone: Vec<(i32, i32)> = self.meshes.keys().copied().filter(|k| !world.chunks.contains_key(k)).collect();
        for k in gone {
            if let Some(m) = self.meshes.remove(&k) {
                for pair in m.sections {
                    for s in pair {
                        if s.vao != 0 {
                            gl::delete_vertex_array(s.vao);
                            gl::delete_buffer(s.vbo);
                        }
                    }
                }
            }
        }
    }

    pub fn clear(&mut self) {
        let keys: Vec<(i32, i32)> = self.meshes.keys().copied().collect();
        for k in keys {
            if let Some(m) = self.meshes.remove(&k) {
                for pair in m.sections {
                    for s in pair {
                        if s.vao != 0 {
                            gl::delete_vertex_array(s.vao);
                            gl::delete_buffer(s.vbo);
                        }
                    }
                }
            }
        }
    }

    /// Draws one pass (0 = opaque and cut-out, 1 = translucent).  `offset_loc` is the uniform that
    /// receives each section's position relative to the camera.
    pub fn draw(&mut self, pass: usize, camera: DVec3, frustum: &Frustum, radius: i32, offset_loc: i32) {
        if pass == 0 {
            self.stats.sections_drawn = 0;
            self.stats.quads_drawn = 0;
        }
        let cam_chunk = ((camera.x.floor() as i32) >> 4, (camera.z.floor() as i32) >> 4);
        self.order.clear();
        self.order.extend(self.meshes.keys().copied().filter(|(cx, cz)| (cx - cam_chunk.0).abs() <= radius && (cz - cam_chunk.1).abs() <= radius));
        if pass == 1 {
            self.order.sort_by_key(|(cx, cz)| -((cx - cam_chunk.0).pow(2) + (cz - cam_chunk.1).pow(2)));
        } else {
            self.order.sort_by_key(|(cx, cz)| (cx - cam_chunk.0).pow(2) + (cz - cam_chunk.1).pow(2));
        }
        for &(cx, cz) in &self.order {
            let mesh = &self.meshes[&(cx, cz)];
            let ox = (cx as f64 * 16.0 - camera.x) as f32;
            let oz = (cz as f64 * 16.0 - camera.z) as f32;
            for (si, pair) in mesh.sections.iter().enumerate() {
                let s = pair[pass];
                if s.quads == 0 {
                    continue;
                }
                let oy = ((MIN_Y + si as i32 * 16) as f64 - camera.y) as f32;
                if !frustum.intersects_box(Vec3::new(ox, oy, oz), Vec3::new(ox + 16.0, oy + 16.0, oz + 16.0)) {
                    continue;
                }
                gl::uniform3f(offset_loc, ox, oy, oz);
                gl::bind_vertex_array(s.vao);
                gl::draw_elements(gl::TRIANGLES, (s.quads * 6) as i32, gl::UNSIGNED_INT, 0);
                self.stats.sections_drawn += 1;
                self.stats.quads_drawn += s.quads;
            }
        }
        gl::bind_vertex_array(0);
    }
}

impl Default for ChunkRenderer {
    fn default() -> Self {
        ChunkRenderer::new()
    }
}
