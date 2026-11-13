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
| `layout.png` | the radar maker's render of the compiled map (`layout-v1.png`: before the organic pass) |
| `test-tracks.png` | where the bots went (grey) and died (red: shot, purple: the zone) in the last sim run |
| `navgen.py` | makes the bots' nav from the .bsp in ~40 s (the game's own learner takes hours on a map this big); with `--check br_valley.layers.json` it also runs the map's checks on the compiled hulls (below) |
| `br_valley.layers.json` | written by `build_map.py` for `navgen.py --check`: the connectors (every stair, ladder, ramp, walkway), the ground the layers are measured from, tree canopies, supply drops |
| `../../plugins/game/` | what ships (setup copies this tree into the game): `maps/br_valley.bsp/.nav/.lsv/.res`, `overviews/br_valley.bmp/.txt` |

Rebuild: `python3 build_map.py`, then compile in the map editor (Compile tab, Normal or Final), or the same through
the compile routes; then `python3 navgen.py <game>/cstrike/maps/br_valley.bsp <game>/cstrike/maps/br_valley.nav
--check br_valley.layers.json`, load the map once so the bots build their knowledge (.lsv, ~5-10 min in the
background), and redo the overview (Radar maker; it writes the .res too).

**The placement validator.** `build_map.py` stops, writing nothing, if (`ALLOW_OVERLAP=1` writes the map anyway):
- two things overlap (every brush is tagged with what it belongs to: a building, a bridge, stairs, a rock...);
- the ground rises into a structure (stairs through a bank; for stairs only each step's own tread counts);
- a stair, ladder, ramp or walkway doesn't meet a floor at its top or bottom with room to stand, or the floor
  doesn't go on past the end (straight on, or to the left or right): stairs into a wall, into the air, to nowhere;
- a prop (crate, car, container, hay, logs, sandbags, fence, cart, rock, tree, bush) floats (most of its footprint
  over air) or is buried (the ground up most of a box's side, or a rock/bush that barely shows);
- a drop-in spot or supply drop would land on something over it (a roof, a tower, a canopy);
- a drop-in spot has no gun on the ground within a 10-second run (it adds a pistol near any that has none).
It prints the loot per layer (crates, tiers, guns). `navgen.py --check` then checks the compiled map on the real
collision hulls: places you can't get back from without a fall over 200 (traps; every spot a drop-in can land on is
tried too), high or underground pockets with fewer than 2 ways out (each ladder is one way; walking or dropping ways
count apart when they go different ways or are 128+ apart), ladders not joined at both ends, connector ends off the
walk, a drop-in spot more than a 10-second walk from a gun, a supply drop with no ground beside it (bots open crates
from within 80). It prints the walkable area per layer.

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
- Five cabins at odd spots between the places; ~120 trees in groves of different sizes plus loose ones
  (irregular foliage from head height up, so they block sight; at least 40 of open ground between
  canopies), ~25 boulders in formations (tilted tops, some tall standing stones), ~40 bushes.
- **Upper deck**: plank walkways with railings link the rooftops and upper floors off the street: in
  the town house A's roof to the hall's 3rd floor, the hall's 2nd floor to the store's roof, and a
  catwalk over the main street to the roof across; at the factory the hall's roof to the office's roof.
- **River**: a channel 512-1024 wide, 160 deep with cliff banks, bending in an S down the valley (corners
  cut at 45 degrees, boulders against the banks); three bridges (north, middle, south road), two fords
  (ramps), stairs out of the bed by every bridge and three more places, and the tunnel under it.
- **Roads**: curves through the places that follow the ground (thin overlays the bots and players
  walk straight through, in 512-unit pieces so VIS still culls them): the main road through the
  town's main street to the middle bridge, the north road past the radio hill, the south road past the
  gas station, and dirt tracks between.
- **Landmarks**: West Peak (z ~550, between the fort and the town), the east ridge between the quarry and
  the bunker, the radio hill (448) and its tower, the two mesas.
- **Below** (z -512, 192 high, v3): a second level under almost the whole valley, 36 rooms in a 6 x 6 grid
  with solid ribs and two doorways each way between them: natural caves in the west and the north (under the
  hills: rock, mud, timber, yellow lamps), the quarry's workings, the army bunker in the south-east (blue lamps),
  utility basements under the factory and the gas station (orange), the town's cisterns in the middle (green).
  The older tunnels run through them: the town sewer, south to the gas station, west to the factory, the river
  tunnel to the bunker, the mine gallery from the farm under radio hill and the river to the lumber camp, the
  storm drain and the east drain. Ways up: 6 stair trenches and 11 ladder hatches, each under a lit portal.
- **High routes** (v3): the town's rooftop quarter (five walkways, a fire ladder, drop gaps), the factory's roofs
  and the fuel-line bridge down to the gas station's roof, the aqueduct from radio hill over the river, the barn's
  roof deck, the farmhouse roof, the silo and chimney perches (two ladders each), the radio tower (two ladders),
  the watchtower and sawmill roof, the bunker roof, the mesas (each with a second way down).

Mode data in the BSP (gm_components): 28 respawn points spread by farthest-point picking and the same
28 spots as drop-in landing spots (`ls_marker kind=br_drop`, all on the ground), 32 game spawns, 100 loot
crates and 49 floor weapons weighted by layer (the surface tiers 1-2, pistols and SMGs; high places 3-4 and the
sniper rifles; below 3-5 and rifles, 5 in three vaults), a gun within a 10-second run of every drop-in spot,
8 supply drop spots on open, dry ground (moved off the river, trenches and structures), the safe zone
(radius 5100, covers everything) and a six-stage shrinker (60/50, 50/45, 45/40, 40/30, 30/25, 25/25 s).
The zone entity sits at z 1536 on purpose: the shrinker picks random centres by testing open air 64
units above the zone's height, which would fail inside hills if it sat on the ground.

## Play it

After setup copies `plugins/game/` into the game: Play page → Battle Royale → map `br_valley` (or
`changelevel br_valley` on a server: `br_*` maps start in Battle Royale). 16-28 players is the sweet
spot (fill with bots). Squads: Game Mode Menu → Battle Royale → Squads 2 or 4.
