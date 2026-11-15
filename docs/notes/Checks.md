# Checks notes (map editor: the live checks)

## 2026-11-13 — deeper live checks in the Checks panel
The basic checks (`editor/checks.rs`: limits, broken brushes, textures, spawns, targets) are unchanged and still
what `checks::run` returns. `checks::run_with` adds the deeper ones (the route uses it), in `editor/checks/`:

| # | check | code(s) | one-click fix (one undo step) | where |
|---|---|---|---|---|
| 0 | z-fighting: drawn faces coplanar, overlapping (16 sq units+), same facing, on different brush entities or an entity and the world (world/func_detail/func_group against each other is skipped; NULL, SKIP, CLIP, AAATRIGGER, hint, origin, sky, trigger/zone/ladder brushes, hidden visgroups and faces nobody sees are skipped) | `z-fighting` | Nudge 1 unit (the entity's face out); also NULL the hidden face (when one covers the other), Make it world (plain func_wall) | surfaces.rs |
| 1 | stuck in solid: spawns with the player hull, VIP start, hostages, armoury_entity / weapon_ / item_ / ammo_ boxes; buried func_button | `stuck-in-solid` | Move it to free space (up first, then rings out to 128; spawns keep clear of other spawns) | things.rs |
| 2 | micro-gaps under 1 unit between structural world brushes (measured over the shared part); axis-aligned brushes with sides off whole units | `micro-gap`, `off-grid` | Close the gap (moves the off-grid face onto its neighbor); Snap to whole units | surfaces.rs |
| 3 | CS gameplay by prefix: under 10 spawns a team (de_/cs_/as_/es_), spawns overlapping, spawns with no walkable floor within 64, buy zones (spawns outside their team's zone; a note when there are none), de_ bomb site, cs_ hostages + rescue (rescue missing is a note: CT spawns count), as_ VIP start + safety zone, es_ escape zone, info_map_parameters (one only, buying 0-3, bomb radius 1-2048). Quiet for ls_, br_, deathrun_/dr_, fun_, surf_, jail_/jb_/ba_jail and maps with `ls_modes` (mode kits check those) | `few-spawns`, `spawns-overlap`, `spawn-no-floor`, `no-buyzone`, `spawn-no-buyzone`, `no-bombsite`, `no-hostages`, `no-rescue`, `no-vip`, `no-vip-zone`, `no-escape`, `map-parameters` | Place a spawn / hostage / VIP start; overlap: Move it to free space | things.rs |
| 4 | faces nobody can see: against a solid (covered completely by opaque solids: world, func_detail, solid func_wall) or the void (outside a sealed map), not NULL; static brushes only (moving ones show their faces). Count, how many face the void, and the saving: lighting KB and faces the compiler would keep (it drops world faces covered by world brushes and the outside itself) | `hidden-faces` | NULL them all (worked out again when clicked) | surfaces.rs, views.rs |
| 5 | texture seams: neighboring coplanar faces with the same texture and axes sharing an edge, scale or shift (mod the texture size; 16 when the size isn't known, so no false alarms) different | `texture-seam` | Align with neighbor | surfaces.rs |
| 6 | stretched textures: scale under 0.1 or over 4 (Settings in the panel change both; kept per computer) | `stretched-texture` | Reset the scale to 1 (keeps the face's middle in place) | surfaces.rs |
| 7 | ladders: no walkable floor near the foot, no ledge near the top (96+ tall ladders), not within 2 units of a wall (on its own only a note: it still climbs) | `ladder-gap` | (hint) | things.rs |
| 8 | lighting: sky with no light_environment; enclosed rooms (sealed open space of 128³ or more) with no light, light texture or sky; lights inside solid (light, light_spot) | `no-sun`, `dark-room`, `light-in-solid` | Place a light_environment / a light; Move it to free space | things.rs, views.rs |
| 9 | heavy views: from the last compile's VIS data (work folder or cstrike/maps .bsp: faces each leaf can see, cached by file) or, without one, a sampled estimate (rays from standing spots through a coarse grid). Up to 5 hotspots far apart (over 2500 faces in a leaf's potential view, 1200 sampled), drawn in 3D, with the tips (HINT, func_detail, NULL); when half the map is over the limit it's one line for the whole map instead (an open map: br_valley's leaves see ~9800 faces, ls_outpost's ~1100-1250) | `heavy-view` | (hint) | views.rs, tools/editor/heavy.rs |

Every finding has `at` (shown, Look flies there, a dot or a hotspot ball in 3D), Pick (brush/entity, the face too),
`hint` (the suggested fix), and `fix` + `fixData` / `moreFixes` for the one-click fixes (`checks/fixes.rs`, run from
`history::cmd_fix`). The panel moved to `ChecksPanel.tsx` (filters by level, groups by kind, Fix all per kind,
Settings, Show in 3D); `CheckMarkers.tsx` draws the findings (mounted in EditorScene next to LeakLine).

**Speed** (release, Apple Silicon, the load ~20): ls_outpost (229 brushes) 54 ms, br_valley (5204 brushes) ~370 ms
(faces nobody can see ~200 ms, the sampled views ~100 ms). The coarse grid is 32-unit cells, doubled until it's under
3M cells.

**Tests**: `cargo test editor::checks` (a purpose-built map per check and its fix, undo; our maps;
`LS_CHECKS_RMF=<file.rmf>` a Hammer import; `LS_CHECKS_MAPS=1` prints what they find). Playwright:
`tests/editor-checks.spec.ts` (Chromium) and `tests/editor-checks-webkit.spec.ts` (WebKit), both running
`editor-checks.shared.ts`: the panel, Pick/Look, Move it to free space (one undo), Nudge 1 unit, filters, the
scale limits.

## Findings on our maps (all checked by hand; real, left to the maps' builders)
- **ls_outpost**: 2 lights give no light: the floodlight's `light` at -1096 -264 340 is inside the mast (func_detail
  -1104..-1040, -272..-208, 0..352), the red route's at 1068 300 120 inside its own ~SPOTRED block. 437 faces nobody
  sees (124 the compiler keeps, ~11 KB of lighting). No z-fighting, gaps, stuck things, seams or dark rooms.
- **br_valley**: 12 lights inside solids (tunnel lights at z -360 inside OUT_RK4 rock func_details, lamp lights inside
  their CSTRIKE_ME4METL poles); 8 z-fighting spots (light-strip func_illusionary flush with the dock at z 65, an OUT_WD
  post, the pavement at -320, and strips crossing at -464); 48 texture seams (OUT_RK3 cliffs with different scales on
  neighboring brushes), 101 brushes off whole units (stairs and ramps cut in thirds: on purpose, notes only); 9594
  faces nobody sees (3183 kept: ~1.4 MB of lighting, mostly rocks pushed into the terrain).
- **Hammer import** (ReGameDLL's regamedll_test_map_v8.rmf, 760 brushes, ~60 ms): 3 func_water surfaces flush with the
  floor around them (z -32, 48x48 each: z-fighting), 47 faces of `black` scaled 0.06 x 0.13 (on purpose, but they are
  what the check is for), 28 seams, 2 ladders a few units off their walls (notes), 3 CT spawns outside the CT buy zone,
  2 + 2 spawns (no prefix: notes off). No stuck spawns, no lights in solid.

## Requests from Checks
- **P5**: the Checks panel moved to `ChecksPanel.tsx`; dockPanels.tsx imports it from there (one line) and
  EditorScene mounts `<CheckMarkers />` after `<LeakLine />` (one line, marked `// Checks`). The old `ChecksPanel` and
  `FIX_LABEL` in Panels.tsx are unused now: please delete them when you next commit Panels.tsx (I left your file alone).
  Also `assets.rs` builds its Issue with `..checks::issue(..)` now (Issue grew fields), and `budget::work_dirs` is
  `pub(super)` (heavy.rs reads the compiled .bsp from there).

## 2026-11-13 — the checks run off the editor's lock
`GET checks` takes a snapshot under a brief lock (a clone of the document, the cached WAD set, the asset places),
runs every check without the lock, and answers `{"stale": true}` when the document changed meanwhile (the page drops
it and asks again). The last answer is kept per editor by (document, revision, scale limits), so asking again for the
same document is instant. The page debounces (350 ms) and runs one check at a time (another after it when edits
came in). Fixes apply to the current document by id, as before. Test: `checks_run_off_the_lock` (an edit made while
the checks run on a 3600-box map goes through at once; the run answers stale; a fix still applies after another edit).

## 2026-11-13 — maps test not flaky
`our_maps_pass` (was `our_maps_pass_and_run_fast`): the speed limit is only checked with `LS_CHECKS_TIMING_TEST=1` in a
release build (a busy Mac or a debug build made it flaky). ls_outpost's findings are enforced always; br_valley's
(being rebuilt) only with `LS_CHECKS_STRICT_MAPS=1`. Its current .map has 6 spawns stuck in solid (BRmap request).

## 2026-11-13 — br_valley strict again
br_valley v6 (71f7c21e) has only notes left, so `our_maps_pass` enforces it always, like ls_outpost (no allowances for it).
