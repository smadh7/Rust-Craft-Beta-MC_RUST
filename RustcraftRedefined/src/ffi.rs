//! Bindings to the C++ native core (`cpp/vc_core.h`): noise, terrain kernel and section mesher.
#![allow(non_camel_case_types)]

pub const MIN_Y: i32 = -64;
pub const HEIGHT: i32 = 384;
pub const MAX_Y: i32 = MIN_Y + HEIGHT; // exclusive
pub const SEA_LEVEL: i32 = 63;
pub const LAVA_LEVEL: i32 = -55;

pub const MAT_AIR: u8 = 0;
pub const MAT_STONE: u8 = 1;
pub const MAT_DEEPSLATE: u8 = 2;
pub const MAT_WATER: u8 = 3;
pub const MAT_LAVA: u8 = 4;
pub const MAT_BEDROCK: u8 = 5;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VcPerlin {
    pub ox: f64,
    pub oy: f64,
    pub oz: f64,
    pub perm: [u8; 512],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct VcFbm {
    pub octaves: i32,
    pub pad: i32,
    pub freq: [f64; 8],
    pub amp: [f64; 8],
    pub scale: f64,
    pub p: [VcPerlin; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VcColumn {
    pub temperature: f32,
    pub humidity: f32,
    pub continentalness: f32,
    pub erosion: f32,
    pub weirdness: f32,
    pub pv: f32,
    pub base_height: f32,
    pub surface: i32,
}

pub const DIR_NONE: u8 = 6;
pub const Q_TINT: u8 = 1;
pub const Q_INSET: u8 = 2;
pub const Q_DOUBLE: u8 = 4;
pub const Q_NOSHADE: u8 = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VcQuad {
    pub pos: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
    pub slot: u8,
    pub cull: u8,
    pub dir: u8,
    pub flags: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VcGeom {
    pub first_quad: u32,
    pub quad_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct VcTexSet {
    pub layer: [u16; 8],
}

pub const S_TRANSLUCENT: u8 = 1;
pub const S_AO: u8 = 2;
pub const S_CULL_SAME: u8 = 4;
pub const S_OFFSET: u8 = 8;
pub const S_LAVA: u8 = 16;
pub const S_WATERLOGGED: u8 = 32;
pub const S_FULLBRIGHT: u8 = 64;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VcStateInfo {
    pub geom: u16,
    pub texset: u16,
    pub flags: u8,
    pub occlude: u8,
    pub tint: u8,
    pub fluid: u8,
}

#[repr(C)]
pub struct VcMeshTables {
    pub states: *const VcStateInfo,
    pub geoms: *const VcGeom,
    pub quads: *const VcQuad,
    pub texsets: *const VcTexSet,
    pub fixed_tints: *const u32,
    pub num_states: u32,
    pub water_still: u16,
    pub water_flow: u16,
    pub lava_still: u16,
    pub lava_flow: u16,
}

#[repr(C)]
pub struct VcMeshInput {
    pub blocks: *const u16,
    pub light: *const u8,
    pub tint_grass: *const u32,
    pub tint_foliage: *const u32,
    pub tint_water: *const u32,
    pub origin_x: i32,
    pub origin_y: i32,
    pub origin_z: i32,
}

/// One mesh vertex, 20 bytes.  See `gfx::chunks` for the matching vertex layout.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VcVertex {
    pub x: i16,
    pub y: i16,
    pub z: i16,
    pub layer: u16,
    pub u: i16,
    pub v: i16,
    pub rgba: [u8; 4],
    pub sky: u8,
    pub block: u8,
    pub dir: u8,
    pub pad: u8,
}

#[repr(C)]
pub struct VcTerrainGenOpaque {
    _private: [u8; 0],
}

extern "C" {
    pub fn vc_perlin_init(p: *mut VcPerlin, seed: u64);
    pub fn vc_perlin_sample(p: *const VcPerlin, x: f64, y: f64, z: f64) -> f64;
    pub fn vc_fbm_init(f: *mut VcFbm, seed: u64, octaves: i32, base_freq: f64, lacunarity: f64, gain: f64, scale: f64);
    pub fn vc_fbm_sample(f: *const VcFbm, x: f64, y: f64, z: f64) -> f64;
    pub fn vc_terrain_gen_size() -> usize;
    pub fn vc_terrain_gen_init(gen: *mut VcTerrainGenOpaque, seed: u64);
    pub fn vc_terrain_column(gen: *const VcTerrainGenOpaque, x: i32, z: i32, out: *mut VcColumn);
    pub fn vc_terrain_chunk(gen: *const VcTerrainGenOpaque, cx: i32, cz: i32, materials: *mut u8, columns: *mut VcColumn);
    pub fn vc_mesh_section(
        tables: *const VcMeshTables,
        input: *const VcMeshInput,
        out_opaque: *mut VcVertex,
        cap_opaque: u32,
        out_translucent: *mut VcVertex,
        cap_translucent: u32,
        counts: *mut u32,
    ) -> i32;
    pub fn vc_core_version() -> u32;
}

/// Fractal Perlin noise evaluated by the native core.
pub struct Fbm(Box<VcFbm>);

impl Fbm {
    pub fn new(seed: u64, octaves: i32, base_freq: f64, scale: f64) -> Fbm {
        // SAFETY: VcFbm is plain data; vc_fbm_init fully initialises it.
        unsafe {
            let mut b: Box<VcFbm> = Box::new(std::mem::zeroed());
            vc_fbm_init(&mut *b, seed, octaves, base_freq, 2.0, 0.5, scale);
            Fbm(b)
        }
    }
    #[inline]
    pub fn sample(&self, x: f64, y: f64, z: f64) -> f64 {
        // SAFETY: the pointer refers to an initialised VcFbm owned by self.
        unsafe { vc_fbm_sample(&*self.0, x, y, z) }
    }
    #[inline]
    pub fn sample2(&self, x: f64, z: f64) -> f64 {
        self.sample(x, 0.0, z)
    }
}

/// Owner of the opaque terrain generator state.
pub struct TerrainKernel {
    buf: Vec<u64>,
}

impl TerrainKernel {
    pub fn new(seed: u64) -> TerrainKernel {
        // SAFETY: the buffer is at least vc_terrain_gen_size() bytes with 8-byte alignment.
        unsafe {
            let words = (vc_terrain_gen_size() + 7) / 8;
            let mut buf = vec![0u64; words];
            vc_terrain_gen_init(buf.as_mut_ptr() as *mut VcTerrainGenOpaque, seed);
            TerrainKernel { buf }
        }
    }
    fn ptr(&self) -> *const VcTerrainGenOpaque {
        self.buf.as_ptr() as *const VcTerrainGenOpaque
    }
    pub fn column(&self, x: i32, z: i32) -> VcColumn {
        let mut c = VcColumn::default();
        // SAFETY: valid generator and output pointer.
        unsafe { vc_terrain_column(self.ptr(), x, z, &mut c) };
        c
    }
    /// Fills `materials` (16 * 16 * 384) and `columns` (256).
    pub fn chunk(&self, cx: i32, cz: i32, materials: &mut [u8], columns: &mut [VcColumn]) {
        assert!(materials.len() >= 16 * 16 * HEIGHT as usize && columns.len() >= 256);
        // SAFETY: buffer sizes checked above.
        unsafe { vc_terrain_chunk(self.ptr(), cx, cz, materials.as_mut_ptr(), columns.as_mut_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_layouts_match_the_header() {
        assert_eq!(std::mem::size_of::<VcVertex>(), 20);
        assert_eq!(std::mem::size_of::<VcQuad>(), 84);
        assert_eq!(std::mem::size_of::<VcStateInfo>(), 8);
        assert_eq!(std::mem::size_of::<VcColumn>(), 32);
        assert_eq!(std::mem::size_of::<VcPerlin>(), 536);
        assert_eq!(std::mem::size_of::<VcFbm>(), 8 + 64 + 64 + 8 + 536 * 8);
        assert_eq!(unsafe { vc_core_version() }, 2);
    }

    #[test]
    fn noise_is_deterministic_and_bounded() {
        let a = Fbm::new(99, 4, 1.0 / 64.0, 1.0);
        let b = Fbm::new(99, 4, 1.0 / 64.0, 1.0);
        let c = Fbm::new(100, 4, 1.0 / 64.0, 1.0);
        let mut differs = false;
        for i in 0..200 {
            let (x, z) = (i as f64 * 3.7, i as f64 * -1.3);
            let v = a.sample(x, 5.0, z);
            assert_eq!(v, b.sample(x, 5.0, z));
            assert!(v.abs() <= 1.0);
            differs |= (v - c.sample(x, 5.0, z)).abs() > 1e-6;
        }
        assert!(differs);
    }
}
