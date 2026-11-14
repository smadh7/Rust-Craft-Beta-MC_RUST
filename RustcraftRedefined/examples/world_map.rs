//! Renders a top-down map and a vertical slice of generated terrain to PNG files.
//! `cargo run --release --example world_map -- <seed> <chunks> <out-prefix>`
use rustcraft::art::blocktex;
use rustcraft::ffi::{MAX_Y, MIN_Y};
use rustcraft::util::png;
use rustcraft::world::biome::Biome;
use rustcraft::world::block::{reg, F_WATER, TINT_FOLIAGE, TINT_GRASS, TINT_WATER};
use rustcraft::world::chunk::{CoordMap, Region};
use rustcraft::world::gen::Generator;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let n: i32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(24);
    let prefix = args.get(3).cloned().unwrap_or_else(|| "map".into());
    let gen = Generator::new(seed);
    let r = reg();
    // Average colour of each texture.
    let avg: Vec<[f32; 3]> = r.textures.iter().map(|name| {
        let img = blocktex::generate(name).unwrap_or_else(blocktex::missing);
        let (mut s, mut w) = ([0f32; 3], 0f32);
        for p in &img.px { let a = p[3] as f32 / 255.0; for k in 0..3 { s[k] += p[k] as f32 * a; } w += a; }
        [s[0] / w.max(1.0), s[1] / w.max(1.0), s[2] / w.max(1.0)]
    }).collect();
    let t0 = std::time::Instant::now();
    let mut chunks: CoordMap<Box<rustcraft::world::chunk::Chunk>> = CoordMap::default();
    for cz in -1..=n { for cx in -1..=n { chunks.insert((cx, cz), gen.terrain(cx, cz)); } }
    let t_terrain = t0.elapsed().as_secs_f64();
    let t1 = std::time::Instant::now();
    for cz in 0..n { for cx in 0..n {
        let mut reg3 = Region { cx0: cx - 1, cz0: cz - 1, size: 3, chunks: (0..9).map(|i| chunks.remove(&(cx - 1 + i % 3, cz - 1 + i / 3))).collect() };
        gen.decorate(&mut reg3, cx, cz);
        for (i, c) in reg3.chunks.into_iter().enumerate() { if let Some(c) = c { chunks.insert((cx - 1 + i as i32 % 3, cz - 1 + i as i32 / 3), c); } }
    }}
    let t_dec = t1.elapsed().as_secs_f64();
    let size = (n * 16) as usize;
    let mut px = vec![0u8; size * size * 4];
    let mut counts = std::collections::HashMap::new();
    let colour = |s: u16, biome: Biome| -> [f32; 3] {
        let ms = &r.mesh_states[s as usize];
        let layer = if r.has(s, F_WATER) && r.block(s).name == "water" { 0 } else { r.texsets[ms.texset as usize].layer[1] };
        let mut c = avg[layer as usize];
        let info = biome.info();
        let tint = match r.block(s).tint { TINT_GRASS => Some(info.grass), TINT_FOLIAGE => Some(info.foliage), TINT_WATER => Some(info.water), _ => None };
        if let Some(t) = tint { c = [c[0] * ((t >> 16) & 255) as f32 / 255.0, c[1] * ((t >> 8) & 255) as f32 / 255.0, c[2] * (t & 255) as f32 / 255.0]; }
        c
    };
    for z in 0..size { for x in 0..size {
        let ch = &chunks[&((x / 16) as i32, (z / 16) as i32)];
        let i = (z % 16) * 16 + x % 16;
        let biome = Biome::from_u8(ch.biome[i]);
        *counts.entry(biome.name()).or_insert(0u32) += 1;
        let top = ch.top[i] as i32;
        let s = ch.get(x % 16, top, z % 16);
        let mut c = colour(s, biome);
        let hn = if z > 0 { chunks[&((x / 16) as i32, ((z - 1) / 16) as i32)].top[((z - 1) % 16) * 16 + x % 16] as i32 } else { top };
        let mut shade = 1.0 + ((top - hn) as f32 * 0.08).clamp(-0.3, 0.3);
        if r.block(s).name == "water" {
            let mut d = 0; while ch.get(x % 16, top - d, z % 16) == s && d < 40 { d += 1; }
            shade = 1.0 - (d as f32 / 40.0) * 0.55;
        } else { shade *= 0.8 + (top as f32 - 63.0) / 400.0; }
        for k in 0..3 { c[k] = (c[k] * shade).clamp(0.0, 255.0); }
        let o = (z * size + x) * 4;
        px[o] = c[0] as u8; px[o + 1] = c[1] as u8; px[o + 2] = c[2] as u8; px[o + 3] = 255;
    }}
    std::fs::write(format!("{prefix}_top.png"), png::encode_rgba(size as u32, size as u32, &px)).unwrap();
    // Vertical slice through the middle (z fixed).
    let h = (MAX_Y - MIN_Y) as usize;
    let mut sl = vec![0u8; size * h * 4];
    let zc = size / 2;
    for x in 0..size { for yi in 0..h {
        let y = MAX_Y - 1 - yi as i32;
        let ch = &chunks[&((x / 16) as i32, (zc / 16) as i32)];
        let s = ch.get(x % 16, y, zc % 16);
        let c = if s == 0 { if y > ch.top[(zc % 16) * 16 + x % 16] as i32 { [150.0, 190.0, 255.0] } else { [20.0, 20.0, 26.0] } } else { colour(s, Biome::from_u8(ch.biome[(zc % 16) * 16 + x % 16])) };
        let o = (yi * size + x) * 4;
        sl[o] = c[0] as u8; sl[o + 1] = c[1] as u8; sl[o + 2] = c[2] as u8; sl[o + 3] = 255;
    }}
    std::fs::write(format!("{prefix}_slice.png"), png::encode_rgba(size as u32, h as u32, &sl)).unwrap();
    let mut list: Vec<_> = counts.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1));
    let total = (size * size) as f32;
    println!("terrain {:.1} ms/chunk, decorate {:.1} ms/chunk", t_terrain * 1000.0 / ((n + 2) * (n + 2)) as f64, t_dec * 1000.0 / (n * n) as f64);
    println!("{}", list.iter().map(|(n, c)| format!("{n} {:.1}%", *c as f32 / total * 100.0)).collect::<Vec<_>>().join(", "));
}
