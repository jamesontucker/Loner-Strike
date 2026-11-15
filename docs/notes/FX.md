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

## What landed (2026-11-14)

- **Map editor** (4a536202): env_beam, env_laser, gibshooter, env_shooter, env_blood, env_funnel,
  env_beverage, env_message, trigger_cdaudio, target_cdaudio in cstrike.fgd (each confirmed with
  `LINK_ENTITY_TO_CLASS` in ls-next3's dlls/; keys and flags from effects.cpp / triggers.cpp).
  Left out on purpose: env_lightning and env_debris (aliases of env_beam / env_spark),
  speaker, spark_shower, test_effect, env_bombglow (internal or debug). Previews: beams and lasers
  as colored lines between their ends (a ring for a random radius), shooters and blood as arrows,
  models for env_shooter / gibshooter / env_beverage, a glow for env_funnel (EffectPreviews.tsx).
  Checks: beam-no-sprite, beam-no-end, beam-end-missing, shooter-no-model, message-empty,
  effect-unnamed (missing sprite and model files are the assets check's, which already covers them).
- **Studio** (7294f8de): see the commit; nodes in gen/fx.rs, block in make_nodes.py.

## How it was checked

- `cargo test editor::` and `cargo test studio::` (137 pass on 7294f8de), amxxpc (the generated
  plugins, every_node_fx included, compile with no warnings), `tsc -b`.
- A test map with every new FGD entity (two info_targets for the beam, a laser target, a
  trigger_auto → multi_manager firing the rest) compiled clean with SDHLT (no leak, no errors).
- One sim (a clone of LSPLAY, `-insecure`, load under 15, deleted after) on that map with the
  every_node_fx debug plugin and 5 bots: no AMXX run time errors, no engine errors; per the
  debugger's events, every round each bot spawn ran Dynamic Light (point and following), Decal,
  both Gibs, Beam Ring, Play Sound at Location (point and entity), Play Sound on Entity, Loop Sound
  (both) and Stop Sound (all three kinds), and made Spawn Entity (env_sprite at a bot with offset,
  rotation offset, velocity and scale; env_beam; env_spark), Spawn Weapon On Ground (AWP at a bot
  with offset and velocity; Kevlar at a point, turned), Spawn Prop (three ways), Spawn Creature
  (at a bot with offset and velocity) and Create Component (turned). Play Sound 2D said "nobody to
  hear it" (only bots, which it skips; the custom sound is missing on purpose). Spawn NPC and
  Spawn Interactable were refused by gm_components at those spots (its own room check); the
  transform found the spot (same path as the components that were made).
- Old build note: the sim's mp.dll was LSPLAY's (the effect entities are stock HLSDK code in every
  fork version; the armoury items past the M249 need ls-next3 and weren't tried in the sim).
