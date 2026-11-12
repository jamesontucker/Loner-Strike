# Tools crew notes

## Requests

- **Adding a tool to the Tools page (everyone):** append ONE line to `app/ui/src/views/tools/registry.tsx`:
  `{ id, label, blurb, icon?, beta?, load: () => import("./<tool>/<Page>") }`. The page is a default export
  taking `ToolProps` (`{ maps }`), loaded on first open in its own chunk. Don't edit Tools.tsx. The nav focus
  `"<id>:<arg>"` opens your tool; read the arg with `useToolArg()` from `stores/tools.ts`. Keep three.js out of
  your page unless it's a 3D tool. (Spray maker is in; the D request below is done.)

- (B, UI) `main.rs::mac_menu`: a "Tools" item under Go sending `section:tools` (the page already
  answers that event). Owner: agent A / whoever owns main.rs.
- (B, UI) Maps page map sheet: an "Open in Tools" button calling `go("tools", "map:" + name)`. The
  Tools page already opens a map from that focus string. Maps.tsx isn't mine.

- (D, spray maker) Tools.tsx + `stores/tools.ts`: a "Spray maker" tool in the tools row. Add `"spray"` to
  `Tool`, `spray` / `Spray maker` to the Segmented values/labels, and render
  `<SprayMaker />` (lazy is fine: `import("./spray/SprayMaker")`, default export) when `tool === "spray"`.
  Focus string `spray` could open it too. It needs nothing else (no three.js); its CSS comes with it
  (`spray/spray.css`). Owner: agent B.

## Progress

### Agent A (Rust core: goldsrc readers + map routes) — M1/M2 server side done

**Built**
- `goldsrc/mod.rs` (LE readers, `Error` with plain words, caps: BSP 64 MB, WAD 256 MB, texture side 1024,
  `PARSER_VERSION`, `png()`), `bsp.rs` (all viewer lumps, every offset checked, `read_lumps` reads single lumps
  from disk, a test fixture builder: cube room + func_wall), `entities.rs`, `wad.rs` (WAD3 + miptex → RGBA,
  `{` index 255 → alpha 0), `lightmap.rs` (engine extents, style 0, shelf-packed pow2 atlas, 1 px edge padding,
  4×4 white block for unlit faces), `mesh.rs` (faces → triangles, LSMB writer `write_bundle`, shared for C's bundles).
- `tools/mod.rs` (`local_routes()`, merged on `Listener::Local` in server.rs, + `overlays::routes()`),
  `tools/cache.rs`, `tools/mapinfo.rs`.
- Real maps: `LS_TEST_GAME` over all 391 cstrike/czero maps here: no errors, no panics, every map has
  triangles; de_survivor 21k triangles, 1024² lightmap, 95/95 textures, ~22 ms parse+build (release).

**Contract details B should know (all in the bundle header too)**
- Section `offset` is absolute from the bundle start (4-aligned), `length` in bytes, `stride` = components,
  plus `count` = elements. JSON is padded with spaces up to the data.
- `indices` are global (into the whole vertex arrays); draws are ranges of `indices` (`start`/`count` in
  indices, for `addGroup`), sorted by model then texture name. Each model also has `{index, start, count,
  entityIndex, classname, origin, mins, maxs, renderMode?, renderAmt?}` in `models[]`.
- **`origin` per model:** brush entities built around an origin brush (rotating doors, func_rotating) sit at 0,0,0
  in the buffers; translate that model's group by `origin`. World is always [0,0,0].
- Triangles are counter-clockwise seen from the front (three's default FrontSide), whatever the map stored.
- `flags` is a number; bits in `flagBits`: sky 1, water 2 (`!`/`*`/`water…`), trigger 4 (`aaatrigger`, clip/null/
  origin/hint/skip/bevel, or any `trigger_*` entity's model), transparent 8 (`{` textures).
- `uv` is already divided by texture size; `uv2` 0..1 into `/lightmap.png` (RGB, raw: apply the ×2 overbright/gamma).
- Errors: JSON routes (maps, info, entities) answer 200 `{error}` (400 for a bad name), like the rest of the app.
  Binary routes (mesh, lightmap.png, tex) answer `{error}` JSON with 404 (map/texture not there), 422 (damaged),
  400 (bad name), 409 (no game set up). Binary answers carry `ETag` + `Cache-Control: no-cache`; 304 on match.
- `/tex/<name>.png`: name case-insensitive, URL-encode it (`%7Bgrate.png`, `%21water.png`). 404 → draw the checker.
- `/info`: `{name, game, size, bounds, models, brushEntities, entities, classes{}, textures[{name,width,height,from}],
  missingTextures[], wads[{name,found,folder?,error?}], missingWads[], spawns{t[],ct[]} ({origin,yaw}), message,
  skyname, title}`; `from` is "embedded", the WAD's name as the map lists it, or "missing".
- `/maps`: `{maps:[{name, game, bspSize, nav:{version,places,bspSize,matchesMap}|null, lsv:{stage,done}|null,
  missingWads[], wads[], title, cat, source, other?}]}` (lsv key check and recordings are left to agent C).

**Security (from the review)**: embedded textures keep only their own record (40 + mips + palette), at most 64 MB in
all; mesh output capped at 4M vertices / 12M indices, ≤ 64 edges per face, and surfedges walked ≤ 4× the lump
(→ "damaged"). Tests: `embedded_textures_copy_only_their_record`, `overlapping_faces_are_capped`.

**How to try it**
- `cd app/src-tauri && cargo test goldsrc:: tools::` (fixture room, WAD with `{grate`, router tests incl. 304,
  bad names, damaged map, not on the LAN listener).
- Real maps: `LS_TEST_GAME="$HOME/Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life" cargo test --release real_maps -- --nocapture`
  (add `LS_TEST_DUMP=<folder>` to save each lightmap atlas as PNG).
- With the app running (`npm run tauri dev`, port shown in the app; the guard wants Host 127.0.0.1:<port>):
  `curl -s 127.0.0.1:<port>/api/tools/maps | head -c 600`, `.../api/tools/map/de_dust2/info`,
  `curl -s -o /tmp/d2.lsmb .../api/tools/map/de_dust2/mesh`, `.../lightmap.png`, `.../tex/%7Bfence.png`, `.../entities`.
- Cache: `~/Library/Caches/Loner-Strike/tools/<map>/<size>-<mtime>-p<version>/` (Windows: `%LOCALAPPDATA%\gg.lonerstrike.app\cache\tools`).

**Left / notes**
- Not done: lightmap styles 1-3, animated (`+0~`) textures, sky box images (only `skyname` in info).
- Build needed empty `app/src-tauri/resources/linux/{ls-voice,ls-intent}` and `resources/windows/hamsandwich`
  folders in this worktree (they're untracked binaries in the main checkout); I created them empty, not committed.
- Texture lookup order: embedded → worldspawn's WADs in their listed order, each searched by base name in
  cstrike, valve, czero (CZ game: czero, cstrike, valve), then the `_downloads` folders. A texture in a WAD the
  map doesn't name counts as missing (as in the game).

### Agent B: map viewer UI (M1-M3 client side) — 2026-11-12

**Built** (all in `app/ui`):
- Nav: `tools` in `store.ts` Section/SECTIONS; `App.tsx` NAV entry under a "Workshop" group, after
  Hosting, only on this computer (`ON_THIS_COMPUTER`), with a small **Beta** tag; lazy page
  `lazyPage(() => import("./views/tools/Tools"))`; wrench icon in `icons.tsx`.
- `views/tools/Tools.tsx`: SectionHead with the Beta tag and the line "New in 1.0: look inside maps
  and watch the bots. More tools are on the way."; tool switch (Map viewer | Replay placeholder);
  opens a map from the nav focus `map:<name>`.
- `MapViewer.tsx`: picker built from the app's `MapCard`s (with search); then the viewer: map name
  and "Other map", a camera switch (Orbit / Fly / Top-down), banners for missing textures and
  missing WADs (from `/info`), the Canvas filling the page height, a 300 px right column (Layers,
  Inspector) and a status bar (camera x y z and heading in GoldSrc units, what's under the mouse
  and where, a hint for the current camera).
- `viewer/bundle.ts`: LSMB decoder with zero-copy typed arrays (checked: views share the fetched
  buffer); rejects damaged bundles with "This map's 3D data couldn't be read…" (short, bad magic,
  truncated, bad JSON length, index past the vertices, draw past the end, newer version). It also
  works out whether triangles wind CW against the normals and flips the side drawn if they do.
  `encodeBundle` is the JS reference writer (the dev mock uses it).
- `viewer/materials.ts`: one PNG per texture (repeat wrap, mipmaps, anisotropy up to 8, `flipY`
  off), magenta checker when a texture won't load, lightmap atlas through `uv2` (three's `uv1`)
  with overbright x2 in a non-colour-managed canvas (`linear flat`), so the numbers match the game.
  "Lighting" off swaps to a simple sun (Lambert). Sky is a flat colour, water 60 % see-through,
  `{` textures alpha-tested, tool textures/triggers only with the Triggers layer. Walls are drawn
  from the front only, so from outside you look into the map (the sky lid doesn't hide it).
- `viewer/World.tsx`: one geometry per model sharing the bundle's buffers, a group per draw;
  brush entities placed at their entity `origin` (A's vertices are raw BSP, origin not baked);
  `trigger_*`/zones under Triggers & zones, other `func_*` under Doors & breakables; picking skips
  hidden draws and sky; hover shows texture and the point.
- `viewer/Entities.tsx`: point entities as one instanced mesh (spawns player-sized, T orange /
  CT blue, hostages green, lights tinted by `_light`, items, sounds, markers); picked entity gets an
  outline (point) or a tint (brush), plus lines to what it targets / what targets it.
- `viewer/Cameras.tsx`: Orbit (drei OrbitControls, damping), Fly (click grabs the mouse with
  pointer lock, falls back to drag-to-look; WASD/arrows, Shift x3; click picks at the crosshair
  while flying; Esc lets go), Top-down (orthographic, north up like the Python PNGs, fits the map).
  F / O / T switch; Esc clears the pick. "Look at" from the Inspector moves any camera.
- `viewer/Scene.tsx`: `frameloop="demand"`, dpr <= 1.5, one root group rotated -90 deg about X;
  pointer events measured with `getBoundingClientRect` so the app's text-size zoom doesn't throw
  picking off. WebGL is checked before the Canvas ("Your system can't show 3D"); a lost context
  or a crash inside the 3D view shows "The 3D view stopped" with Try again.
- `panels/Layers.tsx` (Textures, Lighting, Spawns & items, Doors & breakables, Triggers & zones,
  each with a "?"; remembered per device), `panels/Inspector.tsx` (classname, keys,
  target/killtarget/master/netname values link to the named entity, "Set off by" list, Look at,
  Copy origin/centre, surface texture + point; with nothing picked a searchable entity list).
- `toolsApi.ts` (+ `stores/tools.ts`): TanStack Query hooks; the mesh read shows "Reading the
  map…" in the top bar (activity store) and as a spinner over the view; `{error}` with 200 from
  the JSON routes is treated as an error; binary routes' `{error}` text is shown as is.
- Dev-only mock (`mock.ts`, loaded only when `import.meta.env.DEV`, not in the release build): a
  test room (textures, lightmap, water, fence, crate, trigger, spawns) used when the tools routes
  don't answer (no route / app not running), or with `?mock` / `localStorage ls.toolsMock = "1"`.

**Bundle sizes** (`npm run build`): start chunk `index-*.js` 509.17 kB (163.38 gz) before ->
509.68 kB (163.59 gz) after (+0.5 kB: nav entry, icon, lazy stub). CSS 102.2 -> 105.4 kB. New
lazy chunks: `three-*.js` 687.7 kB (177.3 gz), `Tools-*.js` 189.0 kB (62.4 gz: R3F + drei + our
code). Only `three` is a manual chunk: naming R3F/drei made Rollup move React (and a CJS helper)
into the 3D chunks and preload them at start, so they ride in the Tools chunk instead. Pins as
planned: three 0.170.0, @types/three 0.170.0, @react-three/fiber 8.17.10, @react-three/drei
9.117.3 (install and build clean; drei pulls zustand ^5, deduped with the app's).

**How to try it**
- Real data: run the app, open Tools in the sidebar (Workshop, Beta), pick a map.
- UI alone: `cd app/ui && npm run dev`, open `http://localhost:5173/?mock`. With no app on
  :8027 the shell itself needs `/api/setup/scan`, `/api/app`, `/api/state`, `/api/maps`,
  `/api/servers` (a 20-line stub server is enough; set `localStorage ls.setupLater = "1"`).
  Any map then opens the test room.

**Contract notes / questions for A**
- Checked against `goldsrc/mesh.rs`: header, sections (absolute offsets, byte lengths), draws
  (index units, flag bits, `entityIndex`) match what the decoder expects. The decoder ignores
  `count`/`models`/`textures`/`lightmap`/`flagBits` for now (it computes bounds from positions).
- Brush models: the UI adds the entity `origin` to model vertices (as BSP stores them). If the
  writer ever bakes origins in, tell me and I'll drop the offset.
- Texture names go in the URL with `encodeURIComponent` (`{fence` -> `%7Bfence`, `!water` stays).
- Not yet tried on a real dust2 bundle in the browser (only the mock + decoder tests); that's the
  first thing to check once the app runs from this branch.

**Left**: test on real maps (dust2, de_survivor, a surf map, a missing-WAD map, CZ) in the Tauri
webview, including pointer lock in WKWebView; the Replay tool (M6) and the nav/knowledge layers
(M4/M5) UI; the Go menu item and "Open in Tools" (requests above); site docs/CHANGELOG.


### Agent C (Rust overlays: nav, knowledge, probe, replay) — M4/M5/M6 server side done

**Built**
- `goldsrc/nav.rs`: .nav reader for versions 1-5 (areas with both corners and the ne/sw heights,
  connections per direction, hiding spots, approach and encounter data kept, places) and a v5 writer;
  real navs (fixed cs_rats2, de_dust2) write back byte for byte. `z_at`, `closest_point`, `area_at` (the
  game's GetNavArea), `one_way_links`, `dangling_links`. `build_ladders` redoes the game's BuildLadders
  from `func_ladder` boxes with our own traces (facing, top/bottom pulled past blockages, bottom area,
  forward/left/right/behind top areas, top height, dangling).
- `goldsrc/hull.rs`: hull 0 (point, from nodes + leaves) and hull 1 (player, clip nodes) traces, a port of
  surfsim.py's Hull, with solid brush entities (func_wall/door/breakable...) by origin; `floor_below`,
  `ceiling_above`, `contents`. Implements `nav::LineTrace` for the ladders.
- `goldsrc/lsv.rs`: .lsv v6 reader + writer, the brain's own checks (exact length, indices in range,
  counts clamped, an angle past the areas ends the list) and `nav_key` (LsNavKey). Sizes asserted:
  head 44, pair 6, area 315. Real files: keys match their navs (cs_rats2 fixes, de_dust2, de_nuke), and the
  fixed cs_rats2 knowledge does NOT match the unfixed nav.
- `tools/replay.rs` (declared from overlays.rs with `#[path]`, see Requests): recordings stream-parsed
  into per-player columns, rounds, kills with `cause` enemy/world/trigger_hurt/suicide (+ `teamKill`),
  falls (world deaths faster than 580 u/s down, with where it last stood and the drop), stuck spans
  (< 20 u/s for 8 s, no enemy in view, in play), 128-unit heat grids (seconds, deaths, seconds stuck),
  hits, shots, events, chat. Trace files: per-bot rows (24 columns, commas in why/look handled), #why,
  #attack, #seen, #mark; split into map visits (#map or the clock going back) and the visit whose bots
  stand where the recording has them is kept (same game clock, offset 0). 200 MB cap with a plain
  message; lines over 4 KB skipped; trace rows capped per visit.
- `tools/overlays.rs`, `routes()`:
  - `GET /api/tools/map/:name/nav` JSON: `{version, bspSize, matchesMap, places[], areas[{id, attr, nw, se,
    neZ, swZ, conns[4][ids], hiding[{id,pos,flags}], place (1-based, 0 none), approach, encounters}],
    ladders[{entity, top, bottom, length, dir, bottomArea, topForward, topLeft, topRight, topBehind (area ids
    or null), dangling}], oneWay[[from,to]], danglingLinks[], attrBits, spotFlags, dirs, warning}`.
  - `GET /api/tools/map/:name/know` LSMB bundle (A's `write_bundle`), header `{kind:"know", stage,
    stageName, done, count, navCount, key, navKey, keyMatches, warning, traces, spentSeconds, pairs, bangs,
    materials{code: name}, objectTypes[], stages[], units{}}`; sections (count = areas unless said):
    `sight` u8 (count² bits row-major), `pairA/pairB` u16, `pairQuality/pairFlags` u8, `bangA/bangB` u16,
    `bangThick/bangMat` u8, `coverLow/coverHigh` u16×16 (units), `coverMat` u8×16, `ceiling` u16 (units),
    `sees` u16, `high` u8, `cornerCount` u8 + `corners` f32×32 (8 × x,y,z,yawDeg), `angleCount` u8 +
    `angles` f32×40 (8 × yawDeg, pitchDeg (+up), dist, weight, areaIndex), `objectCount` u8 + `objects`
    f32×48 (8 × type,x,y,z,top,mat), `exposedStand/exposedCrouch` u16×8, `pointCoverLow/High` u16×9.
    Indices are nav file order. Warning text when the key differs: "Knowledge was built for a different nav…".
  - `GET /api/tools/map/:name/probe?x&y&z&dir=down|up` → `{hit, dir, z (feet height of the floor, or head
    height of the ceiling), normal, fraction, contents}` or `{hit:false, startSolid, contents}`. x/y/z = feet.
  - `GET /api/tools/recordings` → `{recordings:[{id, file, map, when, size, hasTrace, seconds, bots,
    players, kills, rounds, error?}], folders[]}` from `<app dir>/tools/recordings/`, the game's
    `addons/amxmodx/data/simrec/`, and files opened this session.
  - `POST /api/tools/recordings/open` (needs `x-panel: 1`) → dialog → `{id, recording}` or `{cancelled}`.
  - `GET /api/tools/recording/:id` LSMB bundle: header = `{kind:"replay", id, file, map, hasTrace, seconds,
    bots, players[{id,name,bot,team,start,count}], rounds[{start,end}], kills[{t,killer,victim,headshot,
    weapon,cause,team_kill,at,fall{from,from_t,drop,speed}}], hits, shots, events, chat, stuck[{id,from,to,at}],
    heat{cell,x0,y0,w,h}, falls, start, end}`; sections `t x y z vx vy vz yaw pitch` f32, `hp duck ground
    weapon venemy team` u8, `keys` u32 (all players back to back; slice with start/count), `heatTime` f32,
    `heatDeaths` u32, `heatStuck` f32 (w×h row-major).
  - `GET /api/tools/recording/:id/trace` → `{map, file, offset:0, matched, rows, matches, truncated,
    columns[24], tasks[20], bots[{id, rows[[...24 values]]}], why[{t,id,team,at,why}], attack[...],
    seen[...], marks[...]}`.
  - Errors: JSON routes 200 `{error}` (400 bad name/id/point); binary routes `{error}` with 404/413/422.
- Security pass (coordinator's review): ladder boxes must be finite and inside ±32768, the pull-in loop is
  an integer step count capped at 1000 (`crafted_ladder_boxes_end`); hull recursion and contents walks stop
  at depth 1024 (looping trees → solid, `damaged_maps_are_errors`); nav counts are checked against the bytes
  left before allocating; .lsv sizes are checked exactly before anything is read by them (≤ 6000 areas);
  recordings/traces: total and per-line caps, ids ≤ 64, heat grid bounded by the world size, trace rows and
  notes capped per visit.

**How to try it**
- `cd app/src-tauri && cargo test nav lsv hull overlays` (fixtures: a 3-area nav, a box room BSP with a
  ladder and a func_wall, a built .lsv, a rats2_v5 recording + trace snippet in `tools/testdata/`).
- Real files: `LS_TEST_SIM=/Users/jameson/loner-strike-sim LS_TEST_GAME="$HOME/Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life" cargo test -- nav lsv hull overlays`
  (navs round-trip, .lsv keys match, de_dust2 floors match its nav, de_nuke ladders build, the whole
  W1-1 rats2_v5 recording: 63 kills, falls found, its trace matches).
- App running: `curl -s 127.0.0.1:<port>/api/tools/map/cs_rats2/nav | head -c 400`,
  `.../map/cs_rats2/probe?x=0&y=0&z=100`, `.../recordings`. To see the fixed rats2 nav, copy
  `loner-strike-sim/navpack/fixes/cs_rats2.{nav,lsv}` into cstrike/maps. To list a sim recording, copy it
  (and its `trace_*.csv`) into `~/Library/Application Support/Loner-Strike/tools/recordings/`, or use Open.

**Requests**
- (A) `tools/mod.rs`: add `pub mod replay;` and I'll drop the `#[path = "replay.rs"]` line in overlays.rs
  (it's `crate::tools::overlays::replay` until then). overlays.rs uses mod.rs's private `Fail`, `Done`,
  `blocking`, `find_map`, `locate`, `json_answer` — please keep those names (or tell me).
- (A) `/api/tools/maps` could show the .lsv key check: `lsv::nav_key(&nav) == Lsv head.key` (bytes 8..12 of
  the .lsv) — needs the full nav read; fine for the cached facts.
- The plan says navs.rs/navpack.rs should switch to `goldsrc::nav` for parse_head/place_count; not done
  (not my files).

**Left / notes**
- Ladders: BuildLadders' GetGroundHeight samples several start heights; mine traces once (only used for a
  fallback nearest-area pick). de_nuke's two vent ladders find no top area (they end under breakable grates,
  which the game's traces hit too, I believe; unverified in game).
- Nav coverage readout (share of upward floor faces near an area) isn't in the nav route yet — it needs the
  BSP faces (A's bsp.rs); easy to add as `coverage` there.
- Recordings: the map name comes from the file name (longest tail that's an installed map); the trace's
  `#map` is in the trace answer. Player ids reused after a reconnect keep the last name.

### Agent D (spray maker) — done, waiting on the Tools.tsx entry (request above)

**Built**
- `goldsrc/wadwrite.rs`: picture → spray → `tempdecal.wad`. Median-cut palette (pure Rust), optional
  Floyd-Steinberg dithering, fit to a valid size keeping the shape (see-through padding), Lanczos resize with
  premultiplied alpha, see-through from the picture's alpha / the corner colour / none, one-colour sprays.
  Reader too (`read_wad`), used for "Spray now" and the round-trip tests.
- `tools/spray.rs` (`routes()`, merged in `tools::local_routes()` with `pub mod spray;` — A's routes were in):
  `GET /api/tools/spray` (games found, the spray in each as PNG, read-only, can put back, the chosen picture),
  `POST .../pick` (system file dialog, like /api/gb/add-file; the page never names a path; the picture is kept in
  memory), `POST .../preview` (choices → PNG data URL as the game draws it, size, colours),
  `POST .../install` ({choice, games, lock}), `POST .../restore` ({game}). POSTs need X-Panel; local listener only.
  Pictures: PNG/JPEG/BMP/TGA/WebP, ≤ 40 MB, ≤ 8192 px a side, damaged/odd files give plain words.
- `views/tools/spray/` (`SprayMaker.tsx`, `sprayApi.ts`, `spray.css`): choose a picture, live preview on a CSS
  brick wall (2x, multiply blend), Size (Small/Medium/Large, default Large), Full colour / One colour (+ paint
  colour, paint light parts), See-through (picture's own / plain background / none), Smooth photo colours,
  Install for (each game found), Keep it from being replaced (default on), Install with busy state and
  "Spray installed. Press T in game to spray it.", and a "Spray now" panel per game with "Put back the previous one".

**The format, as verified** (the game's own czero/ and cstrike/ tempdecal.wad here, decals.wad, community guides)
- WAD3, one lump of type 0x40, miptex with 4 mips + 256-colour palette, padded to 4 bytes. The game names its lump
  `LOGO`, community tools `{LOGO`; only the first lump is read. We write `{LOGO`. Our 64x64 output is
  byte-for-byte the game's layout (6296 bytes); the game's files read back and re-write identically.
- Palette colour 255 decides the look: pure blue (0,0,255) = colour spray, index 255 see-through; anything else =
  one-colour ("indexalpha"): every pixel in colour 255 with the index as opacity (the game's own file: colours 0-254
  black, 255 = cl_logocolor orange, background index 0). So colour sprays always get blue at 255, and a one-colour
  paint of pure blue is nudged to (0,0,254).
- Size: sides multiples of 16, ≤ 256, width×height ≤ 12288 (guides say 12288 or 14336; we use the smaller).
  Large = 96x96 square, 128x96 for 4:3, 144x80 for 16:9.
- The game rewrites tempdecal.wad from `logos/<cl_logofile>.bmp` when a spray is picked in Options > Multiplayer.
  The community makes the file read-only; we do too by default (unix 0444 — CrossOver/Wine sees that as read-only;
  Windows read-only attribute), and clear it before the next install. A changed spray reaches others on the next
  server join (it's uploaded as a custom resource).
- Backups: the replaced file goes to `<app dir>/sprays/backups/<game>-<ms>.wad`, 10 kept per game. "Put back"
  restores the newest (keeping the read-only state) and uses that copy up, so pressing it again goes further back.

**How to try it**
- `cd app/src-tauri && cargo test wadwrite` and `cargo test tools::spray` (size rules, palette, quantiser, round trip, game layout, install
  with backup/read-only/restore in temp folders, damaged files).
- Real files: `LS_TEST_GAME="$HOME/Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life" cargo test real_sprays -- --nocapture`.
- A picture as every kind of spray: `LS_SPRAY_SAMPLE=<picture> LS_TEST_DUMP=<folder> cargo test sample_picture -- --nocapture`.
- In the app (once the Tools entry is in): Tools > Spray maker > Choose a picture > Install spray, start the game,
  join a server, press T.

**Left / not done**
- Not seen in a real game yet: it needs the Tools entry, then a check in CS and CZ (spray shows, colours right,
  read-only holds after opening Options).
- No crop tool (the picture is fitted whole); no drag-and-drop (the page can't name files by design).


### Agent E: map editor core (2026-11-12)

**Built** (`app/src-tauri/src/editor/`, mounted with `pub mod editor;` in main.rs; pure Rust, no routes or UI)
- Map document (versioned project JSON): brushes as three-point planes with Valve 220 texture axes,
  entities with ordered key-values, groups, materials (texture sizes), world settings (WAD list + worldspawn keys).
- Geometry: plane sets to face polygons, validity (closed, convex, no degenerate or duplicate faces), bounds,
  volume, convex hulls.
- Operations: box, cylinder, move/rotate/scale/mirror with texture lock, push/pull a face, clip, hollow,
  carve, merge (convex hull, says when it isn't exact), snap to grid; texture fit/centre/shift/scale/rotate,
  align to world/face; `brush_mesh` gives polygons + UVs for drawing.
- .map read/write: Valve 220 and the old standard format in, Valve 220 out, TrenchBroom groups, readable
  errors, byte-stable round trip.
- Undo/redo: transactions that commit as one step or roll back, drags fold into one step, saved/dirty state,
  and ~20 editor commands (`history::cmd_*`).
- Checks: GoldSrc limits, broken brushes, outside the world, missing textures, spawns, dangling targets.
- docs/TOOLS-EDITOR.md: the compile path (SDHLT as an optional download; on a Mac its Windows exes run
  through the CrossOver wine we already use for HLDS), what's realistic for 1.0.0, and the UI-to-core map.

**Try it**: `cd app/src-tauri && cargo test editor::` (38 tests).

**Left**: routes (proposal in TOOLS-EDITOR.md §3.1, to agree first), a face-picking helper, vertex editing,
FGD parsing, prefabs, BSP "trace over" import, compile runner + leak line from `.lin`.

**Requests**: none yet. The UI agent needs the editor routes agreed in TOOLS-PLAN §3.8 before building.

### Agent B: real routes, overlays (M4-M6 UI), registry — 2026-11-12, later

**Run the dev app from this worktree** (builds the UI, then the app with it; first build ~1 min):

    cd ~/loner-strike-tools/app/ui && npm install && npm run tauri dev

It's the real app (same settings and app folder as an installed Loner-Strike; if the installed one is open it
takes another port). Tools is in the sidebar right after Servers, tagged Beta. `cargo build` of this branch
passes (checked 2026-11-12).

**Checked against real data** (bundles made by A's/C's own Rust code in a scratch harness, served at the real
routes): de_dust2, cs_rats2, surf_ski_2 meshes with textures and lightmaps; the fixed cs_rats2 nav + .lsv;
de_dust2 nav + .lsv; the rats2_v5 W1-1 recording (12 bots, 63 kills, 13 falls) with its trace. Decoder checks
in `bundle.ts` hold: zero-copy views, CCW winding detected, damaged bundles rejected with plain words.

**Changes to match the contract**: brush models placed by the bundle's `models[].origin`; sky faces drawn only in
Fly (sky volumes have faces on both sides, so from above they'd hide the map); water front-only (it has a face
per side); JSON routes' 200 `{error}` treated as errors; a 404 that carries `{error}` is the app's words, not a
missing route (only a bare 404 brings in the dev mock).

**Overlays (first pass, all in the Map viewer and Replay)**
- Layers: "Bots' nav" switch, then Colour by (connected or cut off / place names / crouch and jump spots) and
  "What bots learned" (Cover, What they see, Wallbangs, Corners, Angles they watch), with plain help lines,
  loading spinners, and the route's own warnings (no nav, nav for another map version, knowledge for a different
  nav, still learning).
- `viewer/NavLayer.tsx`: tiles on the four corner heights (islands red; ladders count as links), area outlines,
  links (one-way orange with arrowheads), hiding spots (sniper spots yellow), ladders, place names (DOM labels,
  no font download). Click an area: Inspector shows size, heights, moves, group, hiding spots, links per
  direction (click to go), one-way in/out, and the area's knowledge facts.
- `viewer/KnowLayer.tsx`: cover rose (16 directions, crouched yellow / standing blue), sightlines from the picked
  area (yellow = sees even a crouching player), wallbangs by material, corners with opening direction, watched
  angles by weight; area tints for cover and "how much it sees".
- Replay tool (`Replay.tsx`): recordings list (map, date, length, bots, kills, "with bot thoughts") + "Open a
  recording…" (the app's dialog, busy spinner); opens the recording's own map. `panels/Timeline.tsx`: play/pause
  (Space), speed 1-16×, scrubber with round, kill and fall ticks, players (pick; Follow keeps the Orbit camera
  on them), "Just now" feed and a list of every fall/map death (click jumps 2 s before, looks at the spot).
  `viewer/ReplayLayer.tsx`: hull boxes (stand/duck) in team colours, view lines, 15 s trails, names, kill crosses,
  fall crosses with a line from where the bot stood, heat map (time / deaths / stuck, 128-unit squares).
  Picked bot: the trace row at that moment (state, task, why + age, look, enemy), its last notes, and its goal
  and heard noise drawn in yellow.

**Registry**: `views/tools/registry.tsx` (see Requests). Each tool is its own chunk now.

**Sizes now**: start chunk 509.63 kB (163.57 gz), unchanged from before Tools. Lazy: `three` 687.7 kB,
`MapViewer` 232.4 kB (R3F + drei + viewer + overlays), `Tools` 3.7 kB, `Replay` 3.4 kB, `SprayMaker` 7.9 kB.

**Acceptance (plan)**: to see the fixed rats2 nav in the app, copy `loner-strike-sim/navpack/fixes/cs_rats2.{nav,lsv}`
into cstrike/maps (C's note). To list the rats2_v5 recording, copy
`results/rats2_v5/W1-1_classic_cs_rats2_150726_cs_rats2_1794524249.csv` and `trace_W1-1_classic_cs_rats2_150726.csv`
into `~/Library/Application Support/Loner-Strike/tools/recordings/`, or use Open. Not yet looked at in the
real Tauri window (only Chromium via the dev server): pointer lock and WebGL in WKWebView still to check.

**Left / ideas**: nav coverage readout (needs C's `coverage`), exposure/objects/point-cover modes, a trace
timeline of #why notes, bot view cones as meshes, Replay heat by round.


## Decision: compiling with SDHLT (user + coordinator, 2026-11-12)
- Compiler: https://github.com/seedee/SDHLT, pinned to v1.3.1 (release asset `sdhlt-v1.3.1.zip`, 2026-10-16).
  Windows binaries only (sdHLCSG/sdHLBSP/sdHLVIS/sdHLRAD, x86 and x64). Licence: GPLv2 with the Valve SDK 2.3 EULA
  overriding where needed (and id's Quake 2 licence) — so we DON'T bundle it.
- Download on first compile from the official GitHub release, verify a pinned SHA-256, keep its LICENSE.md beside it,
  store under the app dir (e.g. tools/sdhlt/v1.3.1/). Windows: run the exes directly. macOS: run them through the
  app's existing CrossOver/Wine path (as it runs HLDS). Stream output to the editor; translate common errors (leak,
  limits, missing textures) into plain words.
- Separate task after the editor UI. Fallback if not ready: .map export + direct BSP edits.
