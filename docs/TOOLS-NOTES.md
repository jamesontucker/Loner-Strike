# Tools crew notes

## Requests

- (B, UI) `main.rs::mac_menu`: a "Tools" item under Go sending `section:tools` (the page already
  answers that event). Owner: agent A / whoever owns main.rs.
- (B, UI) Maps page map sheet: an "Open in Tools" button calling `go("tools", "map:" + name)`. The
  Tools page already opens a map from that focus string. Maps.tsx isn't mine.

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

