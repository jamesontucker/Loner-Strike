# BRmap notes (second map builder: br_valley, a map made for Battle Royale)
The first part of these notes (limits research, first compile) is also in ../TOOLS-NOTES.md ("BR map: br_valley").

## 2026-11-12 — br_valley

### Size: what GoldSrc allows (research)
Everything networked is +/-4096, so that is the box, in z too:
- Engine (our ReHLDS checkout `~/Nextcloud/.../Other/ReHLDS`, `engine/common.cpp`): `MSG_WriteCoord` = `short(f*8)`,
  `MSG_ReadCoord` = `short/8` → +/-4096 at 1/8 unit. Every temp entity (beams, the zone ring, sprites, decals), sound
  origin and AMXX `write_coord` goes through it.
- `cstrike/delta.lst`: `entity_state_t` origin DT_SIGNED 16 bits / 8 → +/-4095.9; `entity_state_player_t` 18 bits / 32
  → +/-4096; `clientdata_t` 21 bits / 128 → +/-8192 (your own prediction only). Other players, crates and items past
  4096 wrap for everyone else.
- SDHLT v1.3.1 (strings in the shipped exes): geometry +/-32768 (`-worldextent`); not the limit.
- Client: far clip (`MaxRange`) defaults to 4096: a big map needs worldspawn `MaxRange` (br_valley: 12000).
- So: playable -3584..3584 = **7168 x 7168 (~182 m a side, ~2.4x de_dust2's width)**, a 256-wide ring of tall cliffs
  behind an invisible clip wall (no box feel), tunnels at z -512, the sky at **z 3584** (open over everything, room for
  a drop-in from ~3000 up). Measured on the nav (shortest walk, 250 u/s): farm → bunker (corner to corner) 10 140 u /
  **41 s**, factory → lumber camp 35 s, west edge → east edge on the main road 37 s, radio hill → gas station 31 s (+50 %
  over the straight line: river, cliffs, mesas). Straight across 29 s. 1-2 minutes of crossing isn't possible inside
  +/-4096; the map gets its size from routing, height (tunnels, rooftops, towers, mesas) and the zone instead.
- Volume: surface 7168² (~51 M u²) + tunnels ~6 900 u of 192-wide corridor + two bunker halls + ~25 roofs/floors
  above ground (town hall 3 floors + roof, 7 roofs with stairs, 3 towers by ladder, the mesas at 320-352).

### What (maps-src/br_valley/)
- `build_map.py` → `br_valley.map` (Valve 220; Tools > Map editor > Import). Tables at the top: pads, mesas, hills,
  roads, tunnels, stair trenches, hatches. Reused ls_outpost's writer (`box`, `stairs`, grid-merge fill, func_detail
  props); added convex `prism` brushes for sloped terrain, a `building()` helper (doors 128, windows, switch-back
  stairs, roof access with a parapet), `ladder()` (func_wall `{CSTRIKE_LE6LAD` + func_ladder, with a backing board),
  stair trenches and ladder hatches into the tunnels, woods (trees whose foliage starts at head height, rocks, bushes)
  from a noise function, a farthest-point spread for spawns and field crates.
- `navgen.py`: **the bot nav without the game's learner** (see "Nav" below).
- `README.md` (the layout), `preview.png` (generator top-down: spawns white, crates yellow, weapons orange, drops blue,
  tunnels purple), `layout.png` (the radar maker's render), `test-tracks.png` (where bots went and died in the sim).
- Ships in `plugins/game/` like ls_outpost: `maps/br_valley.bsp/.nav/.lsv/.res`, `overviews/br_valley.bmp/.txt`
  (~9.6 MB: bsp 4.2, lsv 4.3, overview 0.8, nav 0.3). The compile also installed bsp/nav/res/overview into this Mac's
  game folder (as Playtest does).

Places: town (6 buildings, the 3-floor town hall, sewer below), factory (hall with catwalk and a roof ladder, office,
chimney ladder), farm (barn with loft, farmhouse, silo), radio hill (z 448, tower by ladder, best crate), docks (boathouse,
piers, stairs into the bed), gas station, quarry pit, bunker (roof, two rooms, a bunker complex below), lumber camp
(watchtower ladder), two mesas (fort 352, lookout 320; cliffs, one ramp each), 5 cabins. River 512 wide, 160 deep with
cliff banks: 3 bridges, 2 fords, stairs out by every bridge; plus the tunnel under it. Mode data in the BSP: 28 respawn
points, 32 game spawns, 50 loot crates (tier 1 open/cabins … 5 in the bunker complex, radio tower, fort), 27 floor
weapons, 8 drop spots, `ls_safe_zone` r 5100 at z 1536 + 6-stage shrinker (3400/2200/1300/700/300/0). Day: `cliff`
sky, sun from the south-east at 52°, warm interiors lights, tunnels lit every ~380.

### Compile (Final through Q's pipeline, low priority)
No leak, 1 VIS warning ("Leaf portals saw into leaf", a sliver portal at the backdrop cliff's foot near -3606 1152;
harmless). models 14, planes 30 %, worldfaces 40 %, clipnodes 59 %, worldleaves 62 %, AllocBlock 67 %, lightdata 1.9 MB,
visdata 0.7 MB, 4.2 MB .bsp, 71 textures from cs_dust.wad / cstrike.wad / halflife.wad. Open terrain: VIS can't hide
much (735 of 1336 leaves visible on average); hills, woods, buildings and the river cliffs break sightlines.
One compile hung in sdHLVIS (0 % CPU at 22 %) while the Mac was loaded; Stop then stayed on "Stopping…" (the job waits
on wine); restarting the dev server cleared it (for Q).

### Nav: `navgen.py` (the game's learner takes hours here)
- The brain's learner never finished on br_valley (2 tries, 60 and 15+ min, then stopped): `LearnStep` does ~13
  `GetGroundHeight` calls per sample, each up to 10 point traces of 9999 units, and `CNavNode::GetNode` is a linear list
  search (O(n²) over ~80 000 samples); then the hiding-spot analysis is O(n²) traces too (`bot_nav_analyze` on our nav
  ran 40+ min without finishing).
- `navgen.py` walks the same 25-unit grid with the player's own hull (hull 1 from the .bsp + solid brush entities), in
  four sub-moves per sample with step-up 18 (stairs), drops up to 200 one way, slope normal z >= 0.7, seeded from every
  spawn, respawn point, crate, weapon and drop spot; samples → rectangles (linked both ways, flat within 8) → nav v5
  areas with links. ~80 000 samples, 3.6 M traces, **20 s**, 3 643 areas. No hiding/approach/encounter spots (the brain's
  .lsv is built on first load as usual; `bot_nav_analyze` can add them if wanted). Ladders are built by the game at load.
- Nav fixer reach check: **all 3 643 areas reachable, 0 traps, 6 ladders all linked top and bottom.** It took fixes in
  the map: stacked stair flights (switch-backs now), crates/spawns on rocks and props, a factory roof you could drop on
  but not leave (roof ladder), bunker roof landing, ladder tops 8 above their platforms with backing boards.

### Tested (Sim/BRMAP, 20 bots, Battle Royale solo, `br_dropin 0`, brain r43348308295, gm_components +
gm_battleroyale from HEAD 22:30, gm_simrec; port 27368)
- Final run (the shipped bsp/nav, 25 min): **5 complete games of 3.4, 5.9, 4.8, 3.9 and 4.0 min**, 19-20 kills each.
  Bots covered 519 of the 784 surface cells (66 %) plus the tunnels; fights in the town, at the bridges and in the
  river bed, the lookout and fort mesas, the factory, the docks (`test-tracks.png`: grey = where bots went, red = shot,
  purple = the zone). The brain built its knowledge (.lsv, 4.3 MB) in ~5 min on first load. No bot stuck on geometry:
  every "idle 12 s+" spot was a respawn point with a bot in the Buy state (below). An earlier run on the same layout:
  5 games of 4.1-5.6 min.
- Game length: shorter than the 6-12 min target with 20 bots. Half the deaths were bots that never left their spawn
  (below); the rest of the bots hunt by noise and meet within ~4 min on a map this size. With people (slower, more
  careful) and the Buy fix it should run longer; the mode's knobs (`br_drop`, `br_zone_start`, fewer hunters) are W1's.
- **Not the map:** 26-28 % of all bot samples were in the Buy state, standing on their respawn point with a knife for the
  whole game; those bots died to the zone (49 of 108 deaths in the final run). Request for W1/W2 in docs/notes/W1.md. With the drop-in on,
  16 of 20 died in the first 25 s (hit mid-air → normal gravity → 3000-unit fall): request in W1.md; tests use
  `br_dropin 0`.

### Requests
- **(W1)** in docs/notes/W1.md "Requests from BRmap": drop-in fall deaths; bots stuck in Buy in BR; `random_center` air
  test vs terrain (br_valley puts its zone at z 1536 to work round it).
- **(W2, brain)** the learner on big maps: a hash for `CNavNode::GetNode`, one ground trace per sample instead of ten
  9999-long ones, would make learning such maps minutes instead of hours; or ship `navgen.py`'s approach in the app.
- **(coordinator)** br_valley ships in `plugins/game/` as ls_outpost does (setup copies it); mappresets `br_*` → br
  already. The nav server (`navs/list.json`) could carry br_valley.nav/.lsv too. My test server Sim/BRMAP is removed.
- **Port clash:** other agents' servers (W2LS2 27360, W2LS3 27363, W2OPA 27365, W2OPB 27366) were on my 27360-27369
  range; I used 27368.

## 2026-11-13 — br_valley v2: organic pass, placement checks, upper deck

### What changed (`maps-src/br_valley/build_map.py`; before: `layout-v1.png`, after: `layout.png`)
- **River**: an S down the valley, 2-4 cells wide, corners cut at 45°, boulders against the banks; straight only where
  it's crossed (3 bridges, 2 fords, bed stairs, the docks' piers); the tunnel under it kept. Rows 23+ now start a cell
  further east: the radio hill's pad met the river at row 25 and the north bridge's west end sat in a 448-high slope.
- **Roads**: curves through the places, thin non-solid overlays cut to the terrain's triangles (func_illusionary in
  512 tiles, so VIS still culls them). Solid road pieces had pushed clipnodes over the limit.
- **Height**: West Peak (~550) and a ridge on the east bank. Mesa ramps now follow the hillside across their width
  (three planar pieces, each foot point 4 below its own ground) instead of one flat foot that sank into one side.
- **Cover**: trees in groves of different sizes plus loose ones, boulder formations with tilted tops, bushes; all
  irregular. Canopies keep 40 units of open ground between them and bushes 40 from trees and rocks; bank boulders
  stay clear of the 45° corners (each of these left a slot a player could drop into and not leave). Cabins at
  irregular spots; grass follows the slope.
- **Upper deck** (`walkway()`; `building()` takes `updoors` and `roof_gaps`): town: house A's roof → the hall's 3rd
  floor, the hall's 2nd floor → the store's roof, a catwalk over the main street → the roof across; factory: the
  hall's roof → the office's roof (a 16 step up). All four are in the nav and linked.
- **Placement checks** (the generator stops, writing nothing; `ALLOW_OVERLAP=1` overrides):
  1. overlaps: every brush is tagged with what it belongs to (a building, a bridge, stairs, a rock...); two groups
     may not overlap (exact convex footprints), only touch. Cover is only placed where nothing else stands. It found:
     the row-11 ford through the docks' piers and stairs (ford moved to row 13), the lookout ramp into the quarry
     shed (it now goes east), the bunker trench's railings in the bunker's wall, a car in the town hall's wall,
     riverside boulders on piers, 45° river corners cutting piers.
  2. ground: a structure the ground rises into (above its top somewhere while its top shows elsewhere: stairs
     through a bank, a hillside through a floor). It found the north bridge's west end (above), the gas trench's
     east end past its pad (trench rims are now fixed at the pad's height) and both mesa ramps' feet.

### Compile (Final through the compile routes on my dev server, niced, `-low`, threads all but two)
95-139 s, no leak. planes 44 %, worldfaces 40 %, clipnodes 75 %, worldleaves 66 %, AllocBlock 64 %, lightdata 1.8 MB,
visdata 0.7 MB, 4.4 MB .bsp. Warnings: 3-4 road overlay entities' boxes touch more than 48 leaves (the engine uses
a simpler vis check for them; harmless).

### Nav
`navgen.py`: 78 614 samples, 21 s, 4 792 areas. Reach check: **all reachable, 0 traps, 6 ladders linked** (it had
found the two drop-in slots above: between canopies on a slope, and between a bank boulder and a 45° corner).


## Rule from Y (2026-11-13): touch only your own server processes
- Never attach a debugger (`winedbg <pid>`, `attach`, DebugBreakProcess, anything that puts a thread into another
  process) to a running hlds under CrossOver: the injected thread starts at 0xfff4fadc and kills the server.
- Stop, signal or inspect only the processes your own script started: by the PID you got when you started it, or by a
  pattern that names your own sim folder (`Sim[\\/]<yours>[\\/]hlds.exe`). Never by name (`hlds.exe`, `winedbg`) and
  never by "the newest process". Before a start, make sure your port is free (`lsof -nP -iUDP:<port>`).
- Stacks and crashes without a debugger: `drive_c/Loner-Strike/Sim/crashes.log` (every crash in the bottle, full
  winedbg report), and with fork builds from ls-creatures `f351765` on, `cstrike/ls_crash*.log` and
  `cstrike/ls_watchdog.log` in the server folder.

### Tested (Sim/BRMAP, port 27368, 20 bots, Battle Royale solo, gm_battleroyale + gm_components built from HEAD
1c94661 = W1's drop-in and buy-state fixes, so **`br_dropin 1`**; 25 min; `test-tracks.png`)
- Game 1: **6.6 min, 19 kills** (town, open ground, lumber camp, docks, radio hill; the last 4 to the zone). Game 2:
  down to 3 players in 3.2 min, then the server stopped (below). Bots covered 580 of the ~784 surface cells (74 %,
  v1: 66 %) plus the tunnels.
- **Drop-in: no fall deaths on landing** (v1: 16 of 20 dead in 25 s). But many knife kills in the first 1-12 s after
  the loot-up (8 in game 2): bots land next to each other. W1's.
- No bot stuck on geometry: the "idle 12 s+" spots are campers holding a spot (6-9 campers a game).
- One fall: a camper on the radio tower stood on the 40-high railing and walked off the west side (440 down, 58 hp).
  Once; left as is (a 64 railing would block the lookout's view).
- **Server stalls, not the map:** both runs stopped mid-game with the process alive. The first (00:15, `br_dropin 0`,
  pre-fix plugins) after 2.5 min. The second at 00:51:35, the moment QA's QA5/QA7/QA8 servers started; LSPLAY and the
  DR servers were restarted around 01:01 too, and W1 saw the same at 23:50. Before that the server ran at 0.86-1.0x
  real time on a loaded Mac (load ~17-23).
- gm_components had warned "a component at 2560 -1600 256 is inside a wall": a rock formation stood on that supply-drop
  spot. Cover now keeps 160 clear of every drop spot (skipped at placement, so the rest of the layout didn't move).
  The shipped build has this fix; the 25-min test ran on the build just before it (only cover at drop spots differs).
  The shipped .lsv was built by loading the shipped bsp/nav.

### Ships (plugins/game/)
`maps/br_valley.bsp/.nav/.lsv/.res`, `overviews/br_valley.bmp/.txt` from this build; this Mac's game folder has them too.
`maps-src/br_valley/layout-v1.png` is v1 for comparison. My test server Sim/BRMAP and my dev server (8077) are
stopped and removed.


## 2026-11-13 — br_valley v3: three layers (the user's feedback on v2)

### The plan
The valley can't get wider than +/-4096, so it gets its room from height and depth: a second level under almost
all of it, and high routes over the places, so that the three layers together give much more to explore than a flat
map in the same footprint. Every high or low place has at least two ways out; every way down to the surface is a
stair, a ladder or a drop of at most 176 (a fall of 200 costs 14 hp; nothing designed is over 176).

Top-down, north up, one character per 256 units (the playable 7168 square). Left: the surface and the high routes;
right: the level below.

```
 surface + high                  below (z -512, 192 high)
................~~~.........                                
.ffffff.........~~~.cc......     cccccccc cccccccc cccccccc 
.ffffff.v...RR..~~~....LLLL.     cccccccE cccccccc cccccccc 
.fffvff.....RR..~~~....LLLL.     cccEcccc cccccccc cccccccc 
.fffvff...........~~...LLLL.     cccEcccc cccccccc cccccccc 
.fffvff..........v~~v..LvLL.     cccE============E==E===Ecc 
.fffvff.cc.......~~.........     cccEcccc cccccccc cccccccc 
........cc.......~~.........     cccccccc c=cccccc ccccccc= 
............v.....~~..MM....     cccccccc c=Eccccc ccccccc= 
..cc...TTTTTTTT...~~..MM....               =              = 
..cc...TTTTTTTT..~~.........     ccccssss s=ssssss sssswww= 
.......TTTTTTTT..~~.........     ccccssss s=ssssss sssswww= 
.......vTTTTvTv..~~~.v.QQQQ.     ccccssE====E=Esss ssEswww= 
.......TTTTTTTv..~~~...QQvQ.     ccccssss =sssEsss sssswwE= 
.......TTTTTTTv..~~~...QQvQ.     ccccssss =sssEsss sssswwE= 
.......TTTTTTTv..~~~.v.QQvQ.     ccccssss =sssEsss ssEswwE= 
.MM..............~~~........     ccccssss =sssssss ssssww=w 
.MM..........DDD.~~~..c.....     ccccssss =sssssss ssssww=w 
.MM...cc.....DDD..~~..c.....              =              =  
......cc.....DDD.~~.........     ccccssss =sssssss ssssbb=b 
.FvFFFF..........~~.........     cEccssss =sssssss ssssbb=b 
.FvFFvF..GGG....~~..........     cE==E===================bb 
.FvFFFF..GGG....~~..........     cEccssss=ssssssss ssssb=bb 
.FFFFFF..vvvv..~~......vBBB.     c=ccuuuuEEEEuuuuu uuuuE=bb 
.FFFFFF..GGG...~~....v.vBBB.     c=ccuuuu uuuuuuuu uuEuE==b 
.FFFFFF..GGG...~~......vBBB.     c=ccuuuu uuuuuuuu uuuuE==b 
.FFFFFF........~~~.....vBBB.     ccccuuuu uuuuuuuu uuuuEbbb 
................~~~~........                                
```
Left: `T` town, `F` factory, `f` farm, `R` radio hill, `G` gas station, `B` bunker, `L` lumber camp, `Q` quarry,
`D` docks, `M` mesas (the fort, the lookout), `c` cabins, `~` river, `^` the high crossings (the aqueduct in the
north, the fuel-line bridge in the south), `v` the ways down (stair trenches and ladder hatches). The rooftops, towers
and catwalks are inside the places (see the list below).
Right: rooms by what they are: `c` natural caves and the old mine (the west and the north, under the hills),
`w` the quarry's workings, `b` the army bunker (south-east), `u` utility basements (under the factory and the gas
station), `s` the town's cisterns (the middle); `=` the older tunnels running through them (sewer, storm drains, the
mine gallery, the river tunnels), `E` the ways up. Between rooms: solid ribs with two doorways each way.

**Connections between the layers**
- Surface to below: 6 stair trenches (town, gas station, factory, farm/mine adit, bunker, quarry) and 11 ladder
  hatches; every one has a portal (posts and a lintel, timber for the mine and caves, steel elsewhere) with a lamp
  under it, and a light at its foot, so a way down reads as one from across the field.
- Surface to high: building stairs (every reachable roof), outside ladders (the hall's fire ladder, the farmhouse,
  the barn's hay hoist, the bunker, the factory hall), tower ladders (radio tower and silo two each, chimney two, the
  watchtower one plus stairs), the mesas' ramps and second stairs, the aqueduct's stone stairs, and drops of at most
  176 off every low roof through a gap in its parapet.
- High to high: the town's walkways (house A's roof to the hall's 3rd floor, the hall's 2nd floor to the store's roof,
  over the main street, house D's roof to house A's 2nd floor, house E to house F), the factory hall's roof to the
  office roof, the office roof down the fuel-line bridge to the gas station's roof, the watchtower to the sawmill roof.

**Loot by layer** (riskier is better): the surface (open, many ways to run) tiers 1-2 and pistols/SMGs; high places
(you're seen, the ways off are few and slow) tiers 3-4 and the sniper rifles; below (dark, close, the zone can catch
you far from a way up) tiers 3-5 and rifles, the best in three vaults (under the town hall, the deep chamber under
radio hill, the bunker's core). Every drop-in spot has a gun on the ground within a short run (see the checks).

### Why each thing is there (what it's for, in and out, loot)
- **Town** (the middle west). For: close fights in streets and a rooftop quarter over them. In/out on the ground: six
  buildings, every one with a door at street level. Up: each reachable roof has its stairs inside, the hall a fire
  ladder from the street behind it; the low roofs (house D, F, the store) have gaps in their parapets to drop 160 to
  the street. Across: five walkways (see above) so the roofs are one place with 15 ways down. Down: the sewer stair
  trench in the south-east corner, hatches by house A and in the main street. Loot: street level tier 2; upper floors
  and roofs 3-4, a sniper on the hall's roof; the town vault (tier 5) is in the room under the hall. Cars, crates and
  sandbags in the streets: cover across the main street.
- **Factory** (south-west). For: a big hall to fight through, its roof as a perch over the south. The hall has three
  ground doors; inside, the catwalk's stairs and, at its far end, a ladder up through the roof hatch (the catwalk is a
  way to the roof, not a dead end); outside, a ladder on the north wall. The roof joins the office roof by a plank
  walkway; the office has stairs; from the office roof the fuel-line bridge runs down to the gas station's roof. The
  chimney (ladders up two sides) is the sniper perch. Down: the yard's stair trench and a hatch. Containers: yard cover.
- **Gas station** (south middle): the canopy and pumps are cover on the road; the shop's roof is where the fuel-line
  bridge lands (stairs inside, a drop gap on the south). Down: the long stair trench behind it.
- **Farm** (north-west). The barn: hay bales for cover, the loft by its stairs, the roof deck by a ladder up through a
  hatch from the loft or the hay hoist's ladder outside. The farmhouse: stairs to the roof, a ladder down the back.
  The silo: ladders up both sides to a railed top with a sniper rifle. The mine's adit (a stair trench, timber portal)
  in the yard leads to the west caves and the mine gallery.
- **Radio hill** (north middle, z 448): the highest ground; the tower (ladders on two sides, tier 5 crate) and the
  hut. The aqueduct leaves the hill's east cliff for the far bank (two ways off it: the hill and its stone stairs).
- **Docks** (west bank): piers with crates over the river (cover along the water), the boathouse (roof by its stairs,
  drop gaps north and east), steps from the river bed up onto the bank.
- **River**: three bridges, two fords, stone steps out of the bed by the bridges and in two more places, each flight
  climbing straight onto the bank it stands against. Under it run the old river tunnel (south) and the mine gallery
  (north), and the pump-house hatches on both banks.
- **Quarry** (east, a pit at -128): rock piles for cover, the shed (roof by stairs), the stair trench down to the
  quarry's workings. (v2's gantry, a post and a beam that did nothing, is gone.)
- **Bunker** (south-east): two rooms, the roof by the outside stairs or the north wall's ladder; the stair trench to
  the bunker below (its core: tier 5).
- **Lumber camp** (north-east): log piles and the sawmill for cover; the watchtower beside the sawmill (a ladder, and
  stairs down to the sawmill's roof, a 176 drop to the yard all round; no roof on the tower any more, so nobody lands
  on top of it); a hatch into the east caves.
- **Mesas**: the fort (old walls, a crate, a rifle; the ramp on the east, the north gate's steps onto West Peak's
  flank) and the lookout (sandbags, a sniper; the ramp on the east, the cliff stairs on the north).
- **Cabins**: five huts at odd spots between places, each with a crate (tier 1-2): somewhere to land and arm up.
- **Below**: 36 rooms in a 6 x 6 grid with doorways (two each way) and the older tunnels through them; caves (rock,
  mud, warm yellow lamps), the quarry's workings (white lamps), the bunker (concrete, blue lamps), the utility
  basements (orange), the cisterns (concrete, green lamps). Rock pillars or concrete pillars, lumps in the cave walls,
  barriers in the bunker and basements: cover. Every room has a crate (3, 4 in caves and the bunker, 5 in the vaults),
  every other room a gun (rifles in the caves and the bunker, SMGs elsewhere).

### The placement validator, extended (build_map.py before writing; navgen.py --check on the compiled map)
See maps-src/br_valley/README.md for the full list. New in v3: every stair, ladder, ramp and walkway is registered
as a connector with its two ends; both ends must meet a floor with room to stand and the floor must go on past the
end (straight on, left or right); props must not float or be buried; drop-in spots and supply drops must land on
their own ground; every drop-in spot needs a gun within a 10-second run. On the compiled map (the real hulls):
traps (incl. every spot a drop-in can land on, every 64 units), high/underground pockets with fewer than 2 ways
out, ladders joined at both ends, connector ends on the walk, the walk from each drop-in spot to a gun, ground
beside every supply drop. Found and fixed on the way: stairs out of the river bed that rose along the bank and
ended over the channel (now each climbs straight onto its bank; stone steps so they read against the rock); the
fort's south wall, sandbags and crate hanging off the mesa's east edge;
tree trunks and foliage buried in hillsides (trees placed at the corner's lowest ground); the factory catwalk that
led nowhere (now a ladder from its end up through a roof hatch); the watchtower roof only a drop-in could reach
(gone; the tower is open with stairs to the sawmill roof); the barn, farmhouse and silo roofs with no way off; the
silo in a 32-unit gap by a fence; a chimney ladder facing a 32-unit slot; a rock lump blocking the mine gallery's
mouth (the west mine had one way out); bushes on slopes too steep to step off and bushes under tree canopies (both
traps); a cliff on the quarry pit's north rim (the terrain is now bounded by its fixed pads: at most 112 a cell from
any pad, bank or rim); the quarry trench whose top ran onto that slope; a street crate in the west sewer hatch's way
out; an aqueduct pier in the river bed at the foot of a flight of bed steps; bed steps at the north end that led
onto a strip by the map's edge (removed).

### Compile, nav, area (Final; the shipped build)
- SDHLT through CrossOver (the compile routes' commands, `-low`, niced), 2 min. **No leak.** Limits: clipnodes
  30106/32767 (**91.9 %**, the tightest), AllocBlock 56/64 (87.5 %), worldleaves 5398/8192 (65.9 %), planes 44.7 %,
  marksurfaces 37.7 %, visdata 0.7 MB (8.5 %), lightdata 2.5 MB; 5.6 MB .bsp. Warnings: one func_illusionary (lamps)
  touches 79 leaves (the engine's simpler vis check; harmless), an ambiguous-leaf sliver in hulls 1/3 at -1590 -3140
  (the nav walks there). **There is little room left for more brushes (clipnodes); more geometry means fewer trees.**
- Nav (navgen.py, rectangles up to 32 samples now): 138 996 samples, **5431 areas** (the brain builds its map
  knowledge only for navs of up to 6000 areas; the first v3 nav had 6312 and the bots played without it), all
  checks pass: 0 traps, 49 high/underground pockets all with 2+ ways out, 25 of 25 ladders joined, 73 connectors on
  the walk, drop-in spot to a gun median 546 / worst 1625 units of walking (6.5 s), 8 of 8 supply drops openable.
- **Walkable area per layer** (nav samples x 625): **under 35.9 M sq units, surface 44.1 M, high 6.9 M; 86.9 M in
  all = 1.69x the 51.4 M of the flat footprint** (v2: under 5.2 M, high 6.7 M, about 1.1x). The 2-3x target isn't
  reached: the underground is already a full second level under almost the whole map, and the compile limits
  (clipnodes 92 %, AllocBlock 88 %) leave little room for a third deck or more high routes; 2x would need ~16 M more,
  e.g. a second underground level under part of the map in place of some of the surface cover.

### Tested (my sim copy BRV3 of BRPLAY, port 27364, 20 bots, Battle Royale solo, `br_dropin 1`, brain r43395720981,
gm_battleroyale + gm_components built from HEAD 965fb0c, gm_simrec, a probe counting players who own a gun)
- **Run 1** (the first v3 nav, 6312 areas: the brain made no map knowledge, see above), 40 min, 4 complete games:
  7.8 / 10.1 / 8.1 / 3.0 min (17 s loot-up + 451 / 586 / 468 / 166 s of fighting); drop-in deaths 0; deaths to the
  world (the zone) 5 / 6 / 5 / 2 of 19; supply drops opened 1/3, 0/3, 2/3, 0/1; armed when fighting started 6, 3,
  1, 3 of 20; bots that were in each layer per game: below 1-2, surface 20, high 11-14 (time 1-11 % below, 4-8 %
  high).
- **Run 2** (the shipped nav, 5431 areas; the brain built its knowledge in the first minutes: 6.8 MB .lsv),
  35 min, 3 complete games: **8.2 / 5.8 / 9.8 min** (17 s + 475 / 333 / 571 s); **drop-in deaths 0**; **zone deaths
  5 / 3 / 4** of 19; **supply drops opened 1 of 3, 2 of 2, 1 of 3**; **armed when fighting started 2, 4, 3 of 20**;
  **bots in each layer: below 0 / 1 / 0, surface 20, high 12 / 13 / 14** (time 0-1 % below, 2-12 % high).
- What that says: the map holds (no landing deaths, no one stuck, games in the 6-10 min target), bots use the high
  layer (most land on or climb to a roof or tower at some point), but they hardly go below and rarely pick up a gun
  in the loot-up. Both are bot behaviour, not geometry (every room below is on the nav and reachable; every drop-in
  spot has a gun within a 6.5 s walk): requests for W1/W2 in docs/notes/W1.md. With people the layers should fill
  differently; a person can see a portal and go down.
- Screenshots (not in the repo; scratch folder of this session): 150 views, both ends of every connector
  (`shots/conn/*.jpg`), 16 views of the three layers (`shots/tour/*.jpg`), contact sheets (`sheets/conn_*.png`,
  `sheets/tour_*.png`), the radar top-down (`overview.png`; also maps-src/br_valley/layout.png).
- My sim copy BRV3 and my dev servers (8369, Vite 5369) are stopped and removed.
