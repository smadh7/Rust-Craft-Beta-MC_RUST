// Noise primitives and the overworld terrain kernel.
//
// The terrain model is an original design in the spirit of modern multi-noise
// generators: five low frequency climate fields pick a base height, a 3D
// displacement amplitude and peak jaggedness; a 3D density field then decides
// which blocks are solid, and separate cave fields carve the result.
#include "vc_core.h"
#include "vc_noise.h"

// ------------------------------------------------------------------ noise --
static inline double fade(double t) { return t * t * t * (t * (t * 6.0 - 15.0) + 10.0); }

static inline double grad(int h, double x, double y, double z) {
    h &= 15;
    double u = h < 8 ? x : y;
    double v = h < 4 ? y : ((h == 12 || h == 14) ? x : z);
    return ((h & 1) ? -u : u) + ((h & 2) ? -v : v);
}

extern "C" void vc_perlin_init(VcPerlin* p, uint64_t seed) {
    uint64_t s = seed;
    p->ox = (double)(vc_splitmix(&s) >> 11) * (1.0 / 9007199254740992.0) * 256.0;
    p->oy = (double)(vc_splitmix(&s) >> 11) * (1.0 / 9007199254740992.0) * 256.0;
    p->oz = (double)(vc_splitmix(&s) >> 11) * (1.0 / 9007199254740992.0) * 256.0;
    for (int i = 0; i < 256; i++) p->perm[i] = (uint8_t)i;
    for (int i = 255; i > 0; i--) {
        int j = (int)(vc_splitmix(&s) % (uint64_t)(i + 1));
        uint8_t t = p->perm[i];
        p->perm[i] = p->perm[j];
        p->perm[j] = t;
    }
    for (int i = 0; i < 256; i++) p->perm[256 + i] = p->perm[i];
}

extern "C" double vc_perlin_sample(const VcPerlin* p, double x, double y, double z) {
    x += p->ox;
    y += p->oy;
    z += p->oz;
    double fx = vc_floor(x), fy = vc_floor(y), fz = vc_floor(z);
    int X = (int)((int64_t)fx & 255), Y = (int)((int64_t)fy & 255), Z = (int)((int64_t)fz & 255);
    x -= fx;
    y -= fy;
    z -= fz;
    double u = fade(x), v = fade(y), w = fade(z);
    const uint8_t* P = p->perm;
    int A = P[X] + Y, AA = P[A] + Z, AB = P[A + 1] + Z;
    int B = P[X + 1] + Y, BA = P[B] + Z, BB = P[B + 1] + Z;
    return vc_lerp(w,
                   vc_lerp(v, vc_lerp(u, grad(P[AA], x, y, z), grad(P[BA], x - 1, y, z)),
                           vc_lerp(u, grad(P[AB], x, y - 1, z), grad(P[BB], x - 1, y - 1, z))),
                   vc_lerp(v, vc_lerp(u, grad(P[AA + 1], x, y, z - 1), grad(P[BA + 1], x - 1, y, z - 1)),
                           vc_lerp(u, grad(P[AB + 1], x, y - 1, z - 1), grad(P[BB + 1], x - 1, y - 1, z - 1))));
}

extern "C" void vc_fbm_init(VcFbm* f, uint64_t seed, int32_t octaves, double base_freq, double lacunarity, double gain, double scale) {
    if (octaves > VC_MAX_OCTAVES) octaves = VC_MAX_OCTAVES;
    if (octaves < 1) octaves = 1;
    f->octaves = octaves;
    f->pad = 0;
    uint64_t s = seed;
    double freq = base_freq, amp = 1.0, sum = 0.0;
    for (int i = 0; i < VC_MAX_OCTAVES; i++) {
        f->freq[i] = freq;
        f->amp[i] = amp;
        if (i < octaves) {
            sum += amp;
            vc_perlin_init(&f->p[i], vc_splitmix(&s));
        }
        freq *= lacunarity;
        amp *= gain;
    }
    f->scale = scale / sum;
}

extern "C" double vc_fbm_sample(const VcFbm* f, double x, double y, double z) {
    double total = 0.0;
    for (int i = 0; i < f->octaves; i++) {
        double q = f->freq[i];
        total += f->amp[i] * vc_perlin_sample(&f->p[i], x * q, y * q, z * q);
    }
    return total * f->scale;
}

// ---------------------------------------------------------------- terrain --
struct VcTerrainGen {
    uint64_t seed;
    VcFbm temperature, humidity, continent, erosion, weird;
    VcFbm base3d, jagged;
    VcFbm cheese, spag1, spag2, spag_mod, noodle1, noodle2, noodle_gate, entrance;
};

extern "C" size_t vc_terrain_gen_size(void) { return sizeof(VcTerrainGen); }

extern "C" void vc_terrain_gen_init(VcTerrainGen* g, uint64_t seed) {
    g->seed = seed;
    uint64_t s = seed ^ 0x52554354435246ULL;
    // Climate.  Scales are calibrated so each field has a standard deviation of roughly 0.4.
    vc_fbm_init(&g->temperature, vc_splitmix(&s), 4, 1.0 / 2000.0, 2.0, 0.5, 3.1);
    vc_fbm_init(&g->humidity, vc_splitmix(&s), 4, 1.0 / 900.0, 2.0, 0.5, 3.1);
    vc_fbm_init(&g->continent, vc_splitmix(&s), 6, 1.0 / 1700.0, 2.0, 0.5, 3.2);
    vc_fbm_init(&g->erosion, vc_splitmix(&s), 5, 1.0 / 1500.0, 2.0, 0.5, 3.2);
    vc_fbm_init(&g->weird, vc_splitmix(&s), 4, 1.0 / 450.0, 2.0, 0.5, 3.1);
    // Shape.
    vc_fbm_init(&g->base3d, vc_splitmix(&s), 4, 1.0 / 96.0, 2.0, 0.5, 2.6);
    vc_fbm_init(&g->jagged, vc_splitmix(&s), 3, 1.0 / 42.0, 2.0, 0.5, 2.4);
    // Caves.
    vc_fbm_init(&g->cheese, vc_splitmix(&s), 2, 1.0 / 64.0, 2.0, 0.45, 2.4);
    vc_fbm_init(&g->spag1, vc_splitmix(&s), 2, 1.0 / 84.0, 2.0, 0.4, 2.4);
    vc_fbm_init(&g->spag2, vc_splitmix(&s), 2, 1.0 / 84.0, 2.0, 0.4, 2.4);
    vc_fbm_init(&g->spag_mod, vc_splitmix(&s), 1, 1.0 / 120.0, 2.0, 0.5, 2.0);
    vc_fbm_init(&g->noodle1, vc_splitmix(&s), 1, 1.0 / 38.0, 2.0, 0.5, 2.0);
    vc_fbm_init(&g->noodle2, vc_splitmix(&s), 1, 1.0 / 38.0, 2.0, 0.5, 2.0);
    vc_fbm_init(&g->noodle_gate, vc_splitmix(&s), 1, 1.0 / 150.0, 2.0, 0.5, 2.0);
    vc_fbm_init(&g->entrance, vc_splitmix(&s), 1, 1.0 / 70.0, 2.0, 0.5, 2.0);
}

namespace {

struct Climate {
    double t, h, c, e, w;
};

struct Shape {
    double pv;     // peaks and valleys
    double height; // base terrain height (world Y)
    double disp;   // amplitude of the 3D displacement, in blocks
    double jag;    // amplitude of the jagged peak noise, in blocks
};

inline void climate_at(const VcTerrainGen* g, double x, double z, Climate* out) {
    out->t = vc_fbm_sample(&g->temperature, x, 0.0, z);
    out->h = vc_fbm_sample(&g->humidity, x, 0.0, z);
    out->c = vc_fbm_sample(&g->continent, x, 0.0, z);
    out->e = vc_fbm_sample(&g->erosion, x, 0.0, z);
    out->w = vc_fbm_sample(&g->weird, x, 0.0, z);
}

void shape_of(const Climate& cl, Shape* s) {
    const double C = cl.c, E = cl.e;
    const double pv = 1.0 - vc_fabs(3.0 * vc_fabs(cl.w) - 2.0);

    // Erosion: 1 = rugged, 0 = flat.
    static const double ex[] = {-1.0, -0.78, -0.375, -0.2225, 0.05, 0.45, 0.55, 1.0};
    static const double ey[] = {1.0, 0.82, 0.48, 0.30, 0.12, 0.06, 0.035, 0.0};
    const double ef = vc_curve(ex, ey, 8, E);

    // Ridge profile across peaks and valleys.
    static const double rx[] = {-1.0, -0.85, -0.6, -0.2, 0.2, 0.7, 1.0};
    static const double ry[] = {0.0, 0.0, 0.05, 0.16, 0.36, 0.72, 1.0};
    const double ridge = vc_curve(rx, ry, 7, pv);

    // Ocean floor relative to sea level, with a mushroom island plateau far out.
    static const double ox[] = {-1.20, -1.05, -0.98, -0.51, -0.44, -0.20};
    static const double oy[] = {6.0, 6.0, -36.0, -36.0, -20.0, -12.0};
    const double ocean = vc_curve(ox, oy, 6, C);

    // Land relative to sea level.
    static const double lx[] = {-0.19, 0.0, 0.3, 1.0};
    static const double ly[] = {1.5, 4.0, 9.0, 16.0};
    static const double ax[] = {-0.19, 0.03, 0.3, 0.6, 1.0};
    static const double ay[] = {5.0, 26.0, 64.0, 88.0, 105.0};
    double land = vc_curve(lx, ly, 4, C) + vc_curve(ax, ay, 5, C) * ef * ridge;
    land += 26.0 * vc_smoothstep(0.1, 0.5, C) * vc_smoothstep(-0.3, -0.7, E); // plateaus
    land = vc_lerp(vc_smoothstep(0.55, 0.72, E), land, 0.4);                  // swamps
    const double river = vc_smoothstep(-0.78, -0.93, pv);
    land = vc_lerp(river, land, -6.0);

    const double inland = vc_smoothstep(-0.20, -0.105, C);
    const double rel = vc_lerp(inland, ocean, land);

    const double shattered = vc_smoothstep(0.40, 0.47, E) * vc_smoothstep(0.60, 0.53, E);
    double disp = 2.5 + 24.0 * ef * vc_smoothstep(-0.14, 0.3, C) + 34.0 * shattered * vc_smoothstep(-0.05, 0.25, C);
    disp = vc_lerp(river, disp, 1.5);
    disp = vc_lerp(inland, 3.5, disp);

    s->pv = pv;
    s->height = (double)VC_SEA_LEVEL + rel;
    s->disp = disp;
    s->jag = 38.0 * vc_smoothstep(0.45, 0.9, pv) * ef * vc_smoothstep(0.0, 0.4, C) * (1.0 - river);
}

inline void column_from(const Climate& cl, const Shape& sh, VcColumn* out) {
    out->temperature = (float)cl.t;
    out->humidity = (float)cl.h;
    out->continentalness = (float)cl.c;
    out->erosion = (float)cl.e;
    out->weirdness = (float)cl.w;
    out->pv = (float)sh.pv;
    out->base_height = (float)sh.height;
    out->surface = (int32_t)vc_floor(sh.height);
}

// Cave density: negative inside a cave.  `depth` is the approximate distance below the surface.
double cave_density(const VcTerrainGen* g, double x, double y, double z, double depth) {
    const double ch = vc_fbm_sample(&g->cheese, x, y * 1.4, z);
    const double cheese_thr = 0.60 + 0.40 * vc_smoothstep(48.0, 8.0, depth);
    double d = (cheese_thr - ch) * 28.0;

    const double s1 = vc_fabs(vc_fbm_sample(&g->spag1, x, y * 1.3, z));
    const double s2 = vc_fabs(vc_fbm_sample(&g->spag2, x, y * 1.3, z));
    const double thick = 0.058 + 0.034 * vc_fbm_sample(&g->spag_mod, x, y, z);
    d = vc_min(d, (vc_max(s1, s2) - thick) * 60.0);

    if (vc_fbm_sample(&g->noodle_gate, x, y, z) > 0.12) {
        const double n1 = vc_fabs(vc_fbm_sample(&g->noodle1, x, y * 1.2, z));
        const double n2 = vc_fabs(vc_fbm_sample(&g->noodle2, x, y * 1.2, z));
        d = vc_min(d, (vc_max(n1, n2) - 0.045) * 40.0);
    }
    return d;
}

} // namespace

extern "C" void vc_terrain_column(const VcTerrainGen* g, int32_t x, int32_t z, VcColumn* out) {
    Climate cl;
    Shape sh;
    climate_at(g, (double)x, (double)z, &cl);
    shape_of(cl, &sh);
    column_from(cl, sh, out);
}

extern "C" void vc_terrain_chunk(const VcTerrainGen* g, int32_t cx, int32_t cz, uint8_t* mat, VcColumn* cols) {
    enum { NX = 5, NY = VC_HEIGHT / 8 + 1 };
    const int bx = cx * 16, bz = cz * 16;
    const uint32_t hseed = (uint32_t)(g->seed ^ (g->seed >> 32));

    Climate cl[NX][NX];
    Shape sh[NX][NX];
    static const double fx3 = 1.0, fy3 = 1.5;
    double dt[NX][NX][NY]; // terrain density: > 0 is ground
    double dc[NX][NX][NY]; // cave density: <= 0 is carved

    for (int j = 0; j < NX; j++) {
        for (int i = 0; i < NX; i++) {
            const double x = (double)(bx + i * 4), z = (double)(bz + j * 4);
            climate_at(g, x, z, &cl[i][j]);
            shape_of(cl[i][j], &sh[i][j]);
            const Shape& s = sh[i][j];
            double top = s.height;
            if (s.jag > 0.01) top += s.jag * vc_fabs(vc_fbm_sample(&g->jagged, x, 0.0, z));
            const bool entrance = vc_fbm_sample(&g->entrance, x, 0.0, z) > 0.1;
            for (int k = 0; k < NY; k++) {
                const double y = (double)(VC_MIN_Y + k * 8);
                const double n3 = vc_fbm_sample(&g->base3d, x * fx3, y * fy3, z * fx3);
                const double t = (top - y) + n3 * s.disp;
                dt[i][j][k] = t;
                double c = 8.0;
                if (y <= top + 24.0) {
                    c = cave_density(g, x, y, z, top - y);
                    // Keep a roof over caves: thick below sea level so water never meets cave air.
                    const double need = (y <= (double)VC_SEA_LEVEL + 1.0) ? 9.0 : (entrance ? 0.0 : 4.0);
                    if (t < need) c += (need - t) * 0.8;
                }
                dc[i][j][k] = c;
            }
        }
    }

    // Columns: bilinear climate, exact shape.
    for (int z = 0; z < 16; z++) {
        const int cj = z >> 2;
        const double tz = (double)(z & 3) * 0.25;
        for (int x = 0; x < 16; x++) {
            const int ci = x >> 2;
            const double tx = (double)(x & 3) * 0.25;
            Climate c;
#define VC_BI(f) vc_lerp(tz, vc_lerp(tx, cl[ci][cj].f, cl[ci + 1][cj].f), vc_lerp(tx, cl[ci][cj + 1].f, cl[ci + 1][cj + 1].f))
            c.t = VC_BI(t);
            c.h = VC_BI(h);
            c.c = VC_BI(c);
            c.e = VC_BI(e);
            c.w = VC_BI(w);
#undef VC_BI
            Shape s;
            shape_of(c, &s);
            column_from(c, s, &cols[z * 16 + x]);
            cols[z * 16 + x].surface = VC_MIN_Y - 1;
        }
    }

    // Blocks: trilinear interpolation of both density fields.
    for (int ck = 0; ck < NY - 1; ck++) {
        for (int ly = 0; ly < 8; ly++) {
            const int yi = ck * 8 + ly;
            const int y = VC_MIN_Y + yi;
            const double ty = (double)ly * 0.125;
            double ct[NX][NX], cc[NX][NX];
            for (int j = 0; j < NX; j++) {
                for (int i = 0; i < NX; i++) {
                    ct[i][j] = vc_lerp(ty, dt[i][j][ck], dt[i][j][ck + 1]);
                    cc[i][j] = vc_lerp(ty, dc[i][j][ck], dc[i][j][ck + 1]);
                }
            }
            uint8_t* layer = mat + (size_t)yi * 256;
            for (int z = 0; z < 16; z++) {
                const int cj = z >> 2;
                const double tz = (double)(z & 3) * 0.25;
                for (int x = 0; x < 16; x++) {
                    const int ci = x >> 2;
                    const double tx = (double)(x & 3) * 0.25;
                    const double t = vc_lerp(tz, vc_lerp(tx, ct[ci][cj], ct[ci + 1][cj]), vc_lerp(tx, ct[ci][cj + 1], ct[ci + 1][cj + 1]));
                    uint8_t m;
                    if (y == VC_MIN_Y) {
                        m = VC_MAT_BEDROCK;
                    } else if (y < VC_MIN_Y + 5 && (int)(vc_hash3(bx + x, y, bz + z, hseed) % 5u) >= y - VC_MIN_Y) {
                        m = VC_MAT_BEDROCK;
                    } else if (t <= 0.0) {
                        m = y < VC_SEA_LEVEL ? VC_MAT_WATER : VC_MAT_AIR;
                    } else {
                        const double c = vc_lerp(tz, vc_lerp(tx, cc[ci][cj], cc[ci + 1][cj]), vc_lerp(tx, cc[ci][cj + 1], cc[ci + 1][cj + 1]));
                        if (c <= 0.0) {
                            m = y <= VC_LAVA_LEVEL ? VC_MAT_LAVA : VC_MAT_AIR;
                        } else if (y < 0) {
                            m = VC_MAT_DEEPSLATE;
                        } else if (y < 8 && (int)(vc_hash3(bx + x, y, bz + z, hseed ^ 0x9e37u) & 7u) >= y) {
                            m = VC_MAT_DEEPSLATE;
                        } else {
                            m = VC_MAT_STONE;
                        }
                    }
                    layer[z * 16 + x] = m;
                    if (m == VC_MAT_STONE || m == VC_MAT_DEEPSLATE || m == VC_MAT_BEDROCK) cols[z * 16 + x].surface = y;
                }
            }
        }
    }
}

extern "C" uint32_t vc_core_version(void) { return 2; }
