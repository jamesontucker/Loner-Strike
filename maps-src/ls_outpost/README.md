# ls_outpost (source)

A walled desert outpost at night, built for Last Stand (ls_* maps start in Last Stand through
mappresets.ini). Version 2: rebuilt after a design review (docs/TOOLS-NOTES.md, "Map builder:
ls_outpost design review"). Our own layout; textures only from cs_dust.wad, cstrike.wad (CS 1.6)
and halflife.wad, Half-Life's `night` sky, stock ambience sounds. Players download nothing.

| File | What |
|---|---|
| `build_map.py` | the generator: open ground and the compound's measures at the top, then walls, firing step, keep, armory, barricades, props, entities, light, sound |
| `ls_outpost.map` | its output (Valve 220); opens in Tools > Map editor (Import) |
| `../../plugins/game/` | what ships (setup copies this tree into the game): `maps/ls_outpost.bsp/.nav/.lsv/.res`, `overviews/ls_outpost.bmp/.txt` |
| `layout.png` | top-down picture (the radar maker's render) |
| `test-tracks.png` | a sim run: red creatures, blue defenders, yellow a creature standing still 8 s+ with nobody near |

Rebuild: `python3 build_map.py`, then compile in the map editor (Compile tab, Normal). If the
geometry changes, teach the bots again (nav + .lsv), redo the overview (Radar maker) and .res.

## How it plays

- **The compound** (walls 160 high round a 1024 x 896 courtyard). 16 CT spawns in the middle.
  - **Three gates** (west, east, south), 128 x 96, each closed by a **boarded barricade**: an
    `ls_barricade` CLIP brush filling the opening (solid to everything that moves; gm_components lets
    bullets and sight through) and five `func_illusionary` boards named `<gate>_boards` (`boards=`) that
    drop off one by one as it weakens. hp 800, hold USE to repair board by board ($100). Creatures pile
    up outside; you shoot them through the boards.
  - **Firing step** (z 112) inside the west, south and east walls and over each gate (gatehouses),
    two stairs up from the courtyard. From it you see down each route over the parapet. Creatures
    can't get onto it from outside.
  - **The keep** (north wall): the last stand. One 128-wide door and two windows onto the courtyard.
  - **The armory** (NE, by the east gate): Barney's counter (`info_target` `lst_shop`), lockers, the
    only `func_buyzone`. Lit bright, caution stripes and a light strip over the door.
  - Generator and the radio mast with a red beacon (NW): a landmark from every route.
  - `lst_base`: an `ls_damage_zone` (off, invisible, no damage) sized to the courtyard, for the
    overrun rule.
- **West, the field**: open ground, long sightlines from the west gate and the firing step. A white
  floodlight mast, a burnt-out jeep, sandbags, crates. Two storm drains (NW, SW) round the corners.
  **Supply drops** land in the field just outside the west wall: climb down the ladder from the firing
  step (a gap in the parapet with caution stripes), grab it, climb back. Creatures can't climb.
- **East, the alley**: from the east gate's apron, two bends, red warning lamps, a crashed truck;
  a storm drain at the end.
- **South, the canyon road**: from the south gate's apron, two bends, a burning truck (orange light,
  fire sound); a storm drain at the end.
- One floor level outside (0) and one raised level inside (the firing step). Routes 160+ wide, 45-degree
  bevels in every turn, props 64+ from walls or flush, stairs (16 up), one ladder (only for defenders).

Components: 4 wave spawners (one per drain), 3 barricades, 3 supply drop spots, 4 loot crates (one per
route + the keep), 4 weapon spawners (courtyard x2, a sniper rifle on the firing step, a rifle in the field).
