# Loner-Strike "Tools": map viewer → map editor, bot learning and simulation visualisers

Implementation plan, 2026-11-12. Planning only; nothing in the repo was changed.

Paths used below: `APP` = `/Users/jameson/Nextcloud/Jameson/Development/Other/loner-strike/app`,
`SIM` = `/Users/jameson/loner-strike-sim`, `FORK` = `/Users/jameson/Nextcloud/Jameson/Development/Other/ReGameDLL_CS`.

---

## 0. Key decisions (the short version)

| Decision | Choice | Why |
|---|---|---|
| Where parsing lives | **Rust, in the app's local axum server**; the UI only decodes typed arrays and renders | The app has no Tauri `invoke` commands at all: every page talks HTTP to `127.0.0.1:<port>/api/...` (`APP/src-tauri/src/server.rs`). Rust already reads BSP entity lumps (`maps.rs::title_from_bsp`), nav headers (`navs.rs::parse_head`) and .lsv headers (`navpack.rs::lsv_done`). Big files (a 30 MB BSP, a 3 MB recording, a 750 KB trace) are read once, cached, and sent as binary; the UI never sees raw lumps. The same parsers later feed the nav fixer, health check and (phase 2) BSP writer. |
| BSP/WAD/nav/.lsv readers | **Our own, clean-room, in `src-tauri/src/goldsrc/`**; `qbsp` and `goldsrc-rs` crates used as reference, not dependencies | The formats are small and we already own verified Python ports (`SIM/surfsim.py`, `surfpng.py`, `navpack/navarea.py`, `mapknow.py`). No Rust crate writes BSP30 (needed in phase 2), the GoldSrc crates are young/0-star, and the Valve SDK headers can't be copied (non-free licence; see §2). |
| 3D in the UI | **three + @react-three/fiber + @react-three/drei**, loaded only when Tools opens (`lazyPage`, own Vite chunk) | What the user asked for; the app already lazy-loads big pages (`App.tsx` lines 28-35). Three stays out of the start bundle (~500 KB today). |
| JS GoldSrc loaders | Not used as dependencies (hlviewer.js is raw WebGL, goldsrc-bsp-viewer is weeks old); **web-hlmv (MIT) is the candidate for MDL** later | Parsing is in Rust; in TS we only need decoding + rendering. |
| Coordinates | Keep **GoldSrc units and axes in every buffer**; one root `<group rotation={[-π/2,0,0]}>` turns z-up into three's y-up | One conversion point; nav, .lsv, recordings and the BSP all stay in the same numbers the Python tools print. |
| File access | UI asks by **map name** (validated with `maps::safe_arg`), Rust resolves files in the game folders it already knows (`Game::game_dirs()`); recordings are picked with the dialog plugin and referred to by an id. Tools routes are **computer-only** (`Listener::Local`), like `mapcopy::local_routes()` | Same security model as the rest of the app: the page can never name a path. |
| Compilers (phase 3) | **SDHLT** as a separate, optional download run as a child process; never linked or vendored | ZHLT-lineage licence is GPLv2 *plus* the Valve SDK EULA, not GPL-3-compatible for linking; separate binaries are fine. No macOS builds exist → we build arm64 in our CI (open question). |
| Order | Phase 1 = viewer + nav/.lsv overlays + replay. Phase 2 = nav fixer, map health check, BSP entity edits. Phase 3 = brush editor + compile. Phase 4 = game-mode creator | Phases 1-2 replace the by-hand cs_rats2 loop; the editor is months of work and builds on the same parsers. |

---

## 1. What exists today (verified)

### 1.1 The app
- **Tauri 2 shell, but all UI↔Rust traffic is HTTP.** `main.rs` binds `127.0.0.1:8027` (or a free port), `server.rs::router()` merges per-module `routes()`, `local_routes()` (computer only) and `owner_routes()`. Handlers are axum functions; blocking file work goes through `tokio::task::spawn_blocking` (`panel_api.rs::with_game`). Binary responses already exist (`/thumb/:file` returns PNG/JPEG bytes with cache headers).
- **UI:** React 18.3.1, TS 5.6, Vite 5.4, Zustand 5, TanStack Query 5. Navigation is `useNav` (`store.ts`): `Section` union + `SECTIONS` list + `NAV` array in `App.tsx` (groups Game / People / Online / My library / Hosting / App) + the macOS "Go" menu in `main.rs::mac_menu`. Pages render inside `<PageBoundary><Suspense>` and big ones are `lazyPage(() => import(...))`.
- **Conventions to follow:** slow actions go through `useActions().run(key, fn, "Doing…")` (spinner + label kept in `useActivityStore` across page switches), `BusyButton`, `useToast`, `useConfirm`; plain-language copy; `SectionHead`, `Info`, `Sheet`, `Skeleton`, `LoadError`; Steam-2004 olive theme tokens in `styles.css` (`--panel`, `--signal`, `--t`, `--ct`, `--raised`). `useDirty()` for unsaved edits.
- **Tests:** 269 `#[test]`s in Rust (unit tests next to code, axum `oneshot` router tests in `server.rs`); **no JS test runner**.
- **Maps the app knows:** `Game::maps()`/`scan_maps()` in `maps.rs` list `maps/*.bsp` in `cstrike` and `czero`; `find_in(dir, name, exts)` resolves files case-insensitively. Nav pack installer `navpack.rs` puts `<map>.nav` + `<map>.lsv` next to the BSP. `navs.rs::LocalLearner` already runs a **hidden dedicated server on this computer** to teach bots a map (reusable for the health check and .lsv rebuilds).
- **Licence:** the repo is **GPL-3.0** (`LICENSE`, `Cargo.toml license = "GPL-3.0"`).
- **Bundle today:** `ui/dist/assets/index-*.js` 509 KB, CSS 102 KB, lazy pages 14-118 KB. The UI is embedded in the binary with `rust-embed` and gzipped on the fly.

### 1.2 The data we want to draw
- **BSP30** (CS 1.6 / CZ): 15 lumps; we need entities (0), planes (1), textures/miptex (2), vertices (3), texinfo (6), faces (7), lighting (8), clipnodes (9), edges (12), surfedges (13), models (14). `SIM/surfpng.py` already turns faces into polygons (surfedge sign → vertex pick, side → flip normal) and `SIM/surfsim.py` ports `SV_RecursiveHullCheck` over hull 1 (32×32×72 player box) including brush-entity hulls, teleports, pushes and `trigger_hurt`.
- **WAD3**: miptex 8-bit indexed + 256×RGB palette after the 4 mips; `{` textures use index 255 as transparent; `!` water, `sky`, `aaatrigger`, `+N`/`-N` animated/random. Embedded textures have non-zero mip offsets; otherwise the name is looked up in the WADs listed in worldspawn `"wad"` (basenames; search `cstrike/`, `valve/`, `czero/`). `navpack/res.py` already extracts the `wad` list and referenced `.mdl/.spr/.wav`.
- **.nav v5** (`FORK/regamedll/game_shared/bot/nav_file.cpp`, `SIM/navpack/navarea.py`): magic `0xFEEDFACE`, version, bsp size, place table, then areas: id, u8 attributes (CROUCH 1, JUMP 2, PRECISE 4, NO_JUMP 8), extent lo/hi (nw = lo, se = hi), neZ, swZ, 4 connection lists (N = -y, E = +x, S = +y, W = -x), hiding spots (id, pos, flags), approach spots, encounter paths, u16 place (1-based). **No ladders in the file**: the game rebuilds them from `func_ladder` brush entities (`BuildLadders`), so the viewer must too (`SIM/knowscreen.py:35` does).
- **.lsv v6 map knowledge** (`FORK/regamedll/dlls/bot/cs_bot_ls.cpp` 3454-3482, 4020-4096; struct in `cs_bot.h` 1733-1763): packed little-endian: 44-byte `FileHead` (magic "LSV2", version 6, `key` = FNV-1a of the nav (`LsNavKey`, 3507), `count` = n areas, `stage` 0-4, resume counters, `pairs`, `bangs`, stats) + sight bitmatrix `(n*n+7)/8` bytes (bit `i*n+j`) + `PairInfo[pairs]` {a,b u16; quality u8; flags u8 (1 = crouched head seen)} + `BangInfo[bangs]` {a,b; thick; mat (CHAR_TEX letter)} + `LsAreaKnow[n]` 315 bytes each (16-direction cover at crouch/stand heights, materials, ceiling, sees, high-ground share, up to 8 corners {x,y,z i16, yaw u8}, 8 angles {yaw u8, pitch i8, dist u16, weight u8, area u16}, 8 objects {type, x,y,z,top, mat}, exposure bitmasks per 3×3 point, point cover bitmasks). **All indices are positions in `.nav` file order, not nav ids.** Written as `<gamedir>/maps/<map>.lsv` in stages while bots play; `SIM/mapknow.py` already draws it as SVGs. Length must match exactly or the brain discards it.
- **gm_simrec recordings** (`SIM/gm_simrec.sma`): `addons/amxmodx/data/simrec/<map>_<systime>.csv`, no header, row type in column 1: `S` (every 0.1 s per living player: t,id,team,x,y,z,vx,vy,vz,yaw,pitch,duck,ground,weapon,clip,keys,hp,venemy,enemydist,matedist), `F` shots, `H` hits (players only; fall damage never appears), `K` kills (falls/world deaths = `K` with weapon `world`/`trigger_hurt`), `R` round start/end, `P` players (id,name,bot), `C` chat, `E` events (planted, defused, pickups…). Position is `pev_origin` (hull centre, 36 above the feet when standing). Team 1 = T, 2 = CT.
- **Brain trace** (`lstrace.csv`, written by the game DLL in the server root when `bot_ls_trace 1`; `cs_bot_ls.cpp` 2617-2732): `#cols` header, one row per living bot every 0.5 s (state name, task int, enemy, leader, goal_x/y, noise_x/y, `why` label + age, speed, view_off, attacking, `look` description, x,y,z, hp, weapon, crouched, pitch) plus `#why`, `#attack`, `#seen`, `#mark`, `#map` lines.
- **Sweeps** (`SIM/sweep.sh`, `sweepreport.py`): a bots-only round per map on CrossOver clones, then per-map flags: crashes, plugin errors, `.lsv` loaded, kills/bot-min, stuck %, still %, nav coverage %, top stuck cells (128-unit grid), spawn sitters, world-death share (flag > 20 %). The cs_rats2 loop was: sweep → read report → `stuckwhy.py`/`deathwhy.py` → look at PNGs → type a `fixes/cs_rats2.json` → `navarea.py`/`navlink.py` → rebuild `.lsv` on a sim server → sweep again → `package.py`. Steps 2-6 are what Tools should absorb.
- **Surf** (branch `ls-surf`, `cs_bot_surf.cpp`): in-game port of surfsim + a Go-Explore archive (nodes with parent links, per-chunk samples {o, v, move}, roots). **Never saved to disk**; drawing it needs a new dump command in the fork (§5.5).

---

## 2. Open-source tools: what we can use and how

All checked against the repos' LICENSE files on 2026-11-12. Our app is GPL-3.0, free, publicly distributed.

| Project | Licence | Use for us | Verdict |
|---|---|---|---|
| **BSPguy** (wootguy) | Unlicense (public domain) | C++ GUI/CLI; reads and writes BSP30 (entity/face/lightmap/model edits, VIS+RAD recompile, merge). Windows binaries only, OpenGL 2.1, no mac mention | Not a library. **Crib its algorithms** (lightmap packing, clipnode → polygon hull display, BSP writing) freely; optionally bundle the .exe on Windows later. |
| **TrenchBroom** | GPL-3.0 | Qt editor; "Half-Life (experimental)" config, Valve220 .map, WAD3; no compilers built in; macOS arm64 builds | UX reference for the phase-3 brush editor. Code is GPL-3-compatible but C++/Qt, so ideas not code. |
| **ericw-tools** | GPL-2.0 | `qbsp -hlbsp` writes BSP30 but lighting is Quake-style, no `light_environment`/HLRAD features, alpha; Darwin zips exist | Fallback "preview compile" on macOS only if SDHLT can't be built there. Not for shipping maps. |
| **ZHLT 3.4 / VHLT v34 / SDHLT (seedee, 2026)** | "GPLv2, but the Valve SDK 2.3 EULA and id's Q2 licence override it where needed" | The real GoldSrc compilers (hlcsg/hlbsp/hlvis/hlrad). SDHLT is maintained (v1.3.1, 2026-10). Windows zips; CI builds Linux; **no macOS job** | **Ship as a separate optional download** (like the voice models), run with `Command`, licence file beside it, never linked/vendored, app stays free. Build arm64 macOS ourselves. |
| **J.A.C.K.** | proprietary | — | FGD/feature reference only. |
| **Half-Life SDK** (`bspfile.h`) | Valve custom "only for free", non-OSI | — | **Don't copy code.** Format facts aren't copyrightable; write from the public specs (hlbsp.sourceforge.net, TWHL WAD3 spec) and our Python ports. |
| **hlviewer.js** (MIT, TS) | MIT | BSP30 + WAD3 + lightmaps + demos; raw WebGL + solid-js | Reference for lightmap/WAD decoding only. |
| **goldsrc-bsp-viewer** (MIT) | MIT | three.js + React, BSP30/WAD3/lightmaps, FGD metadata; new (2026), one author | Reference / fallback if we ever parse in TS. |
| **web-hlmv** (MIT, three.js, TS) | MIT | GoldSrc MDL loader + animation | **Candidate for the model browser** (phase 2); known weapon bone bug. |
| **qbsp** crate 0.17 (MIT/Apache-2) | MIT | BSP29/BSP2/BSP30 reader, lightmap atlas, meshing; no writing | Reference for atlas packing; not a dependency (glam, Quake-centric API). |
| **goldsrc-rs** 2.0 (MIT) | MIT | zero-copy BSP/WAD/MDL/SPR reader, "not thoroughly tested" | Maybe for MDL/SPR decoding in Rust later; not for BSP. |
| `sourcenav` (Rust) | **AGPL-3.0** | Source nav versions | **Avoid.** |

---

## 3. Architecture

### 3.1 Data flow

```
game folders (cstrike/, czero/, valve/)            app cache (sys::app_dir()/cache/tools/)
  maps/<map>.bsp  .nav  .lsv  *.wad  overviews/      <map>-<size>-<mtime>.mesh  (binary bundle)
          │                                           <map>-...-tex/<name>.png, lightmap.png
          ▼  Rust: src-tauri/src/goldsrc/* + tools/*  ◄─────────────────────────┘
   /api/tools/...  (Listener::Local only, JSON + application/octet-stream)
          │
          ▼  UI: views/tools/* (lazy chunk)  → decode typed arrays → zustand tools store → R3F scene
```

- Rust parses once per file version and writes the processed bundle to the cache; the handler streams the cached bytes with `Cache-Control: no-cache` + `ETag` = size+mtime so the browser revalidates cheaply.
- The UI keeps three kinds of state: **documents** (mesh bundle, entities, nav, knowledge, recording — TanStack Query keyed by `["tools", kind, map, version]`), **view state** (layers on/off, selection, camera preset, replay time — a zustand store `stores/tools.ts`), and **edits** (phase 2; a command stack with undo).

### 3.2 Coordinates
- GoldSrc: right-handed, **z up**, x east, y north; yaw 0 = +x counter-clockwise toward +y; **pitch positive = looking down**. 1 unit ≈ 1 inch. Player hull 32×32×72 standing (origin at centre, 36 above the feet, eyes at feet+53 ≈ origin+17), 32×32×36 crouched. The brain's "eyes" are nav floor + 60 (stand) / + 34 (crouch).
- three.js: right-handed, y up. Mapping `(x, y, z)_gs → (x, z, -y)_three` is a proper rotation (−90° about X), so **one root `<group rotation={[-Math.PI/2, 0, 0]}>`** holds everything and every buffer stays in GoldSrc numbers. Picking returns three-world points; convert back with `root.worldToLocal`. Cameras take `up = (0,1,0)` as usual.
- View direction from (yaw, pitch) in GS: `(cos y·cos p, sin y·cos p, −sin p)`.
- Top-down preset: +y up on screen, like `surfpng.py`/`placepng.py`, so pictures match the Python tools. Radar overlay (`overviews/<map>.txt` ZOOM/ORIGIN/ROTATED) follows `SIM/radarpng.py`.
- Nav area corners: nw `(lo.x, lo.y, lo.z)`, se `(hi.x, hi.y, hi.z)`, ne `(hi.x, lo.y, neZ)`, sw `(lo.x, hi.y, swZ)`; draw as two triangles.
- `.lsv` indices → nav areas by **file order**; verify `FileHead.key` against a Rust port of `LsNavKey` (FNV-1a over the area count, then each area's id and integer centre) and show "Knowledge was built for a different nav" when it differs.

### 3.3 Mesh bundle (binary, `application/octet-stream`)
Little-endian. `"LSMB"`, u32 version, u32 json length, JSON header, then raw sections at 4-byte alignment. The JSON lists sections `{name, offset, length, type: "f32"|"u32"|"u16"|"u8", stride}` and the draw table. In TS a `DataView` gives zero-copy `Float32Array` views.

Sections (phase 1): `positions f32×3`, `normals f32×3`, `uv f32×2` (texture, already divided by texture width/height, tiling allowed), `uv2 f32×2` (lightmap atlas), `indices u32`, `faceOfTriangle u32` (for picking → face → texture/entity), `draws[]` in the header: `{model: 0|N, texture: "name", start, count, flags: sky|water|trigger|transparent}`. Brush-entity models (`*N`) get their own draw ranges with `entityIndex`, so the UI can toggle/colour them and move them later (phase 2). Textures are PNG per name (`/tex/<name>.png`, RGBA, index-255 transparency applied); lightmap is one PNG atlas (`/lightmap.png`) plus `uv2`. Each draw is a `geometry.addGroup(start, count, materialIndex)` on one merged `BufferGeometry` per model: draw calls = number of textures (~150-300 on a big map), which WebGL handles easily.

### 3.4 Performance for big maps
- de_dust2 ≈ 10k faces / ~40k triangles; the biggest custom maps ~3-4× that. Geometry is trivial for the GPU; the cost is parsing + texture decoding, which is in Rust, cached, and bounded (BSP ≤ 64 MB, like `navpack::MAX_FILE`).
- Lightmaps: luxel = 16 units; per face `w = ceil(max_s/16) − floor(min_s/16) + 1` (same for t), RGB8 per style; pack style 0 (+ style 1-3 later) into a 2048² or 4096² atlas with 1-px padding, like `qbsp`/BSPguy. Done in Rust; the UI multiplies `map × lightMap` with a gamma/overbright factor (GoldSrc looks right around ×2 with gamma 1.8).
- Sky faces are drawn in a flat sky colour and excluded from picking; water gets a translucent material; `aaatrigger` draws only when the Triggers layer is on.
- Replay: recordings are turned into column arrays per bot (`t[]`, `x[]`, `y[]`, `z[]`, `yaw[]`, `pitch[]`, `hp[]`, `duck[]`, `weapon[]`) in Rust (≈ 10 Hz × 12 bots × 600 s = 72k samples, a few MB) and sent as a bundle; trace rows (0.5 Hz) as JSON per bot. Scrubbing is a binary search per bot; interpolation between samples; trails are a `Line2`/`LineSegments` with a moving draw range. Heatmaps are a 128-unit grid (as `sweepreport.py`) rendered as a `DataTexture` on a floor-hugging plane, or per-cell instanced quads.
- Nav: areas as one merged `BufferGeometry` with a colour attribute (place/attribute colouring without re-upload); connections as one `LineSegments`; labels via Drei `<Text>` (SDF) only for place names (≤ 40), never per area.
- `.lsv`: the sight matrix is decoded on demand per selected area (one row = n bits) — the full matrix for 737 areas is 68 KB, fine to send whole; sightlines are drawn only from the selected area(s) or as a density colour per area (`sees`).
- Canvas `frameloop="demand"` when nothing animates; `dpr` capped at 1.5 on Retina; `powerPreference: "high-performance"`.

### 3.5 File access and security
- Tools routes are mounted only on the computer listener (`server.rs::router`, `Listener::Local` arm) via a new `crate::tools::local_routes()`. Phones never see them (3D on a phone over Wi-Fi isn't a goal; the Maps page can later show a static top-down PNG from the same bundle if wanted).
- Map names go through `maps::safe_arg` (`^[A-Za-z0-9_.$\-]{1,64}$`); files are resolved by `Game::game_dirs()` + `find_in`; WADs by basename only, searched in `cstrike`, `valve`, `czero` (never a path from the request). Recording files: `POST /api/tools/recordings/open` uses `tauri_plugin_dialog` (already a dependency; see `/api/gb/add-file`) and returns an opaque id; the server keeps the id→path table in memory and also lists `sys::app_dir()/tools/recordings/` and the game's `addons/amxmodx/data/simrec/`.
- Reads are size-capped and parsed with bounds checks (every offset from the file is validated; a malformed download must return a message like "This map file is damaged", never panic — wrap parsers in `catch_unwind` in the handler like `diagnostics::log_errors` expects).
- Phase 2 writes (.nav/.lsv/.bsp) write to `<path>.part` then rename (the brain does the same), keep a `.bak` the first time, and refuse while the game has the map loaded (check `GameState.map`).

### 3.6 Loading three.js lazily (download size)
- New deps: `three`, `@react-three/fiber`, `@react-three/drei`, `@types/three`. Minified+gz: three ≈ 160 KB, R3F ≈ 30 KB, drei (tree-shaken: Text, Html, Line, OrbitControls, PointerLockControls, Stats) ≈ 60-120 KB, troika-three-text (pulled by `<Text>`) ≈ 80 KB. Raw ≈ 1.3-1.8 MB, which is what grows the binary (rust-embed stores raw; served gzipped).
- Rules: nothing under `views/tools/` is imported statically from `App.tsx`; the Tools page is `lazyPage(() => import("./views/tools/Tools"))`; `vite.config.ts` gets `build.rollupOptions.output.manualChunks: { three: ["three"], r3f: ["@react-three/fiber", "@react-three/drei"] }` so the vendor chunks are cached independently of our code; `import { Text } from "@react-three/drei"` (ESM, tree-shakeable), never `drei/core` barrel re-exports we don't need. Check with `vite build` that `index-*.js` doesn't grow.
- Optional: an app setting "Tools" could be hidden on phones (already: `ON_THIS_COMPUTER`).

### 3.7 Module layout

**Rust (`APP/src-tauri/src/`)**
```
goldsrc/mod.rs            // shared: Vec3, LE readers, Error type, size caps
goldsrc/bsp.rs            // BSP30 lumps → structs (header, planes, verts, edges, surfedges, faces, texinfo, miptex, models, lighting, clipnodes, nodes/leafs later)
goldsrc/entities.rs       // entity lump → Vec<HashMap>, worldspawn wad list, referenced files (move navs::referenced_files + res.py logic here)
goldsrc/wad.rs            // WAD3 directory + miptex → RGBA; texture lookup order; animated/{ handling
goldsrc/lightmap.rs       // per-face lightmap extents, atlas packing, uv2
goldsrc/mesh.rs           // faces → merged geometry per model/texture, the LSMB bundle writer
goldsrc/hull.rs           // port of surfsim.Hull: clipnode trace, contents, brush-entity hulls, triggers (used by nav fixer "drop a point to the floor", surf lab, health check)
goldsrc/nav.rs            // .nav v5 read (full) + write; BuildLadders from func_ladder; navs.rs/navpack.rs switch to it for parse_head/place_count
goldsrc/lsv.rs            // .lsv v6 read (+ LsNavKey port, stage names, material letters)
tools/mod.rs              // local_routes(): the /api/tools/* handlers, cache keys, dialog
tools/cache.rs            // sys::app_dir()/cache/tools: key = file size + mtime + parser version; prune
tools/replay.rs           // gm_simrec CSV + lstrace.csv → per-bot columns, events, rounds; fall/world-death detection
tools/mapinfo.rs          // one JSON summary per map: bounds, entity counts by class, textures + where found/missing, nav/lsv status, spawns
```
Phase 2 adds `goldsrc/bspwrite.rs`, `tools/navedit.rs`, `tools/health.rs` (reuses `navs::LocalLearner`), `tools/compile.rs`.

**UI (`APP/ui/src/`)**
```
views/tools/Tools.tsx         // the section: a toolbar of tools (Map viewer | Replay | later: Health, Nav fixer…) + the open tool
views/tools/MapViewer.tsx     // page: map picker (reuses MapCard/useMaps), layer panel, inspector, status bar; owns the Canvas
views/tools/viewer/Scene.tsx  // <Canvas> + root rotated group + lights + camera rigs
views/tools/viewer/World.tsx  // the BSP mesh (materials per draw, sky/water/trigger flags)
views/tools/viewer/Entities.tsx  // point entities as instanced billboards by class group; brush entities tinted; picking
views/tools/viewer/NavLayer.tsx  // areas, connections, hiding spots, ladders, place labels; hover/select
views/tools/viewer/KnowLayer.tsx // .lsv: cover rose, sightlines, wallbangs, corners, angles, objects, exposure
views/tools/viewer/ReplayLayer.tsx // bots as hull boxes + view cones, trails, kill/fall markers, heatmap plane
views/tools/viewer/Cameras.tsx // Fly (pointer lock + WASD, shift = fast, GoldSrc speeds), Orbit, Top-down presets
views/tools/viewer/coords.ts  // gs↔three helpers, yaw/pitch → dir, hull sizes
views/tools/viewer/bundle.ts  // LSMB decoder → typed arrays
views/tools/viewer/materials.ts // texture loading (per-name PNG, NearestMipmap/Linear, wrap repeat), lightmap combine
views/tools/panels/Inspector.tsx  // selected entity / nav area / bot: key-values, links, "copy coordinates"
views/tools/panels/Layers.tsx     // switches (Switch component) grouped: World, Entities, Nav, Knowledge, Replay
views/tools/panels/Timeline.tsx   // play/pause, speed, scrubber, round markers, event list
views/tools/toolsApi.ts       // fetch helpers (getBundle(path) → ArrayBuffer, getJson), query hooks
stores/tools.ts               // zustand: open map, layers, selection, camera preset, replay time/speed/following bot
```
Plus: `store.ts` (`Section` + `SECTIONS` + "tools"), `App.tsx` (`NAV` entry, lazy page, route branch), `main.rs::mac_menu` (Go → Tools), `icons.tsx` (`tools` icon), `styles.css` (a `.tools-*` block: the viewer takes the page's full height, panels dock right), `types.ts` (Tools types or `views/tools/types.ts`), `CHANGELOG.md` + site docs page when it ships.

### 3.8 Routes ("Tauri commands")
All GET unless noted, all `Listener::Local`, all answer `{error}` JSON on failure in the app's plain-language style.

| Route | Returns |
|---|---|
| `/api/tools/maps` | the game's maps with tool facts: bsp size, has nav (version, places), has lsv (stage, key matches nav), missing wads/files, recordings available |
| `/api/tools/map/:name/info` | `mapinfo.rs` JSON: bounds, models count, textures `{name, from: "embedded"\|"cstrike.wad"\|…\|"missing"}`, entity counts by class, spawns, worldspawn message/skyname |
| `/api/tools/map/:name/mesh` | LSMB bundle (cached) |
| `/api/tools/map/:name/tex/:tex.png` | RGBA PNG (cached); 404 → the UI draws a magenta checker |
| `/api/tools/map/:name/lightmap.png` | lightmap atlas (cached) |
| `/api/tools/map/:name/entities` | `[{index, classname, origin?, angles?, model?, keys{}}]` |
| `/api/tools/map/:name/nav` | `{version, bspSize, places[], areas[{id, attr, nw, se, neZ, swZ, conns[4][], hiding[], place}], ladders[]}` |
| `/api/tools/map/:name/know` | JSON header (counts, stage, keyMatches, materials) + `sight` bitset and arrays as a small LSMB-style bundle |
| `/api/tools/map/:name/probe?x&y&z&dir=down` | hull trace result (floor height, contents) — the "drop to floor" helper used by nav editing and the inspector |
| `/api/tools/recordings` | known recordings `{id, file, map, when, seconds, bots, hasTrace}` |
| `POST /api/tools/recordings/open` | file dialog (CSV); adds to the list; `{id}` or `{cancelled}` |
| `/api/tools/recording/:id` | replay bundle: players, per-bot columns, events (`K`/`H`/`F`/`E`/`C`/`R`), fall deaths classified |
| `/api/tools/recording/:id/trace` | trace rows per bot + `#why/#attack/#seen/#mark` events, mapped onto the recording's clock |

### 3.9 Dependencies (pinned)
UI (`package.json`): `three 0.170.0`, `@types/three 0.170.0`, `@react-three/fiber 8.17.10`, `@react-three/drei 9.117.3` — the last R3F 8 / drei 9 line that targets React 18 (the app is on 18.3.1; moving to React 19 + R3F 9 + drei 10 is a separate decision). Verify the three/drei pair compiles on install; if a newer 8.x/9.x pair exists at build time, take it. Optional dev: `vitest 2.1.x` for the pure TS modules (bundle decoder, coords, timeline maths).

Rust (`Cargo.toml`): nothing new for parsing (std + existing `image` for PNG encoding, `flate2`, `serde_json`, `sha2`). Phase 2: none for nav/bsp writing; compile step uses `std::process::Command`. Avoid `qbsp`/`goldsrc-rs` as dependencies (see §0).

### 3.10 Testing
- **Rust unit tests** next to each parser with synthetic fixtures: a tiny hand-built BSP (one cube room, two textures, one embedded; the writer we need in phase 2 can be started as a test helper now), a WAD3 with a `{` texture, a 3-area nav written by the `nav.rs` writer and read back, an `.lsv` built from a struct and round-tripped, recording/trace CSV snippets copied from `SIM/results/rats2_v4`. Assert exact struct sizes (`PairInfo` 6, `LsAreaKnow` 315, header 44) and the "length must match" rule.
- **Real-file tests, opt-in:** `LS_TEST_GAME=<Half-Life dir>` runs parsers over every `maps/*.bsp` + `.nav` + `.lsv` there (no panics, mesh triangle count > 0, lsv key matches for pack pairs). Not in CI (no game files), but run locally before a release.
- **Router tests** in `server.rs` style: tools routes 403 on the LAN listener; bad names 400; missing map → a readable error.
- **UI:** manual checklist (de_dust2 textures + lightmaps, cs_rats2 + fixes nav + rats2_v4 recording shows the fall spots, a surf map, de_survivor (biggest stock), a GameBanana map with a missing WAD, a CZ map in CZ) and `npm run build` size check. `vitest` for `bundle.ts`, `coords.ts`, timeline search if added.

### 3.11 Risks
| Risk | Mitigation |
|---|---|
| WebGL in the Tauri webview (WKWebView / WebView2) | Both support WebGL2; test early on a Mac and Windows VM; `frameloop="demand"` keeps the fans quiet. Show a "Your system can't show 3D" `LoadError` if context creation fails. |
| Download/bundle growth (~1.5 MB raw) | Lazy chunk, manual chunks, measured in CI; acceptable for a desktop app that already ships plugins/mods/catalog. |
| Drei/three version churn vs React 18 | Pin; upgrade deliberately. |
| `.lsv` misread (packed layout) | Round-trip tests + `key` check + compare counts with `lonerstrike_mapknow.log` and `mapknow.py` output on de_dust2. |
| Lightmap/texture correctness on odd maps (missing WADs, `+` animations, scaled textures, `{` masks) | Degrade per texture (checker), never fail the map; info route lists what's missing. |
| Big recordings (old_root is 4.6 GB) | Only open per file; stream-parse; cap at e.g. 200 MB with a message. |
| Scope creep into the editor | Phase gates below; phase 1 ships as a viewer with no write path. |

---

## 4. Phase 1 in detail: Map viewer + overlays + replay

Goal: open any installed map in the app, walk around it, see every entity, lay the bots' nav and knowledge over it, and replay a recording on it — enough to do the cs_rats2 investigation in the app instead of from PNGs and CSVs. No file is written.

### 4.1 Navigation and page shell
- `store.ts`: add `"tools"` to `Section` and `SECTIONS`. `App.tsx`: `NAV` gets `{ id: "tools", label: "Tools", icon: "tools", group: "Workshop" }` after Hosting (its own sidebar group so it doesn't read as part of Hosting); `const Tools = lazyPage(() => import("./views/tools/Tools"))`; route branch `section === "tools" && <Tools app={app} maps={allMaps} focus={focus} />`. `main.rs::mac_menu`: a "Tools" item under Go. `icons.tsx`: a wrench/compass glyph.
- `Tools.tsx`: a `SectionHead` ("Tools", note: "Look inside maps, watch the bots learn and play them back") and a segmented row of tools (phase 1: **Map viewer**, **Replay**); the `focus` string opens a tool directly (`"map:de_dust2"`, `"replay:<id>"`), so the Maps page's map sheet gets an "Open in Tools" button via `go("tools", "map:" + name)` (the `focus` mechanism already exists for Settings/Servers).
- Layout: the viewer takes the whole content height (`.tools-page { height: calc(100vh - var(--strip-h) - …) }`, Canvas fills the left, a 320 px dockable panel column on the right with Layers / Inspector / Timeline, the status bar underneath shows the camera's GoldSrc coordinates, the hovered face's texture, and FPS when a dev flag is on). Keyboard: `F` fly / `O` orbit / `T` top-down, `Esc` releases pointer lock, `Space` play/pause in replay.
- Busy states: loading a map runs through `useActions().run("tools:open:" + name, …, "Reading the map…")`; first-time texture/lightmap building on a big map can take a second or two, so the Canvas shows the app's spinner overlay until the bundle resolves, and a `Skeleton` in the panels.

### 4.2 Milestones (each shippable behind the Tools page)

**M1 — Rust readers + untextured mesh (≈ 1 week)**
- `goldsrc/bsp.rs`, `entities.rs`, `mesh.rs`, `tools/cache.rs`, routes `maps`, `info`, `mesh`, `entities`.
- Faces → triangles (fan per face polygon, port of `surfpng.py` polygon code), normals from planes (flip by `side`), per-model draw ranges, `faceOfTriangle`.
- UI: Scene + World (flat shaded by normal, a `MeshLambertMaterial` per draw with a hash colour per texture name), Orbit + Fly cameras, status bar with coordinates. Acceptance: de_dust2 and de_survivor open in under 2 s warm, free fly at 60 fps.

**M2 — Textures and lightmaps (≈ 1 week)**
- `wad.rs` (directory, miptex, palette, `{` masks, lookup order `embedded → wads named by worldspawn in cstrike/, valve/, czero/`), `lightmap.rs` (extents, atlas, uv2, style 0), `tex` and `lightmap` routes, `materials.ts` (repeat wrap, mipmaps, `map × lightMap` with overbright 2.0 and a "Lightmaps" switch; sky as flat colour; `!` water translucent; `aaatrigger`/`clip`-like names flagged).
- `info` lists textures by source and the missing ones (first reuse for the Map doctor idea).
- Acceptance: dust2 looks like the game with lights off/on; a map with a missing WAD still opens with checkers and a banner "2 texture files are missing: …".

**M3 — Entities, picking, inspector (≈ 0.5-1 week)**
- Point entities as instanced billboard sprites per class group with colours: spawns (T orange `--t`, CT blue `--ct`), hostages, bomb/hostage/buy zones (brush, tinted volumes), lights (small), sounds, `info_*`, `env_*`, `trigger_*` (brush; the **Triggers** switch), `func_*` solid brush entities (the **Doors & breakables** switch, tinted), ladders (`func_ladder`, drawn as the nav will use them).
- Raycast picking (face → draw → model → entity, or sprite → entity); hover highlight; the Inspector shows classname, all keys, targets (`target`/`targetname` links clickable), "Copy origin", "Look at". A searchable entity list grouped by classname.
- Acceptance: click a `trigger_hurt` on a deathrun map and read its `dmg`; find all `info_player_start` on rats2.

**M4 — Nav overlay (≈ 1 week)**
- `goldsrc/nav.rs` full reader + `BuildLadders` from `func_ladder` boxes (as `knowscreen.py`), `nav` route; `navs.rs::parse_head`/`navpack.rs::place_count` switch to it.
- `NavLayer`: areas as a merged mesh (colour by place, or by attribute crouch/jump/precise, or by island/connected component; 4-corner heights so slopes show), connections as lines (one-way as arrows, like `navlink.py` directions), hiding spots (dots, flag colours), place labels (`<Text>` at each place's centroid), ladders. Hover shows id/attributes/place; click selects (Inspector: extents, neighbours per direction, hiding/approach counts) and highlights neighbours.
- "Nav coverage" readout: share of upward floor faces (nz > 0.7, world model, like `surfpng`) with an area within 32 units — the same number `sweepreport.py` prints, so the two agree.
- Acceptance: cs_rats2 with `SIM/navpack/fixes/cs_rats2.nav` shows the 12 added beam areas and the one-way drop; isolated islands are obvious.

**M5 — Map knowledge overlay (≈ 1 week)**
- `goldsrc/lsv.rs` (+ `LsNavKey`), `know` route (per-area arrays + sight bitset + pairs + bangs).
- `KnowLayer` modes (Layers panel, radio-style): **Cover** (area tint by mean cover, 16-direction rose on the selected area at crouch/stand heights, materials), **Sightlines** (lines from the selected area to every visible area centre, width/alpha by quality, crouched-only in a second colour; "how much it sees" heatmap from `sees`/`high`), **Wallbangs** (lines coloured by material, label thickness), **Corners** (points + opening direction arrows), **Angles to watch** (arrows with pitch, thickness by weight; the `area` they point to flashes on hover), **Objects** (low cover/doors/breakables/boxes/ladders as boxes), **Exposure** (3×3 points per area coloured by how many angles see them; point cover rosettes). Header facts in the Inspector: version, stage (`pairs/areas/angles/exposure/done`), traces, CPU time, key match.
- Acceptance: pick a dust2 Long A area and see the same lines `mapknow.py` draws; a mismatched nav/lsv pair shows the warning.

**M6 — Replay (≈ 1-1.5 weeks)**
- `tools/replay.rs`: stream-parse recordings (`S/F/H/K/R/P/E/C`), per-bot columns, rounds, kills with `cause: "enemy"|"world"|"trigger_hurt"|"suicide"`, "fall" = world death with |vz| from the last samples beyond the safe fall speed, "stuck" spans (speed < 20 for ≥ 8 s with no enemy in view, as `analyze.py`/`sweepreport.py`), 128-unit grid heat counts (time spent, deaths, stuck). Trace CSV parsed per bot, aligned to the recording clock by `#map` + time; recordings list from `data/simrec/` and the app's own folder; the dialog route.
- `Timeline` panel: play/pause/speed (1-16×), scrubber with round markers and kill ticks, bot list with team colours (click to follow), event feed (kills, falls, chat, events). `ReplayLayer`: bots as hull boxes (standing/ducked) + view cone + name label, trails (last 15 s), kill markers (skull sprite; fall markers in a distinct colour), heatmap switch (deaths / time / stuck). Selected bot's Inspector shows the trace row at the current time (state, task name from the `TaskType` table, `why` + age, `look`, goal as a line to `goal_x/y`, noise as a ping) and the last `#why` notes — "why did the bot do that".
- Acceptance: load `SIM/results/rats2_v4/W1-1_classic_cs_rats2_143716_*.csv` + its trace, jump to each `world` death and see where the bot fell from, with the nav + beam areas on.

**M7 — Polish, tests, docs (≈ 0.5-1 week)**
- Error paths and copy, the fixture tests of §3.10, bundle-size check, CHANGELOG lines, a docs page on the site ("Tools: look inside a map"), a screenshot for What's new.

Phase 1 total: **≈ 6-7 weeks** of focused work (one person), with M1-M3 usable after 2-3 weeks.

### 4.3 Things deliberately left out of phase 1
- Clip hulls as solids (needs clipnode → polygon conversion; BSPguy does it). Instead the `probe` route and a "Walkable floor" layer (sample the hull on a 32-unit grid and colour floor cells by whether a standing player fits) give most of the value for nav work and are cheap with `hull.rs`.
- PVS/leaf culling (not needed for frame rate at these sizes), lightmap styles 1-3 (switchable lights), animated textures, MDL/SPR models in the scene (phase 2 model browser), demo (.dem) playback.

---

## 5. Phase 2 and beyond

### 5.1 Editing: BSP-direct vs a brush project
Two different products; do them in this order.

**2a. BSP-direct edits (BSPguy-style, weeks):** needs `goldsrc/bspwrite.rs` (rewrite all 15 lumps with new offsets; keep unknown bytes intact). Safe, high-value edits: entity add/edit/delete (spawns, buy zones, bomb targets, `trigger_hurt`, lights' values, `worldspawn` message/wad list), rename/replace a texture (same size, or re-point texinfo to an embedded one), move/duplicate brush-entity models, strip a map's embedded textures into a WAD or embed its WADs (fixes "missing wad" downloads), ripent-style export/import of the entity lump. Geometry edits on compiled BSPs (vertex moves, new brushes) are hard (BSP tree, clipnodes, VIS/RAD) and not worth re-doing BSPguy; if ever, bundle BSPguy's CLI on Windows.

**3. Brush editor (months):** own document model (the user's research types are the right shape: `MapDocument{world, brushes[{faces[{plane, texture, uAxis, vAxis, offset, scale}]}], entities, groups}`), saved as our project JSON plus Valve220 `.map` export/import (`nomap` crate as reference), CSG in Rust (convex polytopes from planes, clipping, hollow/carve), R3F viewport with Drei `TransformControls`/`PivotControls` gizmos, texture browser from the WADs we already decode, FGD-driven entity panel (ship a HalfLife + CS FGD; J.A.C.K.'s extensions as reference only), undo as a command stack, BSP import = brush-entity models + world faces as non-editable reference ("trace over") plus entities as editable — be explicit that a compiled BSP can't be turned back into clean brushes. TrenchBroom is the UX reference (clip tool, vertex tool, texture lock). Phase gate: only start after 2a and the nav fixer have shipped.

### 5.2 Compiling
- Toolchain: **SDHLT** (hlcsg → hlbsp → hlvis → hlrad), run as child processes with a progress parser (each tool prints percentages) through the existing setup `Job`/`JobProgress` UI. Distributed as an **optional download** installed from Tools ("Map compiler, about 10 MB"), so the app download stays as is and the licence stays separate (its terms file next to the binaries; the app is free, which the Valve SDK EULA requires). Windows: official zips. macOS: no builds exist; add a CI job that builds SDHLT with clang for arm64 (and x86_64) and sign/notarize with our key — **verify SDHLT compiles on macOS first** (open question §8). Linux servers don't need it.
- Fallback for macOS previews if SDHLT won't build: `ericw-tools qbsp -hlbsp` + `vis` + `light` (GPL-2, Darwin zips) — lower lighting fidelity, no `light_environment`, flagged as "preview".
- Validate before compiling: GoldSrc limits (MAX_MAP_CLIPNODES 32767, planes 32767, faces 65535, lightdata 0x200000 unless `-extra`, entity count, brush count), missing WADs, leaks (parse `.lin`/`.pts` from hlcsg/hlbsp and draw the leak line in the viewer).

### 5.3 Nav fixer (the cs_rats2 fixes as a GUI; ≈ 1.5-2 weeks after phase 1)
- On the nav overlay: **Add area** (drag a rectangle on the floor; z from the hull probe at each corner; attribute checkboxes; place from the nearest named area, as `navarea.py` `place_from`), **Link / unlink** (click two areas; two-way, or one-way "drops to" like `drops_to`), **Delete area** (and its back-links), **Move/resize** (gizmo), **Set place** (dropdown of the place table + new name), **Split** later. Edits are a command stack (undo/redo, `useDirty` so the app asks before leaving).
- Save: `nav.rs` writer emits v5 byte-exact for untouched areas (new areas flat like `navarea.py`), writes `<map>.nav.part` → rename, keeps `<map>.nav.bak` once, refuses while the map is loaded in the game.
- **Rebuild the .lsv automatically**: knowledge is only built by the brain while bots play, so reuse `navs.rs::LocalLearner` (hidden dedicated server on this computer): copy map + new nav, start with bots and `bot_ls_fight 1`, poll `maps/<map>.lsv` stage (4 = done) or the `lonerstrike_mapknow.log` line, copy back, then `navs.rs` sends both to servers as today. The job shows in `JobProgress` ("Bots are studying the new nav… 1 of 4 stages"). Alternative (later): an offline Rust builder using `hull.rs` traces — tempting but it must match the brain exactly (it uses the engine's trace, doors/glass rules, sky brushes), so keep the game as the builder.
- Also here: a "Problem spots" list fed by the health check (§6) and by replays (world deaths, stuck cells) with "Go there" buttons.

### 5.4 Playtesting with bots
- Already possible: Play page launches the game on a map with bots (`/api/app/launch`); the Tools page gets a "Play this map with bots" button that saves (nav fixer) or compiles (editor) first, copies the result to `maps/`, then launches. Scenario tests (spawn validation, traversal, trap testing) = the health check run with a chosen mode and length, report in the viewer.

### 5.5 Surf / deathrun simulation visualisers
- **Surf lab:** needs a fork change: a console command (e.g. `ls_surfdump`) or automatic save of the explorer archive to `maps/<map>.lssurf` (nodes: parent, t, landed, o, v, firstAct, numActs; samples: o, v, move; roots). Then `tools/surf.rs` reads it and the viewer draws the tree as polylines coloured by move/speed, with a scrubber over iterations to "watch it grow", plus the ramp faces highlighted (`ramps.py` logic) and teleports/pushes from entities. Offline: port `surfsim` to Rust (`hull.rs` is most of it) so the app can simulate one line from a clicked start + yaw and compare with the in-game `ls_surfcheck` print.
- **Deathrun:** traps are `trigger_hurt`/`func_door`/`func_breakable`/`trigger_multiple` + buttons; the viewer links `func_button.target` → trap and animates door travel (`angles`/`lip`/`speed`) to show kill volumes; real trap timing needs the game (health check scenario "walk the course").

### 5.6 Game-mode creator
- **Realistic in the app now:** a no-code **mode designer** that writes what the mode system already reads: `plugins/configs/gamemodes/<mode>.cfg` (cvars), `<mode>.plugins.ini` (which plugins), `mappresets.ini`/`maps/` lists, `addons.ini`, bot settings (`botplay.ini`), personalities (`people.ini`) — forms with validation and a "Try it" button; sharing as a zip ("Game Mode Package": manifest + the files above). No engine or client change.
- **Needs a game DLL or plugin change:** custom entities (`surf_start`, `deathrun_trap`…) → either an AMXX plugin that interprets them (preferred, ships with the mode package) or the ReGameDLL fork; node logic / Lua scripting → a new scripting plugin with a sandboxed API (big; AMXX plugins must be compiled, so "visual logic" would be an interpreter plugin reading `logic.json`). **Needs client changes (not possible with the stock CS 1.6 client):** custom HUD elements beyond HUD messages/sprites, new weapons/models beyond replacements, new network messages. Say so in the UI: "Custom modes can change rules, weapons, respawns, teams, bots and map lists. New game mechanics need a plugin."

---

## 6. What else fits under Tools (user's list, scoped)

Reuse key: **V** = the phase-1 viewer, **P** = the Rust parsers, **S** = sim tooling (`LocalLearner` hidden server, gm_simrec, sweep logic), **F** = needs a fork/plugin change.

| Tool | Scope | Reuses | Wave |
|---|---|---|---|
| **Map health check** | Pick maps (or all installed) → the hidden dedicated server (`navs.rs::LocalLearner`, generalised to run a mode with N bots for T seconds with `gm_simrec` + `bot_ls_trace`) → report per map like `sweepreport.py`: crashes, plugin errors (console log), `.lsv` loaded, kills/bot-min, stuck %, still %, nav coverage %, stuck cells, spawn sitters, world-death share; "Open in viewer" places the spots. Runs in the background at low priority with `JobProgress`. | S, P, V | **First wave** |
| **Nav fixer** | §5.3: problem spots from the health check and replays on the nav overlay; add/link/unlink/delete/move areas; set places; save; rebuild .lsv via the hidden server; send to servers. | V, P, S | Second (right after first wave) |
| **Replay & heatmaps** | Phase 1 M6: routes, deaths, falls, fights (`#attack/#seen`), chat; heatmaps for time/deaths/stuck; A/B compare two recordings side by side later. | V, P | **First wave** (in phase 1) |
| **"Why did the bot do that"** | Phase 1 M6 Inspector + a dedicated timeline strip per bot (state/task/why bands over time, like a flame chart; click → jump). Shipping only trace parsing + UI. | V, P | First wave (in phase 1), strip view second |
| **Map knowledge viewer** | Phase 1 M5. | V, P | First wave (in phase 1) |
| **Surf / deathrun lab** | §5.5; blocked on a fork dump command for the archive; the offline surfsim port works without it. | V, P, F | Third |
| **Map doctor** | For downloaded maps: parse the entity lump for `.mdl/.spr/.wav` and `wad` (as `navpack/res.py` and `navs::referenced_files`), check they exist in `cstrike`/`valve`/`czero` (and the GameBanana archive the map came from), generate a `.res` file, embed missing WAD textures (2a writer), nav present? lsv present? Hook into Get more's install check (which already refuses maps missing a model). Mostly Rust + a list UI; no 3D needed. | P (+ 2a writer for fixes) | Second (cheap, high value) |
| **Callout editor** | Place names on the nav overlay: rename, merge, draw a place over areas (rules like `callouts.py`, applied by area centre), copy names from the CZ nav (`names.py::name_areas` ported to Rust). Writes v5 place table in place (ids/extents unchanged, so the .lsv stays valid). | V, P | Second (with the nav fixer) |
| **Texture (WAD) and model (MDL) browser** | WAD: grid of textures from the game's WADs and a map's embedded ones (`wad.rs` → PNG), search, "which maps use it". MDL: web-hlmv (MIT) in an R3F canvas or a Rust MDL decoder; sequences, skins, bodygroups; "used by these maps/skins". | P, V (canvas) | Third |
| **Spray maker** | Image → `tempdecal.wad` (one 64×64-ish paletted miptex, quantised to ≤ 256 colours with optional `{` transparency; `content.rs` already knows `tempdecal.wad`). Rust: `image` crate + a median-cut quantiser (~200 lines) + the WAD3 writer; UI: pick image (dialog), crop/size, preview, "Use as my spray" (copies into `cstrike/` and sets `cl_logofile`). No 3D. | P (WAD writer) | **First wave** (small, fun, independent) |
| **Crosshair / config / binds builder** | Forms → `userconfig.cfg`/`autoexec.cfg` lines (binds, crosshair cvars, rates), with a crosshair preview; read the existing files first and only touch a marked block. No 3D. | — | Second/third (small) |
| **Demo manager** | List `cstrike/*.dem` with map/date/size from the header, rename/delete, "Play in game" (`playdemo` via launch args), later playback in the viewer (hlviewer.js shows `.dem` parsing is doable in TS; a Rust port fits our pattern). | P (later V) | Third |
| **Bot personality editor** | Forms over `plugins/configs/gamemodes/people.ini` (archetypes, weights, conflicts) and per-bot profiles; preview "who shows up" with the same name-seeded roll the plugin uses (port the roll to Rust to show the real result); the Bots page already edits related settings. | — | Later |
| **No-code game mode designer** | §5.6: forms writing `<mode>.cfg`, `.plugins.ini`, map lists, addons, bot settings; export/import as a package; "Try it" launches. Builds on the existing mode/cfg system, no engine change. | S (launch) | Later |

**Suggested first wave alongside the viewer:** Map health check, Replay & heatmaps (part of phase 1), Spray maker. The health check reuses `LocalLearner` and the sweep report logic; the spray maker needs only the WAD writer and is a good small first shipping Tool while the viewer is still being built. The nav fixer, map doctor and callout editor are the natural second wave because they share the nav writer and the viewer's selection model.

---

## 7. Effort summary (one focused person)

| Phase | Weeks |
|---|---|
| 1. Viewer + nav/.lsv overlays + replay (M1-M7) | 6-7 |
| First wave extras: spray maker 0.5-1, health check 1.5-2 (hidden server generalisation + report) | 2-3 |
| 2. Nav fixer 1.5-2, map doctor 1, callout editor 0.5-1, BSP entity/texture edits (writer) 2, WAD browser 0.5 | 5-6 |
| 3. Compiler packaging (SDHLT mac build, job UI, validation) 2-3; brush editor MVP (boxes, move/scale, textures, entities, .map export, compile, play) 8-12; clip/vertex tools, prefabs, undo polish +6-8 | 16-23 |
| Surf lab (incl. fork dump command) 1.5-2; MDL browser 1-2; demo manager 1-2; config/binds 0.5-1 | 4-7 |
| 4. Mode designer (no-code) 3-4; personality editor 1; packages/sharing 2 | 6-7 |

---

## 8. Open questions

1. **Release timing.** 1.0.0 is meant to be the definitive release with a long gap after it (memory note). Does Tools go into 1.0.0 (phase 1 only, as a "preview" section) or open the 1.1 line? Phase 1 is ~6 weeks; it would push 1.0.0.
2. **Sidebar placement.** A new "Workshop" group with one "Tools" entry (internal tool switcher), or several sidebar entries (Map viewer, Replay, Health…)? The plan assumes one entry with a switcher; the macOS Go menu gets one item.
3. **React 19 / R3F 9?** Staying on React 18 pins us to R3F 8 + drei 9 (maintenance line). Upgrading React is a separate, app-wide change.
4. **Lightmap look.** Ship with the overbright/gamma that matches the game's default `gamma 2.5 / lightgamma 2.5`, or add a slider? (Start with a fixed ×2.0 and a switch; tune on dust2.)
5. **SDHLT on macOS.** Does it build with clang on arm64 (it's CMake and CI builds Linux, so likely)? Who hosts the optional download (`dl.lonerstrike.com`, like the nav pack)? Only matters for phase 3.
6. **.lsv rebuild**: keep the game as the only builder (slow but exact) or invest in an offline Rust builder that must match the brain's traces? Plan says the game, via the hidden server.
7. **Surf archive dump format**: a fork change is needed; agree the file layout (`maps/<map>.lssurf`) with the brain work so both sides land together.
8. **Recordings on servers**: should `gm_simrec` ship as an optional add-on (per server/game, like voice) so the health check and replay work on any machine, or stay a sim-only plugin copied in by the health check job only? (Plan: the health check installs it only into the hidden server; players never see it.)
9. **Phone access**: Tools are computer-only in this plan. OK?
10. **Texture names and copyrighted content**: the viewer shows Valve textures from the user's own install (fine); the WAD browser must never export them into shared packages by default.
