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

## The spawn outcome standard (every solid spawn node)

Applies to Spawn Prop, Spawn Creature, Spawn Entity, Spawn Weapon On Ground and Create
Component (done here), and Spawn Projectile, Spawn NPC, Spawn Interactable and Trigger Volume
(their owners). Trigger volumes aren't solid: they take the outputs but never check the spot.

**If the spot is blocked** (param `blocked`; Unreal's Spawn Collision Handling, named in the help
and keywords: collision handling, force spawn, always spawn):

| Value | Label | Unreal | What it does |
|---|---|---|---|
| `force` | Always spawn, ignore collisions | Always Spawn, Ignore Collisions | exactly there, even overlapping (it may get stuck) |
| `nearby_always` | Try a nearby spot, but always spawn | Adjust If Possible But Always Spawn | moved if a free spot is within Max distance, else in place anyway |
| `nearby` | Try a nearby spot, don't spawn if still blocked | Adjust If Possible But Don't Spawn If Colliding | moved, or Failed (Blocked) |
| `exact` | Don't spawn if blocked | Do Not Spawn | exactly there, or Failed (Blocked) |
| `retry` | Retry every N seconds up to M tries | (ours) | exactly there when it clears; else Failed (Retries used up); 0 tries: until clear |

Defaults: `nearby` for props, creatures, NPCs (and weapons, entities, components); `exact` for
interactables. Params: `max_dist` (default 128, 16-1024), `retry_every` (default 1 s),
`retry_tries` (default 10, 0 = until clear).

**Outputs** (pin ids fixed):

| Pin | Type | Meaning |
|---|---|---|
| `then` | exec | right away, whatever happened (not again on a retry), for code that doesn't care |
| `spawned` | exec | once, when it exists (maybe later, with Retry) |
| `failed` | exec | once, when it gives up |
| `out` "Entity" | entity | the thing (valid on Spawned; 0 otherwise) |
| `ok` "Success" | bool | for pure checks |
| `moved` "Moved" | bool | it was put at a nearby spot, not the asked one |
| `reason` "Failure reason" | enum:ls_spawn_fail | the built-in Spawn Failure enum |

Spawn Failure values (numbers are the order): None 0, Blocked 1, Out of entity slots 2, Limit
reached 3, Missing file 4, Retries used up 5, Bad input 6. It's in graph.rs `BUILTIN_ENUMS` and
schema.ts, and the WHY_* constants in gen/fx.rs.

**The shared helper** (gen/fx.rs): `spawn_flow(gen, id, n, def, b, pre, place, create,
create_why, apply, exit, err, next, after)` writes the whole body: the `pre` checks (each a
condition with its Failure Reason and debugger words: files loaded, `lsg_ent_room`...), the spot
(`fx::place`), `lsg_xf_free(o, size, mode, maxd, moved)` (Pawn: a hull trace at the spot, then
rings every 16 units out to Max distance, 3 heights each, each in sight of the spot), the create
call, `fx::apply`, the outputs, and Retry (a `public lsg_xr{i}` timer re-running the node, task
`LSG_TASK + 410000 + i`; each wait carries its own ctx, spot and try count via `lsg_xr_wait` /
`lsg_xr_back`, and waits for the first try's spot to clear, not the entity's spot now).

Lasting client state (X's `lsg_msg_state`): none of the FX nodes set any; lights, decals, gibs,
rings and sounds are one-off effects and stay on `lsg_msg_room`.

Sim check (de_dust2, 2 bots, a prop at each spawning bot): Try a nearby spot moved it every time
(16/16), Don't spawn if blocked failed every time with Blocked (16/16), Retry spawned once the bot
had walked off (11 of 11 waits that cleared, 1-6 s after the spawn), no run time errors. `hull_size`: 0 small (weapon, item), 1 medium (prop, component), 2 tall
(creature, NPC: HULL_HUMAN), -1 not solid (no check). The node kinds must be in `fx::STORE` (for
the extra output slots). Registry side: make_nodes.py's post-process loop adds the params,
outputs, help (BLOCKED_HELP) and keywords; reuse BLOCKED_CHOICES.

Is Valid / Is Valid? take any entity handle (a spawn node's Entity included); a removed one, or
one going this frame (FL_KILLME), isn't valid.

Fallback chains: wire Failed into another spawn node (spawn at A, on Failed at B), or Switch on
Enum on Failure reason (testdata every_node_fx: pfail -> pB -> why).

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
