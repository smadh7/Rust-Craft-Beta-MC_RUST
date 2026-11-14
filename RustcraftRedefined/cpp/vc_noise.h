// Shared inline helpers for the native core: math without libm, hashing, noise.
#ifndef VC_NOISE_H
#define VC_NOISE_H

#include "vc_core.h"

#if defined(_MSC_VER) && !defined(__clang__)
#include <math.h>
static inline double vc_floor(double v) { return floor(v); }
static inline double vc_fabs(double v) { return fabs(v); }
static inline double vc_sqrt(double v) { return sqrt(v); }
#else
static inline double vc_floor(double v) { return __builtin_floor(v); }
static inline double vc_fabs(double v) { return __builtin_fabs(v); }
static inline double vc_sqrt(double v) { return __builtin_sqrt(v); }
#endif

static inline double vc_min(double a, double b) { return a < b ? a : b; }
static inline double vc_max(double a, double b) { return a > b ? a : b; }
static inline double vc_clamp(double v, double lo, double hi) { return v < lo ? lo : (v > hi ? hi : v); }
static inline double vc_lerp(double t, double a, double b) { return a + t * (b - a); }
// Smooth 0..1 ramp between e0 and e1; works with e0 > e1 for a falling ramp.
static inline double vc_smoothstep(double e0, double e1, double x) {
    double t = vc_clamp((x - e0) / (e1 - e0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

static inline uint64_t vc_splitmix(uint64_t* state) {
    uint64_t z = (*state += 0x9e3779b97f4a7c15ULL);
    z = (z ^ (z >> 30)) * 0xbf58476d1ce4e5b9ULL;
    z = (z ^ (z >> 27)) * 0x94d049bb133111ebULL;
    return z ^ (z >> 31);
}

static inline uint32_t vc_hash3(int32_t x, int32_t y, int32_t z, uint32_t seed) {
    uint32_t h = seed ^ ((uint32_t)x * 0x85ebca6bu) ^ ((uint32_t)y * 0xc2b2ae35u) ^ ((uint32_t)z * 0x27d4eb2fu);
    h ^= h >> 15;
    h *= 0x2c1b3c6du;
    h ^= h >> 12;
    h *= 0x297a2d39u;
    h ^= h >> 15;
    return h;
}

// Piecewise linear curve through (xs[i], ys[i]); xs must be increasing.
static inline double vc_curve(const double* xs, const double* ys, int n, double x) {
    if (x <= xs[0]) return ys[0];
    for (int i = 1; i < n; i++) {
        if (x <= xs[i]) {
            double t = (x - xs[i - 1]) / (xs[i] - xs[i - 1]);
            return ys[i - 1] + t * (ys[i] - ys[i - 1]);
        }
    }
    return ys[n - 1];
}

#endif
