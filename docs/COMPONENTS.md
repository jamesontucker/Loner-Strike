# Game mode components (`gm_components`)

Phase A of [GAMEMODES-PLAN.md](GAMEMODES-PLAN.md): the reusable gameplay pieces modes are built from.
Each component is an `ls_*` entity with typed keys. This one plugin runs all of them, shows them to
players within what the stock 1.6 client can draw, tells Loner-Strike's bots what they mean, and
calls `ls_on_*` forwards in every plugin.

| File | What |
|---|---|
| `plugins/scripting/gm_components.sma` | the plugin (one file, so setup can compile it like our other plugins) |
| `plugins/scripting/include/ls_components.inc` | natives and forwards (loner.inc includes it) |
| `app/src-tauri/src/editor/lonerstrike.fgd` | definitions for the map editor (and Hammer / J.A.C.K. / TrenchBroom) |
| `app/src-tauri/src/editor/components.json` | the editor's drag-in library: group, title, how to draw it |
| `plugins/configs/gamemodes/loot.ini` | loot tables |
| `plugins/configs/gamemodes/components/<map>.<mode>.ini` | map packs that ship (Last Stand's for five stock maps) |

## Getting components into a map

1. **In the BSP.** Place `ls_*` entities in the editor (`lonerstrike.fgd`). While the map loads the
   plugin swaps the classname for `info_target`, keeps every key, and turns it back into the
   component when it spawns. Without gm_components on the server, the game just leaves them out.
   `ls_barricade` is a brush entity; the others are points (areas use `radius` or `size`).
2. **Map packs, no BSP edits.** One component per line, `classname x y z key=value ...`
   (quotes for values with spaces). Loaded at map start, in this order, all of them:
   - `addons/amxmodx/configs/gamemodes/components/<map>.ini` and `<map>.<mode>.ini` (shipped)
   - `addons/amxmodx/configs/components/<map>.ini` and `<map>.<mode>.ini` (the server's own)

   `<mode>` is the Game Mode Menu's mode id (`data/gamemodes/current.txt`). z can be a bit above the
   floor: components find the floor under them (a component inside a wall keeps the height it was
   given, with a line in the log).
   ```
   ls_capture_point 1152 2464 120 name=A radius=200 capture_secs=10
   ls_zone_shrinker 0 0 0 stages="2400:30:30:2 1200:30:25:5" start_delay=20
   ```
3. **In game, for building packs.** Admin console (`ADMIN_CVAR`):
   `ls_comp_spawn <classname> [key=value ...]` makes one where you aim (from the server console: give
   `x y z` after the classname), `ls_comp_list`, `ls_comp_del <ent>`, `ls_comp_save` (writes every
   pack and run-time component to `configs/components/<map>.ini`), `ls_comp_start <ent|0>`,
   `ls_comp_stop <ent|0>`, `ls_comp_reset`.
4. **From a mode.** `ls_comp_create("ls_capture_point", origin, "radius=160 name=^"B^"")`.

Models a key names (`model=`) must be loaded while the map loads (BSP or pack); a component made later
with a model that wasn't loaded shows its default instead. All weapon models and our defaults are
loaded up front.

## Shared keys, rules and the round

| Key | Meaning |
|---|---|
| `targetname` | its name: modes find it with `ls_comp_by_name`, map logic sets it off (a button whose `target` is this name) |
| `name` | a label players see ("B"); defaults to `targetname` |
| `enabled` | 1/0: on at round start |
| `target` | entities to set off when its main thing happens (see each component) |
| `team` | `0`/`any`, `1`/`t`, `2`/`ct` |
| `radius`, `height`, `size` | areas: a cylinder (`radius` around, `height` above the floor) or a box (`size` = "x y z", centred on the point); a brush entity uses its brush |

- Every component goes back to how its keys say at each new round (`ls_comp_round_reset 1`);
  run-time crates are removed. Times "after the round starts" count from Round_Start (freeze time
  over); a map with no rounds gets its timers 20 s after the map starts.
- Being set off by map logic (`Use`): areas switch on/off, an extraction opens/closes, a door
  toggles, a trap fires, anything with a start (shrinker, wave, drop, platform, meeting) starts.
- Cvars: `ls_comp_hud 1` (status line + announcements), `ls_comp_beams 40` (beam segments per
  player per second, 0 = no rings), `ls_comp_bots 1` (bot hints), `ls_comp_bot_fallback 1`
  (`ls_bot goto` when the brain lacks `ls_mode`), `ls_comp_spawns 1` (use respawn points),
  `ls_comp_spawn_far 0` (pick the point furthest from others: battle royale), `ls_comp_radar 0`
  (radar dot trick, unverified), `ls_comp_round_reset 1`, `ls_comp_debug 0` (log events and every
  `ls_mode` line), `ls_comp_wave_announce 1` (the "Wave N" / "cleared" banners; a mode with its own sets 0).

## What players see (stock client only)

- **One status line**, bottom middle, own HUD sync object: "Taking point B: 45%", "OUTSIDE THE ZONE -
  12 m to your left", "Hold USE to open", "Extracting... 3 s", "Checkpoint 2/5 0:34.2". Two lines at
  most, a warning first and in red. Refreshed only when it changes (or once a second). The 4 slots in
  loner.inc stay the mode's.
- **Director HUD** for rare announcements (point taken, zone closing, drop incoming, wave N, meeting).
- **BarTime / BarTime2** for hold actions (open, repair, lock, extract, taking a point), **StatusIcon**
  `rescue` while extracting, **RoundTime** counting down to the zone's next change, red **ScreenFade**
  on zone/damage ticks.
- **Beams**, sent unreliable, at most once a second, only to people nearby, within
  `ls_comp_beams` segments per person: the zone ring (24 segments at the player's height, the next
  ring dimmer) for players within 1500 u of the edge, plus the 3 nearest areas (16 segments, 12 when
  far; boxes as 4-segment outlines). A `TE_BEAMCYLINDER` pulse when the zone starts closing. Supply
  crates get one vertical beacon beam a second.
- **Glow** by loot tier (grey, green, blue, purple, gold); spawn protection glows.
- `ls_comp_radar 1`: a radar dot at the zone centre (when outside), an open extraction or a crate,
  using a free player slot's `Radar` message. Not yet checked on the stock client.

## How bots learn what it means

Components send `ls_mode` server commands (agreed with W2 in TOOLS-NOTES, brain branch `ls-modes`)
when their state changes and all at once at Round_Start after `ls_mode reset`. Only if the cvar
`ls_mode_ver` exists; otherwise nothing is sent, and with `ls_comp_bot_fallback 1` a third of the
bots walk to capture points, open extractions and meetings with the brain's older `ls_bot <i> goto`.

| Command | Sent for |
|---|---|
| `goal <all\|t\|ct\|bot> <kind> x y z ent prio until radius` | capture (capture / defend by owner), extraction (`extract`), barricade (`defend` for its team), meeting (`meeting`), next checkpoint per bot (`go_hold`) |
| `goal_clear <who> <ent>` | the goal ends (extraction closes, barricade breaks, meeting ends, a player extracted) |
| `attract c<ent> x y z r weight until team` | capture points (for the side that doesn't hold it), weapon spawners and crates while full (weight by tier), landed supply crates (90) |
| `avoid c<ent> x y z r cost until team` | damage zones (cost 1-10 from damage per second, -1 = never from 50/s), armed traps with damage or `danger=1` (6), wave spawners for the defending team (3), a falling supply crate's landing spot (5) |
| `forget c<ent>` | taken / empty / disarmed / switched off |
| `block <door\|barricade ent> -1\|0` | a locked closed timed door, a standing barricade (`blocks_bots=1`); 0 lifts it |
| `zone cx cy cz r ncx ncy ncz nr shrink_at` / `zone_clear` | the safe zone and the next circle |
| `platform c<ent> <train ent> <moving 0\|1> x y z` | a moving platform started or stopped (top centre) |
| `profile <bot> <name>` | a class with `bot_profile` given to a bot |

Modes can send their own lines with `ls_comp_bots_hint("goal all defend 1 2 3")`.

## The components

Forwards marked *(handled)* skip the default when they return `PLUGIN_HANDLED`.

### ls_respawn_point
Where a team spawns. If a map has any for a team, that team's players are moved to a free one
(none within 64 u) 0.1 s after spawning, random or furthest from others (`ls_comp_spawn_far`).
- Keys: `team`, `angle`, `protect_secs` (3; can't be hurt, glows), `role` (for the Mode Kit).
- Forward: `ls_on_respawn_point(ent, id)` *(handled: stays where the game put them)*.
- Natives: `ls_respawn_pick(id)`, `ls_respawn_at(id, ent = 0)` (respawns a dead player there).
- Bots: none needed (they spawn there).

### ls_weapon_spawner
An item lying on the floor (its own world model, turning, tier glow); walk over it to take it, back
after `respawn_secs`. Players who already have a gun in that slot walk over it.
- Keys: `item` (`ak47`, `vesthelm`, `hp:50`, `table:rifles`...), `tier`, `respawn_secs` (30, 0 =
  once), `once`.
- Forwards: `ls_on_weapon_taken(ent, id, item[])` *(handled: not given)*, `ls_on_weapon_respawn(ent)`.
- Natives: `ls_spawner_set_item(ent, item[])`, `ls_spawner_respawn(ent)`.
- Bots: `attract` while it's there.

### ls_loot_container
A crate (`models/w_weaponbox.mdl` or `model=`); hold USE for `open_secs` to get `items` rolls from
`table`, only items up to its `tier`. Bots that stand by a full one open it (they can't hold USE yet).
- Keys: `table` (default), `tier`, `items` (2), `open_secs` (2), `once` (1), `respawn_secs`, `glow`,
  `model`, `target` (set off when opened).
- Forwards: `ls_on_loot(ent, id)` *(handled: nothing given)*, `ls_on_loot_item(ent, id, item[])` *(handled: skip it)*.
- Natives: `ls_loot_fill(ent, table[])`, `ls_loot_open(ent, id)`, `ls_loot_roll(table[], out[], len, tier)`,
  `ls_give_item(id, item[])`.
- Bots: `attract`, weight by tier, while full.

### ls_supply_drop
A spot a crate falls on (`models/mil_crate.mdl`): announced, falls for `fall_secs` from under the sky
(or 2000 u up), lands with smoke, opened like a container (gold glow, red beacon until opened).
Crates are removed at the next round.
- Keys: `at` (seconds into the round, -1 = manual), `chance` (%), `fall_secs` (20), `table` (drop),
  `items` (4), `open_secs` (3), `beacon`, `target` (set off on landing).
- Forward: `ls_on_drop(ent, stage, origin[3])` (0 announced, 1 landed, 2 opened; ent = the crate) +
  the loot forwards.
- Natives: `ls_drop_call(origin[3], table[] = "")` (anywhere, returns the crate), `ls_comp_start(marker)`.
- Bots: `avoid` the landing spot while it falls, then strong `attract`.

### ls_safe_zone
The circle (2D) players must be inside; outside takes `damage` every `interval`, red flash, the
status line says how far and which way.
- Keys: `radius` (3000), `damage` (1), `interval` (1).
- Forwards: `ls_on_zone_tick(ent, id, Float:dmg)` *(handled: no damage)*, `ls_on_zone_cross(ent, id, outside)`,
  `ls_on_zone_stage(ent, stage)`.
- Natives: `ls_zone_set(ent, center[3], r)`, `ls_zone_shrink(ent, center[3], r, secs)`,
  `ls_zone_radius`, `ls_zone_center`, `ls_zone_inside(ent|0, id)`, `ls_zone_damage(ent, dmg)`, `ls_zone_next(ent|0, center[3], &radius, &closing)` (seconds to the next change, the next circle, whether it's closing now).
- Bots: `zone` (now and next circle, when it closes).

### ls_zone_shrinker
Shrinks a safe zone in stages "radius:warn:shrink:damage": warn (next circle drawn, announced,
round clock counts down), then closes in over `shrink` seconds with the stage's damage.
- Keys: `zone` (targetname, empty = first), `stages`, `start_delay` (30, -1 manual),
  `random_center` (1: each centre lands inside the last circle, on open ground), `final_r`, `roundtime`.
- Forward: `ls_on_zone_stage(zone ent, stage)` when a stage starts closing.
- Natives: `ls_zone_stage(ent)`, `ls_zone_skip(ent)`, `ls_comp_start/stop`.
- Bots: the zone's `zone` line, sent at each warn.

### ls_damage_zone
Hurts players inside (fire, poison, shock, radiation, plain) every `interval`; red ring nearby,
"Danger: fire" on screen.
- Keys: area keys, `dmg` (10), `interval` (1), `kind`, `team` (who it hurts), `hud_warning`, `show`.
- Forward: `ls_on_damage_zone(ent, id, Float:dmg)` *(handled: no damage)*.
- Natives: `ls_comp_enable(ent, on)`; map logic toggles it.
- Bots: `avoid` with cost from damage per second.

### ls_timed_door
Runs `func_door`/`func_door_rotating`s named `door` (or `door=*12`, a brush model, for maps whose doors have no name): opens at `open_at`, closes at `close_at`, closes
again `open_secs` after opening, can't be used by players before `locked_until`; with `lockable` a
player of `team` holds USE by it for `lock_time` to lock it for `lock_secs`. Its doors are given the
game's "toggle" flag so they stay where they're put.
- Forwards: `ls_on_door(ent, open)`, `ls_on_door_locked(ent, id, Float:secs)`; `target` fires on opening.
- Natives: `ls_door_open`, `ls_door_close`, `ls_door_lock(ent, secs)`, `ls_door_is_open`.
- Bots: `block` while locked and closed (they go round), lifted when it opens or unlocks.

### ls_trigger_trap
Sets off its `target` entities (buttons, hurts, doors, env_*) after `delay`, hurting players inside by
`dmg`, then rearms after `rearm_secs`.
- Keys: area keys, `fire_on` (`touch`/`use`), `team_only`, `delay`, `dmg`, `rearm_secs` (5, 0 once),
  `armed`, `danger`, `trap_kind`.
- Forward: `ls_on_trap(ent, activator, victims)`.
- Natives: `ls_trap_fire(ent, activator)`, `ls_trap_arm(ent, armed)`.
- Bots: `avoid` while armed when it does damage or `danger=1`.

### ls_capture_point
Stand on it to take it: `capture_secs` alone, up to 3x faster with more (`multiplier`), frozen while
both sides are on it, decays back when nobody is (`decay`). `ffa=1`: held by one player (King of the
Hill). Ring in the owner's colour, BarTime while taking.
- Keys: area keys, `capture_secs` (10), `multiplier`, `decay` (1), `team_lock`, `start_owner`, `ffa`, `target`.
- Forwards: `ls_on_capture(ent, side, id)`, `ls_on_capture_contested(ent)`, `ls_on_capture_lost(ent, side)`,
  `ls_on_capture_tick(ent, side, id)` (each second held: once with id 0, then per holder on it).
- Natives: `ls_cp_owner`, `ls_cp_progress`, `ls_cp_capturer` (-1 contested), `ls_cp_set_owner`, `ls_cp_lock(ent, secs)`.
- Bots: `goal capture` for whoever doesn't hold it, `goal defend` for the holders, `attract`.

### ls_barricade
Call of Duty style by default (`see_through 1`): nobody walks through an intact barricade (players,
bots, creatures: movement uses hull traces, which the barricade's brush (a CLIP brush filling the
opening is ideal) or box stops), but bullets and sight pass the gaps: a point trace that hits the
barricade or one of its `boards=` brushes carries on past it (an FM_TraceLine hook), and the
entity carries `pev_iuser4 = 1` (the bots' shoot-through marker). Boards disappear one by one as
it weakens and come back one by one as it's repaired; broken, the opening is open to everyone;
it re-forms only when nothing stands in it and no creature is within 250 units. `see_through 0`
is a plain solid wall.
A wall (a brush in the map; in a pack, a box of `size`, stretched wall to wall with `fit=x|y`, drawn
as a beam fence that goes from green to red with health, or shown with `model`; or `adopt=` an
existing `func_wall`/`func_breakable` by name or `*model`) with `hp`; shots and grenades break it (its own team
can't, unless `friendly_damage`), it tints as it weakens, breaks with wood gibs. Its team holds USE
on it for `repair_secs` to add a quarter (`repair_cost` money each).
- Forwards: `ls_on_barricade_damaged(ent, attacker, hp)` (each quarter lost), `ls_on_barricade_broken(ent, attacker)`,
  `ls_on_barricade_repaired(ent, id, hp)`; `target` fires when broken.
- Natives: `ls_barricade_hp`, `ls_barricade_set_hp`, `ls_barricade_repair(ent, id = 0)`.
- Bots: `block` while standing, `goal defend` for its team; cleared when broken.

### ls_wave_spawner
Waves shared by every spawner: wave n gets `wave_min`..`wave_max` enemies (linear over `waves`) from
each spawner, one every `rate` s within `radius`. Each enemy: `ls_on_wave_spawn` (a mode spawns its
own creature and calls `ls_wave_track`); otherwise a dead player of the spawner's `team` comes back
there (bots first; people too with `humans=1`). Cleared when all are spawned and dead.
- Keys: `team` (1), `wave_min` (4), `wave_max` (12), `waves` (10), `rate` (1.5), `radius`, `types`,
  `auto` (start by itself after `prepare`, next wave `between` s after a clear), `humans`, `target` (on clear).
- Forwards: `ls_on_wave_start(wave)`, `ls_on_wave_spawn(ent, wave, origin[3], types[])` *(handled: you spawned it)*,
  `ls_on_wave_cleared(wave)`.
- Natives: `ls_wave_start(wave = 0)`, `ls_wave_stop`, `ls_wave_current`, `ls_wave_alive`, `ls_wave_track(ent)`.
- Bots: the defending team `avoid`s the spawn area a little.

### ls_extraction_zone
Open from `open_at` to `close_at`; a player of `team` inside for `hold_secs` (BarTime, rescue icon,
50% faster per extra player with `shared`) pays `cost` and extracts: out of the round with no death
counted (`action=spectate`), weapons taken (`strip`) or nothing (`none`).
- Forwards: `ls_on_extract_open(ent, open)`, `ls_on_extracting(ent, id, Float:secs)`,
  `ls_on_extract(ent, id)` *(handled: skip the action)*; `target` fires per extraction.
- Natives: `ls_extract_open(ent, open)`, `ls_extract_is_open`, `ls_extracted(id)`.
- Bots: `goal extract` while open.

### ls_checkpoint
Race gates, `order` 1..N; passing them in order, the last finishes a lap (time shown).
- Keys: area keys, `order`, `team`, `target`.
- Forwards: `ls_on_checkpoint(ent, id, order)`, `ls_on_lap(id, lap, Float:secs)`.
- Natives: `ls_checkpoint_of(id)`, `ls_checkpoint_laps(id)`, `ls_checkpoint_reset(id = 0)`.
- Bots: `goal <bot> go_hold` at their next checkpoint (checkpoint 1 at round start).

### ls_moving_platform
Watches a `func_train`/`func_plat`/`func_tracktrain` named `platform`; sets it off at `auto` seconds.
- Forward: `ls_on_platform(ent, moving)`.
- Natives: `ls_platform_go`, `ls_platform_moving`.
- Bots: `platform` lines (stopped/moving and where), for the brain's ride rule.

### ls_player_class
A class: `hp`, `armor`/`helmet`, `speed`, `gravity`, `weapons`, `model`, `limit`, `team`; picked from
`say /class` (`menu=1`) or given by a mode; `default=1` gives it to the team's classless players.
Applied at spawn (or at once in the first 15 s of a round).
- Forward: `ls_on_class(id, name[])` *(handled: refused)*.
- Natives: `ls_class_set(id, name[])`, `ls_class_get`, `ls_class_menu(id)`, `ls_class_find(name[])`.
- Bots: `profile <bot> <bot_profile>` when the key is set.

### ls_meeting_point
A player inside holds USE for 1 s (`calls` per round, after `cooldown`): everyone alive gathers in a
circle around it, frozen, for `duration` s; a vote menu (`say /vote`) opens after 3 s; the most votes,
if more than "skip" and no tie, is voted out (killed, no frag lost).
- Keys: area keys, `duration` (45), `cooldown` (60), `calls` (1), `callers` (1 = only the mode),
  `freeze`, `gather`, `target` (fires at the end, activator = who went).
- Forwards: `ls_on_meeting_start(ent, caller)`, `ls_on_meeting_vote(ent, voter, target)`, `ls_on_meeting_end(ent, ejected)`.
- Natives: `ls_meeting_call(ent, caller)`, `ls_meeting_end`, `ls_meeting_active`, `ls_meeting_vote(voter, target)` (bots vote through this).
- Bots: `goal all meeting` for the length of the meeting.

## Any component (natives)

`ls_comp_create`, `ls_comp_remove`, `ls_comp_is`, `ls_comp_kind`, `ls_comp_find(classname, start)`
(entity order), `ls_comp_by_name`, `ls_comp_count`, `ls_comp_get/getf/set` (set re-reads the key at
once), `ls_comp_enable/enabled`, `ls_comp_start/stop(ent|0)`, `ls_comp_reset(ent|0)`,
`ls_comp_load_pack(path under configs/)`, `ls_comp_inside(ent, id)`, `ls_comp_brain()`,
`ls_comp_bots_hint(line)`, `ls_give_item(id, item)`, `ls_comp_state(ent)` (crate full/empty, spawner there/taken, extraction open, barricade standing, supply crate falling/landed/opened). Forward `ls_on_component(ent, kind[])` when one is ready.

## Example: King of the Hill in a few lines

```pawn
#include <amxmodx>
#include <loner>          // brings ls_components.inc

new g_score[33];

public plugin_init() { ls_register_mode("koth", "King of the Hill", "1.0", "me"); }

public ls_on_capture(ent, side, id)   { ls_announce(0, 3.0, "New king!"); }
public ls_on_capture_tick(ent, side, id)
{
	if (id && ++g_score[id] >= 120)
		ls_announce(0, 5.0, "We have a winner!");
}
```
with `ls_capture_point x y z ffa=1 radius=160` in the map's pack.

## Tested

On a dedicated server (CrossOver HLDS clone, de_dust2, 10 bots, a test pack plus components made
from a test plugin through the natives): capture with contest and ticks, zone shrink with crossing
and damage ticks, weapon spawner pickups, crates opened by bots, supply drop announce/land/open with
items, barricade broken by damage, extraction (BarTime, action none), a class with weapons/HP/speed,
waves respawning dead T bots and clearing, round resets; on cs_assault: a timed door found by brush
model opening and closing on time, a trap fired with victims, checkpoints and a lap, a meeting called,
voted on and a bot voted out. Not yet: platforms, the beams' and HUD's look on a real client, the radar
trick, and `ls_mode` with W2's brain.
