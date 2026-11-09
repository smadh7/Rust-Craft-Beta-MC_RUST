//! Offline voxel core. Rendering and the playable client are not implemented yet.
use std::collections::HashMap;
pub const CHUNK_X: usize = 16;
pub const CHUNK_Z: usize = 16;
pub const WORLD_H: usize = 64;
pub const CHUNK_VOLUME: usize = CHUNK_X * CHUNK_Z * WORLD_H;
pub const PADDED_VOLUME: usize = 18 * 18 * 66;
pub const AIR: u8 = 0;
extern "C" {
    fn vc_generate_chunk(cx: i32, cz: i32, seed: u64, out: *mut u8);
    fn vc_terrain_height(x: i32, z: i32, seed: u64) -> i32;
    fn vc_mesh_chunk(padded: *const u8, out: *mut u32, capacity: u32) -> u32;
}
pub fn terrain_height(x: i32, z: i32, seed: u64) -> i32 { unsafe { vc_terrain_height(x,z,seed) } }
pub fn generate_chunk(cx: i32, cz: i32, seed: u64) -> Vec<u8> {
    let mut blocks = vec![0; CHUNK_VOLUME];
    unsafe { vc_generate_chunk(cx, cz, seed, blocks.as_mut_ptr()) }; blocks
}
/// Returns two packed words per quad. The native core describes the layout.
pub fn mesh(padded: &[u8]) -> Vec<u32> {
    assert_eq!(padded.len(), PADDED_VOLUME);
    let needed = unsafe { vc_mesh_chunk(padded.as_ptr(), std::ptr::null_mut(), 0) };
    let mut words = vec![0; needed as usize];
    let actual = unsafe { vc_mesh_chunk(padded.as_ptr(), words.as_mut_ptr(), needed) };
    assert_eq!(actual, needed); words
}
/// Copy a chunk and its four neighbours into the mesher's padded input.
/// Missing neighbours are air; remesh when neighbours subsequently load.
pub fn pad_chunk(center: &[u8], neighbours: [Option<&[u8]>; 4]) -> Vec<u8> {
    assert_eq!(center.len(), CHUNK_VOLUME);
    for n in neighbours.iter().flatten() { assert_eq!(n.len(), CHUNK_VOLUME); }
    let mut p = vec![0; PADDED_VOLUME];
    let idx = |x: usize,y: usize,z: usize| (y*16+z)*16+x;
    let pi = |x: usize,y: usize,z: usize| (y*18+z)*18+x;
    for y in 0..64 { for z in 0..16 { for x in 0..16 { p[pi(x+1,y+1,z+1)] = center[idx(x,y,z)]; }
        if let Some(n)=neighbours[0] { p[pi(0,y+1,z+1)] = n[idx(15,y,z)]; }
        if let Some(n)=neighbours[1] { p[pi(17,y+1,z+1)] = n[idx(0,y,z)]; }
    } for x in 0..16 {
        if let Some(n)=neighbours[2] { p[pi(x+1,y+1,0)] = n[idx(x,y,15)]; }
        if let Some(n)=neighbours[3] { p[pi(x+1,y+1,17)] = n[idx(x,y,0)]; }
    }} p
}
/// Generated chunks and edits, indexed using Euclidean coordinates for negative positions.
pub struct World { pub seed: u64, chunks: HashMap<(i32,i32), Vec<u8>> }
impl World {
    pub fn new(seed:u64)->Self { Self { seed, chunks:HashMap::new() } }
    pub fn load_chunk(&mut self,cx:i32,cz:i32) { self.chunks.entry((cx,cz)).or_insert_with(|| generate_chunk(cx,cz,self.seed)); }
    pub fn get(&self,x:i32,y:i32,z:i32)->Option<u8> {
        if !(0..64).contains(&y) { return Some(AIR); }
        self.chunks.get(&(x.div_euclid(16),z.div_euclid(16))).map(|c| c[((y as usize)*16+z.rem_euclid(16) as usize)*16+x.rem_euclid(16) as usize])
    }
    pub fn set(&mut self,x:i32,y:i32,z:i32,block:u8)->bool {
        if !(0..64).contains(&y) { return false; }
        if let Some(c)=self.chunks.get_mut(&(x.div_euclid(16),z.div_euclid(16))) { c[((y as usize)*16+z.rem_euclid(16) as usize)*16+x.rem_euclid(16) as usize]=block; true } else { false }
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn deterministic() { assert_eq!(generate_chunk(-1,2,42), generate_chunk(-1,2,42)); }
    #[test] fn negative_coords() { let mut w=World::new(1); w.load_chunk(-1,-1); assert!(w.set(-1,63,-1,7)); assert_eq!(w.get(-1,63,-1),Some(7)); assert_eq!(w.get(0,63,0),None); }
    #[test] fn solid_cube_is_six_quads() { let c=vec![1;CHUNK_VOLUME]; assert_eq!(mesh(&pad_chunk(&c,[None;4])).len(),12); }
    #[test] fn neighbour_culls_face() { let c=vec![1;CHUNK_VOLUME]; assert_eq!(mesh(&pad_chunk(&c,[Some(&c),None,None,None])).len(),10); }
}
