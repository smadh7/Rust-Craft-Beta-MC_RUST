# Research and scope notes

This is a clean-room feature comparison, not copied source or assets. The 26.2 release notes describe additional underground cave content, blocks and a reactive creature, along with rendering experiments and social features. A complete modern voxel sandbox also requires extensive inventories, recipes, tools, armor, structures, creatures and behavior, multiple dimensions, terrain and lighting systems, user interfaces, audio, accessibility, persistence and testing. This prototype does not provide those systems yet. The social and account features are intentionally excluded under this project's offline requirement.

## Mode behavior used as a design reference

- Creative emphasizes unrestricted building, quick block removal, flight and freedom from ordinary survival damage.
- Survival centers on resource gathering, health, hunger, tools, crafting and dangers.
- Hardcore is a one-life survival variant with higher difficulty and a way to view the world after death in the reference game. This project currently locks the world instead; it has no spectator mode or difficulty system.

## Work still needed

1. A full inventory (the current E-key picker only assigns blocks to the hotbar), item stacks, drops, item pickup, tools, durability, recipes, crafting interface and additional blocks.
2. Enemies, passive creatures, combat, difficulty, meaningful food sources, proper healing and environmental hazards.
3. Reliable death screens, spawn points, hardcore spectator behavior and robust crash recovery.
4. Caves, additional biomes and dimensions, structures, day/night, lighting and sound.
5. Modern rendering, profiling, predictable chunk scheduling, accessibility and actual Windows gameplay regression tests.
6. A controlled performance benchmark against comparable workloads; no claim of outperforming another game is currently supported.

Source-code licensing does not automatically grant rights to separately bundled game assets. No third-party game assets or source were imported; all current visuals are original procedural work. No online functionality is planned under the present requirements.
