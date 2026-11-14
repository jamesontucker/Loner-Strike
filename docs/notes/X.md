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
  "mode": { "uid": "7309d9f8-7f61-4c15-ad16-4f5260cb9787", "id": "laststand", "name": "Last Stand", "description": "Hold the base against waves" },
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
| `mode` | `id`: the mode's real id, as modes.ini, the Play page and Playtest use it (`laststand`, `br`; a Studio mode: its graph's mode id); 2–24 of `a-z 0-9 _`, starting with a letter. `name` ≤ 40, `description` ≤ 60 (plain text). |
| `mode.uid` | required: the mode's permanent identity, a UUID v4 (lower case). Made once when the mode is made; a rename or a new short id never changes it. Match modes by uid; show and file by `id`. |
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

**Mode identity (uid), 2026-11-13.** Every mode has a permanent `uid` (UUID v4) beside its short `id`
(the game id: modes.ini, the cfg and plugin file names, `ls_marker`'s `mode`, Playtest).
- Built-in modes' fixed uids: **Last Stand (`laststand`) `7309d9f8-7f61-4c15-ad16-4f5260cb9787`**,
  **Battle Royale (`br`) `b972ea10-43af-42cb-81f0-79e493a87a11`**. Put them in their kits (W1).
- Studio graphs: `mode.uid` is made when the mode is made (new or from a template: a template's copy
  gets a new one); a graph from before uids gets one the first time it's opened (saved back). A rename
  keeps it.
- Installing a Studio mode: if its uid is installed already, it goes there (whatever short id that is);
  else under its own id, unless another mode has that id (one not made in the Studio, or another uid):
  then the first free `<id>_2`, `<id>_3`… The compile/Playtest answer says `installedAs`. The uid is
  written in `<id>.plugins.ini`'s first line (`; Made with the Game Mode Studio (uid …): …`); a Studio
  install from before uids counts as the same mode. The kit goes in the kits folder under the installed id.
- Library items and snippets carry `fromMode` (the uid of the mode they came from). A remix gets a new
  uid and `remixOf` = the original's uid (T).

**In the map** (W1's `ls_marker`, adopted): every marker is a point entity of class **`ls_marker`** with
- `kind` = the marker id (or an `ls_*` component classname: gm_components turns it into that component),
- `mode` = the mode id (the modes.ini id: `laststand`, `br`, a Studio mode's id), a comma list of them, or `any` / empty / left out (any mode with a marker of that id),
- an area: `size "x y z"` (a box round the origin) or `radius`; a path: one `ls_marker` per spot with
  `order` 1, 2, 3… (several paths of one marker: `path` 1, 2…); optional `targetname`, `angle`;
- the marker's own keys (missing → the kit's default).

**At run time** gm_components keeps the `ls_marker`s (not drawn, not solid) and answers
`ls_marker_find(const kind[], start = 0)` (next matching entity after `start`, 0 when none) and
`ls_marker_get(ent, const key[], out[], len)`. A Studio mode reads its markers itself (FM_KeyValue and
FM_Spawn in its plugin's plugin_precache, the same rules: `kind`, `mode` incl. comma lists and `any`,
`path`/`order`), so it needs no gm_components and gives the same answers with it loaded.

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

## 2026-11-13: the mode-building pieces that exist today, as nodes
From the audit's step 1 (natives that already exist):
- **Bots** (all through `ls_comp_bots_hint`, so gm_components loads with them): **Bots: Phase**
  (loot / fight / normal), **Don't Buy**, **Friends** (two groups: all, CT, T, a wired player, or a tag),
  **Tag** (`role`), **Prefer Layer** (below / surface / high / own choice), **Fight Creatures**
  (`enemyclass ls_creature`), **Goal** (defend / capture / use / loot / go and hold, at an entity, priority,
  seconds, radius) and **Clear Goals**, **Offer** (perk / bench / box / trap / turret spot, price, order)
  and **Clear Offer**. "Who" is all, CT, T, the wired bot, or `@tag`.
- **Barricades:** Barricade Health, Set Barricade Health, Repair Barricade (fully, or one step as a
  player), events Barricade Damaged and Barricade Repaired.
- **Loot:** Roll Loot (a loot.ini table, a tier) → Item, Rolled.
- **Zone:** Zone Stage, Zone Next (seconds, next radius, closing), Next Zone Stage Now.
- **Supply Drop Landed** event.
- **Spawn Creature** has Health (0: the type's own), Tier (0-3) and Pay (money to its killer, paid when it
  dies); a **Creature Health** readout.
- Tests: the nodes `every_node.graph.json` has no room for (400 is the most a graph holds) are in
  `every_node_2.graph.json` (`make_every_node_2.py`); the coverage test reads both; both builds snapshotted
  and compiled with amxxpc, no warnings.
- **Sim (a copy of LSPLAY on ls_outpost with a safe zone, a shrinker and spot1 added):** barricade set to
  50, repaired to 800 (Barricade Repaired 800); Roll Loot "he"/"vest"/"hp:50"; Zone Stage 0, then 1
  two seconds after Next Zone Stage Now; Spawn Creature with health 300 (Creature Health 300), its
  killer's money 800 → 950 a second after the kill (pay 150); Supply Drop Landed. Bot hints: on the newest
  brain build (r43537330251) every hint was taken (no "ls_mode:" usage line); LSPLAY's older brain lacks
  `offer`. Not seen live: Barricade Damaged (no creature reached the gate in 2 minutes), the tier's look.
- **Templates:** these alone don't make Last Stand Lite or Royale Lite play like ours; they move onto
  the new pieces with W1's (round table, shop, pickups...), and then the side-by-side check on a sim.

## Audit (2026-11-13): can a Studio user rebuild Last Stand and Battle Royale?
Short answer today: **the skeleton yes, the game no.** A Studio mode can run waves of creatures from map
spawners, a shrinking zone, supply drops, squads, revives, buying rules, a HUD and menus, but most of
what makes our two modes feel finished lives in their own Pawn code, not in pieces the Studio has. So
the site's "built from the same pieces Game Mode Studio gives you" isn't true yet. Below: each feature,
where it lives (a = the mode's own code, b = a gm_components native/forward, c = a bot-brain `ls_mode`
hint, d = not built), whether a Studio user can do it **today**, and the piece that closes the gap.
"Partly" means it can be imitated with general nodes but not to the same standard.

### Last Stand
| Feature | Lives in | Studio today | Gap → piece (owner) |
|---|---|---|---|
| Waves from spawners, wave start/stop, wave events | b (`ls_wave_*`, `ls_on_wave_*`) | **Yes** (Start/Stop Wave, Wave Started/Spawned/Cleared) | — |
| Round table (types, counts, health ramp per round, waves.ini) | a (`wave_row`, `round_hp`, `load_waves`) | Partly: Creature Spawn takes a type and a place only; counts and ramps by hand with loops | Creature Spawn with **health, pay, tier** inputs; a **Round Table** component or `ls_wave_*` taking a table (W1 native, X node) |
| Special rounds (hound pack, snark swarm) and boss rounds (garg 10, gonarch 15) | a | Partly (by hand: branch on the round number, spawn a type) | covered by the table above + a "boss" flag on spawn (health bar) |
| Creature roster / types / fallbacks | a + Y's DLL | Partly: any type by name; no fallback when the server lacks it | `ls_creature_has(type)` native → "Creature Type Exists" node (Y/W1, X) |
| Tiers and enrage | a (`tier` key) + Y's DLL | **No** | `tier` input on Creature Spawn (X; key already in the DLL) |
| Alive cap | a (`alive_cap`) | Partly (Creatures Alive + a branch; no cap on spawners) | cap setting on the wave spawner / `ls_wave_*` (W1), node input (X) |
| Barricades: break, repair (hold USE) | b (`ls_barricade_*`, forwards) | Partly: Barricade Broken event only | **Barricade HP / Set HP / Repair** nodes, **Barricade Repaired / Damaged** events (X, natives exist) |
| Armory / trader break (lst_shop, Barney keeper) | a (`shop_menu`, `keeper_*`) | Partly: Show Menu (4 options) + Give Weapon + money nodes; no keeper NPC, no priced shop | **Shop** component (items, prices, a keeper at a marker; bots shop) (W1), Studio "Shop" nodes (X) |
| Points economy (pay per hit/kill/head, wave money) | a (`pay`, `ham_creature_*`) | Partly: Creature Killed + Add Money; no hit/headshot info on creatures | **Creature Hurt** event with attacker, damage, headshot (Y/W1 forward, X event) |
| Power-ups (resupply, double pay, one-hit kills, airstrike) | a (`spawn_powerup` …) | **No** (no pickup entity) | **Pickup** component (model, glow, touched-by event) (W1), Spawn Pickup / Pickup Taken nodes (X) |
| Perks (tough, quick hands, double tap, quick revive, light feet) at `lst_perk` markers | a (`buy_perk`, ham hooks) | Partly: Set Health/Speed; no reload/fire-rate/revive-speed changes | **Player modifiers** natives (reload, deploy, fire rate, revive time) (W1 or X in the Studio plugin), a **Perk Machine** component (W1) |
| Downed / revive | a | **Yes** (Down Player, Revived, Down Timeout) | revive-speed modifier for perks (above) |
| Upgrade bench | a (`bench_upgrade`) | Partly (Damage Scale per attacker by hand) | **Weapon level** native/state + Bench component (W1), nodes (X) |
| Electric trap (bought, zaps creatures at a gate) | a (`trap_on`, `tick_traps`) | Partly: Fire Trap for an `ls_trigger_trap` | **Hurt in radius / Hurt creatures in area** node (X, in the Studio plugin) + a priced trap (Shop/offer) |
| Turret | d | No | planned by nobody yet: W2 asked for `offer … turret`; Y has a "miniturret" creature |
| Mystery box (random weapon, moves) | a (`box_use`, `box_move`) | Partly (Player Used + Random + Give Weapon; only func_button uses) | **Player Used Marker** event (X, marker + USE) + a Box component (W1) |
| Bots: shop, repair, revive, perks, trap, box | c (`goal`, `offer`) | **No** | bot hint nodes (below) |

### Battle Royale
| Feature | Lives in | Studio today | Gap → piece (owner) |
|---|---|---|---|
| Drop-in (from the sky, spread landing, br_drop markers, no fall damage) | a + markers | Partly: Teleport/Set Velocity/No Fall + Find Marker; no spread or open-air search | **Drop In** node (spread landing at markers or open air) (W1 native, X node) |
| Loot-up phase (no damage, then fight) | a + c (`phase loot/fight`) | Partly: Damage Block + timers; bots not told | **Bots: phase** node (X) |
| Loot tiers on crates | b (`tier` key, `ls_loot_roll`) | Partly: Create Component with keys `tier=…`; no roll node | **Roll Loot** node (`ls_loot_roll`) (X) |
| Zone, stages, damage | b (`ls_zone_*`) | **Yes** (Set/Shrink Zone, In Zone, Zone Stage/Tick) | Zone Next / Stage / Skip nodes (X, natives exist) |
| Supply drops | b (`ls_drop_call`) | **Yes** (Call Supply Drop) | Drop Landed event (`ls_on_drop`) (X) |
| Squads (squad mates friends, no friendly fire) | a + c (`role`, `friend`) | Partly: Squad nodes; bots not told who's a friend | **Bots: friends / role** nodes (X) |
| Basic BR HUD (kills, squad health) | a | Partly (Show HUD Text, by hand) | — (a "Squad HUD" node later) |
| Coming: healing items, armor levels, death crates, rarity glow, scarce ammo, a fuller BR HUD, knocked-down state, squad lobby | d | No | nothing written by W1 yet; each one should come as a component or native with a Studio node, as below |

### Bots: telling the brain how to play
| Hint (`ls_mode …`) | Used by | Studio today | Piece |
|---|---|---|---|
| `set <mode>` | both | No | sent by the Studio plugin itself (the mode's id) |
| `phase off/loot/fight` (`noenemies`) | BR | No | **Bots: Phase** node |
| `nobuy 1/0` | BR | No | **Bots: Don't Buy** node |
| `friend <a> <b> 1/0` (`@sqN`) | BR | Partly (Set Squad tells the bots the squads) | **Bots: Friends** node |
| `role <p> <tag>` | BR | No | **Bots: Tag** node (for `@tag` targets) |
| `layer <who> below/surface/high/-` | BR | No | **Bots: Prefer Layer** node |
| `enemyclass ls_creature 1` | LS | No | **Bots: Enemies Include Creatures** (or automatic when the mode spawns creatures) |
| `goal <who> <kind> <xyz> <ent> <prio> <secs> <radius>`, `goal_clear` | both | Partly (Bot Go To / Hold / Follow / Release) | **Bots: Goal** (defend, capture, use, loot, go_hold) node |
| `offer <who> <kind> <xyz> <ent> <cost>`, `offer_clear` | LS | No | **Bots: Offer** node ("this can be bought/used here for N") |
All of these go through one native that exists today, `ls_comp_bots_hint(line)`, so the bot nodes
can be built now; they're next.

### Plan (order)
1. Now (natives exist): bot nodes; Barricade HP/Repair + Repaired/Damaged events; Roll Loot; Zone
   Next/Stage/Skip; Drop Landed; Creature Spawn with health/tier/pay inputs (keys the DLL reads).
2. With W1 (interfaces in W1.md): Round Table / wave caps, Shop + keeper, Pickup, Perk Machine and
   player modifiers, Bench/weapon levels, Box, Drop In, Creature Hurt (with Y), and each coming BR
   system as a component with natives and forwards. I add the nodes as each lands.
3. Then the Last Stand Lite and Royale Lite templates move onto those pieces, and the site line is
   true when a template plays like the real mode (checked side by side on a sim).

## 2026-11-13: tests never install into the real game
QA found pw_v3 and pw_v3_2 to _4 ("V3 check") installed in the real game (the coordinator removed them;
backup in the scratchpad's pw_v3_backup). Now:
- **Guard:** `studio::guard_real_game` refuses, under test (`cfg(test)`: the unit tests and the Tools dev
  server the browser tests run against, which is a test binary), to write into a real Half-Life install
  (the usual Steam places, or anything inside one): compile/install, `register_mode` (so community
  installs too), uninstall, Open as code, switching the game, debugger commands. It says to point
  LS_TEST_GAME at a copy (a sim server) for real installs. Outside tests it does nothing. Unit test
  `tests_never_write_into_the_real_game`.
- **Specs:** `studio-v3.spec.ts` answers every Studio write route in the page (compile, playtest,
  switch, source, uninstall, debug command, the game launch), and an `afterAll` uninstalls and deletes
  whatever its modes might have left (pw_v3, pw_tall, pw_shots and their `_2`… ids);
  `studio-playtest.spec.ts` the same for pw_playtest. Checked: after both specs in Chromium and WebKit
  the real game's modes.ini is byte for byte the same and no `pw_` files are there (the cleanup's
  uninstalls were refused by the guard, as they should be).
- **Other specs (not mine; for their owners):** `editor-assets.spec.ts` calls
  `/api/tools/compile/start` with `install: true` against the dev server's game and relies on the
  refusal (a missing sprite) to keep the map out of the game; `callouts.spec.ts` saves names through the
  real callouts route once and puts the old names back afterwards (a write into the real game's nav).
  tools.spec, editor-*, p10, navfix-ux, shared-ux: no installs, compiles or game writes found (prefabs,
  spray designs and Studio graphs go to the dev server's temp app folder). The same kind of guard in
  the map compile/install and callouts save routes would make those safe too.

## 2026-11-13: menus and details polish (user's real-app pass)
- **One right-click menu for the Studio** (`studio/ContextMenu.tsx`): opens at the pointer, kept on
  screen, keyboard on the first item, ↑ ↓ Home End, Enter, Escape / a click elsewhere / scroll / leaving
  the window closes it and gives the keyboard back; Shift+F10 or the menu key opens it on a focused item.
  The browser's own menu never shows in the Studio or its dialogs (text boxes keep the system's).
- **Every My mode item** (events, Functions, Macros, locals, variables, structs, enums, map markers):
  Rename, Duplicate (a Function or Macro with its whole graph), Find uses (selects them, says where),
  Save to my library (Functions, Macros, structs, enums; greyed with why on the others), Delete (asks
  first; when it's used, says where: "1 node on the Event Graph, the variable “X”"). Select an item and
  press Delete or Backspace for the same. (`studio/parts.ts`.) Library items: Add, Export, Delete;
  snippets: Add, Delete. Sidebar items take the keyboard when clicked (WebKit doesn't by itself).
- **Canvas:** a node's menu (Open its graph for a call, Duplicate, Comment around, Breakpoint, Delete,
  on the selection it's in), a wire's (Add a reroute point, Break), a tab's (Close, Close the others,
  Close all). Find in this mode: ↑ ↓ Enter, closes on Escape and on a click elsewhere.
- **Details panels:** the comment's colours are a labelled field on the same left edge, 24 px swatches
  8 px apart, the picked one ringed; buttons first in a line no longer step 6 px in (the app's
  `p > .btn.small` margin); fields in boxes the same 13 px as the rest; a marker's "Use a circle" at
  its own width, its key rows name / type / × on one line; removing a used variable asks first.
- Tests: `tests/studio-v3.spec.ts` "our right-click menus…" (Chromium and WebKit, with all the others).
- **The Studio never scrolls as a whole:** it fills the window (`.content:has(.st-page)`, no longer only
  a direct child; `.st-page` flex 1 1 0, min-height 0; the page clips) and its panels scroll inside
  themselves; the Studio's own focus() calls use preventScroll. The page scroll seen earlier came from a
  wrapper around the page (tried for the right-click work, not committed) breaking the direct-child
  selector. Test: "the Studio never scrolls as a whole" (every-node graph, 1440×900 and 1024×680:
  picking items, the canvas, Home/F, going to a node, a menu, the Find list, the end of My mode;
  Chromium and WebKit); it fails if the page grows again.

## 2026-11-13, night: map markers in the Studio, kits, mode uids
- **Graph:** `markers` (id = the map's `kind`, label, description, type point/area/path, min/max, keys
  number/text/bool/choice with defaults, colour, icon, an area's size or radius) and `mapKit` (size,
  players, notes); checked in `graph::marker_problems` (also used for kits). `mode.uid` (see above).
- **Editor:** My mode > **Map markers** (add, pick → Details: name, id, description, shape, counts,
  colour, icon, area box/circle, keys) and **Map advice**; drag a marker onto the canvas for Find Marker.
- **Nodes** (category Map markers): **Find Marker** (marker, Which 1st/2nd… → Marker, Position, Found),
  **Marker Count**, **For Each Marker** (Each, Marker, Number, Then). Paths come in `path`, `order` order.
  **Marker Key** (type `marker.get`; the marker entity and a key name → its text and number, the
  kit's default when the map left the key out).
- **QA round 3:** Set/Shrink Zone with a wired Shrinking zone stopped with `ls_zone_center` run-time
  error 10: gm_components' zone natives take the **safe zone** (`ls_safe_zone`), and a Shrinking zone only
  drives one. The node now uses the wired Safe zone, or the one the wired Shrinking zone drives (its
  `zone` key), and says "That isn't a zone" otherwise; Create Component can make a Safe zone. A loop
  whose rounds wait at a breakpoint now says "N rounds (some still wait at a breakpoint)", not "N times".
- **Kits:** `studio/kit.rs`: `kit_of(&Graph)` (markers, components the nodes act on minus those the
  graph makes, creature types, advice), `check_kit`, `install_kit`, `list_kits_in`. Saved next to the
  graph on every save (`<id>.kit.json`), installed to `<app dir>/kits/` by `register_mode` (every install:
  Install, Playtest, community). Routes `GET /api/tools/studio/kits`, `GET /api/tools/studio/kit/:id`.
- **Tests:** kit generation/refusals/install (`kit.rs`), uid install/clash (`mod.rs`), every_node and a
  `markers.graph.json` snapshot, amxxpc compiles both with no warnings. **Sim (XMK, de_dust2 with 9
  `ls_marker`s added to its entity lump):** For Each Marker ran 3 times (mode `mk_test`, `any`, none; the
  `other_mode` one and an unknown kind left out) with the right origins, Marker Count 3, the area found,
  the path's 2nd spot by `order` (444 555 66 from orders 3/1/2), the 99th not found; the same with W1's
  gm_components (d857c24) loaded. Later run: Marker Key read `wave` 3 / `side` south from the map and the
  defaults (1, north) elsewhere; Set Zone through the Shrinking zone `shr` and Shrink Zone on the Safe zone
  `sz` both ran ("done", no run-time errors); with a breakpoint in For Each the loop said "3 rounds (some
  still wait at a breakpoint)".

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

## Rule from Y (2026-11-13): touch only your own server processes
- Never attach a debugger (`winedbg <pid>`, `attach`, DebugBreakProcess, anything that puts a thread into another
  process) to a running hlds under CrossOver: the injected thread starts at 0xfff4fadc and kills the server.
- Stop, signal or inspect only the processes your own script started: by the PID you got when you started it, or by a
  pattern that names your own sim folder (`Sim[\\/]<yours>[\\/]hlds.exe`). Never by name (`hlds.exe`, `winedbg`) and
  never by "the newest process". Before a start, make sure your port is free (`lsof -nP -iUDP:<port>`).
- Stacks and crashes without a debugger: `drive_c/Loner-Strike/Sim/crashes.log` (every crash in the bottle, full
  winedbg report), and with fork builds from ls-creatures `f351765` on, `cstrike/ls_crash*.log` and
  `cstrike/ls_watchdog.log` in the server folder.

## Requests from QA, round 3 (2026-11-13; full table in docs/notes/QA.md "Round 3")
Confirmed fixed for real: #1-#7, Create trap/extraction, In Zone, #9 (Player Hurt on blocked, names with spaces,
Creature Count, Team Changed, uninstall leftovers); templates play; `npm run test:studio` 20/20; library routes.
Still open: **Zone Set / Shrink with a wired Shrinking Zone** (`Entity 162 is ls_zone_shrinker, not ls_safe_zone`, then
run-time error 10 in `ls_zone_center`; flow ends) — with W1. New (Low): For Each reports "N times" before the rounds
waiting at a breakpoint have run. Pending (not committed when tested): Entry/Return details, shortcuts, snippet
routing, library export/import, the leave flow.

## From P5 (2026-11-13): your guard on the map side too
`studio::guard_real_game` now also guards the map compile's install (`compile.rs` plan with `install: true`, and
`install_bsp` itself) and the .res write (`doctor::write_res`, which the editor's Playtest calls). Checked against the
dev server: a clean map with `install: true` gets your refusal and nothing lands in the real cstrike/maps.
editor-assets.spec.ts keeps its real call (the missing sprite still stops it first) with a note. Your message says
"the Game Mode Studio" for these too; a wording like "Under test Loner-Strike doesn't write into your real game"
would fit both, your call.

## ==== W3: Battle Royale components for Studio nodes (from W3, 2026-11-13) ====
W3 builds the BR systems as gm_components features (W1's queue item 9). Each is landed (committed) when listed here;
full text in COMPONENTS.md "Battle Royale pieces", signatures in ls_components.inc ("W3: Battle Royale pieces").
Cvars are the mode's switches (a Studio mode can set them with its cvar node).
- **Healing items** (25140068): cvars `ls_heal_carry 1` (carried, else instant), `ls_heal_limits "6 2 3"`,
  `ls_heal_secs "3 7 4"`, `ls_heal_key 1` (F), `ls_heal_bots 1`. Kinds `bandage`, `medkit`, `drink`.
  `ls_heal_give(id, const kind[], n = 1)` → taken; `ls_heal_count(id, const kind[] = "")`;
  `ls_heal_use(id, const kind[] = "")` → 1 started; `ls_heal_cancel(id)`; `bool:ls_heal_using(id)`.
  Forward `ls_on_heal(id, const kind[], stage)` (0 starting, PLUGIN_HANDLED refuses; 1 used; 2 stopped).
- **Armor levels** (6b4aecdf): cvar `ls_armor_levels 1`; loot `vest1..3`, `helmet1..3`.
  `ls_armor_set(id, vest, helmet)` (-1 keeps one); `ls_armor_get(id, &vest, &helmet)` → armor points.
  Forward `ls_on_armor(id, vest, helmet)`.
- **Scarce ammo** (060d9a90): cvars `ls_ammo_scarce 1`, `ls_ammo_mags 1`; items `ammo` (two magazines per gun) and
  typed `ammo:762`, `ammo:556:60`... through `ls_give_item`. No natives of its own.
- **Rarity glow** (bed0141b): cvar `ls_rarity_glow 1`; `ls_item_tier(const item[])` → 1-5; `ls_glow(ent, tier)`
  (0 = off). Colours white, green, blue, purple, gold.
Next: death crates, the match HUD (kill feed, spectate), drop-in (`ls_drop_in`), squads + lobby, knocked-down squads.
## ==== end W3 ====
