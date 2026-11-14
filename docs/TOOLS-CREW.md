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
Tools ships in 1.0.0 as **Tools (Beta)** (scope since widened: see "Scope update" below). Viewer first (entities/inspector, nav and
knowledge overlays, replay). No editing. Solid over complete: clean errors, no crashes on odd maps,
main bundle unchanged. It merges to main only after two reviews, the security fixes with tests,
green cargo test / npm run build, a size check and a real check in the app.

## Scope update (from the user): editing and the spray maker are in 1.0.0 too
All as Beta. Order: viewer (A, B, C) → spray maker (D, independent, now) → nav fixer (after nav.rs
writer + viewer land) → map editing: BSP-direct edits first (entity keys, move/place entities,
texture swaps — works on any map), and the modern-Hammer brush editor (geometry core E now; UI
after the viewer). Compiling is the riskiest part (GoldSrc compilers are Windows tools under
Valve's SDK terms): if it isn't ready, the editor ships with .map export + direct BSP edits.

- **Spray maker (agent D):** `app/src-tauri/src/goldsrc/wadwrite.rs`, `app/src-tauri/src/tools/spray.rs`
  (its own `pub fn routes()`; ask A in TOOLS-NOTES to merge it, or add `pub mod spray;` + merge
  line in tools/mod.rs yourself only after A has finished), `app/ui/src/views/tools/spray/*`.
- **Editor core (agent E):** `app/src-tauri/src/editor/*` (brush geometry, plane math, CSG ops,
  .map read/write, the map document model, command-based undo/redo) — pure Rust with tests, no
  routes or UI yet; mount `pub mod editor;` in main.rs with one line.

## More tools (user approved, 2026-11-12)
Each new tool lives in its own folder (`app/ui/src/views/tools/<tool>/`, `app/src-tauri/src/tools/<tool>.rs`
with its own `pub fn routes()`), mounted with one `pub mod` + one merge line in tools/mod.rs, and
registered with ONE appended line in `app/ui/src/views/tools/registry.tsx` (B creates it; re-read
the file right before appending). Shared files: one-line additions only.
- **F: map health check** (bots on any map in the background → report: falls, stuck spots, coverage, errors).
- **G: map doctor + callout editor** (missing files / .res / nav present; name places on the nav).
- **H: texture & model browser** (WAD textures, MDL models in 3D, GameBanana browse + import, own files).
- Queued: crosshair & config builder, demo manager (list/play via the game's +playdemo; later parse
  .dem into the 3D replay), bot personality editor, game mode lab (surf/deathrun lab + no-code designer).

## Committing (required, 2026-11-12)
Many agents share one staging area. Never run a bare `git commit`. Always commit with paths:
`git commit <your paths> -m "..."` (this commits only those paths, whatever else is staged) and never
`git add` other people's files. Before committing, check `git diff --cached --stat` shows only yours.
- Never swap, stash, overwrite or restore files on disk to separate your changes from someone else's uncommitted work in a shared file, not even for a moment. If you can't commit only your lines, stage just your hunks with a separate index (GIT_INDEX_FILE=… git apply --cached / git update-index), or leave a note in TOOLS-NOTES for the file's owner to commit them.
- Never use broad `pkill -f <pattern>` / `killall`: other agents run the same kinds of servers. Stop only processes you started, by PID (record it when you start one) or by a pattern that includes your own unique port/folder.
- Notes: from now on, write your progress notes in your OWN file `docs/notes/<your agent name>.md` (e.g. docs/notes/P5.md), not in the shared TOOLS-NOTES.md — several agents committing one shared file keeps sweeping others' text into commits. Cross-agent requests go in the recipient's file under "Requests from <you>". TOOLS-NOTES.md stays as the history.
- Disk (2026-11-13, the disk filled up twice): build only in the shared `app/src-tauri/target` — no private `--target-dir`, no extra clean copies of the repo with their own builds. If you truly need a clean-copy build, use `cargo clean --target-dir <it>` and delete the copy as soon as you're done, in the same session. Downloaded test corpora go in one folder you delete when finished. Check `df -h /` before big builds/downloads; under 10 GB free, stop and tell the coordinator.

## Keep the user's Mac usable (2026-11-13)
- Map compiles (sdHLCSG/BSP/VIS/RAD): at most `-threads 6`, always under `nice -n 15`, and `-full` VIS only for the final
  shipping compile. A 14-thread full VIS pushed the load to 58 with swap full and killed a window on the user's desktop.
- Up to 4 of your own sim servers at a time (user, 2026-11-13), as long as the whole Mac stays under 8 sim servers in
  all (`pgrep -fl 'Sim\\.*hlds.exe' | wc -l`) and the load average is under 20. Only 2 while you're also compiling.
  Delete each sim copy when its run ends. `cargo` builds: `-j 6`.
- Check `uptime` before a heavy step; if the load average is over 20, wait.

## Commits in a shared worktree (2026-11-13)
- Everyone shares one git index. Never leave files staged (`git add` then wander off), and never commit from the index:
  always `git commit -m "..." -- <your paths>` against the live tree, then check `git show --stat` touches only your
  files and lines. Two commits today swept in other agents' work (one reverted a perf commit, one took staged WIP).

## Test on current builds (user, 2026-11-14)
- Every sim, stand, demo or QA run uses current builds: the newest green **ls-next3** brain build (the run whose
  head SHA matches `git ls-remote ... refs/heads/ls-next3`; check it the moment you start, not from memory) and the
  plugins and maps from the current `tools` HEAD, compiled fresh. Verify the brain inside the sim (grep mp.dll for the
  features you're testing) before trusting a result.
- Testing your own unmerged brain branch is fine, but say so in the report, and re-run on ls-next3 after it merges.
- Results from an older build don't count: say "old build" and re-run.
- LSPLAY/BRPLAY and any file handed to the user get the newest ls-next3 build plus the current plugins.
