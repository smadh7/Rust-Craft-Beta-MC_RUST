use rustcraft_redefined::{generate_chunk, mesh, pad_chunk};
use std::time::Instant;
fn main() {
    let start=Instant::now();
    let chunks: Vec<_>=(0..256).map(|i| generate_chunk(i%16,i/16,12345)).collect();
    println!("World generation: 256 chunks in {:?}",start.elapsed());
    let start=Instant::now();
    let mut quads=0;
    for c in &chunks { quads+=mesh(&pad_chunk(c,[None;4])).len()/2; }
    println!("Meshing: 256 chunks, {quads} quads in {:?}",start.elapsed());
}
