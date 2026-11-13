# br_valley (source)

A river valley by day, built for Battle Royale (br_* maps start in Battle Royale through
mappresets.ini). Our own layout; textures only from cs_dust.wad, cstrike.wad (CS 1.6) and
halflife.wad, Half-Life's `cliff` sky, stock ambience sounds. Nothing to download for players
beyond the map itself.

| File | What |
|---|---|
| `build_map.py` | the generator: tables at the top (pads, mesas, hills, roads, tunnels, trenches, hatches), then terrain, river, underground, buildings, cover, mode data, light |
| `br_valley.map` | its output (Valve 220); opens in Tools > Map editor (Import) |
| `preview.png` | the generator's own quick top-down (heights, roads, river, tunnels in purple, spawns white, crates yellow, floor weapons orange, drop spots blue) |
| `layout.png` | the radar maker's render of the compiled map |
| `test-tracks.png` | where the bots went (grey) and died (red: shot, purple: the zone) in the last sim run |
| `navgen.py` | makes the bots' nav from the .bsp in ~20 s (the game's own learner takes hours on a map this big); then the nav fixer's reach check |
| `../../plugins/game/` | what ships (setup copies this tree into the game): `maps/br_valley.bsp/.nav/.lsv/.res`, `overviews/br_valley.bmp/.txt` |

Rebuild: `python3 build_map.py`, then compile in the map editor (Compile tab, Normal or Final), or
the same through the compile routes. If the geometry changes: `python3 navgen.py <game>/cstrike/maps/br_valley.bsp
<game>/cstrike/maps/br_valley.nav`, check reach in the nav fixer, load the map once so the bots build their
knowledge (.lsv, ~10 min in the background), and redo the overview (Radar maker) and the .res (Map doctor).

## Size

GoldSrc sends every position as +/-4096 (see docs/TOOLS-NOTES.md "BR map"), so the whole map sits
inside that box: playable 7168 x 7168 (-3584..3584, about 182 m a side at an inch a unit), a ring of
tall cliffs behind an invisible wall to look at, tunnels at z -512, the sky at 3584 (room for a
drop-in). Straight across is 29 s at knife speed; walking it on the nav: corner to corner (farm to
bunker) 41 s, west to east on the main road 37 s, radio hill to the gas station 31 s (+50 % for the
river and cliffs). The size comes from routing, height (tunnels, roofs, towers, mesas), woods that
break sightlines, and the zone.

## Layout (north up; the river runs north-south east of the middle)

- **Town** (middle west, z 64): six buildings on two streets; the town hall has three floors and a
  roof (the highest roof in the valley, a sniper rifle on top), two more with roofs you can reach,
  cars and crates in the streets. A ladder hatch and a stair trench lead to the sewer under it.
- **Factory** (south west): a 830 x 700 hall with a catwalk, an office block with a roof, containers,
  a chimney with a ladder (sniper perch). Stair trench and a ladder hatch to the tunnels.
- **Farm** (north west): a barn with a hay loft, a two-floor farmhouse, a silo, fences.
- **Radio hill** (north middle, z 448): the high point; a lattice tower with a ladder to a platform
  (best crate on the map), a hut.
- **Docks** (west bank): a boathouse with a roof, three piers over the river, stairs down to the bed.
- **Gas station** (south middle): a canopy over the road, a shop with a roof, a stair trench down.
- **Quarry** (east, z -128, a pit): rock piles, a shed with a roof, a stair trench to the tunnel north
  of the bunker.
- **Bunker** (south east, z 160): thick concrete, two rooms, a roof with a parapet and stairs, sandbags;
  the bunker complex underneath (two halls, the best loot) through its stair trench.
- **Lumber camp** (north east): an open sawmill, log piles, a cabin, a watchtower with a ladder.
- **Mesas**: the old fort (west, z 352) and the lookout (east, z 320): cliffs all round, one ramp each.
- Five cabins between the places; woods in groves (~145 trees with foliage from head height up, so they
  block sight; ~35 rocks, ~40 bushes) between them.
- **River**: a 512-wide channel 160 deep with cliff banks; three bridges (north, middle, south road),
  two fords (ramps), stairs out of the bed by every bridge and at the docks.
- **Tunnels** (z -512, 192 wide): the town sewer, south to the gas station, west to the factory, and
  a long tunnel east under the river to the bunker complex and north to the quarry: a fourth way
  over the river, out of the zone's sight. Lit every ~380 units.

Mode data in the BSP (gm_components): 28 respawn points spread by farthest-point picking, 32 game
spawns, ~50 loot crates (tier 1 in the open and cabins, 2-3 in most places, 4-5 in the town hall,
bunker, radio tower and fort), ~27 floor weapons, 8 supply drop spots in open ground, the safe zone
(radius 5100, covers everything) and a six-stage shrinker (60/50, 50/45, 45/40, 40/30, 30/25, 25/25 s).
The zone entity sits at z 1536 on purpose: the shrinker picks random centres by testing open air 64
units above the zone's height, which would fail inside hills if it sat on the ground.

## Play it

After setup copies `plugins/game/` into the game: Play page → Battle Royale → map `br_valley` (or
`changelevel br_valley` on a server: `br_*` maps start in Battle Royale). 16-28 players is the sweet
spot (fill with bots). Squads: Game Mode Menu → Battle Royale → Squads 2 or 4.
