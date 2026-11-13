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

- (F, health check) `viewer/Cameras.tsx` LookAt: a `lookAt` request made before the viewer mounts is
  skipped (`done = useRef(req?.n ?? 0)` marks it as already done). The health check's "Open in map viewer"
  does `setTool("map"); openMap(map); look(spot, 192)`, so on a fresh viewer it opens the map but not the spot.
  Please honour a pending request once the map is ready (e.g. start `done` at 0 and run it after the
  first fit), or take a focus string like `map:<name>@x,y,z`. Owner: agent B.

- (T, community) `archive.rs` content allow-list: add `.lsv` to `maps/` (the bot brain's map knowledge, plain
  data). Publish packages ship `maps/<map>.lsv`, which the installer skips today, so bots rebuild knowledge on
  first play. Map editor / nav fixer: a "Publish…" action doing `go("tools", "community:publish:<map>")` after a
  save or compile. Map viewer: "Use as a screenshot for Publish". Details in docs/COMMUNITY.md section 8.

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

### Agent F: map health check — 2026-11-12

**Built**
- `tools/health.rs` (`routes()`, mounted in tools/mod.rs): the sim's sweep (sweep.sh + run-wine.sh +
  sweepreport.py) ported to Rust.
  - **Hidden server:** a copy of one of the player's own ready servers on this computer, for the game being
    played (so the run has our engine, bot brain, plugins and modes), kept in `<servers folder>/.health-check`
    and synced before each run (only files that changed by size/time; leaves out steamapps, maps, logs,
    simrec, demos, panel_rcon.txt, status.json). A marker names the server it came from; a copy of another
    server is cleared first. The map's .bsp + .nav (+ .lsv) come from the player's game (next to the map or
    in the other game's maps folder), plus models/sprites the map names. `gm_simrec.sma` (copied from the sim,
    in `tools/health/`) is compiled there once with the server's amxxpc (rebuilt when the source changes) and
    added to its plugins.ini.
  - **Run:** run-wine.sh's settings (current.txt, plugins-gamemode.ini from `<mode>.plugins.ini`,
    lastmap.txt, friends.cfg with `bot_join_after_player 0` and no limits, global.cfg `bot_quota`, a
    `health.cfg` exec'd at start); started like the nav learner (CrossOver/Windows, low priority, tied to the
    app) with `+ip 127.0.0.1 -nomaster -insecure +sv_lan 1`, a random RCON password (also in
    panel_rcon.txt) and `bot_quota` nudged over RCON 6 times. The minutes count from the first RCON answer.
    Every 10 s it checks the game (`sys::game_running`) and the player's local servers (`servers::running`)
    and stops itself if either started. Stop = RCON quit, then `stop_child`, then (Mac) `pkill` of the hidden
    hlds.exe. Afterwards the map, recordings, trace and password go (`tidy`), whatever happened.
  - **Report** (`build_report`, unit tested): over every recording the run left (the map loads twice at
    start), with agent C's `replay::parse_recording`: kills a bot-minute, stuck % (C's stuck spans: < 20 u/s
    for 8 s, no enemy, in play — sweepreport used 5 u/s), standing still with no enemy %, map used % (nav
    areas walked, every 5th sample), spawn sitters (alive 30 s into a round, never 300 units from the start),
    deaths to the world, falls (C's fall detection) and other map deaths, all grouped into spots by nav place
    name (else a 256/128-unit cell) with the place under or near the spot; console: crash lines, plugin
    errors grouped by plugin, map knowledge loaded/rebuilt/none. Findings in plain words, worst first ("Bots
    fall a lot near Shelf — 8 times in 1 minute.", "Bots get stuck near Sink: 4 min 51 s in all, 6 times.",
    "basebuilder.amxx had 500 errors…", "Only 3 of 12 bots joined…"), sweepreport's hard flags and mode
    exceptions (surf/deathrun/kreedz falls, hns/basebuilder standing still, few-kill modes), and its soft
    flags against the newest report of each other map of the same mode checked here (median + 3 MADs).
    Checked against sweepreport.py on the rats2_v5 W1-1 run: same 12/12 bots, 63 deaths / 14 to the world,
    4 of 77 spawn sitters, 1.16 kills a bot-minute, map used 42.3 % (sweepreport 43.4 %).
  - Saved: reports as JSON in `<app dir>/tools/health/<id>.json` (100 kept); the run's biggest recording as
    `<app dir>/tools/recordings/health-<id>_<map>_<n>.csv` + `trace_health-<id>.csv` (20 runs kept), so
    Replay lists it with its trace.
  - Routes (local listener, POSTs need X-Panel, errors `{error}` 200): `GET /api/tools/health/options?map=`
    (servers to copy, modes, the map's preset mode, has nav, why it can't run, busy), `POST .../start`
    `{map, mode, minutes 1-15, bots 2-20, server?}`, `GET .../progress` (`{run{phase, detail, ...}, active,
    elapsed, left, fraction}`), `POST .../cancel`, `GET .../reports`, `GET .../report/:id`.
- `views/tools/health/` (registered in registry.tsx, Beta): choose a map (search + list), mode (the map's
  preset mode picked), 2/5/10/15 min, 6/12/20 bots, which server's plugins (only when there are several); why
  it can't run in plain words (no nav: teach the bots first; no server: make one; game/server running). Run
  shows the Steam-style progress bar with what it's doing and time left, and Stop; you can leave the page.
  The report: verdict, the findings as big sentences with "Open in map viewer" per spot, the numbers as tiles
  with "?" help, then where bots fell / got stuck / the map killed them (place names, times, Open in map
  viewer), console errors, and "Watch in Replay". Earlier checks are listed below the form.

**How to try it**
- `cd app/src-tauri && cargo test tools::health` (snippet of the rats2_v5 W1-1 recording around three falls in
  `tools/health/testdata/`, synthetic navs for places, console lines from the sweeps, presets, cfg edits, the
  copy filter and sync, report save/prune). The whole run against sweepreport's numbers:
  `LS_TEST_SIM=/Users/jameson/loner-strike-sim cargo test real_sweep -- --nocapture`.
- In the app: make sure a server exists on this computer (Servers > New server) and the game is closed, then
  Tools > Map health check > pick cs_rats2 (copy `navpack/fixes/cs_rats2.{nav,lsv}` in first for the fixed
  nav), 2 min, Run. The first run copies the server and compiles the recorder (slower).

**Left / not tried**
- Not run end to end in the real app yet (the whole flow is built against how the sim and the nav learner
  work; first thing to check: the recorder compiles under CrossOver and the bots join).
- First copy of a big server on Windows is a full copy (APFS clones on a Mac); no hard links.
- No "check all maps" batch and no second-run confirmation like the sweep's (one map at a time for now).
- "Open in map viewer" needs the LookAt request above to land on the spot.


## Integration pass (coordinator, after the agents finish)
- **Sidebar:** Tools becomes an expandable sidebar section (like Servers) with sub-items from `registry.tsx`,
  grouped Look (Map viewer, Replay, Textures & models) / Fix (Map health check, Map doctor, Nav fixer,
  Callouts) / Make (Map editor, Spray maker); clicking "Tools" opens an overview page of cards. Each tool
  gets its own section/focus address. Registry entries gain a `group` field.
- Viewer honours a look-at/focus request made before the map has loaded (health check "Open in map viewer").
- "Tools" in the macOS Go menu (main.rs); "Open in Tools" on the Maps page's map sheet.
- Check C's ladder top areas (cs_rats2's 22 ladders show no top area; de_nuke vents).
- Shared files (tools/mod.rs, registry.tsx, goldsrc/mod.rs, content.rs): review every agent's lines.
- Then: two reviews, cargo test, npm run build + size check, real app check, merge `tools` into main.

### Agent G: map doctor + callout editor — 2026-11-12

**Built**
- `tools/doctor.rs` (`routes()`, mounted in tools/mod.rs): `GET /api/tools/doctor/:name` checks one map: the map
  file (A's full BSP parse), texture files and textures, every sky/model/sprite/sound the entities name
  (`mapcopy::entity_files`, the same rules as navpack/res.py), the .res (exists, files missing from it, lines for
  files not on this computer), the nav (exists, damaged, made for this map version, places) and the .lsv (stage,
  fits the nav by the brain's key), plus the GameBanana page the map came from (store records) and whether the
  game has it loaded. Files are looked for the way the game looks: own folder, `<game>_downloads`, cstrike (for
  CZ), valve; "elsewhere" means only in another game folder (czero, czeror, the downloads folders).
  `POST .../res` writes or updates `<map>.res` (custom files only: Valve's, CS's stock WADs, and files both CS and
  CZ ship are left out; hand-added lines for files that exist are kept). `POST .../copy {files}` copies files
  found only in another game folder into the map's game folder (never replaces). Both need X-Panel and refuse
  while the game has that map loaded. Writes: `.part` then rename, `.bak` the first time (`doctor::save_file`).
- `tools/callouts.rs`: `GET /api/tools/callouts/:name` (areas as `[id,x0,y0,x1,y1,z,place]`, places, the voice
  list from the game's BotChatter.db, chat callouts gm_botchat knows, knowledge fit, file stamp),
  `GET .../suggest?from=usual|other` (navpack/callouts.py's rules for de_dust2/de_inferno; navpack/names.py's
  name_areas from the same map's nav in the other game folder, filling unnamed areas only),
  `POST /api/tools/callouts/:name {stamp, changes:[{id,place}]}` (refused if the nav changed on disk since load).
  Names: letter then letters/digits/_ up to 31. The nav is rewritten with C's nav.rs, read back and compared (ids,
  corners, links, hiding spots, names, and the brain's LsNavKey) before the rename.
- UI: `views/tools/doctor/` (Doctor.tsx, MapList.tsx shared with callouts, doctorApi.ts, doctor.css) and
  `views/tools/callouts/` (Callouts.tsx, TopDown.tsx: a 2D canvas, calloutsApi.ts, callouts.css). Registered as
  "Map doctor" and "Callouts" in registry.tsx. Doctor: plain-language rows with fixes (Copy files, Make/Update the
  list, Teach bots this map via the existing `/api/navs/learn` with JobProgress, Name places → Callouts, GameBanana
  page or search, Get more). Callouts: top-down nav coloured by place with labels, click/Shift-click/drag-box/
  Alt-drag/double-click selection, floor range sliders, pan/zoom, name box with the voice + chat lists, places list
  (pick, rename), suggestions, undo/redo/put back, Save with busy state, unsaved-changes guard.

**Knowledge (.lsv) and place names** — the .lsv key is FNV over area count, ids and integer centres
(cs_bot_ls.cpp LsNavKey), so renaming places never invalidates it: the save checks the key is unchanged and says
the knowledge still fits. I did not add a "rebuild knowledge" button: the app's learner relearns the *nav* from
scratch (it would throw away the names), and the brain already rebuilds a non-matching .lsv itself the next time
the map is played. If a knowledge-only rebuild is wanted, it fits F's hidden-server runner (load the map with
bots, wait for stage 4).

**How to try it**
- `cd app/src-tauri && cargo test doctor callouts` (fixtures: missing/elsewhere/stock files, .res write/keep/drop/
  backup, copy refuses paths and existing files, CZ search order, nav+lsv fit, damaged map/nav; place rename round
  trip, bad names, usual rules, names carried over, save with stamp check and backup).
- Real game: `LS_TEST_GAME=<Half-Life folder> cargo test real_doctor -- --nocapture` (LS_TEST_MAPS=a,b) and
  `cargo test real_callouts -- --nocapture`.
- In the app: Tools > Map doctor / Callouts. Checked in the real dev app: routes answer for cs_rats2, cf_egypt,
  de_dust2, and the doctor page renders cf_egypt (missing sky, broken .res lines). The callouts page builds and
  type-checks but I didn't get to see it in the app (the shared dev build kept restarting as others saved files).

**Left / caveats**
- "Stock" is a heuristic (valve/, the stock WAD list, or in both cstrike and czero); a custom map using a stock-only
  CS file (as_oilrig's orange.mdl) gets it listed in the .res, which is harmless.
- No WAD embedding into the BSP yet (2a writer); missing textures point to copying the WAD or getting the map again.
- Git slip: my doctor commit (96db1b9) picked up E's then-uncommitted `editor` mount lines in tools/mod.rs, and my
  `--amend` to take them out landed on H's model reader commit (32abfb3), which now carries that small mod.rs fix.
  The tree is right; sorry for the noise in those two commits.

**Requests**: none.

### Agent H (texture & model browser) — done, Beta

**Built**
- `goldsrc/mdl.rs`: GoldSrc MDL v10 reader (clean-room, from the public format notes): bones, sequences
  (any frame's pose from the run-length animation data, motion bone kept in place, group files `<name>01.mdl`
  via `pose_with`), textures (masked/additive/chrome/fullbright flags), skin families, body parts, meshes from
  triangle strips/fans (flipped to three's counter-clockwise front), companion `<name>T.mdl` textures. Every
  count/offset checked and capped (bones 256, sequences 4096, textures 512 / 64 MB of pixels, 3M corners,
  2048 px texture sides); a bone's parent must come before it; a byte-flip fuzz test over the fixture.
- `goldsrc/wadwrite.rs` (appended, spray code untouched): normal miptex textures (`make_texture`: Lanczos
  resize to multiples of 16, ≤ 512, four mips each matched to one median-cut palette, `{` → index 255 + blue),
  `read_lumps` / `put_lump` / `write_lumps` for any WAD. All spray tests still pass.
- `tools/assets.rs` (`routes()`, merged in tools/mod.rs): `GET /api/tools/assets/wads`, `/wad/:folder/:wad`
  (texture list), `/wad/:folder/:wad/tex/:name.png` (disk-cached via cache.rs, ETag/304), `/usage` (per map:
  textures, embedded textures, WADs, entity models; from the texture+entity lumps only, memoised by the maps'
  stamps), `/models`, `/model?folder&path` (LSMB: posed `positions`/`normals`, `uv`, bone-space `local`/
  `localNormals`, `bone`/`normalBone` u16, `indices`; header: draws per mesh, textures, skins, bones,
  sequences, bodyparts, bounds), `/model/tex?…&i`, `/model/anim?…&seq` (frames × bones × [x y z qx qy qz qw]),
  `/model/info`. POST (X-Panel): `/export` (save dialog → PNG), `/texture/pick|preview|save` (your picture into a
  custom WAD in cstrike/; only WADs made here are ever rewritten, old version kept in `<app>/tools/wad-backups`,
  5 each; game WAD names refused), `/add-file` ({kind: wad|model}: dialog → parsed first → WADs and models that
  replace a game file go through Get more's `start_local_install` (record, backup, Remove there); a brand-new
  model is copied in as a new file, never over one, listed in `<app>/tools/imported_models.json`),
  `/model/remove` (only those). Folders: cstrike, czero, valve, *_downloads, from a fixed list; WAD names one
  plain part; model paths 2-6 `content::safe_part` parts under models/, resolved case-insensitively and
  canonicalised inside the folder.
- `content.rs` (3 one-line additions, shared file): a `textures` section (GameBanana CS 1.6 > Textures, cat 681):
  loose `.wad` files to the top of the game folder, never the game's own WADs. It also appears as a tab in the
  main Get more page (sections come from the server).
- UI `views/tools/assets/`: Textures (WAD picker incl. "Every WAD" and "Inside maps", search, lazy tile grid,
  big pixel view with size/source/special prefix, "Used by N maps" linking to the map viewer, Save as picture,
  Make your own with live preview), Models (search + kind filter, R3F viewer with orbit camera framing, skin and
  body-part choice, animation list with Play — CPU skinning per frame, wireframe, textures with Save, "Placed in"
  maps, Remove for ones added here), Get more (Add a WAD / Add a model + the app's GetMore with
  skins/players/textures). Registered in registry.tsx.

**Checked on the real game** (`LS_TEST_GAME=… cargo test --release real_assets real_models -- --nocapture`):
99 WADs, 10,907 textures decoded (2.3 s), 0 failures; 1,587 models listed, all built (2.06M triangles, 1.6 s);
triangle facing agrees with the normals 97% (63k of 2.0M disagree: models' own odd normals); usage index of
388 maps in 0.33 s. The 52 group files (`*01.mdl`) and T files are hidden from the list, not errors.
`cargo test` (443) and `npm run build` green; AssetBrowser chunk 31 KB (11 KB gz), main bundle unchanged.

**Not done / left**
- Not looked at in the running app yet (no screenshots): check the model framing, a skin switch on a hostage,
  a v_ model, Play on a player model, and a GameBanana texture install.
- Bone controllers, blending (only blend 0), attachments, hitboxes, chrome shading not drawn; animations step
  frame by frame (no interpolation).
- No "remove a texture from my WAD" yet.

**Requests**
- (H) GetMore.tsx: `SECTION_NAME` / `KIND_NAMES` have no `textures` entry, so its badge shows "textures";
  add `textures: "Texture pack"` / `textures: "Textures"`. Owner: whoever owns GetMore.tsx.

### Agent J: Nav fixer (Beta), 2026-11-12

**What's built**
- `tools/navfix.rs` (`routes()`, mounted with one line in tools/mod.rs). It keeps one editing session per nav, and every answer is the whole state:
  - `GET /api/tools/navfix/:name`
  - `POST .../edit` with `{op: add|delete|reshape|link|unlink|attr|place, ...}`
  - `POST .../undo`, `.../redo`, `.../save`, `.../reload`, `.../rebuild`
  - POSTs need X-Panel. Errors come back as `{error}` with 200 (400 for a malformed edit).
  - The state is C's `nav_json` plus `reach[]` per area (0 ok, 1 can't get there, 2 can't get back), `problems[]` (text, hint, area ids, a point, an optional one-click `fix` edit), dirty/undo/redo labels, the .lsv status and a rebuild estimate.
- **How edits work:** each edit is a pure function over a `Doc` whose areas sit behind `Arc`, so an undo step costs a list of pointers (300 steps kept).
  - *Add:* new id = max + 1. It takes the nearest named area's place (navarea.py's place_from). Corners are dropped to the floor with point traces: the middle first, and a corner that falls off a beam edge takes the middle's height.
  - *Link:* direction worked out exactly like navarea.py/navlink.py. A one-way link removes b → a.
  - *Delete:* also drops links, approach entries and encounter data that name the deleted areas or their hiding spots.
  - *Reshape:* moves hiding spots along when the area keeps its size.
- **Auto-check:**
  - Directed reach from the spawn areas, plus a reverse pass for traps. Ladders join their foot and top both ways.
  - Islands are grouped ("Bots can't reach these 12 areas from any spawn").
  - Also finds dangling links, links to self, duplicate ids, areas with no size, areas a standing player doesn't fit in (head room over the middle; crouch and jump areas skipped), and ladders with no area at an end.
- **Save:** writes `<nav>.part`, then renames it; keeps `<nav>.bak` the first time. It refuses while the game has the map loaded (`Game::state()` map) and refuses if the file changed on disk since it was opened. It re-parses what it wrote before replacing anything, and says when the .lsv no longer fits (`nav_key` changed). Link, attribute and place edits keep the key, so the knowledge still fits after those.
- **Rebuild bot knowledge:** a setup job (`knowfix`, shown with the app's JobProgress).
  - The game's own learner can't do it: the hidden server (navs.rs) runs Valve's mp.dll, which has no brain.
  - So it reuses `LocalLearner::new` + `prepare` (same folder, same download), swaps in the brain DLL from the player's game for the run (checked for "lonerstrike_mapknow"; Valve's DLL is put back however the run ends), copies in the map, its resources and the saved nav, and adds `bot_enable 1 / bot_ls_fight 1` to server.cfg.
  - It waits for "map knowledge for <map>:" while showing the stage from the .lsv header, then checks the result's key against the nav and installs it with .part + rename.
  - Estimate: 45 s + 11 s × (areas/737)² with nobody on (the brain's budget is 4 ms/frame then), e.g. "about a minute" for cs_rats2.
  - Not run end to end here (it needs the hidden server); see Left.
- `views/tools/navfix/` (`NavFixer.tsx`, `FixScene.tsx`, `navfixApi.ts`, `navfix.css`), one line in registry.tsx.
  - Map list: doctor's MapList, nav maps only.
  - The 3D view is its own Canvas using B's `World`, `Cameras`, `coords`, `useModelMeshes`, `areaColours`, `canDraw3d`/`SceneBoundary`, all read-only.
  - Modes: Select (Shift adds), Draw area (drag on any surface; the box is laid at that height; left-drag draws while right-drag pans), Link (click A then B; Shift or the checkbox makes it a one-way drop; the chain carries on from B so a beam is click, click, click).
  - Colours: where bots can get to, places, or how bots move.
  - Panels: Changes (Undo/Redo with labels, Start over, Save with "Saving…"), the picked area(s) (facts, links with Unlink, attribute checkboxes, place dropdown + New name…, position/size inputs, Snap to floor, Delete, Look at), Problems (Show me highlights the areas and moves the camera, plus one-click fixes), Bot knowledge (status, Rebuild with time estimate and inline job progress).
  - Keys: Cmd/Ctrl+Z, Shift+Cmd/Ctrl+Z or Ctrl+Y, Delete, Esc, V/D/L, O/T. `useDirty("navfix")` asks before leaving. The Fly camera is swapped for Orbit (the mouse is needed for drawing).

**Acceptance (cs_rats2)**
- `rats2_fix_matches_the_hand_made_one` runs on every `cargo test`. It sends the raw nav (fixture `tools/testdata/cs_rats2_raw.nav.gz`, 78 KB) the same edits the page sends (12 adds, the links, the one-way drop from 1155 to 2). The result is navpack/fixes/cs_rats2.nav byte for byte (sha256 checked).
- `real_rats2_on_the_map` (LS_TEST_SIM + LS_TEST_GAME) draws the same areas with floor snapping on the real cs_rats2.bsp. Every corner lands within 1 unit of the hand fix (256 / 448), ids and links are identical, and every new area is reachable.
- The checker on the raw nav: 7 problems, including "A ladder has no area at its top" at the beam ladders (it points at the very spot that needed fixing). After the fix: 5.

**How to try it**
- `cd app/src-tauri && cargo test navfix` (13 tests: edits round-trip through the file, one-way links, delete, islands/traps, the other checks, save/backup/refusals, the routes, the rats2 fix).
- Real files: `LS_TEST_SIM=/Users/jameson/loner-strike-sim LS_TEST_GAME="$HOME/Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life" cargo test navfix -- --nocapture`.
- In the app: Tools > Nav fixer > cs_rats2. With the raw nav in cstrike/maps: Top-down, Draw area along the beam, Link from 274 along it, Shift-click the drop to area 2, Save, Rebuild bot knowledge.

**Notes for others**
- C's `build_ladders` misses tops on cs_rats2 (9 of 22 ladders find no top area; some collapse to length 0). Islands would be badly wrong without a fix, so the checker's `ladder_ends` also counts any area within 72 units of the ladder box's foot or top. That took cs_rats2's raw nav from 120 "unreachable" areas to 6.
- Hull 1 traces from `MapHulls::trace` come back all-solid at many spots where `ceiling_above` from the same start says there's room (cs_rats2 areas 65, 146, 273, ... under the big furniture). That's why the stand-room check uses point traces. Possibly an entity tree (`solids`) answering all-solid for a short trace near it. Worth a look by C.

**Requests**
- (B) `Scene` could take extra children (layers) and export its `pointerEvents`. The nav fixer has its own small Canvas because Scene's layer list is fixed.
- (navs.rs owner) The knowledge rebuild copies a few small private pieces of `LocalLearner` (spawn with low priority, free port, resource copy). A `LocalLearner::run_server(map, extra_cfg)` would let navfix.rs drop them.

**Left**
- Not clicked through in the real app yet. The running app on this computer is the installed one, without these routes, and testing against the real game's maps folder would change real navs. First check: the rats2 flow above, including pointer picking at the app's text sizes.
- Rebuild not run end to end (needs the hidden server). Check that the brain DLL loads in Valve's HLDS under CrossOver and that bots join with `bot_enable 1` from server.cfg.
- Split, and a drag gizmo for move/resize (numeric inputs for now); multi-area link; a Problem spots feed from the health check and replays (plan 5.3).

### Agent I: map editor (Beta) — 2026-11-12

**Built**
- `tools/editor.rs` (mounted in tools/mod.rs, contract in TOOLS-PLAN §3.8): one open document per app run,
  held in Rust with E's `History`. `GET doc / meshes?doc&since / checks / textures / wads / tex/:name.png`,
  `POST cmd / undo / redo / new / open / import-map / save / export-map`. The server keeps when each brush last
  changed, so `meshes?since=REV` returns only those (plus `removed`); a new docId or no `since` gives everything.
  Push/pull drags send the distance from where the drag started with one `drag` id, so steps are computed from
  the brush as it was (face numbers can't shift) and fold into one undo step. Hollow on several brushes is one
  step. Texture sizes are learned from the map's WADs (40-byte miptex headers) so UVs are right. Files only through
  the system dialog; Save writes back to the project (`.lsmap`, E's JSON), atomic write. Never panics: numbers are
  range-checked, work runs in spawn_blocking, the session lock survives a poisoned mutex.
- `views/tools/editor/` (registered in registry.tsx): start screen (New map: open-sky room with 2 CT + 2 T spawns
  and a sun; Open a project; Import a .map). The editor: one 3D view (B's Orbit/Fly cameras, F toggles), flat
  Top/Front/Side on 2/3/4 (1 back to 3D); toolbar Select Q, Move W, Rotate E, Scale R (drei TransformControls,
  live preview, one command on release), Box B / Cylinder C (drag a footprint on the floor or on top of a brush,
  height and sides in the options bar), Clip X (two clicks, Tab picks the side, Enter cuts), Entity N (click a
  surface), Texture T (click paints, Shift whole brush, Alt picks up). Select + drag a face of a picked brush =
  push/pull (axis-aligned faces land on the grid). Snapping on by default, Alt ignores it, [ ] change the grid.
  Ctrl+Z / Ctrl+Shift+Z (Ctrl+Y), Delete, Ctrl+D duplicate, arrows/PgUp/PgDn nudge, H hollow, Ctrl+S save.
  Side panels: Inspect (entity keys with colour pickers for `_light`/`rendercolor`, an angle dial, target name
  suggestions, "sets off / set off by" links; brush actions hollow/merge/carve/snap/mirror/tie to entity/group;
  face alignment shift/turn/scale/fit/centre/world/face; map settings: sky + WAD list from the game folders),
  Outline (groups with hide/ungroup, entities, world brushes), Textures (the map's WADs, search, click to choose,
  double-click to paint), Checks (E's checks, live; click an issue to pick it and look at it).
  target -> targetname links are lines in 3D. Brush meshes are drawn with B's MapMaterials (read-only reuse of
  coords.ts, materials.ts, Cameras.tsx, bundle.ts readLsmb, Scene.tsx canDraw3d/SceneBoundary).
- Bundle: `MapEditor-*.js` 112 kB (33 kB gz) lazy chunk + 6 kB CSS; the start chunk is unchanged.

**How to try it**
- `cd app/src-tauri && cargo test tools::editor` (new map, commands, undo/redo, meshes since a revision,
  a 3-step drag as one undo, bad commands change nothing, project/.map files, textures from a WAD).
- In the app: Tools > Map editor.
- Without the app: `LS_TEST_GAME="<Half-Life folder>" cargo test serve_editor_for_ui -- --ignored --nocapture`
  serves just the editor routes on 127.0.0.1:8027, then `npm run dev` and a page that mounts `MapEditor` inside
  ToastProvider/ConfirmProvider. Checked that way in Chromium with the real game WADs: new map, box drawing,
  push/pull (one undo), gizmo move with snapping, texture painting from the browser, entity placing, clip,
  view switching, live checks. Not tried yet in the Tauri webview (file dialogs only work there).

**Left / notes**
- Not done: texture drag handles on the face (buttons and fields only), vertex editing, prefabs, FGD-driven
  entity forms (keys are free-form with help text for the common ones), drag-and-drop from the texture browser,
  carve/merge previews, multi-face texture selection, compiling (separate task).
- Big imported maps draw one mesh per brush (fine for a few thousand brushes; merge by texture if it's slow).
- `ops::move_face` leaves float noise in plane points (e.g. 192.00000000000006); harmless in the view, E may want
  to `tidy` there.
- Requests (B): `viewer/Scene.tsx` could export its `pointerEvents` (I copied the ten lines). Its context-lost
  listener would also fire when a canvas is unmounted on purpose; I check `gl.domElement.isConnected` first.

## Polish & feature pass (user request, after the integration pass)
User feedback example: docs/polish-spray-before.png — the spray maker's option buttons stack vertically in a
narrow right column, "The picture's own" wraps onto three lines, checkbox labels wrap; controls squeezed right.
1. **UI polish (all tools, screenshot-driven):** shared form rows / segmented pills / checkboxes / panels from the
   app's existing components and settings-page styles; no wrapped labels; layouts good at small and large window
   sizes; consistent spacing; empty, loading and error states; tooltips; visible focus. Screenshot every tool page
   at 3 window sizes, fix, re-screenshot.
2. **Editor (Hammer parity, modernised):** texture application tool (face sheet: shift/scale/rotate/justify,
   world/face align, treat as one, texture lock, drag-align on face, recent textures); FGD-driven entity forms
   (CS 1.6 FGD: typed fields, flags, choices, help); vertex editing; more primitives (wedge, spike, arch, sphere);
   3-mode clip; carve; flip/rotate 90; 2D box select; select groups vs objects; hide/isolate/visgroups; arrow-key
   nudge; [ ] grid size; camera bookmarks; paste special (arrays with offset/rotation); copy/paste between maps;
   map properties (sky, WADs, light); entity report; measure tool; check-for-problems with fixes; leak trace
   (pointfile) shown in 3D after compile; run-map dialog (Fast/Final presets) + cordon region; prefabs + library;
   autosave, backups, crash recovery, recent files.
3. **Quality of life (all tools):** search everywhere; go to coordinates; viewer bookmarks, screenshots, measure;
   keyboard shortcut sheet (?); remember last map/tool per page; shareable "open this spot" links between tools.
4. **Developer experience:** one command to run any tool page in a browser with real data; Playwright screenshot
   tests per tool page (catch layout regressions); one shared API client + error handling instead of per-tool copies.

### Integration pass (agent L) — 2026-11-12

**Sidebar and addresses**
- Tools is its own sidebar group after Hosting (the user's call). The group heading reads "Tools" with the Beta tag and folds the group (`ls.toolsOpen`). Under it: **Overview** (the cards page), then the tools grouped **Look** (Map viewer, Replay, Textures & models), **Fix** (Map health check, Map doctor, Nav fixer, Callouts) and **Make** (Map editor, Spray maker). The open tool is highlighted. This lives in `App.tsx` `ToolsNav`. Computer only, as before.
- `registry.tsx` entries now have `group` and `icon`, and shorter one-line blurbs. The blurb shows on the overview card and under the tool's title.
- Tools.tsx no longer has the Segmented row. It shows the overview, or the open tool under its own title.
- Which tool is open lives in `stores/toolPage.ts`, a tiny store, so the sidebar doesn't pull the viewer's state into the start bundle. `useTools.setTool` writes there.
- Addresses, all through `go("tools", ...)`:
  - `overview`
  - `<id>` or `<id>:<arg>` (read the arg with `useToolArg`)
  - `map:<name>` or `map:<name>@x,y,z[,size]`
  - `replay:<id>`
- Links between tools now use these addresses: health check "Open in map viewer" (`map:…@spot`) and "Watch in Replay", doctor "Name places" (`callouts:<map>`), callouts "Check the map in the map doctor".
- The Maps page's map sheet has "Open in Tools": own game only, not on a server, not other-game maps.
- macOS Go menu: Tools, after Servers.
- Get more labels the new section: "Texture pack" and "Textures". H's content.rs `textures` section is kept.

**Requests done**
- The viewer honours a look-at made before the map is up. `Cameras.tsx` LookAt waits for the camera controls, then clears the request. `setTool` drops a request left waiting by another tool.
- `viewer/Scene.tsx` exports `pointerEvents`, and `Scene` takes `children` (extra layers inside the root group).
  - The editor's copy of `pointerEvents` is gone.
  - FixScene still has its own Canvas (no entities, no click-to-clear), but now uses `pointerEvents`. Picking there was off at text sizes other than 100%.
- False "3D view stopped": Scene and FixScene now ignore a context lost by a canvas that's being put away (`isConnected`), as I did in the editor.
- `editor::ops::tidy` snaps values within 1e-6 of a 1/1024 step, and `move_face` tidies its points (test added).
- `goldsrc/mod.rs`: the module list is in one sorted block at the top. That edit was swept into K's commit 6ea2b7f; harmless.

**Nav and hull (C's code; a helper ran this, commit 85bb88c)**
- Ladders: our building already mostly matched the game's BuildLadders.
  - cs_rats2's 8 topless ladders are topless in the game too: thin func_wall plates (36, 106), a vent grate (39), floors with no nav area (11, 183, 226), and only an area behind (65, 121). The sim's recordings agree: bots almost never climb those.
  - de_nuke's vent ladders (114, 180) end at grates that are solid when ladders are built.
- Real differences fixed in `hull.rs` (new `trace_ignoring`, entity boxes 1 unit bigger like absmin/absmax, which entity was hit) and `nav.rs` (new `ground_height` like GetGroundHeight, used for the ladder foot; ladder 65 now has a bottom):
  - A trace that started inside a brush entity lost its start-solid flag.
  - Entity boxes weren't padded by 1 unit.
  - The ladder-foot fallback used a single downward trace.
- Hull 1 "false all-solid" at cs_rats2 areas 65, 146 and 273 is real overlap: 4-unit vent grates, and a 25-wide area 15.5 units from a wall corner. The short-trace vs `ceiling_above` mismatch was the start-solid bug.
- Tests: `real_rats2_ladders`, `real_rats2_standing_room`, `ground_height_like_the_game`, plus a start-solid unit test.
- J's workarounds in navfix.rs are both left in, with their comments updated.
  - The 72-unit ladder fallback: without it, cs_rats2 shows 120 unreachable areas, yet bots in the recordings stand in 39 of them. The reach model is missing some other way bots get around (J to look).
  - The point-trace head room check: a player box at an area's centre can genuinely be stuck.
  - `real_rats2_on_the_map` problems: raw 7 → 6, fixed 5 → 4.

**Shared files reviewed:** tools/mod.rs (C's `#[path]` replay line is already gone; routes merged once each), registry.tsx, goldsrc/mod.rs, content.rs, main.rs, server.rs (one merge line), App.tsx, store.ts (only `tools` added). No duplicates left. `main.rs` has `pub mod editor;` among plain `mod`s. That keeps E's not-yet-used core API free of dead-code warnings, so I left it.

**Checks:** `cargo test` 452 passed, 0 failed, 12 ignored. `npm run build` is green.
- Start chunk is 514.02 kB (165.05 gz). That's against main 509.17 kB (163.38 gz) and this branch before the pass 509.79 kB (163.64 gz).
- The +4.2 kB is the tool list in the sidebar: registry names, blurbs and the lazy import stubs with their preload lists. Every tool page is still its own chunk.

**Left for the real-app check**
- Sidebar: the Tools group folding, highlights, Overview cards at small and large window sizes, ⌘-number order, and Go > Tools on macOS.
- Health check "Open in map viewer" landing on the spot on a fresh viewer (Orbit and Top-down).
- Nav fixer picking at 90% and 125% text size (it now uses `pointerEvents`).
- Leaving a tool page doesn't show "The 3D view stopped".
- "Open in Tools" from a map sheet: Back should go to Maps, not a dead sheet.
- Ladders: confirm in game, with developer 1 for the "Unconnected ladder" messages, that cs_rats2's 8 and de_nuke's 2 are topless there too.

## Queued (user approved, 2026-11-12): Plugin Studio and an MCP connector
- **Plugin Studio (Tools > Make):** easy AMX Mod X scripting. Monaco editor with Pawn highlighting; autocomplete and
  hover docs generated from the server's AMXX .inc files; one-click compile with the bundled amxxpc, errors inline in
  plain words; one-click "try it" (compile, load on the local server, reload, live log panel); templates/snippets
  (new mode, command, on player killed, give item, HUD message, menu); a Loner-Strike include (loner.inc) wrapping
  the component library, bot orders, safely shared HUD channels and mode registration; "open as code" from the Game
  Mode Studio. No Lua.
- **MCP connector:** the app exposes its Tools as MCP tools for coding agents (maps: list/read/query, doctor, health
  check; editor: create map, brushes/entities/components, checks, export, compile, playtest; nav fixer ops; game
  modes: graphs, generate/compile plugins, start a local server in a mode with bots, read logs; server control).
  Off by default (Settings switch), local connections only, per-session token, read-only unless edits are allowed,
  destructive actions confirmed in the app. Built on the existing local HTTP API (server.rs).

### Agent K: the map viewer drawn like the game — 2026-11-12

**Built** (details, sources and formulas: docs/TOOLS-RENDER.md)
- Rust: `goldsrc/lightmap.rs` keeps every light style (atlas planes: pictures, then the faces' style numbers;
  `uv2` unchanged), `light_point`/`first_hit` (the game's light sample under a point); bundle header
  `lightmap.{maps,planes}`; `PARSER_VERSION` 2 (old caches rebuild). New `tools/render.rs` (mounted with one
  `pub mod` + one merge line): `GET /api/tools/map/:name/look` (the player's gamma/brightness/texgamma/
  lightgamma/gl_overbright/texture mode from config.cfg, sky sides, the sun, light at each model entity) and
  `/sky/:side` (TGA → PNG). Two lines in `tools/mod.rs` now pass `atlas.full_height()` to the PNG writer.
- UI: `viewer/gameLook.ts` (tables, styles, shaders), `materials.ts` rewritten inside (game look with Lighting
  on; sun shading with it off, as before), `World.tsx` (look, sky, render modes per brush entity, `GameClock`
  for flicker/pulse/water), `Models.tsx` (studio models placed in the map, lit like the game), Layers ▸
  **Look: Like the game | Bright** (`LookChoice.tsx`, one row inserted in `panels/Layers.tsx`). `Scene.tsx`:
  one line passes `entities` to `World`.
- Dev: `window.__lsViewer.camera(x,y,z,pitch,yaw,fov)` / `.shot()`, `app/ui/render-dev.html?map=&cam=&look=`,
  `cargo test serve_tools_for_ui -- --ignored` (all Tools routes on :8027 without the app), in-game kit
  `docs/render-compare/` (`lscompare.cfg` + `ls_camera.sma`, GoldSrc has no setpos).

**For agent I (editor) / anyone using MapMaterials**: the API is backwards compatible — same constructor
(new optional 6th `lightmapInfo`), `get(d)` (new optional `render`), `setOptions` (new optional `look`),
plus `setSettings`, `setSky`, `uniforms`. With `lighting: false` nothing changed. `OVERBRIGHT`/`SKY_COLOUR`
still exported.

**How to try it**: Tools ▸ Map viewer, any map, Lighting on (Fly shows the sky box). Or without the app: see
"Checking against the game" in TOOLS-RENDER.md. Checked in Chromium on de_dust2 (out/in, sky), cs_office
(indoor + glass), cs_havana (additive cones), cs_747 (strobe style animates 0/71 at 10 Hz), de_aztec (world
water, func_water, `{` fence in mode 4, a lit pred_plant model). `cargo test` and `npm run build` green.

**Left**: real in-game screenshots side by side (steps written, game not launched by me); detail textures (CZ),
gl_max_size downscaling, sprites/decals/dynamic lights/renderfx, model animation, animated `+0~` textures.

**Note**: my first commit (6ea2b7f) also carried someone's uncommitted reorder of the `pub mod` lines in
`goldsrc/mod.rs` (same modules, moved to the top) — harmless, but it's in my commit, not theirs. Someone is also
running my `serve_tools_for_ui` harness (debug build) on :8027.

### Agent P1: UI polish pass — in progress (2026-11-12)
**For P2 (editor) and P3: shared pieces are in `app/ui/src/views/tools/common/`** — please reuse them instead of
a tool's own copy: `import { Rows, Row, Check, Empty, Loading, MapList } from "../common"`.
- `Rows` / `Row name note info wide stacked`: a settings row like the app's Settings page (name + help left,
  control right at its natural width; drops under the name when there isn't room, never squeezes).
- `Check`: checkbox + label on one line. `Empty title text [children=buttons] [plain]`, `Loading text`.
- `MapList`: the searchable map rows (was `doctor/MapList.tsx`), now with its own CSS (the callouts list was
  unstyled when opened before the doctor) and an empty state (`none=`).
- CSS in `common/common.css` (`tl-` prefix), loaded with the first tool, not in the start bundle. Segmented
  labels inside `.tl-page`/`.tl-row`/`.tools-bar` never wrap.
- P3: `common/api.ts` is yours; I didn't touch it (add it to `common/index.ts` if you like).

### Agent W1: game mode components (gm_components) — interface proposal, 2026-11-12
Building Phase A's component plugin (GAMEMODES-PLAN §2.2/§2.3/§12 A, the 16+1 first-cut components).
Proposals below; reply under this heading (or your own) if you want something different. I'll build to these.

**For W2 (brain): the `ls_mode` server commands I will send** (plan §4.1, with a tag added so facts can be replaced/removed)
- Detection: please register a cvar `ls_mode_ver` (value `1`) when the commands exist. Without it I send no
  `ls_mode` lines (no "unknown command" spam) and fall back to the existing `ls_bot <i> goto` for a few goals.
- `ls_mode reset` — forget every fact and goal (I send it at map start and each new round, then re-send state).
- `ls_mode goal <who> <kind> <x> <y> <z> [ent] [prio] [until]` — who: bot index | `all` | `t` | `ct`;
  kind: `go_hold defend capture extract loot use ride investigate flee meeting`; prio 0-100 (default 50);
  until = seconds from now, 0 = until cleared.
- `ls_mode goal_clear <who> [kind|ent]`
- `ls_mode avoid <tag> <x> <y> <z> <r> <cost> [until]` — nav cost (damage zones, traps, wave spawns).
- `ls_mode attract <tag> <x> <y> <z> <r> <weight> [until] [team]` — worth going to (loot, spawners, drops,
  capture points); team 0 all, 1 T, 2 CT.
- `ls_mode forget <tag>` — drop an avoid/attract. Tags are `c<entity index>` (one per component; a second
  avoid/attract with the same tag replaces the first).
- `ls_mode block <ent> <until>` — a closed timed door costs as a wall (until 0 = unblock).
- `ls_mode zone <cx> <cy> <cz> <r> <ncx> <ncy> <ncz> <nr> <shrink_at>` and `ls_mode zone_clear` (plan's).
- I only send on state changes (and at round start), never per frame.

**For R (loner.inc): natives/forwards.** I'll ship them in `plugins/scripting/include/ls_components.inc`
(library `ls_components`, `#pragma reqlib`/optional like the kit), so loner.inc can just `#include <ls_components>`
and the signatures can't drift. gm_components itself won't include loner.inc (a plugin can't call natives it
registers). Naming: actions `ls_<component>_<verb>` (`ls_cp_owner`, `ls_zone_set`, `ls_door_lock`…), generic
`ls_comp_*` (`ls_comp_create(classname, origin, "key=value …")`, `ls_comp_find`, `ls_comp_get/set`,
`ls_comp_enable`, `ls_comp_start/stop`, `ls_comp_reset`); forwards all `ls_on_<event>` (`ls_on_capture(ent, team, id)`,
`ls_on_capture_tick`, `ls_on_zone_stage`, `ls_on_zone_tick`, `ls_on_extract`, `ls_on_loot`, `ls_on_wave_start`,
`ls_on_wave_spawn`, `ls_on_door`, `ls_on_trap`, …). Full list lands in docs/COMPONENTS.md. HUD: gm_components
uses one sync object (bottom-middle status line) + rare DHUD + BarTime/StatusIcon, so loner.inc's 4 slots stay
the mode's.

**For P2 (editor FGD forms):** component definitions as `app/src-tauri/src/editor/lonerstrike.fgd` (same style as
cstrike.fgd) plus `app/src-tauri/src/editor/components.json` (library metadata: group, visual kind, radius key,
colour, one-line blurb) for the drag-in panel. Please load the FGD beside `CSTRIKE_FGD` when you wire forms.

**Map packs (no BSP edits):** `addons/amxmodx/configs/components/<map>.ini` and `<map>.<modeid>.ini` (the
user's own), then the shipped `configs/gamemodes/components/…` (source: `plugins/configs/gamemodes/components/`).
Lines: `ls_capture_point 120 -340 64 radius=160 capture_secs=8`.

### Agent W2: brain interface for modes (ReGameDLL fork, branch `ls-modes`) — reply to W1, 2026-11-12
**Brain interface for modes.** W1's proposal above is accepted as-is; I build to it, with these additions
(all optional trailing args, so W1's lines work unchanged):
- `ls_mode_ver` cvar = `1` when the commands exist (as asked).
- `ls_mode reset` forgets the *component* facts: goals, avoid/attract/block, zone. Mode-level facts (roles,
  friend/foe ties, enemy classes, `set`) survive a reset, so the kit doesn't re-send them every round.
  `ls_mode reset all` (or a map change) clears everything.
- `goal <who> <kind> <x> <y> <z> [ent] [prio] [until] [radius]` — **radius added as the 9th arg** (capture /
  extract / go_hold area; 0 or missing = the kind's default). prio 0-100: >= 80 every bot does it, 30-79 most
  do (the persona decides), < 30 a hint only some take up. Kinds as W1 listed. `who` also takes `@<role>`.
- `goal_clear <who> [kind|ent]` as proposed.
- `avoid` cost: `0..10` = how much more an area inside costs to walk through (1 mild, 10 strong), `-1` =
  never path through it (unless the bot is already inside: then it walks out).
- `zone` as the plan's form; `shrink_at` is seconds from now. Bots outside the ring head in (careful ones
  keep a margin from the edge), and before `shrink_at` they head into the next ring.
- Mode-level (the kit's, not components'):
  - `ls_mode set <id>` — the mode tag (informational; `off` = `reset all`).
  - `ls_mode role <who> <role|->` — tag players (predator, traitor, zombie...). Used by `@role` selectors.
  - `ls_mode friend <who> <who2> [0|1]` — `who` treats `who2` as a teammate (even in free-for-all:
    BR squads). `ls_mode foe <who> <who2> [0|1]` — treats them as an enemy even on the same team
    (traitors). Bot-side only; friendly fire is the mode's business. Set after roles (selectors resolve then).
  - `ls_mode enemyclass <classname> [threat]` — non-player enemies (creatures): bots with no player enemy
    in sight shoot visible entities of that class (threat weights the pick; `0` removes).
    `ls_mode enemyclass_clear`.
  - `ls_mode status` — prints the state to the server console (for testing).

### Agent T: Community creations (Tools > Share > Community, Beta) — 2026-11-12

**Built** (design, schema, rules and server-side work: docs/COMMUNITY.md)
- `tools/community.rs` (one `pub mod` + one merge line in tools/mod.rs): browse (GameBanana's attribute search
  for the tag `Software Used: LonerStrike` merged with the Loner-Strike index `dl.lonerstrike.com/community/index.json`;
  kept copy and tag-only fallbacks), one creation (GameBanana page + index entry + trust), Try it (re-checks the tag
  and trust, picks a clean file, starts the existing `MapStore::start_section_install` job), Publish readiness
  (reuses `doctor::diagnose`/`file_checks`/`res_text`, newest `health::Report` for the map, `health::preset_mode`),
  the package zip (map, nav, finished .lsv, fresh .res, overview, custom files, README, loner-strike.json; screenshots
  beside it), register (paste the GameBanana link: game, tag and the package's MD5 checked; sent to
  `list.lonerstrike.com/community/submit` or kept as "waiting" in `<app dir>/tools/community/mine.json`), Yours,
  and "show in folder". Trust rules in `community::trust` (content / data-only / reviewed / verified / unreviewed / pulled).
- `views/tools/community/` (Community.tsx, Publish.tsx, Yours.tsx, communityApi.ts, community.css), registered in a
  new **Share** group (registry.tsx: `ToolGroup` + `TOOL_GROUPS` gained "Share", plus my one entry). Addresses:
  `community`, `community:publish[:<map>]`, `community:yours`, `community:mod:<id>`, `community:spray:<id>`.

**GameBanana findings**: no public upload API (submit on the site; the app prepares and checks). apiv11 has no tag
filter; `Util/Search/Results?_csvFields=attribs` searches tags, but splits on hyphens and ORs ("Loner-Strike"
matched 680 pages with "Strike"), hence the one-word tag value. Pages list each file's MD5, used to match the
package. To verify with the first real upload: the search finds `LonerStrike`.

**How to try it**: `cargo test tools::community` (index parsing and bad entries, kept-copy fallback, trust rules,
tag matching on search records, the merged list, file picking, links, MD5, a package built from a fixture game and
read back through `archive::plan_rels`, refusals, registration check); `cargo test community_live -- --ignored`
(GameBanana answers). In the app: Tools > Community (empty until something carries the tag), Publish > pick a map.

**Left**: modes and prefabs can't be tried until the Game Mode Studio's installer and the editor's prefab library
exist (pages say so; trust metadata and UI ready). The index file and the inbox don't exist on the servers yet
(COMMUNITY.md section 7). Not checked in the running app (the shared build was mid-change by others).
I re-read health reports from `<app dir>/tools/health` myself (`health::load_reports` is private).

**Git slip**: my first commit's staged files (community.rs + the two mod.rs lines) were swept into a292b8d
"Game modes plan: the Creature Framework…" by a commit that ran between my `git add` and `git commit`. Tree is
right; the message isn't mine. Tip for everyone: stage and commit in one command, by path.

### Agent Q: compiling with SDHLT + Playtest — 2026-11-12

**Built**
- `tools/compile.rs` (own routes, mounted with one `pub mod` + one merge line in tools/mod.rs): `GET status`,
  `POST setup` (the download), `POST start {preset, name?, cordon?, install?}`, `GET progress?from=N`, `POST cancel`
  under `/api/tools/compile/`. One job at a time (download or compile), on its own thread, stoppable, its output
  streamed by line number (plus the unfinished "10%...20%..." line).
- SDHLT v1.3.1 straight from the official release (`sdhlt-v1.3.1.zip`, 3 030 448 bytes, SHA-256
  `d206ddb73dc0d37f577b70b7e2503277a5092e7ab3e5b198a949fb49273ea8ea`), only after the page asks; checked, unpacked
  with the setup module's safe unzip into `<app dir>/tools/sdhlt/v1.3.1/` with LICENSE.md/README.md beside the tools
  (Linux builds dropped) and a marker naming the pinned sum. Not via the dl.lonerstrike.com mirror (we don't host it).
- Windows: the Win64 exes directly (Win32 on 32-bit). macOS: CrossOver's wine `--bottle B --wait-children`, like
  amxxpc/HLDS. Wine only sees the bottle's C: (this machine's bottle has no Z:), so the tools are copied once to
  `C:\Loner-Strike\Compiler\sdhlt-v1.3.1\` and maps compile in `C:\Loner-Strike\Compiler\work\<map>\`; 32-bit
  bottles (cxbottle.conf WineArch win32) get the Win32 builds. Stop kills the launcher and pkills the Windows process.
- Presets: Fast = CSG, BSP, RAD `-fast` (no VIS); Normal = all four; Final = VIS `-full`, RAD `-extra`.
  (SDHLT's settings.txt adds `-low` itself.)
- The .map: E's `write_map` of a copy of the open document (`editor::open_document()`, a small marked hook at the
  end of tools/editor.rs) with the `wad` key rewritten to the real files (C:\ paths through wine; a WAD wine can't
  see is copied next to the map) plus sdhlt.wad; every non-stock WAD goes in with `-wadinclude` so players don't
  need it. Missing WADs stop before compiling, with their names.
- Output -> plain-language problems (`parse_output`): LEAK (with the entity it starts from, matched to the editor's
  entity, and the leak line from `.lin`, else `.pts` thinned to 4000 points), `Exceeded MAX_*` and chart rows over
  100% (worldleaves etc.) and AllocBlock, missing textures (linked to a brush using it), WADs that won't open,
  "Entity N, Brush M: ..." (mixed contents, outside world, broken faces...) mapped back to editor brush/entity ids
  through the .map order, other Error/Warning lines; Description/Howto Fix lines become `help`. A leak fails the
  compile (SDHLT exits 0 and writes a leaky .bsp).
- Cordon: done at export (SDHLT has no cordon): brushes touching the box kept whole, point entities outside it
  dropped (light_environment kept), sealed in six SKY slabs. Editor ids survive so problems still link.
- Playtest (`install: true`): the .bsp into `<game>/maps/<name>.bsp`, the old one backed up to
  `<app dir>/tools/compile/backups/` (5 per map kept) if different; refuses stock map names.
- UI (mine, all new): `views/tools/editor/CompilePanel.tsx` (+ `compile.css`, `compileApi.ts`, `LeakLine.tsx`).
  Not installed: "Compiling maps needs SDHLT, a free GoldSrc map compiler (about 2.9 MB). Download it?" with the
  licence line and a progress bar. Then: map name, Fast/Normal/Final, "Only the area around what's picked", Compile /
  Stop, progress, problems (click = select + look; leak: show/hide the line, go to start/end), the output in a drawer.
  **Playtest** (mode + bots + "Teach bots the map first"): compile -> copy -> the map doctor's `.res` route ->
  `/api/navs/learn` (waits on the setup job) -> asks to close the game if it's open -> `/api/app/launch {map, bots,
  mode}` (the Play page's listen server with bots; it joins by itself). Steps shown as "Compiling…", "Lighting…",
  "Teaching bots the map…", "Starting the game…", "Joining…".

**How to try it**
- `cargo test tools::compile` (real SDHLT logs as fixtures in `tools/testdata/compile/`: leak, missing texture, a
  clean RAD; a limits/entities log; .lin/.pts; checksums; command lines per platform; bottles; WAD rewriting; cordon;
  installing with backups; routes).
- Real compile (opt-in): `LS_TEST_GAME="<Half-Life folder>" LS_TEST_SDHLT=<sdhlt-v1.3.1.zip> cargo test real_compile
  -- --ignored --nocapture` compiles the starter room (Fast) and opens the .bsp. Passes here through CrossOver (~2 s).

**Request (P2, editor)** — two one-line mounts, when you're at a good point:
- `MapEditor.tsx`: a side tab `["compile", "Compile"]` (add `"compile"` to `Tab`) rendering `<CompilePanel />`
  (`import { CompilePanel } from "./CompilePanel"`).
- `EditorScene.tsx`: `<LeakLine />` next to `<Links />` inside the GoldSrc root group (`import { LeakLine } from
  "./LeakLine"`). Points are GoldSrc units; it hides itself when there's no leak.
  Checked in a clean copy of HEAD with both lines added: `npm run build` green, everything stays in the MapEditor
  chunk (124 kB), start chunk unchanged. Optional: a "Playtest" button in the top bar could just open the tab.

**Left / notes**
- Not tried in the running app yet (needs the mounts); the panel's flow is untested end to end with a real game
  launch. Native Windows not run (command lines are unit-tested).
- The worktree's `npm run build` is red right now from someone else's `dev/toolsDev.tsx` (missing
  `../common/address`), and `cargo test` from someone's navfix.rs edit (E0061 at navfix.rs:2120); both green on a
  clean copy of HEAD with my commits (cargo 541 passed).
- Ideas: re-light only (`sdHLRAD` on the last .bsp), `-estimate` time left, a "Copy the log" button, compiling while
  the game runs uses `-low` already.
- Git slip: my notes commit a6a6838 also carried agent W2's then-uncommitted "brain interface for modes" section (the whole file was staged). Content intact; only the commit message is mine.

### Agent S: AI agents (MCP connector) — 2026-11-12

**Built** (all details, safety model and the manual test: docs/MCP.md)
- `app/src-tauri/src/mcp/` (`mod mcp;`, a `--mcp-stdio` check at the top of `main()` and `mcp::start` in main.rs;
  `.merge(crate::mcp::local_routes())` in server.rs; `mcp`/`mcp_changes` in app_api's Settings; `tower` moved to
  dependencies, already in the tree through axum):
  - `rpc.rs`: JSON-RPC over Streamable HTTP, own code (no `rmcp`: see MCP.md). Dual-era: 2026-07-28 (stateless,
    `_meta` version, mirrored headers checked, `server/discover`) and 2025-11-25/06-18/03-26 (`initialize` +
    `Mcp-Session-Id`).
  - `catalog.rs`: 55 tools, 3 resources + 4 templates, 2 prompts. Every tool calls the app's own routes in-process
    (the Local router through `guard_local`, `X-Panel: 1`), so no tool logic is duplicated; answers trimmed to
    ~48 KB. Maps/doctor/health, editor (incl. any `cmd` op), Q's compile + playtest, nav fixer, replay summary,
    the player's own local servers (start/stop/map/mode/bots).
  - `mod.rs`: own listener on 127.0.0.1:8029 (or a free port), only while on; Host/Origin checks; per-run token
    (Bearer, constant-time); read-only unless "Allow changes"; risky actions ask in a native dialog (Allow /
    Don't allow), and ask when the state can't be told (coordinator's security review, fixed in d84b0cc);
    activity log; `/api/mcp` routes for Settings (app window listener only).
  - `stdio.rs`: `loner-strike --mcp-stdio` bridge (std only).
- UI: `views/AgentSettings.tsx` (own lazy chunk, 5.7 kB) mounted with one line in **AppSettings.tsx** (that's the
  app's Settings page; Settings.tsx is the game's settings): switch, Allow changes (asks first), copy setup command
  (HTTP or stdio), token show / New token, "What agents did".

**How to try it**: `cargo test mcp::` (16 tests: protocol both eras, header mismatch, versions, tool listing,
token/host/origin, read-only, asking, unknown-state asking, trimming, real app routes, the endpoint, the bridge,
compile/playtest). Without the app: `LONER_MCP_TOKEN=... LONER_MCP_CHANGES=1 cargo test serve_mcp_for_try --
--ignored --nocapture`, then curl or the bridge. Checked that way with the real game (de_dust2 info/nav; new map +
box + checks through the bridge). In the app: Settings > AI agents, then the Claude Code steps in MCP.md.

**Left**
- Not run inside the Tauri app or with Claude Code itself (would change the user's Claude config); the native
  dialog path (`ask_person`) is untested by hand.
- 8029 was taken here by someone's node dev server, so the try-out used a random port; the setup command always
  shows the real one, but the token and maybe the port change per run, so agents need the command re-run after a
  restart.
- No game-mode tools yet (graphs, generate/compile plugins, logs): add them when those routes exist.

**Requests**
- (I, editor UI) The editor page should pick up changes made by an agent: refetch `meshes?since=` / `doc` when
  `revision` moves (e.g. poll `GET /api/tools/editor/doc` state every couple of seconds while the page is open, or
  on window focus). Today a box added over MCP shows up only after the page reloads its document.

### Agent P2: map editor feature pass (Hammer parity, modernised) — 2026-11-12

**Core (`editor/`, 63 tests)**
- `shapes.rs`: wedge, spike (pyramid/cone, 3-64 sides), sphere (4-32 facets round, half as many bands; facets
  planar, one brush), arch (N block pieces between angles; arc, start, wall). Grid-rounded when that stays exact.
- `vertex.rs`: `move_vertices(brush, picks, delta)` rebuilds the hull round the moved corners; refuses (with a reason)
  a move that would flatten the brush, dent it inwards, or swallow another corner. Faces keep the nearest old texture.
- `texture.rs`: `justify` (left/right/top/bottom/centre/fit, on one face or several treated as one), `set_values`
  (the face sheet's numbers; a new rotation turns the axes).
- `history.rs`: `cmd_array` (paste special: N copies, each one step on: offset + turn about x/y/z + stretch round the
  selection's middle), `fragment` + `cmd_paste` (clipboard as .map text, between maps; brush entities come along),
  `cmd_fix` (check fixes in one undo step), `cmd_replace_texture`, `History::mark_unsaved`.
- `checks.rs`: issues carry `fix` and `texture`; trigger_relay/auto/camera/changetarget no longer count as brush
  entities with no brushes.
- `fgd.rs` + `cstrike.fgd`: FGD reader (base classes, size/color, choices, flags, help, `+` strings, line-numbered
  errors) and **our own Counter-Strike 1.6 definitions** (75 classes). Licence check: no clearly shippable CS FGD was
  found (Valve's needs the SDK EULA; community ones carry no licence), so these are written from scratch for
  Loner-Strike. `EXTRA_FGDS` takes more files after ours (W1: add `include_str!("lonerstrike.fgd")` there).

**Routes (`tools/editor.rs`)**: cmd ops `add_shape`, `move_vertices` (drag id, from where the drag started), `array`,
`paste`, `fix`, `align` with `justify`/`set`/`one`; `POST copy`, `GET fgd`, `GET recent` + `POST open-recent {index}`,
`POST autosave` (only when dirty), `GET recovery`, `POST recover` / `discard-recovery`. Meshes carry each face's
alignment (`align`, 11 numbers). Save copies the old project to `<app>/editor/backups/` (last 10 per name) and clears
the autosave. `LS_EDITOR_PORT` moves the dev server off 8027.

**Page (`views/tools/editor/`)**
- Face sheet (`FaceSheet.tsx`): shift/scale/turn fields (blank when the picked faces differ), nudges, justify
  L/R/T/B/Centre/Fit, treat as one, fit N×M, align to world/face, texture lock, recent textures. Texture tool: click
  picks faces (Shift adds), "Click paints" mode paints, Alt picks up; drag a picked face to slide its texture
  (grid-snapped, one undo), drag the red/blue arrow ends to stretch, the yellow knob to turn (15° steps; Alt free);
  arrows shift the texture.
- Entity forms (`EntityForm.tsx`, `fgd.ts`): typed fields, choice lists, flag checkboxes, colour + brightness,
  angle dial, target suggestions, help on every key; unknown keys under "Other keys"; unknown classes stay raw.
  Entity tool's class box and "Tie to entity" list come from the FGD.
- Tools: Vertex (V; corner handles, drag snaps, Shift-click several, arrows/PgUp/PgDn nudge), Shape (C; cylinder,
  wedge, spike, sphere, arch), Measure (M). Clip keeps both / behind / in front (Tab). Turn 90° and Flip on X/Y/Z.
- Selection/view: box select in Top/Front/Side (drag on empty space; left button no longer pans there, right/middle
  does), Pick groups / Pick objects, H hide, Shift+H isolate, U show all, visibility sets in Outline (named, kept per
  map on this computer), [ ] grid, camera bookmarks Ctrl+1-9 store / 1-9 go (views moved to Shift+1-4;
  Ctrl+digit is caught first in the editor, so on Windows Ctrl+1-9 doesn't switch sections while it's open; ⌘ on a
  Mac still does).
- Ctrl+C/X/V copy, cut, paste (.map text on the system clipboard, with the editor's own copy as a fallback);
  Ctrl+Shift+V paste special (Row/Stairs/Ring/Spiral presets); Ctrl+H hollow.
- Dialogs: Map properties (title, sky, draw distance, base light, sun colour/brightness/pitch/angle, buying, bomb
  radius, WADs), Entity report (search, class filter, go to, pick, delete).
- Checks: a fix button per issue and "Fix all N" per kind; missing textures swap for the current texture; missing
  spawns hand you the entity tool.
- Autosave every 3 minutes when there are changes; the start screen offers to recover it, and lists recent files
  (also a Recent… menu in the bar).
- P4's Compile tab and LeakLine are mounted (their request).
- `editor-dev.html` + `editorDev.tsx`: the editor alone in a browser. Checked in Chromium (Playwright) at 1440×900
  and 1100×720 against the real game WADs: face sheet + drag-slide, multi-face pick, sphere/arch, vertex handles,
  box select, entity form, paste special, map properties, entity report, checks, texture browser.

**How to try it**: `cargo test editor` / `cargo test tools::editor`. Browser: `LS_EDITOR_PORT=8127 LS_TEST_GAME=<Half-Life>
cargo test serve_editor_for_ui -- --ignored`, then a Vite dev server proxying `/api` to 8127, open `/editor-dev.html`.

**Left / skipped**: prefabs + library; vertex editing of edges/faces as their own handles (pick their corners instead);
a 3D-view box select; sound preview in entity forms; studio model preview for point entities; visibility sets are
per computer, not saved in the project; the FGD isn't user-replaceable from the UI yet (the parser is ready).
`cargo test` right now: 579 pass, 2 fail in `tools::studio` (someone's work in progress, not the editor).

### Agent R: Plugin Studio (Beta), 2026-11-12
Tools > Make > **Plugin Studio**: write AMX Mod X plugins in the app.

**What's there**
- UI `views/tools/plugins/`: files on the left (My plugins = `<app dir>/plugin-studio/my-plugins`, the game's
  `addons/amxmodx/scripting`, its include files + loner.inc read only), Monaco in the middle, Problems / Game log /
  Compiler output underneath. New plugin from 8 templates (empty, game mode with loner.inc, chat command, on player
  killed, give items on spawn, HUD message, menu, timer). Ctrl/Cmd+S saves, Ctrl/Cmd+Enter compiles. Address
  `plugins:mine/<file>.sma` opens a file (for "open as code" from a future Game Mode Studio).
- Editor: monaco-editor 0.52.2 (pinned), core only (`edcore.main` + one editor worker, no TS/JSON languages),
  loaded with `import("./monaco")`. A Monarch grammar for Pawn (`pawn.ts`: ^ escapes, tags, directives, doc
  comments; known natives and constants coloured once the word list arrives), the app's olive theme. Completions
  (natives/stocks with parameter snippets, forwards as `public name(...) { }` at a line's start, constants,
  `#include <` names, this file's own functions/variables, snippets), hover docs (signature, description,
  parameters, returns, notes, deprecated, file), signature help.
- Rust `tools/plugins.rs` (+ `plugins/inc.rs` include reader, `plugins/diag.rs` compiler output), routes
  `/api/tools/plugins` (overview), `/file` (GET read, POST save), `/delete`, `/words`, `/compile`, `/try`,
  `/untry`, `/log`. Own game only (`local_game`, refused for remote/dedicated), local listener, POSTs need X-Panel.
  Safe names (`[A-Za-z0-9_][A-Za-z0-9_.-]*\.(sma|inc)`, no `..`, ≤64), files directly in the folder, links out
  refused; saves write `<name>.part` then rename; UTF-8 or Latin-1 kept as it was; 2 MB cap. Loner-Strike's own
  plugins in the game folder and includes are read only ("Save a copy in My plugins").
- Compile: the game's `scripting/amxxpc.exe` through `setup::core::windows_command` (CrossOver wine on a Mac, as
  setup does), one at a time, one retry if Wine stops it part way, `-i<app>/plugin-studio/include` holding the
  app's loner.inc (embedded with include_str!). Output to `<app>/plugin-studio/build/<name>.amxx`. Errors and
  warnings parsed (`file(line[ -- line]) : error|warning|fatal error N: msg`, any slashes) and explained in plain
  words (about 60 codes), marked on their lines; ones in includes are listed with their file.
- Try it: compile, copy the .amxx into the game's `plugins/` (refused if a plugin of that name is there and the
  Studio didn't put it there, or it's one of ours), list it in `configs/plugins-studio.ini` (AMX Mod X loads
  `plugins-*.ini` beside plugins.ini; entries get `debug` so errors name the line), remembered in
  `data/gamemodes/studio_plugins.json`, then the map restarts through `gm_restartmap` (the app's existing
  `Game::command` path) if a map is up. "Take it out" undoes all of that.
- Game log: polls `/log` every 1.5 s; reads the newest `logs/L*.log` and `error_*.log` from a byte cursor (first
  look: last 64 KB, last 50 lines) and keeps the plugin's own lines, load failures naming it and run time error
  traces that start with it.
- `plugins/scripting/include/loner.inc` + `docs/LONER-INC.md` + `plugins/scripting/examples/ls_example_mode.sma`
  ("Deagle Duel"): mode registration on top of gamemodes.amxx's `gm_modes` natives, 4 HUD slots on their own sync
  objects + director HUD announce, give weapon with ammo / strip / health / armour / teleport / team / roles,
  timers and a countdown, bot orders over the fork's `ls_bot` command (work today), and the Mode Kit natives from
  GAMEMODES-PLAN 2.3 declared with `#pragma reqlib modekit` plus a module/native filter installed from loner.inc's
  own `plugin_natives()` (opt out with `LS_OWN_PLUGIN_NATIVES` + call `ls_optional()`), so plugins compile and load
  without the kit. TODOs point at the plan. Both compile clean (no warnings) with amxxpc 1.10.

**Checks**
- `cargo test` on HEAD: 544 passed (clean export; the worktree itself currently has someone's unfinished file that
  doesn't compile). Plugin Studio tests: include reader (sample + odd input), diagnostics from real amxxpc output
  fixtures (`tools/testdata/amxxpc/`), path safety incl. symlinks, save/read/list/delete, Latin-1 round trip, try
  and take out, log following, loner.inc words. With `LS_TEST_GAME`: `real_includes` (68 includes → 5126 words:
  1270 natives, 80 forwards, 387 stocks, 1146 defines, 2130 constants; docs and params on get_user_name etc.) and
  `real_compile` (loner.inc example compiles with no diagnostics; a broken file gives errors on the right lines).
- `npm run build` green. Start chunk +0.33 kB (514.43 → 514.76 kB, gzip 165.19 → 165.30) for the registry line
  only. PluginStudio chunk 21.2 kB (7.7 gz) + 5.0 kB CSS; Monaco chunk 3.31 MB (855 kB gz) + 133 kB CSS + editor
  worker 232 kB + codicon font 80 kB, loaded only when the Studio opens.
- Browser check against a mock of the routes: editor, colours, completion, signature help, compile → markers and
  Problems, Try → Game log, New plugin, narrow window. Found and fixed: the app's `.main` (page shell) made Monaco's
  suggestion rows columns (scoped override in plugins.css).

**Left / for the real-app check**
- Try it against a running game (gm_restartmap and the log lines in real AMX Mod X logs).
- Compiler under CrossOver on a slow first start (Wine bottle boot) takes a few seconds; the button says so.
- Dedicated servers on this computer aren't offered (own game only); easy to add with a server picker.
- Ideas: go-to-definition into include files, format document, a "Make it a mode" button that writes the
  modes.ini line and `<id>.plugins.ini`.
