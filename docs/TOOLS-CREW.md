# Tools crew brief (phase 1: the map viewer)

Read docs/TOOLS-PLAN.md first (sections 0, 3 and 4). This brief says who builds what, so three
people can work in this worktree at once without stepping on each other.

## Where and how
- This worktree (`~/loner-strike-tools`, branch `tools`) is separate from the release work in the
  main checkout. Commit here in small, clear commits (plain-language messages, no AI co-author
  lines). Don't push, don't merge, don't touch other branches or the main checkout.
- Only edit the files you own (below). If you need a change in someone else's file, write it in
  `docs/TOOLS-NOTES.md` under "Requests" and work around it.
- Build and test what you write: `cd app/src-tauri && cargo test` / `cargo build`;
  `cd app/ui && npm install && npm run build`. Leave the build green when you stop.

## Product direction (from the user)
- A modern Hammer: keep the depth mappers need (real brushes, entities, texture alignment,
  compiling), but modernise the experience. Plain words, obvious defaults, everything
  discoverable. Phase 1 is a viewer: no editing.
- For the editor phases, design toward: one 3D view first (2D top/front/side a key away, not four
  panes up front); direct manipulation (click a face and drag to push/pull, W/E/R gizmos, snapping
  on by default, Alt to ignore it); brushes under the hood with modelling-style tools (box,
  cylinder, clip, hollow) and click-and-drag texture alignment with live preview; an entity
  inspector with real controls (colour pickers, angle dials, sound preview, tooltips) and
  target/targetname links drawn in 3D; live checks (leaks, BSP limits, missing textures, spawns)
  instead of after a compile; one-click playtest with our bots and our overlays; prefabs and a
  searchable drag-and-drop asset browser.
- Follow the app's existing patterns: React 18 + TypeScript + Vite + Zustand + TanStack Query in
  `app/ui`; the Rust axum server in `app/src-tauri/src/server.rs` (no Tauri `invoke`); slow
  actions show a spinner and a label; UI copy is plain and friendly (see existing views).

## The contract (agree on this, don't change it alone)
- Routes and the LSMB mesh bundle: TOOLS-PLAN.md section 3.3 and 3.8, exactly.
- Coordinates: GoldSrc units everywhere in buffers; one root group rotated -90 deg about X (3.2).
- Map names validated with the existing `maps::safe_arg`; tools routes only on the local
  listener; parsers never panic on bad files (return a readable error).

## Who owns what
- **Rust core (agent A):** `app/src-tauri/src/goldsrc/{mod,bsp,entities,wad,lightmap,mesh}.rs`,
  `app/src-tauri/src/tools/{mod,cache,mapinfo}.rs`, the one-line hookups in `main.rs`/`server.rs`
  needed to mount `tools::local_routes()` on the local listener, and Cargo.toml if truly needed.
  Routes: `/api/tools/maps`, `/map/:name/info`, `/mesh`, `/tex/:tex.png`, `/lightmap.png`,
  `/entities`. `tools/mod.rs` must also `pub mod overlays;` and merge `overlays::routes()` into
  the local routes; create `tools/overlays.rs` with an empty `pub fn routes()` only if it doesn't
  exist yet (agent C fills it).
- **Rust overlays (agent C):** `app/src-tauri/src/goldsrc/{hull,nav,lsv}.rs`,
  `app/src-tauri/src/tools/{overlays,replay}.rs`. Routes (in `overlays::routes()`):
  `/api/tools/map/:name/nav`, `/know`, `/probe`, `/recordings`, `POST /recordings/open`,
  `/recording/:id`, `/recording/:id/trace`. Add `pub mod hull; pub mod nav; pub mod lsv;` to
  `goldsrc/mod.rs` only by appending those lines (agent A owns the rest of that file).
- **UI (agent B):** everything under `app/ui/src/views/tools/`, `app/ui/src/stores/tools.ts`,
  the small hookups in `store.ts`, `App.tsx`, `icons.tsx`, `styles.css` (a `.tools-*` block),
  `vite.config.ts` (manual chunks) and `package.json` (three / R3F / drei pinned as in 3.9).
  Until the Rust routes exist, build against the contract with a small mock in `toolsApi.ts`
  behind a dev flag.

## Reference code (real and working; port, don't copy blindly)
- BSP faces, textures, entities, hull traces: `/Users/jameson/loner-strike-sim/surfsim.py`,
  `surfpng.py`, `ramps.py`, `surfmap.py`. Nav v5 read/write: `/Users/jameson/loner-strike-sim/navpack/navarea.py`,
  `navlink.py`. .lsv layout: `/Users/jameson/Nextcloud/Jameson/Development/Other/ReGameDLL_CS/regamedll/dlls/bot/cs_bot_ls.cpp`
  (LSV save/load) and `/Users/jameson/loner-strike-sim/mapknow.py`. Recordings: `/Users/jameson/loner-strike-sim/gm_simrec.sma`,
  `analyze.py`; samples in `/Users/jameson/loner-strike-sim/results/rats2_v5/`.
- Game files to test with: `~/Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life/cstrike/maps/` (de_dust2, cs_rats2, surf_ski_2...);
  the fixed nav `/Users/jameson/loner-strike-sim/navpack/fixes/cs_rats2.nav` (+ .lsv).

## When you finish a piece
Append to `docs/TOOLS-NOTES.md`: what you built, how to try it, what's left, and any requests.

## Release target (from the user)
Tools ships in 1.0.0 as **Tools (Beta)**: phase 1 only (viewer, entities/inspector, nav and
knowledge overlays, replay). No editing. Solid over complete: clean errors, no crashes on odd maps,
main bundle unchanged. It merges to main only after two reviews, the security fixes with tests,
green cargo test / npm run build, a size check and a real check in the app.
