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
