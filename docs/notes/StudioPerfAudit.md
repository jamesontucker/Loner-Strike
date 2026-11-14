# Game Mode Studio performance audit (2026-11-14)

This is an assessment only: no code was changed. It follows PerfAudit.md (items 4, 5, 6, 8 and 9 were fixed after
it). Findings are ranked by how much they hurt real use, worst first. **QW** marks a quick win (about half a day or
less, low risk).

## How it was measured

**Machine.** The user's Mac while other agents worked. The load average was 8–16, with 5 other sim servers running.
Absolute numbers will be better on a quiet machine, so compare rows with each other.

**Builds.** Three builds were measured:
- the Vite dev server (the "dev UI");
- a production build of the working tree at 02:18 (about `462b4872` plus uncommitted edits);
- a production build of a clean snapshot of `tools` HEAD `90ecb741` (03:12).

Each row says which build it came from. Other agents were editing Studio files during the audit. Vite hot reloads
reset the dev page several times, so the debugger numbers come from the HEAD production build.

**Backend.** A release build of `serve_tools_dev` over a sim copy (`Sim/SPA`, an APFS clone of LSPLAY), with its own
app folder.

**UI.** Driven by headless Playwright at 1920×1080:
- Chromium: the main platform; Windows uses WebView2;
- WebKit: the Mac app's engine.

Frame times are the gaps between requestAnimationFrame calls: 16.7 ms is smooth. A "step" is one mouse move, and
"per step" is the wall time over all the moves. Latency is the time from the input event to two frames later.
Hot spots come from Chromium CPU profiles.

**Graphs** (generated, in my scratchpad):
- **one400**: 367 nodes, 510 wires and 27 comment boxes on one Event Graph. It has reroutes, 4 Functions that each
  play a per-player Timeline (float, color and event tracks with Ease and Custom keys), and a 256-row, 6-column data
  table read by Get Row.
- **big400**: the same graph spread over 16 event graphs.
- **one1k**: 952 nodes and 1,320 wires.
- **sim**: a 22-node mode for the game.

**1,000+ nodes isn't possible.** The Studio refuses graphs over 400 nodes, 1,200 wires or 100 comments
(`graph.rs:42-44`, `schema.ts:144`). The 952-node rows were measured as headroom only: in the dev UI, with the
schema's limits raised in the browser.

**In game.**
- `Sim/SPA` on de_dust2, with the newest green ls-next3 brain (CI 43585243650, `a868a82`, `ls_perf` and
  `ls_creature_ver` found in its mp.dll).
- Plugins compiled fresh from tools HEAD (`plugins-fresh.sh`), `-insecure`, maxplayers 32, 20 bots,
  `ls_creature_adaptive 0` and 90 creatures.
- A micro-benchmark: the Studio's own generated plugin, release and Playtest (debug) builds, plus a `pa_bench`
  server command. It times each hot function with `tickcount()` over 300–20,000 calls.
- An A/B with `amxx pause`, read from `ls_perf 10`.

---

## 1. Mac (WebKit): a single big Event Graph takes seconds to open, edit and undo, because of the wire layer

**What's slow** (one400, 174 nodes and 264 wires drawn at the opening view, WebKit):

| Step | Now | With `.react-flow__edges { will-change: transform }` |
|---|---|---|
| Open the mode | 7.9–9.5 s (02:18 build), 4.6 s (HEAD; dev 4.6 s) | 0.77–1.1 s |
| A small edit (a 4-move drag), input to paint | p50 147 ms, p95 5.5 s (02:18); **p50 3.3 s, p95 8.8 s** (HEAD) | p50 132–148 ms, p95 194–211 ms |
| Undo / redo | p50 65–82 ms (02:18); **p50 5.1 s** (HEAD) | 83–88 ms |
| Data table opened after editing | 2.9–3.5 s (02:18), 4.6 s (HEAD) | 0.42 s |
| Dragging all 367 selected nodes | 813 ms per step, frames up to 1.4 s | 148 ms per step |
| Pan, everything in view | frames p50 60 ms | p50 16 ms, p95 48 ms |

Chromium opens the same graph in 0.34 s, and its edits paint in 2 frames. A graph spread over 16 event graphs opens
in 0.29 s in WebKit, because only about 28 nodes are drawn.

**Where the cost goes.** Bisected with CSS:
- hiding the wires takes the open from 8.3 s to 1.0 s;
- nothing else changed it: node shadows, exec pin `clip-path`, dashes, the minimap, the background or the wire hit
  areas.
- The page shows two frames of about 4 s each: the first render, then the fit.

The wires live in one SVG layer the size of the graph, scaled by the viewport transform. WebKit redraws that whole
layer whenever the canvas or the wires change: a commit, a selection, the table sheet opening over the canvas.
Promoting it to its own compositing layer stops that. This is likely also the cause of PerfAudit #4's 4.3 s open of
every_node in WebKit, which was never fixed.

**Who it affects.** Mode makers on Macs with a big mode on one tab. Splitting a mode into event graphs hides it.
Windows (WebView2) is fine.

**Fix idea.** Add `will-change: transform` (or `transform: translateZ(0)`) on `.react-flow__edges` in `studio.css`.
**QW**, XS. Check that wires stay sharp when zoomed in the real Mac app:
- X's note in `studio.css:91` warns that a filter made WebKit draw a layer as a bitmap;
- in a headless screenshot after zooming from 0.2 to 2.5 the wires looked the same, but WKWebView may differ.

If they blur, set the property only while panning or dragging, or draw wires per tab section.

**Fixed (X2, 38949bf8).** `.st-canvas .react-flow__edges { will-change: transform }` in `studio.css`. Sharpness: in
Playwright's WebKit, the wires at zoom 2 (after opening at 0.25 and zooming in) are pixel-identical with and without
it. **Checked in the real Mac app too (X2, 2026-11-14):** the debug build (`target/debug/loner-strike`) run with a
throwaway `HOME` and `CFFIXED_USER_HOME` (its settings, WebKit data and caches all landed there; the real
`~/Library` WebKit, Caches and Preferences entries unchanged, no game found), its window pointed at a dev page by
`LONER_UI_URL`, every_node opened, zoomed 0.25 → 2.5 → 1.6 with the wire layer promoted, plain, then promoted again;
`screencapture -l` of the window: promoted and promoted-again identical, plain differs in 249 of 250,000 pixels
(where several green wires overlap), edge sharpness within 0.13%, no visible difference. Kept on (no need to gate it
to interaction). Measured (every_node, dev UI, WebKit; the spec's open is to the first
node drawn): open 1,320 → 657 ms; undo p95 within 44–48 ms (was up to 212 ms).

## 2. Mac (WebKit): the debugger's "node ran" flash makes a big canvas run at 13–20 fps while a Playtest streams

**What's slow** (one400 attached to a Playtest's events, written the way `debug_runtime.pawn` writes them, a batch
every 0.25 s; HEAD build):

| | WebKit frames | WebKit pan while streaming | Chromium frames |
|---|---|---|---|
| 400 events/s (6-node flows) | p50 49 ms, p95 68 ms | frames 231–743 ms | p50 16.7, p95 16.8 |
| 1,500 events/s (Timeline Update chains) | **p50 73 ms, p95 90 ms**, max 903 | 782 ms frames | p50 16.7, p95 16.7, max 50 |
| 1,500/s, `.st-flash { animation: none }` | **p50 17 ms, p95 38 ms** | | |
| 1,500/s, wires on their own layer (#1) | p50 87 ms | | |
| 1,500/s, wire dash animation off | p50 75 ms | | |

**Where the cost goes.**
- Each node that runs remounts a flash: `Nodes.tsx:233`, `<div key={run.at} className="st-flash" />`.
- That flash animates a 22 px `box-shadow` (`studio.css:76-77`), which repaints every frame for 0.9 s.
- With hundreds of nodes running, WebKit repaints continuously.
- In Chromium, the same stream keeps the main thread 28% busy: 2.2 s of 10 s in style, layout and paint, and 0.16 s
  in `applyEvents`. JS is not the problem.

**Who it affects.** Mode makers debugging bigger modes on a Mac. Small modes are fine: the live 22-node sim mode
streamed 364 events/s at p95 18 ms in WebKit.

**Fix idea.**
- Animate only `opacity` of a pre-drawn glow, which the compositor handles.
- Re-key a node's flash at most every 0.5 s.
- Skip it for nodes outside the view.
- **QW**, S.

**Fixed (X2, 38949bf8).** The glow is drawn once (static `box-shadow` on `.st-flash`) and only its `opacity`
animates, which the compositor handles; a node's flash starts again at most twice a second (`key` on the half
second). Nodes out of view aren't drawn (React Flow's culling), so they have no flash. Measured with a new
studio-perf case (every_node, ~1,500 node runs a second through a routed `/debug/events`): WebKit frames p50/p95
25/33 → 16/24 ms; Chromium 17/17 both.

## 3. Dragging many selected nodes re-renders every one of them on every step

**What's slow** (one400, Ctrl+A then drag, opening view):

| Build | Per step | Frames |
|---|---|---|
| Chromium production | 89 ms | p95 67 ms, max 83 ms |
| Chromium dev | 134 ms | p95 117 ms |
| WebKit production | 813 ms (148 ms with #1's fix) | |
| 952 nodes, Chromium dev | 280 ms | |
| 952 nodes, WebKit dev | 4.5 s | |

A single node drags fine:
- Chromium: 17 ms per step at zoom 1, 26–34 ms zoomed out;
- WebKit: p50 18–20 ms.

**Where the cost goes** (CPU profile of the drag, dev):
- `LogicNode` (`Nodes.tsx:91`) and `Pin` (`Nodes.tsx:57`) render for every moved node, every step;
- the wire memo (`Canvas.tsx:204-207`) calls `realEnds` and `outType` for every wire, every step;
- React Flow mounts and unmounts nodes crossing the culling edge (`setInitialProperties`, `removeChild`).
- `live(moveItems)` commits each step to the store, so the whole tree reconciles.

**Who it affects.** Anyone moving a section of a big graph: tidying, Align, select-all.

**Fix ideas.**
- While a multi-node drag runs, let React Flow own the positions and commit once on drag stop.
- Make `LogicNode` independent of position: wrap it in `memo` with data that doesn't change on a move.
- Skip rebuilding wires when only positions changed.
- M.

**Fixed (X2, 38949bf8).** A drag's positions stay in the Canvas (`dragPos`) and go into the graph once at the drop
(`endDrag`: one store commit, one undo step; also for a selection-box drag, whose stop wasn't handled before).
`LogicNode`, `RerouteNode` and `CommentNode` memo on id and picked only (React Flow's position props change every
step). Found on the way: every new node object we gave React Flow lacked `measured`, so React Flow treated a moved
node as new and measured it (and its wire ends) again every step; sizes from its dimension changes are now handed
back. Measured (every_node, all picked, 40-step drag, dev UI): Chromium 89/99 → 18/18 ms a step (zoom 1 / zoomed
out; 18 ms is the input rate), frame p95 83 → 17 ms; WebKit 112/114 → 5/10 ms a step, frame p95 183/187 → 37/21 ms.

## 4. The Studio's drag perf test measures nothing

`tests/studio-perf.spec.ts` keeps its frame-loop flag in `window.__on`. d3 (inside React Flow) stores its event
listeners on the same property of `window`. When a drag or pan starts, d3-drag and d3-zoom add listeners to `window`
and throw `TypeError: on.push is not a function`, so nothing moves.

- I reproduced it: every drag after the first frame sampling threw, and the node stayed where it was.
- Renaming the flag made the drags real.
- So PerfAudit #4's "after" numbers in X.md (3 ms per drag step) measured a drag that did nothing. The real numbers
  are in #3 and in "Measured and fine".

**Who it affects.** The regression guard for #3 and PerfAudit #4.

**Fix.** Rename the flag (I used `__paOn`) and assert the dragged node's `transform` changed. **QW**, XS.

**Fixed (X2, 38949bf8).** `__lsPerfOn` / `__lsPerfF`; each drag checks its node moved (and the first six picked
ones for a drag of everything); the view is set with the Studio's `__studioZoom` dev hook so the dragged node is
really under the mouse; new rows: open, undo, a drag of everything picked, and the debugger stream (#2). The
testdata path is read relative to the spec file, so it runs from any folder.

## 5. Playtest builds: per-player Timelines and For Each Creature cost 7–14× more, and the events file grows about 160 MB an hour

**What's slow** (the sim mode, 20 bots, 90 creatures, micro-benchmark; times per call):

| Hot pattern | Release | Playtest (debug) |
|---|---|---|
| Timeline played per player: one tick of all 32 copies | 73 µs | **997 µs** |
| For Each Creature, 90 alive (12 alive) | 270 µs (140 µs) | **2,003 µs** |
| Player Hurt's chain (Get Row + Set + Set vector) | 11.6 µs | 91 µs |
| Player Hurt hook, whole | 15.5 µs | 96.5 µs |
| One debugger event (`lsd_emitf`) | – | 7.2 µs (was 181 µs before PerfAudit #5's fix) |
| Flushing a full 24 KB buffer | – | 120 µs |
| `lsd_poll` with no command waiting | – | 21 µs every 0.25 s |
| A live tweak (`set`, then `lsd_vars` for 20 players) | – | 1.26 ms per click |

**What that means in a Playtest.**
- With 32 players, the Timeline costs about 20 ms of server CPU a second at 20 ticks a second, in 1 ms bursts.
- The events file grew 44 KB/s (364 events/s) with 20 bots: 2.6 MB a minute.
- The file is only emptied at map start. In about 6 minutes with the benchmark it reached 77 MB.
- The page reads it in 512 KB / 3,000-event pieces, so it keeps up. The size is a disk issue, not a page issue.

**Where the cost goes.**
- Every tick of every copy emits enter and exit for each Update node with full player JSON (`lsd_in_player`): about
  12k `gset` events in 2 MB of the log.
- The Timeline's own exit is already rate-limited to 4 Hz per copy (`gen.rs` `timeline_code`). Its Update chain
  isn't.
- Dead players' Timelines keep ticking: events from `alive:false, hp 0` players were seen.

**Who it affects.** Mode makers playtesting with many players or creatures; the Playtest panel already says debug
builds aren't for judging performance. Players on real servers aren't affected.

**Fix ideas.**
- In debug builds, emit a Timeline's Update chain at most 4 Hz per copy, as its exit already is. S.
- Rotate or cap the `.events` file (for example at 20 MB, starting again; the page already handles `reset`). S.
- Optional: stop per-player Timeline copies when that player dies. S.

**Fixed (X, 0ad5fff2 and ec184f06).**
- Sampling: each node writes at most 4 runs a second. The rest are counted, and the next run written carries the count
  (`"skipped"`), so the page's run counts and Test run's stay right. A node's inputs are only turned into JSON for a run
  that's written. A Timeline's 4 Hz progress note shares its node's budget; Finished is always written.
- Nobody watching: the page's event reads keep `<mode>.seen` fresh (how far it has read). If it hasn't changed for
  15 s, nothing but breakpoints is written. With no `.seen` (Test run, or before the page's first read), runs are
  written, sampled. A Playtest start removes an old one.
- Rotation: the file starts over once the page has read all of it past 8 MB, or at 32 MB regardless. The page sees it
  shrink and reads from the top (its usual reset).
- Measured with this audit's bench (`pa_bench`, the same sim graph, 20 bots, 12 creatures alive; before = tools at
  92a639ec, after = ec184f06; same server, one plugin swapped at a map change):

| Hot pattern, debug build | Before | After | Release (for scale) |
|---|---|---|---|
| Timeline, one tick of 32 copies | 933 µs | **87–97 µs** | 77–80 µs |
| For Each Creature (12 alive) | 387 µs | **140 µs** | 137 µs |
| Player Hurt's chain (Get Row + Set + Set vector) | 90 µs | **0.8–1.0 µs** | 0.2 µs (was 11.8) |
| Player Hurt hook, whole | 93 µs | **4.5 µs** | 4.0 µs (was 15.5) |
| Events file growth, normal play | 50 KB/s | **2.6 KB/s** | – |
| Events file growth, nobody watching (stale `.seen`) | 50 KB/s | **0 B/s** | – |

  Rotation, checked with a build whose cap was 200 KB and a "reader" keeping `.seen` at the end: the file grew to
  198 KB, started over (14 KB), and climbed again. Not done: stopping a dead player's Timeline copy (it's now
  sampled like the rest).

## 6. Data tables: Get Row searches the whole table, once per pin read

**What's slow** (release build, 256-row number-key table; per call):

| Key | Cost |
|---|---|
| Key in the first row | 0.05 µs |
| Key in the last row, or missing | **11.6–12.2 µs** |

- A Get Row node with two columns wired searches twice. In big400.sma, Give Weapon's gun and ammo come from one Get
  Row, and the code has `lsg_dt0_find(...)` twice on one line.
- Text keys run `equal()` per row. I didn't measure them, but they will cost more.

**Where the cost goes.** `tables.rs` `pawn()` writes a linear `lsg_dt{t}_find`. The generator (`gen.rs:818-830`)
inlines `lsg_dt{t}_{col}(lsg_dt{t}_find(key))` at every pin read, because pure nodes aren't cached.

**Who it affects.** Players on servers running modes that read tables in hot events. One Get Row on Player Hurt is
fine: about 1–3 ms of CPU a second at hundreds of hits a second. Inside For Each Player on a hit, it is
32 × 2 × 12 µs = 0.75 ms per hit.

**Fix ideas.**
- Find the row once per node run, into a ctx slot (as Find Component and Creatures Alive are now cached). S.
- Number keys that are 0..N-1, as the editor makes them: index directly. S.
- Text keys: a Trie built in `plugin_init`. S.
- Or add a cost warning for Get Row inside loops on hot events. S.

**Fixed (X, 0ad5fff2).** `lsg_dt{t}_find` keeps the last key's row: the tables never change, so it's always right,
and a Get Row with two pins wired searches once. Number keys in strict order (as the editor makes them; 8+ rows)
are found by halving. Text keys in a table of 16+ rows go through a Trie made at the first search. Smaller or
unordered tables are searched row by row, and the first row with the key still wins. Rust tests cover each search,
and the real amxxpc builds them. Bench, release build, 256 rows:

| Get Row | Before | After |
|---|---|---|
| Key in the last row, or missing | 11.6 µs | 0.05 µs (the same key again) |
| A different key every call | 6.0 µs | **0.7–0.8 µs** (halving) |
| Two pins on one key | 12.0 µs | **0.8–0.9 µs** |

## 7. Version history: every save replays the whole history

**What's slow** (backend, big400 = 367 nodes):

| Step | Time |
|---|---|
| Save, first | 9 ms |
| Save after 50 kept versions, one node moved each time | 42 ms |
| Save after 50 kept versions, every node moved each time (Tidy up, Align, a drag of all) | **206 ms**; history file 1.9 MB for a 205 KB graph |
| Versions list | 234–290 ms |
| Opening the oldest version | 80–194 ms |

At 952 nodes:
- saves take 108–283 ms;
- the list takes **1.34 s**;
- opening a version takes 300 ms.

In the page, the Versions panel opened in 114–130 ms with 13 versions, and picking the oldest took 44–71 ms.

**Where the cost goes** (`history.rs`):
- `record_at` reads the file and applies every step to rebuild the newest graph before diffing.
- `diff` uses `Vec::contains` and `iter().any` inside loops over the lists, which is O(n²).
- `apply` finds each changed item with a linear search.
- `list()` rebuilds and clones all 50 versions, and counts "changed" with a nested loop over nodes for each version.

**Who it affects.** Mode makers with big modes who save often. It's in the Save button's wait, and the list is in the
Versions panel.

**Fix ideas.**
- Keep the newest graph in the history file, or in memory, so a save diffs against it without replaying.
- Index the lists by id (`HashMap`) in `diff` and `apply`.
- Store each version's added, removed and changed counts when it's recorded, so `list()` reads them instead of
  rebuilding.
- S–M.

**Fixed (X2, e64863ce).** The history file (version 2) keeps the newest save whole (`head`), so a save diffs against
it with no replay; older saves are the change back from the one after (every tenth whole), with each save's node
counts kept, so `list` reads them and rebuilds nothing; `diff`/`apply` index by id. Past 50 saves, or while the file
is over 4 MB, the oldest go. Version 1 files are read and carried on. `history_bench` (ignored test; 60 saves with
every node moved): 367 nodes save 151 → 29 ms, list 196 → 15 ms, oldest 133 → 24 ms; 952 nodes 738 → 39, 1,099 →
21, 643 → 30 ms, file 7.5 → 4.0 MB (26 kept). The Versions panel lists and opens through the same routes.

---

## Measured and fine

**Editor UI** (one400 unless noted; Chromium production / WebKit production):

| What | Measured |
|---|---|
| Pan, everything in view (174 drawn) | Chromium p50 16.7, p95 16.7 ms. WebKit: see #1 |
| Pan, zoom 1 (11 drawn) | Chromium 16.7 / 16.8 ms. WebKit p50 16–17, p95 20 ms |
| Wheel zoom | Chromium p95 16.7 ms. WebKit p50 17 ms, p95 18–56 ms |
| Drag one node, zoom 1 | Chromium 17 ms per step. WebKit p50 18 ms |
| Drag one node, zoomed out | Chromium 26–34 ms per step. WebKit p50 19–20 ms |
| Event graph tab switch (big400, 16 graphs) | 31 ms |
| big400 open (16 graphs) | 176 ms (Chromium), 291 ms (WebKit) |
| Palette open (right click) | Chromium 27–42 ms. WebKit 42 ms |
| Palette search, per keystroke, all 449 nodes | p50 32 ms, p95 43–61 ms, in both browsers |
| Palette rows | "e" matches 495 rows (the list isn't virtualized, and doesn't need to be yet); "give" 28; "creature" 40 |
| Quick check request | 6–7 ms (p95 24 ms), 94 KB sent |
| Quick check, edit to answer (300 ms debounce included) | 376–465 ms |
| Quick check, 952 nodes | 15 ms, 226 KB |
| Undo / redo | Chromium 32–49 ms (952 nodes: 65 ms). WebKit 65–105 ms (see #1) |
| Memory over 300 edits (each an undo step) | Chromium heap 20 → 22 MB, RSS 263 → 288 MB. WebKit RSS 627 → 687 MB. 952 nodes: heap 28 → 29 MB. No leak seen |
| Data table (256 rows × 6 columns, 16.6k DOM elements, 6,942 `<option>`s for the weapon column) | open 163 ms (Chromium), 173–238 ms (WebKit, when #1 doesn't hit) |
| Data table edits | cell commit 22–36 ms; weapon pick 44–70 ms; typing 31 ms per key |
| Save (Ctrl+S) in the page | 16–18 ms request with a short history |

**Backend**

| What | Measured |
|---|---|
| Validate | small graph 2–4 ms, big400 6 ms (p95 24), 952 nodes 8 ms |
| Generate | big400 8.1 ms (116 KB of source), debug 8.4 ms (173 KB) |
| Compile (full check, real amxxpc under CrossOver) | small 1.4 s, sim mode 1.5 s, big400 1.9 s, every_node 1.95 s |
| Install compile | release 1.9 s, Playtest build 2.5 s; an unchanged graph 32 ms (cached by hash). Mostly Wine start-up, as before |
| Graph file (pretty JSON) | 204 KB for 367 nodes, 470 KB for 952; saves outside game folders keep no backup copy |
| Debugger poll (`/debug/events`) | 1.8–3 ms each; up to 3,000 events / 350 KB per poll at 1,500 events/s |

**Debugger, live.** The 22-node sim mode attached to the running Playtest build (364 events/s): frames p95 16.7 ms
(Chromium) and 18 ms (WebKit). The latest-path list grew to 400–900 nodes between Player Hurt events, because
Timeline ticks never start a new path. That was harmless at this size.

**In game, release build** (per call, micro-benchmark):

| Pattern | Measured |
|---|---|
| Timeline, 32 copies | 73 µs per pass (2.3 µs per copy): about 1.5 ms of CPU a second at 20 ticks a second. It also works out tracks nothing reads (the unused Tint color) |
| For Each Creature from a 1 s timer | 270 µs per second with 90 creatures. The cost warning for it in hot events already exists |
| Player Hurt hit position | capture (TraceAttack pre hook) 0.05 µs; Hit Position read 0.15–0.2 µs; the whole hook without the table 4 µs |
| Mode settings at each new round | load and apply 30 µs with no settings file, 140–188 µs with one; once a round |

**A/B with `amxx pause`.** It was within noise, in both directions:

| Build | Running | Paused |
|---|---|---|
| Release | 50.9 fps | 55.1 fps (more creatures alive while paused: 27 against 19) |
| Debug | 50.4 fps | 30.9 fps |

Bots take 15–18 ms a frame, so this Mac can't resolve sub-millisecond plugin costs. The micro-benchmark numbers
above are the reliable ones.

**Bundle.** The StudioPage chunk is 1,008 KB minified / 231 KB gzipped (693 KB at PerfAudit). It includes
nodes.json (300 KB). Node help (456 KB) is already lazy. This is worth a look, but it isn't hurting anything
measured.

## Not covered

- **1,000+ nodes as a real mode:** the 400-node limit refuses it. The 952-node rows are headroom with the limits
  raised in the dev page only.
- **The real Mac app (WKWebView) and Windows (WebView2):** Playwright's WebKit and Chromium stood in. Check #1's fix
  for sharp wires in the real Mac app.
- **32 real players:** I ran 20 bots. 32 Timeline copies were timed through the benchmark.
- **Text-key tables:** not benchmarked.
- **A quiet machine:** in-game absolute fps.
- **Mode settings on a remote server.**
- **Dev-UI numbers:** they ran against a working tree other agents were editing (hot reloads), so the production
  rows are the ones to trust.

Throwaway scripts are in my scratchpad and are not in the repo:
- `ui.mjs`, `dbg.mjs`, `backend.mjs`, `prof.mjs`, `writer.py`;
- `gen_big.py`, `gen_sim.py`, `ab.sh`, `simstart.sh`;
- the `pa_bench` harness (`bench_*.inc`).

Sim copy `Sim/SPA` is deleted, and my dev servers are stopped.
