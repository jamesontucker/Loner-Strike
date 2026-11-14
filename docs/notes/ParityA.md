# Parity A notes (map editor: 2D views, primitives, extrude and vertex editing)

## Plan (2026-11-13)
What already exists (checked in the code): Top/Front/Side panes with grid, snapping, box-pick, box/shape create
and gizmo move; layouts Single, Quad, 1+3 and Two with saved-layout migration (`panes.ts`); shapes cylinder,
wedge, spike, sphere and arch (`editor/shapes.rs`); push/pull a face; a vertex tool with corner, edge and face
handles. So I only build the gaps:

1. **2D views:** 8 resize handles plus drag-inside-to-move on the selection box in Top/Front/Side
   (`FlatHandles.tsx`, my file); zoom to the cursor; a grid that follows the zoom and the grid size and draws
   in one call (`FlatGrid.tsx`, my file). Hookups in `EditorScene.tsx` are one line each.
2. **Primitives:** torus, cone (point or cut-off top), arch and torus add-height (spiral), in `editor/shapes.rs`
   with tests; a Shape settings panel with a preview before placing (`ShapeParams.tsx`, my file).
   P5 owns the drag ghost fix (`shapePreview.ts`); I'll ask for or add the new shapes' outlines there once it's committed.
3. **Extrude:** Shift-drag a face of a picked brush pulls out a new brush (`editor/extrude.rs`, `extrude_face`
   command); vertex tool: split an edge or face (Ctrl-drag its middle handle adds a corner) and merge corners.

## Done
**1. 2D views** (e8f90dda, tests 6a156c8b): `FlatHandles.tsx` (8 handles stretch the selection, the moved
edge on the grid; dragging a picked brush in a flat view moves it, corner on the grid; live preview; one undo step),
`FlatGrid.tsx` (one draw call; lines every grid step, doubling while closer than 6 px; 64- and 1024-unit lines; axes),
`zoomToCursor` on the flat views' MapControls. Tests: `tests/editor-flat.spec.ts`, `tests/editor-flatperf.spec.ts`.
- br_valley in Quad, Top view (Playwright, 1440×900): WebKit 31 fps panning, 108 fps zooming; headless Chromium
  (software drawing) 4 / 9. A flat pane on br_valley is ~1200 draws: brushes are ~3 (merged), the rest are entities
  one by one (~480 outlines, ~440 Lambert boxes, ~150 basic meshes, ~100 fat lines). Request to P5 below.
- Every pane redraws on any change (one canvas), so panning one flat view also redraws the 3D pane.

**2. Primitives** (d80291a0): cone (Top %: a point or cut off; the top ring is the base scaled toward the point so
sides stay flat), torus (pieces, tube sides, tube width, arc, start, add height; neighbors share their end exactly),
arch add height (spiral stairs). Shape tool bar fields + **Preview…** popover with every setting and a turning
wireframe (`ShapeSettings.tsx`, settings in `shapeParams.ts`, `ls.editor.shapeParams`). Tests: shapes.rs units
(validity, convexity, shared ends, grid), the command test, `tests/editor-shapes.spec.ts` (Chromium + WebKit).

**3. Extrude and vertex editing** (a0c5f385): Shift-drag a face of a picked brush in 3D pulls out a new brush
(`extrude_face`; `editor/extrude.rs`: a prism over the face, ends in the face's texture, sides continue the source's
faces; the far side on the grid; a drag is one "Extrude" step and the new brush is picked; dragging back onto the
face removes it). Vertex tool: ⌘/Ctrl-drag an edge's or face's middle splits it (`add_vertex`: a new corner, refused
when it would dent the brush, lie flat or swallow a corner); **Merge corners** joins the picked corners of a brush
at their middle (`merge_vertices`); dropping a corner on its neighbor already merged (tested). UI in `extrude.ts`
and `VertexActions.tsx`. Tests: extrude.rs and vertex.rs units (convexity, volumes, textures, refusals),
`extrude_split_and_merge` (the commands, one undo step per drag), `tests/editor-extrude.spec.ts` (Chromium + WebKit).

**4. Big-map drawing (from the coordinator)**: point entities batched (`EntityBatch.tsx`: one instanced box mesh
with a colour per entity, one set of outlines, one of facing arrows, sprite glows as one point cloud, one instanced
mesh per studio model+body+skin; picking kept: each instance hits as a stand-in object with `{kind: "entity", id}`, so
hitOf/extras/Marquee are unchanged); target lines in one dashed draw; P4's component points and the Checks dots via
`InstancedMarkers`. Per-pane redraws (`paneRedraw.ts`): each pane's portal has its own invalidate (camera moves, the
cross-hair, a ghost mark only that pane); store changes and the canvas's invalidate mark every pane; a catch-up redraw of
all panes 250 ms after pane-only changes; `preserveDrawingBuffer` keeps panes that aren't redrawn. While the camera is
dragged (middle/right button) a pane stops its hover raycasts. br_valley Quad, Top view (editor-flatperf.spec.ts):

| | before | after |
|---|---|---|
| draws per pane (Top) | ~1200 | ~46 (brushes 3, entities ~6, overlays) |
| WebKit pan / zoom | 31 / 108 fps | 55 / 122 fps (4.4 ms per pane render; the pan figure is page frames under Playwright's synthetic mouse moves) |
| Chromium (software) pan / zoom | 4 / 9 fps | 22-28 / 50-54 fps |

Editor suites: Chromium 44 passed (P5's editor-perf failed once at load 38, passes alone), WebKit 45 passed.

**5. Shear (from the coordinator)**: in Top/Front/Side, ⌘/Ctrl-drag an edge's middle handle: that edge slides along
itself by whole grid steps, the opposite edge stays (a box becomes a slanted wall), live preview, one "Shear" step
(`shear` command, `editor/shear.rs`). Affine, so brushes stay convex with the same volume; steeper than 8:1, the same
axis twice or a bad number is refused with a reason. Texture lock and scale lock apply. Tests: shear.rs units (every
axis pair on a spike, a box's corners on whole units), the command test, `editor-flat.spec.ts` (Chromium + WebKit).

**6. Editor rendering, PerfAudit #1-#3 (from the coordinator)**: `brushBuffers.ts` + `SharedBrushes.tsx`.
- One set of brush buffers for the whole map, shared by all panes: a triangle pool per texture and outline pools
  (world / brush-entity / sky-only). Each brush owns its ranges for good; an edit rewrites only touched brushes in
  place (BufferAttribute update ranges; a pool switches to one whole upload past 256 ranges); picking, hiding and the
  cordon fill a brush's range with NaN and refill it later. A pool only regrows (new geometry, old disposed) when full.
- Selection is an overlay (one mesh per texture + one outline draw from cached per-brush edges), disposed 1 s after
  it's replaced (disposing at once let a pane re-upload it: that was a leak). One picker object per pane (box test,
  then pickRay's brush raycast through a stand-in mesh) replaces 5,400 invisible meshes per pane.
- Box-select highlight: one shared mesh (`sharedTriangles`) instead of a mesh per brush, which had uploaded every
  touched brush's geometry and kept it (3,882 GPU geometries after a deselect -> 156).
- Picked point entities through EntityBatch (picked style); hot point entities through InstancedMarkers.
- Model geometry/materials cached per model+body / model+skin for the session (no texture reloads on edits);
  EntityBatch keyed on entity content, materials shared (no program drop/recompile), InstancedMesh.dispose() kept.
- MergedBrushes.tsx (P5's) is no longer used; EntityBox removed from EditorScene (PickedEntities batches).

Numbers, br_valley Quad, WebKit, `tests/editor-bigedit.spec.ts` (load 12-18; prod = vite build + preview):

| | before (audit prod / my dev) | after prod | after dev |
|---|---|---|---|
| click select, worst frame | 291 / 176 ms | 45-46 ms | 53 ms |
| nudge+undo x10, frame p95 | 177 / 213 ms | 30-31 ms (p50 17) | 61 ms |
| box select ~3200 brushes | 1.4 s / 1245 ms (1961) | 112 ms (3214) | 115-151 ms |
| deselect | - / 1750 ms | 43-47 ms | 61-67 ms |
| 10 select/deselect cycles | +300 MB RSS each (audit) | - | geometries 235 -> 157, textures 70 -> 70 |

Chromium (software GL): JS heap 79 -> 77 MB over 10 cycles (after GC), geometries flat; frame times there are
~450-550 ms per redraw of the 4 textured panes and not meaningful. Chromium editor suite: 51 passed, 1 skipped.

## Where I am (paused 2026-11-13 ~19:35 for the usage limit)
- Committed: the rendering work above. Tested: Chromium full editor suite (51 passed), WebKit bigedit perf spec.
- **Next:** re-run the WebKit editor suite (`-c <scratch>/pw-pa-webkit-all.config.ts`: testDir app/ui/tests,
  testMatch editor-*.spec.ts, browserName webkit) - it was stopped mid-run with no failures so far.
- Nudge p95 is 30 ms (target 16): the rest is the 654 KB doc refresh and React re-rendering doc readers. P5 has
  document deltas (`applyChanges`/`catchUp`, checks debounce) in progress in store.ts/editorApi.ts/tools/editor.rs;
  re-measure with editor-bigedit.spec.ts once that lands. A little shader-program churn remains per edit (drei Line
  programs released/recompiled; not from brush/entity drawing) - worth a look after that.
- Always: Playwright under `nice -n 15 --workers=1`, only while load < 20 (coordinator).

**7. Where a slow nudge / drag-move frame goes (2026-11-13 22:30, before further fixes)**
`tests/editor-framesplit.spec.ts` instruments the page from outside (production build): render loop (R3F + three.js,
with the time inside WebGL calls: draw / upload / texture / shader), React (its scheduler tasks), store (the app's
promise continuations after a response: meshes, the document patch, listeners), input handlers, timers; fetch is
elapsed time (waiting, not CPU). "Unattributed" = the rest of the frame: GC, style/layout/paint, compositing, the
GPU process, idle. GPU: WebKit has no EXT_disjoint_timer_query, so a second pass calls gl.finish() after each render
callback (the GPU's time to finish the frame's draws); WebKit clamps timers to 1 ms. br_valley, Quad, 1440x900.

| run (load) | case | frame p95 / worst | render loop (WebGL in it) | React | store | input | unattributed | gl.finish |
|---|---|---|---|---|---|---|---|---|
| WebKit, Apple GPU (19) | nudge+undo | 29 / 40 ms | 6 / 2 (<1) | 0 / 9 | <1 | 0 | 23 / 29 | - |
| WebKit (17) | nudge+undo, finish pass | 28 / 34 ms | 2 / 2 (<1) | 10 / 8 | <1 | 0 | 16 / 24 | <1 ms |
| WebKit (19) | drag-move | 25 / 33 ms | 1 / 2 | 0 / 1 | 0 | 4 / 4 | 20 / 26 | - |
| WebKit (17) | drag-move, finish pass | 25 / 32 ms | 0 / 1 | 0 / 1 | 0 | 2 / 2 | 23 / 28 | <1 ms |
| Chromium, ANGLE Metal M3 Max (16) | nudge+undo | 16.8 / 16.8 ms | - | - | - | - | - | timer query available |
| Chromium, same GPU (16) | drag-move | 16.7 / 16.8 ms | - | - | - | - | - | |
| Chromium, SwiftShader (17) | nudge+undo | 250 / 350 ms | 4 | 0-3 | - | 0 | 245 / 343 | software GL: not representative |

Averages over WebKit's slow frames (>17.5 ms, about 100 of 360 in a nudge run): render loop 1.6 ms, React 3.3 ms,
store 0.6 ms, input 0.1 ms: **our JS is about 5-6 ms of a 22-23 ms frame**; the GPU finishes each frame's draws in
under 1 ms. Chromium on the same Mac's GPU (the WebView2 proxy) holds 60 fps through the same edits: br_valley perf
spec there = nudge+undo p95 17 / max 17 ms, click 22 ms, 3215-brush box select 56 ms, deselect 17 ms.
**Reading:** neither our bookkeeping nor GPU draw time is the limit; the extra 6-12 ms per slow frame happens in
WebKit outside page script (compositing / presenting the WebGL canvas, GC, layout). Next to test (not done yet):
preserveDrawingBuffer (WebKit copies the drawing buffer each frame to keep it), the 4 per-frame
getBoundingClientRect calls in PaneView (forced layout after DOM changes), and GC from per-edit allocations; and the
real app's WKWebView rather than Playwright's headless WebKit. WebGPU would not help a CPU/compositor-side cost.
Load: these ran at 15-19 (sims); a load-under-5 run is queued.

**8. Per-edit work removed, and what was tried (2026-11-13 23:00)**
Kept (measured, suites green):
- Outliner rows: each row's element cached by what it shows (row object identity with P5's deltas, picked,
  hidden, locked, open, name, type, texture); handlers through a ref, so a cached row never calls stale code.
- Content Browser memoized: the dock re-rendered it and its thumbnail grid on every edit.
- `StableLine.tsx`: drei's Line (9.117.3) disposes a line's material whenever `points` is a new array; the
  component markers and links got new arrays every edit, so ~150 line materials were disposed per edit and three
  dropped and recompiled their shader programs. Same component, material disposed only on unmount; the editor's
  `Line` imports point at it. Per edit: 0 disposed (was ~190).
- `editor-flat.spec.ts`: waits for the handles' dev hook (registered in an effect after the commit that shows the
  selection) instead of reading it at once; that race was P5's "Front and Side" WebKit failure.
Tried and reverted (no measurable effect, so not kept): a per-frame drawing budget (focused pane first, others a
frame later); preserveDrawingBuffer off with every pane drawn each frame; pane/canvas rects cached with
ResizeObserver instead of read per frame.

Results, br_valley Quad, production build:

| engine | nudge+undo p95 / max | drag-move p95 | click | box select 3214 |
|---|---|---|---|---|
| Chromium on the GPU (WebView2 proxy), before this round | 17 / 17 ms | 16.7 ms | 22 ms | 56 ms |
| Chromium on the GPU, after | 17 / 17 ms | 16.7 ms | 23 ms | 67 ms (load 19) |
| WebKit (Playwright, Apple GPU), start of the round | 33 ms | - | 52 ms | 102 ms |
| WebKit, after | 25-29 / 35-37 ms | 24-25 ms | 40-48 ms | 121-145 ms |

Mac: stops here per the user's priority (Windows first). WebKit's slow frames hold 5-6 ms of our JS and <1 ms of GPU
draws; React commits force 2-11 ms of style/layout (3-5 commits per edit frame); a one-box map still shows nudge
p95 20 / worst 27 ms, so part of it is per-edit DOM work, not map size. Next if Mac matters: fewer React commits
per edit (batch the cmd -> meshes -> doc -> checks updates into one), the per-undo toast's layout, and measuring in
the app's own WKWebView (not done: running the release app would start its background jobs on the user's data).
Suites: Chromium 52 passed / 1 skipped (+ editor-flat fixed, 3/3 twice); WebKit 53 passed / 1 skipped.

**9. Mac per-edit cost, round 2 (2026-11-14)** - kept: the editor's top component reads only the document id (it read
the whole document and the busy count, so every edit re-rendered the whole editor); an edit's meshes, document patch,
note and busy count land in one synchronous step (one React commit; `syncMeshes(after)`); the toast stack has
`contain: layout style` (it already animated transform/opacity only). React time in WebKit's p95 nudge frame 10 -> 3 ms.
Frame times (production build, p95 nudge+undo / drag-move): WebKit br_valley 28-29 / 24 -> 29 / 26 ms; WebKit one box
20 / 20 -> 19 / 20 ms; GPU Chromium 16.7 / 16.8 both maps, before and after. WebKit's remaining frame cost follows the
canvas size (one box: 960x600 p95 18, 1440x900 19-20, 1920x1200 20-21 ms) and is the same in a real WKWebView window
(a minimal host, GPU composited: nudge p95 26 / worst 37, drag 21 / 31 ms), so it's WebKit presenting the canvas,
not our work. Stopped there (Windows first).

**10. Wine incident, 2026-11-14 00:50 (my first QA4Z start; W1's three sims went silent)**
Timeline from process start times: 00:50:21 I started a fresh Valve CZ server (Sim/QA4Z, straight from SteamCMD)
without `-insecure` and without `steam_appid.txt`; 00:50:34 it died with "FATAL ERROR: Unable to initialize Steam" (its
wine wrapper stayed behind, PID 5004, since stopped); 00:50:46 the Steam bottle's wineserver started again from nothing
(with services.exe, explorer.exe, plugplay, winedevice at 00:50:47-49), so the one every sim was attached to had gone;
W1's sims stopped answering within that minute. The likely cause is the Steam API in a server started without
`-insecure` reaching for the bottle's Steam client and taking the wine session down when that fails; I can't prove the
last step (that run's logs were overwritten). **Crew rule I'm following now: never start a Valve server in the shared
bottle without `-insecure` (and a `steam_appid.txt`, as LSPLAY clones have).** The app's hidden server is not this path:
navs.rs `learn_args` always passes `-insecure`, which is why CS 1.6 learning has run without a `steam_appid.txt`.

## Left / ideas
- Extrude in the flat views (Shift is box-pick there), edge-move mode for slanted edges.
- Entities drawn one by one dominate the flat panes on big maps (request to P5).

## Testing notes
- My dev servers: Rust on 8093 (started as `... devserve::tests::serve_tools_de` so a `pkill -f serve_tools_dev`
  doesn't hit it), Vite on 5393 with HMR off (other agents' edits reloaded pages mid-test).
- WebKit reports fetches cut off by a reload as "access control checks" page errors: the specs ignore those.

## Note (2026-11-13)
2b9ddff1 (the MCP commit) undid 4f578685 (big-map drawing): it deleted EntityBatch.tsx and paneRedraw.ts and reverted
the hunks in EditorScene, PaneView, EntityModels, P4Scene, CheckMarkers, editor-flatperf.spec.ts and P5.md, most likely
committed from an index made before 4f578685. Restored as it was in the next commit. Whoever commits from a separate
index: build it from the current HEAD (git read-tree HEAD) right before committing.

## From P5 (2026-11-13): document changes per command (perf audit #1d, #7), the shape
So your per-brush dirty rendering and my document patches line up. Nothing changes in the meshes route
(`meshes?doc=&since=` still sends changed brushes + `removed`); what's new is about the *document* (`useEditor.doc`):
- Every `cmd`, `cmd2`, `undo`, `redo` and `history/jump` answer has `changes` = `{docId, from, revision, brushes:
  DocBrush[], removedBrushes: number[], entities: DocEntity[], removedEntities: number[], groups, world, editor,
  bounds, stats}` + the usual state fields. `from` is the revision it's relative to. `GET doc?doc=<id>&since=<rev>`
  gives the same with `"changes": true` (or the whole document when it's too far back).
- `store.applyChanges(c)` patches `doc` in place: **unchanged DocBrush/DocEntity objects keep their identity**; only
  changed ones are new objects, removed ones drop out, added ones are inserted in id order. The `brushes` and
  `entities` arrays are new arrays only when something in them changed. So key memos on the objects (or on
  `doc.revision` + ids), not on array identity: a nudge of one brush gives a new `doc.brushes` array whose other
  5400 elements are the same objects, and the same `doc.entities` array.
- `doc.revision` now means "the revision of the content the page holds": `mergeState` no longer moves it (nor
  `docId`); only `applyChanges`, `catchUp` (doc?since) or a whole document do. If your code read `doc.revision` right
  after a command to mean "the server's revision", use the command answer's `state.revision`.
- Server side, history notes touched brushes/entities on the document (`MapDocument::touched`, `take_touched`), so
  `Session::track` no longer clones and compares the whole brush map; `Session.bounds` keeps each brush's bounds.
Shout in P5.md if you need another field (e.g. per-brush face counts) in the change set.

## Regression sweep (2026-11-14, `tools` ced20c90 in a clean worktree)
| Suite | Result |
|---|---|
| Rust app (`cargo test`) | 894 passed, 0 failed, 24 ignored |
| Rust ls-control (app/server-agent) | 36 passed |
| Rust voice-agent / intent-agent | 6 passed / no tests |
| List service (services/serverlist, unittest) | 230 tests OK (1 skipped) |
| community-publisher (node --test) | 12 passed |
| UI unit (scripts/test-*.mjs: cover, dock, frametime, roles, selection, studio, windows) | 66 passed, 0 failed; tsc -b and the US English check pass |
| amxxpc, every .sma in git (178: plugins, mods, Studio examples) | 178 compile; warnings only in third-party mods (and DeathrunManager's deprecated client_disconnect) |
| Browser specs, Chromium large + small and WebKit (435) | first pass 390 passed, 24 failed, 21 skipped; after reruns 7 repeatable |

Repeatable browser failures, with owners (notes in their files): `tools.spec.ts:124` focus mode editor, all projects
(an unsaved map left by an earlier spec makes "New map" ask; 3fc433ad, P5.md); `editor-batch.spec.ts:257` small
(6fa5807a, P5.md); `editor-content.spec.ts:34`, `:76` small (timeouts at 1024x680; 9a28bb66, P5.md);
`studio-v3.spec.ts:550` WebKit ("Add graph" doesn't select the new tab; X.md).
Mine, fixed: `editor-extrude`, `editor-flat`, `editor-shapes` failed in the small project (2D panes ~120 px tall at
1024x680); they now pin 1440x900 (0ccbc425). Also fixed while doing the merged update bar (38459fdb): every Sheet
closed itself at once in dev (React's double mount: the first mount's own Back landed after the second mount's
history entry); `closeSheetsAbove` now ignores a Back a sheet started itself.
Flaky (once, then passed): `studio-v3.spec.ts:153` (a leftover "pw_one"), `studio-help.spec.ts:91` WebKit,
`studio-playtest.spec.ts:15` small and WebKit (two "Compiling" statuses at once).
Not failures (the sweep's setup, passing once fixed): `studio-perf.spec.ts:55` and `studio-v3.spec.ts:325` read a
testdata path relative to the working folder (run from app/ui); `tools.spec.ts:308` Plugin Studio got a 403 on
Monaco's codicon font because the sweep's node_modules was a symlink outside Vite's `fs.allow`.
