# QA — independent real tests of Tools (editor, compile, Game Mode Studio, Last Stand, Battle Royale)

2026-11-12, agent QA (+ three QA sub-runs). Everything below was run for real: dev servers on the real routes, SDHLT
through CrossOver, HLDS sims (QA1 27380, QA4/QA5 27386-7, QA7/QA8 27388-9, all clones of Sim/LSPLAY) and once the real
CS 1.6 client. No product code was changed. Bugs are filed in the owners' notes under "Requests from QA":
editor → P5.md, compile → P4.md, Game Mode Studio → X.md, Last Stand / BR / components → W1.md, DLL/creatures → Y.md.
Scripts, logs and screenshots: QA's scratch (`scratchpad/qa/`), not in the repo; third-party map sources were only
downloaded there (licences: TWHL 6398 and 6452 CC BY; 6216 All Rights Reserved, read only) and the WADs QA put in the
game folder for compiling were removed afterwards.

## Summary
| Area | Features tested | Pass | Partial / not verifiable | Fail |
|---|---|---|---|---|
| RMF/JMF import → compile → HLDS (8 real files) | 8 | 2 (pmk_m2, pmk_m3 load like the reference) | 1 (carver: no inside) | 5 (geometry drift → leaks / broken brushes) |
| Export / compile / play (from scratch, leak, cordon, outpost, valley, client) | 9 | 9 | 0 | 0 |
| Editor features through the UI (WebKit + Chromium) | ~60 rows | 41 | 12 (incl. not reached) | 1 to check by hand (3D push/pull) |
| Game Mode Studio nodes + routes | 27 rows | 17 | 4 | 6 |
| Last Stand + Battle Royale playthroughs | 23 rows | 14 | 4 | 5 |

Worst: (1) RMF/JMF import moves slanted brushes → real Hammer/J.A.C.K. maps leak (P5); (2) Studio Drop Weapon crashes
HLDS (DLL, X + Y); (3) Studio modes never load gm_components, so templates and component/creature nodes stall (X);
(4) a Studio breakpoint inside For Each freezes the server with no Resume (X); (5) Last Stand bots never buy guns, BR
start loadout and drop-in broken (W1). **Y's hang after a boss-wave loss: not reproduced** (3 boss losses, see Y.md).

## 1. Real Hammer / J.A.C.K. imports (agent QA, devserve 27381, HLDS Sim/QA1 port 27380)

Sources (real projects, kept outside the repo in QA's scratch, not redistributed):
- `regamedll_test_map_v8.rmf` (ReGameDLL_CS `extra/Test Maps/src`, Hammer RMF 2.2, + `regamedll_zedit.wad` and the shipped `.bsp`)
- TWHL vault 6452 "2020 Practice" (CC BY): `pmk_m1/2/3.rmf` (RMF 2.2) + their BSPs
- TWHL vault 6398 "A Dozen Map Sources" (CC BY, J.A.C.K.): `de_kobbl_v102.jmf`, `aim_stadium_gsu_v01.jmf`, `pustinya_v0002.jmf`, `carver1.jmf`
- TWHL vault 6216 de_kobbl v1.0.2.1 release BSP (for comparing the JMF import's entities; All Rights Reserved, only read)

Route used: the Project panel's Open on a .rmf/.jmf beside a project (`POST /api/tools/editor/project/action {action:"open"}`),
because `import-other` (File > Import) only works with the app's native dialog (see bug QA-12). Then Save (project), reopen,
Export .map, live checks, `POST /api/tools/compile/start` Normal, copy the .bsp to an HLDS and load it (console log +
an entity-count plugin `qa_ents` on QA1). Reference: QA's own RMF reader (`rmfcount.py`, reads the whole file to the
last byte) for counts, and QA's own RMF→.map converter (`rmf2map.py`, Hammer's stored plane points, as Hammer exports)
compiled with the same SDHLT command lines.

| Map | Counts source → editor (brushes / faces / entities / brush ents / groups / visgroups / textures) | Entity keys | Geometry vs source | Compile (Normal) | HLDS load |
|---|---|---|---|---|---|
| regamedll_test_map_v8.rmf | 760/4834/442/154/18/0/77 → identical | 431/442 identical; 8 spawnflags differ (QA-2), 3 game_text messages lost (QA-3) | 154 brushes off by >0.1 u, worst 2.7 u (QA-1) | **FAIL**: leak + 2 illegal brushes; reference .map from the same RMF: no leak, no illegal brushes | reference build loads (455 edicts, 151 brush models); a cordoned part of the import compiles and loads (90 edicts) |
| pmk_m1.rmf | 172/1059/62/21/4/1 ("subway")/41 → identical | 62/62 | 47 brushes >0.1 u, worst 2.3 u | **FAIL**: leak; reference: clean | reference loads |
| pmk_m2.rmf | 185/1138/77/52/3/0/66 → identical | 77/77 | 9 >0.1 u, worst 0.26 u | PASS (6 s) | PASS: 73 edicts / 33 brush models, same as the reference build |
| pmk_m3.rmf | 308/1677/109/35/1/0/60 → identical | 109/109 | 104 >0.1 u, worst 9.3 u | PASS with 3 "illegal brush" warnings (reference: 0) | PASS: 89 / 19, same as reference |
| de_kobbl_v102.jmf (CS) | 5118 brushes, 1137 entities, 11 groups, 11 visgroups (6 hidden ones noted) | every targetname of the released BSP present; class counts ≥ release | 52 brushes the editor itself calls broken, one with empty bounds | **FAIL**: "brush outside world" (the empty brush); live checks also show false limit errors (QA-6) | – |
| aim_stadium_gsu_v01.jmf (CS) | 738 brushes, 385 entities, 16 groups, 3 visgroups | – | 5 illegal brushes at compile | **FAIL**: leak | – |
| pustinya_v0002.jmf (CS) | 1008 brushes, 69 entities | – | – | **FAIL**: leak + planes 132 % of the limit | – |
| carver1.jmf (tutorial) | 226 brushes, 1 entity | – | – | fails with the raw "Portal file … does not exist" (no entities: expected to fail, words aren't friendly, QA-10) | – |

Imports are fast (0.01–0.12 s) and never errored. Counts, groups, visgroup names, entity keys and point origins match.
**Brush geometry does not**: see QA-1. Everything an RMF/JMF user imports with slanted brushes comes out a bit moved,
which leaks or breaks brushes on 4 of 6 real maps that compiled cleanly from the same source.

## 2. Real export / compile / play

| Test | Real test done | Result | Evidence |
|---|---|---|---|
| Build from nothing via routes | new → delete all → box 1024×1024×256 → hollow 16 (6 walls) → textures on 5 walls → fit/shift/scale/rotate/world/justify on the floor → crate + clip keep-both → cylinder → box tied to func_door (+speed, angles) → 2 CT + 2 T spawns, light, light_environment → components ls_weapon_spawner + ls_loot_container → prefab builtin:stairs → duplicate + undo + redo → live checks → save | PASS (26/26 steps, counts checked in `GET doc`) | `scratchmap/build_log.json` |
| Export + compile Normal + install (the Playtest compile path) | compile `{preset: normal, install: true}` | PASS: CSG/BSP/VIS/RAD in 5 s, installed to cstrike/maps | BSP entity lump: 10 entities, 2 models |
| Playtest in the real game | started CS 1.6 through CrossOver's Steam `-applaunch 10 +map qa_box` exactly like the app's launch | PASS: listen server on qa_box, bots joined, learned the map (nav saved), room/door/textures visible | `client_qa_box_game.png`, client console log. The app's own Playtest button (`/api/app/launch`) needs the app window, not run. Note: the game started in mode "fefwefew" (someone's Studio test had set current.txt) |
| qa_box on a dedicated server | QA1, classic and laststand | PASS: 40 edicts, func_door brush model, 4 bots; in Last Stand "2 components on qa_box" | load logs |
| Leak detection | wall deleted, compile Fast | PASS: fails with "The map leaks…", leak line from `.lin` (10 points through the gap), linked to the entity it starts from | |
| Cordon | same leaky map, cordon box + spawns + keep lights | PASS: compiled sealed, playtest spawns added, loads on QA1 (40 edicts, bots) | also on the imported RMF (a 700×700 area: compiled, warnings for targets outside, loads) |
| ls_outpost source | open .map, save, export, compile Normal, Last Stand on QA1 | PASS: export identical except the header comment and a trailing `;` in `wad`; compiled entity lump identical to the shipped ls_outpost.bsp (100 ents, 22 models); "19 components on qa_outpost" | |
| br_valley source (uncommitted, snapshot 22:28) | same | PASS: 2974 brushes, compile 243 s (VIS 152 s), only number formatting differs on export (87.500 → 87.5), loads (66 edicts, 13 brush models) | |

Also by route (QA): autosave → `kill -9` the dev server → restart → recovery found (7 brushes) → recover → 7 brushes
(PASS); two saves → two backups listed in the Project panel (PASS); history panel route lists the undo steps (PASS).

## 3. Editor features through the UI (Playwright, WebKit and Chromium; results checked in `GET doc`)
| Feature | Real test | WebKit | Chromium | Result |
|---|---|---|---|---|
| Box tool in Top / in 3D | drag; brush exactly at the grid points / on z=0 | ✓ | ✓ | PASS |
| Undo/redo (buttons, Mod+Z, Mod+Shift+Z) after box, hollow, carve, merge, clip, vertex, find/replace, duplicate | brush counts back and forth | ✓ | ✓ | PASS |
| Click pick, box select in a flat view | | ✓ | ✓ | PASS |
| Box select in 3D | | – | – | not reached |
| Hollow / carve / merge | 1→6 walls; 8→11 (cutter kept); 8→7 | ✓ | ✓ | PASS |
| Clip tool (Tab cycles sides, Enter) | +1 brush at x=−192 | ✓ | ✓ | PASS |
| Vertex tool | corner −128→−96; .map stays whole numbers | ✓ | ✓ | PASS (float noise in UI sizes, P5 #9) |
| Transform dialog, mirror, turn 90°, line up / spread out | | ✓ | ✓ | PASS |
| Group / ungroup, pick groups vs objects | | ✓ | ✓ | PASS |
| Tie to entity / move to world | | ✓ | ✓ | PASS (selection after tie, P5 #9) |
| Copy / paste / paste special (4 copies) / cut | 8→12→8 | ✓ | ✓ | PASS |
| Alt ignores snap, snap to grid (Mod+B) | | ✓ | ✓ | PASS |
| Entity tool; FGD form: colour+brightness, flags, choices, angle dial, new key | `_light`, spawnflags=1, rendermode=1, angle=90 | ✓ | ✓ | PASS |
| Duplicate with renaming (Mod+D) | t1 → t2 and the relay's target → t2, originals kept, one undo | ✓ | ✓ | PASS |
| Multi-entity keys | angle 270 on 2 spawns, one undo "Set angle on 2" | ✓ | ✓ | PASS |
| Find and replace textures | 36 places / 6 brushes; one undo; redo | ✓ | ✓ | PASS (also used by QA on 4 real imports) |
| Select same texture / same type | | ✓ | ✓ | PASS |
| Select touching / all / none / invert | | – | – | not reached |
| Go to (editor id, "Entity 0, Brush 0") | | ✓ | ✓ | PASS |
| Hide / show all / isolate; lock / unlock all | | ✓ | ✓ | PASS |
| Outliner search / eye / lock | search filters; eye/lock buttons not reached | ✓ | ✓ | PARTIAL (P5 #9) |
| Visibility sets | new from pick, untick hides, pick contents | – | ✓ | PASS |
| Texture browser + apply | "crate" 88 tiles, 6/6 faces | ✓ | ✓ | PASS |
| Texture tool + face sheet (shift 16, scale 0.5, turn 45, justify, fit) | one face only | – | ✓ | PASS; also all align actions by route on qa_box (fit, shift, scale, rotate, world, justify) |
| Fit N×M, texture lock on move, drag handles | | – | – | not reached |
| Shapes: cylinder, wedge, spike, sphere, arch | 0 broken brushes | – | ✓ | PASS |
| Move tool drag; arrow nudge | +64 x; nudge did nothing | – | ✓ | PASS / check by hand |
| Measure | 502.41 units | – | ✓ | PASS |
| Push/pull a face in 3D | drag up 80 px on the picked box | – | ✓ | FAIL? check by hand (P5 #9) |
| Move/rotate/scale gizmos, Shift-drag copy | | – | – | not reached |
| History panel (list / jump) | list OK; jump test mis-clicked | – | ✓ | PARTIAL |
| Live checks with fixes, click to pick | | – | ✓ | PASS |
| Entity report, map information, grid dialog, `]` | | – | ✓ | PASS |
| Map properties | opens; edit not reached | – | ✓ | PARTIAL |
| Unreal camera RMB+WASDQE with a search box / Inspect field focused | view moves, nothing typed | – | ✓ | PASS |
| Blender / Hammer camera presets | | – | ✓ | not verified |
| Layouts (Single, Quad, 1+3, Two), maximise (Shift+Space, double-click) | | – | ✓ | PASS |
| Per-pane drawing style | listed; switching not reached | – | ✓ | PARTIAL |
| Docking: drag tab to bottom, float, remembered after reload, presets, ⋯ menu | | – | ✓ | PASS (resize handle not verified) |
| Command palette / cheat sheet / key sets | | – | – | not reached |
| Prefabs: place, save what's picked; replace with prefab | | – | ✓ | PASS / not reached |
| Components library (18) → ls_barricade | | – | ✓ | PASS (+ by route: 2 and 19 components seen by gm_components in HLDS) |
| Publish… | opens Community publish for the map | – | ✓ | PASS |
| Save/open/export/import through the UI, History & restore page | | – | – | not reached in the UI (all done by route in parts 1-2) |
| Unsaved-changes question; console errors | | ✓ | ✓ | PASS, none |
WebKit covered the first half of the list; the run was cut before the second half (Chromium only there).

## 4a. Game Mode Studio (sims QA4/QA5, bots; evidence = Studio debug events + a QA dump plugin + console)
| Node / feature | Result | Evidence / bug |
|---|---|---|
| Player actions: give weapon/item, health, armour, money, frags, speed, gravity, glow, model, role | PASS | `hp=150 ap=77 money=2034 frags=2 grav=0.50 maxspeed=200 … m249` |
| Player readouts | PASS | |
| Freeze, teleport, launch, no-fall, set team, strip, kill, respawn | PASS | teleport moved, strip → knife, respawn → glock18 |
| Drop Weapon | **FAIL: HLDS crash** | X #1, Y |
| Damage scale / set / block | PASS (Player Hurt fires for blocked hits) | hp 100→88 at ×0.5; X #9 |
| Down / revive / timeout | PASS | 8 downed, 1 revived by a bot at 50 HP |
| Squads + friendly fire off | PASS | 114 hurt, 0 kills in 250 s |
| Buying rules incl. bots | PASS | pistols only / m249+ak47 / money untouched |
| Saved values survive kill -9 | PASS | 4 → 5,6,7 after restart |
| Round clock set/add/pause/resume, Time Up | PASS | Time Up at 66.02 s (≈65) |
| Variables, maths, conditions, functions, flow, timers | PASS | |
| Events (map, round, join/leave, team, bomb) | PASS (Team Changed ×3) | bomb defused never happened |
| Player used / touched | not tested | no fitting entity, bots don't USE |
| Chat command | PASS | `cc called` |
| Menu | not verifiable with bots | needs a person |
| Game settings, use entity | PASS | |
| Bot orders (go to, hold, follow, release) | plausible | not measured to the unit |
| Component nodes | MIXED | X #2, #8 |
| Creature nodes | FAIL alone, PASS with gm_laststand | X #3 |
| Last Stand Lite template | **FAIL: stalls** | X #4 |
| Royale Lite template | FAIL as installed; PASS with gm_components | X #2/#4 |
| Compile / playtest / install / uninstall / source / delete routes | PASS (leftovers on uninstall) | real game folder, cleaned up |
| Switch an open game | not run (needs the game) | |
| Breakpoints server side | PASS | world paused 8 s, resume runs the node |
| Breakpoints in the page (WebKit + Chromium) | **FAIL** | X #5, #6 |
| `npm run test:studio` | 12/14 | X #9 |
| studio-playtest.spec (WebKit + Chromium) | PASS | |

## 4b. Last Stand and Battle Royale (QA7 ls_outpost v2, QA8 br_valley + de_dust2, bots)
| Feature | Result | Evidence / bug |
|---|---|---|
| 7 full stands (Normal/Easy, 5-8 bots), phases, "wave N held" | PASS | |
| Loss → new stand (5 losses) / a win (lst_waves 3) | PASS | win not logged (W1 #7) |
| Creatures, 3 boss waves, 361 kills, best-wave record | PASS | records 2 → 4 |
| Barricade boards drop / repair / break → not solid | PASS | 5/5 → 3/5 → 0/5 → 5/5 |
| Barricades block movement | PASS | 0 overlaps (0.5 s probe) |
| Bullets pass barricades | PASS | 504/504 (`ls_comp_tracestats`) |
| Hold-USE repair by a person, shop menu, HUD/chat text | not verifiable without a person | |
| Shop: bots buy at the armory; Barney loaded | PASS | |
| CS buying between waves (bots) | **FAIL** | W1 #1 |
| Revive | PASS | back in 5 s mid-wave |
| Supply drops land / opened | land PASS; opened FAIL (0/27) | W1 #5 |
| Creatures through intact gate | FAIL? | W1 #4 |
| AMXX / engine errors | PASS, none | |
| BR 6 games to a winner, zone stages + damage, crate loot | PASS | |
| BR start loadout | **FAIL** | W1 #2 |
| BR drop-in | **FAIL** (with br_dropin 0: PASS) | W1 #3 |
| BR squads | PARTIAL | ran; friendly fire between mates not checked |
| br_valley loot hints | FAIL (32-slot cap ×1092 log lines) | W1 #6 |
| **Y's hang after a boss-wave loss** | **not seen** | 22:27:11, 22:32:36, 22:44:21 losses; rcon answered after each (Y.md) |

## Not covered (needs the app window or a person)
The app's own Playtest button end to end (`/api/app/launch`; QA launched the client the same way by hand instead),
File > Import's native dialog, Studio "switch an open game", menus / HUD / hold-USE repair as a player, the real
WKWebView / WebView2 compositor (Playwright WebKit/Chromium stand in), native Windows.
