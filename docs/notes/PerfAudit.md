# Performance audit (2026-11-13)

This is an assessment only: no code was changed. The list is ranked by how much each problem hurts real use,
worst first. **QW** marks a quick win (about half a day or less, low risk).

## How it was measured

**The machine.** Everything ran on the user's Mac while other agents were working. The load average was 11–17,
with 6–7 other sim servers running. Absolute numbers will be better on a quiet machine. Compare the rows with
each other rather than with a fast PC.

**Backend.** A release build of the Tools routes (`serve_tools_dev`, built from `tools` at 18:16), with its own
app folder.

**UI.** Driven by headless Playwright WebKit (the Mac app's engine) at 1600×1000, against two builds:
- Vite dev: for render and draw counts (`__lsEditorGl`);
- a production build of `index`, `editor-dev` and `tools-dev` (`vite build`, served by `vite preview`): for
  frame times.

**Frame times** are the gaps between requestAnimationFrame calls. 16–17 ms means smooth. Over 50 ms is a
visible hitch.

**CPU hot spots** come from Chromium CPU profiles (CDP) of the same steps in the dev build.

**Memory** is the RSS of the WebKit WebContent and GPU processes, sampled with `ps`.

**In-game** measurements used a clone of the LSPLAY sim (the brain with `ls_perf`), de_dust2, 12 bots, and
sys_ticrate 100/1000:
- an A/B of the Studio "Every Node" mode with `amxx pause`;
- a micro-benchmark plugin that times the generated code's hot patterns with `tickcount()`.

**The map** was br_valley: 3.4 MB .map, 5432 brushes, 756 entities. The editor ran in the Quad layout.

---

## 1. Map editor: every edit, however small, redraws the whole map (and reloads model textures)

**What's slow.** On br_valley, nudging one brush with an arrow key and undoing it is janky. 30 nudge+undo
pairs (60 edits) took 28.8 s (WebKit, production build):
- frame gaps were p50 89 ms and p95 177 ms;
- 142 of 313 frames took over 100 ms;
- the page stays at about 5–10 fps for the whole editing session.

Each edit sends this chain:

| Request | Size | Time |
|---|---|---|
| `POST cmd` | | 30–150 ms |
| `GET meshes?since` | 2 KB | 50–120 ms |
| `GET doc` | 654 KB, the whole document | 140–250 ms; up to 1.3 s under contention |
| `GET checks` | | 450–600 ms of server CPU, every edit |
| `GET budget` (when its panel is open) | | 50 ms |

**Who it affects.** Everyone editing a big map; every mapper on br_valley-sized maps.

**Likely causes** (from the CPU profile of 5 nudges):
- **The whole map is re-merged in every pane.** `EditorScene.tsx` `Brushes` re-merges every unpicked brush
  whenever the `brushes` Map changes (`useMemo(..., [brushes, sel, ...])` → `MergedBrushes.tsx`
  `mergeBrushes`). `Brushes` lives inside each pane's portal, so this happens 4 times in Quad.
- **Model textures are reloaded.** `EntityBatch.tsx` rebuilds `built` (keyed on `ents`, a new array after every
  `refreshDoc`). That re-creates every model's InstancedMesh, and `modelMaterials` (`EntityModels.tsx`) loads
  every model texture again with `TextureLoader`. In the profile, `texSubImage2D` texture uploads took 235 ms
  per 5 small edits and 1.6 s for one big-selection nudge.
- **The full document is re-sent.** `store.ts` `cmd` → `refreshDoc()` downloads the full 654 KB `/doc` after
  every command. Then React re-renders everything that reads `doc`: the outliner and the entity batch. In the
  profile, `prepareUpdate`, `jsxDEV` and `updateHostComponent` took about 1 s per 5 edits.
- **Checks run in full after each edit.** `store.ts` `refreshChecks` waits only 350 ms. The checks run off the
  lock and stale answers are dropped, but each run still burns about 0.5 s of CPU per edit.

**Fix ideas:**
- (a) Cache model textures by model+skin in a module-level map. Rebuild `EntityBatch` from entity *content*
  (key by id and revision), not array identity. **QW**, S.
- (b) Merge once for all panes: lift `merged`/`mergeBrushes` out of the per-pane `Brushes` into a shared store
  or memo. S–M.
- (c) Make the merge incremental: split the merged map into spatial chunks (for example 1024-unit cells) and
  rebuild only the chunks whose brushes changed. M.
- (d) Send document deltas: the `cmd` response (or `/doc?since=rev`) returns only the changed entities and
  groups, and the store patches them in place. M.
- (e) Checks: debounce 1–1.5 s and skip while the user is still editing, then go incremental per brush. **QW**
  for the debounce; M for incremental.

## 2. Map editor: a click that selects one brush freezes the page for about 0.3 s on a big map

**What's slow.** Clicking a brush in the Top view of br_valley gave one frame of 291 ms (production) or 341 ms
(dev). Box select is worse (see #3). Deselecting (Esc) costs the same.

**Who it affects.** Every selection on big maps. It feels like input lag.

**Likely cause.** Selection is part of the merge key. `Brushes` splits brushes into merged and picked by `sel`,
so any selection change re-merges all 5400 brushes, times 4 panes (`EditorScene.tsx` `Brushes` → `mergeBrushes`).

**Fix ideas.**
- Keep the merged geometry for all brushes and hide picked ones by attribute: a per-vertex "picked" flag or
  alpha, or degenerate the picked brush's index range. Draw the picked brushes on top separately.
- Then a selection change touches only the picked brushes.
- Pairs with #1(b). S–M.

## 3. Map editor: big selections stall for seconds, redraw slowly and keep memory

**What's slow** (box-selecting half of br_valley, 3142–3269 brushes):

**Time.**
- The box select itself had a 1.4 s frame (production) or 1.7 s (dev).
- Each redraw afterwards takes 47 ms with about 6,400 draw calls per redraw. Hovering then gives p95 58 ms and
  a max of 717 ms.
- Nudging that selection: `meshes?since` returns 7.2 MB (150–950 ms) and undo takes 2.8 s end to end.

**Memory.** RSS of the WebKit WebContent process:

| Step | RSS |
|---|---|
| br_valley open | 775 MB (GPU process 135 MB) |
| after the big selection | 2,356 MB |
| after Esc | 2,694 MB |
| after 3 more select/deselect cycles | 3,565 MB (GPU 524 MB) |
| after 10 s idle | 3,565 MB, not given back |

That is about +300 MB per big select/deselect cycle.

**Who it affects.** Anyone who box-selects a large area: moving a whole section, grouping, select-all. Over a
session it can push a Mac into swap.

**Likely cause.**
- Each picked brush is drawn one by one with `BrushMesh`.
- Each `BrushMesh` builds `new THREE.EdgesGeometry(b.geometry, 20)` itself, which is expensive and not cached.
  `MergedBrushes.tsx` has an `edgeCache`, but `BrushMesh` doesn't use it.
- All of that happens in every pane (4×).
- Every selection change also allocates a new full merged copy of the map per pane (#2).
- The memory that isn't returned is either GPU buffers not disposed when the picked meshes unmount, or merged
  arrays kept alive. It needs a heap snapshot to tell which. Either way the growth is real and repeatable.

**Fix ideas.**
- Draw the picked set merged too: one mesh per texture plus one outline buffer, built from the cached
  `edgesOf()`.
- Share it across panes.
- Then check that geometries are disposed when a merge is replaced: `MergedBrushes` disposes them on change,
  but verify in a heap snapshot.
- M.

## 4. Game Mode Studio: big graphs drag, wire and pan at 1–5 fps

**What's slow** (WebKit):

| Graph | Open | Drag one node | Drag a new wire | Pan (drag empty space) |
|---|---|---|---|---|
| every_node (374 nodes, 509 wires, ~10k DOM elements) | 4.3 s, production | 191 ms per frame at zoom 1 (dev); about 1 s per frame zoomed out (production); same with everything selected | 262 ms per frame | 56–64 ms per frame |
| every_node_2 (116 nodes) | | p50 28–33 ms, p95 60 ms | | 57 ms per frame |
| Last Stand Lite (75 nodes) | | p50 16–18 ms, smooth | | 19 ms |
| Royale Lite (53 nodes) | | p50 16–18 ms, smooth | | 20 ms |

The palette search, the "Find in this mode" box and undo are fine on every graph (p95 under 33 ms).

**Who it affects.** Mode makers with real modes past about 100 nodes. The built-in templates are fine today.

**Likely cause.** Every drag step re-renders every node.
- `Canvas.tsx` `onNodesChange` → `live(moveItems)` makes a new `graph`.
- Every `LogicNode` (`Nodes.tsx` 71–80) subscribes to the whole `s.graph`, `s.problems` and `s.debug.runs`.
- Each render does `graph.nodes.find` and `graph.edges.some` per pin, and `watchOut` scans edges, so each step
  is O(nodes × edges).
- The `edges` memo depends on `graph`, so all wires are rebuilt too.
- React Flow has no `onlyRenderVisibleElements`, so panning moves and paints all 10k elements.
- The profile is dominated by React reconciling `LogicNode` (`watchOut` at `Nodes.tsx:120`, `Nodes.tsx:90`) and
  `EdgeWrapper`.

**Fix ideas.**
- (a) Per-node selectors: subscribe to the node by id, and get a precomputed "wired pins" set per graph
  revision (one pass over edges). `runs[id]` instead of all `runs`. Compare `problems` per node. **QW**-ish, S–M.
- (b) While dragging, let React Flow own the positions and commit to the store on drag stop. Today it commits
  every step. S.
- (c) `onlyRenderVisibleElements` on `<ReactFlow>`. **QW**, S. Check that the minimap and edges still look
  right.

## 5. Studio Playtest (debug) builds: every node run writes to disk

**What's slow.** Measured on the sim (de_dust2, Windows hlds under CrossOver):
- One debugger event is `fopen` + `fprintf` + `fclose` = **181 µs**.
- Each node run emits at least enter and exit, so 2 events, plus per-input formatting.
- Example: a For Each Player over 32 players with 4 nodes inside is about 256 events, which is about **46 ms in
  one server frame**. At tick 100 that is a visible hitch for everyone on the server.
- The `.events` file is only cleared at map start, so it grows all map.

**Who it affects.** Anyone playtesting from the Studio: the user and their bots. It doesn't affect shipped modes;
release builds have no instrumentation (`gen.rs` emits it only when `debug`).

**Likely cause.** `debug_runtime.pawn` `lsd_emitf` (lines 113–121) opens, appends and closes the file once per
event.

**Fix idea.**
- Buffer events in memory (a fixed-size ring) and write them in the existing 0.25 s `lsd_poll`.
- Skip emitting when no Studio is listening (an attach file, or a cvar the app sets).
- **QW**, S–M.

## 6. Studio "Save: Set" rewrites the whole save file on every call

**What's slow.** `lsg_saved_flush` with 128 keys (32 players × 4 stats) took **1.09 ms per call** on the sim.
Wired to Player Hurt, a kill or a creature kill, that is several ms per second in busy fights. A shotgun blast
alone can be 9 hits.

**Who it affects.** Servers running Studio modes that save stats in combat events.

**Likely cause.** The generated `lsg_saved_set` (`gen.rs`, the SAVED block, about lines 3583–3653) calls
`lsg_saved_flush()` straight away. The flush snapshots the trie, writes a `.new` file and renames it over the old
one.

**Fix idea.** Set a dirty flag and flush on a 5–10 s task. The flushes at new round, disconnect and map end
already exist. **QW**, S.

## 7. Editor backend: work done while holding the document lock

**What's slow.**
- `GET meshes?since` for the 3142-brush nudge took 150–950 ms. During it, `GET doc` waited up to 1.3 s and undo
  took 358–660 ms.
- Autosave takes 79–149 ms on br_valley and holds the lock while it serializes the 3 MB document and writes it to
  disk. So every few minutes an edit can wait that long.

**Who it affects.** Big maps, big selections, and edits that land during an autosave.

**Likely causes** (`app/src-tauri/src/tools/editor.rs`):
- `api_meshes` → `meshes_bundle` runs `ops::brush_mesh` and builds JSON for every changed brush while holding the
  lock.
- `Session::track` compares every brush and clones the whole brush map (`self.prev = self.doc.brushes.clone()`)
  after every command, even a one-brush nudge.
- `api_autosave` → `autosave` runs `to_json` and `write_atomic` under the lock.

**Fix ideas.**
- Take the changed brushes (clones or `Arc<Brush>`) under the lock, then build meshes and write autosaves off it.
- Track changes from the history `Change` items instead of diffing and cloning the whole map.
- M. The autosave part is **QW** (S).

## 8. Studio debugger: the canvas re-renders every node on every event batch

**What's slow.** While a Playtest is attached, `StudioPage.tsx` polls `debug/events` every 60–150 ms and
`useStudio.events` updates `debug.runs`. Every `LogicNode` subscribes to the whole `s.debug.runs`
(`Nodes.tsx:79`), so every batch re-renders every node. That is the same per-render cost as a drag step in #4:
about 200 ms for 374 nodes.

This is inferred from the measured re-render cost. A live Playtest wasn't run in the browser. On a big graph the
Studio would be close to fully busy while debugging, and a little worse with the 1 s ticks (`StudioPage.tsx:418`,
`DebugPanel.tsx:18`).

**Who it affects.** Mode makers debugging bigger modes.

**Fix idea.** The same per-node selectors as #4(a): `s.debug.runs[id]`. S. **QW** once #4(a) is done.

## 9. Generated plugins: scanning nodes are pasted inline, and the Studio never warns about cost

**What's slow** (measured on the sim, de_dust2, about 250 edicts):
- A full entity scan (Creature Count, Comp Find, Pick or For Each Entity) costs **88–114 µs**, more on bigger
  maps (BR maps run 600+ edicts).
- Pure nodes are generated inline at every use (`gen.rs` `Gen::output`), so a Creature Count read in a While
  condition rescans on every pass.
- Loops are capped at 10,000 each, but not in total, so nested loops can reach 10^8.
- Cheap by comparison: a For Each Player pass with alive + origin is 0.5 µs, and HUD text to every player costs
  nearly nothing on the CPU (it is 32 network messages per fire).

**Who it affects.** Servers running modes made by people who don't know these costs.

**What's fine** (read in code):
- The Studio has no per-frame events.
- Timers are clamped to at least 0.1 s (`gen.rs` `lsg_secs`).
- Touch events are limited to once per second per player.
- The A/B on the sim of the "Every Node" mode (374 nodes, all events and timers) against the same server with it
  paused showed no fps difference above noise. With 12 bots at tick 1000: all on 425 fps; Studio mode paused
  401 fps; it and gm_components paused 403 fps. The Mac's load was the limit.

**Fix ideas.**
- Cache a scanning node's value once per event run: evaluate it into a local at first use, like Unreal's
  pure-node caching per exec. M.
- Add the warnings listed in the next section, through the existing warning badges (`Problem::warn` in
  `graph.rs`, shown as a badge in `Nodes.tsx`). S–M per warning. The first three are **QW**.

### Patterns the Studio should warn mode makers about

Costs are measured on the sim where given; otherwise read from the generated code.

1. **Save: Set in a frequent event** (Player Hurt or About to Hurt, a kill, Creature Killed or Hurt, Zone Tick,
   a Repeat under 1 s): "This rewrites the save file every time (about 1 ms). Save on round end or on a timer
   instead." Not needed once #6 is fixed.
2. **A scanning node read inside a loop or a While condition** (Creature Count, Comp Find, Players array, Count
   Alive): "This looks through every entity (about 0.1 ms) each time round the loop. Set a variable before the
   loop."
3. **Nested loops** (any For, While, For Each Array or For Each Entity inside another): "Up to 10,000 × 10,000
   passes can freeze the server."
4. **For Each Entity or Pick Entity in a frequent event or a timer under 1 s**: "This runs every time for every
   entity on the map."
5. **Repeat or Set Timer under 0.5 s with real work inside** (loops, HUD, bot commands): "This runs N times a
   second for every player." HUD faster than 1 Hz is also network spam.
6. **Delay inside a frequent event or a For Each**: each fire adds another task, because it doesn't cancel the
   previous one. "Use Retrigger."
7. **Messages, HUD or chat inside For Each Player in a frequent event**: about 32 network messages per fire.
8. **A heavy chain on Player Hurt, About to Hurt or Creature Hurt**: these run once per hit, which can be
   hundreds of times a second with shotguns and creatures.
9. **Playtest builds** (a note in the Playtest panel): "Debug builds log every node to disk. Don't judge
   performance with them" (#5).

## 10. App: idle polling (cheap now, but always on)

**What's slow.** On a fresh profile the release app idles cheaply. Over 60 s idle it used 0.23 s of CPU, and its
WebView 0.13 s: about 0.6% of one core. RSS was 150 MB for the app and 58–67 MB for the WebView. The local
server answers 0.8 s after launch.

But the request rates are high:
- about 43 requests a minute on Home: `/api/state` every 2 s, servers every 5 s, reports every 60 s;
- about 83 a minute on the Map Editor, because "outside changes" adds `/api/tools/editor/state` every 1.5 s.
  I saw that one in every idle measurement.

About a dozen Rust threads also wake every 2–15 s. Worst is `notify.rs` `watch`: every 5 s it rereads the
settings file and checks the servers.

**Who it affects.** Laptop battery, and everyone with servers configured. Those costs grow with the number of
servers. I didn't measure that (fresh profile only).

**Likely causes:**
- `ui/src/queries.ts:54` `useGameState` (2 s, mounted from `App.tsx`), and `:140` `useServers` (5 s, always on);
- `views/tools/editor/extras.ts:365` `POLL_MS = 1500`;
- `ui/src/queries.ts:13-19`: `refetchOnWindowFocus` with `staleTime: 1000`, so every focus refetches everything;
- Rust: `notify.rs` (5 s), `servers.rs` `sync_loop` (6 s, plus a file walk every minute), `watch` (3 s),
  `watch_frozen` (15 s), `smartchat/mod.rs` (2 s even when it's off), `schedule_restarts` (15 s).

**Fix ideas.**
- Poll `/api/state` every 2 s only while a game or server is running, otherwise every 15 s.
- Stop polling servers when the list is empty.
- Editor: 1.5 s → 5 s, and only when MCP or a second window can change the document.
- `staleTime` of 30 s by default.
- Park the Rust watchers when nothing is running.
- Each is **QW** (S).

## 11. gm_components: always-on per-frame hooks

**What's there** (read in code; the sim A/B in #9 couldn't separate their cost from noise):
- `register_forward(FM_TraceLine, ...)` (`gm_components.sma:477`) on every map and in every mode, called for
  every engine trace. Its stats counters run before the early return.
- Three separate PreThink handlers: `fm_prethink` :3705, `fm_w3_prethink` :6792 and `up_prethink` :10600.
- Plus `fm_w3_postthink` :7430 and `fm_w3_cmdstart` :5596, all registered unconditionally.
- Estimate: 10k–100k Pawn calls/s at tick 100–1000 with 32 players, about 0.5–3% of a core.

**Who it affects.** Every server running a components mode: Last Stand, BR, Studio modes.

**Fix ideas.**
- Register the TraceLine hook only when the map has a see-through barricade.
- Merge the three PreThinks into one.
- Register PostThink only while someone is dropping in.
- Put the trace counters behind the debug cvar.
- **QW**, S. Low priority, because it was unmeasurable on this machine.

Smaller ones in gm_components:
- `solid_when_clear`, `up_trap_tick` and `up_bots_traps` each scan the whole entity list per barricade or trap,
  every 0.25–1 s.
- `usable_near` does trie lookups per human per component every 0.25 s tick.
- Fix: build a "creatures alive" list once per tick, and cache key values at `comp_init`. S.

gm_laststand and gm_battleroyale are cheap: 0.2–1 s timers and no per-frame hooks. `tick_powerups` (0.2 s)
could fold into the 1 s tick.

---

## Measured and fine (no action)

**Map editor**
- **Opening br_valley:** the paste of 3.4 MB takes 113 ms. From click to drawn and counted: 308 ms
  (production) to 383 ms (dev). The first `meshes` is 7.3 MB in 120–250 ms; `doc` is 654 KB in 22 ms.
- **3D and 2D navigation in Quad:** fly, wheel zoom, middle-drag pan and hover all ran at p50 16–18 ms and p95
  19–28 ms. Panes only redraw on demand: 0 renders in 5 s idle. A redraw costs 1.8–4 ms and about 27 draws per
  pane, about 1.4 M triangles.
- **Dragging a picked brush in Top:** p50 18 ms. One `cmd` at release, with a single 157 ms hitch at release
  from #1.
- **Content Browser:**
  - `content/list`: models 20–60 ms, sounds (3661) 40–95 ms, sprites 24 ms.
  - The Models tab draws 600 tiles in 60 ms. Thumbnails are drawn only for the tiles you can see, one model at a
    time, and frames stayed at 16–20 ms while drawing.
  - The Sounds tab takes 249 ms; search is smooth.
  - Thumbnails are cached only per session (data URLs in memory). A disk cache would save redrawing them on the
    next run (nice to have).
- **Entity I/O:** `/io` takes 3 ms. The page's `ioLinks` is linear.
- **Compile kickoff:** the work runs in a thread and `compile/status` answers in 3–5 ms. That route clones the
  whole document just to read its name (`compile.rs` `open_document().map(|(_, n)| …)`); a name-only accessor
  is a trivial **QW**.
- **Budget:** 47–56 ms, run 700 ms after edits while its panel is open.
- **Checks on an unchanged document:** cached, 5 ms.

**Studio**
- **Backend** (every_node): validate 8 ms, generate 9 ms (87 KB of source; 148 KB for debug).
- **amxxpc under CrossOver:** every_node 1.9 s (debug 2.5 s), Last Stand Lite 1.3 s, Royale Lite 1.0 s. Mostly
  Wine start-up. Unchanged graphs are already skipped by hash.

**App**
- **Bundles** (production): the main `index` chunk is 281 KB. `MapEditor` 497 KB, `StudioPage` 693 KB, `three`
  688 KB and `monaco` 3.3 MB are all lazy. `StudioPage` is the biggest page chunk; it may be worth checking
  whether the templates and `nodes.json` could load on demand.

## Not covered

- A live Studio Playtest attached in the browser (#8 is inferred).
- Startup of an app profile with servers configured (only a fresh profile was measured).
- A heap snapshot to separate a real leak from lazy GC in #3.
- A quiet machine for absolute in-game numbers.

The test scripts were throwaway: `editor-perf.mjs`, `studio-perf.mjs`, `mem.mjs`, `cb.mjs` and `drag.mjs`
(Playwright), plus `pa_bench.sma`. They are not in the repo; ask if they should be added as specs.
