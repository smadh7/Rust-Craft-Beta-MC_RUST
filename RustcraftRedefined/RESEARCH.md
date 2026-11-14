# Target: Minecraft Java Edition 26.2

Notes on the version this project aims to match. Only publicly documented behaviour is used
(official changelogs and long-established community knowledge of game mechanics); no game code
or assets were consulted or copied.

## 26.2 "Chaos Cubed" (released 2026-06-16)

| Feature | Status here |
| --- | --- |
| Sulfur Caves biome | Generated: sulfur and cinnabar surfaces, spikes, springs. Placement rule is our own |
| Sulfur and Cinnabar block sets (base, polished, bricks; slab, stairs, wall; chiseled) | All blocks present; hardness values are estimates |
| Sulfur Spike (stalactites and stalagmites) | Generated and placeable with thickness states; no growth, and no falling when unsupported |
| Potent Sulfur, crafted from nine sulfur blocks | Present, with the recipe |
| Geysers: Potent Sulfur on magma or lava under 1-4 water sources | Implemented: random eruptions over magma, continuous over lava, upward push on player, mobs and items, spray particles and a hiss. No nausea gas |
| Sulfur Cube: passive, hops, 8 health (small: 4), spawns in Sulfur Caves | Implemented |
| Sulfur Cube absorbs a full block, loses its AI, ignores fall and player damage, can be punched and pushed | Implemented; also swallows block items lying nearby and follows players holding a block |
| Twelve archetypes (Regular, Bouncy, Slow Bouncy, Slow Flat, Fast Flat, Light, Fast Sliding, Slow Sliding, High Resistance, Sticky, Explosive, Hot) | Implemented. The published notes describe them in words (fast/slow, bouncy, friction, drag, buoyant); the numbers here are our estimates |
| Shears remove the block; bucket picks up a large cube; slime ball grows a small one; splits in two when killed | Implemented |
| Absorbed TNT: 6 s fuse when lit, 0.75-3 s when set off by an explosion, no small cubes afterwards | Implemented (lit with flint and steel; there is no redstone or fire) |
| Magma inside a cube hurts on contact | Implemented for the player |
| Sulfur Cube in minecarts, dispenser interactions | Not implemented (neither minecarts nor dispensers exist) |
| Music disc "Bounce", five new music tracks, new sounds | Not implemented: there is no music, and the only sounds are generic synthesised effects |
| Friends List | Out of scope: it is an online account service |
| Vulkan renderer option | Not implemented; the renderer is OpenGL 3.3 / WebGL 2 |
| Mob hitbox and eye-height tweaks, hostile Hoglins, bed bounce | Not implemented (those mobs and beds do not exist here) |

Sources for the 26.2 behaviour, all public write-ups of the official changelog:
<https://www.bisecthosting.com/blog/minecraft-26-2-chaos-cubed-update-release-date-patch-notes-more>,
<https://www.sportskeeda.com/minecraft/minecraft-java-26-2-update-patch-notes-sulfur-caves-sulfur-cube-cinnabar-blocks>,
<https://syntaxmine.com/articles/sulfur-cube-minecraft-26-2>,
<https://twistedvoxel.com/minecraft-java-edition-26-2-adds-sulfur-caves-sulfur-cube-and-friends-list/>.

## Mechanics already matched

- Movement: gravity 0.08 and drag 0.98 per tick, jump velocity 0.42, walk speed 0.1 with ground
  friction 0.6 x 0.91, sprint x1.3 with a 0.2 jump boost, sneaking x0.3, step height 0.6,
  player box 0.6 x 1.8 (1.5 sneaking), eye height 1.62 / 1.27. Unit tests check the resulting
  speeds and jump height.
- World dimensions: build height -64 to 320, sea level 63, deepslate below Y 0, bedrock floor.
- Light: levels 0-15 for sky and block light, attenuating by one per block.
- Day length 24000 ticks at 20 ticks per second.
- Block identifiers, block state properties and hardness values follow the published ones.
- Mining time: `speed / hardness / 30` per tick with the right tool, `/ 100` without, slowed
  five times in water and five times in the air; tool tiers gate drops.
- Furnace: 200 ticks per item, coal burns 1600 ticks.
- Hunger: exhaustion, saturation, regeneration above 18 food, starvation at 0.
- Fluids: water spreads 7 blocks and ticks every 5 game ticks, lava 3 blocks every 30;
  flow prefers the nearest drop within 4 blocks; two sources over solid ground make a third.
- Random ticks: three per section per tick; crops, saplings, grass, farmland, leaves.
- Explosions: 1352 rays, strength `power x (0.7..1.3)`, losing 0.225 per 0.3-block step plus
  `(resistance + 0.3) x 0.3` in blocks; TNT power 4 with an 80 tick fuse, creeper power 3.
- Mob walking speed: the square of the movement attribute times 43.17 blocks per second
  (zombie 2.3, pig 2.7, spider 3.9), and 10 ticks of invulnerability after a hit.

## Known differences

- Terrain is an original multi-noise design, so seeds do not reproduce Minecraft worlds.
- Biome selection is an approximation, not the vanilla parameter table.
- All textures are procedural and deliberately do not imitate the original pixel art.
