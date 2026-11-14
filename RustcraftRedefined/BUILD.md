# Building

The project has no crate dependencies. You need:

- Rust 1.70 or newer (`cargo`)
- A C++17 compiler for the native core — any one of: Visual Studio Build Tools, clang, GCC,
  MinGW-w64, or Zig. `build.rs` finds it automatically; set `CXX` to choose one.

## Windows

```
cd RustcraftRedefined
cargo build --release
target\release\RustcraftRedefined.exe
```

With the MSVC toolchain, `build.rs` uses `cl` (through `vcvarsall` if needed), `clang-cl`,
`clang++`, `zig c++` or `g++`, whichever it finds first.

## Browser

```
rustup target add wasm32-unknown-unknown
cd RustcraftRedefined
tools/build-web.sh
```

The script needs a C++ compiler that can target `wasm32` (Zig or clang) and `python3`. It writes
`web/rustcraft_wasm.js`, which embeds the WebAssembly module so `web/index.html` also works when
opened directly from disk. To serve it instead: `python3 -m http.server --directory web`.

## Cross-compiling the Windows executable from Linux or macOS

```
rustup target add x86_64-pc-windows-gnu
tools/build-windows.sh
```

This needs a MinGW-w64 linker, or Zig: point `CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER` at a
wrapper that runs `zig cc -target x86_64-windows-gnu`. The executable committed at the repository
root was produced this way and has not been run on Windows.

## Tests and tools

```
cargo test                                    # unit tests
cargo run --release --example world_map -- 1 40 map    # top-down map and slice of seed 1
cargo run --release --example texture_sheet -- textures.png
node tools/shot.mjs out.png "newworld 1" "time set noon"   # headless screenshot (needs puppeteer-core)
```

Game data (logs, screenshots, later worlds) goes to `%APPDATA%\Rustcraft Redefined` on Windows,
`$XDG_DATA_HOME/rustcraft-redefined` elsewhere, or `RUSTCRAFT_HOME` if set. The browser build
uses IndexedDB.
