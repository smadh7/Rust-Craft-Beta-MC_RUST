//! Writes a contact sheet of every block texture: `cargo run --example texture_sheet -- out.png`.
use rustcraft::art::blocktex;
use rustcraft::util::png;
use rustcraft::world::block::reg;
fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "textures.png".into());
    let names = &reg().textures;
    let cols = 32usize;
    let rows = (names.len() + cols - 1) / cols;
    let scale = 3usize;
    let cell = 16 * scale + 2;
    let (w, h) = (cols * cell, rows * cell);
    let mut px = vec![40u8; w * h * 4];
    for (i, name) in names.iter().enumerate() {
        let img = blocktex::generate(name).unwrap_or_else(blocktex::missing);
        let (ox, oy) = ((i % cols) * cell + 1, (i / cols) * cell + 1);
        for y in 0..16 * scale {
            for x in 0..16 * scale {
                let c = img.px[(y / scale) * 16 + x / scale];
                let o = ((oy + y) * w + ox + x) * 4;
                let checker = if ((x / 6) + (y / 6)) % 2 == 0 { 70 } else { 95 };
                let a = c[3] as u32;
                for k in 0..3 {
                    px[o + k] = ((c[k] as u32 * a + checker * (255 - a)) / 255) as u8;
                }
                px[o + 3] = 255;
            }
        }
    }
    std::fs::write(&out, png::encode_rgba(w as u32, h as u32, &px)).unwrap();
    println!("{} textures -> {out} ({w}x{h})", names.len());
}
