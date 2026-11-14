#!/bin/sh
# Cross-compiles the Windows executable from Linux or macOS and copies it to the repository root.
# Needs the x86_64-pc-windows-gnu Rust target plus a MinGW-w64 or Zig toolchain; set
# CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER if the linker is not the default one.
# On Windows itself, simply run `cargo build --release`.
set -eu
cd "$(dirname "$0")/.."
cargo build --release --target x86_64-pc-windows-gnu "$@"
cp target/x86_64-pc-windows-gnu/release/RustcraftRedefined.exe ../RustcraftRedefined.exe
ls -l ../RustcraftRedefined.exe
