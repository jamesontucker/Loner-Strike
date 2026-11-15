# FX: engine effects in the editor and the Studio (the effects agent)

## The spawn transform standard (every spawn node)

Pin ids, the same on every spawn node (Spawn Prop, Creature, NPC, Interactable, Create Component,
Spawn Entity, Spawn Weapon On Ground; Spawn Projectile and Spawn Trigger Volume should match):

| pin | type | meaning |
|---|---|---|
| `at_ent` | entity | At entity: its spot (Spawn Prop keeps its old `at`, so saved graphs work) |
| `pos` | vector | At position (wins over At entity) |
| `offset` | vector | Local offset along At entity's own forward, right, up |
| `angles` | vector | Rotation (pitch, yaw, roll), set outright |
| `angles_offset` | vector | Rotation offset, added to At entity's facing (or to the thing's own) |
| `vel` | vector | Velocity: starts it moving (a still thing becomes MOVETYPE_TOSS) |
| `scale` | number (param `scale`, default 1) | models and sprites only; components have no Scale or Velocity |

Setting `copy_rot` ("Face the way At entity faces", default on). For the five nodes that existed
before, a saved graph without the setting keeps it off, so it makes the same plugin as before.

Codegen helpers (gen/fx.rs, reuse them): `fx::place(gen, id, at_pin, fallback)` finds the spot
into `o` (a declared `Float:o[3]`); `fx::apply(gen, n, def, id, ent_expr, at_pin, scale_ok, yaw_param)`
gives the line that turns, pushes and sizes the new entity; Pawn `lsg_xf_at` / `lsg_xf_set`.
Requests to the creature fixer (Spawn Projectile) and W2 (Spawn Trigger Volume): use these ids;
W2 already uses `pos`, `angles`, `at_ent`, `offset`.
