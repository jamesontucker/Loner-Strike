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

**3. Extrude and vertex editing**: Shift-drag a face of a picked brush in 3D pulls out a new brush (;
: a prism over the face, ends in the face's texture, sides continue the source's faces; the far
side on the grid; a drag is one "Extrude" step and the new brush is picked; dragging back onto the face removes it).
Vertex tool: ⌘/Ctrl-drag an edge's or face's middle splits it (: a new corner, refused when it would
dent the brush, lie flat or swallow a corner), **Merge corners** joins the picked corners of a brush at their middle
(); dropping a corner on its neighbor already merged (tested). , .
Tests: extrude.rs and vertex.rs units (convexity, volumes, textures, refusals),  (commands,
one undo step per drag),  (Chromium + WebKit).

## Left / ideas
- Extrude in the flat views (Shift is box-pick there), shear in 2D, edge-move mode for slanted edges.
- Entities drawn one by one dominate the flat panes on big maps (request to P5).

## Testing notes
- My dev servers: Rust on 8093 (started as `... devserve::tests::serve_tools_de` so a `pkill -f serve_tools_dev`
  doesn't hit it), Vite on 5393 with HMR off (other agents' edits reloaded pages mid-test).
- WebKit reports fetches cut off by a reload as "access control checks" page errors: the specs ignore those.
