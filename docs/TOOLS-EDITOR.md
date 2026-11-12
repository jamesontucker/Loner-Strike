# Map editor: core, compiling, and how the UI drives it

Written 2026-11-12 by the Tools crew (agent E). Companion to TOOLS-PLAN.md §5.1-5.2 and the
"modern Hammer" direction in TOOLS-CREW.md.

## 1. What the core is (built, `app/src-tauri/src/editor/`)

Pure Rust, no routes or UI yet, mounted with `pub mod editor;` in `main.rs`. 38 unit tests.

| File | What it holds |
|---|---|
| `math.rs` | `Vec3`, `Plane` (outward normal, `n.p = d`), `Mat3`, `Affine`, `Bounds`; tolerances (`PLANE_EPS` 1e-4, `POINT_EPS` 1e-3) |
| `doc.rs` | `MapDocument` (versioned, `DOC_VERSION = 1`): `world` (WAD list + worldspawn keys), `brushes`, `entities`, `groups` (all `BTreeMap<Id, _>`, ids never reused), `materials` (texture sizes), `revision` (bumped on every change). `Brush { faces, entity, group }`; `Face { points: [Vec3;3], texture, align: TexAlign }`; `TexAlign` = Valve 220 (u/v axis + offset, rotation, scale). Project JSON via `to_json` / `from_json` (rejects newer versions, repairs `next_id`). |
| `geom.rs` | Plane set → face polygons (clip a huge square by every other plane), CCW from outside (three.js front faces); `validate` (≥4 faces, no collinear points, no duplicate planes, closed, every face on the solid, finite); bounds, volume, vertices; convex hull planes; picking .map points for a polygon/plane |
| `texture.rs` | Quake base axes, world/face alignment, `from_standard` (old Quake face format), shift, scale and rotate around an anchor, fit (N×M repeats), centre, offset tidy, **texture lock** (`locked`, exact for move/rotate/scale/mirror) |
| `ops.rs` | `make_box`, `make_cylinder` (3-64 sides, rounds to the grid when that stays valid), `transform`/`translate`/`rotate`/`scale` (mirror keeps faces outward), `move_face` / `move_face_to` (push/pull; faces it hides are dropped, the moved face must survive), `clip`, `intersect`, `subtract` (carve), `hollow` (inner brush carved out of the outer: walls don't overlap), `merge` (convex hull; reports whether it was exact), `snap_plane_points`, `snap_vertices`, `brush_mesh` (polygons + normalised UVs for drawing) |
| `mapfile.rs` | `.map` reader (Valve 220 and Quake standard faces, `{FENCE`-style names, comments, Quake 2 extras ignored, TrenchBroom `func_group` groups/layers become editor groups; readable line-numbered errors, never panics, 64 MB cap) and writer (always Valve 220, `"mapversion" "220"`, WAD list in worldspawn; groups flattened by default, optional TrenchBroom groups). Round-trip is byte-stable. |
| `history.rs` | Undo/redo. Each edit is a `Change` (before/after of every touched brush/entity/group/world). Edits run in a `Tx` that applies as it goes and commits as one step or rolls back (also on drop, so a failing command leaves nothing behind). `merge_key` folds a drag into one step. `mark_saved` / `is_dirty`. Commands: `cmd_add_brush`, `cmd_delete`, `cmd_transform` (moves point entities and turns their `angle`/`angles` too), `cmd_move_face`, `cmd_clip` (keep back/front/both), `cmd_hollow`, `cmd_carve`, `cmd_merge`, `cmd_snap`, `cmd_set_texture`, `cmd_align` (any texture helper as a closure), `cmd_set_key`, `cmd_add_entity`, `cmd_tie_to_entity`, `cmd_move_to_world`, `cmd_set_world`, `cmd_group`, `cmd_ungroup`. |
| `checks.rs` | `run(doc, known_textures)` → sorted `Issue { level, code, message, brush, entity }` (serde-ready); `stats(doc)` for a status bar. Flags broken brushes, outside ±4096, brush/entity/model/texture counts, entity text size, long keys/texture names, missing textures (tool textures like NULL/SKIP/HINT downgraded to info), no WADs, missing CT/T spawns, brush entities with no brushes, point entities with no origin, dangling `target`s. Leak detection is not done (see §3). |

Design choices worth knowing:
- **Faces are stored as the three .map points**, not as normal+distance. That keeps .map files
  round-tripping exactly and lets every transform act on points; the plane is derived.
- **The document lives in Rust.** three.js gets polygons/UVs from `brush_mesh` and draws them.
  Ids are stable, so the view can cache a mesh per brush id and only rebuild the ones a
  `Change` touched (`Change::touched_brushes`, `doc.revision`).
- **Commands never half-apply**: errors are friendly strings and the document is unchanged.
- Geometry is f64 with whole-unit tidying (31.999999 → 32) so exported files stay clean.

Not in the core yet (next steps, in order of value): vertex/edge editing beyond snapping
(TrenchBroom's vertex tool), FGD parsing for the entity panel, prefab files (a `.map` fragment
or our JSON of a selection), BSP import as "trace over" reference, leak finding, `.map`
export with WAD paths rewritten for the compiler (small: map `world.wads` to real paths).

## 2. Compiling GoldSrc maps

The pipeline is four separate programs, run in order on `name.map`:
`hlcsg` (brushes → hulls, textures, WAD list) → `hlbsp` (BSP tree, portals; writes the `.bsp`,
or `name.lin`/`name.pts` when the map leaks) → `hlvis` (visibility; `-fast` for previews) →
`hlrad` (lighting, the slow one; `-extra` for release quality).

### 2.1 The options (checked 2026-11-12)

| Tools | Licence | Binaries | macOS | Verdict |
|---|---|---|---|---|
| **SDHLT** (github.com/seedee/SDHLT, v1.3.1, 2026-10-16, active) | `LICENSE.md`: GPL (v2 text), "In addition to the GPL, the Valve SDK 2.3 EULA overrides any rights you may have obtained in the GPL, when needed", plus the id Quake 2 licence. GitHub shows NOASSERTION. | Release zip: `tools/Win32/sdHL{CSG,BSP,VIS,RAD}.exe`, `tools/Win64/*_x64.exe`, `tools/Linux/sdHLCSG`…, plus `sdhlt.wad` (required tool textures), `sdhlt.fgd`, `lights.rad`, `wad.cfg` | No build. CMake knows Linux and Windows only (Darwin falls to "Unknown system"); CI builds Windows x86/x64 and Ubuntu. A clang/arm64 port looks feasible (the Linux path is POSIX + pthreads) but nobody has shown it working. | **The one to use.** Maintained, best output, progress is easy to parse. |
| VHLT v34 (vluzacn, ~2014) | Same ZHLT wording | Old Windows builds; source mirrors (twhl-community/VHLT-V34, FreeSlave/vhlt) | No | Superseded by SDHLT (which is based on it). |
| ZHLT 3.4 (~2006) | Same | Windows only | No | Obsolete. |
| **ericw-tools** (github.com/ericwa/ericw-tools, 2.0.0-alpha11, 2026-06-05) | GPL-2.0 (`COPYING`), no Valve clause | Darwin, Linux and win64 zips | Darwin zip is **x86_64 only** (runs under Rosetta); arm64 rebuild plausible | `qbsp -hlbsp` writes BSP30 (`-wadpath`, `-xwadpath`). Lighting is Quake-style: no HLRAD features (`light_environment`, texture lights, `lights.rad`) verified; alpha quality. A "preview compile" at most. |

Command lines (SDHLT; `_x64` suffix on Win64; run with the `.map`'s folder as working dir):

```
sdHLCSG -threads N  path/name          # WADs from the worldspawn "wad" key; only used ones are kept
sdHLBSP -threads N  path/name          # leak -> name.lin + name.pts, no .bsp
sdHLVIS -threads N  -fast path/name    # -full for release
sdHLRAD -threads N  path/name          # -extra for release; -bounce N, -smooth N as options
```

Useful flags: `-wadinclude name` (embed a WAD's textures in the BSP; we should do this for any
custom WAD so players don't need it), `-nowadtextures`, `-estimate`, `-low` (lower process
priority: good while the game is running), `-chart` (limits report). Outputs: `name.bsp`,
`name.log`, `name.err` (on failure), `name.lin`/`name.pts` (leak line), `name.prt` (portals for
VIS), intermediates (`.p0-.p3`, `.b0-.b3`, `.wa_`, `.hsz`) to clean up. Progress: each stage
prints `10%...20%...` (`"%d%%..."` in `threads.cpp`) and ends with `(N.NN seconds)`; a regex on
`(\d+)%` per tool gives a smooth bar across the four stages (weight RAD the heaviest).

WAD paths: the `wad` key normally holds Windows paths from the mapper's PC. For compiling we
should export a temporary `.map` whose `wad` key lists the real paths of the WADs on this
computer (we already know the game folders), plus `sdhlt.wad` for tool textures.

### 2.2 Can we ship it?

- **Bundling inside the app installer: no.** The ZHLT-lineage licence is GPL plus an overriding
  Valve EULA; that's not GPL-3-compatible, so it must never be linked or vendored into our code.
- **A separate, optional download: yes, with care.** Running unmodified binaries as child
  processes is aggregation (every editor and batch compiler has done this for 20 years). Best:
  download the official SDHLT release zip from GitHub on demand, pinned by version and sha256 (the
  setup module already does pinned, checksummed downloads), install it under the app's data
  folder, keep `LICENSE.md` beside it, and say "Map compiler (SDHLT, separate licence)". Our app
  stays free, which the Valve SDK EULA requires. Not legal advice; the user should be comfortable
  with it before release.
- ericw-tools is plain GPL-2.0 (check the headers for "or later" before ever linking; as a
  child process it doesn't matter).

### 2.3 Per platform

- **Windows:** SDHLT `Win64` exes, run directly. Straightforward.
- **macOS (arm64 and Intel):** Loner-Strike already requires CrossOver on a Mac: the game and the
  Windows HLDS run through `CrossOver.app/.../bin/wine` (`servers.rs::CROSSOVER_WINE`,
  `steam.rs`). So the realistic 1.0.0 route is **the same SDHLT Windows exes through CrossOver's
  wine**, exactly like HLDS. No new toolchain, same output as Windows. Cost: Wine overhead
  (threads work; RAD is CPU-bound, expect roughly native-x86-under-Rosetta speed) and path
  translation (pass `Z:\...` or bottle-relative paths, as `servers.rs` does for HLDS).
- **Native macOS arm64 SDHLT**: needs a Darwin branch in SDHLT's CMake and fixes for whatever
  breaks under clang; then our own CI build, signed and notarized. A good post-1.0 project
  (and worth offering upstream). Until then don't promise it.
- **Linux** (servers only): SDHLT ships Linux binaries; not needed for the app.

### 2.4 Honest 1.0.0 scope

- **Feasible for 1.0.0 (Beta):** the editor core here, `.map` import/export, live checks, and a
  compile button that runs SDHLT as an optional download (Windows native, Mac via CrossOver),
  with a progress bar, the log in a drawer, and the leak line drawn from `name.lin` (a list of
  points; trivial to parse). Then "Play it": copy the `.bsp` to `cstrike/maps` and launch with
  our bots (TOOLS-PLAN 5.4).
- **Fallback if compiling isn't ready:** ship the editor with `.map` export (open it in J.A.C.K.
  or compile with any HLT front-end) plus the BSP-direct edits. Nothing in the core depends on
  the compiler.
- **Not for 1.0.0:** native arm64 SDHLT, ericw-tools as a preview path (different output and
  lighting would confuse people), leak finding before a compile (needs hull flood-fill: hlcsg's
  job), vertex-level editing beyond snap.

## 3. How the modern-Hammer UI drives the core

One 3D view first; every interaction is a core call that returns a new document revision. While
dragging, the UI previews locally (or calls the op functions without committing) and commits on
mouse-up with one `merge_key` per drag, so undo takes back the whole gesture.

| Interaction | Core call |
|---|---|
| Draw a box (click-drag on the grid, drag up for height) | `ops::make_box` (preview) → `cmd_add_brush` |
| Cylinder tool (sides in the tool bar) | `ops::make_cylinder(base, r, h, sides, grid)` → `cmd_add_brush` |
| **Click a face and drag** (push/pull) | `ops::move_face_to(brush, face, mouse point on the face's normal line, lock)` for the live preview; `cmd_move_face(..., drag_id)` per step or once on release. Snapping: round the distance to the grid unless Alt |
| W / E / R gizmos (move / rotate / scale) | `cmd_transform(brushes, entities, Affine, texture_lock, label, drag_id)`; pivot = selection centre (`geom::brush_bounds`) |
| Mirror (flip buttons) | `cmd_transform` with `Mat3::scale(-1, 1, 1)` around the centre |
| Clip tool (click two/three points, Tab cycles which side stays) | `Plane::from_map_points` → `cmd_clip(ids, plane, Keep::Back/Front/Both, texture)`; preview with `ops::clip` |
| Hollow (thickness field, default 8) | `cmd_hollow(id, thickness)` |
| Carve with selection | `cmd_carve(cutter, overlapping ids)` (Hammer's carve; warn that it makes many pieces) |
| Merge | `cmd_merge(ids)`; if `exact == false`, say "these didn't form one convex shape: the result fills the gaps" |
| Snap to grid | `cmd_snap(ids, grid)`; snapping on by default, Alt ignores |
| Texture browser: click a texture, click faces (or drag onto a face) | `cmd_set_texture(&[(brush, Some(face))], name)`; Shift = whole brush (`None`) |
| Texture panel: shift / scale / rotate fields and drag handles with live preview | `cmd_align(faces, drag_id, |a, poly, size, n| texture::shift / set_scale / rotate(...))` |
| Fit, centre, align to world / face buttons | `texture::fit`, `center`, `align_to_world`, `align_to_face` through `cmd_align` |
| Texture lock toggle (on by default) | the `lock` argument of transform / move-face |
| Entity inspector (fields, colour pickers, angle dial) | `cmd_set_key(entity, key, value)`; empty value removes |
| Place an entity from the browser | `cmd_add_entity(Entity::new(class) + origin)` |
| Tie to entity / move to world | `cmd_tie_to_entity(ids, "func_wall")`, `cmd_move_to_world(entity)` |
| Group / ungroup, hide / lock groups | `cmd_group`, `cmd_ungroup`; hidden/locked flags on `Group` |
| Map settings (sky, WADs, light) | `cmd_set_world` |
| Undo / Redo (Ctrl+Z / Ctrl+Shift+Z, labels in the menu) | `History::undo/redo`, `undo_label/redo_label` |
| Live checks panel (click an issue to select the brush/entity) | `checks::run(doc, wad textures)` after each change; `Issue.brush/entity` ids |
| Status bar | `checks::stats` |
| Open / save project, import / export .map | `MapDocument::from_json/to_json`, `mapfile::read_map/write_map` |
| Drawing | `ops::brush_mesh(brush, |tex| doc.texture_size(tex))` → positions + UVs per face; textures from the WAD decoder agent A built |
| Picking a face under the mouse | ray vs. `geom::brush_polygons` (planes are in the polygons; a small `pick` helper can be added to the core when the UI starts) |

### 3.1 What the UI agent will need from the Rust side (proposed, not built)

Routes under `/api/tools/editor/` on the local listener only, one open document per session
(kept in server state with its `History`):
`POST new | open (project or .map, picked with the dialog) | save | export-map`,
`GET doc` (entities, groups, world, revision), `GET meshes?since=REV` (brush meshes changed
since a revision, binary like the LSMB bundle), `POST cmd` (`{op: "hollow", ids, thickness}` →
`{revision, changed, error?}`), `POST undo | redo`, `GET checks`. These are a proposal for the
crew contract (TOOLS-PLAN §3.8), to agree before anyone builds them.
