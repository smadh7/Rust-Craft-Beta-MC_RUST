# Rustcraft Redefined

An original, unofficial offline voxel sandbox. This is an **experimental prototype**, not the requested polished v1. It uses no accounts, official services, telemetry, network calls, or downloaded assets. The Windows executable is cross-compiled; the latest changes have not yet been play-tested on Windows.

## What's here

The C++ prototype provides a startup menu, named worlds with selectable numeric seeds, four graphics presets, four simple first-person hand styles, a resizable first-person window, seeded terrain, sandy and grassy regions, trees, snowy peaks, water, chunk streaming, greedy face meshing, procedural block textures, movement, jumping, flight, a hotbar, targeting, breaking and placing blocks, and local block-edit persistence. The skin choices currently change only the visible first-person hand and sleeve; there is no full character model or custom skin import yet. See CONTROLS.md.

Choose Creative, Survival or Hardcore for a **new** world. These modes are early prototypes, not complete implementations. Use **Create world & play** with a unique world name and a whole-number seed, or select a world and click **Play selected world**. To choose another world, close the game and reopen it. Quick play is explicitly unsaved. World edits are saved separately under `%APPDATA%\Rustcraft Redefined\Worlds\`; graphics and skin choices are saved under the same user data folder. If APPDATA is unavailable or unwritable, USERPROFILE is tried instead. If no storage is available, only unsaved quick play is offered. Player position, mode, health, hunger, block counts, food, hotbar assignments and world edits are saved. World selection fixes the mode at creation; old worlds open in Creative.

When an older world is first saved by this version, it is upgraded to the new save format; a `.v1-backup` or `.v2-backup` copy is kept alongside it. Older executables cannot read the upgraded file. Back up important saves before trying experimental builds. If a valid old `world.bin` is present, it is copied to **Legacy World** the first time that entry is missing. The original file is not moved or modified. Keep a backup of important saves; malformed or incomplete saves are rejected instead of being overwritten.

The terrain generator changed when trees and climate regions were added. Existing saves keep their block edits, but unedited terrain may look different, and edits made against the old terrain can appear in unexpected places.

## Graphics presets

Choose F1–F4: Potato / Basic (2-chunk radius, flat sky), Normal (3, gradient and sun), High (4, gradient, sun and simple clouds), or Very High / Peak (5, a warm sunset palette, soft cloud shapes and animated water surfaces). All presets use original procedural textures, face shading and distance fog. Block breaks emit particles; Survival and Hardcore show progressive cracks while mining. Preset changes rebuild chunk visuals incrementally, so the colour change may take a moment. Larger view distances and Peak's water tessellation can reduce performance. Graphics and skin settings are remembered when storage is available.

Peak's water waves displace the rendered top surface only: fluid flow, collision and swimming still use static water blocks. Its glints and sunset glow are simple visual approximations. These remain fixed-function graphics improvements, **not** a modern shader pipeline: Peak does not yet match the supplied reference images. There are no actual reflections, dynamic shadows, post-process bloom, volumetric lighting or realistic water physics. Achieving that look requires substantial rendering work and Windows/GPU testing.

## Limitations and roadmap

The client currently uses C++ as a fallback because the development environment lacked a Rust toolchain. The earlier Rust/C++ library and native Rust benchmark remain in the folder but are not used by the prototype executable. The executable uses Windows Universal C Runtime API DLLs in addition to graphics and other system DLLs. This does **not** meet the originally requested restricted import list, and it has not been verified on a plain Windows installation. No extra DLLs are included. A working graphics driver with legacy OpenGL support is required.

Creative has unlimited hotbar blocks, instant breaking and flight. The early Survival/Hardcore modes have limited block counts, basic mining, one log-to-planks shortcut, berries, hunger, health, swimming, fall and drowning damage, and death handling. Survival respawns and loses inventory; a dead Hardcore world is retained but locked. These are simplified rules, not full game modes: there are no dropped items, hostile mobs, full inventory or crafting interface, tools, armor, difficulty settings, or spectator mode. The E-key block picker can assign the current hotbar slot; it is not a full inventory. Hardcore relies on successful local saves and is not tamper-proof. See CONTROLS.md.

Loaded chunks and saved edits are indexed, loaded neighbors are reused for meshing, and a conservative camera frustum skips off-screen chunks. Transparent chunks are sorted but individual faces are not. There are no caves, dimensions, day/night cycle, proper step handling, multithreaded generation, or optimized modern renderer. Chunk generation is spread over frames but can still cause stalls. There are no mobs, sounds, or advanced lighting and circuit systems. These are future work, not promises of parity with any existing game. LAN multiplayer would require a change to the strict no-network requirement.

See BUILD.md and CONTROLS.md. Licensed under the MIT license in LICENSE.

See RESEARCH.md for the scope comparison and outstanding work. No performance comparison with another game has been measured.
