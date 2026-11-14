// Rustcraft Redefined - native voxel core (C ABI).
//
// The core is deliberately freestanding: it uses no libc, no C++ standard
// library, no exceptions, no RTTI and never allocates.  Every buffer is owned
// by the Rust caller.  That keeps one source tree usable for Windows, Linux
// and WebAssembly builds.
#ifndef VC_CORE_H
#define VC_CORE_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// ---------------------------------------------------------------- world ----
#define VC_MIN_Y (-64)
#define VC_HEIGHT 384
#define VC_SEA_LEVEL 63
#define VC_LAVA_LEVEL (-55)

// Material codes written by the terrain kernel.
enum {
    VC_MAT_AIR = 0,
    VC_MAT_STONE = 1,
    VC_MAT_DEEPSLATE = 2,
    VC_MAT_WATER = 3,
    VC_MAT_LAVA = 4,
    VC_MAT_BEDROCK = 5,
};

// ---------------------------------------------------------------- noise ----
typedef struct VcPerlin {
    double ox, oy, oz;
    uint8_t perm[512];
} VcPerlin;

#define VC_MAX_OCTAVES 8
typedef struct VcFbm {
    int32_t octaves;
    int32_t pad;
    double freq[VC_MAX_OCTAVES];
    double amp[VC_MAX_OCTAVES];
    double scale; // output multiplier (normalisation * calibration)
    VcPerlin p[VC_MAX_OCTAVES];
} VcFbm;

void vc_perlin_init(VcPerlin* p, uint64_t seed);
double vc_perlin_sample(const VcPerlin* p, double x, double y, double z);
void vc_fbm_init(VcFbm* f, uint64_t seed, int32_t octaves, double base_freq, double lacunarity, double gain, double scale);
double vc_fbm_sample(const VcFbm* f, double x, double y, double z);

// -------------------------------------------------------------- terrain ----
typedef struct VcTerrainGen VcTerrainGen;

// Climate and shape of one block column.
typedef struct VcColumn {
    float temperature;
    float humidity;
    float continentalness;
    float erosion;
    float weirdness;
    float pv;          // peaks and valleys, derived from weirdness
    float base_height; // terrain height before 3D noise, in world Y
    int32_t surface;   // Y of the highest solid block (VC_MIN_Y - 1 if none)
} VcColumn;

size_t vc_terrain_gen_size(void);
void vc_terrain_gen_init(VcTerrainGen* gen, uint64_t seed);
// Point query; `surface` is an estimate (base height) because no blocks are generated.
void vc_terrain_column(const VcTerrainGen* gen, int32_t x, int32_t z, VcColumn* out);
// Fills 16*16*384 material codes, indexed ((y - VC_MIN_Y) * 16 + z) * 16 + x,
// and 256 columns indexed z * 16 + x.
void vc_terrain_chunk(const VcTerrainGen* gen, int32_t cx, int32_t cz, uint8_t* materials, VcColumn* columns);

// --------------------------------------------------------------- meshing ----
// Face / direction indices used everywhere: 0 -Y, 1 +Y, 2 -Z, 3 +Z, 4 -X, 5 +X.
#define VC_DIR_NONE 6

// Quad flags.
#define VC_Q_TINT 1u         // multiply by the block's tint colour
#define VC_Q_INSET 2u        // interior face: light and AO sampled around the block's own cell
#define VC_Q_DOUBLE 4u       // also emit the reversed face
#define VC_Q_NOSHADE 8u      // skip directional shading (emissive parts)

typedef struct VcQuad {
    float pos[4][3]; // block-local, 0..1
    float uv[4][2];  // 0..1 across the texture
    uint8_t slot;    // index into the state's texture set
    uint8_t cull;    // direction to test for occlusion, or VC_DIR_NONE
    uint8_t dir;     // facing direction used for shading and light sampling
    uint8_t flags;
} VcQuad;

typedef struct VcGeom {
    uint32_t first_quad;
    uint32_t quad_count;
} VcGeom;

typedef struct VcTexSet {
    uint16_t layer[8];
} VcTexSet;

// State flags.
#define VC_S_TRANSLUCENT 1u   // drawn in the blended pass
#define VC_S_AO 2u            // full opaque cube: occludes ambient light
#define VC_S_CULL_SAME 4u     // faces between identical states are hidden (glass, ice)
#define VC_S_OFFSET 8u        // deterministic XZ jitter (plants)
#define VC_S_LAVA 16u         // fluid is lava (otherwise water)
#define VC_S_WATERLOGGED 32u  // geometry plus a water source
#define VC_S_FULLBRIGHT 64u   // ignore world light

typedef struct VcStateInfo {
    uint16_t geom;   // 0 = nothing to draw
    uint16_t texset;
    uint8_t flags;
    uint8_t occlude; // bit d set: face d is a full opaque square
    uint8_t tint;    // 0 none, 1 grass, 2 foliage, 3 water, 4+ = fixed_tints[tint - 4]
    uint8_t fluid;   // 0 none, else 1 + level (1 = source, 2..8 flowing, 9+ falling)
} VcStateInfo;

typedef struct VcMeshTables {
    const VcStateInfo* states;
    const VcGeom* geoms;
    const VcQuad* quads;
    const VcTexSet* texsets;
    const uint32_t* fixed_tints; // 0xRRGGBB
    uint32_t num_states;
    uint16_t water_still, water_flow, lava_still, lava_flow;
} VcMeshTables;

// One 16^3 section padded by one block on every side (18^3 cells).
// Cell index: (y * 18 + z) * 18 + x with the section starting at (1, 1, 1).
typedef struct VcMeshInput {
    const uint16_t* blocks;
    const uint8_t* light;          // sky in the high nibble, block in the low nibble
    const uint32_t* tint_grass;    // 18 * 18 columns, 0xRRGGBB, index z * 18 + x
    const uint32_t* tint_foliage;
    const uint32_t* tint_water;
    int32_t origin_x, origin_y, origin_z; // world position of the section's first block
} VcMeshInput;

typedef struct VcVertex {
    int16_t x, y, z;   // section-local position * 256
    uint16_t layer;    // texture array layer
    int16_t u, v;      // texture coordinate * 4096
    uint8_t r, g, b, a;
    uint8_t sky, block; // light, 0..255
    uint8_t dir;
    uint8_t pad;
} VcVertex;

// Writes at most cap_* quads (4 vertices each) per stream and stores the number of
// quads that exist in counts[0] (opaque + cutout) and counts[1] (translucent).
// Returns 1 when everything fitted, 0 when the caller must retry with larger buffers.
int32_t vc_mesh_section(const VcMeshTables* tables, const VcMeshInput* in,
                        VcVertex* out_opaque, uint32_t cap_opaque,
                        VcVertex* out_translucent, uint32_t cap_translucent,
                        uint32_t* counts);

uint32_t vc_core_version(void);

#ifdef __cplusplus
}
#endif
#endif
