# Loner-Strike's own game modes: components, framework, studios and the modes built from them

Written 2026-11-12. Planning only; nothing else in the repo was changed. Companion to
TOOLS-PLAN.md (§5.4 playtesting, §5.6 game-mode creator), TOOLS-EDITOR.md, TOOLS-NOTES.md and the
four research notes in docs/research/ (existing-modes.md, big-maps.md, client-limits.md,
monstermod.md), which §16 summarises.

Paths: `REPO` = this repository, `FORK` = a checkout of the ReGameDLL_CS fork
(jamesontucker/ReGameDLL_CS), `APP` = `REPO/app`.

---

## 0. Key decisions (the short version)

| Decision | Choice | Why |
|---|---|---|
| The core idea | **A library of reusable gameplay components**: each is a map entity (`ls_*`, FGD-style typed keys) + logic in one shared plugin (`gm_components.amxx`), brain-side only where per-frame bot reasoning needs it + a bot hook so the brain knows what to do with it. Modes are components + rules. | Any mode assembled from components gets working bots, editor support, checks, graph nodes and `loner.inc` calls for free. The user's direction. |
| Where modes live | On the existing gamemodes system as ordinary mode ids (`br`, `ex`, `pred`, `ls`, `tr`, …) with `<id>.cfg`, `<id>.plugins.ini`, `mappresets.ini` prefixes, catalog parts | gamemodes.sma already does switching, per-mode cvars, saved settings, presets, map lists, bot style and the `bot_ls_*` handover. Nothing there needs replacing. |
| Rules: data, graph or code | Three ways to say a mode's rules, all producing the same thing (a mode plugin on top of the kit): **data** (`<id>.mode.ini`, no code), **graph** (Game Mode Studio, compiled to Pawn), **code** (Plugin Studio with `loner.inc`, ~40 lines for a whole mode). Mechanics that need loops, traces or timers live in components, not in rules. | AMXX has no scripting layer; a generic interpreter is the trap TOOLS-PLAN §5.6 warned about. XDM and Cross Product make the same split (§2.6). No Lua (user). |
| Game Mode Studio runtime | **Compile the graph to generated Pawn → amxxpc (already bundled) → a normal mode plugin.** A graph-runner interpreter is not built for the first milestone. | Fits the mode system as-is, performs like any plugin, "open as code" is a file write, debugging rides on the server log. Hot reload is a local-server restart (seconds), good enough. §9.4 |
| How bots learn a mode | `ls_mode` server commands + a `ModeGoal` layer in the brain (beside `LsOrder`/`LsPuppet`), fed by components. `gm_botnav` stays for third-party mods we don't own. | The brain's senses, noise memory, `.lsv` map knowledge and team play are the differentiator; a velocity autopilot throws them away. |
| Creatures | Our own `ls_creature` on a `CCreatureImprov` factored out of `CHostageImprov` (feasible, §6.2). **MonsterMod offered as an optional catalog add-on** ("classic Half-Life monsters"), never bundled; creature modes take either through one interface, with bot-zombies as the always-works fallback. | User decision (coordinator, 2026-11-12). MonsterMod carries Valve SDK code; the HL1 models are in Steam's shared depot so CS-only players most likely have them, verified at install (§6.3). |
| Client | Stock CS 1.6 client only: 4 HUD channels + 8 director slots (one HUD manager), menus, `BarTime`, `StatusIcon`, `RoundTime`, a fixed client beam pool, 192-byte user messages, fade/shake/fog, radar dot tricks, MOTD. | Facts in §3.6 from docs/research/client-limits.md. |
| Player count | 32 (HLDS hard cap); bots fill. BR "20-40" = 20-32. | Engine. |
| Agents | **An MCP connector** in the app (off by default, local, per-session token, read-only unless edits are allowed, destructive actions confirmed in the app) exposing maps, editor, nav, modes/studios, server control. | User request; the app already has a local axum API every page uses, so tools are thin wrappers (§11). |
| Order | Components + kit → Last Stand → Battle Royale → studios → Predator → Extraction → Traitor; Commander/Hunted/Heist/King of the Ramp as follow-ons | Least new bot work first, headline second; the studios need a working kit to compile against. Open question (§15). |

---

## 1. What exists today (verified in the repo and the fork)

### 1.1 The mode system (`REPO/plugins/scripting/gamemodes.sma`, 5376 lines)
- `modes.ini` lists modes (`id "Name" "Description" "options command" "flags"`); each has
  `<id>.plugins.ini` (written to `configs/plugins-gamemode.ini`, loaded on the next map) and
  `<id>.cfg` (executed after `base.cfg`, then saved global/mode settings, then add-on cfgs:
  `apply_all`). `base.cfg` resets every cvar a mode may touch; new modes add theirs there.
- `mappresets.ini` (prefix → mode), `maps/<id>.txt`, `presets.ini`, `votemodes.ini`,
  `botplay.ini` (team/solo/hold per mode or map).
- `STEERED_MODES` / `steered_mode()` turns the brain's fight/solo off for modes a plugin drives
  through `gm_botnav`; `apply_ls_cvars()` has the last word on `bot_ls_*`.
- Add-ons: `addons.ini` + `addons/<id>.ini|cfg`, loaded via `plugins-gmaddons.ini`.
- Catalog (`REPO/catalog/components.json` + `parts/mode-*.json`, `addon-*.json`): `vendor`,
  `fetch` (archive/url + sha256 per file), `patches`, `compile` (sma → amxx with the bundled
  amxxpc: `setup/core.rs::compile_plugin`), `maps` (GameBanana), `dirs`
  (`APP/src-tauri/src/setup/components.rs`). Nothing handles a Metamod plugin DLL yet
  (`foundation.rs` only adds `amxmodx_mm.dll` to `addons/metamod/plugins.ini`).

### 1.2 Bot plumbing
- **Brain** (`FORK/regamedll/dlls/bot/cs_bot_ls*.cpp`, ~12k lines; `cs_bot_surf.cpp` on `ls-surf`):
  cvars `bot_ls_fight|solo|match|routes|lines|pauses|own_memory|trace`; `ls_bot <idx> <verb>` with
  `hold crouch jump look nod goto follow release persona buyfor buyplan throw plant dropbomb push`
  (puppet = a plugin's habit, dropped when a fight starts; `LsOrder` = a person's order, resumed
  after); `LsHeard`/`LsHeardBreak` noise memory; `LsMapKnow` (`maps/<map>.lsv`: sightlines,
  wallbangs, cover, corners, angles, exposure, high ground); team play (`cs_bot_ls_team.cpp`);
  personas from `people.ini` through `gm_people`.
- **gm_botchat** turns chat/voice into `ls_bot` orders and answers from game state: most of
  "Commander" exists. **gm_botnav**: A* on the nav mesh, velocity steering, hiding spots, flood
  distances. **gm_humanbots / gm_botfun / gm_people**: aim model, habits, personas.
- **Hostages** (`FORK/regamedll/dlls/hostage/`): `CHostage : CBaseMonster`, `CHostageImprov :
  CImprov` (`game_shared/bot/improv.h`): `MoveTo/LookAt/FaceTo`, `TrackPath`, ladders, jump, crouch,
  local nav, `IsVisible`, noise/injury/player timers, `Frighten`, idle/follow/escape/retreat states,
  `HostagePathCost`, `KeepPersonalSpace`. `MAX_HOSTAGES` (12) is a `CHostageManager` cap, not an
  improv cap. `monster_scientist` aliases `CHostage`; there are no other HL monsters in CS.
- Mods already patched for CZ bots: zp5, basebuilder, deathrun (`dr_bot_traps`), hns (`hns_bot_*`).

### 1.3 Tools (`APP/src-tauri/src/{goldsrc,tools,editor}`, `APP/ui/src/views/tools`)
- BSP/WAD/nav/.lsv/MDL readers, viewer (three.js, lazy chunk), overlays, replay, health check, map
  doctor, callout editor, nav fixer, spray maker, texture/model browser, **map editor** (Rust core
  with history/checks; UI with box/clip/texture/entity tools; no FGD forms, prefabs or compiler
  yet; SDHLT compile is a decided separate task). All pages talk HTTP to the app's local axum
  server (`server.rs`), which is what the MCP connector wraps.
- `maps.rs` finds `overviews/<map>.bmp|.tga` as a radar thumbnail; nothing writes one.
- Play page launches the game with bots (`/api/app/launch`); the panel API (`srv_mode`, `srv_map`,
  `srv_set`, `srv_status` in gamemodes.sma) controls a running server.

---

## 2. The framework: components + rules ("Mode Kit")

### 2.1 Layers

```
 modes.ini / <id>.cfg / <id>.plugins.ini / mappresets.ini    gamemodes.sma: switching, cvars, menu   (existing)
 ──────────────────────────────────────────────────────────────────────────────────────────────
 Rules (one of):   <id>.mode.ini (data)  |  <id>.graph.json → generated gm_<id>.sma (Studio)  |  gm_<id>.sma (loner.inc)
 Map:              ls_* entities in the BSP, or maps/<map>.<id>.ini (map pack for stock maps)
 ──────────────────────────────────────────────────────────────────────────────────────────────
 gm_components.amxx   every ls_* component: entity lifecycle, state, events (forwards), actions (natives),
                      HUD contributions, bot hooks → ls_mode                                        (new)
 gm_modekit.amxx      mode runtime: definition reader, roles, phases, win conditions, spawn/respawn,
                      equipment/shop, scoring/persistence, HUD manager, scheduler/events, modifiers,
                      bot bridge, creature bridge                                                   (new)
 gm_mk_*.amxx         optional shared features: infection, bounty, race, squads                    (new)
 include/loner.inc    the one include a mode author needs (wraps both plugins)                     (new)
 ──────────────────────────────────────────────────────────────────────────────────────────────
 ReGameDLL fork       ls_mode commands, ModeGoal + nav-cost hooks in the brain, enemy classes in the
                      senses, ls_creature + CCreatureImprov, hiding-place learning                  (new, fork)
 ──────────────────────────────────────────────────────────────────────────────────────────────
 App                  component library in the editor (drag-in, inspectors, 3D visuals, checks),
                      Game Mode Studio (nodes), Plugin Studio (Pawn), mode designer forms, catalog
                      parts, Playtest, overview generator, MCP connector                            (new)
```

Rule of thumb: **a value a server owner might change is data; a behaviour that needs a loop, a
trace or a timer is a component; what makes this mode this mode is its rules** (data, graph or
40 lines of Pawn).

### 2.2 The component contract

Every component is the same five things, which is what lets the editor, the brain, the Studio,
`loner.inc` and the MCP tools treat them uniformly:

1. **Entity**: classname `ls_<name>`, point or brush, typed keys (FGD-style; `catalog/components/
   <name>.json`, also exported as `lonerstrike.fgd`). Custom classnames survive map load through
   the standard fakemeta trick: `FM_KeyValue` sees `classname = ls_*`, swaps it for `info_target`
   (point) or `func_wall` (brush), keeps the keys in a trie, and `FM_Spawn` post restores the
   classname and hands the entity to the component. Map packs (`maps/<map>.<id>.ini`) create the
   same entities at map start for maps that lack them.
2. **Logic**: in `gm_components.amxx` (one plugin, one file per component under
   `scripting/components/*.inc`, included), with state per entity index. **Brain-side** only when
   the behaviour is the bot's own reasoning each frame (what a zone means for where to stand,
   path costs for doors and damage, learned hiding places, creature AI); the plugin still owns the
   rules and tells the brain through `ls_mode`.
3. **Events and actions**: forwards (`ls_cp_captured(ent, team)`) and natives
   (`ls_cp_lock(ent, secs)`) in `loner.inc`; the same list is the Studio's node registry and the
   MCP tool list for modes.
4. **Bot hook**: one of the brain verbs below, set by the component when its state changes, so
   bots react without the mode author writing a line.
5. **Editor**: inspector from the keys, a 3D visual (volume, line, badge), and check rules
   (counts, reachability, placement) the mode's `[map] requires` references.

Brain verbs the hooks use (`ls_mode …`, §4.1): `goal <bot> <kind> <pos> [ent] [prio]` with kinds
`go_hold | defend | capture | extract | loot | use | ride | investigate | flee | meeting`;
`avoid <pos> <r> <cost> [until]` (nav-area cost, e.g. damage zones, outside the safe zone);
`block <ent> <until>` (a closed timed door costs as a wall until then: detour);
`attract <pos> <r> <weight>` (loot, supply drops, capture points worth going to);
`friend/enemyclass/role/evidence/profile/modifier` (§4).

### 2.3 The component library

"Wraps" = built on an existing GoldSrc/CS entity (the plugin spawns or adopts it); "new" = the
plugin implements it from `info_target`/`func_wall` and traces. Logic column: P = plugin only,
P+B = plugin + brain-side part.

| Component (`ls_*`) | Wraps | Keys (main) | Logic | Events → / Actions ← | Bot hook | Editor visual |
|---|---|---|---|---|---|---|
| `ls_capture_point` | new (radius or brush) | radius, capture_secs, team_lock, decay, score_per_sec | P | → captured(team), contested, lost; ← lock, reset, set_owner | `capture`/`defend` for the owning side; `attract` | cylinder volume, owner colour, progress ring |
| `ls_safe_zone` | new | radius, damage_outside (table), warn_secs, shrink_to (targetname), shrink_secs | P+B (where to stand: edge/cover/exposure; when to leave) | → stage_changed, player_outside/inside; ← set(center, r), shrink(next), pause | `avoid` outside (rising cost), `go_hold` inside near cover | sphere/cylinder, next ring dashed, shrink line |
| `ls_zone_shrinker` | new (logic-only) | stages = "r:warn:shrink:dmg …", random_center, final_r | P | → stage(n) ; ← start, skip, stop | (drives `ls_safe_zone`) | timeline badge on the zone |
| `ls_damage_zone` | **trigger_hurt** (adopts or spawns) | dmg, interval, kind (fire/poison/shock), on_off, hud_warning | P | → player_hurt; ← enable, disable | `avoid` with cost ∝ dmg (brain already avoids always-on trigger_hurt: gm_botnav's "deadly ground"; the fork gets the same rule) | red volume |
| `ls_checkpoint` | new | order, radius, team, once, score | P | → reached(id, order), lap_done; ← reset | `attract` next checkpoint in order (race/KZ) | numbered gate |
| `ls_timed_door` | **func_door / func_door_rotating** + timer | open_at, close_at (phase seconds), open_secs, locked_until, lockable (BarTime) | P+B (path cost while closed) | → opened, closed, locked(by); ← open, close, lock(secs) | `block` while closed (detour), `use` when a bot may open it | door with a clock badge, state colour |
| `ls_trigger_trap` | **trigger_multiple/once + targets** (buttons, trigger_hurt, func_door, env_*) | target, delay, rearm_secs, team_only, trap_kind | P | → armed, fired(by, victims); ← arm, fire, disarm | `avoid` radius when known; traitor/predator bots `use` the button | lines to targets, danger volume |
| `ls_weapon_spawner` | **armoury_entity** (spawns it; CS gives the weapon/ammo on touch) | item (csw or table), count, respawn_secs, tier, once | P | → taken(id, item), respawned; ← set_item, respawn | `attract` weighted by tier/need (bots already pick up `armoury_entity`) | model of the weapon, tier colour |
| `ls_loot_container` | new (model + BarTime) | table, tier, open_secs, items, once, glow | P | → opened(id), item_given; ← fill(table), lock | `loot` goal (new brain state), `attract` | crate model, tier glow |
| `ls_supply_drop` | new (falling model, beacon beam, smoke) | table, fall_secs, beacon, chance, at_phase | P | → announced, landed, opened; ← call(origin) | `attract` strong; `avoid` while falling | drop line to the sky, landing disc |
| `ls_player_class` | new (logic) | name, team/role, hp, armor, speed, gravity, weapons, model (player model), abilities, limit | P | → class_set(id); ← set(id, class), menu(id) | `profile` + persona scaling; `modifier` hints (knives only…) | class card in the inspector (no 3D) |
| `ls_respawn_point` | **info_player_start / info_player_deathmatch** (adopts; adds keys) | team/role, wave, protect_secs, enabled, order | P | → used(id); ← enable, disable | (the brain spawns there) | spawn marker with team colour |
| `ls_extraction_zone` | new (radius or brush, BarTime) | hold_secs, open_at, close_at, cost, team_shared, once | P+B (when to extract: value/time/HP/threat) | → opened, closed, extracting(id), extracted(id); ← open, close | `extract` goal; camp option for ambush personas | cylinder, open/closed colour, schedule badge |
| `ls_moving_platform` | **func_train / func_plat / func_tracktrain** (adopts) | speed, wait, path (path_corner), auto | P+B (wait for it, ride it) | → arrived(stop), departed; ← go, stop | `ride`: bot waits at the stop area and steps on (the nav mesh has no moving areas: the brain gets a "ride" rule: stand on it until it stops at the goal side) | platform path as a line, stops numbered |
| `ls_barricade` | **func_breakable / func_wall** (brush) | hp, repair_cost, repair_secs, blocks_bots, team | P+B (creatures attack it; bots fall back when it breaks) | → damaged, broken, repaired; ← set_hp, repair(id) | `defend` behind it; `block` for enemies' paths while standing | brush tint by HP |
| `ls_wave_spawner` | new (point) | wave_min, wave_max, types (creature types), rate, radius, path (patrol) | P (+ creature AI in the fork) | → wave_started(n), spawned(ent), wave_cleared; ← start(n), stop | `defend` facing it (angle from `.lsv`), `avoid` for survivors | wave badge, spawn radius |
| `ls_meeting_point` | new (radius) | radius, callers (role), duration, vote (menu), freeze, cooldown | P | → meeting_started(by), voted(id, target), ejected(id); ← call, end | `meeting` goal (stand, talk, vote) | round table disc |
| `ls_objective` | new (point or brush) | kind (collect/carry/destroy/hold/reach), count, item model, score, carrier_speed | P | → progress(obj, id), done(obj) | `attract`/`go_hold`/`capture` by kind | kind icon, link to related entities |
| `ls_turret` | new (model, traces) | dmg, range, rate, hp, team, cost | P | → target_acquired, destroyed; ← enable | `avoid` for enemies, `defend` near it | cone of fire |
| `ls_defence` | new (uses the above: tripmine = beam + trigger, floodlight = `light` toggle, lockdoor = `ls_timed_door.lock`, noise = sound) | kind, target, uses, team | P | → placed, triggered | `avoid` when known; `use` by the defending side | kind icon, laser line |
| `ls_creature_spawn` | new | type, count, respawn, patrol (path), home_r | P (creature AI in the fork, §6) | → spawned, killed(ent, killer) | creatures target bots; bots get `enemyclass` | creature badge |
| `ls_patrol_path` | **path_corner** chain | wait, loop, speed | P | — | bots with the guard profile walk it (`go_hold` sequence), creatures too | polyline |
| `ls_fallback_point` | new | order, team | P | — | `defend` here when the previous line breaks | numbered flag |
| `ls_shop_zone` | **func_buyzone** (adopts) or radius + menu | catalog (shop.ini), between_waves_only | P | → bought(id, item) | `buyplan` (exists) | shaded volume |
| `ls_alarm` | new (logic) | levels (0-3), decay, reinforce (spawner), lock (doors) | P | → level(n); ← raise, lower | guard bots `investigate`/`hunt` by level | level lamp |
| `ls_camera` | new (view cone traces) | fov, range, sweep, alarm | P | → spotted(id) | `avoid` the cone for sneaking bots | cone |
| `ls_hiding_hint` | new (point) | radius | P+B | — | predator/guard search weight; survivors `flee` here | eye icon |
| `ls_body` | runtime only | — | P | → found(id), identified(id) | `investigate` | — |

Sound/light helpers (`ls_sound`, `ls_light_toggle`) are thin wrappers over `ambient_generic`/`light`
for the Studio's Audio nodes.

### 2.4 The mode definition (`<id>.mode.ini`): rules as data

Structure, not engine settings (those stay in `<id>.cfg`). Read by `gm_modekit.amxx` at
`plugin_cfg`; unknown keys are warnings in the log and in the app's checker.

```ini
[mode]
id = br            name = Battle Royale
base = ffa         ; ffa | teams | coop | roles        (who is an enemy of whom)
players = 8-32     round = single                     ; single | rounds | waves | continuous
components = ls_safe_zone ls_zone_shrinker ls_loot_container ls_weapon_spawner ls_supply_drop ls_respawn_point
features = squads  ; optional gm_mk_* plugins
bots = br          ; profile in botprofiles.ini
rules = data       ; data | graph:<file> | plugin:<name>   (how the rest of the rules are given)
map_prefix = br_   map_fallback = pack

[teams]      roles = survivor:*    hide_roles = 0    team_names = Survivors, Survivors
[win]        conditions = last_squad     draw = time
[spawn]      mode = random   respawn = none   protect = 3   start_hp = 100   start_armor = 0
[equipment]  start = knife   buy = none   money = 0   drop_on_death = all   ammo = scarce
[timing]     freeze = 10   round = 15   phases = lobby:20 drop:30 play end:10
[objectives] ; name = kind, params   (ls_objective instances or kit objectives)
[scoring]    kill = 10   objective = 50   win = 100   persist = none        ; none | stats | stash
[abilities]  ; role = ability list (modes/<id>/abilities.ini)
[map]        requires = ls_respawn_point:16 ls_safe_zone:1 ls_loot_container:30   optional = ls_supply_drop:4   nav = required   size = large
[events]     supply_drop = every:120, chance=70     storm_surge = at_phase:final, chance=30
[modifiers]  pool = default   chance = 25   max = 1
[hud]        top = phase_timer   left = objective   right = alive_count   center = events
```

Companion files in `modes/<id>/`: `loot.ini`, `waves.ini`, `roles.ini`, `abilities.ini`,
`events.ini`, `shop.ini`. Shared: `modifiers.ini`, `botprofiles.ini`, `creatures.ini`. The schema
(`catalog/modekit-schema.json`) is generated from the Pawn tables so the app's forms, the Studio's
Zod schema and the plugin can't drift.

### 2.5 How the modes are assembled

| Mode | base / round | Components (count) | Rules that aren't a component |
|---|---|---|---|
| **Battle Royale** | ffa (+squads) / single | safe_zone 1, zone_shrinker 1, loot_container 30+, weapon_spawner (loot points), supply_drop 2-6, respawn_point 16+ (random far spawns) | lobby/drop/play phases, last squad wins, spectate squad on death |
| **Extraction** | roles PMC/scav / single | extraction_zone 2-4 (schedules), loot_container 40+, weapon_spawner, objective 0-3, creature_spawn / respawn_point(scav) 6+, respawn_point(pmc) 8+ at the edges, timed_door (gated routes) | stash persistence, insurance, MIA at raid end, carried value |
| **The Predator** | roles 1:* / rounds | player_class ×2 (predator, survivor), extraction_zone 1-2 (open_at 3:30), defence 6-12, trap optional, respawn_point(pred) high, respawn_point 16, hiding_hint, (safe_zone optional: the ground closes in) | abilities (leap/cloak/track/pulse), survivors win by extracting or killing, learning file |
| **Last Stand** | coop / waves | wave_spawner 4+, barricade 2-12, fallback_point 1-3, shop_zone, turret (buyable), respawn_point(CT) 16, creature_spawn (boss) | waves.ini progression, money per kill/wave, buy between waves, revive, endless record |
| **Traitor** | roles hidden / rounds | meeting_point 1-3, weapon_spawner 8+, trap (tr_only buttons), respawn_point 16, body (runtime), objective optional | role assignment, identify/DNA, karma, votes, win by elimination/majority/time |
| BR + Zombies | br + infection feature | as BR | first death per squad becomes a zombie (player_class zombie), zombies ignore zone damage 10 s |
| Extraction + Bounty | ex + bounty feature | as EX | a random PMC carries a bounty; HUD arrow; extract with it pays more |
| Heist + Traitor | heist + roles | alarm 1, camera 4-8, timed_door (vault, lockdown), trap (lasers), objective (carry bags), extraction_zone (van), patrol_path (guards), creature_spawn/respawn(guards) | alarm escalation; one crew member is the informant (trips alarms "by accident") |
| Surf + DM | surf cfg + data | respawn_point, weapon_spawner | instant respawn, score; a `.mode.ini` over the surf mode |
| KZ + Race | kreedz + race feature | checkpoint ×N | everyone starts together, first to the end |
| Commander | classic + squads | — | squad HUD, `gm_botchat_obey 3`, fireteam names |
| Hunted | roles / rounds | objective chain, alarm, patrol_path, timed_door, camera, hiding_hint, extraction_zone | guards with the hunter profile; stealth from HNS's light rule |
| King of the Ramp | surf + data | capture_point 1 (on the ramp top), respawn_point | points per second held; surf bots (`ls-surf`) |

The library covers all of them; what each mode adds in rules is small, which is the point.

### 2.6 Prior art: XDM and Half-Life: Cross Product (researched 2026-11-12)

- **XDM** (X-Half-Life Deathmatch, Xawari; source mirror github.com/LegendaryGuard/XDM-SourceCode,
  MPL-2.0 for Xawari's files, Valve SDK licence for the rest). Modes are **C++ classes** in
  `dlls/gamerules/` (`CGameRulesCTF`, `CGameRulesDomination`, `CGameRulesLMS`,
  `CGameRulesRoundBased`, coop…) with ~100 virtual policy hooks (`IsAllowedToSpawn`,
  `FItemShouldRespawn`, `CheckEndConditions`, `PlayerRelationship`…). Selection: `mp_gamerules`
  cvar, else auto-detect from `maps/<map>_pre.cfg` (`mp_maprules`), the map-name prefix table
  (`DM_ CO_ CTF_ DOM_ LMS_ TM_ RM_ …`), or a worldspawn key. **Per-mode entity sets** come from
  `maps/<map>_patch.ent` (brace-delimited entity list spawned via `CBaseEntity::Create`, when
  `sv_loadentfile`) and map cfgs with conditional sections gated on the active game type.
  "Mutators" are flat orthogonal cvars (`mp_instagib`, `mp_telegib`, `mp_nofriction`,
  `mp_defaultitems`, `mp_bannedentities`, `mp_weaponstay`, `mp_allowmonsters`…), which stack
  because each is an independent check.
- **Cross Product Multiplayer** (Aura engine, Phoenix Project Software; fork of Adrenaline Gamer).
  **Gametype** = a code-defined ruleset players can't change (arcade, ffa, ctf, dom, instagib,
  sgbow, swat, tdm); **gamemode** = a cfg in `gamemodes/` of server commands selected with
  `sv_aura_gamemode`; a "Custom Game Composer" UI writes those cfgs; "Item Cache" configures
  weapons/items/player properties; CTF/DOM "definition files" place objectives per map. Exact
  formats unverified (wiki behind an anti-bot wall).

Lessons we take: game-type policy in code, tunables in data, per-mode entities as external
files applied per map (our map packs are XDM's `_patch.ent` idea), composition as one base plus
orthogonal toggles (our modifiers), prefix detection (already ours). What we add: components
with bot hooks (neither has bots worth the name), and three authoring routes to the same plugin.
What we can't copy: both ship a custom client.

### 2.7 Combinations and round modifiers

- **A combination is a mode** whose `components`/`features` pull in another mode's feature
  plugin; features register `provides`/`conflicts` (`mk_feature_register("infection", "teams",
  "roles")`) so an impossible combo fails at map start and in the designer. Infection here is our
  own small `gm_mk_infection` (player_class zombie with claws/HP/speed; humans keep CS guns), not
  ZP, which manages teams itself and would fight the kit.
- **A round modifier is a temporary rule** rolled at round start from `modifiers.ini`
  (`name | weight | cvars | flags | conflicts | modes`), announced on the HUD, applied as cvars +
  feature flags, undone at round end (the kit keeps before-values, as `base.cfg` does per map).
  Pool: low gravity, headshots only, knives + grenades, fog, night (+flashlights), one-hit,
  vampire, bounty on the leader, double loot, fast zone, silent footsteps, random guns at spawn,
  infinite ammo. Bots get `ls_mode modifier <name>` hints.

### 2.8 Map packs for stock maps

`maps/<map>.<id>.ini` lists component entities for maps that weren't built for the mode:
`ls_loot_container 1200 -300 64 tier=2`. The editor writes it from its entity tool with a BSP
open as reference, the health check reads it, the mode's catalog part ships it. This is how BR
and Extraction get playtested on `de_cbble`/`cs_militia` (big-maps.md) before `br_` maps exist.

### 2.9 `loner.inc`: one include, a whole mode in ~40 lines

`REPO/plugins/scripting/include/loner.inc` (the catalog's `compile.include` already supports an
include dir) wraps `gm_components` + `gm_modekit` natives/forwards with short names and sane
defaults. The same surface is the Studio's node registry and the MCP mode tools.

```pawn
#include <loner>

public plugin_init()
{
    ls_mode("kotr", "King of the Ramp", LS_BASE_FFA);          // registers with gamemodes.sma's menu text
    ls_rule_win(LS_WIN_SCORE, 300);
    ls_rule_respawn(LS_RESPAWN_INSTANT, 2.0);
    ls_rule_equip("ak47 m4a1 deagle vesthelm", LS_BUY_NONE);
    ls_hud_slot(LS_HUD_TOP, "score");                            // the manager draws it
    ls_bots_profile("kotr");                                     // botprofiles.ini
}

public ls_round_started()           { ls_say_all("Hold the ramp top to score!"); }

public ls_cp_captured(ent, team, id) { ls_score_add(id, 25); ls_bots_goal_all(LS_GOAL_CAPTURE, ent, 0.8); }

public ls_cp_tick(ent, id)          { ls_score_add(id, 1); if (ls_score(id) >= 300) ls_win(id); }

public ls_player_killed(victim, killer) { if (ls_cp_owner(0) == victim) ls_hud_center_all("The king is dead!", 3.0); }
```

Groups in the include: `ls_mode/ls_rule_*` (registration and rules), `ls_<component>_*` (actions)
+ forwards `ls_<component>_<event>`, `ls_hud_*` (slots, bar, icon, fade, shake, fog), `ls_bots_*`
(goal, avoid, profile, friend, enemyclass, evidence), `ls_score_*`/`ls_persist_*`, `ls_phase_*`,
`ls_timer_*` (countdown/delay/repeat with ids), `ls_give/ls_strip/ls_teleport/ls_health`,
`ls_team/ls_role`, `ls_sound/ls_say_*`, `ls_creature_*`. Everything a Studio node does is one of
these calls, which is what makes "open as code" honest.

---

## 3. Per-mode design

### 3.1 Battle Royale (`br`, maps `br_*`)

**Rules.** 8-32 players, solo/duo/squad from the mode's options menu (`say /br`), one 12-15 minute
round, everyone on one CS team with `mp_freeforall` damage and kit "parties" exempted from
friendly fire. Spawn knife-only at far-apart `ls_respawn_point`s (or random nav areas with the
largest flood distance), 3 s protection. Loot: `ls_weapon_spawner`s with tiers (pistol/SMG/
rifle/sniper/armour/grenade/medkit), `ls_loot_container`s (2 s BarTime), `ls_supply_drop`s (2-4,
best tier, beacon). Ammo scarce (one spare magazine per pickup). `ls_safe_zone` + `ls_zone_shrinker`
in 5-6 stages (radius, warn, shrink, dmg/s outside: 1 2 4 8 15). Win: last survivor/squad.

**Flow.** lobby (20 s) → drop (30 s, no damage) → play (stages) → end (10 s banner) → restart or
next map after N rounds.

**HUD.** Zone ring: `TE_BEAMCYLINDER` always animates outward (client-limits.md §1), so it is
only the **pulse** on each shrink. The standing ring is **24-32 `TE_BEAMPOINTS` segments** at ground
height (plus a sparser head-height ring), life 1.2 s, refreshed once a second, sent **unreliable**
(`MSG_ONE_UNRELIABLE`: a 32-segment ring is ~1 KB, and the reliable channel kicks at ~4 KB) and only
to players within ~1500 u of the edge; everyone else gets the pulse and the HUD. Alternative when
the beam pool is tight: the ring as persistent `env_beam` entities (networked once; costs edicts).
The next ring is dimmer and drawn only in the warn phase. Outside: `Fog` grey + red `ScreenFade`
with each damage tick and the `Damage` arrow; centre slot "OUTSIDE THE ZONE – 320 m ↖" with a text
compass. `RoundTime` shows the time to the next shrink in the client's own round clock. Radar: a
`Radar` message with a spare player index puts a dot at the zone centre (a documented trick);
`HostagePos` dots for drops (verify they render without real hostages). Top: alive / stage; left:
squad HP; right: ammo. Death → spectate your squad. Lobby/squads: `show_menu`, `/squad name`.

**Bots.** Loot nearest (greed × tier / distance), fight when it pays (persona + gear vs the heard
weapon class), track gunfire when armed and healthy, go to the zone early or late per persona,
pick the next ring's edge with cover and exposure to the centre (`.lsv`), camp late, treat drops
as bait or prize, squads follow their leader with spacing.

**Map.** 16+ spawns, 1 zone (else a random central nav area), 30+ containers/spawners, 0-6 drops,
nav required, large. ±4096 cube: stack vertically with teleports, fog, func_detail (big-maps.md).

**Reused vs new.** Reused: `gm_deathmatch` spawn logic, `gm_botfun` pickups, flood distances,
`mp_freeforall`. New: the zone/loot/drop components, squads feature, br rules (~600 lines of
loner.inc Pawn or a graph), loot/zone/flee goals. Effort: components 2 weeks, mode 1, tuning 1.

### 3.2 Extraction (`ex`, maps `ex_*`)

**Rules.** 6-16 PMCs (solo/duo), AI scavs (creatures or scav-profile bots), the odd boss, 10-15
minute raids; late joiners spawn as scavs. **Stash** per authid (SQLite via the kit; nvault by
name without accounts, warned), chosen pre-raid from a menu; loot `ls_loot_container`s and
valuables (stash value only), optional `ls_objective`s, **extract** at an `ls_extraction_zone`
(hold 8 s, schedules, some cost money, one always open). Extracting banks carried gear; dying
loses it; insurance returns one weapon; MIA at raid end; stash cap and value curve; scav runs for
the broke.

**Flow.** Stash menu (15 s) → raid (edge spawns) → extracts open/close by timetable → MIA → stash
summary (MOTD HTML table) → next raid.

**HUD.** Left: carried value + items; right: extracts and states, compass arrow; centre: events;
`BarTime` on extract/loot; `StatusIcon` "rescue" when inside an extract.

**Bots (PMC).** Loot by value, avoid contested areas (noise + recent deaths clustered per area:
the team-play "site hit" counter generalised), retreat when hurt (cover + medkit item), ambush
(camp an extract or a rich container when the persona is a lurker/camper), extract on
value/time/HP/threat and switch extracts when one is watched. Bots have stashes too (regulars
arrive with what they earned). Scav bots: newbie aim profile, patrol a home area, swarm noise.

**Map.** 2-4 extracts, 40+ containers, 6+ scav spawns/creature spawns, 0-1 boss, 8+ edge PMC
spawns, nav. Fallback pack on `cs_militia`, `de_aztec`.

**Reused vs new.** Reused: BR's loot and objectives, roles, basebuilder's SQLite precedent,
creature bridge. New: stash/insurance/MIA (~1 week), extraction timetable, ex rules (~800 lines),
pmc/scav profiles, contested-area memory in the brain. Effort: 3-4 weeks after BR.

### 3.3 The Predator (`pred`, maps `pred_*`)

**Rules.** 1 predator (human volunteer or bot with the `hunter` profile) vs 4-15 survivors, 5 minute
rounds. Predator `ls_player_class`: 400 HP + armour, speed 400 (`set_user_maxspeed`), knife, abilities
from `abilities.ini`: **leap** (velocity push on +use, 8 s), **cloak** (render alpha 30 for 6 s,
breaks on attack), **track** (6 s: moving survivors glow for the predator only, a per-client
`set_rendering` through an `AddToFullPack` hook), **pulse** (every 30 s: "3 survivors within 40 m").
Survivors: pistol + 2 magazines, 1 grenade, no buy, `ls_defence`s (tripmine, floodlight, lock
door, noise can). Survivors win by extracting (an `ls_extraction_zone` opening at 3:30, shared
BarTime: more inside = faster) or killing the predator; the predator wins otherwise.

**HUD.** Predator: ability bar in the bottom slot, `ScreenShake` heartbeat near prey,
`StatusIcon` "escape" when the extraction opens. Survivors: ammo + nearby defences right, dread
darkening (`Fog`/`ScreenFade`) when the predator is cloaked within 15 m, extraction timer top.
Both: red director text on a predator kill.

**Bots.** Survivors pair up, hold defences, run to the extraction by the least exposed route,
flee the predator (`hunted` profile), grenade the leap. **AI predator**: patrol high ground, noise
first, cloak before closing, leap to break line of sight, retreat under 150 HP, and **learn hiding
places**: the kit logs where survivors were found (`data/gamemodes/pred_<map>.ini`: area id, count,
last round); the brain's search order weights areas by past success with decay. `ls_hiding_hint`s
seed it. The same file feeds Hunted's guards.

**Map.** 1-2 extracts, 6-12 defences, 1 predator spawn (high), 16 survivor spawns, nav, dark
lighting preset, indoor/outdoor mix.

**Reused vs new.** Reused: roles, extraction, HNS's darkness rule, botfun look/hold. New:
abilities (per-player speed/render/cooldowns, 1 week), defences (1 week), pred rules (~700 lines),
hunter/hunted profiles + learning (brain). Effort: 3 weeks.

### 3.4 Last Stand (`ls`, maps `ls_*`)

**Rules.** Co-op, 1-16 on CT, waves from `ls_wave_spawner`s per `waves.ini` (count, types, HP ×,
speed, specials, boss every 5th). Enemies: our creatures (runner/brute/spitter/screamer/boss) or
MonsterMod monsters when installed or **bot zombies** (T bots, knives, `bot_zombie 1`, zp5-style
class) as the fallback. Money per kill and wave, buy between waves (`ls_shop_zone`: CS buy menu +
`shop.ini`: ammo refill, armour, medkit, `ls_turret`, barricade repairs), `ls_barricade`s with HP
(creatures attack what blocks them; +use repair with BarTime, cost per 25 %), downed players
revived in 30 s, dead return next wave. Win: all waves (or endless with a persisted record).
Lose: team wiped in one wave (3 lives per map in endless).

**Flow.** prepare 45 s → wave (trickle 20-40 s, bosses announced) → cleared 15 s → … 

**HUD.** Top "WAVE 7 — 12 left"; left money + teammates' HP/DOWN; right nearest barricade HP;
centre boss warnings + `ScreenShake`; glow shells by enemy type; shop as `show_menu`.

**Bots (CT).** `defend` facing the spawners (angles from `.lsv`), fall back to `ls_fallback_point`s
when a barricade breaks, `buyplan` between waves, `revive` downed players, repair when rich, call
specials via botchat.

**Map.** 4+ spawners, 2-12 barricades, 1-3 fallback points, shop zone, 16 CT spawns, nav
(creatures path on it). Fallback pack: `cs_assault`, `de_inferno`, `zm_` maps.

**Reused vs new.** Reused: CS buy/money, roles, HUD manager, creature bridge, zp5 bot-zombie
patches. New: wave spawner, barricade, turret, revive (~2.5 weeks), ls rules (~600 lines),
creatures (§6, separate). Effort: 3 weeks + creatures.

### 3.5 Traitor (`tr`, maps `tr_*`)

**Rules.** 6-16 players on one CS team (`hide_roles=1`): ~25 % traitors (know each other, private
chat `/t`, shop: silenced pistol, 30 s C4, fake ID, body remover), 1 detective per 8 (public,
DNA: +use on a body lists who was near it in the last 20 s from the kit's position log), the rest
innocents; pistol start, rifles from `ls_weapon_spawner`s. Bodies persist (`ls_body` over the
corpse) and must be **identified** (+use: name, role, time, damage type). **Meetings** at an
`ls_meeting_point` (a detective, or two players, or "body found" once per player): 45 s frozen,
chat, `show_menu` vote, majority ejects, role revealed. Karma persisted (TTT's rule: innocents who
kill innocents do less damage next rounds). Win: traitors dead / traitors ≥ innocents / time
(innocents).

**HUD.** Role card (centre, 5 s), left role + karma + bodies found, traitors see allies' names in
red (and a per-client glow), meeting = frozen view + chat + menu, results as director text, MOTD
round summary. Secrecy tricks from TTT: one team, scoreboard alive/dead only ("MIA" until a body
is found: block `DeathMsg`, rewrite `ScoreInfo`; to verify on the stock client).

**Bots.** §4.4: a suspicion table per bot fed by evidence it could perceive; personalities decide
how fast and loud they accuse; traitor bots pick isolated targets in low-exposure areas, frame,
and manage who suspects them; innocents group, identify, report, vote on evidence; detectives
scan. Meeting talk from `botchat-tr.ini` keyed by personality letters with name/place
placeholders.

**Map.** 1-3 meeting points, 8+ weapon spawners, traitor traps optional, 16 spawns, nav,
interiors with doors and several routes. Fallback pack: `cs_office`, `cs_italy`.

**Reused vs new.** Reused: roles, HUD, persistence, botchat, shop. New: bodies/ID/DNA (1 week),
meeting point (1 week), position log, traitor shop/traps (0.5), tr rules (~900 lines), suspicion
model (brain, 3-4 weeks). Effort: 4 weeks + bots.

### 3.6 Client-limit budget (every mode; numbers from the research)

| Channel | Used for | Limit (stock client) |
|---|---|---|
| `set_hudmessage` 1-4 | kit slots top/left/right/bottom; only the HUD manager writes (sync objects) | 4 channels (−1 auto); a 5th overwrites; ~479 visible chars, HUD font, no per-word colour |
| `show_dhudmessage` | centre announcements | 8 slots per client (9th overwrites the oldest), 128 chars; keep to 1-2 |
| `BarTime`/`BarTime2` | loot, extract, repair, revive, meeting | one per player at a time |
| `RoundTime`, `HideWeapon` | zone/wave countdown in the client's round clock; hide radar/money/timer where a mode wants | cheap |
| `StatusIcon` | zone/extract/objective icons from the client's own `sprites/hud.txt` (c4, rescue, escape, buyzone, defuser, vipsafety, suit_full, dmg_*) | only sprites the client already has |
| `ScreenFade`, `ScreenShake`, `Fog`, `Damage` | zone damage, dread, boss, modifiers | cheap (`<<12` fixed point, fade ≤ ~16 s) |
| Beams (`TE_BEAMPOINTS` rings/lasers, `TE_BEAMCYLINDER` pulses, `TE_BEAMFOLLOW` trails, `TE_GLOWSPRITE`/`kRenderGlow` markers through walls) | zone rings, lasers, beacons, objective markers | a fixed client beam pool (64-128 depending on build: verify) and ~500 temp entities; life is a byte of 0.1 s (max 25.5 s); send repeating visuals unreliable, ≤1/s; lasers counted per map by the checks |
| User messages | everything above | **192 bytes** per message; reliable buffer overflow (~4 KB) kicks the client |
| Render fields per viewer (`AddToFullPack` hook) | tier glow, cloak, traitor/predator sight | new small fakemeta hook |
| Radar | `Radar` dots at arbitrary points (spare player index), `BombDrop` planted flash, `HostagePos` (verify without hostages) | dots only, no shapes; the client decides colours |
| Scoreboard | `ScoreAttrib` (dead/bomb/VIP), `ScoreInfo` (any numbers), `TeamScore` | Traitor's "MIA" and custom columns |
| Menus | lobby, squads, stash, shop, votes | 10 keys, one menu at a time; newmenus paginate |
| MOTD | stash/round summaries, mode help | static HTML |
| Precache | creature models/sounds, sprites | **512 models (incl. sprites), 512 sounds**; stock maps already use 100-200; the checks count them |
| Edicts | crates, drops, bodies, defences, creatures | default 900; `-num_edicts` up to 2048 (index > 2047 breaks networking) |
| Players | 32 | HLDS/ReHLDS |

---

## 4. Bot behaviour per mode (the brain)

### 4.1 Plumbing: `ls_mode` and `ModeGoal`

New server commands beside `ls_bot` (`cs_bot_ls.cpp`):

```
ls_mode set <id>                                   ls_mode profile <bot|all> <name>
ls_mode goal <bot> <kind> <x y z> [ent] [prio] [until]   kinds: go_hold defend capture extract loot use ride investigate flee meeting
ls_mode goal_clear <bot> [kind]
ls_mode avoid <x y z> <r> <cost> [until]          nav-area cost (damage zones, outside the safe zone, traps)
ls_mode block <ent> <until>                        a closed timed door costs as a wall (detour)
ls_mode attract <x y z> <r> <weight> [until]       things worth going to (loot, drops, capture points)
ls_mode zone <cx cy cz> <r> <ncx ncy ncz> <nr> <shrink_at>
ls_mode enemyclass <classname> [threat]            ls_mode friend <bot> <player>
ls_mode role <bot> <role> [known …]                ls_mode evidence <bot> <about> <kind> <weight>
ls_mode modifier <name>                            ls_mode item <bot> <kind> <count>
```

In the brain: `struct LsModeGoal { kind, at, ent, prio, until }` per bot; `LsModeTick()` runs after
`LsOrderTick()` and before the stock state machine. Priorities: a fight wins; a person's `LsOrder`
beats a mode goal unless zone damage is imminent; mode goals beat idle hunting. Goals are
advisory: the component says "loot here", the persona decides whether this bot cares. Nav costs
(`avoid`/`block`) go into the brain's path cost functor so every existing state detours. The
`ride` rule: wait on the platform's stop area, step on, stand still until it stops where the goal
is. `ModeGoal` reuses `MoveToState`, `HideState`, `InvestigateNoiseState` and the `.lsv` queries.

Behaviour profiles (`configs/gamemodes/botprofiles.ini`, read by the fork and by the kit):

```
; name    | greed | caution | fight_pick | zone_margin | extract_value | group | camp | investigate
br        | 0.7   | 0.5     | 0.5        | 25          | -             | 0.3   | 0.4  | 0.6
scav      | 0.3   | 0.2     | 0.9        | -           | -             | 0.6   | 0.1  | 0.9
pmc       | 0.8   | 0.7     | 0.4        | -           | 0.6           | 0.5   | 0.5  | 0.4
hunter    | -     | 0.2     | 1.0        | -           | -             | 0     | 0.2  | 1.0
hunted    | -     | 0.9     | 0.1        | -           | -             | 0.7   | 0.8  | 0
defender  | 0.4   | 0.6     | 0.8        | -           | -             | 0.9   | 0.9  | 0.2
guard     | -     | 0.5     | 0.8        | -           | -             | 0.4   | 0.3  | 0.9
innocent  | 0.2   | 0.6     | 0.3        | -           | -             | 0.8   | 0.3  | 0.7
traitor   | 0.2   | 0.8     | 0.6        | -           | -             | 0.3   | 0.5  | 0.3
```

Each bot's persona (`people.ini` via `LsSetPersona`) scales its profile, so two bots with the same
profile still differ.

### 4.2 Senses and enemies that aren't players

Vision/`IsEnemy` only consider `CBasePlayer` today. For creatures (and MonsterMod monsters):
a registered enemy-class list, enemy-candidate iteration over those entities in
`cs_bot_vision.cpp` and the LS senses, threat weighting (a brute at 10 m beats a runner at 30 m),
and the `gm_humanbots` aim model accepting entity targets (`hb_bot_target` returning an edict).
Creatures use `EmitSound`, so `LsHeard` treats their sounds as enemy noise when the class is
registered. Creatures target bots as players (§6.2).

### 4.3 New goals

| Goal | Lives in | Brief |
|---|---|---|
| `loot` | new `LsLootState` (MoveTo + on-arrival +use/pickup, "someone's here" abort) | targets by (tier × greed)/distance, avoid areas with recent enemy noise unless well armed |
| `zone` | `LsModeTick` + MoveTo + `avoid` costs | leave the outside through cover; late game: the next ring's edge with the best cover/exposure |
| `extract` | MoveTo + hold | on value/time/HP/threat; switch when an enemy is seen near it |
| `capture` | MoveTo + hold in the volume | contest/defend by side |
| `hunt` | `HuntState` + noise-first senses + the learned hiding list | patrol high ground, investigate every noise, search weighted areas |
| `defend` | `HideState` facing the given threat direction | hold, fall back when a barricade breaks, `revive` |
| `ride` | new small rule | wait, step on, stand, step off |
| `investigate`/`suspect` | Traitor (§4.4) | go to a body, use it, act on suspicion |
| `flee` | new: maximise `.lsv` cover, break line of sight, hide (hints first) | hunted survivors, outgunned BR bots, hurt PMCs |
| `meeting` | stand still, talk via botchat, vote | Traitor |

### 4.4 Traitor: suspicion and personalities

`suspicion[player]` (0..1, decaying) per bot. The kit sends `ls_mode evidence` lines with the event's
position; the brain discards what this bot couldn't perceive (`IsVisible` + noise range).
Kinds/weights: `shot_at_player` 0.5 (first shooter), `near_body` 0.2, `found_body_unreported` 0.4,
`traitor_weapon` 0.6, `lied_in_meeting` 0.5 (claim vs position log), `followed_me` 0.1,
`vouched_by_detective` −0.5, `killed_confirmed_traitor` −0.4. Behaviour: > 0.4 keep distance and
watch; > 0.7 call a meeting or shoot if aggressive (temper tilty / style rusher) and alone with
the suspect; vote the highest over a threshold, quiet personalities abstain more. Traitor bots run
"who suspects me" from chat accusations and either isolate the accuser or play along.

### 4.5 Commander and the voice path

`gm_botchat` already maps orders to `ls_bot`. Commander = a `.mode.ini` over classic with the
squads feature (fireteam names, `/alpha go long`), `gm_botchat_obey 3`, squad HUD in the left slot;
the planned voice-to-text (0.1.3 plan) feeds the same parser. 1 week once the kit exists.

---

## 5. Hunted, Heist, King of the Ramp (coordinator's ideas, brief)

- **Hunted**: 1-4 humans sneak through a map held by `guard`-profile bots with full senses (noise
  memory, HNS's light rule, doors, breakables); `ls_objective` chain, `ls_alarm` (spotted → converge
  → search with the learned hiding list), `ls_extraction_zone`. Data over components. 1.5 weeks
  after Predator.
- **Heist**: crew (humans + bot crew) vs AI security (guard bots, `ls_camera` cones, `ls_trap`
  lasers, vault `ls_timed_door` + drill BarTime). `ls_alarm` levels 0-3: stealth → suspicious →
  alarm (reinforcements from a spawner, doors lock) → lockdown (roof extraction only). Loot bags
  slow you (`carrier_speed`), carry to the van. 3 weeks after Extraction.
- **King of the Ramp**: surf + combat: one `ls_capture_point` on the ramp top, free guns, instant
  respawn, surf bots (`cs_bot_surf.cpp`) with `fight_pick` high near the hill. The 40-line example in
  §2.9. 1 week after BR if `ls-surf` is merged.

---

## 6. Creatures and NPCs

### 6.1 One interface, three backends

`creatures.ini` defines **types**; each has per-backend blocks:

```
[runner]
hp = 60   speed = 300   damage = 10   attack_range = 48   attack_rate = 1.0   glow = 0 160 0
sounds = zombie/idle*.wav zombie/attack*.wav
ls  = model=models/player/zombie_source/zombie_source.mdl anims=walk run attack1 die   ; our ls_creature
mm  = monster_zombie                                                                    ; MonsterMod
bot = zombie_classic                                                                    ; bot-zombie fallback
```

`ls_creature_spawn(type, origin, flags)` picks the first available backend in the mode's order
(`mk_creature_backend "ls mm bot"`): the fork's `ls_creature` when the fork runs, MonsterMod when
its Metamod plugin is loaded and the models exist, otherwise bot zombies (T bots, `bot_zombie 1`,
knives, the type's HP/speed) so a creature mode always plays. Kill/hurt/noise events arrive
through the same forwards.

### 6.2 Our own creatures on the hostage AI (recommended; feasibility checked)

`CHostageImprov` already provides (verified in `hostage_improv.h/.cpp`): nav-mesh pathing with
`HostagePathCost`, `TrackPath`, ladders, `DiscontinuityJump`, crouch, local obstacle avoidance
(`hostage_localnav.cpp`), `KeepPersonalSpace`, `IsVisible`, last-seen/injury/noise timers,
`Frighten`, `OnGameEvent`, a state machine and animation helpers. What ties it to hostages: the
`CHostage*` in the constructor, chatter/scare types, and `CHostage`'s rescue logic + 12-cap manager.

Plan in the fork:
1. **Factor** `CCreatureImprov : CImprov` out of `CHostageImprov` (locomotion, local nav, look/face,
   vision, timers, personal space); `CHostageImprov` becomes a thin subclass so hostages don't
   change. ~1 week.
2. **`ls_creature`** (`CBaseMonster`): `ls_creature spawn <type> <x y z>` or `create_entity` + keys;
   own manager (soft cap from the edict budget, e.g. 48 alive); `TakeDamage`/`Killed` with gibs
   and hit groups; `Classify()` so `IsEnemy` and the senses can test it; `EmitSound` noise; glow per
   type. States: idle/wander (home or `ls_patrol_path`), chase (nearest seen/heard player or bot),
   attack (melee; ranged variants trace or spawn a projectile), flinch, **break** (attack the
   blocking `func_breakable`/`ls_barricade` on `OnMoveToFailure`), flee (spitter/screamer keep
   range), die. Specials: boss (ground pound = `ScreenShake` + radius damage), screamer (pulls
   others), bomber. ~2-3 weeks for four types + boss.
3. **Models/animations**: sequences by name from the type's `anims`, so player models work first
   (zp5's zombie models are already fetched by the catalog; `hostage.mdl` for civilians); HL1
   models when present (§6.4); for 1.0.0 **a small set of our own creature models** (4 + boss)
   under our licence, until then player-model zombies.
4. **Brain integration**: §4.2. 5. **Nav**: creatures need the mesh; without it straight-line chase
   + local nav, and the health check says so.

Why not HL1 monster code: the fork can't take HLSDK monster AI (licence), it is schedule-based and
knows nothing of the nav mesh or our senses, and stealth in Hunted/Extraction should use the same
noise/sight model as the bots.

### 6.3 MonsterMod as an optional add-on ("classic Half-Life monsters")

What the research found (2026-11-12):
- The original is botman's "Monster" Metamod plugin (2002, v3.00.00), GPL-2.0 and explicitly
  "based on the Half-Life SDK": it contains HLSDK monster/AI code, so the **Valve SDK licence
  applies beside the GPL** (non-commercial, redistribution limits). Monsters: agrunt, barney,
  bigmomma, bullsquid, controller, headcrab, hassassin, hgrunt, houndeye, islave, scientist,
  snark, zombie; config via `monster_precache.cfg`, `monster_skill.cfg`, cvar `monster_spawn`,
  `monster` command. Mirror: github.com/mlibre2/monster_plugin. Rick90's later "Monster Mod"
  (ModDB) added monsters; its source/licence unverified.
- Forks (monstermod.md): **github.com/JulianR0/monstermod-redo** (Giegue's rewrite) is the
  maintained one: GPL-2.0, Linux and Windows tiers marked done (`monster_mm.dll` /
  `monster_mm_i386.so` in `addons/monstermod/`), **no GitHub releases published, so we build it**;
  github.com/QuakerLife/monstermod-redo is an older fork of it; github.com/tjanok/monstermod-redux
  (7 commits) trims the precache and adds spawn-at-location commands. **Nothing named "MonsterMod
  Reloaded" exists**; "redo" is what people mean. It is a Metamod plugin with its own monster code
  (that's why the original ran on CS and TFC), so it should load under Metamod-P/-R with ReGameDLL,
  but **no repo confirms CS 1.6 or ReGameDLL**: a spike (2-3 days: build redo for Windows and Linux,
  run under Metamod-P + ReHLDS + the fork, check precache and bots) comes before the add-on entry.
  Monsters navigate on `info_node`s and **move poorly on CS maps without them** (the redo ships a
  node viewer); our editor can place `info_node`s, and the mode's checks can warn, but this is the
  practical reason our own nav-mesh creatures are the default and MonsterMod the classic option.
  Known: monsters attack bots but stock bots never shoot monsters (our fork fixes that, §4.2);
  each monster type adds many precache slots (512 cap); hostages vs monsters untested; Linux builds
  need g++ 4.8-era libstdc++ or HLDS's bundled one removed.
- **HL1 models**: CS (app 10) installs depot 11 "Counter-Strike Base Content" plus **depot 1 "Base
  Goldsrc Shared Content" from app 70**, shared by the GoldSrc apps; SteamDB's depot-1 listing shows
  `valve/models/agrunt01.mdl`, `barney.mdl`, gibs, monster and weapon models, and community reports
  say "all of the HL content is still installed" for CS-only owners. Local check (monstermod.md):
  the HLDS install in the app's CrossOver bottle (`Loner-Strike Servers/.base/valve/models/`) has
  `zombie.mdl` and `hgrunt.mdl`, so **servers** have them; clients load from their own `valve/`
  (not verified on a CS-only account; HL's single-player depot 71 isn't installed with CS). The
  installer **checks the files** anyway, and we never put Valve `.mdl`s on FastDL.

Catalog entry `catalog/parts/addon-monstermod.json` (kind `addon`) and what the installer gains:
- `fetch` of a pinned build (version + sha256 per file) into `addons/monstermod/` (dll/so, cfgs,
  `addons/monstermod/maps/<map>.cfg` spawn files). Since redo publishes no releases, we build it
  from a pinned commit and host the binaries where the nav pack lives (dl.lonerstrike.com), with the
  source link and commit in the part; GPL + the HLSDK's non-commercial terms allow that for a free
  project. `licence` field shown on the card and a `LICENSE` kept beside the files: "separate
  licence (GPL + Valve SDK terms), not part of Loner-Strike" (the SDHLT precedent).
- New installer key **`metamod`**: lines to add to `addons/metamod/plugins.ini`
  (`win32 addons\monstermod\dlls\monstermod_mm.dll`, `linux …_i386.so`), removed on uninstall
  (`foundation.rs` has the `amxmodx_mm.dll` pattern; generalise).
- New installer key **`needs`**: files that must exist in the game folders
  (`valve/models/zombie.mdl`, `headcrab.mdl`, `hgrunt.mdl`, `valve/sound/zombie/*.wav`, …). Missing →
  card message: "Classic Half-Life monsters need Half-Life's own models. This computer doesn't have
  them (Half-Life isn't installed or hasn't been run). Loner-Strike's own creatures work without
  them." It installs but stays off; the kit's backend probe repeats the check at map start; the
  panel shows the same note for servers (FastDL can't ship Valve's files).
- Add-on settings (`addons/monstermod.ini`): monsters, counts, difficulty. The kit's `mm` backend
  maps creature types to `monster_*` classnames; Last Stand can run waves of HL grunts.
- Not bundled, never compiled into our tree, never required by a mode.

### 6.4 Models

| Source | Licence | Who has them | Use |
|---|---|---|---|
| HL1 monster models in `valve/models` (shared depot 1) | Valve; not redistributable | most CS installs (verified at install) | MonsterMod; `ls_creature` when present |
| CS/CZ player models, `hostage.mdl` | Valve, with the game | everyone | civilians, early zombies |
| zp5 / basebuilder community models (fetched, sha256-pinned) | community | on install | Last Stand's first zombies |
| Our own low-poly creatures (4 + boss) | ours | shipped with the part | the 1.0.0 answer |
| CC0 generic packs (e.g. OpenGameArt "50+ Monsters Pack 3D") | CC0 | fetched | need Blender → SMD → `studiomdl` (512×512 texture cap); the compiler's licence must be checked (HL SDK tools vs open ones) |
| No CC-licensed GoldSrc-format monster pack was found | — | — | — |

---

## 7. Mode-aware map editor

### 7.1 Component library panel (drag-in)

A new side panel "Components" lists the library grouped (Zones, Spawns & Loot, Doors & Traps,
Objectives, Defence, Creatures, Social), with a search box; drag a card into the 3D view to place
it (point entities snap to the floor; brush components like `ls_barricade` wrap the selected
brushes, or draw a box). Definitions come from `catalog/components/*.json` (the same files the
plugin's schema and the Studio read), which the existing entity panel turns into typed fields
(the "FGD-driven forms" item in TOOLS-NOTES): number/float with units, choices, targetname pickers,
tables (loot table names from the mode), colours, and per-key help.

### 7.2 3D visuals

Zones as translucent volumes (cylinder/sphere/brush tint) with draggable radius handles; next-ring
and shrink lines for `ls_safe_zone`; paths (`ls_patrol_path`, platforms) as polylines with stop
numbers; links (trap → targets, defence → door) as lines (the editor already draws target →
targetname); drop lines to the sky with a red mark where the sky isn't open; camera/turret cones;
badges (wave number, checkpoint order, schedule); nav-aware tint (red when the point has no nav
area under it, from `goldsrc/nav.rs`). A "Mode preview" toggle shows which components the current
mode uses and greys the rest.

### 7.3 Mode checks with fixes

From `[map] requires/optional` plus per-component rules (`editor/checks.rs` + `tools/health`):
counts ("12 more loot containers"; fix: scatter onto low-exposure nav areas from the `.lsv`, else
random walkable areas), clustering (merge within 200 u), reachability (an extract whose nav area
doesn't connect to spawns: flood on the mesh, as the nav fixer does), wave spawners visible from
the defended area (warn), predator spawn not high ground (hint), zone centre near the edge (fix:
nav centroid), missing nav (offer "learn with bots"), meeting points too near spawns, timed-door
schedules outside the round time, platform paths with no `path_corner`s. Each is
`Issue{code, fix}`; click-to-fix calls a `cmd_*`.

### 7.4 Templates, prefabs, map packs

"New map" gets a mode picker: greybox templates with the required components placed
(`catalog/modes/<id>/templates/*.map`), compiling to a playable map in one click; a prefab library
(loot shelf, barricade doorway, drop pad, extraction gate, meeting table, tripmine corridor); and
map-pack editing (BSP as reference → place components → save `maps/<map>.<id>.ini`, no compile).

### 7.5 Playtest in a chosen mode: §10.2.

---

## 8. Game Mode Studio (Tools > Make): a node editor for rules

### 8.1 What it is

A Blueprint-style graph editor where a mode's **rules** are nodes and wires; the components
are its vocabulary. Built in the app's React/TS UI with **React Flow** (`@xyflow/react`, MIT) in a
lazy chunk (like three.js), **Zod** (MIT) for the graph schema and validation, and auto-layout
through **`@dagrejs/dagre` (MIT)** by default. **ELK.js is EPL-2.0**, which the FSF lists as
GPL-incompatible unless the work names GPL as a secondary licence (ELK doesn't): keep it out of the
GPL-3 app bundle unless that is re-checked; dagre is enough for left-to-right flow graphs.

React Flow only displays and edits. The source of truth is **our graph JSON**
(`modes/<id>/<id>.graph.json`: nodes `{id, type, params, pos}`, edges `{from.pin → to.pin}`,
groups, comments, subgraphs) validated by Zod schemas generated from a **node-definition registry**
(`catalog/nodes/*.json`) that the editor, the compiler and the MCP tools all read. Each node
definition: category, title, pins (exec in/out, typed data pins), params, the `loner.inc` call(s) it
compiles to, help, and which component it belongs to.

### 8.2 Node categories (v1 set)

| Category | Nodes |
|---|---|
| Events | Round Started, Round Ended, Player Spawned, Player Killed (victim, killer, weapon, headshot), Player Hurt, Phase Changed, Timer Fired, Chat Command, **component events** (Capture Point captured/contested/lost, Zone stage changed, Player outside/inside, Extraction extracted, Door opened/closed, Trap fired, Weapon taken, Container opened, Drop landed, Wave started/cleared, Meeting started/voted/ejected, Barricade broken/repaired, Checkpoint reached, Creature killed, Alarm level) |
| Player | Give Weapon, Strip Weapons, Set Health, Set Armor, Set Speed, Set Money, Teleport (to entity / to point), Set Class, Freeze/Unfreeze, Respawn, Kill, Slay All |
| Conditions | Is Alive, Has Weapon, Is Team, Is Role, Is Bot, Is In Zone, Compare (num/str), Any/All, Random Chance, Count Alive (team/role) |
| Teams | Assign Team, Change Role, Assign Roles (from roles.ini), Balance |
| World | Spawn Entity (component or stock), Remove Entity, Activate Door (open/close/lock), **component actions** (Zone set/shrink, Capture lock/reset, Extraction open/close, Trap arm/fire, Spawner set item/respawn, Container fill, Drop call, Wave start/stop, Meeting call, Barricade repair, Alarm raise/lower, Turret enable, Creature spawn), Set Lighting, Set Cvar (allow-listed) |
| Timers | Countdown (secs, on_tick, on_done), Delay, Repeat (every N, until), Phase (named, HUD) |
| Scoring | Add Points, Set Score, Set Winner (player/team/role), End Round (reason), Persist Set/Get |
| Bots | Set Aggression, Set Behaviour (profile), Goal (go_hold / defend / capture / extract / loot / flee / avoid area), Friend/Squad, Enemy Class, Give Evidence, Say (botchat line) — all via `ls_mode`/`ls_bot` |
| Audio/Messages | Play Sound (precached list), Broadcast (HUD slot / centre / chat, colour), Say To Player, Fade/Shake/Fog, Status Icon |
| Flow | Branch, Sequence, Gate, Once, For Each Player (team/role/alive), Variables (Get/Set: number, string, player, entity; per-round or per-map), Subgraph call, Comment, Reroute |

### 8.3 Editor features (and what's feasible)

Context-sensitive node search (drag from a pin lists only nodes with a compatible pin); typed pins
(exec, number, string, bool, player, entity, team, role, vector) with coercions declared in the
registry and colour-coded wires; comments and sticky notes; reroute nodes; collapsible groups;
**subgraphs** saved as files (`modes/_shared/*.graph.json`) with their own pins, reusable across
modes; copy/paste as JSON (clipboard); undo/redo (a command stack over the graph JSON, like the map
editor's `History`); validation errors on nodes (Zod + semantic passes: unconnected required pins,
type mismatches, unreachable nodes, a loop without a Delay, more than 4 HUD slots in use, cvars
not allow-listed); minimap; auto-layout button; "open as code" (writes the generated `.sma` and
opens Plugin Studio on it, read-only banner "generated: edit the graph, or Detach to code").

**Debugging over the local server link.** The generated plugin, when `ls_debug 1` (set only by
Playtest on the local server), logs each node firing as one line (`[lsdbg] n=17 t=123.4 p=3 v=…`)
through `log_amx`; the app already runs the local server as a child process / reads its logs, so
it tails the AMXX log (`addons/amxmodx/logs/`) and highlights nodes within ~100 ms, with recent
pin values in a tooltip and counters per node. **Breakpoints are "soft"**: a node marked as a
breakpoint compiles to `ls_debug_break(n)`, which on the local server freezes everyone (`FL_FROZEN`
+ `bot_stop 1`), pauses the kit's phase timers, and tells the app; the Studio shows the frozen state
and the variable values; **Continue** unfreezes; **Step** = continue until the next node fires.
True single-stepping of Pawn or pausing the engine is not feasible without the client's `pause`
command, so this is what we offer and we say so in the UI. Breakpoints and debug logging compile
out when `ls_debug` is off (no cost on real servers).

### 8.4 Runtime: compile to Pawn (recommended) vs a graph runner

| | (a) Graph → generated Pawn → amxxpc → `gm_<id>.amxx` | (b) Generic "graph runner" plugin interpreting compiled graph data |
|---|---|---|
| Fit with the mode system | Perfect: it is a normal mode plugin in `<id>.plugins.ini`; servers without the app run it | Needs the runner plugin + a data file per mode; also fine, but a second kind of thing |
| Toolchain | amxxpc is already bundled (`core::compile_plugin`); generated code is straight-line `loner.inc` calls | A Pawn interpreter for a typed graph: a big plugin, cell-typed data, slow string handling |
| Performance | native | interpreted in Pawn: fine for events, worse for per-frame or For-Each-heavy graphs |
| Hot reload | compile (1-2 s) → restart the local server map (`srv_map` or `sv_restart` + `amxx` reload on map change: ~5-10 s) | re-read the data file at round start: seconds, no restart |
| Debugging | log lines emitted by generated code; "open as code" is honest | the runner logs node ids; code view is impossible |
| Failure modes | generated code must always compile: the generator is tested against every node's template; errors shown on nodes | runtime type errors inside the interpreter |
| Effort | generator ~2 weeks (templates per node, variables, subgraph inlining, exec ordering), tests 1 | interpreter ~4 weeks + data compiler ~1 |

**Recommendation: (a) for the first milestone**, and probably for good. Hot reload is the only
thing (b) wins, and a 5-10 s local restart is acceptable in an editor loop (the Playtest button
already does it). The graph stays the source of truth (shipped in the mode's catalog part with the
generated `.sma` as an artefact), so a future runner could still read it. Lua: not pursued
(user decision; Plugin Studio covers code).

### 8.5 First milestone (end to end: editor → live server with bots)

Nodes: **Round Started, Player Killed, Is Team, Give Weapon, End Round** (plus Broadcast for
feedback). Flow: new graph from a template ("Team DM: give the killer's team a rifle") → validate →
compile (generator → amxxpc; errors mapped back to nodes) → register the mode (writes
`modes.ini` line, `<id>.cfg`, `<id>.plugins.ini` with `gm_components gm_modekit gm_<id>`) →
Playtest (local server in the mode with N bots, `ls_debug 1`) → nodes light up as bots die →
join. Then: Timers, Variables, Conditions set, component events/actions as the components land.

### 8.6 Where graphs fit with data-only and code modes

`[mode] rules = graph:<id>.graph.json` in the `.mode.ini`; the generated plugin reads the same
`.mode.ini` for everything data can express (teams, spawn, equipment, HUD slots), so a graph only
holds what is specific. A data-only mode (`rules = data`) is run by `gm_modekit` alone. A code
mode (`rules = plugin:gm_<id>`) is the Plugin Studio route. All three appear the same to
gamemodes.sma, the catalog and the MCP tools.

---

## 9. Plugin Studio (Tools > Make > Code): AMX Mod X scripting made easy

- **Editor**: Monaco (MIT) in a lazy chunk; a Monarch tokenizer for Pawn written by us (~150
  lines: keywords, `#` directives, `^` escapes, tags, `public`/`stock`/`native`/`forward`), themes
  matching the app.
- **Language help from the server's own `.inc` files**: a small Rust parser (`tools/pawn.rs`)
  reads `addons/amxmodx/scripting/include/*.inc` (and our `loner.inc`) for `native`, `forward`,
  `stock`, `#define`, `enum`, `const` with their doc comments → completion items, signature help,
  hover docs, go-to-definition (opens the include read-only). Served at `/api/tools/pawn/symbols`
  (cached per include set) and `/api/tools/pawn/compile`.
- **Inline errors**: run the bundled amxxpc (`core::compile_plugin` already does; expose the raw
  output), parse `file(line) : error 0NN: message` / `warning 2NN`, show squiggles and a problems
  list; one click on an error jumps.
- **One-click compile + load + reload + live log**: compile → copy to `plugins/` → ensure the
  plugin is in the current mode's `<id>.plugins.ini` → on the local server `amxx pause/unpause`
  for a changed plugin isn't enough (AMXX loads plugins per map), so **reload = `sv_restart`/
  `changelevel` of the local server** through the panel API (`srv_map`), 5-10 s, with the server
  console and AMXX log streamed into a Log drawer (filtered by plugin; `log_amx` lines clickable to
  the source line when they carry `file:line`). `ls_debug` messages from `loner.inc` helpers.
- **Templates/snippets**: "Mode in 40 lines" (§2.9), "Component event handler", "Chat command",
  "Timer", "HUD slot", "Bot goal", "Persist stat"; snippets for every `loner.inc` call with
  placeholders; "open as code" from the Studio lands here.
- **`loner.inc`** is the product of this work as much as the editor: every component and kit
  call, documented with `/** */` so hover docs come from the same source; the Studio's node registry
  references these symbols, so docs are written once.
- Files live in the game's `scripting/` folder (the user's own) or in a mode's folder; Save writes
  the file; the catalog part packer picks it up as `compile[]`.

---

## 10. Tools additions

### 10.1 Radar / overview generator (format researched 2026-11-12)

Files: `cstrike/overviews/<map>.bmp` + `<map>.txt` (the GL renderer prefers a `.tga` and falls back
to an 8-bit BMP; software mode reads only BMP, so we write BMP, optionally TGA too). Text syntax,
case-insensitive keys, unknown key = parse failure, only the first layer is used by the client:

```
// overview description file for de_dust.bsp
global
{
    ZOOM 1.20
    ORIGIN 101 1071 -192
    ROTATED 0
}
layer
{
    IMAGE "overviews/de_dust.bmp"
    HEIGHT -192
}
```

`global` also accepts `inset X Y W H` (default 4 4 240 180). BMP: **1024×768, 8-bit indexed**;
transparency = a palette entry of pure green `0x00ff00`. Mapping (`cl_dll/hud_spectator.cpp`,
`DrawOverviewLayer`): the image spans world extents ±4096/ZOOM along the long axis and
±4096/(ZOOM·4/3) along the short one, centred on ORIGIN (X Y, and Z = the layer's HEIGHT in the
game's own output), ROTATED swaps axes. Several `layer` blocks parse but the client uses the first.
The game's own method is `dev_overview 1` in a 1024×768 window + screenshot; there is no official
generator (HLTV reads the same files). The overview is for **spectators and HLTV only**: it does not
replace the in-game radar circle. Clients get custom overviews when `maps/<map>.res` lists
`overviews/<map>.txt` and `.bmp` (in-engine download or FastDL), so the generator also writes or
appends the `.res` entry and the map copy step (`servers.rs` already knows `overviews/` as a
custom-map need). Sources: TWHL "Creating Overviews", fysiks' notes, Valve's `hud_spectator.cpp`,
client-limits.md §7.

Implementation (`APP/src-tauri/src/tools/overview.rs` + a button in the viewer and the editor):
1. Top-down orthographic render: ask the three.js viewer to render an `OrthographicCamera` to an
   offscreen 1024×768 canvas with the viewer's lit materials and POST the PNG back (less code than
   a Rust rasteriser and identical to what the viewer shows); a headless Rust fallback later.
2. Bounds: world extents from the BSP, optionally cropped to the nav mesh so void doesn't waste
   pixels; choose ROTATED to put the long axis horizontal; ZOOM = 4096 / half-extent of the long
   axis; ORIGIN = the centre; HEIGHT = the chosen slice's top.
3. Write the 8-bit BMP (palette quantised from the PNG; `goldsrc::wad` already handles palettes;
   mark sky/void pixels pure green) and the `.txt`, into the game's `overviews/` and the server's.
4. Options: height slice per floor for stacked maps, darken outside the nav, markers for mode
   components (zone centre, extracts), "check in game" through Playtest in spectator.
Effort: 1 week.

### 10.2 Editor Playtest button

Save → compile with SDHLT (decided: pinned download, Windows exes, CrossOver on macOS) → copy
`name.bsp` into the game's and the local server's `maps/` → write the map pack if needed → start
the local server in the chosen mode (`srv_mode`, `srv_map`, bots = N; the mode defaults from the
map prefix) → join (`/api/app/launch +connect`). With a graph or plugin under test, `ls_debug 1`
and the Studio/Plugin Studio attach to the log. Stages shown; errors in plain words (leak →
pointfile drawn; no nav → "bots learn it first"). Effort: 1 week after the compile task.

---

## 11. MCP connector: letting coding agents drive the tools

**Shape.** An MCP server inside the app's existing local axum listener (`/mcp`, streamable HTTP
JSON-RPC; the `rmcp` Rust SDK (Apache-2.0) or a small hand-rolled handler, the protocol is thin).
**Off by default**; a Settings toggle "Let coding agents use the tools" starts it on
`127.0.0.1:<port>` only, shows a **per-session token** (new on every start, copied from the UI;
`Authorization: Bearer`), and an audit log of every call. Two switches under it: **Allow edits**
(off → only read/query tools are listed) and **Allow server control**. Tools carry MCP annotations
(`readOnlyHint`, `destructiveHint`); any destructive call (overwrite a map, delete, stop a server
with players, uninstall) blocks on an **in-app confirmation dialog** (the existing
ConfirmProvider) and returns "declined" if refused. A ready-made client line is shown:
`claude mcp add --transport http loner-strike http://127.0.0.1:<port>/mcp --header "Authorization: Bearer <token>"`.

**Tool groups** (thin wrappers over the routes the pages already call; names are stable API):

| Group | Read-only | With edits / server control |
|---|---|---|
| maps | `maps.list`, `maps.info` (entities, bounds, nav/lsv status), `maps.query_entities`, `maps.doctor`, `maps.health_check` (runs the hidden server scenario, returns the report), `maps.overview_preview` | `maps.generate_overview`, `maps.write_map_pack` |
| editor | `editor.open`, `editor.doc`, `editor.checks`, `editor.export_map` (to a temp path) | `editor.new`, `editor.cmd` (the same `{op, …}` as `POST cmd`: brushes, entities, components, textures, groups), `editor.undo/redo`, `editor.save`, `editor.compile` (SDHLT, streams progress), `editor.playtest` |
| nav | `nav.info`, `nav.issues` | `nav.fix` (ops the nav fixer offers), `nav.learn` (bots learn, via the hidden server) |
| modes | `modes.list`, `modes.definition`, `modes.components` (the registry), `modes.graph_get`, `modes.validate_graph`, `modes.generate_pawn` (returns the sma), `pawn.symbols`, `pawn.compile_check` (compile to a temp dir) | `modes.graph_set`, `modes.save_definition`, `modes.compile_plugin`, `modes.register` (modes.ini + cfg + plugins.ini), `modes.package` (catalog part zip) |
| server | `server.list`, `server.status`, `server.logs` (tail, filter) | `server.start_local` (mode, map, bots, `ls_debug`), `server.set_mode`, `server.change_map`, `server.rcon_allowlisted` (`bot_quota`, `sv_restart`, our `gm_*`/`ls_*` commands only), `server.stop` (destructive) |

Design rule that makes this natural: every tool is one existing or planned HTTP route, so the UI,
the Studios and agents share one code path and one permission model; nothing in the MCP layer
touches files itself. Phases: M1 read-only maps/modes/pawn tools + the toggle/token/audit (1 week);
M2 editor and nav edit tools behind "Allow edits" (1 week); M3 server control + playtest (1 week);
docs page "Using Loner-Strike with a coding agent".

---

## 12. Phased roadmap (one person who knows the repo; add ~30 % for review and testing)

| Phase | What | Weeks | Depends on |
|---|---|---|---|
| **A. Components + kit core** | `gm_components.amxx` with the first components (respawn_point, weapon_spawner, damage_zone, timed_door, trigger_trap, capture_point, safe_zone + shrinker, loot_container, supply_drop, barricade, wave_spawner, extraction_zone, checkpoint, moving_platform, player_class, meeting_point), the classname trick, map packs; `gm_modekit.amxx` (definition reader, roles, phases, win, spawn/respawn, equipment/shop, scoring/persistence, HUD manager, scheduler, modifiers); `loner.inc` v1 + docs; schema; `base.cfg` additions; fork: `ls_mode` commands, `ModeGoal`, nav costs (`avoid`/`block`), `attract`, `ride`; component definitions json; mode designer forms (minimal) | 6 | — |
| **B. Last Stand** | waves.ini, turret, revive, shop extras, `ls` rules via `loner.inc`, defender profile; enemies = bot zombies first | 2.5 | A |
| **C. Creatures v1** | `CCreatureImprov` factor-out, `ls_creature` (runner, brute, spitter, boss), brain enemy classes + aim model on entities, creature bridge; MonsterMod spike (fork choice, Windows/Linux, ReGameDLL) → add-on part + `metamod`/`needs` installer keys + messages | 4 + 0.5 | A (parallel with B) |
| **D. Battle Royale** | squads feature, `br` rules, br profile + loot/zone/flee goals, map packs for 2 stock maps, radar tricks verified, beam budget measured | 3 | A |
| **E. Editor: component library** | drag-in panel, typed inspectors from the component json, 3D visuals, mode checks with fixes, map-pack editing, templates/prefabs (first 3), overview generator | 4 | A (compile task separate) |
| **F. Game Mode Studio M1** | React Flow shell (lazy), registry + Zod schemas, graph JSON + undo, the five first-milestone nodes, generator → amxxpc, register mode, Playtest hookup, debug highlighting from the log | 4 | A, Playtest |
| **G. Plugin Studio + loner.inc polish** | Monaco + Pawn tokenizer, `.inc` symbol parser, completion/hover/signature, inline amxxpc errors, compile+load+reload+log, templates/snippets, "open as code" | 3 | A |
| **H. The Predator** | abilities, defences, `pred` rules, hunter/hunted profiles, hiding-place learning | 3 | A, D (zone optional) |
| **I. Studio M2** | Timers, Variables, Conditions, component event/action nodes for every shipped component, subgraphs, soft breakpoints, auto-layout, copy/paste, comments/groups/reroutes | 4 | F, components as they land |
| **J. Extraction** | stash (SQLite), insurance/MIA, timetable, `ex` rules, pmc/scav profiles, contested-area memory | 4 | D, C |
| **K. MCP connector** | M1 read-only + toggle/token/audit; M2 edit tools; M3 server control/playtest; docs | 3 | E, F, G (tools exist to wrap) |
| **L. Traitor** | bodies/ID/DNA, meeting point polish, shop/traps, `tr` rules; suspicion model + botchat-tr | 4 + 3 (bots) | A |
| **M. Combinations, modifiers, small modes** | infection, bounty, race features; combo definitions; modifier pool tuning; Commander, King of the Ramp | 3 | D, J, surf merge |
| **N. Hunted, Heist** | guard profile, alarm, camera, bags | 4 | H, J |
| **O. Templates, prefabs, own creature models** | 5 mode templates, prefab library, 4 + 1 creature models | 3 (+ art) | E, C |

Total ≈ 60 weeks of engineering plus art. A self-contained first milestone that feels like a
release: **A + B + C (bot zombies first) + D + E + F + G** ≈ 26 weeks: two new modes with bots, the
component library in the editor, both studios end to end, a playtest loop. Per the 1.0.0 memory
note (feature complete, long gap after), the user decides whether that goes into 1.0.0 or opens
1.1 (§15).

---

## 13. Risks

| Risk | Why it matters | Mitigation |
|---|---|---|
| Beam, message and HUD budgets | a fixed client beam pool, 192-byte messages, a reliable channel that kicks at ~4 KB, 4 HUD channels for everything | Rings as 24-32 unreliable segments ≤1/s to nearby players only (or `env_beam` entities); lasers and precache counted per map by the checks; one HUD manager with slots; measure net graph with 32 players |
| MonsterMod has no releases and needs `info_node`s | we build and host it; monsters wander badly on node-less CS maps | spike first; our nav-mesh creatures are the default; the editor places nodes and the checks warn |
| Edicts on big maps (loot, bodies, creatures) | default 900, hard 2047 | Per-component budgets and recycling, `-num_edicts 2048` from the server control service, peak edicts in the health check (`task_edicts` exists) |
| The custom-classname trick | brush components and keyvalue ordering differ between compilers | `FM_KeyValue` + `FM_Spawn` post is the established pattern; test with SDHLT and J.A.C.K.-compiled maps; map packs as the fallback |
| Brain changes destabilising Classic | the brain is tuned for CS | Everything under `ls_mode`: no change when no mode is set; the sim A/B setup runs Classic before/after |
| Generated Pawn that doesn't compile | a Studio that fails on compile is worse than none | Every node template compiled in CI; errors mapped to nodes; "open as code" always available |
| Studio debugging expectations | no real breakpoints | Say "soft breakpoints" in the UI; node highlighting from the log is the main tool |
| Creature AI quality | hostage AI is simple | Creatures are only creatures; humans/guards are bots with profiles |
| MonsterMod on Windows/ReGameDLL | forks differ; nothing documents compatibility | A spike first; Linux-only add-on if needed; nothing depends on it |
| HL1 models absent | some installs | `needs` check with a clear message; our creatures work without |
| Maps | no `br_`/`ex_` maps; ±4096 | Map packs on stock maps first, templates, big-maps.md guidance |
| Persistence/cheating (stash) | authid spoofing on non-Steam servers | nvault by name only with a warning; owners can reset |
| Traitor with mostly bots | may feel hollow | the suspicion model is the point; go/no-go after 2 weeks of H |
| MCP safety | agents editing files or stopping servers | off by default, local, token, read-only default, confirmations, audit log, rcon allow-list |
| Licences | GPL plugins, Valve SDK bits (SDHLT, MonsterMod), EPL (ELK), community models | nothing with a Valve clause or EPL in our tree; dagre not ELK; catalog parts pin and credit; own creature models for 1.0.0 |
| Scope | 15 phases | the 26-week milestone is self-contained; later modes are independent |

---

## 14. Effort summary

| Block | Weeks |
|---|---|
| Components + kit + loner.inc + brain plumbing (A) | 6 |
| Modes: LS 2.5, BR 3, Pred 3, EX 4, TR 7 | 19.5 |
| Creatures + MonsterMod add-on (C) | 4.5 |
| Editor component library + overview (E) | 4 |
| Game Mode Studio M1 + M2 (F, I) | 8 |
| Plugin Studio (G) | 3 |
| MCP connector (K) | 3 |
| Combos/modifiers/small modes, Hunted/Heist, templates/models (M, N, O) | 10 + art |

---

## 15. Open questions for the user

1. **Which modes first?** Proposed: Last Stand → Battle Royale → (studios) → Predator → Extraction →
   Traitor. BR first is possible (the zone/loot components then lead phase A).
2. **1.0.0 or 1.1?** The 26-week milestone (components, two modes, editor library, both studios)
   is ~6 months. Feature-complete 1.0.0 with it, or 1.0.0 without and this opens 1.1?
3. **Component set for phase A**: the 16 listed, or a smaller first cut (respawn, spawner,
   zone+shrinker, container, barricade, wave spawner, extraction) with the rest as they're needed?
4. **Creature models**: commission/make our own set (who, cost), or ship with player-model zombies +
   HL1 models when present?
5. **MonsterMod fork**: accept a Linux-server-only add-on if no fork runs on Windows HLDS under
   ReGameDLL, or drop it until one does?
6. **Extraction persistence**: per server (plan) or synced through the community service later?
7. **Definition format**: INI as proposed (AMXX 1.9-safe) or JSON (needs 1.10's json natives; which
   AMXX does the installer pin?).
8. **Traitor's one-team secrecy** (kills radar/team chat; TTT does it) vs two teams with cosmetic
   hiding?
9. **Studio hot reload**: is a 5-10 s local restart acceptable, or is the graph-runner's instant
   reload worth its cost later?
10. **MCP scope**: local only (plan) or also reachable from the server control service for remote
    servers later?
11. **Surf branch**: King of the Ramp / Surf+DM need `ls-surf` merged; planned before these?
12. **Voice for Commander**: wait for voice-to-text (0.1.3 plan) or ship chat-only first?

---

## 16. Research status and follow-ups

- Read and folded in: `docs/research/existing-modes.md` (no established BR/Extraction on CS 1.6;
  TTT by GuskiS worth studying; ZP's round-mode/extra-item API as the composable pattern; one HUD
  manager), `docs/research/big-maps.md` (±4096 cube, stack vertically, prototype on stock maps),
  `docs/research/client-limits.md` (TE layouts and caps, 192-byte messages, DHUD 8×128, `Radar`
  dot trick, `RoundTime`/`HideWeapon`, scoreboard messages, overview via `.res`, precache 512:
  §3.1, §3.6, §10.1), `docs/research/monstermod.md` (JulianR0/monstermod-redo as the maintained fork,
  no releases, `info_node` dependence, HLSDK terms, HL models present on the local HLDS: §6.3-6.4),
  and the prior-art/overview/AMXX research summarised in §2.6, §3.6, §10.1.
- To check in code or in game before building: `HostagePos` dots without real hostages; the
  client beam pool on the 2013+ build (64 vs 128); per-client `set_rendering` via an
  `AddToFullPack` hook on ReGameDLL; `BarTime` outside defusing; `DeathMsg`/`ScoreInfo` hiding for
  Traitor; player-model sequence names per model; `mp_freeforall` with kit parties; the
  `FM_KeyValue` classname trick with brush entities compiled by SDHLT; `monstermod-redo` built for
  Windows under Metamod-P/ReHLDS/ReGameDLL; HL1 models on a CS-only Steam account; ELK's licence if
  anyone wants it over dagre; which AMXX version the installer pins (JSON natives).

## Addendum (user, 2026-11-12): Game Mode Studio debugging, made explicit
1. **Node highlighting:** highlight the node executing now; show execution paths; execution wires drawn
   differently from data wires; show each node's last result; failed conditions marked visibly.
2. **Soft breakpoints:** per-node breakpoints; reaching one freezes gameplay (world + timers); show the current
   node and its input values; Resume; "disable all breakpoints".
3. **Runtime link (server → app):** the generated plugin emits debug events the app turns into highlighting —
   `node_enter {nodeId}`, `node_exit {nodeId}`, `node_error {nodeId, message}`, `breakpoint_hit {nodeId}`,
   `world_paused`, `world_resumed` — over the local server link (e.g. log lines or a UDP/rcon channel the app reads;
   pick the least fragile), only when debugging is on (zero cost otherwise). No instruction-level stepping.
4. **The graph is ours:** our own versioned JSON schema is the source of truth; React Flow is only its editor, so
   the UI can change without breaking published modes.
5. **Live variable inspection** while frozen at a breakpoint: current round and game phase, alive counts, timers,
   objective/component state, and per player: health, team/role, current weapon (plus any graph variables).

## Addendum (user, 2026-11-12): the Creature Framework and Creature Studio
- **One framework, providers behind it:** a common creature interface (spawn, think, take damage, set target,
  set behaviour, is alive; categories and behaviour profiles) with providers: **ls_creature** (native, first-class:
  hostage-AI-derived CCreatureImprov + bot nav + the brain's senses) and **MonsterMod Redo** (optional catalog add-on,
  GPL-2.0 under the HL SDK terms) through an **adapter** — don't assume its entities fit the interface; wrap what
  it exposes. Bots treat both alike (shoot / avoid / hunt via the enemy-class hooks).
- **References, not code:** the original HL SDK monster AI (schedules, conditions, navigation, combat), original
  MonsterMod (compatibility), Sven Co-op (co-op NPC design). Read for behaviour; write our own.
- **Categories:** infected (headcrab-like, zombies, carriers), hostile aliens (ranged, melee, bosses), wildlife
  (passive, territorial, predators), mechanical (turrets, drones, robots, security), bosses (multi-phase),
  special (flying, teleporting, support, summoning). One framework + behaviour profiles; a turret is a creature
  that doesn't move.
- **Creature Studio (Tools > Make):** type, health, speed, aggression, detection radius, attack type/damage,
  behaviour (chase/patrol/guard/flee), target selection, respawn; spawn groups and waves placed through the editor's
  component library; later custom behaviours via Game Mode Studio nodes.
- **NodeMod** (Node.js in GoldSrc via Metamod): research only; scripting stays AMX Mod X + loner.inc.
