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

## Left / ideas
- Extrude in the flat views (Shift is box-pick there), edge-move mode for slanted edges.
- Entities drawn one by one dominate the flat panes on big maps (request to P5).

## Testing notes
- My dev servers: Rust on 8093 (started as `... devserve::tests::serve_tools_de` so a `pkill -f serve_tools_dev`
  doesn't hit it), Vite on 5393 with HMR off (other agents' edits reloaded pages mid-test).
- WebKit reports fetches cut off by a reload as "access control checks" page errors: the specs ignore those.
