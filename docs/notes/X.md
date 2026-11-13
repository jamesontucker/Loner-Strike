# Agent X — Game Mode Studio

Earlier rounds are in docs/TOOLS-NOTES.md ("Agent X" sections). From here on, notes go here.

## 2026-11-12, late: loose ends, Playtest feedback, crisp zoom, snippets, Blueprint-style editor

**Saved values survive a crash.** Saved values no longer use nvault (it only wrote at map end; a killed
server lost everything). Each mode has its own file, `addons/amxmodx/data/studio/<mode>.saved`
(`key<TAB>value` lines), kept in memory and written on every change, through `<mode>.saved.new`
renamed over the old file (on Windows the old one goes first; the .new one is read if a crash lands
between). Checked: the sim harness kills HLDS at the end of a run — all 6 saves were in the file; a
second run loaded them and added to them.

**Buying rules and bots: no bypass.** Live on a sim copy with `mp_startmoney 16000`: with Allow Buying
"nothing" every bot still held its spawn pistol after the buy time; the same graph with "everything"
had bots holding AK-47, M4A1, FAMAS, SG 552 and M249. So the cstrike module's `CS_OnBuyAttempt` sees the
bots' purchases too; nothing extra needed. (The "nothing" choice is labelled so: the spawn pistol stays.)

**Playtest wasn't broken, just silent.** The button works. Before, nothing showed what was happening
while it compiled (amxxpc under CrossOver, a few seconds) and while the game started (Steam, CrossOver,
the map: up to a minute the first time). Now:
- the button becomes the current step with its seconds and a Cancel: Saving, Compiling (the first
  time under CrossOver takes longer), Checking the game, Starting / Restarting / Switching the open game;
- the Playtest panel counts "Waiting for the game… N s" with what's likely going on (starting the game
  and the map; after 20 s the first-start note; after 90 s "check the game's window");
- an unchanged graph isn't compiled again (hash of the source and includes; 0 s instead of seconds);
- an open game is switched to the mode with a map change (`POST /api/tools/studio/switch`: the mode's
  plugin list and current.txt as a start writes them, then `gm_map` over the console) instead of being
  closed and started again; if that fails it offers the restart;
- a Playwright test (`tests/studio-playtest.spec.ts`) checks the button is there and enabled on a clean
  graph and that pressing it shows the steps and the wait (compile and launch answered by stubs).

**Crisp zoom (WebKit on the Mac, Chromium/WebView2 on Windows).** After a pan or zoom settles, the
view's offset is put on whole device pixels (`devicePixelRatio`, so 1.25/1.5 too: no half-pixel blur,
no 1 px jitter between pans), and on WebKit only (by user agent) the viewport is drawn again so it
doesn't keep a lower-resolution bitmap of the zoomed layer. No filters on wires (a drop-shadow on lit
wires made WebKit rasterize them), nothing uses will-change. Radar maker's PanZoom: `will-change` removed
and its offset rounded the same way (two small edits in overview/). Callouts' TopDown already draws its
canvas at the device pixel ratio; the map editor draws with three.js. Checked with Playwright WebKit
and Chromium at device pixel ratios 1, 1.25, 1.5, 2 and zoom 0.5–2 after wheel zooming: text and
borders sharp in both. The real WKWebView compositor can't be run headless, so that last step is the
user's eyes.

**Comment boxes no longer catch the canvas gestures.** Only the title bar (move, with the nodes inside)
and the resize handles take the mouse; inside, the canvas pans and box-selects as anywhere (checked in
WebKit and Chromium: a drag inside a box pans the view).

**Snippets.** 14 built-in pieces in `app/ui/src/views/tools/studio/snippets/` (made by
`scripts/make-studio-snippets.py`, laid out by `npm run tidy:studio`): wave system, countdown and start,
shop menu, revive, zone that shrinks, supply drop every minute, loadout on spawn, last squad standing,
kill reward, creatures at markers, round timer win, team score to 10, teleport on touch, chat command.
Click (into free space under the graph) or drag one in: its nodes, wires and variables are added in a
comment box named after it; variables join by name, type and scope, or get a new name on a clash. Save
the selection as your own snippet (`/api/tools/studio/snippets`, `snippet/:id`, `snippet/:id/delete`;
kept in `<app>/tools/studio/snippets/`, checked like a graph: graph data only, same text rules).
Every built-in snippet checks clean as a mode and is compiled with amxxpc in the compile test.
Spawn Creature now also takes a wired entity (creatures at every marker).

**Blueprint-style editor (UI only, the graph JSON is unchanged).**
- Left "My mode" panel: Events, Functions, Variables (+ to add; drag a variable onto the canvas for
  Get/Set), Snippets (search, click or drag; save the selection; delete your own).
- Right "Details": the selected node, variable (name, type, one for, kept, start value, used by),
  comment (colours) or the mode (name, description, game settings).
- Bottom "Results": Problems (click to go to the node) and Compiler output; above the shared ActionBar.
- Toolbar: Undo, Redo, Tidy up, Find in this mode, map, bots, Playtest. Function crumbs: looking at a
  Function adds "Function: name" to the breadcrumbs.
- Nodes coloured by kind like Blueprints (events red, values green, functions purple, flow grey, actions
  blue), flow pins white pentagons (hollow until wired), data pins coloured circles by type, wires
  solid in their type's colour (flow white and thicker), a dark grid with major lines, minimap.

**Checks:** `cargo test studio` 33 passed (incl. the amxxpc compile of every test graph, both templates
and all snippets, no warnings); `npm run test:studio` 14 passed; `npm run build` green (Studio chunk
508.6 kB, start chunk 529.45 kB); the Playtest Playwright test passes.

**For S (MCP):** new Studio routes if you wrap them: `POST /api/tools/studio/switch`,
`GET /api/tools/studio/snippets`, `POST /api/tools/studio/snippet/:id`, `POST /api/tools/studio/snippet/:id/delete`.

## 2026-11-13: graph version 3 (Blueprint staples), My library, QA fixes, a light app touch

**Graph version 3** (versions 1 and 2 open as 3; nothing new is written for them). New optional
`enums`, `structs`, `functions`, `macros`; a node or comment with `"graph": "<id>"` is on that
Function's or Macro's own graph; library marks (`from: {lib, version}`) on those four.
- **Graph tabs**: Event Graph plus a tab per open Function or Macro (double-click it in My mode or
  double-click a Call/Macro node; × or middle-click closes; drag to reorder). The address is
  `studio:<mode>/<tab>`, so Back and the crumbs (`Function: name`) step out of a tab.
- **Functions** (own graph: Function Entry with the inputs, Return Node(s) with the outputs, local
  variables reset each run, pure or with flow; Call / Call Pure nodes; Set Timer by Function).
  No Delay/Repeat/Countdown/Menu inside a Function (they'd return early): use a Macro.
  **Macros** (any number of flow and data pins, Delays allowed) are copied in where used before
  the plugin is made (`expand.rs`); the debugger shows the Macro node for its copies.
  **Collapse to Function / Macro** (right-click with a selection).
- **Types**: arrays of every type (incl. structs), maps (text/number/player keys to plain types or
  enums), structs, enums, vectors, timer handles, for variables, pins and Function members.
  Generic nodes take the type of their first wire. In the plugin: arrays are cellarray handles
  (every index clamped, at most 10000 items), maps tries, vectors and structs slots in rings (256
  vectors, 64 structs; variables have their own slots), so — like text — they don't last through a
  Delay unless kept in a variable (checked).
- **Nodes** (220 now): Sequence, Do Once, Do N, Gate, Flip Flop, For Loop (+ with Break), While
  (bounded), For Each over an array (with Break), Switch on number/text/enum, Is Valid (pure and
  branch), Is Player, Is Creature, Equal, Select, Retriggerable Delay, Clear Timer, Is Timer Active,
  Print String, Math Function / Clamp / Lerp / In Range / Number, mod and power, Text / Append /
  Format Text (`[0]`..`[3]`) / Contains / Length / To Number / To Text, Make/Break Vector, Vector
  Math, Scale, Length, Distance, Direction, Normalize, Position Of Player (feet, eyes, aim, aim
  point, velocity), Position Of Entity, Set Position, Set Velocity, Make/Break Struct, Set Field,
  Enum Value, Enum To Number, Make Array, Players Array, Add, Insert, Remove Item, Remove Index,
  Set, Clear, Shuffle, Sort, Get, Length, Contains, Find, Random Item, Map Add/Remove/Clear/Find/
  Contains/Length/Keys/Values.
- **Watch values**: hover a pin for the value the game last saw there (an output: as the nodes it
  feeds saw it).
- **Security**: every new name (enum values, fields, members, Functions, Macros) goes through the
  same allow-list; Pawn identifiers are ours (`lsg_fr0`, `lsg_mks0`, `lsg_mka12`); test
  `hostile_v3_names_stay_data`.

**My library** (user request): right-click a Function, Macro, struct or enum in My mode → Save to
my library (name, description, category). The item holds the part and everything it needs (the
structs and enums it uses, Functions and Macros it calls, mode variables it reads or sets), kept in
`<app>/tools/studio/library/<id>.lib.json` (`/api/tools/studio/library`, `library/:id`,
`library/:id/delete`), checked like a graph (graph data only; never events). Saving changed content
again bumps its version. The My library panel lists them by category with search; double-click or
drag one in: the same enum/struct (same values or fields) and variables (same name, type, scope)
are used again, anything else comes in with fresh ids and is renamed on a clash. The copy remembers
`{lib, version}`; when the library has a newer version, its Details offer "Update from library"
(asks first; replaces the graph and pins in place, keeps the name; calls stay). Export as a
`.lib.json` file, Import from files. Community sharing can reuse the same file later.
Tested: `library_items_are_checked_and_versioned` (Rust), the library test in `npm run test:studio`
(dependencies, merge, rename, update in place) and live: a mode built from a library item compiled
with amxxpc and ran with 12 bots ("helper ran" ×12, "kits given: 12").

**QA fixes** (requests below):
1. Drop Weapon: a bot's weapon is now taken away instead of `drop` (which crashes this DLL for bots;
   people still drop). Live: 17 bot drops, no crash.
2. A mode with component nodes lists `gm_components.amxx` before its plugin in its plugins.ini
   (`gen::needs`).
3. Creature types a mode spawns are precached as the map loads (`plugin_precache` adds them to
   `ls_creature_precache`; "all" when waves of components bring theirs). Live: Last Stand Lite on
   ls_outpost precached 8 types and cleared wave 1 (it stalled before); its "laststand" pack still
   isn't on ls_outpost (the components are in the BSP) and the flow carries on.
5. A breakpoint inside a loop: while the world is paused nothing else runs; what would (the loop's
   other rounds, what comes after, other events) waits in order and runs after Resume, pausing
   again at each round's breakpoint. The page keeps "Paused" until `world_resumed` and trusts the
   game's `paused` flag. Live: a breakpoint in For Each Player with 12 bots paused 12 times, each
   Resume ran the next player, then "after loop".
6. A Resume sent with a breakpoint change isn't lost: the plugin moves the command file aside
   before reading it.
7. The debugger shows the value the node uses: in debug builds each input is worked out once into
   a local.
8 and 9: see "2026-11-13, later" below.
Known limits: a breakpoint inside a Function pauses inside it, but the call's outputs were already
read (they show the last values); breakpoints inside pure Functions are best avoided.

**A light Loner-Strike touch** (user request; no change to category, pin, wire or text colours).
Two options were tried; the restrained one (A) is in:
squarer VGUI corners (4 px) and the app's bevel on nodes, a 1 px highlight along the top edge of
title bars (it sits above the text, so title contrast is unchanged: white on the title colours is
5.5–6.5:1 as before; node body text 13.8:1), a bone outline for the selection (the app's yellow now
only means running/paused), an olive tint in the grid, the app's olive as the default comment
colour, bevelled tabs. Option B added the app's bold caps for titles and a dark hatch on title bars
(contrast only goes up) and was left out as too much. Pin and wire colours are untouched, so their
colour-blind distinguishability is unchanged.

| zoom | before | A (in) | B (not taken) |
|---|---|---|---|
| 100% | ![](x-img/skin-none-100.png) | ![](x-img/skin-a-100.png) | ![](x-img/skin-b-100.png) |
| 60% | ![](x-img/skin-none-60.png) | ![](x-img/skin-a-60.png) | ![](x-img/skin-b-60.png) |

**Live sim run of every node** (`every_node.graph.json`, debug build, 12 bots, 150 s): Maths for a
For Loop with Break "0 2.50 0 0 / 1 3.25 1 1 / 2 4 1.41 4" (broke at 3), struct "Loadout: 40 kit
Tank 0 0 0" (clamped field, enum name, vector field), Function Reward "Said: kill!" "Total: 5"
(local reset each call) ×24, maps "12 yes 12 1", pure Function "Double: 8", Macro "gave: 6",
arrays "5 items, has 7 yes at 0", vectors per player, Players Array, switch on enum, no errors
other than the map's missing pack and teleport target.

**Checks:** `cargo test` 695 passed (studio 41, incl. amxxpc compiles of every test graph release and
debug with no warnings); `npm run test:studio` 20 passed; `npm run build` green (Studio chunk
610.5 kB, start chunk 530.75 kB); the Playtest Playwright test passes (WebKit and Chromium).

**For S (MCP):** new routes `GET /api/tools/studio/library`, `POST /api/tools/studio/library/:id`,
`POST /api/tools/studio/library/:id/delete`.

## Mode map kit format (version 1, 2026-11-13)
A **mode map kit** says what a map needs for one game mode, so anyone can make a map for it in the map
editor. It's data only (JSON, no code). One file per mode: `<mode id>.kit.json`.

**Where kits live:** the app's kits folder, `<app dir>/kits/<mode id>.kit.json`. The Studio writes one
for each Studio mode (generated from the graph, saved next to it as `<graphs>/<id>.kit.json`) and copies it
into the kits folder when the mode is installed (Install / Playtest / a community mode). Hand-written
modes (Last Stand, Battle Royale) ship a hand-written kit in `plugins/game/kits/<id>.kit.json`
(setup copies them the same way). Community mode packages carry the kit beside the graph.

```json
{
  "schema": "loner-strike/mode-kit",
  "version": 1,
  "mode": { "id": "last_stand", "name": "Last Stand", "description": "Hold the base against waves" },
  "source": "studio",
  "markers": [
    {
      "id": "creature_spawn",
      "label": "Creature spawn",
      "description": "Where creatures come in. Put them out of sight of the base.",
      "type": "point",
      "required": { "min": 2, "max": 32 },
      "keys": [
        { "name": "wave", "type": "number", "default": 1, "help": "First wave that uses it", "min": 1, "max": 99 },
        { "name": "side", "type": "choice", "default": "north", "help": "Which gate", "choices": [["north", "North"], ["south", "South"]] }
      ],
      "draw": { "colour": "#c0392b", "icon": "skull", "shape": "diamond" }
    }
  ],
  "components": [ { "classname": "ls_wave_spawner", "min": 1, "max": 1, "note": "One wave spawner" } ],
  "creatures": [ "zombie", "brute" ],
  "map": { "size": "medium", "players": { "min": 2, "max": 16 } },
  "notes": "Free text for map makers (up to 600 letters)."
}
```

| field | rules |
|---|---|
| `schema`, `version` | `"loner-strike/mode-kit"`, `1`. A reader refuses other schemas and newer versions it doesn't know. |
| `mode` | `id`: 2–24 of `a-z 0-9 _`, starting with a letter (the modes.ini id). `name` ≤ 40, `description` ≤ 60 (plain text). |
| `source` | `"studio"` (generated from a graph: don't edit by hand) or `"hand"`. |
| `markers[]` | ≤ 32. `id`: 1–24 of `a-z 0-9 _`, unique in the kit. `label` ≤ 30, `description` ≤ 200. |
| `type` | `point` (one spot), `area` (a spot with a `size "x y z"` box or a `radius` round it), `path` (spots in order: 2 or more per path). |
| `required` | `min` 0–64 (0 = optional), `max` 1–64 and ≥ min, or `null` (no limit). For a path: how many paths. |
| `keys[]` | ≤ 8 per marker. `name`: 1–24 of `a-z 0-9 _`, not one of the reserved keys (`classname origin angles model targetname target mode kind order path size radius spawnflags`). `type`: `number` (with optional `min`/`max`), `text` (≤ 60), `bool` (0/1 in the map), `choice` (`choices`: [value, label] pairs, ≤ 16, values like names). `default` fits the type. `help` ≤ 120. |
| `draw` | how editors show it: `colour` `#rrggbb`; `icon` one of `flag skull star crate door person arrow circle bolt heart`; `shape` one of `diamond box sphere cylinder line` (defaults: point diamond, area box, path line). An area also has `size` `[x, y, z]` or `radius` (the default the editor puts on a new one). |
| `components[]` | `ls_*` classnames (gm_components' kinds) the map must have, `min`/`max` as above, `note` ≤ 120. Placed as the component entity itself or as an `ls_marker` with `kind` = the classname (W1). Components the mode makes itself at run time aren't listed. |
| `creatures[]` | creature type names the mode spawns (the map needs room/nav for them; nothing to place). |
| `map` | optional: `size` `small` (≤ 2048 across), `medium` (≤ 4096), `large`; `players` `{min, max}` 1–32. |
| `notes` | ≤ 600, plain text. |

**In the map** (W1's `ls_marker`, adopted): every marker is a point entity of class **`ls_marker`** with
- `kind` = the marker id (or an `ls_*` component classname: gm_components turns it into that component),
- `mode` = the mode id, or `any` / empty / left out (any mode with a marker of that id),
- an area: `size "x y z"` (a box round the origin) or `radius`; a path: one `ls_marker` per spot with
  `order` 1, 2, 3… (several paths of one marker: `path` 1, 2…); optional `targetname`, `angle`;
- the marker's own keys (missing → the kit's default).

**At run time** gm_components keeps the `ls_marker`s (not drawn, not solid) and answers
`ls_marker_find(const kind[], start = 0)` (next matching entity after `start`, 0 when none) and
`ls_marker_get(ent, const key[], out[], len)`. A Studio mode that uses markers loads gm_components (like
any component node) and its Find Marker / For Each Marker nodes use those natives; paths are sorted by
`path`, then `order` in the generated plugin.

**Differences from W1's proposal (for W1):**
1. The kit calls the marker's shape `type` (point / area / path); the map key `kind` is the marker id, as
   you have it.
2. Paths: please pass `path` and `order` through like other keys (the Studio sorts by them).
3. Please resolve markers when the map loads (by plugin_cfg), not only at round start, so a mode's
   Map Started event can find them; and have `ls_marker_find` skip markers whose `mode` is another mode's
   (match the running mode id, `any` or empty).
4. Areas: `size` and `radius` as you proposed (no brush entities), so the editor draws them from keys.
5. Please add `ls_marker_mode(ent, out[], len)` only if you need it; the Studio doesn't.

**Checks** (the editor's map check, the Studio's Playtest): each marker's count within `required`, each
component's count, path markers with 2+ points and no repeated `order`, keys of the right type.

## 2026-11-13, later: feedback round, QA 8 and 9
**Editor:** Function Entry/Return (and Macro Entry/Exit) show the Inputs/Outputs editor in Details.
Keys (never while typing): ⌘S, F5 Playtest, ⌘F find, ⌘W close tab, Ctrl+Tab / Ctrl+⇧Tab, ⌘D duplicate,
F frame, Home frame all, Q straighten, Alt-click a wire to break it; they're listed in the Tools "?" sheet
(`common/Shortcuts.tsx`, `studio` entry; the toolbar's ? button opens it). A snippet with events (or, in a
Function, a wait) goes onto the Event Graph with a toast. Library: Export all (schema
`loner-strike/graph-library-set`), Import takes one item or a whole set. An address with a tab
(`studio:<mode>/<tab>`) opens that tab.
**Leaving:** `store.ts` `confirmLeave()` (asks, forgets the left edits, waits for the sheet's history
step) is used by App.tsx's section nav and server list and by the Tools crumbs/Back; after Leave nothing
asks again or steps back. Going between one mode's Event Graph and its tabs (crumb, Back) doesn't ask and
keeps the edits (`shell.tsx` `sameEdits`). Tested in Chromium and WebKit (`tests/studio-v3.spec.ts`: the
mode crumb, the Studio crumb, the Tools crumb, the Back button, Alt+←, the browser's back; Cancel and
Leave). The app's own sections/server list can't be driven in tools-dev (no app shell there).
**QA 8:** Create Component's Trap and Extraction made nothing because gm_components calls them
`ls_trigger_trap` / `ls_extraction_zone` (the choice keys stay; the plugin gets the real names); a debug
build now says when gm_components makes nothing. Set Zone / Shrink Zone take a Centre entity input; with
no name they keep the zone's own centre (`ls_zone_center`) instead of failing on the default "spot1"
(new nodes default to no name). In Zone on a non-zone component: fixed on W1's side (ff51cea).
**QA 9:** Player Hurt gives the health really taken (pre-hook keeps the health before) and doesn't run for
blocked damage; Team Changed runs once per real change (UNASSIGNED skipped, repeats on spawn skipped);
Creatures Alive counts live `ls_creature` entities now instead of the brain's lagging cvar; Find Component
takes names with spaces (param kind `entname`); uninstall also removes the .sma, the work .amxx/.hash and
the debugger's .bp/.cmd/.events. Not reproduced: "an entity in a HUD node shows empty" (the generated
code prints `Text <entity number>`; needs a live look). test:studio was already green (20).
**Checks:** `cargo test studio` 41 passed (amxxpc compiles with no warnings), snapshots updated;
`npm run test:studio` 20; `npm run build` green; studio-v3 + Playtest specs pass in Chromium and WebKit.
Not run live on a sim this round.

## Requests from QA (2026-11-12, real tests on sims QA4/QA5; full report docs/notes/QA.md)
1. **[Critical] Drop Weapon crashes HLDS** (bots on de_dust2, both tries; "Illegal termination of worker thread
   'CFileWriterThread'"). A bare `engclient_cmd(bot,"drop")` with no Studio mode crashes the LSPLAY brain+creatures mp.dll
   too, so the root is in the DLL (told Y), but the node should avoid it (e.g. a different drop path for bots).
2. **[High] Studio modes never load gm_components**: the installed plugin list has only the Studio plugin, so every
   component node says "gm_components isn't running"; `needs` in nodes.json isn't used by the Rust side.
3. **[High] Creature Spawn fails without Last Stand loaded** ("model isn't precached": only gm_laststand sets
   `ls_creature_precache`). Precache the creature types a graph uses.
4. **[High] Last Stand Lite stalls forever**: kills the T bots, `load_pack laststand` fails on ls_outpost (its components
   are in the BSP), no creatures, wave 1 never clears. Royale Lite stalls as installed (bug 2); with gm_components it
   plays to a winner.
5. **[High] A breakpoint inside For Each leaves the server frozen and the page can't Resume**: other iterations keep
   running and each `node_enter` clears the page's paused state (`debugState.ts`); the page ignores `world_paused` /
   `vars.paused`. Same in WebKit and Chromium.
6. **[Medium] A Resume sent together with a breakpoint change can be lost** (the .cmd file read then deleted by path);
   world stayed frozen 190 s+.
7. **[Medium] The debugger can show a different value from the one used** (inputs evaluated twice in debug builds:
   showed "Condition: True", took False with math.random).
8. **[Medium] Zone Set / Zone Shrink ignore a wired zone** ("No entity has that name"); Create Component for a trap or an
   extraction returns entity 0 with no error; In Zone on a created zone throws AMXX run-time error 10 in
   `ls_zone_inside` (W1 native) and ends the flow.
9. **[Low] Player Hurt fires for blocked damage** with the original amount; ls_outpost's component names have spaces that
   Find Component refuses; Creature Count reads 0 right after a spawn; Team Changed fires 3× per join (first "none");
   an entity sent to a HUD node shows empty; uninstall leaves the .sma/.amxx/.hash/.bp/.cmd; `npm run test:studio` 2 of
   14 fail on the current tree (`timer.delay.handle` registry check, template layout).
Passed for real: every player node, readouts, damage scale/set/block, down/revive, squads, buying rules incl. bots,
saved values across kill -9, round clock, variables/maths/flow/timers/functions, events, chat command, compile/install/
uninstall routes, server-side breakpoint pause/resume, the Playtest button test (both engines).

## Requests from W1 (2026-11-13): the `ls_marker` entity for Mode map kits
I'll follow your kit format when it's here. My proposal for the marker, to agree or change:
- One point entity **`ls_marker`**: `kind` (what it marks), `mode` (`laststand`, `br`, ... or `any`, the default),
  `targetname`, optional `size "x y z"` (a box round the origin), `radius`, `angle`, and any other keys passed through.
- gm_components resolves markers at round start for whatever mode runs: a marker whose `kind` is a component classname
  (`ls_wave_spawner`, `ls_barricade`, `ls_supply_drop`, `ls_loot_container`, `ls_respawn_point`, `ls_safe_zone`,
  `ls_zone_shrinker`, ...) becomes that component with the rest of its keys (only when its `mode` matches). Any other
  kind is a named spot for the mode's plugin: natives `ls_marker_find(const kind[], start = 0)` and
  `ls_marker_get(ent, const key[], out[], len)`.
- Last Stand kit: `lst_shop` (point; the armory, Barney stands there), `lst_base` (box), `ls_wave_spawner`,
  `ls_barricade` (+ `boards=<name>` of the func_illusionary boards), `ls_supply_drop`.
- Battle Royale kit: `ls_respawn_point`, `ls_safe_zone` (`radius`), `ls_zone_shrinker` (`stages`),
  `ls_loot_container` (`tier`), `ls_supply_drop`, `br_drop` (optional preferred landing spots).
- Maps made the current way (ls_outpost, br_valley: named entities and components) keep working.

## From W1 (2026-11-13): ls_marker natives are in (d857c24)
- `ls_marker_find(const kind[], start = 0)` ("" = any kind) and `bool:ls_marker_get(ent, const key[], out[], len)` in
  ls_components.inc, answered by gm_components. Markers are resolved while the map loads (BSP) and from packs at
  precache, so they're there by plugin_cfg / Map Started. Markers of another mode are left out entirely (`mode`: id,
  comma list, `any`, empty), so `ls_marker_find` only sees this mode's. All keys pass through (`path`, `order`, ...).
- A marker whose `kind` is an `ls_*` component classname becomes that component (find it with `ls_comp_find`, not
  `ls_marker_find`); `kind=lst_base` becomes Last Stand's base box.
- Kits: `plugins/game/kits/laststand.kit.json`, `br.kit.json` (mode ids `laststand`, `br`); setup copies them to
  `<app dir>/kits/`.
