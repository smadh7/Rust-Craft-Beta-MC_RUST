//! Sound effects.  There are no audio files: every effect is synthesised from noise and a few
//! sine waves when the game starts, and handed to the platform layer to play.

use crate::platform;
use crate::util::math::DVec3;
use crate::world::block::Sound;
use std::sync::atomic::{AtomicU32, Ordering};

pub const RATE: u32 = 22050;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum Sfx {
    Stone,
    Wood,
    Grass,
    Sand,
    Gravel,
    Glass,
    Snow,
    Wet,
    Pop,
    Hurt,
    Explode,
    Fizz,
    Click,
    Eat,
    Boing,
    Geyser,
    Thud,
}

pub const ALL: [Sfx; 17] = [
    Sfx::Stone, Sfx::Wood, Sfx::Grass, Sfx::Sand, Sfx::Gravel, Sfx::Glass, Sfx::Snow, Sfx::Wet, Sfx::Pop, Sfx::Hurt, Sfx::Explode, Sfx::Fizz, Sfx::Click, Sfx::Eat, Sfx::Boing, Sfx::Geyser,
    Sfx::Thud,
];

static VOLUME: AtomicU32 = AtomicU32::new(0x3f80_0000);

/// Master volume, 0 to 1.
pub fn set_volume(v: f32) {
    VOLUME.store(v.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
}
pub fn volume() -> f32 {
    f32::from_bits(VOLUME.load(Ordering::Relaxed))
}

/// The effect a block makes when it is hit, broken, placed or walked on.
pub fn for_block(sound: Sound) -> Sfx {
    match sound {
        Sound::Wood => Sfx::Wood,
        Sound::Grass => Sfx::Grass,
        Sound::Sand => Sfx::Sand,
        Sound::Gravel => Sfx::Gravel,
        Sound::Glass => Sfx::Glass,
        Sound::Snow => Sfx::Snow,
        Sound::Wet => Sfx::Wet,
        Sound::Wool => Sfx::Snow,
        Sound::Stone | Sound::Metal => Sfx::Stone,
    }
}

struct Synth {
    seed: u32,
    low: f32,
}

impl Synth {
    /// White noise in -1..1.
    fn noise(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / 8_388_608.0 - 1.0
    }
    /// Noise through a one-pole low-pass; `a` near 0 is dull, near 1 is bright.
    fn dull(&mut self, a: f32) -> f32 {
        let n = self.noise();
        self.low += (n - self.low) * a;
        self.low
    }
    /// Noise with the low end removed.
    fn hiss(&mut self, a: f32) -> f32 {
        let n = self.noise();
        self.low += (n - self.low) * a;
        n - self.low
    }
}

fn sine(phase: f32) -> f32 {
    (phase * std::f32::consts::TAU).sin()
}

/// Renders one effect as 16-bit mono samples at `RATE`.
pub fn synth(sfx: Sfx) -> Vec<i16> {
    let mut s = Synth { seed: 0x1234_5678 ^ (sfx as u32).wrapping_mul(0x9E37_79B9), low: 0.0 };
    let ms = match sfx {
        Sfx::Explode => 1300,
        Sfx::Fizz => 420,
        Sfx::Geyser => 340,
        Sfx::Boing => 220,
        Sfx::Glass => 200,
        Sfx::Hurt | Sfx::Eat | Sfx::Snow | Sfx::Sand => 150,
        Sfx::Click => 30,
        Sfx::Pop => 70,
        _ => 120,
    };
    let n = (RATE as usize * ms) / 1000;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / RATE as f32;
        let u = i as f32 / n as f32;
        let mut tone = |freq: f32| {
            phase += freq / RATE as f32;
            sine(phase)
        };
        let v = match sfx {
            Sfx::Stone => s.dull(0.45) * (-t * 42.0).exp() + tone(160.0) * (-t * 60.0).exp() * 0.4,
            Sfx::Wood => s.dull(0.22) * (-t * 38.0).exp() * 0.8 + tone(230.0 - t * 400.0) * (-t * 34.0).exp() * 0.7,
            Sfx::Grass => s.dull(0.6) * (-t * 30.0).exp() * if (i / 90) % 3 == 0 { 0.9 } else { 0.35 },
            Sfx::Sand => s.hiss(0.25) * (-t * 22.0).exp() * 0.6 * (u * 12.0).min(1.0),
            Sfx::Gravel => {
                // Three quick crunches.
                let k = (t * 28.0).fract();
                s.dull(0.5) * (-k * 6.0).exp() * (-t * 14.0).exp()
            }
            Sfx::Glass => {
                let ring = tone(2150.0) + sine(t * 3420.0) * 0.7 + sine(t * 5230.0) * 0.5;
                ring * (-t * 26.0).exp() * 0.3 + s.hiss(0.5) * (-t * 90.0).exp() * 0.7
            }
            Sfx::Snow => s.dull(0.14) * (u * 8.0).min(1.0) * (-t * 20.0).exp() * 1.6,
            Sfx::Wet => tone(320.0 - t * 1300.0) * (-t * 30.0).exp() * 0.7 + s.dull(0.2) * (-t * 40.0).exp() * 0.5,
            Sfx::Pop => tone(820.0 + t * 9000.0) * (-t * 55.0).exp(),
            Sfx::Hurt => tone(330.0 - t * 1000.0) * (-t * 16.0).exp() * 0.8 + s.dull(0.3) * (-t * 30.0).exp() * 0.4,
            Sfx::Explode => s.dull(0.05) * (-t * 3.2).exp() * 3.0 + tone(58.0 - t * 20.0) * (-t * 5.0).exp() * 0.9 + s.dull(0.5) * (-t * 30.0).exp() * 0.5,
            Sfx::Fizz => s.hiss(0.35) * (-t * 9.0).exp() * 0.7,
            Sfx::Click => (if tone(1000.0) > 0.0 { 0.5 } else { -0.5 }) * (1.0 - u),
            Sfx::Eat => {
                let k = (t * 13.0).fract();
                s.dull(0.4) * (-k * 9.0).exp() * 0.9
            }
            Sfx::Boing => tone(170.0 + (u * 3.2).sin() * 260.0) * (1.0 - u).powf(0.7) * 0.9,
            Sfx::Geyser => s.hiss(0.12) * (u * 6.0).min(1.0) * ((1.0 - u) * 6.0).min(1.0) * 0.6,
            Sfx::Thud => tone(95.0 - t * 200.0) * (-t * 24.0).exp() + s.dull(0.1) * (-t * 30.0).exp() * 0.8,
        };
        // Fade the last few milliseconds so nothing clicks when it ends.
        let fade = ((n - i) as f32 / 200.0).min(1.0);
        out.push((v * fade).clamp(-1.0, 1.0));
    }
    // Normalise so every effect peaks at the same level.
    let peak = out.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-6);
    out.into_iter().map(|v| (v / peak * 30000.0) as i16).collect()
}

/// Synthesises every effect and hands it to the platform.
pub fn init() {
    for sfx in ALL {
        platform::sound_load(sfx as u32, &synth(sfx), RATE);
    }
}

pub fn play(sfx: Sfx, volume: f32, pitch: f32) {
    let v = volume * self::volume();
    if v > 0.004 {
        platform::sound_play(sfx as u32, v.min(1.0), pitch.clamp(0.3, 3.5));
    }
}

/// Plays an effect at a place in the world; it fades out over 16 blocks from the listener.
pub fn play_at(sfx: Sfx, at: DVec3, listener: DVec3, volume: f32, pitch: f32) {
    let d = (at - listener).length() as f32;
    let reach = if sfx == Sfx::Explode { 64.0 } else { 16.0 };
    if d < reach {
        play(sfx, volume * (1.0 - d / reach), pitch);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_effect_is_audible_and_short() {
        for sfx in ALL {
            let pcm = synth(sfx);
            assert!(!pcm.is_empty() && pcm.len() <= RATE as usize * 3 / 2, "{sfx:?} has {} samples", pcm.len());
            let peak = pcm.iter().map(|v| (*v as i32).abs()).max().unwrap();
            assert!((29000..=30000).contains(&peak), "{sfx:?} peaks at {peak}");
            let rms = (pcm.iter().map(|v| (*v as f64).powi(2)).sum::<f64>() / pcm.len() as f64).sqrt();
            assert!(rms > 900.0, "{sfx:?} is nearly silent (rms {rms:.0})");
            // The ending is faded out.
            assert!(pcm[pcm.len() - 1].abs() < 400, "{sfx:?} ends abruptly");
            assert_eq!(sfx as usize, ALL.iter().position(|s| *s == sfx).unwrap());
        }
        assert_eq!(for_block(Sound::Wood), Sfx::Wood);
        set_volume(0.5);
        assert_eq!(volume(), 0.5);
        set_volume(1.0);
    }
}
