//! Platform services.  Each target supplies the same small set of functions:
//! a clock, logging, cursor capture, persistent storage and a way to quit.

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(not(target_arch = "wasm32"))]
mod native;
#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(windows)]
pub mod windows;
