
---

## 6. Files: what landed (the asset track, X-fx)

Steps 6-10 for files, landed on tools: 850fd482 (assets core), 8231bfb7 (typed path settings),
fffdfa5d (packaging), f9333628 (table columns), c53491a8 (Decal), 8517ba7d (Sound Loop).

- **Runtime form.** A Model / Sprite / Sound value is an index into `g_lsgAsset` (0 = none),
  one constant table of every file a value can hold: Model / Sprite / Sound nodes, variable and
  Function-local start values, Set Variable values, data table cells of file columns
  (`gen/assets.rs table`). Loaded in `plugin_precache` through `lsg_pre_asset` (the guarded
  `lsg_pre_model` / `lsg_pre_sound`); `lsg_asset_ok(i)` says whether it loaded. The table is
  emitted whenever a file type appears anywhere (so To Text, which shows a file as its name,
  always compiles) and before the variables (the debugger's variable watch reads it).
  Start values are written where there's no `Gen` (global initializers), so `assets::Scope`
  holds the table in a thread-local while a plugin is written.
- **Decal** is a kind of Place Decal's list (`fx::DECALS`), its number from 1; Place Decal with
  one wired picks a name of that kind (`lsg_decal_pick`). **Sound Loop** is Loop Sound's Handle,
  typed `soundloop` (upcasts to entity, so old wires stay good).
- **Typed settings.** `assets::TYPED` (mirrored by `TYPED_FILES` in make_nodes.py): 16 path
  settings retyped model / sprite / sound / visual with the same ids and stored values; 12 get
  a bound pin of the same id (`assets::path_or`: wired, the table's row; unwired, the old
  literal, so plugins are byte-identical). Set Weapon View Model and the engine sprite effects
  take the setting only (the engine wants a load index written at plugin time). Get Bone /
  Attachment Position stay `path` (a model to read bones from, not a file the mode loads).
  Settings may be empty (the node's own checks say what that does, as before); a sound setting
  with `sound/` in front still checks.
- **Checks.** `assets::valid_asset` (Rust) and `validAsset` (schema.ts) share one list,
  `testdata/asset_values.json`. Budget Problems: warn past 64 models+sprites / 96 sounds,
  error past 128 of a kind. Slicing user strings uses `str::get` (no char-boundary panics).
- **Packaging.** `creaturepack::file_value_sites(_mut)`: node settings plus start values and
  table cells **typed as files** (a text value is never taken for a file). Reviewed.
- **Tests.** `gen/assets.rs` tests, `every_node_assets.graph.json` (compiled with amxxpc),
  `creaturepack` `files_in_start_values_and_table_cells_travel_with_the_mode`, studio.test.ts
  (shared value list, file columns, decal kinds). Sim (quest-giver read from v3 with custom
  model and sound files, file variables wired into Play Sound 2D and Spawn Prop): all loaded,
  spawned and played. A mode's plugin must load after gm_components (the app lists it so):
  otherwise `ls_npc_precache` fails in plugin_precache and the rest of it is skipped.
