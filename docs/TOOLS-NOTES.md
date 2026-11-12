# Tools crew notes

## Requests

## Progress

### Agent A (Rust core: goldsrc readers + map routes) — M1/M2 server side done

**Built**
- `goldsrc/mod.rs` (LE readers, `Error` with plain words, caps: BSP 64 MB, WAD 256 MB, texture side 1024,
  `PARSER_VERSION`, `png()`), `bsp.rs` (all viewer lumps, every offset checked, `read_lumps` reads single lumps
  from disk, a test fixture builder: cube room + func_wall), `entities.rs`, `wad.rs` (WAD3 + miptex → RGBA,
  `{` index 255 → alpha 0), `lightmap.rs` (engine extents, style 0, shelf-packed pow2 atlas, 1 px edge padding,
  4×4 white block for unlit faces), `mesh.rs` (faces → triangles, LSMB writer `write_bundle`, shared for C's bundles).
- `tools/mod.rs` (`local_routes()`, merged on `Listener::Local` in server.rs, + `overlays::routes()`),
  `tools/cache.rs`, `tools/mapinfo.rs`.
- Real maps: `LS_TEST_GAME` over all 391 cstrike/czero maps here: no errors, no panics, every map has
  triangles; de_survivor 21k triangles, 1024² lightmap, 95/95 textures, ~22 ms parse+build (release).

**Contract details B should know (all in the bundle header too)**
- Section `offset` is absolute from the bundle start (4-aligned), `length` in bytes, `stride` = components,
  plus `count` = elements. JSON is padded with spaces up to the data.
- `indices` are global (into the whole vertex arrays); draws are ranges of `indices` (`start`/`count` in
  indices, for `addGroup`), sorted by model then texture name. Each model also has `{index, start, count,
  entityIndex, classname, origin, mins, maxs, renderMode?, renderAmt?}` in `models[]`.
- **`origin` per model:** brush entities built around an origin brush (rotating doors, func_rotating) sit at 0,0,0
  in the buffers; translate that model's group by `origin`. World is always [0,0,0].
- Triangles are counter-clockwise seen from the front (three's default FrontSide), whatever the map stored.
- `flags` is a number; bits in `flagBits`: sky 1, water 2 (`!`/`*`/`water…`), trigger 4 (`aaatrigger`, clip/null/
  origin/hint/skip/bevel, or any `trigger_*` entity's model), transparent 8 (`{` textures).
- `uv` is already divided by texture size; `uv2` 0..1 into `/lightmap.png` (RGB, raw: apply the ×2 overbright/gamma).
- Errors: JSON routes (maps, info, entities) answer 200 `{error}` (400 for a bad name), like the rest of the app.
  Binary routes (mesh, lightmap.png, tex) answer `{error}` JSON with 404 (map/texture not there), 422 (damaged),
  400 (bad name), 409 (no game set up). Binary answers carry `ETag` + `Cache-Control: no-cache`; 304 on match.
- `/tex/<name>.png`: name case-insensitive, URL-encode it (`%7Bgrate.png`, `%21water.png`). 404 → draw the checker.
- `/info`: `{name, game, size, bounds, models, brushEntities, entities, classes{}, textures[{name,width,height,from}],
  missingTextures[], wads[{name,found,folder?,error?}], missingWads[], spawns{t[],ct[]} ({origin,yaw}), message,
  skyname, title}`; `from` is "embedded", the WAD's name as the map lists it, or "missing".
- `/maps`: `{maps:[{name, game, bspSize, nav:{version,places,bspSize,matchesMap}|null, lsv:{stage,done}|null,
  missingWads[], wads[], title, cat, source, other?}]}` (lsv key check and recordings are left to agent C).

**Security (from the review)**: embedded textures keep only their own record (40 + mips + palette), at most 64 MB in
all; mesh output capped at 4M vertices / 12M indices, ≤ 64 edges per face, and surfedges walked ≤ 4× the lump
(→ "damaged"). Tests: `embedded_textures_copy_only_their_record`, `overlapping_faces_are_capped`.

**How to try it**
- `cd app/src-tauri && cargo test goldsrc:: tools::` (fixture room, WAD with `{grate`, router tests incl. 304,
  bad names, damaged map, not on the LAN listener).
- Real maps: `LS_TEST_GAME="$HOME/Library/Application Support/CrossOver/Bottles/Steam/drive_c/Program Files (x86)/Steam/steamapps/common/Half-Life" cargo test --release real_maps -- --nocapture`
  (add `LS_TEST_DUMP=<folder>` to save each lightmap atlas as PNG).
- With the app running (`npm run tauri dev`, port shown in the app; the guard wants Host 127.0.0.1:<port>):
  `curl -s 127.0.0.1:<port>/api/tools/maps | head -c 600`, `.../api/tools/map/de_dust2/info`,
  `curl -s -o /tmp/d2.lsmb .../api/tools/map/de_dust2/mesh`, `.../lightmap.png`, `.../tex/%7Bfence.png`, `.../entities`.
- Cache: `~/Library/Caches/Loner-Strike/tools/<map>/<size>-<mtime>-p<version>/` (Windows: `%LOCALAPPDATA%\gg.lonerstrike.app\cache\tools`).

**Left / notes**
- Not done: lightmap styles 1-3, animated (`+0~`) textures, sky box images (only `skyname` in info).
- Build needed empty `app/src-tauri/resources/linux/{ls-voice,ls-intent}` and `resources/windows/hamsandwich`
  folders in this worktree (they're untracked binaries in the main checkout); I created them empty, not committed.
- Texture lookup order: embedded → worldspawn's WADs in their listed order, each searched by base name in
  cstrike, valve, czero (CZ game: czero, cstrike, valve), then the `_downloads` folders. A texture in a WAD the
  map doesn't name counts as missing (as in the game).
