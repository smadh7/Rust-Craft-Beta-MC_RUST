//! Native entry point.  The desktop window back end is implemented for Windows; on other
//! systems use the browser build in `web/` (see BUILD.md).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    {
        if let Err(e) = rustcraft::platform::windows::run() {
            rustcraft::platform::windows::fatal(&e);
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("Rustcraft Redefined {}", env!("CARGO_PKG_VERSION"));
        eprintln!("The native window back end is available on Windows.");
        eprintln!("On this system, run the browser build: tools/build-web.sh, then serve the web/ folder.");
    }
}
