# Rustcraft Redefined

An original, unofficial offline voxel sandbox. This is an **experimental prototype**, not the requested polished v1. The Windows executable has been cross-compiled and inspected, but has not been run or tested on Windows. It may contain serious bugs. It uses no accounts, official services, telemetry, network calls, or downloaded assets.

## What's here

The C++ prototype opens a resizable first-person window and implements a seeded heightmap world, chunk streaming, greedy face meshing, simple solid-colour blocks, water, movement, jumping, flight, a block-selection hotbar, a targeting outline, breaking and placing blocks, fog, and local block-edit persistence. Changes are saved under `%APPDATA%\Rustcraft Redefined\`; if APPDATA is unavailable, USERPROFILE is used instead. If neither location is available, the game runs without saving. The seed is fixed at 12345. See CONTROLS.md.

The client currently uses C++ as a fallback because the development environment lacked a Rust toolchain. The earlier Rust/C++ library and native Rust benchmark remain in the folder but are not used by the prototype executable. The executable uses the Windows Universal C Runtime API DLLs in addition to the graphics and other system DLLs. This does **not** meet the originally requested restricted import list, and it has not been verified on a plain Windows installation. No extra DLLs are included. A working graphics driver with legacy OpenGL support is required.

## Limitations and roadmap

The prototype has not been Windows play-tested. Terrain currently has no trees or caves; colours are flat rather than textured, and transparent surfaces are not depth sorted. There is no sky gradient, sun or moon, day/night cycle, selectable seed, proper step handling, multithreaded generation, frustum culling, or optimized modern renderer. Chunk generation is spread over frames but can still cause stalls. Saves contain block edits only. There is no crafting, inventory, mobs, sounds, advanced biomes, or lighting and circuit systems. These are future work, not promises of parity with any existing game. LAN multiplayer would require a change to the current strict no-network requirement.

See BUILD.md and CONTROLS.md. Licensed under the MIT license in LICENSE.
