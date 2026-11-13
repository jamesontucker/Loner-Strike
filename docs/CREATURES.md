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
| `Entity()` | the edict behind it (null once it's gone) |
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
- **Animation.** Sequences by name from the type's `anim_*` keys. Otherwise by activity (ACT_IDLE/WALK/RUN/MELEE_ATTACK1/
  SMALL_FLINCH/DIESIMPLE with fallbacks), which covers the HL models and `hostage.mdl`, then player-model names. Walk and run
  play back scaled to the real speed.
- **Death.** The death animation holds its last frame and the body stays `corpse_time` seconds. A creature from a command is
  then removed. One placed in the map (or `persist 1`) hides until its `respawn` or the next round. Map-placed creatures reset
  every round (`FCAP_MUST_RESET`); command-spawned ones are released at round restart (`FCAP_MUST_RELEASE`).
- **Limits.** `ls_creature_max` (48) alive creatures. A spawn is refused when fewer than 100 edicts are free.

## 2. The data file: creatures.ini

Path: cvar `ls_creature_file`, default `addons/amxmodx/configs/gamemodes/creatures.ini` (shipped from
`plugins/configs/gamemodes/creatures.ini`; the file is optional). The fork has built-in types, and the file adds new ones or
overrides them by name. `ls_creature reload` re-reads the file, but new models and sounds wait for the next map. Comments
start with `;`, `#` or `//`.

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
frags = 1                   ; score for the killer
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
| `zombie` | infected | 100 hp, 230/80 speed, melee 20, wanders |
| `runner` | infected | 60 hp, 300 speed, melee 10 fast, chases (horde) |
| `brute` | infected | 450 hp, slow, melee 40, scale 1.25, red tint |
| `spitter` | alien | ranged 8 @ 900, flees at 30 %; `bullsquid.mdl` |
| `hound` | wildlife | territorial (aggression 0.35, guard 350), flees at 35 %; `houndeye.mdl` |
| `turret` | mechanical | doesn't move, ranged 6 × 4/s @ 1200, all-round sight; `sentry.mdl` |
| `boss` | boss | 3000 hp, melee 60, chases, scale 1.6, purple tint |

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
| `spawn <type> <x> <y> <z> [yaw] [key=value ...]` | prints the entity index and sets cvar `ls_creature_last` |
| `kill <entity\|all\|type>` | dies (death animation, log line) |
| `remove <entity\|all\|type>` | gone at once |
| `target <entity\|all\|type> <entity index\|0>` | attack this player (0 = choose itself) |
| `behave <entity\|all\|type> <idle\|wander\|patrol\|guard\|chase\|flee> [x y z]` | new behaviour (and home) |
| `adopt <classname> [type]` | register someone else's monsters (see §5) |
| `list` / `types` / `reload` | |

Cvars: `ls_creature_ver` (1, for detection), `ls_creature_file`, `ls_creature_precache`, `ls_creature_max`,
`ls_creature_budget`, `ls_creature_debug` (1 beams for mode/target, 2 path debug, 3 console lines), `ls_creature_last`,
`ls_creature_alive`.

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
rescan while game events flow) as an `LsAdoptedCreature`:
- alive = `deadflag == DEAD_NO && health > 0 && takedamage`
- health from `pev`
- hostility and category from the given type
- `Kill()` through `TakeDamage`
- `SetTarget` and `SetBehaviour` are hints it can't take, because its own schedules run it

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
