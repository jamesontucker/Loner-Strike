# Creatures

The Creature Framework lives in the ReGameDLL fork, `regamedll/dlls/creature/` (branch `ls-creatures`). It gives game modes
creatures to fight or run from, such as zombies, aliens, wildlife and turrets. Bots treat them as enemies, and plugins and the
Studios can create and control them. If no creature is spawned, the game is stock: the precache list stays empty, nothing extra
thinks, and the only added work is one `LsCreaturesExist()` check per bot game event.

Plan background: GAMEMODES-PLAN.md §6 and the addendum "the Creature Framework and Creature Studio".

## 1. The interface

Every creature is registered in one list and reached through `ILsCreature` (`creature.h`). This holds whoever provides it:

| Method | Meaning |
|---|---|
| `Entity()` | the entity, for identity (null once it's gone); for adopted ones only a pointer to compare, never call into it |
| `Vars()` | its entvars: the safe way to read position, health etc. for any provider |
| `Provider()` | `"ls"` (ours) or `"adopted"` (someone else's, such as MonsterMod's) |
| `Type()`, `Category()` | the type from creatures.ini; category: infected, alien, wildlife, mechanical, boss, special |
| `IsAlive()`, `Health()` | |
| `SetTarget(ent)` / `GetTarget()` | whom to attack (null = let it choose) |
| `SetBehaviour(b, spot)` / `GetBehaviour()` | idle, wander, patrol, guard, chase, flee; `spot` = new home |
| `Threat()` | 0..1, so a bot can choose whom to shoot first (damage × rate; ranged and bosses rate higher) |
| `IsHostileTo(player)` | would it attack this player (alive T/CT, filtered by `hates`) |
| `LastNoise(&pos, &when, &range)` | the last sound it made, for bot hearing |
| `Kill()` | dies as if killed by the world |

Providers implement `ILsCreatureProvider` (`Name`, `Available`, `Spawn(type, origin, yaw, keys)`). Registry functions:
`LsCreatureRegister/Unregister`, `LsCreatureCount()` + `LsCreatureAt(i)` (returns null for gone ones, so iterate and skip
nulls), `LsCreatureOf(ent)`, `LsCreaturesExist()`, `LsCreatureSpawn(type, origin, yaw, keys)`.

### The native provider: `ls_creature`
- **`CLsCreature : CBaseMonster`** is the entity, classname `ls_creature`. It has `FL_MONSTER`, is `SOLID_SLIDEBOX`, takes
  damage with hit groups (head ×3, from `CBaseMonster::TraceAttack`) and bleeds per type. `pev->netname` is the type name and
  `m_hEnemy` is its target. Creatures don't hurt each other.
- **`CCreatureImprov : CImprov`** handles locomotion. It is the hostage AI's movement (`CHostageImprov`) written as a sibling
  class, so hostages are untouched. It follows nav mesh paths through `CNavPathFollower` with `HostagePathCost`, and handles
  ladders, ledge jumps (the hostage's three look-aheads), obstacle feelers, wiggling when stuck, and step lifting on touch. It
  pushes doors, smashes `func_breakable`s in the way, and keeps a little room from players and other creatures. It uses the
  type's walk and run speeds. When the goal is close and visible it goes straight there, and with no nav mesh at all it always
  goes straight (a failure after 2.5 s without headway).
- **Senses.**
  - Sight: within `detect_radius` and inside `detect_fov`. When alert (after someone, searching, or hurt in the last 5 s) it
    sees all round at 1.5× the radius. Walls and smoke block sight, glass doesn't.
  - Noticing takes 0.15–1 s depending on distance; it is instant up close or when alert.
  - Hearing: the bots' own table (`IsGameEventAudible`) times the type's `hearing`. A bullet landing within 150 units also
    alerts it.
- **Behaviours.**
  - Home: idle, wander (around home), patrol (a `path_corner` chain with each corner's `wait`; without one it wanders the
    nav areas within `radius`), guard (holds its spot and fights only near it), chase (a hunter that always knows where the
    nearest enemy is: horde mode).
  - Then: chase → melee attack (a wind-up of `attack_delay`, then a hit if the target is still in reach and in sight, with
    view punch) or ranged attack (a trace with `spread`, a tracer, and damage to whatever it hits).
  - Search: it goes to where it last saw or heard someone and looks around.
  - Flee: when below `flee_hp`, or when passive (`aggression 0`) and someone comes close or hurts it. It picks the nav area
    that best puts distance between it and the threat, then comes back after a cooldown.
  - Retaliation: a hit always tells it where the shooter is.
- **Animation.** Sequences come from the model's own table: the type's `anim_*` name first, then by activity (weighted
  as the model asks), then player-model names. `ls_creature_debug 4` prints each choice.
  - Idle, walk and run are resolved once per model, with their ground speeds. A model without a run (the HL zombie)
    plays its walk faster instead.
  - Walk and run play back at the real speed ÷ the sequence's ground speed (0.3–4×), so feet match the ground. There
    is a small margin before switching between idle and moving.
  - Attacks: melee uses ACT_MELEE_ATTACK1/2. Ranged types use ACT_RANGE_ATTACK1 first, which is the bullsquid's spit
    `range`, not its tail `whip`.
  - A swing plays once, from frame 0, sped up to fit the attack rate (1–3×). The creature stands still for it: no
    sliding at the target mid-swing. A target stepping well out of reach breaks the swing off.
  - A flinch plays once and holds the creature still, at most once every 2 s.
  - Death plays once and holds its last frame.
- **Crowds.** Creatures pass through each other: `pfnShouldCollide` returns 0 only for a pair of `ls_creature`s, and
  always 1 when no creature exists, so stock behaviour is untouched.
  - Before this, one creature stopped in a doorway (a spitter shooting from the B doors) jammed everyone behind it.
    Two spawned close together could end up inside each other with neither able to move.
  - The personal-space push (40 units) still keeps them visually apart.
  - A spawn spot inside a player or a wall moves to the nearest free spot within 128 units that it can walk to.
- **Hulls.** Never wider than a player (32). The engine moves a monster with the player hull that fits its size, and
  anything over 36 wide gets the 64×64×64 "large" hull. That hull doesn't fit through doors the nav mesh (made for
  players) leads through, which is how the bullsquid jammed. Wider hulls in the file are narrowed with a console note.
- **Stuck watch** (once a second, while it is trying to go somewhere and isn't swinging):
  - 2 s without 24 units of headway: hop and shove sideways.
  - 4 s: a fresh path and a shove backwards.
  - 7 s, then every 4 s: back to the newest breadcrumb (a spot it walked through lately, recorded every ~100 units)
    that is free and that nobody can see.
  - Past 15 s with no usable breadcrumb: back to its spawn spot, again only if nobody can see it go or arrive.
- **Death.** The death animation holds its last frame and the body stays `corpse_time` seconds. A creature from a command is
  then removed. One placed in the map (or `persist 1`) hides until its `respawn` or the next round. Map-placed creatures reset
  every round (`FCAP_MUST_RESET`); command-spawned ones are released at round restart (`FCAP_MUST_RELEASE`).
- **Limits.** `ls_creature_max` (48) alive creatures. A spawn is refused when fewer than 100 edicts are free.
- **Cost.** Sight traces go only to the nearest 4 hostile players in range, plus the current target. All creatures share
  a budget of 2 path searches per frame; the rest wait a tick. A goal with no path backs off 1, 2, 4, then 8 s before the
  next search. The "straight at it" visibility check runs 5 times a second.
- **Input checks.** Numbers from the file, keys and commands must be finite and are clamped (speeds ≤ 2000, radii ≤ 8192,
  damage ≤ 10000, health ≤ 100000). Yaw is folded into ±360. Positions outside ±8192 are refused. A bad hull (min ≥ max,
  bottom above 0, top under 30, over 256 wide) falls back to the default hull with a console warning.
- **Round reset.** It relies on `REGAMEDLL_FIXES` (`UTIL_ResetEntities`). The `PLAY_GAMEDLL` test configs lack it, so
  there map creatures don't reset each round and command-spawned ones aren't released.

## 2. The data file: creatures.ini

Path: cvar `ls_creature_file`, default `addons/amxmodx/configs/gamemodes/creatures.ini` (shipped from
`plugins/configs/gamemodes/creatures.ini`; the file is optional). The fork has built-in types, and the file adds new ones or
overrides them by name. `ls_creature reload` re-reads the file, but new models and sounds wait for the next map. A type
keeps its slot for the whole server run, so live creatures keep their type. New names are appended. A name the file
dropped is disabled (no new spawns), not removed. Comments start with `;`, `#` or `//`.

```ini
[runner]                    ; type name
base = zombie               ; start from another type (optional)
name = Runner               ; display name
category = infected         ; infected | alien | wildlife | mechanical | boss | special
hp = 60
speed = 300                 ; running
walk_speed = 110
aggression = 1              ; 0 never starts a fight (fights back, or flees if 0), 1 attacks anyone it notices
detect_radius = 1200
detect_fov = 160            ; degrees, 360 = all round
hearing = 1                 ; times the bots' hearing ranges, 0 = deaf
attack = melee              ; none | melee | ranged
damage = 10
attack_range = 64
attack_rate = 1.6           ; attacks a second
attack_delay = 0.25         ; wind-up before it lands
spread = 3                  ; ranged: degrees
flee_hp = 0                 ; flee below this part of its health
behaviour = chase           ; idle | wander | patrol | guard | chase | flee
radius = 400                ; guard/wander/patrol range from home
target = nearest            ; nearest | weakest | attacker | random
hates = all                 ; all | t | ct
respawn = 0                 ; seconds after death it comes back at home (0 = never)
corpse_time = 8
moves = 1                   ; 0 = a turret
frags = 1                   ; score for the killer (only with ls_creature_frags 1: off by default, so mp_fraglimit isn't hit by creatures)
money = 0                   ; money for the killer
model = models/zombie.mdl models/hostage.mdl   ; candidates: the first file that exists wins
skin = 0
body = 0
scale = 1
hull_min = -16 -16 0
hull_max = 16 16 72
glow = 0 0 0                ; always tinted (r g b; 0 0 0 = none)
fallback_glow = 160 160 20  ; tinted only when a later model candidate was used (a stand-in)
blood = red                 ; red | yellow | none
anim_idle =                 ; sequence names, empty = by activity
anim_walk =
anim_run =
anim_attack = attack1
anim_flinch =
anim_die =
snd_idle = zombie/zo_idle1.wav zombie/zo_idle2.wav | player/pl_pain2.wav   ; "a b | c": the first group with a file present
snd_alert =
snd_attack =
snd_hit =
snd_miss =
snd_pain =
snd_die =
ls = model=models/player/zombie_source/zombie_source.mdl anims=walk run attack1 die   ; the plan's backend form also works
mm = monster_zombie         ; ignored by the fork (the kit's MonsterMod mapping)
bot = zombie_classic        ; ignored by the fork (the kit's bot-zombie fallback)
```

Built-in types:

| Type | Category | Notes |
|---|---|---|
| `zombie` | infected | 100 hp, 230/80 speed, melee 20, wanders; `zombie.mdl` (walk only: runs by playing it faster) |
| `runner` | infected | 60 hp, 300 speed, melee 10 fast, chases (horde); `houndeye.mdl` (a real run) |
| `brute` | alien | 450 hp, slow, melee 40; `agrunt.mdl` (alien grunt) |
| `spitter` | alien | ranged 8 @ 900, flees at 30 %; `bullsquid.mdl` |
| `hound` | wildlife | territorial (aggression 0.35, guard 350), flees at 35 %; `houndeye.mdl` |
| `turret` | mechanical | doesn't move, ranged 6 × 4/s @ 1200, all-round sight; `sentry.mdl` |
| `boss` | boss | 3000 hp, melee 60, chases; `garg.mdl` at scale 0.7 (the hull stays player-sized) |
| `headcrab` | infected | 25 hp, small (36 tall), melee 8 by leaping (`anim_attack jump`); `headcrab.mdl` |

**Models.** Each type lists Half-Life's own model first (`valve/models/zombie.mdl` and the like, from the shared depot most CS
installs have; the engine finds `valve/` files from `cstrike`). If that file is missing, `models/hostage.mdl` stands in with a
`fallback_glow` tint, so a mode always plays. The choice is made per map at precache (`ls_creature types` shows "stand-in").
Clients that lack the HL model see an error model; we never put Valve's `.mdl`s on FastDL. Own models for 1.0.0 (§6.4) go
in as the first candidate.

## 3. Spawning and commands

**Precache rule.** The engine only precaches while a map loads. A type can be spawned mid-map only if one of these is true:
- it is listed in `ls_creature_precache` (`""` none = stock, `all`, or `zombie runner ...`), set before the map loads (the
  mode cfg, or `plugin_precache`), or
- an `ls_creature` of that type is placed in the map.

Otherwise the spawn is refused with a message. The fork counts its own precaches and warns past `ls_creature_budget` (40
models + sounds, out of the engine's 512/512). On ReHLDS, `rescount` shows the whole picture.

**Map entity** `ls_creature` (point). Keys: `type`, `behaviour`, `path` (first `path_corner` of a patrol), `radius`,
`respawn`, `health`, `speed`, `damage`, `aggression`, `detect`, `hates`, `persist`, `angles`. The same keys work after
`spawn` on the command line and through `DispatchKeyValue`.

**Server command** `ls_creature`:

| Command | |
|---|---|
| `spawn <type> <x> <y> <z> [yaw] [key=value ...]` | only while a map is running; prints the entity index and sets cvar `ls_creature_last` |
| `kill <entity\|all\|type>` | dies (death animation, log line); adopted ones are refused |
| `remove <entity\|all\|type>` | gone at once; adopted ones are refused |
| `target <entity\|all\|type> <entity index\|0>` | attack this player (0 = choose itself) |
| `behave <entity\|all\|type> <idle\|wander\|patrol\|guard\|chase\|flee> [x y z]` | new behaviour (and home) |
| `adopt <classname> [type]` | register someone else's monsters (see §5) |
| `list` / `types` / `reload` | |

Cvars: `ls_creature_ver` (1, for detection), `ls_creature_file`, `ls_creature_precache`, `ls_creature_max`,
`ls_creature_budget`, `ls_creature_debug` (1 beams for mode/target, 2 path debug, 3 console lines), `ls_creature_last`,
`ls_creature_alive`, `ls_creature_frags` (0: kills don't add score).

Waves are planned for later: `ls_wave_spawner` in gm_components drives `spawn` lines.

## 4. Events for plugins (AMXX)

The brain talks to plugins through server commands and cvars only (`ls_bot`, `ls_mode`, `ls_sites`); there are no custom
forwards. Creatures use the same channels plus what AMXX already has:
- **Hamsandwich** on the class: `RegisterHam(Ham_TakeDamage, "ls_creature", ...)`, `Ham_Killed`, `Ham_TraceAttack`.
  The entity has the standard `CBaseMonster` vtable. Read the type with `pev(ent, pev_netname, ...)`.
- **Log line** on every death, for `register_logevent`:
  `"Name<uid><auth><TEAM>" triggered "ls_creature_killed" (creature "runner") (entity "123")`, or
  `World triggered "ls_creature_killed" (...)`.
- **Cvars** `ls_creature_last` (the index after a `spawn`) and `ls_creature_alive`.
- Creating one from a plugin: `create_entity("ls_creature")`, `DispatchKeyValue(ent, "type", "runner")`, set the origin,
  `DispatchSpawn(ent)`. Check `pev_flags & FL_KILLME` / `is_valid_ent` afterwards (an unknown type or one that isn't
  precached removes itself).
- The kit (gm_components / loner.inc) should wrap these as `ls_on_creature_hurt/killed` forwards and `ls_creature_spawn()`.
  It can also pick the backend in order `ls mm bot` (§6.1).

## 5. Bots

Coordinated with W2 (TOOLS-NOTES.md, "Agent Y: Creatures"):
- `ls_mode enemyclass ls_creature [threat]` works now. The entity is a normal damageable monster.
- Proposed: `ls_mode enemyclass @creatures`, or always on when `LsCreaturesExist()`. The brain would iterate the registry
  and use `IsHostileTo` / `Threat` / `LastNoise`. That covers adopted MonsterMod monsters and passive or team-specific
  creatures.
- Creatures treat bots as players: they see, hear, chase and hit them. Their sounds are recorded per creature
  (`LastNoise`) rather than sent as bot game events, because stock event code assumes a player source.

## 6. MonsterMod Redo through an adapter

MonsterMod is a Metamod plugin with its own monster code, so its entities are not `CLsCreature`s. We don't assume they fit;
we wrap what they expose. `ls_creature adopt monster_zombie zombie` registers every `monster_zombie` (now, and on a 1 s
rescan while game events flow) as an `LsAdoptedCreature`. It finds them through the engine's own search, since other
modules' entities aren't in our classname hash, and it holds the edict plus its serial number:
- Its private data is another module's classes, so it reads only the edict's entvars and never calls a ReGameDLL virtual
  on it.
- alive = `deadflag == DEAD_NO && health > 0 && takedamage`; health comes from the entvars.
- Hostility and category come from the given type.
- `kill` and `remove` are refused with a message (use MonsterMod's own commands). `SetTarget` and `SetBehaviour` are
  no-ops, because its own schedules run it.
- `adopt` refuses players, `worldspawn`, our own classes, hostages, grenades and items, and the `weapon_`, `item_`,
  `ammo_`, `func_`, `info_`, `trigger_`, `env_`, `light`, `path_`, `game_` and `multi` prefixes.
- Not yet tested against MonsterMod: if its entities have no private data at all, they drop out of the list.

So bots, the list, `ls_creature_alive` and the kit treat MonsterMod monsters like ours. Spawning them stays MonsterMod's own
`monster` command, which the kit's `mm` backend issues and then follows with `adopt`. A fuller adapter would be a small
Metamod shim that forwards MonsterMod's spawn/kill to the registry and maps target/behaviour to its `m_hEnemy`/schedules.
That only makes sense after the MonsterMod spike (§6.3) confirms it runs under ReGameDLL.

## 7. What's left

- **Creature Studio UI** (Tools > Make): a form over creatures.ini types, plus spawn groups and waves placed through the editor's
  component library. Custom behaviours come later via Game Mode Studio nodes.
- **Bosses**: phases, ground pound (ScreenShake + radius damage), summons. Only a big melee type for now.
- **Flying and swimming** creatures (no nav mesh in the air), teleporting, support and summoning specials.
- **Turrets**: real yaw/pitch bone controllers and a deploy animation; now the whole model turns.
- **Breaking barricades on purpose**: attack the blocking `func_breakable`/`ls_barricade` on a path failure. Now only what's
  right in front gets smashed.
- **Projectiles** for spitters (a visible glob with travel time), and gibs.
- Our own creature models (4 + boss) as first candidates.
- Bot side: W2's `@creatures` and the aim model on entity targets (`gm_humanbots`).
- **Testing**: the code compiles (syntax-checked with g++ 12; CI builds win32/linux32) but hasn't been run in game yet. Check
  movement on stock maps with and without a nav mesh, HL model animations by activity, and the precache warnings.

## 7b. Measured: crowds, stuck and animations (2026-11-12)

Sim copies of the playtest server (`~/loner-strike-sim/creatures/`: `ls_crec.amxx` records every creature 4×/s,
`crec.py` and `reach.py` read the recordings, `pathrun.sh` and `doorway.sh` are the tests).

- **Cause of the playtest jam.** The spitter's 48×48×40 hull put it on the engine's 64-unit hull. It sat jumping in
  place for 25.6 s near T spawn (-424 -232) and couldn't pass dust2's doors. Creatures also collided with each other,
  so one stopped in a choke held the rest, and two spawned close together could end up inside each other.
- **Pathing benchmark** (de_dust2 Last Stand, wave 1 zombies, defenders frozen and not shooting, so only creature
  movement counts; ~7 min each):

| | stuck (no headway 2.5 s+, not at a barricade or a player) | longest | reached the base | T-spawn lane median / p90 | B lane median / p90 |
|---|---|---|---|---|---|
| before (ls-next r43348308295) | 2.5 % of alive time (62 s) | 36.2 s | 53/54 | 16.5 / 19.5 s | 21.5 / 24.0 s |
| after (ls-creatures r43361583332) | 0.1 % (2 s) | 1.1 s | 36/36 | 16.5 / 21.3 s | 22.0 / 26.4 s |

- **Doorway test** (`doorway.sh`: a spitter in the B double doors, two zombies right behind it). Before: one zombie never
  left its spawn spot (20 s+, jammed) and the spitter attacked with its tail-whip sequence. After: both zombies were
  past the spitter within ~2 s and at the barricade; the spitter spits with `range`.
- **Swings.** Before: zombies moved at a median 19–24 u/s mid-swing, and 20–27 % of swing samples were faster than
  60–100 u/s (the "sliding at you in a superman pose"). After: median 13 u/s. The samples still over 60 are almost all
  the swing's first frame, where the 0.25 s sample window still holds the run-up; about 3 % are later in the swing.
- **Walk/run feet vs ground:** about 10–16 % off at the median, before and after. The HL zombie has only a walk (72
  u/s), played at up to 4×.

## 8. Model sequence tables (for `anim_*` keys)

The sequences the built-in types' models have: index, name, activity (weight when it's above 1), fps, frames, whether it
loops, and ground speed (units/s at framerate 1). `ls_creature` picks by activity, so a row without one is only reachable
by name. `sentry.mdl` has no activities, so the turret names its sequences (`spin`, `fire`, `die`). Dumped with
`~/loner-strike-sim/creatures/mdlseq.py` from `valve/models` and `cstrike/models`.

**zombie.mdl** (44 sequences; only those with an activity, plus walk/run names)

| # | name | activity | fps | frames | loops | ground speed |
|---|---|---|---|---|---|---|
| 0 | idle1 | IDLE (w 4) | 15 | 31 | yes | 0 |
| 1 | turnleft | TURN_LEFT | 20 | 16 | no | 0 |
| 2 | turnright | TURN_RIGHT | 20 | 16 | no | 0 |
| 3 | flinchsmall | SMALL_FLINCH (w 5) | 22 | 7 | no | 0 |
| 4 | flinch | SMALL_FLINCH | 22 | 21 | no | 0 |
| 5 | bigflinch | BIG_FLINCH | 20 | 31 | no | 0 |
| 7 | falling | FALL | 26 | 21 | yes | 0 |
| 8 | attack1 | MELEE_ATTACK1 (w 5) | 15 | 36 | no | 0 |
| 9 | attack2 | MELEE_ATTACK1 | 15 | 21 | no | 0 |
| 10 | walk | WALK | 22 | 60 | yes | 72 |
| 11 | laflinch | FLINCH_LEFTARM | 15 | 16 | no | 0 |
| 12 | raflinch | FLINCH_RIGHTARM | 15 | 16 | no | 0 |
| 13 | llflinch | FLINCH_LEFTLEG | 15 | 16 | no | 0 |
| 14 | rlflinch | FLINCH_RIGHTLEG | 15 | 16 | no | 0 |
| 15 | dieheadshot | DIE_HEADSHOT | 20 | 27 | no | 0 |
| 16 | dieheadshot2 | DIE_GUTSHOT | 22 | 29 | no | 0 |
| 17 | diesimple | DIESIMPLE | 20 | 21 | no | 0 |
| 18 | diebackward | DIEBACKWARD | 18 | 31 | no | 0 |
| 19 | dieforward | DIEFORWARD | 18 | 31 | no | 0 |

**bullsquid.mdl** (24 sequences; only those with an activity, plus walk/run names)

| # | name | activity | fps | frames | loops | ground speed |
|---|---|---|---|---|---|---|
| 0 | walk | WALK | 30 | 150 | yes | 51 |
| 1 | run | RUN | 30 | 37 | yes | 299 |
| 2 | suprisedhop | HOP | 42 | 41 | no | 0 |
| 3 | flinchs | SMALL_FLINCH | 30 | 8 | no | 0 |
| 4 | flinchb | BIG_FLINCH | 30 | 17 | no | 0 |
| 5 | turnleft | TURN_LEFT | 30 | 21 | no | 0 |
| 6 | turnright | TURN_RIGHT | 30 | 21 | no | 0 |
| 7 | idle | IDLE | 30 | 151 | yes | 0 |
| 8 | whip | MELEE_ATTACK1 | 30 | 55 | no | 0 |
| 9 | bite | MELEE_ATTACK2 | 30 | 29 | no | 0 |
| 10 | range | RANGE_ATTACK1 | 30 | 42 | no | 0 |
| 11 | look | INSPECT_WALL | 20 | 116 | no | 0 |
| 12 | seecrab | EXCITED | 30 | 101 | no | 0 |
| 13 | eat | EAT | 30 | 101 | yes | 0 |
| 14 | inspectdown | INSPECT_FLOOR | 30 | 201 | no | 0 |
| 15 | sniff | SNIFF | 15 | 57 | no | 0 |
| 16 | die | DIEFORWARD | 15 | 11 | no | 0 |
| 17 | die1 | DIESIMPLE | 20 | 76 | no | 0 |

**houndeye.mdl** (31 sequences; only those with an activity, plus walk/run names)

| # | name | activity | fps | frames | loops | ground speed |
|---|---|---|---|---|---|---|
| 0 | idle1 | IDLE | 30 | 46 | no | 0 |
| 1 | idle2 | IDLE (w 20) | 30 | 101 | yes | 0 |
| 2 | idle3 | IDLE | 30 | 61 | no | 0 |
| 3 | run | RUN (w 4) | 30 | 19 | yes | 410 |
| 4 | run | RUN (w 2) | 30 | 19 | yes | 410 |
| 5 | run | RUN | 30 | 19 | yes | 410 |
| 6 | die | DIESIMPLE | 20 | 11 | no | 0 |
| 7 | die1 | DIEFORWARD | 20 | 19 | no | 0 |
| 8 | die2 | DIEBACKWARD (w 2) | 20 | 17 | no | 0 |
| 9 | die3 | DIEBACKWARD | 20 | 17 | no | 0 |
| 10 | attack | RANGE_ATTACK1 | 25 | 60 | no | 0 |
| 11 | flinch_small | SMALL_FLINCH | 30 | 19 | no | 0 |
| 12 | flinch_small2 | SMALL_FLINCH | 30 | 10 | no | 0 |
| 13 | die_crumple | DIESIMPLE | 30 | 31 | no | 0 |
| 15 | walk2 | WALK | 30 | 36 | yes | 33 |
| 16 | leaderlook | GUARD | 30 | 121 | no | 0 |
| 17 | sleep | CROUCHIDLE | 30 | 41 | yes | 0 |
| 18 | gotosleep | CROUCH | 30 | 301 | no | 0 |
| 19 | sleeptostand | STAND | 30 | 24 | no | 0 |
| 20 | madidle | IDLE_ANGRY (w 2) | 30 | 31 | yes | 0 |
| 21 | madidle2 | IDLE_ANGRY (w 3) | 37 | 46 | yes | 0 |
| 22 | madidle3 | IDLE_ANGRY | 30 | 31 | yes | 0 |
| 23 | inspect | INSPECT_WALL | 10 | 61 | no | 0 |
| 24 | eat | EAT | 30 | 248 | no | 0 |
| 25 | 180l | TURN_LEFT | 30 | 21 | yes | 0 |
| 26 | 180r | TURN_RIGHT | 30 | 21 | yes | 0 |
| 27 | jumpback | LEAP | 30 | 29 | no | 0 |
| 28 | wakefast | HOP | 30 | 24 | no | 0 |

**sentry.mdl** (6 sequences; only those with an activity, plus walk/run names)

| # | name | activity | fps | frames | loops | ground speed |
|---|---|---|---|---|---|---|
| 0 | idle_off | 0 | 1 | 1 | no | 0 |
| 1 | fire | 0 | 20 | 17 | yes | 0 |
| 2 | spin | 0 | 20 | 17 | yes | 0 |
| 3 | deploy | 0 | 20 | 8 | no | 0 |
| 4 | retire | 0 | 10 | 8 | no | 0 |
| 5 | die | 0 | 15 | 21 | no | 0 |

**hostage.mdl** (129 sequences; only those with an activity, plus walk/run names)

| # | name | activity | fps | frames | loops | ground speed |
|---|---|---|---|---|---|---|
| 0 | walk | WALK | 20 | 17 | yes | 59 |
| 1 | walk_scared | 63 | 20 | 17 | yes | 42 |
| 2 | run | RUN | 25 | 19 | yes | 275 |
| 3 | run1 | 64 | 30 | 18 | yes | 316 |
| 4 | run2 | 64 | 30 | 18 | yes | 316 |
| 5 | 180_Left | TURN_LEFT | 30 | 13 | yes | 0 |
| 6 | 180_Right | TURN_RIGHT | 15 | 13 | yes | 0 |
| 7 | flinch | BIG_FLINCH | 30 | 31 | no | 0 |
| 8 | flinch1 | SMALL_FLINCH | 20 | 9 | no | 0 |
| 9 | laflinch | FLINCH_LEFTARM | 15 | 9 | no | 0 |
| 10 | raflinch | FLINCH_RIGHTARM | 15 | 9 | no | 0 |
| 11 | llflinch | FLINCH_LEFTLEG | 15 | 16 | no | 0 |
| 12 | rlflinch | FLINCH_RIGHTLEG | 15 | 16 | no | 0 |
| 13 | idle1 | IDLE (w 50) | 18 | 31 | yes | 0 |
| 14 | idle3 | IDLE | 15 | 21 | no | 0 |
| 15 | idle4 | IDLE (w 2) | 15 | 21 | no | 0 |
| 16 | idle5 | IDLE | 15 | 21 | no | 0 |
| 17 | idle6 | IDLE | 15 | 21 | no | 0 |
| 18 | idle7 | IDLE (w 10) | 10 | 31 | no | 0 |
| 19 | crouchstand | STAND | 18 | 22 | no | 0 |
| 20 | crouch_idle | CROUCHIDLE | 18 | 11 | yes | 0 |
| 21 | crouch_idle2 | CROUCHIDLE | 18 | 16 | yes | 0 |
| 22 | crouch_idle3 | CROUCHIDLE | 18 | 16 | yes | 0 |
| 23 | crouch_idle3 | CROUCHIDLE | 18 | 16 | yes | 0 |
| 24 | panic | EXCITED | 18 | 21 | no | 0 |
| 25 | fear1 | 58 | 18 | 56 | no | 0 |
| 26 | fear2 | 58 | 18 | 41 | no | 0 |
| 27 | eye_wipe | 58 | 18 | 51 | no | 0 |
| 28 | pull_needle | 33 | 15 | 36 | no | 0 |
| 29 | return_needle | 34 | 15 | 21 | no | 0 |
| 30 | give_shot | MELEE_ATTACK1 | 15 | 24 | no | 0 |
| 31 | diesimple | DIESIMPLE | 24 | 20 | no | 0 |
| 32 | dieforward | DIE_HEADSHOT | 20 | 31 | no | 0 |
| 33 | dieforward1 | DIEFORWARD | 16 | 25 | no | 0 |
| 34 | diebackward | DIEBACKWARD | 15 | 16 | no | 0 |
| 35 | headshot | DIE_HEADSHOT (w 20) | 15 | 22 | no | 0 |
| 36 | gutshot | DIE_GUTSHOT | 15 | 21 | no | 0 |
| 43 | barnacled1 | 40 | 18 | 9 | no | 0 |
| 44 | barnacled2 | 41 | 28 | 41 | yes | 0 |
| 45 | barnacled3 | 42 | 18 | 36 | no | 0 |
| 46 | barnacled4 | 43 | 15 | 36 | yes | 0 |
