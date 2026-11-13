# ls_outpost (source)

A desert relay station at night, built for Last Stand (ls_* maps start in Last Stand through
mappresets.ini). Our own layout; textures only from cs_dust.wad, cstrike.wad (CS 1.6) and
halflife.wad, the valve `night` sky, stock ambience sounds. Nothing to download for players
beyond the map itself.

| File | What |
|---|---|
| `build_map.py` | the generator: layout tables at the top (open ground, raised floors, wall zones), then props, components, lights, sounds |
| `ls_outpost.map` | its output (Valve 220); opens in Tools > Map editor (Import) |
| `../../plugins/game/` | what ships (setup copies this tree into the game): `maps/ls_outpost.bsp/.nav/.lsv/.res`, `overviews/ls_outpost.bmp/.txt` |
| `layout.png` | top-down picture (the radar maker's render) |

Rebuild: `python3 build_map.py`, then compile in the map editor (Compile tab, Normal), or the same
through the compile routes. If the geometry changes, teach the bots again (nav + .lsv) and redo
the overview (Radar maker) and the .res (Map doctor).

## Layout

- **Base** (the courtyard, -448..448 square, walls 128 with invisible clip above, so you shoot
  over them but nobody climbs out): 16 CT spawns in the middle.
  - **Station** (NW): the fallback room, one 128-wide door; its roof (z 160, stairs on its south
    side) is the overwatch over the yard and the drop clearing.
  - **Armory** (SW): the only `func_buyzone`, two doorways (north, east), lockers, caution
    stripes and light strips; an `info_target` named `lst_shop` marks it for gm_laststand.
  - **Gun platform** (E, z 96, stairs) over the east gate; **rampart** (SE, z 128, stairs down
    west) over the wall-top gate.
- **West, the yard**: 1344 x 1280 open ground, long sightlines, staggered crates, sandbags, a
  broken wall and a water tank on legs. Two spawn pockets round corners (NW, SW).
- **East, the alley**: 128 wide, five turns, a roofed tunnel stretch; spawn room at the far end.
- **South, the walkway**: from the spawn yard up stairs to z 128, over the rooftops with two
  bends, into the base on the wall top. Low parapets (clip above).
- **North, the drop clearing**: open sky, two supply drop spots, seen through the north gate and
  from the station roof. No spawns there.

Chokes and barricades (`ls_barricade` brushes, wall to wall, hp 1000, repair 3 s / $100): west gate
192 wide, east gate 128, wall gate 192. Wave spawners: Yard north, Yard south, Alley, Walkway.
Supply drops: 2 in the clearing, 1 in the courtyard. Loot crates: one per lane + clearing + yard.
Weapon spawners: 3 in the base (gear, pistols, a sniper on the gun platform), 1 rifle in the yard.

Corridors are 128+ wide, floors flat, stairs (16 up, 32+ deep) instead of ramps, no ladders.
