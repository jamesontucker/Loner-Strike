# QA round 4, area A: the map editor (a fresh QA tester, 2026-11-14)

What was tested: `tools` 314c7f47. The editor, compile and MCP files had no uncommitted changes in the shared tree when
I ran. The bot brain was ls-next3 run 43604814272 (head `285b895`, the same as `git ls-remote`), checked inside each sim
(`lonerstrike_mapknow` is in mp.dll and the console says "ReGameDLL version: 5.30.0.1289-dev"). The plugins were
compiled fresh into Sim/QA4A from a `git archive HEAD` snapshot. The gm_* plugins all built. The 40 zp50_* plugins
failed on missing includes, which looks like a problem in `plugins-fresh.sh` and is outside area A.

How:
- Two Tools dev servers over Sim/QA4A, with their own LS_APP_DIR, TMPDIR and Vite cache, and HMR off.
- A third dev server over the real Half-Life folder, used only to compile with `install: false`. The real game folder
  had no file newer than my start mark at the end.
- Playwright specs in WebKit and Chromium at 1440x900, plus my own scripts under `app/ui/qa4a-tmp/`, which are
  untracked and removed after.
- Rust tests from the shared target.
- Real SDHLT compiles through CrossOver, with `-threads 6` and the dev server under `nice -n 15`.
- Two sims: QA4A (a clone of LSPLAY, CS 1.6, port 27420) and QA4AZ (Valve's CZ server from SteamCMD, port 27421). Both
  used `-insecure`.

Evidence is in my scratchpad, `/private/tmp/claude-501/-Users-jameson/a1c85d27-df33-445d-9b50-85a20aff9c24/scratchpad/qa4a-evidence/`.
It holds the Playwright logs and JSON, the sim consoles, and `shots/`.

**Counts: 20 rows. PASS 17, FAIL 2 (Budget, Content Browser + credits), NEEDS USER 1 (Playtest from F5 in the built
app). One more part needs the user: a real J.A.C.K. .jmf, inside the import row. There are 6 FAIL findings (A-1 to
A-6). Two of them fail rows; the other four are bugs found along the way in rows that otherwise pass.**

## FAILs (most important first)

### A-1. Budget's "from the last compile" can show an older, different compile (medium-low). Owner: Budget (c6c0c15f, P5 / Parity A)
`budget.rs:233` finds the log by the **document's** name: `work/<doc name>/<doc name>.log`. A compile under any other
name is ignored. The Compile panel offers a name field (`suggestName`), so this is easy to hit. Then the Budget shows
whatever an earlier compile of a map with the document's name left in the shared `Compiler/work` folder. That compile
can be days old or come from another project with the same name.
Repro (dev server or app):
1. Open `ls_outpost.map`.
2. Compile Normal with the name `qa4a_outpost`. It finishes at 08:20.
3. `GET /api/tools/editor/budget` gives `compile.log = .../work/ls_outpost/ls_outpost.log`, `when` = Nov 13 16:19. Its
   rows don't match this compile: nodes 634 vs 633, clipnodes 1508 vs 1506, leaves 416 vs 415, marksurfaces 1850 vs 1840.
4. Compile again under the name `ls_outpost`. Now the rows match the log's chart row for row: models 3.7 %, planes 1.9,
   vertexes 3.1, nodes 1.9, faces 2.1, clipnodes 4.6, leaves 1.3, marksurfaces 2.8, surfedges 1.3, edges 1.4,
   AllocBlock 12.5.

Expected: the Budget reads the compile this document last ran, or it says plainly that no compile of this map is known.

### A-2. Map names that Windows reserves are accepted, then the compile fails with compiler jargon (medium on Windows). Owner: P4 (compile.rs `valid_map_name`, fbb85938)
`POST /api/tools/compile/start` with the name `con`, `NUL`, `aux` or `com1` is accepted (`ok: true`). The compile then
fails with "The compiler said: File read failure", "sdHLBSP stopped with an error", or "Could not open logfile
C:\Loner-Strike\Compiler\work\aux\aux.log". On Windows (the main platform) a file named `con.bsp` can't exist at all.
The names `-` and `_` are accepted and compile. A map called `-` is a poor console argument (`map -`).
Expected: refuse the reserved device names (con, prn, aux, nul, com1-9, lpt1-9, in any case), and names that start with
`-`, with the usual "Map names can only…" style message. Probably the same check belongs in Save as, Export and
Publish.

### A-3. Stock CS 1.6 models show as "Not checked yet / doesn't know where this file is from" when Condition Zero isn't installed (medium-low). Owner: T (credits.rs, d4b35435) with the map doctor's `doctor::is_stock` (96db1b98)
In the Content Browser, `p_galil`, `v_famas` and the other stock models say **From: The game's own**. Their credit badge
says **Not checked yet** with "Loner-Strike doesn't know where this file is from. Add a credit for whoever made it (or
mark it as yours)." `content_credits` (`/api/tools/credits/asset`) answers `source: unknown, verdict: unknown`. The cause
is `is_stock()`: a cstrike file counts as stock only when it is in `valve/`, or in **both** `cstrike/` and `czero/`. Any
player who owns only CS 1.6 has no czero folder, so every stock cstrike model, sprite and sound is "unknown". The same
rule feeds Publish's credits (`credits_for_files`).
Repro: a dev server with LS_TEST_GAME at a Half-Life folder without `czero/` (any LSPLAY clone) › Content › Models ›
click p_galil. The badge and content_credits now agree with each other (the row's literal check), but both are wrong
for a stock file. Screenshot: `qa4a-evidence/shots/content-credit.png`.

### A-4. File › Import (or Open) clicked while "Starting a new map…" is still running is silently dropped (low). Owner: P5 (store.ts `file()`, 58481bf3)
`store.file()` returns at once when `fileBusy` is set: `if (get().fileBusy) return;`. The menu items stay enabled, and
nothing tells the person their click was ignored.
Repro (WebKit):
1. Leave a big dirty document open (br_valley pasted).
2. Reload, then New map › Leave them.
3. Within about a second, while "Starting a new map…" shows, pick File › Import a .map….
4. No `import-map` request is sent and the map stays empty.

`qa4a-tmp/busy2.mjs` reproduced it: the busy label lasted 1.1 s, Import was clicked at 0.6 s, and the map still had 6
brushes. It doesn't happen in Chromium, where the busy time is shorter.
This is also why `editor-bigmap.spec.ts` **fails in WebKit when it runs right after editor-bigedit.spec.ts** (the full
editor suite in alphabetical order). It passes alone. Expected: disable the File items while busy, or queue the click,
or say "Still starting the new map…".

### A-5. 2D handles: dragging an edge past the opposite side leaves a 1-unit sliver off the grid (low). Owner: Parity A (FlatHandles.tsx:207-208, e8f90dda)
Steps in Quad, Top view, grid 16, with a 128³ box picked:
1. Drag the east middle handle 300 px to the left, past the west edge. The box becomes `[-64..-63]` wide: 1 unit, not
   on the 16 grid.
2. Then drag a corner onto the opposite corner. The box becomes 1×1×128.

This is one undo step and doesn't crash, but the row says "the edge on the grid". Expected: stop at one grid step (or
flip, as Hammer does). Screenshot: `shots/flat-invert.png`.

### A-6. Copy: "1 texture aren't in any texture file here" (low, US English). Owner: P5 (tools/editor/workspace.rs:676)
After importing `regamedll_test_map_v8.rmf`, the note reads "1 texture aren't in any texture file here (e.g. null)". Only
the noun gets the plural. The verb should be "isn't" for one.

## Rows

| Row | Result | Evidence |
|---|---|---|
| Big-map perf | **PASS** | br_valley Quad, Top view, `editor-flatperf`: WebKit **60 fps** panning, 120 zooming; GPU Chromium 60 / 121; software Chromium 45 / 84. `editor-framesplit`: WebKit nudge+undo p95 24 / worst **33 ms**, drag-move p95 23 / worst **34 ms**; GPU Chromium (ANGLE Metal) nudge worst 16.8 ms, drag worst 66.6 ms (one long frame). Software Chromium (SwiftShader) has 200 ms frames; it's not the app's engine, so information only. `editor-idle`: **2 requests in 60 s** in both engines (one `state`, one `selection` long poll). `editor-perf` (3000 brushes) 62 fps in WebKit. Load 8-15. |
| Frame-time readout | **PASS** (browser) | View › Show frame time on br_valley: "measuring…", then "60 fps · p50 16.0 · p95 19.0 · worst 19 ms" idle in WebKit, and the worst goes to 35 ms while flying (right mouse + W) and back. Chromium GPU 59-60 fps / p95 16.7. ⌘⌥⇧F hides it. Shots `frametime-*.png`. In range of the perf specs. The built app wasn't run (see Playtest). |
| Selection for agents | **PASS** | Page → agent: shift-picking 3 brushes in the Outline showed up in `GET selection` in **177 ms** (WebKit) and 188 ms (Chromium), with sizes (64/80/96 × 64 × 64), bounds and center. Painting it (what `editor_set_texture {selection:true}` sends) changed only those 3 of 5 brushes. One undo put all 3 back ("Apply texture"). Agent → page: `POST selection {by: agent}` picked 2 others in the page in **183 / 92 ms**. With `/api/**` blocked: 7 asks in 40 s, never more than 1 a second, backing off 1-2-4-8-10-10 s. MCP over JSON-RPC (the `serve_mcp_for_try` harness on a scratch app dir, real `/mcp` with a token): 175 tools; `editor_select [12,13]` → `editor_selection` lists both; `editor_set_texture {selection:true}` paints them; a missing id or an empty selection gives "Nothing is selected in the editor. Ask the person to select it, or give ids." Rust `agent` 9/9 and `selection` 1/1. The page and the MCP endpoint weren't in one process (the dev server has no /mcp), so the live Claude Code run against the built app is still MCP's area F row. |
| Checks | **PASS** | Purpose-built maps through the API, each finding at the right spot. stuck-in-solid, micro-gap, off-grid, stretched-texture, light-in-solid, z-fighting (a func_wall flush with the world: 1 → 0 → back to 1 on undo), texture-seam (3 → 2 per fix), hidden-faces: each one-click fix is **one step "Fix a problem"** and one undo brings the finding back. no-sun and dark-room are `place:` fixes (the page switches to the Entity tool, by design). ladder-gap is a hint only. **An edit isn't blocked while checks run on br_valley**: 5 nudges took 23-24 ms each while a 3.1 s check ran (it came back `stale`, as designed). Specs `editor-checks` / `editor-checks-webkit` pass. Rust `editor::checks` passes (inside `editor` 155/155). Information: on two coincident boxes (one a func_wall) "Nudge 1 unit" fixes one face pair, and the finding stays for the next. |
| Budget | **FAIL** (A-1) | With a matching name: the bars equal the compile log's chart row for row (see A-1). A red bar asks before Playtest/Publish, and the Last Stand roster: `editor-budget` passes in both engines. |
| Leak tracing | **PASS** | Real Fast compile of a 1024×1024×256 room whose east wall stops 64 short of the ceiling: "The map leaks, so it can't be finished. Close the gap the red line goes through…". `GET leak`: hole at **(512, 0, 224)**, in the gap. Nearest brushes are the **ceiling (#13) and the short wall (#17), both 32 u away**. Advice is given. Closing the gap with one box: the compile is Done and `leak` is `{leaked:false}`. `editor-leak` (bar, Go to leak, Pick brushes) passes in both engines. |
| Entity I/O view | **PASS** (built map) | **ls_outpost has no I/O at all**: 0 links, 8 named entities, no gates or multi_managers as entities; its gates are `ls_barricade` brushes. So the row's map can't show this. On a built map (multi_manager with `door1 0.5`, `light1 2.25`, `door1#1 3`; a func_button with target and master; a trigger_once with killtarget and a missing target), `GET io` lists all 7 links with their delays (0.5, 2.25, 3.0), `master` and `kills` kinds, and 1 broken link. `editor-io` (click a link picks both ends) passes in both engines. |
| Extrude, vertex split/merge, shear | **PASS** | Each refusal gives a reason. A concave new corner: "…would dent the brush inwards or lie flat on a face… Brushes have to stay convex." Merging opposite corners: "would leave a corner inside the brush". Merging all corners: "would flatten the brush". Moving a corner in: "would dent the brush inwards…". Extrude 0 or negative: "Drag outwards…". 1e7: "out of range". Shear 9:1: "too steep". Each success is one step (Extrude / Split / Shear). Rust extrude/vertex/shear tests and `editor-extrude`, `editor-flat` (shear) pass in both engines. Information: a refused edit still moves `revision` by 1 (no history step is added), so pages refetch for nothing. |
| Primitives | **PASS** | `editor-shapes` passes in both engines. Rust `shapes` 9/9 (valid, convex, base corners on the grid). 120 random placements (cone, torus, arch, sphere, wedge, spike, cylinder; sides 3-64, arc, start, wall, add_height, top, tube_sides) gave 111 placed with no invalid brush in Checks. The 9 refused were arc 720 ("The arc is out of range"; spirals stop at one turn). Odd inputs are all refused with a reason (sides 2 / 100000, radius 0, negative height, tube 2, wall thicker than the arch, cone top 5 / -1, unknown shape). |
| Face sheet | **PASS** | `editor-facesheet` (numbers, justify, fit, the three aligns, Pick up, right-click paint, Alt+right-click carry) passes in both engines. **Wrap round an edge** (`wrap_texture`, what Alt+right-click sends) from a box's +X face (GENERIC015V, shift 13/29, scale 0.5/0.75) onto its +Y face is one step "Apply texture and alignment". On the shared edge both faces give the same (u, v) at z 0/64/128: (269, 29) / (269, -56.33) / (269, -141.67). Moving away from the edge, u keeps counting on: north 269 → 301 → 333, east 269 → 237 → 205. The texture runs on. |
| 2D views | **PASS**, with A-5 | `editor-flat` (8 handles stretch with the edge on the grid, a drag moves, a box picks, Box draws, zoom to the cursor) passes in both engines. A-5 is the over-drag case. |
| Content Browser + credits | **FAIL** (A-3) | `editor-content` (thumbnails, search, size, drag a model/sprite/sound onto the map, Choose… on a key, Use selected) and `editor-batch` drops pass in both engines. The badge equals content_credits, but both are wrong for stock files without CZ (A-3). A GameBanana download was not tried (it needs a download into the sim; area G covers it). |
| Compile presets | **PASS** | Fast: CSG, BSP, RAD `-fast` (no VIS). Normal: all four, no extra flags. Final: VIS `-full`, RAD `-extra`. Every stage got `-threads 6` from the test-only `LS_COMPILE_THREADS`. ls_outpost Normal in 10-28 s: 1395 faces, VIS 5294 bytes, light 317841 bytes, 32+32 spawns. Bad names are refused ("Map names can only use letters, numbers, _ and - (up to 31)."), and `de_dust2` gets "is one of the game's own maps", except for A-2. Under test, install into the real game is refused, and the real game folder is unchanged. |
| Playtest on a sim, CS 1.6 | **PASS** | The compiled `qa4a_outpost.bsp` went into Sim/QA4A by hand (the dev server can't install: compile needs the real game, and the real game is guarded). Three ways, current brain: **(1)** as the app's hidden server (no plugins, 1 bot): nav saved after ~4 min, then "map knowledge for qa4a_outpost: 365 areas, 34100 pairs in sight, 581 wallbang spots…, done 50 s after it started". **(2)** plugins + 1 bot: nav + knowledge in **150 s**. **(3)** plugins + 5 bots: **152 s**, then "[Game Mode] Bots finished learning the map. Map timer restarted". Note: my first two 5-bot starts (bound to the LAN IP, around 08:14 and 08:28) stopped at "Analyzing… 360 of 365" with the process's CPU time frozen and no reply on UDP. That window overlapped a bottle crash at 08:23 (crashes.log: "crash: pid 208 … WineDbg attached to pid 00d0", with `winedbg --auto 208` **still hanging, not mine, left alone**). The third start with the same settings passed, so I count it as the shared bottle, not the brain. |
| Playtest on a sim, CZ | **PASS** | Valve's CZ server (SteamCMD) with the brain copied in as navs.rs does (CZ's own bot profile kept: the bot was "Yanni"), learn_args and LEARN_CFG: nav saved and map knowledge ("365 areas, 34100 pairs…") **45 s** after start, then again 51 s on a second start. The first CZ server stopped answering rcon about 9 minutes after it finished, with its CPU time frozen, around the 08:23 bottle crash above. The second stayed up: rcon status fine at 66 s, 166 s and 684 s (11 min), with no new entry in crashes.log. |
| Playtest from F5 in the built app | **NEEDS USER** | F5 installs into the player's own game and starts it: the user's real install. Not done here. |
| Prefabs | **PASS** | Through the API: an empty name, no pieces, or missing ids are refused with plain words. "Café sign ✓" → id `caf_sign`, name kept. "../../evil" → id `evil`, nothing outside the folder. A 500-character name is cut to a 48-character id and a 64-character name. Place on a floor and on a wall is one step "Place prefab" (a group of 3 brushes and 1 light). A far point gives "That point is outside the map." Edit name and tags, Export (a zip with `.map`, `.json`, `.png`, README, `loner-strike.json`), Delete all 4: none left. Rust `prefab` 11/11, and the prefab parts of `editor-kits` / `editor-ux` pass. |
| Import .map / .rmf / .jmf (round 3 regressions) | **PASS** (.map, .rmf), **NEEDS USER** (.jmf) | `regamedll_test_map_v8.rmf` **is on this Mac** (in every regamedll checkout, `extra/Test Maps/src/`), contrary to the self-check. Rust `real_rmf`: 760 brushes, 0 invalid, 442 entities, 18 groups. Through the routes: import-other (760 / 442, notes about cameras and one missing texture, see A-6) → Save as (`.lsmap`, 4.25 MB) → Export a .map → re-import the .map (760 / 442) → **Normal compile Done** (3 compiler warnings from the source map: an ambiguous leaf in a func_wall, two func_illusionary bounding boxes). br_valley's .map: import, autosave, Save as, Recent (`editor-bigmap`) pass alone in both engines. No real J.A.C.K. `.jmf` exists on this Mac. Rust jmf 2/2 are synthetic. |
| Autosave, Save As, Recent, recovery | **PASS** | Autosave → `kill -9` the dev server → restart: `recovery` = `{found: true, brushes: 10, entities: 5}`. `recover` brings back all 10 brushes and the worldspawn `message`, marked unsaved. Save as / Recent: `editor-bigmap` (26.9 MB `.lsmap` of br_valley, reopened whole). |
| Editing tools, odd inputs, storms, small windows | **PASS** | 36 odd commands, each refused in plain words: a box past ±1e9 ("outside the map"), 0-thick, a 0.01 box, an empty / 40-character / Unicode texture name, scale 0, a 100000-copy array, a 100 KB key value, a `../../evil` class, junk paste, grid 0. A min>max box is fixed up. Nothing crashed. Undo/redo storm: 60 arrow nudges, then 80 × ⌘Z, then 80 × ⌘⇧Z, with the result exactly as before (both engines). 40 concurrent undo/redo/move requests: the page and server agree, with no errors. 30 rapid layout switches, 80 view switches and 8 rapid Saves: no page errors. Small windows: no sideways scroll down to 480×320. Usable at 1024×680 (Quad panes 291×86; the Content dock is taller than the views). At 800×500 the panes are 70 px tall, but the app's window can't be smaller than 820×600 (main.rs). The editor suites: WebKit 49/50 in the full order (A-4 is the 1) and 50/50 with bigmap alone; Chromium 50/50. Perf specs 5/5 in each. |

## Rust tests (shared target, 314c7f47)
- `editor`: 155/155 (4 ignored).
- `compile`: 32/32.
- `mcp`: 39/39.
- `shapes`: 9.
- `rmf`: 9, plus `real_rmf` on the real file.
- `jmf`: 2.
- `prefab`: 11.
- `selection`: 1.
- `agent`: 9.
- `assets`: 14.
- `credits`: 9.
- `content`: 10.
- `leak`: 5.
- `budget`: 3.

## Notes for the harness (not product bugs)
- The dev server answers every "save" dialog with `LS_DEV_PICK_SAVE`. Prefab Export uses `LS_DEV_PICK` instead, so a
  prefab export overwrote my scratch copy of br_valley with a zip. Point the two at throwaway files. The maps-src
  original was not touched.
- Wine turns `-game czero` into `C:\…\czero` when the shell's working directory has a folder of that name. Start sims
  from outside the Sim folder.
- A hand-started server without `+ip 127.0.0.1` binds the LAN address, and rcon to 127.0.0.1 then times out.

## Cleanup
Both sims were stopped by their PIDs, along with their leftover wine wrappers, which were mine. Sim/QA4A and Sim/QA4AZ
were deleted. My Compiler/work folders were removed: qa4a_*, con, NUL, aux, com1, `-` and `_`. ls_outpost's work
folder now holds my recompile. My dev servers, Vite instances and untracked configs are gone. The `winedbg --auto 208`
process is someone else's and was left alone.
