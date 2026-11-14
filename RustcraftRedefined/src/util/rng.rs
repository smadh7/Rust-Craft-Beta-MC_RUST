//! Deterministic random numbers (xoshiro256++ seeded through splitmix64) and hashing.

#[inline]
pub fn splitmix(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Stateless 64-bit mix of a seed and three coordinates.
#[inline]
pub fn hash64(seed: u64, x: i32, y: i32, z: i32) -> u64 {
    let mut s = seed ^ (x as u32 as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (y as u32 as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f) ^ (z as u32 as u64).wrapping_mul(0x1656_67b1_9e37_79f9);
    splitmix(&mut s)
}

/// 64-bit FNV-1a, used to turn names and seed strings into numbers.
pub fn hash_str(s: &str) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

#[derive(Clone, Debug)]
pub struct Rng {
    s: [u64; 4],
}

impl Rng {
    pub fn new(seed: u64) -> Rng {
        let mut st = seed;
        Rng { s: [splitmix(&mut st), splitmix(&mut st), splitmix(&mut st), splitmix(&mut st)] }
    }
    /// Generator for a position-dependent decision (chunk features, per-block randomness).
    pub fn at(seed: u64, x: i32, z: i32, salt: u64) -> Rng {
        Rng::new(hash64(seed ^ salt.wrapping_mul(0x9e37_79b9_7f4a_7c15), x, 0, z))
    }
    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let result = s[0].wrapping_add(s[3]).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }
    #[inline]
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }
    /// Uniform in 0..1.
    #[inline]
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 * (1.0 / 16_777_216.0)
    }
    #[inline]
    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }
    /// Uniform integer in 0..n (n > 0).
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        ((self.next_u32() as u64 * n as u64) >> 32) as u32
    }
    /// Uniform integer in lo..=hi.
    #[inline]
    pub fn range(&mut self, lo: i32, hi: i32) -> i32 {
        if hi <= lo {
            return lo;
        }
        lo + self.below((hi - lo + 1) as u32) as i32
    }
    #[inline]
    pub fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }
    #[inline]
    pub fn one_in(&mut self, n: u32) -> bool {
        self.below(n) == 0
    }
    /// Triangular distribution centred on `mid` with half-width `spread`.
    pub fn triangle(&mut self, mid: f32, spread: f32) -> f32 {
        mid + spread * (self.f32() - self.f32())
    }
    pub fn gaussian(&mut self) -> f32 {
        // Sum of uniforms: good enough for particle spread and loot jitter.
        let mut t = 0.0;
        for _ in 0..6 {
            t += self.f32();
        }
        (t - 3.0) * 1.414
    }
    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.below(items.len() as u32) as usize]
    }
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i as u32 + 1) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_in_range() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            let v = a.next_u64();
            assert_eq!(v, b.next_u64());
        }
        let mut r = Rng::new(7);
        let mut seen = [false; 6];
        for _ in 0..2000 {
            let v = r.range(-2, 3);
            assert!((-2..=3).contains(&v));
            seen[(v + 2) as usize] = true;
            let f = r.f32();
            assert!((0.0..1.0).contains(&f));
        }
        assert!(seen.iter().all(|s| *s));
    }

    #[test]
    fn position_hash_varies() {
        assert_ne!(hash64(1, 0, 0, 0), hash64(1, 1, 0, 0));
        assert_ne!(hash64(1, 0, 0, 0), hash64(2, 0, 0, 0));
        assert_ne!(hash_str("abc"), hash_str("abd"));
    }
}
