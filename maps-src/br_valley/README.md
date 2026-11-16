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
| `radar_marks.py` | run after the radar maker: a yellow arrow over every stair trench and hatch (the ways down), nothing outside the play square |
| `navgen.py` | makes the bots' nav from the .bsp in ~40 s (the game's own learner takes hours on a map this big); with `--check br_valley.layers.json` it also runs the map's checks on the compiled hulls (below) |
| `br_valley.layers.json` | written by `build_map.py` for `navgen.py --check`: the connectors (every stair, ladder, ramp, walkway), the ground the layers are measured from, tree canopies, supply drops |
| `../../plugins/game/` | what ships (setup copies this tree into the game): `maps/br_valley.bsp/.nav/.lsv/.res`, `overviews/br_valley.bmp/.txt` |

Rebuild: `python3 build_map.py`, then compile in the map editor (Compile tab, Normal or Final), or the same through
the compile routes; then `python3 navgen.py <game>/cstrike/maps/br_valley.bsp <game>/cstrike/maps/br_valley.nav
--check br_valley.layers.json`, load the map once so the bots build their knowledge (.lsv, ~5-10 min in the
background), and redo the overview (Radar maker; it writes the .res too), then `python3 radar_marks.py
<game>/cstrike/overviews/br_valley.bmp`.

**The placement validator.** `build_map.py` stops, writing nothing, if (`ALLOW_OVERLAP=1` writes the map anyway):
- two things overlap (every brush is tagged with what it belongs to: a building, a bridge, stairs, a rock...);
- the ground rises into a structure (stairs through a bank; for stairs only each step's own tread counts);
- a stair, ladder, ramp or walkway doesn't meet a floor at its top or bottom with room to stand, or the floor
  doesn't go on past the end (straight on, or to the left or right): stairs into a wall, into the air, to nowhere;
- a prop (crate, car, container, hay, logs, sandbags, fence, cart, rock, tree, bush) floats (most of its footprint
  over air) or is buried (the ground up most of a box's side, or a rock/bush that barely shows);
- a drop-in spot or supply drop would land on something over it (a roof, a tower, a canopy);
- a drop-in spot has no gun on the ground within a 10-second run (it adds a pistol near any that has none);
- (v4) rocks within 40 of each other, cover on steep ground, bushes under a canopy (each leaves a slot you can't climb out of);
- (v6) a light inside a brush (it would give no light): it moves each to the nearest spot 24 clear first, and stops if
  one is still inside.
It prints the loot per layer (crates, tiers, guns). `navgen.py --check` then checks the compiled map on the real
collision hulls: places you can't get back from without a fall over 200 (traps; every spot a drop-in can land on is
tried too), high or underground pockets with fewer than 2 ways out (each ladder is one way; walking or dropping ways
count apart when they go different ways or are 128+ apart), ladders not joined at both ends, connector ends off the
walk, a named place below with fewer than 2 ways out (v6: the vaults' escapes count), a drop-in spot more than a
10-second walk from a gun, a crate or gun below ground more than a 25-second walk
(stairs and ramps only, no ladders) from the nearest stair trench, a supply drop with no ground beside it (bots open
crates from within 80). It prints the walkable area per layer.

## Size

GoldSrc sends every position as +/-4096 (see docs/TOOLS-NOTES.md "BR map"), so the whole map sits
inside that box: playable 7168 x 7168 (-3584..3584, about 182 m a side at an inch a unit), a ring of
tall cliffs behind an invisible wall to look at, tunnels at z -512, the sky at 3584 (room for a
drop-in). Straight across is 29 s at knife speed; walking it on the nav: corner to corner (farm to
bunker) 41 s, west to east on the main road 37 s, radio hill to the gas station 31 s (+50 % for the
river and cliffs). The size comes from routing, height (tunnels, roofs, towers, mesas), woods that
break sightlines, and the zone.

## Layout (north up; the river runs north-south east of the middle; v4)

**The surface.** Ridges between the places (slopes up to 32 degrees), each crest with rock outcrops and a line of
pines, sunken lanes along three tracks (~110-130 deep: a crouched player is out of sight), foxholes and earth berms
in the open between places. Measured at eye height between points 1.5-3k apart, about 15-17 % of pairs see each other
(5-6 % over 3k), counting terrain, trees, rocks and buildings (v4 was already about 16 % / 6 % counted that way; the v5
terrain cut what the terrain alone shows, 58 % -> 52 %, and moved the true figure by about a point). A cliff ring of
layered rock (lower on the east and west, where the sky's own cliffs stand behind it), a crest with no beat, five crags
set into it (one a side, one by the river's gorge), a broken grassy terrace in front of it on three sides, notches
where the river comes in (a stepped waterfall, foam at its foot) and goes out, and where the main road leaves (a
boarded tunnel). Rock only on the steepest slopes; worn grass and dirt below that.
- **Town** (middle west, z 64): six buildings on two streets, a door at street level on each. The town hall has
  three floors and a roof (a sniper rifle, a fire ladder down the back). Cars, crates and sandbags in the streets.
  Ways down: the sewer trench in the south-east corner, brick kiosks over hatches in the main street, by house A,
  south of the town (the cisterns) and north of it (the storm drain).
- **Factory** (south west): an 830 x 700 hall (a catwalk with stairs and a ladder up through a roof hatch, a ladder
  outside), an office with a roof, containers, a chimney with ladders up two sides. The yard's stair trench goes
  down to the boiler house below.
- **Gas station** (south middle): a canopy over the road, a shop with a roof (the fuel-line bridge lands on it), a
  stair trench down to the tank vault, a tin shed over a hatch.
- **Farm** (north west): the barn (hay loft, a roof deck by a ladder from the loft or the hoist's ladder outside),
  the farmhouse (roof by its stairs, a ladder down the back), an eight-sided silo (a caged ladder on its east face,
  a switchback stair scaffold on its west, a railed top with a sniper rifle), fences, and the mine's adit: a stair
  trench under a 384-high timber head-frame.
- **Radio hill** (north middle, z 448): a lattice tower (a ladder on its east side; on the west a ladder to a caged
  landing half way, then another), a hut; the old rail viaduct leaves its east cliff.
- **Docks** (west bank): the boathouse (roof by its stairs, drop gaps), piers over the river, steps up from the bed,
  a shed over the hatch to the boathouse cellar.
- **Pumping station** (east of the river): a pump house over a hatch and a stair trench into the works below, a water
  tower, pipe runs to hide behind.
- **The east ridge trail** (v5): a plank catwalk along the east ridge's crest between the quarry and the bunker yard,
  a lookout hut with a mast and a red lamp half way (the south-east's landmark), stairs down at both ends.
- **Quarry** (east, z -128, a pit): rock piles, a shed with a roof, a stair trench down to the workings.
- **Bunker** (south east, z 64): two rooms, a roof (outside stairs, a ladder on the north wall), sandbags, a stair
  trench under a roofed housing down to the bunker below.
- **Lumber camp** (north east): the sawmill (roof at 176), log piles, a cabin, the open watchtower (a ladder, and
  stairs down to the sawmill's roof), a head-frame over the hatch into the lumber caves.
- **Mesas**: the old fort (west, z 352: its ramp on the east, the north gate's steps onto West Peak's flank) and the
  lookout (east, z 320: its ramp on the east, the viaduct's south leg arriving on its north edge).
- Five cabins between the places; ~70 tiered pines in groves (the lowest tier from head height up blocks sight;
  you bump into a box round each), boulders 40+ apart (no slots between them), bushes.
- **River**: a channel 512-1024 wide, 160 deep with cliff banks, water 28 deep in it (you wade, you don't swim: a
  splash gives you away), running-water sounds, bending in an S (corners cut at 45 degrees);
  three bridges, two fords, stone steps out of the bed by the bridges and in two more places, each flight climbing
  straight onto its bank.
- **Roads**: curves through the places that follow the ground (thin overlays): the main road through the town to
  the middle bridge, the north road past the radio hill, the south road past the gas station, dirt tracks between.

**High routes.** The town's rooftop quarter (five walkways, a fire ladder, drop gaps, a water tank and chimney
stacks to duck behind) and a walkway from the store's roof east to the middle bridge with stairs down at the bank;
the factory's roofs, the office roof and the fuel-line bridge down to the gas station's roof; the **old rail
viaduct**: from radio hill (z 448) east over the river and the north bridge (a branch of stone stairs down to the
east bank half way), then south along the lookout's flank down to the lookout mesa (z 320) - piers every ~320
with stepped corbels, a 48-thick deck with the old rails on it; the silo, chimney, radio tower, watchtower, barn,
farmhouse and bunker roofs, each with two ways off; (v6) a ladder up to each roof only a drop-in reached before (29:
sheds, kiosks, pump houses, cabins, the hut roofs; `ROOF_POINTS` in `build_map.py`, from navgen's list).

**Below** (z -512, some halls lower): named places, each with its own look, light colour and sound, joined by the
older tunnels; every one reachable on foot from a stair trench (the main ways down; the ladder hatches are extra),
within a 17-second walk of the nearest:
the fort caves (under the fort and West Peak), the west mine (the farm adit; timber props), the north caves, the
deep chamber under radio hill's flank and its stope a level down behind a grate (a vault), the lumber caves, the ridge caves
(caves: rock, mud, cut corners, rock pillars and hanging rock, a raised shelf; yellow lamps, drips); the town's
sewer (a water channel between ledges) and the cistern hall (256 high, columns, a water channel; green); the town
vault under the hall (a grate-floored passage you hear); the boiler house under the factory and the tank vault
under the gas station (boilers, tanks, pipes; orange); the boathouse cellar and the pumping works (pumps, pipes;
blue); the quarry workings (white); the bunker: a hall, the guard room, the generator room and the armoury a level
down (a vault, a grate passage; blue). Each vault (the town vault, the stope, the armoury) has one way in, a
grate-floored passage you hear, and (v6) one way out nobody comes in by: a ladder up the vault's wall to a crawl 96
over the next place's floor and a drop into it (the town vault into the storm drain, the stope into the deep chamber,
the armoury into the bunker hall). The boathouse cellar and the cistern hall have 64 of rock between them. Ways up: 8 stair trenches (the main ways down: town, gas station, factory yard into
the fort caves, bunker, quarry, the farm adit, the town's north edge into the storm drain, the pumping station) under
housings or the adit's head-frame, 12 hatches under kiosks, pump houses, sheds or boarded head-frames; daylight down
every shaft and trench, an EXIT sign at each foot and at both ends of every tunnel, a hip-high
band painted in the place's colour along the tunnels.

Mode data in the BSP (gm_components): 28 respawn points spread by farthest-point picking; drop-in landing spots
(`ls_marker kind=br_drop`, all on the ground): the same 28 plus four by the high layer's ways up; 32 game spawns;
78 crates and 40 floor weapons weighted by risk: the surface tiers 1-2 with tier 4 at six contested places (the
hall's ground floor, the factory floor, the gas station shop, the quarry shed, the sawmill, the boathouse), pistols
and SMGs; high places 3-4 and the sniper rifles; below tier 3 and SMGs, tier 5 and rifles only in the three vaults;
a gun within a 10-second run of every drop-in spot; 8 supply drop spots on open, dry ground; the safe zone
(radius 5100) and a six-stage shrinker (60/50, 50/45, 45/40, 40/30, 30/25, 25/25 s). The zone entity sits at z
1536 on purpose: the shrinker picks random centres by testing open air 64 units above the zone's height.

**Brush budget.** Stair steps are shown and solid to bullets but not to movement (`zhlt_noclip`), with one CLIP
ramp over each flight to walk on; tree foliage likewise, with one CLIP prism round each tree; CLIP helpers in the
detail entity (not the world: they'd split visleaves). Clipnodes 77 %.

## Play it

After setup copies `plugins/game/` into the game: Play page → Battle Royale → map `br_valley` (or
`changelevel br_valley` on a server: `br_*` maps start in Battle Royale). 16-28 players is the sweet
spot (fill with bots). Squads: Game Mode Menu → Battle Royale → Squads 2 or 4.

## Hand-edited source of truth (2026-11-15)

`br_valley.lsmap` is the hand-edited map (cleanup, hints, env_fog, MaxRange 8192, waterfall notch,
func_detail/road groups) and is now the source of truth. Do NOT regenerate from `build_map.py`:
it would wipe the hand edits. Open the .lsmap in the Loner-Strike editor, compile with Final.
