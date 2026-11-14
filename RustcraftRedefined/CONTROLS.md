# Controls

| Key | Action |
| --- | --- |
| Mouse | Look around (click the window to capture the cursor) |
| W A S D | Move |
| Space | Jump; swim up; double-tap to toggle flying in Creative |
| Left Shift | Sneak; descend while flying |
| Left Ctrl, or double-tap W | Sprint |
| Left mouse button | Mine the targeted block (hold); attack a mob; punch a Sulfur Cube ball |
| Right mouse button | Place a block, use an item or a block, eat (hold), interact with a mob |
| Middle mouse button | Pick the targeted block |
| 1-9, mouse wheel | Select a hotbar slot |
| E | Inventory (the item picker in Creative) |
| Q | Drop one item (Ctrl+Q: the whole stack) |
| T or Enter | Chat; `/` opens chat with a slash typed |
| Esc | Game menu; closes any open screen |
| F1 | Hide or show the interface |
| F2 | Save a screenshot |
| F3 | Debug overlay |
| F11 | Fullscreen (Windows) |

## Inventory screens

| Action | Effect |
| --- | --- |
| Left click | Pick up, put down or swap a stack |
| Right click | Pick up half a stack, or put down a single item |
| Shift + click | Move the stack to the other section (or into the furnace, chest, inventory) |
| 1-9 over a slot | Swap with that hotbar slot |
| Q over a slot | Drop one item (Ctrl+Q: all) |
| Click outside the window | Throw the carried stack (right click: one item) |
| Middle click (Creative) | Take a full stack |

## Using things

- Crafting table, furnace, chest: right-click. Sneak to place a block against them instead.
- Bucket: right-click water or lava to fill it, right-click again to pour. Right-click a cow for milk.
- Hoe: till grass or dirt. Shovel: make a path. Axe: strip a log. Bone meal: grow a crop.
- Seeds, carrots and potatoes are planted on farmland; water within four blocks keeps it wet.
- Flint and steel: light TNT.
- Shears: shear a sheep, or cut the block out of a Sulfur Cube.
- Sulfur Cube: right-click with a full block to feed it; punch the resulting ball. An empty
  bucket picks up a large cube; a slime ball grows a small one.

## Commands

Type them in chat with a leading slash. Tools can run them with `rustcraft.command("...")`
in the browser console (no slash).

| Command | Effect |
| --- | --- |
| `gamemode <survival\|creative\|spectator>` | Change game mode |
| `give <item> [count]` | Add items to the inventory |
| `clear` | Empty the inventory |
| `tp <x> <y> <z>` / `tp <x> <z>` | Teleport |
| `time set <day\|noon\|sunset\|night\|midnight\|sunrise\|ticks>` | Change the time of day |
| `time freeze` | Stop or resume the daylight cycle |
| `summon <pig\|cow\|sheep\|chicken\|zombie\|creeper\|spider\|sulfur_cube> [x y z]` | Spawn a mob |
| `kill` | Kill the player; `kill @e` removes every mob |
| `difficulty <peaceful\|normal>` | Turn monsters off or on |
| `setblock <x> <y> <z> <block[state]>` | Place a block, e.g. `oak_stairs[facing=east,half=top]` |
| `fill <x1> <y1> <z1> <x2> <y2> <z2> <block>` | Fill a box (up to 32768 blocks) |
| `spawnpoint` | Respawn here from now on |
| `seed` | Show the world seed |
| `save-all` | Save the world now |
| `help` | List the main commands |
| `newworld [seed] [survival]` | Developer shortcut: a throw-away world that is never saved |
| `rd <chunks>`, `fov <degrees>`, `fly`, `hud`, `debug`, `look <yaw> <pitch>`, `hotbar <1-9> <item>` | Testing aids |
