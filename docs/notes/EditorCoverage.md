# Map editor coverage sweep (1.0, Beta)

QA, 2026-11-14. Goal: every editor tool, menu command, key, panel and setting used at least once, its
effect checked (document, selection, view, file written, dialog shown) and Undo checked where it applies.

## How it was run

- **Rust:** the committed `tools` HEAD **8a46ebf4** (`git archive` into a scratch folder, built in the shared
  target), `serve_tools_dev` on its own port. **UI:** the live `app/ui` tree served by its own Vite (own
  `cacheDir`, HMR off), so UI commits that landed during the run (up to 564fc781) were in it; the only
  editor-side ones were contrast colors and the TrenchBroom wording (5e5202a3).
- **Game:** `Sim/QAEC`, an APFS clone of `Sim/LSPLAY` (deleted afterwards). **App data:** a throwaway
  `LS_APP_DIR` and `TMPDIR` in my scratch folder. Never the real game or the user's app data.
- **Browsers:** Chromium for everything; WebKit (the Mac app's engine) for the input- and drawing-sensitive
  specs (keys, panels and tools in the views).
- **Compile and playtest:** see "Compile, playtest and the sim" below.
- **Specs** (all in `app/ui/tests/`, config `editor-coverage.config.ts`, helpers `editor-coverage.helpers.ts`,
  the shared command cases `editor-coverage.cases.ts`):

| Spec | What |
|---|---|
| `editor-coverage-commands.spec.ts` | every command in `commands.ts` from its menu (or the palette when hidden), effect + Undo; a guard test fails when a command has no case or a label/menu changes |
| `editor-coverage-keys.spec.ts` | every key of the three key sets runs its command's case; the old handler's keys (arrows, Page Up/Down, Esc, 1-9 and Ctrl+1-9, Shift+1-4, Alt, Tab/Enter in Clip), fields swallow keys; a guard test fails when the key tables drift |
| `editor-coverage-dialogs.spec.ts` | each dialog's own controls, Escape on every dialog |
| `editor-coverage-panels.spec.ts` | Inspect (brush, entity, several, nothing), Outline, Textures, History, Checks, Budget, I/O, Gameplay, Content, Prefabs, dock, tool rail and tool options, tools in the views, status bar, MCP selection |
| `editor-coverage-file.spec.ts` | start screen, Save / Save as / Export / Open / Recent / imports, recovery, Project panel |
| `editor-coverage-map.spec.ts` | Compile panel, Playtest (four ways), leaks (compile and pointfile), Publish, Cordon |
| `editor-coverage-trenchbroom.spec.ts` | TrenchBroom fixtures (`fixtures/trenchbroom/`, `make_fixtures.py`): import, groups and layers, export, read back |

### Rerunning before a release

```sh
# 1. a dev server over a sim copy, with throwaway app data; the file dialogs answer with
#    LS_DEV_PICK (a symlink the specs re-point) and LS_DEV_PICK_SAVE
LS_TEST_GAME=<sim copy> LS_APP_DIR=<scratch>/appdir TMPDIR=<scratch>/tmp LS_DEV_PORT=<port> \
LS_DEV_PICK=<scratch>/pick/current.map LS_DEV_PICK_SAVE=<scratch>/out/qa.lsmap \
  cargo test serve_tools_dev -- --ignored --nocapture
# 2. Vite with its own cache (docs/TOOLS-CREW.md), LS_API=http://127.0.0.1:<port>
# 3. the specs (LS_COV_OUT keeps the results out of the shared test-results/)
LS_VITE_PORT=<vite> LS_COV_PICK=<scratch>/pick/current.map LS_COV_SAVE=<scratch>/out/qa.lsmap \
LS_COV_OUT=<scratch>/pw npx playwright test -c tests/editor-coverage.config.ts
```

The dev server tells a project, a .map, a leak line and an FGD apart **by the picked path's extension**, so
one server can't answer every dialog. Run the file and map specs again with `LS_DEV_PICK` (and `LS_COV_PICK`)
ending `.lsmap` (Open a project) and `.lin` (Load pointfile); each test skips on a server it can't use. (An
FGD is read by its content, so Add your own FGD works on the `.map` server.)

## Totals

**Inventory items: 190. PASS 180, FAIL 5, NOT AUTOMATABLE 5.** (A row that names several things, e.g.
"Flip along X / Y / Z ×3", counts each; every key binding is tested on its own (89 key tests per engine) but
counted with its command's row.)

Spec runs. Chromium: commands 136/136; keys 89 key-binding tests + 9 others, all pass; dialogs 17 (2 fail,
F-1); panels 18/18; file 9/9 across the `.map` and `.lsmap` servers; map 12/12 across the `.map` and `.lin`
servers; TrenchBroom 9 (4 fail, TB-1..TB-4). WebKit: keys and panels, 113 passed, 3 skipped (clipboard keys).
The existing editor specs cited below ran too, against the same server: 44 passed, 1 failed
(`editor-kits` "ls_creature's Type lists every creature type", because its creature picker (212c68b3) is newer
than the Rust build under test: not counted; rerun on one build).

---

## FAILs

### F-1. Escape stops closing a dialog once the focused button goes away or greys out (and drops the selection instead)
- **Repro (Transform):** pick a brush, Edit › Transform…, type 32 in Move X, click **Apply**, press **Esc**.
  The dialog stays open, the selection behind it is dropped and the dialog then says "Pick something first."
  The app behind is still inert, so the menus can't be used until **Close** is clicked.
- **Repro (Change keys):** Help › Change keys…, find "Bigger grid", click the **×** on `]`, press **Esc**: the
  sheet stays open (same cause: the × that had focus is gone).
- **Why:** `Sheet` (`app/ui/src/ui.tsx`) listens for Escape on its own element only; Apply (a `BusyButton`)
  disables itself while it works and the removed × unmounts, so focus falls to `<body>`, and Esc reaches the
  editor's window handler (`MapEditor.tsx` `down`, "Escape" deselects), which checks the editor store's
  `dialog` but not the P5 store's (`useP5().dialog`).
- **Expected:** Esc closes the top dialog wherever focus is; the editor ignores Esc while any dialog is open.
- **Spec:** `editor-coverage-dialogs` › "Transform: after Apply…", "Change keys: after removing a key…".
- **Likely owner:** P5 (MapEditor key handler, 552ddb24 / 58481bf3; P5 dialogs) with whoever owns `Sheet`
  in ui.tsx (cd4e0c6b, UI foundation).

### TB-1. A hidden or locked TrenchBroom layer comes in shown and unlocked
- **Repro:** File › Import a .map… on `app/ui/tests/fixtures/trenchbroom/tb_valve_layers.map` (layer
  "Details" has `"_tb_layer_hidden" "1"` and `"_tb_layer_locked" "1"`). The group "Details" comes in with
  `hidden: false, locked: false` (`GET /api/tools/editor/doc` › groups).
- **Expected:** the editor group is hidden and locked (`Group` has both flags; `group_flags` exists).
- **Spec:** `editor-coverage-trenchbroom` › "a hidden, locked TrenchBroom layer…".
- **Likely owner:** editor core (agent E), `editor/mapfile.rs` `read_map` (418c052b).

### TB-2. A TrenchBroom layer marked "omit from export" is exported and compiled
- **Repro:** import `tb_valve_layers.map` (layer "Notes", `"_tb_layer_omit_from_export" "1"`, an AAATRIGGER
  box), File › Export a .map…: the box is in the export; compiled, it's a visible world brush in the BSP.
- **Expected:** like TrenchBroom: left out of exports and compiles (or at least said on import).
- **Spec:** `editor-coverage-trenchbroom` › "a layer TrenchBroom leaves out of exports…". Compile: below.
- **Likely owner:** editor core (agent E), `mapfile.rs` (418c052b); compile writes through the same writer
  (`compile.rs` `write_map(.., WriteOptions::default())`).

### TB-3. Export drops TrenchBroom's groups and layers (a trip back to TrenchBroom loses them)
- **Repro:** import `tb_valve_layers.map` (6 groups/layers), File › Export a .map…, import the export:
  0 groups. `mapfile::WriteOptions { trenchbroom_groups }` exists and is tested, but nothing sets it:
  `editor.rs` `export_map`, `mapsource.rs` (Share and remix's "a Valve 220 .map that TrenchBroom, Hammer
  and J.A.C.K. open"), prefabs and compile all pass `WriteOptions::default()`.
- **Expected:** File › Export a .map… (and the remix source) keeps groups/layers as `func_group` `_tb_*`
  entities (the compile can keep the default), or the export says groups don't come along.
- **Spec:** `editor-coverage-trenchbroom` › "Export keeps every brush face and entity; groups survive…"
  (faces and entities do survive, plane for plane; the group count is the failing line).
- **Likely owner:** editor core / P5 (`tools/editor.rs` `export_map`, b8944a41; `tools/mapsource.rs`, 8afc8d30).

### TB-4. TrenchBroom's worldspawn keys go on to the export and the compiler
- **Repro:** import `tb_valve_layers.map`, Export: the world keeps `"_tb_def" "builtin:HalfLife.fgd"`; the
  compiled BSP's entity lump has it too. Harmless to the game, but `_tb_*` keys are stripped from every
  other entity on import (`mapfile.rs` filters them for entities, not for worldspawn).
- **Spec:** `editor-coverage-trenchbroom` › "TrenchBroom's worldspawn keys (_tb_def)…". Low.
- **Likely owner:** editor core (agent E), `mapfile.rs` (418c052b).

(F-1 has two failing tests and one inventory row; TB-1..TB-4 one test and one row each.)

### Fixed (Parity A, 2026-11-14)
- **F-1:** a sheet now closes on Esc wherever focus is (a window listener for the top sheet, `ui.tsx`), and the
  page behind never gets that Esc; the editor's own key handler ignores keys while any sheet is open
  (`sheetOpen()`, `MapEditor.tsx`). Both `editor-coverage-dialogs` Esc tests pass. (The `ui.tsx` part went in with P5's
  1aeec7f1, which committed that file whole; the rest in e14e23b9.) Rerun: dialogs, panels and keys in Chromium and
  WebKit 245 passed (the panels "tools in the views" test failed once and passed on its rerun), TrenchBroom 9/9,
  editor-kits and editor-bar 14/14 in both engines.
- **TB-1:** a layer's `_tb_layer_hidden` / `_tb_layer_locked` come in as the group's hidden / locked.
- **TB-2:** `_tb_layer_omit_from_export` is kept (`Group.omit_export`); File › Export, Share and remix's source
  .map, prefabs and compiles leave that layer and what's in it out (a copied selection keeps it).
- **TB-3:** File › Export, the remix source .map and prefabs write groups and layers as TrenchBroom's
  `func_group` / `_tb_*` entities (layers with their flags, nesting kept; `WriteOptions::export()`).
  Compiles keep writing no editor groups (`WriteOptions::compile()`), so the compilers never see them.
- **TB-4:** `_tb_*` world keys (`_tb_def`) are dropped on import and never written.
- Tests: Rust `mapfile::trenchbroom_layers_round_trip` (fixture → import → export → re-import keeps groups,
  layers, flags and nesting; compile text has no groups, `_tb_` keys or the omitted layer); the TrenchBroom
  coverage spec passes (two of its lines updated to the agreed behavior: 5 groups after the trip, since the
  omitted layer isn't exported; and the export's world has no `_tb_` keys, its groups do).
- **Wording:** File › Import a .map… has a hint naming TrenchBroom, Hammer and J.A.C.K.; the file dialogs'
  filter reads "Map source (.map: TrenchBroom, Hammer, J.A.C.K.)"; Export's hint says groups and layers go
  back to TrenchBroom; docs `#import-comes-over` says what's kept.
- **UX notes:** Enter commits a light's Brightness; Map settings counts entities without the world (as the
  status bar does); the dock's ⋯ menu items are `menuitem`s.

---

## UX notes (not FAILs)

- **TrenchBroom wording (asked for by the user).** Now naming TrenchBroom: the start screen's **Import a
  .map** tooltip, the import file dialog's title ("Import a .map file (TrenchBroom, Hammer, J.A.C.K.)",
  5e5202a3), site/docs.html (feature list, `#map-editor`, `#map-import`, `#share-remix`, the WAD tip).
  Still not naming it:
  - File menu / palette: `Import a .map…` has **no hint** (`commands.ts` `file.import`), while the Hammer
    item has one ("Brushes, entities, groups and visgroups come over"). Suggest: hint "From TrenchBroom,
    Hammer, J.A.C.K. or any GoldSrc editor; TrenchBroom's groups and layers come in as groups".
  - File dialog filter label: `"GoldSrc map source"` (editor.rs `Pick::Import` / `Pick::ExportMap`).
    Suggest "Map source (.map: TrenchBroom, Hammer, J.A.C.K.)".
  - `Export a .map…` hint `"For any GoldSrc compiler"`: true, but say groups/layers aren't written (TB-3)
    until they are.
  - site/docs.html `#import-comes-over`: "TrenchBroom's groups and layers come in as editor groups" — add
    that a layer's hidden/locked/omit-from-export flags and linked groups aren't kept (TB-1, TB-2; linked
    groups come in as separate groups, each copy its own: fine, but say so).
- **Number fields in Inspect save on blur, not Enter** (e.g. a light's Brightness): text fields like Name
  save on Enter. Low.
- **Entity counts disagree:** Inspect's Map settings says "N+1 entities" (counts worldspawn, `checks.rs`
  stats), the status bar and Map information say N. Low.
- **Dock's ⋯ menu** is `role="menu"` but its items are plain buttons (no `menuitem`); screen readers announce
  an empty menu. Low.
- **Mac app, native menu keys (needs a check in the real app):** the native menu has Hide (⌘H), Minimize
  (⌘M), Close Window (⌘W); the editor uses ⌘H (Hollow, Loner-Strike and TrenchBroom-style hide), ⌘M
  (Transform). In WKWebView the page normally gets the key first and these are `preventDefault`ed, so it
  should be fine, but it can't be checked from a browser.
- **Test harness:** `app_api::settings_path()` ignores `LS_APP_DIR` (unlike `sys::app_dir()`), so a test
  server reads the user's real `settings.json` (read only here) and can't be told about a sim copy's game
  folder; that's why the real compile route can't run against a sim copy (below). Suggest honoring
  `LS_APP_DIR` there in test builds. Also: the dev file dialog decides project/.map/.lin/.fgd by the
  `LS_DEV_PICK` path's extension, hence the three server runs.

---

## Compile, playtest and the sim

- The dev server's compile route needs the game as a Steam/Settings install (`servers::client`), so against a
  sim copy it answers "Set up the game on this computer first" (and `guard_real_game` refuses installs into
  the real game under test). So in the browser the compiler's routes are answered by the spec and what the
  page **sends** is checked: preset, name, all cores, install for a playtest, the .res, the bots' learning, the
  launch with the bots count, and the leak follow-up. Marked PASS (wiring) below, with the real compile
  done from the shell:
- **Real compile of the editor's exports** (SDHLT 1.3.1 x64 through CrossOver, `-low -threads 4`, `nice 15`,
  VIS `-fast`): the four TrenchBroom fixtures imported and exported by the editor (`tb_valve_layers`,
  `tb_standard`, `tb_standard_q2`, `tb_valve_q2extras`) all compile clean: no leak, no errors, one direct light
  each, 13-25 kB BSPs. The Notes layer's AAATRIGGER box is in `cov_valve_layers.bsp` (TB-2). (A first try
  showed my fixture used `light` instead of `_light`; fixed in `make_fixtures.py`.)
- **Sim:** one HLDS sim from the sim copy (`-insecure`, port 27465, load under 18 when it started, the only
  sim of mine): `cov_valve_layers.bsp` (the editor's export of the TrenchBroom Valve 220 fixture) loads
  ("Mapchange to cov_valve_layers", ReGameDLL and our plugins start on it, bots join, no errors or crashes
  beyond the expected "no .nav" note). Stopped by its PIDs and the sim copy deleted. This stands in for the
  game end of Playtest; the launch itself (the real client) wasn't started.

---

## Inventory

Status: PASS, FAIL, NA = NOT AUTOMATABLE (reason given). Spec names are short: `cmd` =
editor-coverage-commands, `keys`, `dlg` = dialogs, `pan` = panels, `file`, `map`, `tb` = trenchbroom.
Every key in the key sets is listed with its command in "Keys" below; a command's row lists its Loner-Strike key.

### File
| Item | Entry point | Status | Spec |
|---|---|---|---|
| New map | File › New map, ⌘⌥N (Hammer ⌘N), start screen | PASS | cmd, keys, file › start screen |
| Asks before replacing unsaved work | any New/Open/Import/Recent with changes | PASS | cmd (file.new), file › Open a project |
| Open a project… | File, ⌘O, start screen | PASS (on the `.lsmap` server) | file › Open a project |
| Recent files | File menu list, start screen list | PASS | file › Save, Save as… |
| Import a .map… | File, start screen | PASS | file, tb |
| Import a Hammer .rmf or J.A.C.K. .jmf… | File, start screen | PASS (.rmf; .jmf is by content too, covered by Rust tests) | file |
| A file that isn't a map / isn't an rmf | the same | PASS (said in words) | file |
| Save | File, ⌘S, bar's Save | PASS | file |
| Save as… | File, ⌘⇧S | PASS | file |
| Export a .map… | File, ⌘E | PASS (Valve 220; read back equal) | file, tb |
| Project files | File › Project files | PASS | cmd |
| Map properties… | File, Map | PASS | cmd, dlg |
| Autosave recovery: Recover it / Throw it away | start screen banner | PASS | file |
| Back to the open map | start screen banner | PASS | file |
| Project panel: Save as a project…, Look again, Rename, Delete | Project panel | PASS | file |
| Project panel: Open, Show | Project panel | PASS (the request is checked; the Finder window itself isn't opened, on purpose) | file |
| Project panel: Find missing files…, Copy into the game / the project | Project panel | PASS via `editor-assets.spec.ts` (existing, run here) | – |

### Edit
| Item | Entry point | Status | Spec |
|---|---|---|---|
| Undo / Redo | Edit, ⌘Z, ⌘⇧Z / ⌘Y | PASS | cmd, keys |
| Undo history (panel; click a step, redo by click) | Edit › Undo history | PASS | cmd, pan › History |
| Cut / Copy / Paste | Edit, ⌘X/⌘C/⌘V | PASS (Chromium; WebKit skips clipboard keys) | cmd, keys |
| Paste special… (Row, Stairs, Ring, Spiral, copies, Cancel) | Edit, ⌘⇧V, Inspect | PASS | cmd, dlg |
| Duplicate | Edit, ⌘D, Inspect, Outline menu | PASS (new names, one grid step over) | cmd, keys, pan |
| Duplicate in place | Edit, ⌘⇧D | PASS | cmd, keys |
| Duplicate (keep names) | palette | PASS | cmd |
| Apply the current texture | Edit, ⇧T | PASS | cmd, keys |
| Delete | Edit, Delete/Backspace, Inspect | PASS | cmd, keys, pan |
| Find and replace… (textures, entity types, values, key names, whole name, case, only picked, Pick them) | Edit, ⌘F (Hammer ⌘⇧F) | PASS | cmd, dlg |
| Transform… (Move, Turn, Stretch, Line up, Spread out) | Edit, ⌘M | PASS (Esc after Apply: FAIL F-1) | cmd, dlg |
| Line up and spread out… | Edit (2+ picked) | PASS | dlg |
| Replace with entity or prefab… (entity, prefab) | Edit | PASS | cmd, dlg |
| Group / Ungroup | Edit, ⌘G / ⌘⇧G (Hammer ⌘U), Inspect, Outline | PASS | cmd, keys, pan |
| Tie to entity (func_wall; type picker in Inspect) | Edit, ⌘T, Inspect | PASS | cmd, keys, pan |
| Move to world | Edit, Inspect (Hammer ⌘⇧W) | PASS | cmd, keys, pan |
| Hollow (wall thickness) | Edit, ⌘H, Inspect | PASS | cmd, keys, pan |
| Carve | Edit, Inspect (Hammer ⌘⇧C) | PASS | cmd, keys |
| Merge | Edit, Inspect | PASS | cmd, pan |
| Snap to grid | Edit, ⌘B, Inspect | PASS | cmd, keys, pan |
| Flip along X / Y / Z | Edit, Inspect (Hammer ⌘L/⌘I, TB ⌘F/⌘⌥F) | PASS ×3 | cmd, keys, pan |
| Turn 90° around X / Y / Z | Edit, Inspect | PASS ×3 | cmd, pan |
| Line up on X/Y/Z: low/middle/high (9) | palette, Transform › Line up | PASS ×9 | cmd, dlg |
| Spread out evenly along X/Y/Z (3) | palette, Transform › Line up | PASS ×3 | cmd, dlg |

### Select
| Item | Entry point | Status | Spec |
|---|---|---|---|
| Select all / none / invert | Select, ⌘A / ⌘⇧A / ⌘I | PASS | cmd, keys |
| Select same texture / type / name | Select | PASS | cmd |
| Select touching / inside | Select | PASS | cmd |
| Go to brush or entity number… (editor and compiler numbers) | Select, ⌘J (Hammer ⌘⇧G) | PASS | cmd, dlg |
| Hide / Hide everything else / Show everything | Select, H / ⇧H / U (TB ⌘H/⌘I/⌘⇧H), Inspect, Outline | PASS | cmd, keys, pan |
| Hide all point entities / triggers and zones / world brushes | Select | PASS | cmd |
| Lock / Unlock everything | Select, L / ⇧L, Outline lock | PASS | cmd, keys, pan |
| Clicks pick whole groups / single objects / brushes | Select menu, tool options | PASS | cmd, pan |
| Click picks, box picks (Auto, Fully inside, Touching), Only what you can see, Box anywhere in 3D | views, tool options | PASS (settings); box drag: `editor-flat.spec.ts` (existing) | pan |

### View
| Item | Entry point | Status | Spec |
|---|---|---|---|
| Layout: Single / Quad / 1+3 / Two | View, ⌥1-⌥4, bar buttons | PASS ×4 | cmd, keys |
| Maximize the pane / put it back | View, ⇧Space (Hammer ⇧Z, TB ⌘Space) | PASS | cmd, keys |
| 3D / Top / Front / Side view | View, ⇧1-⇧4, bar | PASS ×4 | cmd, keys |
| Zoom to selection / whole map | View, Z / ⇧Z (Hammer ⌘⇧E, TB ⌘U) | PASS | cmd, keys |
| Fly / orbit camera | View, F | PASS | cmd, keys |
| Smaller / bigger grid | View, [ / ], Grid picker | PASS | cmd, keys, pan |
| Show / hide the grid | View, ⇧G, Grid dialog | PASS | cmd, keys, dlg |
| Grid and snapping… (size, show, turn snap, 64-unit lines, snap, corner snap) | View | PASS | cmd, dlg |
| Texture lock on/off, Scale lock | View (Hammer ⇧L), tool options | PASS | cmd, keys, pan |
| Show frame time | View, ⌘⌥⇧F | PASS | cmd, keys |
| Snap to grid on/off; Alt held | View, ⇧W, Snap button, Alt | PASS | cmd, keys, pan |
| Snap to corners and edges | View, Grid dialog | PASS | cmd, dlg |
| Move like Unreal / Blender / Hammer | View, first-open banner | PASS ×3 (banner: `editor-nav.spec.ts`) | cmd |
| Camera faster / slower | palette (right mouse + scroll in the view) | PASS ×2 | cmd |
| Models in place of entity boxes | View | PASS | cmd |
| Next drawing style for the pane; pane style picker | View, pane picker | PASS | cmd, pan |
| Panel: Content, Inspect, Outline, Textures, Gameplay, Budget, I/O, Checks, Compile, Project, History | View | PASS ×11 | cmd |
| Panel layout: Default, Hammer, Unreal-like, All on the right; Reset | View | PASS ×5 | cmd |
| Content Browser / Content drawer | View, ⌘⇧Space (Mac) | PASS | cmd, keys, pan |
| Budget / I/O | View | PASS | cmd |

### Tools
| Item | Entry point | Status | Spec |
|---|---|---|---|
| Select, Move, Rotate, Scale, Vertex, Box, Shape, Clip, Entity, Texture, Measure | Tools menu, rail, Q W E R V B C X N T M (Hammer ⇧S ⇧V ⇧B ⇧X ⇧E ⇧A; TB R T C V B) | PASS ×11 | cmd, keys, pan |
| Box: drag draws; Height | Box tool | PASS | pan |
| Shape: 7 kinds, sides, Preview… | Shape tool | PASS (drawing each: `editor-shapes.spec.ts`) | pan |
| Clip: two points, Tab, Enter, Keep both/behind/front, Cut | Clip tool | PASS | keys, pan |
| Entity: class to place, click places | Entity tool | PASS | pan |
| Texture: click picks a face, Click paints, arrows slide | Texture tool | PASS | pan, keys |
| Measure: drag measures | Measure tool | PASS | pan |
| Vertex: arrows move corners, Esc, Merge corners | Vertex tool | PASS (split/merge drag: `editor-extrude.spec.ts`) | keys, pan |
| Move/Rotate/Scale gizmo drags, flat-view handles, push/pull a face, Shift-drag extrude | views | PASS via existing `editor-flat`, `editor-push`, `editor-extrude` specs | – |
| Arrows / Page Up / Page Down nudge (Undo) | keys | PASS | keys |
| Camera marks: Ctrl+1-9 store, 1-9 go | keys | PASS | keys |
| Esc: drops the corners, then the selection | key | PASS | keys |
| Keys typed into a field don't run commands | any field | PASS | keys |

### Map
| Item | Entry point | Status | Spec |
|---|---|---|---|
| Map properties… (title, sky, draw distance, base light, sun color/brightness/pitch/angle, buying, bomb radius, WAD remove/add, Save) | Map, File, Inspect | PASS | dlg |
| Entity definitions: Add your own FGD… / Use ours only | Map properties | PASS | dlg |
| Entity report… (find, class, tick all, Pick, Delete, double-click) | Map, Inspect | PASS | cmd, dlg |
| Map information… | Map | PASS | cmd |
| Check the map (Checks panel) | Map | PASS | cmd, pan |
| Publish… (budget ask, then Community › Publish) | Map, bar | PASS | map |
| Playtest (compile, install, .res, teach bots, launch) | Map, F5, bar, Compile panel | PASS (wiring, compiler answered by the spec) | map |
| Playtest for real in the game | – | NA: starts the user's real client; the compiled map was loaded in a sim instead (above) | – |
| Go to leak / Show or hide the leak line / Load pointfile… | Map, leak bar | PASS (pointfile on the `.lin` server; leak from a compile: answered) | map |
| Compile and play… (Compile panel) | Map, F9 (Hammer) | PASS | cmd, keys |
| Outline / Textures / Properties of what's picked | Map, ⌥Enter | PASS | cmd, keys |

### Help
| Item | Entry point | Status | Spec |
|---|---|---|---|
| Find a command… (search, arrows, Enter, greyed-out items) | Help, ⌘K, ⌘⇧P, bar | PASS | cmd, keys, dlg |
| Keys cheat sheet (Change keys…, Print) | Help, ? | PASS | cmd, keys, dlg |
| Coordinates and axes | Help, axes triad | PASS | cmd |
| First map tour | Help | PASS (steps: `editor-bar.spec.ts`) | cmd |
| Change keys… (key sets, find, Add key, Remove, clash warning, reset, Put back; Hammer switches to Quad) | Help | PASS (Esc after Remove: FAIL F-1) | cmd, dlg |

### Dialogs
| Item | Status | Spec |
|---|---|---|
| Escape closes each of the 13 dialogs with focus inside, selection kept | PASS | dlg |
| Escape after Apply (Transform) or after removing a key (Change keys) | **FAIL F-1** | dlg |
| Paste special, Find and replace, Transform, Go to, Grid, Replace with, Map properties, FGD, Entity report, Find a command, keys sheet, Change keys: each control | PASS (rows above) | dlg |

### Keys (every binding)
| Item | Status | Spec |
|---|---|---|
| Loner-Strike set: 61 key presses (every key of every command; the file keys and F5 are in file / map) | PASS | keys, file, map |
| Hammer-style set: 18 keys that differ | PASS | keys |
| TrenchBroom-style set: 10 keys that differ | PASS | keys |
| Navigation preset keys (Unreal F = zoom) | PASS via `editor-nav.spec.ts` (existing) | – |
| Mac native menu key clashes (⌘H, ⌘M) in the real app | NA: needs the app's WKWebView window | – |

### Panels
| Item | Status | Spec |
|---|---|---|
| Inspect, brush: location, turn, size, scale fields; textures list; wall thickness; Hollow, Merge, Carve, Snap, Duplicate, Paste special, Flip, Turn, Tie (type), Group, Look at, Isolate, Hide, Delete | PASS | pan |
| Inspect, entity: name, color, brightness, flicker, falloff, flags, raw keys, new key, reset, Sets off / Set off by, Move to world, Look at, Duplicate, Delete | PASS (number fields save on blur: UX note) | pan |
| Inspect, several entities: add to all, remove from all | PASS | pan |
| Inspect, nothing picked: sky, WADs, Map properties…, Entity report… | PASS | pan |
| Inspect, faces (face sheet: shift, scale, turn, fit, justify, align world/face/view, pick up, paint) | PASS via `editor-facesheet.spec.ts` (existing) | – |
| Outline: find, show only, sort, hide/lock per row, double-click, right-click (Duplicate, New folder, Ungroup) | PASS | pan |
| Visgroups (new, rename, show/hide, remove) | PASS via `editor-ux.spec.ts` (existing) | – |
| Textures: find, one WAD, click, double-click paints | PASS | pan |
| Checks: filters, Show in 3D, settings, Pick, Look, fix + Undo | PASS | pan |
| Budget: Refresh, rows, Pick | PASS | pan |
| I/O: All, Broken, Picked chain, find, pick, look, Remove | PASS | pan |
| Gameplay: Components / Mode kits, search, Place, component file save/load | PASS (kits' placing: `editor-kits.spec.ts`) | pan |
| Content: 6 sections, search, size, sources, favorites, recent, right-click, Refresh, drawer | PASS (drag onto the map: `editor-content.spec.ts`, `editor-batch.spec.ts`) | pan |
| Content › GameBanana browse/download | NA: needs GameBanana over the network | – |
| Prefabs: save what's picked, search, Place, rename and tags, delete | PASS | pan |
| Prefabs: Share as a package…, Add a prefab from a file | NA here (save dialog to a .zip / pick a .zip: one more server run each); keep both/replace: `editor-push.spec.ts` | – |
| Compile panel: quality, name (bad names refused), all cores, Compile, output, Download SDHLT, Stop | PASS (answered compiler) | map |
| Cordon: box round picked, add, from view, remove, remove all | PASS | map |
| Dock: tabs, ⋯ Float, Dock back, Dock: Left, Close, Open in its own window, fold/open bottom | PASS | pan |
| Dock: drag a tab to another area, resize separators | NA here: covered by `npm run test:dock` (scripts/test-dock.mjs) | – |
| Status bar: picked count and size, counts | PASS | pan |
| MCP selection: page reports; an agent's selection shows | PASS | pan |

### TrenchBroom (.map)
| Item | Status | Spec |
|---|---|---|
| Valve 220 with layers, nested groups, entities in layers and groups, brush entity in a group | PASS | tb |
| Linked groups (`_tb_linked_group_id`) | PASS (come in as separate groups) | tb |
| Hidden / locked layer | **FAIL TB-1** | tb |
| Omit-from-export layer | **FAIL TB-2** | tb |
| Export: every face (plane and texture) and entity | PASS | tb |
| Export keeps groups and layers | **FAIL TB-3** | tb |
| `_tb_def` on worldspawn not passed on | **FAIL TB-4** | tb |
| Quake standard format; Quake 2 extras (standard and Valve) | PASS ×3 | tb |
| Clean compile of each export (SDHLT) | PASS | manual, above |

### WebKit
| Item | Status |
|---|---|
| keys (every key of the three sets, old-handler keys, fields) | PASS: 95 passed, 3 skipped (Cut/Copy/Paste keys: WebKit's clipboard needs a user gesture; the same cases pass in Chromium) |
| panels (Inspect, Outline, Textures, History, Checks, Budget, I/O, Gameplay, Content, Prefabs, dock, tool rail, tools in the views, status bar, MCP) | PASS: 18/18 |
| Clip's Tab/Enter in WebKit | PASS once the spec places the cut from the Top view's handles (my first version clicked fixed screen spots, which missed the box in WebKit: a test problem, not the editor's) |
