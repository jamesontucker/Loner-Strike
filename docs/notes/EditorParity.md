# Map editor parity: Hammer 3.5, J.A.C.K., TrenchBroom

P5, 2026-11-13. Checked in the code on branch `tools` (paths are under `app/ui/src/views/tools/editor/`
and `app/src-tauri/src/`), not from memory. **Have** = works like the others or better. **Partial** = some of
it. **Missing** = not there. Value = how much mappers would feel it (H/M/L). Effort = S (a day or less),
M (a few days), L (a week or more).

| Feature | Hammer 3.5 / J.A.C.K. / TrenchBroom | Loner-Strike now | Status | Value | Effort |
|---|---|---|---|---|---|
| Pointfile / leak tracing | Load .pts/.lin, draw the line, step along it | `tools/compile.rs` reads the `.lin` after a leaking compile; `LeakLine.tsx` draws it through walls in every pane (3D and 2D); the Compile panel shows Show/Hide and "go to each end" buttons on the leak problem | Partial: no fly-along camera, no "brush near the hole", the line only shows from the Compile panel's problem row | H | S-M |
| Face edit sheet / UV | Hammer's Face Edit dialog: shift, scale, rotation, justify, fit, world/face align, treat as one; Alt+right-click applies texture with alignment | `FaceSheet.tsx`: shift, scale, rotation fields, nudge buttons, justify, fit with repeats, world/face align, treat as one, texture lock, recent textures; drag handles on the face in 3D; eyedropper in the paint tool (`EditorScene.tsx` "the eyedropper"); now also the face facts (which brush, facing, world or face aligned) | Partial: no Align to view, no Alt+right-click "apply texture and alignment" | H | S |
| Primitives | Block, wedge, cylinder, spike, sphere, arch, torus (J.A.C.K.) with dialogs | `editor/shapes.rs` + Shape tool: cylinder (sides), wedge, spike (cone/pyramid), sphere, arch (sides, arc, start, wall); live outline of the real shape while dragging (`shapePreview.ts`) | Partial: no torus, no separate cone (a spike with many sides is one), parameters are in the tool bar, no parameter dialog before placing | M | S-M |
| path_corner editing | Draw/edit path chains, insert nodes | Entity links drawn (`Links` in `EditorScene.tsx`), target fields autocomplete, Duplicate keeps keys | Missing: no "add next corner" (auto-name and link), no insert between, no path drawing tool | M (deathrun traps, trains) | M |
| Entity I/O view | Hammer 3.5: target lines; Source-era Hammer: I/O list. TB: link lines | Target/killtarget lines in 3D (`Links`), the picked entity's links highlighted; EntityForm lists "Sets off" / "Set off by" | Partial: no multi_manager fan-out lines, no master links, no broken-link flags, no list panel | H (deathrun, jail, fun maps) | M |
| Extrude / shear / vertex | TB: Shift-drag face extrude, vertex/edge/face moves, merge/split; Hammer: vertex tool, shear in 2D | Push/pull a face (`move_face`), vertex tool with corner, edge-middle and face-middle handles (`SceneExtras.tsx` VertexHandles), carve, merge, clip | Partial: no Shift-drag extrude into a new brush, no shear, no vertex merge/split | M | M |
| 2D ortho views | Top/front/side with grid, box create, move, resize | Top, front, side panes (`Layout.tsx`, `panes.ts`) with grid, snapping, box-pick (`Marquee.tsx`), box/shape create, gizmo move; layouts single, two, quad, one big + three | Partial: no 2D resize handles on the selection box (Hammer's 8 handles), shear missing | H | M |
| Linked groups / instances | TB linked groups; Hammer: none (func_instance is Source) | Groups, prefabs with origin/updates (`PrefabPanel.tsx`, prefab uids) | Missing: no live-linked copies (edit one, all follow) | M | L |
| Entity report / check problems | Hammer: Entity Report, Check for problems (Alt+P) with Fix | `Dialogs.tsx` EntityReport; Checks panel with live checks and Pick/Look; Budget panel for engine limits; Find missing files | Have (z-fighting check coming next) | H | - |
| Compile profiles | Hammer: run map expert with saved configurations; J.A.C.K. profiles | Compile panel quality presets Fast / Normal / Final, Playtest F5, bots learn the map | Partial: no saved custom profiles (own tool switches per profile) | L-M | S |
| Coordinates and axes | Hammer: axis labels in 2D views | Triad in every pane (X red east, Y green north, Z blue up), status bar X Y Z, Help > Coordinates and axes | Have | M | - |
| Transform by numbers | Hammer: Transform dialog; Unreal: Details panel | Inspect Transform section (Location, Rotation/Turn by, Size, Scale, scrubbing letters), Transform dialog, texture lock and scale lock | Have | M | - |

## Suggested order

1. Leak tracing polish (fly-along, nearest brush, auto-show): S-M, felt by every new mapper.
2. Face sheet gaps (Align to view, Alt+right-click apply): S.
3. 2D resize handles: M.
4. Torus, cone and a parameter preview: S-M.
5. Extrude and vertex merge/split: M.
6. Entity I/O view with broken-link flags: M, high value for deathrun/jail/fun maps.
7. path_corner tools, then compile profiles, then linked groups (L).

(The user approved 1-6 on 2026-11-13; they're being built in that order.)
