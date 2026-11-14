use rustcraft::ffi::*;
fn main() {
    let seed: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let k = TerrainKernel::new(seed);
    let n = 400;
    let step = 24;
    let (mut ocean, mut land, mut mount, mut river) = (0, 0, 0, 0);
    let mut sums = [0f64; 5];
    let mut sq = [0f64; 5];
    let mut hist = [0u32; 12];
    for j in 0..n {
        for i in 0..n {
            let c = k.column((i - n / 2) * step, (j - n / 2) * step);
            let v = [c.temperature, c.humidity, c.continentalness, c.erosion, c.weirdness];
            for a in 0..5 { sums[a] += v[a] as f64; sq[a] += (v[a] * v[a]) as f64; }
            let h = c.base_height;
            if h < 62.0 { ocean += 1 } else { land += 1 }
            if h > 110.0 { mount += 1 }
            if c.pv < -0.85 && c.continentalness > -0.1 { river += 1 }
            let b = (((h - 20.0) / 15.0).floor() as i32).clamp(0, 11) as usize;
            hist[b] += 1;
        }
    }
    let t = (n * n) as f64;
    println!("seed {seed}: water {:.1}% land {:.1}% >110 {:.1}% rivers {:.1}%", ocean as f64 / t * 100.0, land as f64 / t * 100.0, mount as f64 / t * 100.0, river as f64 / t * 100.0);
    for (a, name) in ["temp", "humid", "cont", "erosion", "weird"].iter().enumerate() {
        let m = sums[a] / t;
        println!("  {name:8} mean {m:+.3} sd {:.3}", (sq[a] / t - m * m).sqrt());
    }
    print!("  height hist (20.. step 15):");
    for h in hist { print!(" {:.1}", h as f64 / t * 100.0); }
    println!();
    let mut mats = vec![0u8; 16 * 16 * 384];
    let mut cols = vec![VcColumn::default(); 256];
    let t0 = std::time::Instant::now();
    let mut counts = [0u64; 6];
    let mut maxs = -64;
    for cz in 0..12 { for cx in 0..12 {
        k.chunk(cx * 7, cz * 7, &mut mats, &mut cols);
        for m in &mats { counts[*m as usize] += 1; }
        for c in &cols { maxs = maxs.max(c.surface); }
    }}
    let el = t0.elapsed().as_secs_f64() * 1000.0 / 144.0;
    let tot: u64 = counts.iter().sum();
    println!("  chunk {:.2} ms; air {:.1}% stone {:.1}% deepslate {:.1}% water {:.1}% lava {:.2}% bedrock {:.2}% max surface {maxs}", el,
        counts[0] as f64 / tot as f64 * 100.0, counts[1] as f64 / tot as f64 * 100.0, counts[2] as f64 / tot as f64 * 100.0, counts[3] as f64 / tot as f64 * 100.0, counts[4] as f64 / tot as f64 * 100.0, counts[5] as f64 / tot as f64 * 100.0);
    // cave fraction below surface
    let (mut solid, mut cave) = (0u64, 0u64);
    for cz in 0..6 { for cx in 0..6 {
        k.chunk(cx * 11 + 300, cz * 11 - 200, &mut mats, &mut cols);
        for z in 0..16 { for x in 0..16 {
            let s = cols[z * 16 + x].surface;
            for y in -60..s.min(60) {
                let m = mats[((y + 64) as usize * 16 + z) * 16 + x];
                if m == MAT_AIR || m == MAT_LAVA { cave += 1 } else if m != MAT_WATER { solid += 1 }
            }
        }}
    }}
    println!("  underground cave fraction {:.1}%", cave as f64 / (cave + solid) as f64 * 100.0);
}
