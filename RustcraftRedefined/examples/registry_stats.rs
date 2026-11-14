//! Prints registry sizes: `cargo run --example registry_stats`.
use rustcraft::world::block::reg;
fn main() {
    let r = reg();
    println!("blocks {} states {} textures {} geometries {} quads {}", r.blocks.len(), r.states.len(), r.textures.len(), r.geoms.len(), r.quads.len());
}
