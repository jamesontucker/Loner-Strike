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

## Rule from Y (2026-11-13): touch only your own server processes
- Never attach a debugger (`winedbg <pid>`, `attach`, DebugBreakProcess, anything that puts a thread into another
  process) to a running hlds under CrossOver: the injected thread starts at 0xfff4fadc and kills the server.
- Stop, signal or inspect only the processes your own script started: by the PID you got when you started it, or by a
  pattern that names your own sim folder (`Sim[\\/]<yours>[\\/]hlds.exe`). Never by name (`hlds.exe`, `winedbg`) and
  never by "the newest process". Before a start, make sure your port is free (`lsof -nP -iUDP:<port>`).
- Stacks and crashes without a debugger: `drive_c/Loner-Strike/Sim/crashes.log` (every crash in the bottle, full
  winedbg report), and with fork builds from ls-creatures `f351765` on, `cstrike/ls_crash*.log` and
  `cstrike/ls_watchdog.log` in the server folder.
