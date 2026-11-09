# Rustcraft Redefined

An original, unofficial offline voxel sandbox. This is an **experimental prototype**, not the requested polished v1. It uses no accounts, official services, telemetry, network calls, or downloaded assets. The Windows executable is cross-compiled; the latest changes have not yet been play-tested on Windows.

## What's here

The C++ prototype provides a startup menu, named worlds with selectable numeric seeds, four graphics presets, four simple first-person hand styles, a resizable first-person window, seeded terrain, sandy and grassy regions, trees, snowy peaks, water, chunk streaming, greedy face meshing, procedural block textures, movement, jumping, flight, a hotbar, targeting, breaking and placing blocks, and local block-edit persistence. The skin choices currently change only the visible first-person hand and sleeve; there is no full character model or custom skin import yet. See CONTROLS.md.

Use **Create world & play** with a unique world name and a whole-number seed, or select a world and click **Play selected world**. To choose another world, close the game and reopen it. Quick play is explicitly unsaved. World edits are saved separately under `%APPDATA%\Rustcraft Redefined\Worlds\`; graphics and skin choices are saved under the same user data folder. If APPDATA is unavailable or unwritable, USERPROFILE is tried instead. If no storage is available, only unsaved quick play is offered. Player position is not saved.

If a valid old `world.bin` is present, it is copied to **Legacy World** the first time that entry is missing. The original file is not moved or modified. Keep a backup of important saves; malformed or incomplete saves are rejected instead of being overwritten.

The terrain generator changed when trees and climate regions were added. Existing saves keep their block edits, but unedited terrain may look different, and edits made against the old terrain can appear in unexpected places.

## Graphics presets

Choose F1–F4: Potato / Basic (2-chunk radius, flat sky), Normal (3, gradient and sun), High (4, gradient, sun and simple clouds), or Very High / Peak (5, gradient, sun and simple clouds). All presets use original procedural textures, face shading and distance fog. Larger view distances can reduce performance. Graphics and skin settings are remembered when storage is available.

These are modest fixed-function graphics improvements, **not** a modern shader pipeline: Peak does not yet resemble the supplied reference images. There are no reflections, dynamic shadows, bloom, realistic water, volumetric lighting or advanced post-processing. Achieving that look requires substantial rendering work and Windows/GPU testing.

## Limitations and roadmap

The client currently uses C++ as a fallback because the development environment lacked a Rust toolchain. The earlier Rust/C++ library and native Rust benchmark remain in the folder but are not used by the prototype executable. The executable uses Windows Universal C Runtime API DLLs in addition to graphics and other system DLLs. This does **not** meet the originally requested restricted import list, and it has not been verified on a plain Windows installation. No extra DLLs are included. A working graphics driver with legacy OpenGL support is required.

Transparent chunks are sorted but individual faces are not. There are no caves, dimensions, day/night cycle, selectable seed, proper step handling, multithreaded generation, frustum culling, or optimized modern renderer. Chunk generation is spread over frames but can still cause stalls. Saves contain block edits only. There is no crafting, inventory, mobs, sounds, or advanced lighting and circuit systems. These are future work, not promises of parity with any existing game. LAN multiplayer would require a change to the strict no-network requirement.

See BUILD.md and CONTROLS.md. Licensed under the MIT license in LICENSE.
