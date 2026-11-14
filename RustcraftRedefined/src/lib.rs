//! Rustcraft Redefined: an unofficial, clean-room voxel sandbox.
//!
//! Rust owns the game; a small freestanding C++ core (`cpp/`) supplies the hot numeric kernels
//! (noise, terrain density, section meshing) through a C ABI.

pub mod art;
pub mod audio;
pub mod ffi;
pub mod game;
pub mod gfx;
pub mod input;
pub mod item;
pub mod platform;
pub mod util;
pub mod world;
