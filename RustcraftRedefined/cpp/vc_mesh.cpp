// Section mesher: turns a padded 18^3 block/light volume into lit, textured quads.
//
// All block shapes are data.  The Rust side builds a table that maps every block
// state to a list of quads plus a texture set; this kernel only decides which
// quads are visible, lights them (smooth lighting + ambient occlusion) and
// handles fluids, whose surface depends on their neighbours.
#include "vc_core.h"
#include "vc_noise.h"

namespace {

const int PAD = 18;
inline int pidx(int x, int y, int z) { return (y * PAD + z) * PAD + x; }

const int DX[6] = {0, 0, 0, 0, -1, 1};
const int DY[6] = {-1, 1, 0, 0, 0, 0};
const int DZ[6] = {0, 0, -1, 1, 0, 0};
const int OPP[6] = {1, 0, 3, 2, 5, 4};
const float SHADE[6] = {0.5f, 1.0f, 0.8f, 0.8f, 0.6f, 0.6f};
// Tangent axes of each face (0 = x, 1 = y, 2 = z).
const int UA[6] = {0, 0, 0, 0, 2, 2};
const int VA[6] = {2, 2, 1, 1, 1, 1};
const float AO_LEVEL[4] = {0.4f, 0.6f, 0.8f, 1.0f};

struct Corner {
    float ao, sky, block;
};

struct Ctx {
    const VcMeshTables* t;
    const VcMeshInput* in;
    VcVertex* out[2];
    uint32_t cap[2];
    uint32_t count[2];
};

struct QuadOut {
    float pos[4][3];
    float uv[4][2];
    float rgb[4][3];
    float sky[4], block[4];
    float ao[4];
    uint16_t layer;
    uint8_t dir;
};

inline uint8_t to_u8(float v) { return v <= 0.0f ? 0 : (v >= 255.0f ? 255 : (uint8_t)(v + 0.5f)); }

inline void put_vertex(VcVertex* v, const QuadOut& q, int i) {
    v->x = (int16_t)(q.pos[i][0] * 256.0f + (q.pos[i][0] < 0 ? -0.5f : 0.5f));
    v->y = (int16_t)(q.pos[i][1] * 256.0f + (q.pos[i][1] < 0 ? -0.5f : 0.5f));
    v->z = (int16_t)(q.pos[i][2] * 256.0f + (q.pos[i][2] < 0 ? -0.5f : 0.5f));
    v->layer = q.layer;
    v->u = (int16_t)(q.uv[i][0] * 4096.0f + (q.uv[i][0] < 0 ? -0.5f : 0.5f));
    v->v = (int16_t)(q.uv[i][1] * 4096.0f + (q.uv[i][1] < 0 ? -0.5f : 0.5f));
    v->r = to_u8(q.rgb[i][0] * 255.0f);
    v->g = to_u8(q.rgb[i][1] * 255.0f);
    v->b = to_u8(q.rgb[i][2] * 255.0f);
    v->a = 255;
    v->sky = to_u8(q.sky[i]);
    v->block = to_u8(q.block[i]);
    v->dir = q.dir;
    v->pad = 0;
}

void emit(Ctx& c, int stream, const QuadOut& q, bool reversed) {
    const uint32_t n = c.count[stream]++;
    if (n >= c.cap[stream]) return;
    VcVertex* v = c.out[stream] + (size_t)n * 4;
    // Rotate the quad so the triangle split follows the smoother AO diagonal.
    const int start = (q.ao[0] + q.ao[2] < q.ao[1] + q.ao[3]) ? 1 : 0;
    for (int i = 0; i < 4; i++) {
        const int src = reversed ? ((start + 4 - i) & 3) : ((start + i) & 3);
        put_vertex(v + i, q, src);
    }
}

inline const VcStateInfo& info_at(const Ctx& c, int idx) {
    uint32_t s = c.in->blocks[idx];
    if (s >= c.t->num_states) s = 0;
    return c.t->states[s];
}

// Light and ambient occlusion at the four corners of a face.
// out[cu + 2 * cv] is the corner with tangent coordinates (cu, cv).
void face_corners(const Ctx& c, int px, int py, int pz, int dir, bool inset, Corner out[4]) {
    const int bx = px + (inset ? 0 : DX[dir]);
    const int by = py + (inset ? 0 : DY[dir]);
    const int bz = pz + (inset ? 0 : DZ[dir]);
    int u[3] = {0, 0, 0}, v[3] = {0, 0, 0};
    u[UA[dir]] = 1;
    v[VA[dir]] = 1;
    const uint8_t* light = c.in->light;
    const uint8_t lc = light[pidx(bx, by, bz)];
    for (int cv = 0; cv < 2; cv++) {
        for (int cu = 0; cu < 2; cu++) {
            const int su = cu ? 1 : -1, sv = cv ? 1 : -1;
            const int i1 = pidx(bx + su * u[0], by + su * u[1], bz + su * u[2]);
            const int i2 = pidx(bx + sv * v[0], by + sv * v[1], bz + sv * v[2]);
            const int i3 = pidx(bx + su * u[0] + sv * v[0], by + su * u[1] + sv * v[1], bz + su * u[2] + sv * v[2]);
            const bool o1 = (info_at(c, i1).flags & VC_S_AO) != 0;
            const bool o2 = (info_at(c, i2).flags & VC_S_AO) != 0;
            const bool o3 = (info_at(c, i3).flags & VC_S_AO) != 0;
            int sky = lc >> 4, blk = lc & 15, n = 1;
            if (!o1) { sky += light[i1] >> 4; blk += light[i1] & 15; n++; }
            if (!o2) { sky += light[i2] >> 4; blk += light[i2] & 15; n++; }
            if (!(o1 && o2) && !o3) { sky += light[i3] >> 4; blk += light[i3] & 15; n++; }
            const int level = (o1 && o2) ? 0 : 3 - ((int)o1 + (int)o2 + (int)o3);
            Corner& r = out[cu + 2 * cv];
            r.ao = AO_LEVEL[level];
            r.sky = (float)sky * 17.0f / (float)n;
            r.block = (float)blk * 17.0f / (float)n;
        }
    }
}

inline void tint_rgb(const Ctx& c, uint8_t tint, int px, int pz, float rgb[3]) {
    uint32_t col = 0xffffff;
    const int ci = pz * PAD + px;
    if (tint == 1) col = c.in->tint_grass[ci];
    else if (tint == 2) col = c.in->tint_foliage[ci];
    else if (tint == 3) col = c.in->tint_water[ci];
    else if (tint >= 4) col = c.t->fixed_tints[tint - 4];
    rgb[0] = (float)((col >> 16) & 255) * (1.0f / 255.0f);
    rgb[1] = (float)((col >> 8) & 255) * (1.0f / 255.0f);
    rgb[2] = (float)(col & 255) * (1.0f / 255.0f);
}

// ------------------------------------------------------------------ fluids --
inline bool is_fluid(const VcStateInfo& i, bool lava) {
    if (i.fluid != 0) return ((i.flags & VC_S_LAVA) != 0) == lava;
    return !lava && (i.flags & VC_S_WATERLOGGED) != 0;
}

inline float fluid_own_height(const VcStateInfo& i) {
    if (i.fluid == 0) return 8.0f / 9.0f; // waterlogged block
    const int level = i.fluid - 1;
    if (level >= 8) return 8.0f / 9.0f;
    return (float)(8 - level) / 9.0f;
}

float fluid_corner_height(const Ctx& c, int px, int py, int pz, int cx, int cz, bool lava) {
    float total = 0.0f, weight = 0.0f;
    for (int b = 0; b < 2; b++) {
        for (int a = 0; a < 2; a++) {
            const int x = px + cx - 1 + a, z = pz + cz - 1 + b;
            const VcStateInfo& i = info_at(c, pidx(x, py, z));
            if (is_fluid(i, lava)) {
                if (is_fluid(info_at(c, pidx(x, py + 1, z)), lava)) return 1.0f;
                const float h = fluid_own_height(i);
                if (h >= 0.8f) { total += h * 10.0f; weight += 10.0f; }
                else { total += h; weight += 1.0f; }
            } else if (!(i.flags & VC_S_AO)) {
                weight += 1.0f;
            }
        }
    }
    return weight > 0.0f ? total / weight : 0.0f;
}

void emit_fluid(Ctx& c, int px, int py, int pz, const VcStateInfo& info) {
    const bool lava = info.fluid != 0 && (info.flags & VC_S_LAVA) != 0;
    const int stream = lava ? 0 : 1;
    const float bx = (float)(px - 1), by = (float)(py - 1), bz = (float)(pz - 1);
    const uint8_t* light = c.in->light;

    float rgb[3] = {1.0f, 1.0f, 1.0f};
    if (!lava) tint_rgb(c, 3, px, pz, rgb);

    const bool above_same = is_fluid(info_at(c, pidx(px, py + 1, pz)), lava);
    float h00 = 1.0f, h10 = 1.0f, h01 = 1.0f, h11 = 1.0f;
    if (!above_same) {
        h00 = fluid_corner_height(c, px, py, pz, 0, 0, lava);
        h10 = fluid_corner_height(c, px, py, pz, 1, 0, lava);
        h01 = fluid_corner_height(c, px, py, pz, 0, 1, lava);
        h11 = fluid_corner_height(c, px, py, pz, 1, 1, lava);
    }

    QuadOut q;
    for (int i = 0; i < 4; i++) q.ao[i] = 1.0f;

    auto set_light = [&](int idx, float shade) {
        const uint8_t l = lava ? 0xff : light[idx];
        for (int i = 0; i < 4; i++) {
            q.sky[i] = (float)(l >> 4) * 17.0f;
            q.block[i] = (float)(l & 15) * 17.0f;
            q.rgb[i][0] = rgb[0] * shade;
            q.rgb[i][1] = rgb[1] * shade;
            q.rgb[i][2] = rgb[2] * shade;
        }
    };

    // Top surface.
    if (!above_same) {
        const int above = pidx(px, py + 1, pz);
        const bool covered = (info_at(c, above).flags & VC_S_AO) != 0;
        set_light(covered ? pidx(px, py, pz) : above, 1.0f);
        const float fxv = (h00 + h01) - (h10 + h11);
        const float fzv = (h00 + h10) - (h01 + h11);
        const float len2 = fxv * fxv + fzv * fzv;
        static const float cx[4] = {0, 0, 1, 1}, cz[4] = {0, 1, 1, 0};
        const float hs[4] = {h00, h01, h11, h10};
        float dxn = 0.0f, dzn = 1.0f;
        if (len2 > 1e-6f) {
            const float inv = 1.0f / (float)vc_sqrt((double)len2);
            dxn = fxv * inv;
            dzn = fzv * inv;
            q.layer = lava ? c.t->lava_flow : c.t->water_flow;
        } else {
            q.layer = lava ? c.t->lava_still : c.t->water_still;
        }
        for (int i = 0; i < 4; i++) {
            q.pos[i][0] = bx + cx[i];
            q.pos[i][1] = by + hs[i];
            q.pos[i][2] = bz + cz[i];
            const float ox = cx[i] - 0.5f, oz = cz[i] - 0.5f;
            if (len2 > 1e-6f) {
                q.uv[i][0] = 0.5f + (ox * dzn - oz * dxn);
                q.uv[i][1] = 0.5f - (ox * dxn + oz * dzn);
            } else {
                q.uv[i][0] = cx[i];
                q.uv[i][1] = cz[i];
            }
        }
        q.dir = 1;
        emit(c, stream, q, false);
        if (!lava && !covered) emit(c, stream, q, true);
    }

    // Bottom.
    {
        const int below = pidx(px, py - 1, pz);
        const VcStateInfo& bi = info_at(c, below);
        if (!is_fluid(bi, lava) && !(bi.occlude & (1u << 1))) {
            set_light(below, SHADE[0]);
            static const float cx[4] = {0, 1, 1, 0}, cz[4] = {0, 0, 1, 1};
            for (int i = 0; i < 4; i++) {
                q.pos[i][0] = bx + cx[i];
                q.pos[i][1] = by;
                q.pos[i][2] = bz + cz[i];
                q.uv[i][0] = cx[i];
                q.uv[i][1] = cz[i];
            }
            q.layer = lava ? c.t->lava_still : c.t->water_still;
            q.dir = 0;
            emit(c, stream, q, false);
        }
    }

    // Sides: north (-Z), south (+Z), west (-X), east (+X).
    for (int d = 2; d < 6; d++) {
        const int nidx = pidx(px + DX[d], py, pz + DZ[d]);
        const VcStateInfo& ni = info_at(c, nidx);
        if (is_fluid(ni, lava) || (ni.occlude & (1u << OPP[d]))) continue;
        float x0, z0, x1, z1, ha, hb; // edge from (x0, z0) to (x1, z1), counter-clockwise seen from outside
        if (d == 2) { x0 = 1; z0 = 0; x1 = 0; z1 = 0; ha = h10; hb = h00; }
        else if (d == 3) { x0 = 0; z0 = 1; x1 = 1; z1 = 1; ha = h01; hb = h11; }
        else if (d == 4) { x0 = 0; z0 = 0; x1 = 0; z1 = 1; ha = h00; hb = h01; }
        else { x0 = 1; z0 = 1; x1 = 1; z1 = 0; ha = h11; hb = h10; }
        set_light(nidx, SHADE[d]);
        q.pos[0][0] = bx + x0; q.pos[0][1] = by;      q.pos[0][2] = bz + z0;
        q.pos[1][0] = bx + x1; q.pos[1][1] = by;      q.pos[1][2] = bz + z1;
        q.pos[2][0] = bx + x1; q.pos[2][1] = by + hb; q.pos[2][2] = bz + z1;
        q.pos[3][0] = bx + x0; q.pos[3][1] = by + ha; q.pos[3][2] = bz + z0;
        q.uv[0][0] = 0; q.uv[0][1] = 1;
        q.uv[1][0] = 1; q.uv[1][1] = 1;
        q.uv[2][0] = 1; q.uv[2][1] = 1.0f - hb;
        q.uv[3][0] = 0; q.uv[3][1] = 1.0f - ha;
        q.layer = lava ? c.t->lava_flow : c.t->water_flow;
        q.dir = (uint8_t)d;
        emit(c, stream, q, false);
        if (!lava) emit(c, stream, q, true);
    }
}

// ------------------------------------------------------------------ blocks --
void emit_block(Ctx& c, int px, int py, int pz, uint16_t state, const VcStateInfo& info) {
    const VcGeom& geom = c.t->geoms[info.geom];
    const VcTexSet& tex = c.t->texsets[info.texset];
    const int stream = (info.flags & VC_S_TRANSLUCENT) ? 1 : 0;
    const uint8_t* light = c.in->light;
    const uint8_t own = light[pidx(px, py, pz)];

    float ox = (float)(px - 1), oy = (float)(py - 1), oz = (float)(pz - 1);
    if (info.flags & VC_S_OFFSET) {
        const uint32_t h = vc_hash3(c.in->origin_x + px - 1, 0, c.in->origin_z + pz - 1, 0x5eedu);
        ox += ((float)(h & 15u) / 15.0f - 0.5f) * 0.5f;
        oz += ((float)((h >> 4) & 15u) / 15.0f - 0.5f) * 0.5f;
    }

    float tint[3] = {1.0f, 1.0f, 1.0f};
    if (info.tint) tint_rgb(c, info.tint, px, pz, tint);

    Corner corners[12][4];
    uint32_t have = 0; // bit (dir + 6 * inset) set when corners[...] is valid

    const VcQuad* quads = c.t->quads + geom.first_quad;
    for (uint32_t qi = 0; qi < geom.quad_count; qi++) {
        const VcQuad& mq = quads[qi];
        const int dir = mq.dir < 6 ? mq.dir : 1;
        if (mq.cull < 6) {
            const int nidx = pidx(px + DX[mq.cull], py + DY[mq.cull], pz + DZ[mq.cull]);
            const VcStateInfo& ni = info_at(c, nidx);
            if (ni.occlude & (1u << OPP[mq.cull])) continue;
            if ((info.flags & VC_S_CULL_SAME) && c.in->blocks[nidx] == state) continue;
        }

        QuadOut q;
        q.layer = tex.layer[mq.slot & 7];
        q.dir = (uint8_t)dir;
        const float shade = (mq.flags & VC_Q_NOSHADE) ? 1.0f : SHADE[dir];
        const bool smooth = mq.cull < 6 || (mq.flags & VC_Q_INSET);
        const Corner* cs = nullptr;
        if (smooth && !(info.flags & VC_S_FULLBRIGHT)) {
            const bool inset = mq.cull >= 6;
            const int slot = dir + (inset ? 6 : 0);
            if (!(have & (1u << slot))) {
                face_corners(c, px, py, pz, dir, inset, corners[slot]);
                have |= 1u << slot;
            }
            cs = corners[slot];
        }
        for (int i = 0; i < 4; i++) {
            const float lx = mq.pos[i][0], ly = mq.pos[i][1], lz = mq.pos[i][2];
            q.pos[i][0] = ox + lx;
            q.pos[i][1] = oy + ly;
            q.pos[i][2] = oz + lz;
            q.uv[i][0] = mq.uv[i][0];
            q.uv[i][1] = mq.uv[i][1];
            float ao = 1.0f, sky, blk;
            if (info.flags & VC_S_FULLBRIGHT) {
                sky = 255.0f;
                blk = 255.0f;
            } else if (cs) {
                const float l[3] = {lx, ly, lz};
                const float tu = (float)vc_clamp((double)l[UA[dir]], 0.0, 1.0);
                const float tv = (float)vc_clamp((double)l[VA[dir]], 0.0, 1.0);
                const float w00 = (1 - tu) * (1 - tv), w10 = tu * (1 - tv), w01 = (1 - tu) * tv, w11 = tu * tv;
                ao = cs[0].ao * w00 + cs[1].ao * w10 + cs[2].ao * w01 + cs[3].ao * w11;
                sky = cs[0].sky * w00 + cs[1].sky * w10 + cs[2].sky * w01 + cs[3].sky * w11;
                blk = cs[0].block * w00 + cs[1].block * w10 + cs[2].block * w01 + cs[3].block * w11;
            } else {
                sky = (float)(own >> 4) * 17.0f;
                blk = (float)(own & 15) * 17.0f;
            }
            q.ao[i] = ao;
            q.sky[i] = sky;
            q.block[i] = blk;
            const float m = shade * ao;
            if (mq.flags & VC_Q_TINT) {
                q.rgb[i][0] = tint[0] * m;
                q.rgb[i][1] = tint[1] * m;
                q.rgb[i][2] = tint[2] * m;
            } else {
                q.rgb[i][0] = q.rgb[i][1] = q.rgb[i][2] = m;
            }
        }
        emit(c, stream, q, false);
        if (mq.flags & VC_Q_DOUBLE) emit(c, stream, q, true);
    }
}

} // namespace

extern "C" int32_t vc_mesh_section(const VcMeshTables* tables, const VcMeshInput* in,
                                   VcVertex* out_opaque, uint32_t cap_opaque,
                                   VcVertex* out_translucent, uint32_t cap_translucent,
                                   uint32_t* counts) {
    Ctx c;
    c.t = tables;
    c.in = in;
    c.out[0] = out_opaque;
    c.out[1] = out_translucent;
    c.cap[0] = cap_opaque;
    c.cap[1] = cap_translucent;
    c.count[0] = c.count[1] = 0;

    for (int y = 1; y <= 16; y++) {
        for (int z = 1; z <= 16; z++) {
            const uint16_t* row = in->blocks + pidx(1, y, z);
            for (int x = 1; x <= 16; x++) {
                const uint16_t state = row[x - 1];
                if (state == 0 || state >= tables->num_states) continue;
                const VcStateInfo& info = tables->states[state];
                if (info.fluid != 0) {
                    emit_fluid(c, x, y, z, info);
                    continue;
                }
                if (info.flags & VC_S_WATERLOGGED) emit_fluid(c, x, y, z, info);
                if (info.geom != 0) emit_block(c, x, y, z, state, info);
            }
        }
    }

    counts[0] = c.count[0];
    counts[1] = c.count[1];
    return (c.count[0] <= cap_opaque && c.count[1] <= cap_translucent) ? 1 : 0;
}
