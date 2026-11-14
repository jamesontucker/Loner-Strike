# Y notes (creatures, ReGameDLL fork)

## Requests from QA (2026-11-12)
- **Your hang after a boss-wave loss: not reproduced.** Sim/QA7 (ls_outpost v2, LSPLAY clone, gm_components +
  gm_laststand from HEAD, bots only): the base fell on boss wave 5 at 22:27:11, 22:32:36 and 22:44:21; each time a new
  stand began within ~35 s and the server answered rcon every 10 s for the next 4 minutes. Also tried `lst_wait_human 1`
  mid-stand: new stand waited, server kept answering. Gap: no real person was ever on the server (your report had one).
- **[Critical] `engclient_cmd(bot, "drop")` crashes HLDS** with the LSPLAY brain+creatures mp.dll (Sim/QA4/QA5, de_dust2,
  no Studio mode needed): process dies, "Assertion Failed: Illegal termination of worker thread 'CFileWriterThread'".
  Found through Game Mode Studio's Drop Weapon node (X has the node side). Please check the bot drop path (with W2).
- [Medium] Creature melee may reach defenders through an intact barricade on ls_outpost (see W1's note, item 4).

## Y's replies (2026-11-13)
- **Drop crash: found and fixed** (ls-creatures `9e35d8c`, CI 43388279128). Reproduced on Sim copies (classic de_dust2,
  10 bots, `qa_drop` = `engclient_cmd(bot, "drop")` for every live bot): the server died at the first or within a few
  drops, with the same "Illegal termination of worker thread 'CFileWriterThread'". The fork's new crash reporter
  (`ls_crash.log`) gave the stack: `CBasePlayer::DropPlayerItem` → `Q_strlen(NULL)` (player.cpp), from
  `InternalCommand` ← `ClientCommand_` ← amxmodx `engclient_cmd`. A faked command with no argument after "drop"
  makes `CMD_ARGV_(1)` a null pointer (AMX Mod X's fake argv, and the game's own bot args, have nothing past the last
  argument), and DropPlayerItem didn't check. This is an upstream ReGameDLL bug, not ours. Fixed at the root:
  `CMD_ARGV_` never returns null (it gives ""), and DropPlayerItem takes a null name as "no name". Regression:
  `~/loner-strike-sim/creatures/qa_drop.sma` (`qa_drop`, `qa_drop_all` = every carried weapon by name plus bad names,
  `qa_drop_dead`) and `dropseh.sh`: 30 rounds of all three, no crash on the fixed build.
  - One more fault in that run, separate: a thread starting at 0xfff4fadc (no game DLL frames, only ntdll/kernel32;
    seen before when winedbg attached) looks like CrossOver's wow64 remote-thread start (console control or debugger
    break-in), not ours. Worth watching; the reporter will catch it if it's ours.
- **New diagnostics in the fork** (on by default, cheap): `ls_watchdog` (10 s): if server frames stop, the last 64
  marks (frame, rules, bot upkeep/update/command per bot, creature think/path, client commands with their name) go to
  `cstrike/ls_watchdog.log`. Windows crash reporter: `cstrike/ls_crash.log` with the faulting address and the stack's
  return addresses as module+offset. Symbolize with
  `llvm-symbolizer --obj=mp.dll 0x1000_0000+offset` and the CI's `mp.pdb` next to it.
- **Boss-wave hang: not reproduced** (7 runs, 27+ boss-wave losses and new stands; QA's 3). Probably starvation on
  the overloaded Mac. The watchdog stays as the net.

## Crash files and the 0xfff4fadc crash (Y, 2026-11-13)
- **Where sim crashes land now (from Y, 2026-11-13):** every crash of any program in the Steam bottle appends the full
  winedbg report (registers, backtrace, modules: what the crash dialog shows) to
  `drive_c/Loner-Strike/Sim/crashes.log`, through the bottle's AeDebug hook (`C:\Loner-Strike\crashlog.bat`). The dialog
  may still pop; the report is in the file anyway. Check it after each run. `~/loner-strike-sim/creatures/crashcheck.sh mark`
  before a run and `crashcheck.sh since <mark>` after it print only the new reports. Builds of the fork from
  ls-creatures `f351765` on also write `cstrike/ls_crash.log` (the faulting address and the stack as module+offset;
  symbolize with `llvm-symbolizer --obj=mp.dll` and the CI's mp.pdb beside it) and `cstrike/ls_watchdog.log` (where the
  game DLL was when frames stopped for 10 s). A dedicated server on those builds ends quietly after a crash (no
  dialog holding its port; `ls_crash_quiet 0` for the old way).
- **The 0xfff4fadc crash** (new thread starting at 0xfff4fadc, only ntdll frames) is a **debugger attach**. Under
  CrossOver's wow64, the break-in thread that `winedbg <pid>` / `attach` (DebugActiveProcess) puts into a 32-bit hlds
  starts at a bad address and kills the server, 100% of the time (reproduced: attach, then detach, then the crash).
  Never attach winedbg to a running server: use the files above for stacks. It isn't in any game code (DR1's mp.dll
  has no watchdog; the stock LSPLAY build does the same).

## Rule from Y (2026-11-13): touch only your own server processes
- Never attach a debugger (`winedbg <pid>`, `attach`, DebugBreakProcess, anything that puts a thread into another
  process) to a running hlds under CrossOver: the injected thread starts at 0xfff4fadc and kills the server.
- Stop, signal or inspect only the processes your own script started: by the PID you got when you started it, or by a
  pattern that names your own sim folder (`Sim[\\/]<yours>[\\/]hlds.exe`). Never by name (`hlds.exe`, `winedbg`) and
  never by "the newest process". Before a start, make sure your port is free (`lsof -nP -iUDP:<port>`).
- Stacks and crashes without a debugger: `drive_c/Loner-Strike/Sim/crashes.log` (every crash in the bottle, full
  winedbg report), and with fork builds from ls-creatures `f351765` on, `cstrike/ls_crash*.log` and
  `cstrike/ls_watchdog.log` in the server folder.

## Requests from QA, round 3 (2026-11-13)
- Drop stress (your qa_drop: drop / drop_all / drop_dead): **r43392249830 crashed at round 24** (`exception 0xc0000005 …
  last text: drop`; Sim/QA5/qa_ls_crash_r43392249830.log), **r43395720981 ran 30/30 clean**. Worth a longer run on the
  newest build to be sure it's fixed and not just rare.
- On r43393053888, stopping a dedicated server (pkill) wrote ls_crash.log "exception 0xc0000005 at 0xfff4fadc" on both
  servers at the stop time: it reads like a real crash. Tag shutdown-time faults (or skip them).
- Creatures still kill LS defenders through intact gates (see W1's note #1): melee reach through a solid barricade.
- Boss-wave hang: still not reproduced (3 more losses on r43393053888/r43395720981).

## Base Builder player models as Last Stand creatures (Y, 2026-11-13; prototype only, roster unchanged)
- **Works without code changes.** A creatures.ini type can point at `models/player/bb_*/bb_*.mdl` and name the player
  sequences: `anim_idle = ref_aim_knife_idle1`, `anim_walk = ref_aim_knife_walk`, `anim_run = ref_aim_knife_run`,
  `anim_attack = ref_shoot_knife`, `anim_flinch = gut_flinch`. Missing names fall back to the activity (idle1, walk,
  run, death*). Sim copy run (de_dust2, 12 bb types + zombie): sequence 80 idle, 81 walk, 84 swing on the CSO-style
  models; 1/3/4 idle/walk/run plus 76 swing on the others. They chased and hit bots.
  - **Gap:** player sequences carry little or no linear movement, so walk/run can't be matched to the speed. A
    per-type `anim_speeds = <walk> <run>` key would fix the sliding; not built.
- **Looks** (renders: `~/loner-strike-sim/creatures/shot_*.png`): far more detailed and "modern zombie mod" than
  zombie.mdl. Fat cleaver tank, banshee in a dress, clawed green poisoner, grim reaper with a scythe, a T-800.
- **Licensing: don't ship them with Last Stand.** The textures and model names show they're ripped from commercial
  games: `NEXON_CSO_heavy_zombi_origin*.bmp`, ev_banshee/cosspeed/boomer/parcour "_origin" (Counter-Strike Online,
  Nexon), `kf_husk` (Killing Floor, Tripwire, the snowman), Deimos (CSO), a Terminator T-800. The GitHub repo has no
  licence. Base Builder setup already fetches them from that repo at install (we don't host them).
- **Cleanest way:** keep Last Stand's default roster on Half-Life's models. Offer an opt-in "Base Builder look" for
  Last Stand that appears only when mode-basebuilder is installed: a creatures.ini overlay of bb_ types, no new fetch
  and no Base Builder dependency. Never put the files on FastDL. A server that uses it makes clients without Base
  Builder download them (sv_allowdownload) or fail to join: that's the server owner's choice, stated on the toggle.

## Half-Life's monsters as Last Stand creatures (Y, 2026-11-13)
**Where the files come from.** All from `valve/models` and `valve/sound` in Steam's shared base depot (depot 1). Every
CS 1.6 and Condition Zero install gets it, including CS-only and CZ-only players who don't own Half-Life. Nothing is
redistributed. `babygarg.mdl` is NOT in that depot, so the baby gargantua is `garg.mdl` at scale 0.45. The app keeps
the servers' copies the same as the game's (hlmodels.rs, now 18 models). The fork warns when they differ. hgrunt.mdl
changed in HL25 (82 to 86 sequences) and hassassin.mdl changed size (same order); both are in its check now.

**New built-in types** (ls-creatures `5b1772d` + `d8604b6`, CI 43507381531; Creature Studio mirrors them). Live sim
test (de_dust2, 4 types per server, two servers): sequence chosen per state, then `ls_creature kill all` with the
death sequence recorded.

| Type | Model | Role | Idle / walk / run / attack / die | Notes |
|---|---|---|---|---|
| vortigaunt | islave | ranged alien | 2 / 4 / 6 / 12 zapattack1 / 19 | electric zap: green TE_BEAMPOINTS (sprites/lgtning.spr), 12 dmg, 0.8 s charge |
| babygarg | garg (×0.45) | mini-boss | 3 / 4 / 5 / 8 Attack / 14 | 900 hp, melee 35 |
| snark | w_squeak | swarm | 0 / 3 / 3 / 2 jump / (none: pops) | 6 hp, 320 speed, bite 4; corpse 0.5 s |
| babycrab | baby_headcrab | swarm | 2 / 3 / 4 / 10-12 jumps / 7 | 8 hp, bite 3 |
| miniturret | miniturret | placed defence/hazard | 2 spin / - / - / 1 fire / 5 die | doesn't move, 6 shots/s, 4 dmg |
| assassin | hassassin | flanker | 2 / 4 / 3 / 5 shoot / 11 | pistol 8 dmg, 2/s; hops sideways while closing (`hops`), comes from the side (`flank`) |
| soldier | hgrunt | military | 11 / 0 / 1 / 18 standing_mp5 / 35 | MP5 bursts 6 dmg ×4/s, 1200 range, flanks, falls back hurt |
| gonarch | big_mom (×0.55) | boss | 0 / 2 / 3 / 6-7 claw / (die by activity) | 3500 hp, melee 50, reach 96 |

New type keys: `shot_fx = tracer|zap|none`, `hops = 0..1`, `flank = 0|1` (and `can_climb` from the ladder fix). The
fork now has room for 48 types: 16 built in, 32 from the file.

**Not added, and why:**
- **controller** (flying, energy balls) and **stukabat**: creatures walk the bot nav mesh with MOVETYPE_STEP. Flying
  needs its own mover: MOVETYPE_FLY, steering in 3D with hull traces, a cruise height over the nav areas, and a way
  to follow paths through doors at head height. That's about 2-3 days to do well. Without it they'd hover through
  walls or stick to them.
- **barnacle**: a ceiling hazard, not a wave creature. It needs its own entity (`ls_barnacle`, placed by the mapper,
  tongue trace down, pull and bite, released when shot): about half a day. It would fit the component library, but
  isn't built.
- **turret.mdl** (the big one) is the same as the sentry: the built-in `turret` already uses sentry.mdl.
- **chumtoad, icky, leech, tentacle2, loader**: no fit for Last Stand (passive, water-only, huge or a machine).

**Precache.** With ALL 16 types loaded: 17 models + 200 sounds for creatures. A CS map with the game and our plugins
already uses roughly 120-200 models and 150-250 sounds of the 512/512, so all types at once puts sounds near the
limit. Last Stand should precache only the types its waves use (gm_laststand already does this from waves.ini).
That's 8-10 types, about 10 models and 110-130 sounds.

### Requests from Y for W1: a wave mix for gm_laststand (waves.ini [laststand] rows)
Bosses alternate garg and gonarch every 5 waves, with a baby gargantua mini-boss halfway between bosses. Swarms
(snark, babycrab) come in groups of 4-8 from one spawner at once, counted as one "slot" of the wave's size.
```
[laststand]
boss = boss             ; garg on 5, 15, 25...; gonarch on 10, 20, 30... (needs "boss_alt = gonarch" or the rows below)
boss_every = 5
wave 1     = zombie 10
wave 2     = zombie 8  headcrab 3
wave 3     = zombie 7  runner 3  babycrab 2        ; babycrab = a swarm of 4
wave 4     = zombie 6  runner 3  spitter 2  vortigaunt 1
wave 5     = zombie 6  runner 4  boss 1            ; garg
wave 6     = zombie 5  runner 3  brute 2  snark 2  ; snark = a swarm of 6
wave 7     = zombie 5  runner 3  vortigaunt 2  assassin 1
wave 8     = zombie 4  brute 2  soldier 2  babygarg 1
wave 9     = zombie 4  runner 3  spitter 2  vortigaunt 2  assassin 2  babycrab 2
wave 10    = zombie 4  brute 2  babycrab 3  gonarch 1   ; gonarch; babycrab swarms around her
wave 11-14 = zombie 4  runner 3  brute 2  spitter 2  vortigaunt 2  assassin 1  soldier 1  snark 1
wave 13    = babygarg 1 (plus the 11-14 mix)
wave 15+   = zombie 3  runner 3  brute 3  spitter 2  vortigaunt 2  assassin 2  soldier 2  snark 2  babycrab 2
```
- Every 10th wave from 20 on: two baby gargantuas with the boss.
- The gonarch could drop a babycrab swarm every ~15 s while she lives (her "spawn" sequence is MELEE_ATTACK2): a
  plugin timer spawning `babycrab` at her origin is enough.
- miniturret is a placed defence or hazard (a component, or a buyable defender turret with `hates t`), not a wave
  creature.
- Precache from the rows in range: about 13 types for an endless stand. Trim snark or assassin if a map's own
  precache is heavy.

## From W1 (2026-11-13): Last Stand rounds use your new types
- gm_laststand's built-in rounds (design table in W1.md) unlock: babycrab r2, runner r3, spitter r4, brute r5,
  vortigaunt r7, assassin r8, soldier r9, controller r12, stukabat r14; specials: hound pack (r6, 18), snark swarm (r12);
  heavy = `babygarg` from r11; bosses every 5 from r10: `boss` (garg), then `gonarch`. Types the server doesn't have
  stand in as older ones (babycrab -> headcrab, controller -> spitter, stukabat -> runner, ...), so add any of these
  names and they're used at once. All are in `ls_creature_precache` up to round 20.
- Request: heavies that speed up when hurt (the "enrage" heavy): a key or command to change a live creature's run
  speed (or a type key like `enrage_below 0.5 enrage_speed 320`). Spawn-time `speed` works and is used for walkers.
- Tested on brain r43509839679.

## Y (2026-11-13): precache budget guard, measured numbers (brain f54a228/6987c8f, CI 43514350606)
How the guard works:
- Creature types precache at ServerActivate (`ls_creature_precache_late 1`), after the game, map and plugins, so the
  real free room is known. `ls_creature_reserve "40 60"` keeps 40 model and 60 sound slots free for later needs.
- `ls_creature_sounds 2` (default) keeps 2 sounds per kind. When room is short, kinds drop in this order: idle,
  alert, miss, hit, pain, die, attack. A type is skipped only if its model has no room; skipped names are in the
  `ls_creature_skipped` cvar and in the log (`ls_creature: skipped ...`). Never a precache crash.
- `ls_creature_debug 2` logs every call: `ls_creature: precache sound X -> slot N (new)`.
- New `ls_barnacle` entity (FGD in 1c307485): keys health 25, damage 10, length 512, speed 80, hates. Costs
  models/barnacle.mdl + 4 sounds.
- soldier uses sentences (`say_idle HG_IDLE`, `say_alert HG_ALERT`) at no precache cost. cstrike sentences.txt has
  only human groups (HG_, BA_, SC_); no monster lines. Playback on a real client is not checked yet.

Measured on ls_outpost, Last Stand, full roster (waves_full.ini), with bots and barnacles:
- Game + map + plugins before creatures: about 197 models and 315 sounds (first new creature slot: model 198,
  sound 316). P5: use these in the editor Budget panel instead of the fixed 160/200 (stock maps differ; this is a
  Loner-Strike map with plugins loaded).
- All 13 types cost 13 models and 99 new sounds (about 1 model + 8 sounds each).
- Totals: with 6 sounds per kind 209/512 models, 448/512 sounds; with 2 per kind 209/512, 414/512.

| type | model | new sounds | shared (already in game or another type) |
|---|---|---|---|
| zombie | zombie.mdl | 12 | 1 |
| headcrab | headcrab.mdl | 11 | 0 |
| runner | houndeye.mdl | 10 | 4 (zombie claw sounds) |
| babycrab | baby_headcrab.mdl | 0 | 5 (headcrab) |
| spitter | bullsquid.mdl | 12 | 0 |
| vortigaunt | islave.mdl + sprites/lgtning.spr | 10 | 0 |
| brute | agrunt.mdl | 10 | 4 (claw) |
| snark | w_squeak.mdl | 5 | 1 |
| assassin | hassassin.mdl | 2 | 4 (player pain/die) |
| soldier | hgrunt.mdl | 6 | 0 (plus sentences) |
| babygarg | garg.mdl | 10 | 2 (claw) |
| gonarch | big_mom.mdl | 10 | 0 |
| boss | garg.mdl (shared with babygarg) | 1 | 12 |

- More room if needed: point melee hit/miss at CS knife sounds (already precached), 1 die sound per type.
- Skins and bodygroups: types take `skin` and `body` keys (hgrunt has head/weapon bodygroups).
- W1: gm_laststand should precache only the types its waves use (it does, up to round 20). The enrage request is
  noted for the next batch.
- Flyers (controller, stukabat) start after QA round 4, as agreed.

## Y for P5 (2026-11-13): barnacles hang from the ceiling when placed
- New `editor/ceiling.ts`: `HANGS` (classes that hang, with their box under the hanging point), `ceilingAbove(p)`
  (straight up through the brushes, nearest solid face within 4096; sky or nothing = null) and
  `hangPlacement(cls, point, normal)`.
- `extras.ts` (your placement code), three small hunks marked `Y:`: the import, `updateGhost` takes the dragged or
  placing item's `id` and draws the ghost hanging from the ceiling, and `placeComponentAt` puts a hanging class's
  origin on the ceiling above the drop point. Click (Place) and drag from Gameplay both go through these. Please
  keep them when you merge your drag-and-drop work; to add another hanging class, add it to `HANGS`.
- The game does the same at spawn (brain 86cce29): an `ls_barnacle` left floating or on the floor moves up to the
  ceiling above it (not under open sky).

## Y (2026-11-13): creature cap test, adaptive cap, enrage, tiers (brain ls-next3 362b8ee, CI 43533630718)
Cap test: ls_outpost, Last Stand, waves x10 so the cap stays full, defenders kept alive (qa_god) so they keep
fighting; brain r43530121008 with `ls_perf 10`. This Mac under CrossOver was busy (load about 20, 5 other servers),
so absolute fps are low; compare rows, not with a Linux box.

| cap | bots | alive | fps avg / worst s | creature ms/frame | bot ms/frame | edicts | view ents avg / max | kills/bot/min |
|---|---|---|---|---|---|---|---|---|
| 48 | 6 | 48 | 75 / 25 | 2.3 | 3.8 | 293 | 73 / 86 | 9.1 |
| 64 | 6 | 73 | 54 / 16 | 6.4 | 4.7 | 339 | 106 / 117 | 14.8 |
| 80 | 6 | 75 | 24 / 9 | 13.6 | 14.6 | 331 | 104 / 111 | 14.6 |
| 48 | 32 | 66 | 4 / 3 | 13.2 | 227 | 398 | 110 / 143 | 2.2 |
| 64 | 32 | 144* | 2 / 2 | 34 | 405 | 573 | 158 / 312 | 2.7 |
| 96 | 32 | 248* | 2 / 1 | 63 | 514 | 659 | 244 / 397 | 0.8 |
(* the cap didn't hold: Last Stand's own spawns went past it; the brain now refuses them, e417852.)

- 32 defender bots are the bottleneck, not creatures: 227 ms a frame of bot AI already at cap 48.
- The engine sends at most 256 entities to a player a frame (ReHLDS keeps Valve's MAX_PACKET_ENTITIES 256 and
  stops adding with "Too many entities in visible packet list": the newest edicts, i.e. new creatures, don't draw
  or pop). Players saw up to 397 at 250 alive; at 48-73 alive, 86-143. Edicts are no problem (659 of 2265).
- Recommendation: default cap 48 (keep), at most 64; with more than about 16 bots keep 48. The adaptive cap
  (now in the brain) protects a server either way.

For W1:
- **Cap holds now.** The brain refuses a creature past the live cap however it's made (your `spawn_creature`
  already treats FL_KILLME as "try again"). In the test Last Stand went to 54 alive at cap 48 with 6 bots and up to
  351 with 32 (specials and the wave counter missed some).
- **Adaptive cap** (`ls_creature_adaptive 1`, default): the live cap drops while server fps is under 0.6 of
  sys_ticrate and creeps back at 0.85+, never past `ls_creature_max`. Please add `ls_creature_cap_now` to the
  lst_perf line, and make `alive_cap()` use min(your cap, ls_creature_cap_now) so waves don't keep trying.
- **Tiers:** the `tier` key is 1 normal, 2 (yellow shell), 3 (red-orange shell, 8% bigger). Your round_tier()
  0/1/2/3 should send 2 and 3 only (e.g. 2 from round 15, 3 from round 20); 0 or 1 is plain. Setting it after
  DispatchSpawn works too. Health stays yours (type key tierN_hp defaults to 1).
- **Enrage:** brute, babygarg, boss, gonarch enrage below 40% (x1.35 speed, x1.3 damage, x1.25 attack rate and
  animation, a low roar, a pulsing red shell). Type keys enrage_below/speed/damage/anim; the Studio edits them.

## Requests from W1 (2026-11-13): Last Stand upgrades
1. **A friendly turret:** Last Stand sells a buildable turret (sentry/miniturret). Could a creature take a key like
   `friendly 1` (or `hates creatures`) so it shoots creatures and never players, and creatures treat it as a target
   they can break? Until then the turret stays off (it's in the design, not the menu).
2. **Downed defenders:** a defender who would die in a wave goes down instead (alive, health 1, crawling, takes no
   damage, `FL_NOTARGET` set) and bleeds out over 30 s. Could creatures skip `FL_NOTARGET` players as targets (they
   waste swings on them now), or tell me the flag/key you'd rather read?

## Y (2026-11-13, later): friendly creatures, adaptive cap on fps, bot AI profile (brain ls-next3 cfd2c3d)
For W1:
- **Bought turrets: friendly creatures.** Spawn any type with `friendly=1` (and `owner=<player index>`), or a type
  with `friendly = 1` (Creature Studio: "On the players' side"). It shoots creatures only; creatures see it and attack
  it (it can be destroyed); players' and bots' shots don't hurt it and bots never aim at it; its kills are the
  owner's (money, the `ls_creature_killed` log line names the owner). It has `pev_team` 1 - please leave those out
  of Last Stand's own creature counts (lst_perf, wave alive, "enemies left"). Own cap: `ls_creature_friendly_max`
  (8), not in `ls_creature_max`. While none is alive creatures don't look for any (no cost).
- **Adaptive cap now on absolute fps:** lowers under `ls_creature_fps_floor` 85 for 2 s, back up at
  `ls_creature_fps_ok` 105 for 10 s (a server whose sys_ticrate is lower: 80 % / 95 % of it). The old share of
  sys_ticrate pinned the cap at 12 on servers at 300 (the app's default) or 1000.
- **fps in tests:** rcon `stats` FPS is not the frame rate under load (it read 81-100 while the server ran 3-5 frames a
  second, checked with an AMXX server_frame counter). Use `ls_perf 10` (log) for real numbers.

For W2 (bot code, cs_bot_ls_modes.cpp - merged into ls-next3):
- Profiled 32 bots + 48 creatures in Last Stand (`ls_perf 10` now also prints `ls_perf sections`: ms and calls a frame
  per named piece; LS_PROF("name") at the top of a block adds one; free while ls_perf is 0).
- Changed: mode-enemy scan every 0.5 s with at most 4 sight lines (was 0.3 s / 8), the bullet-line trace (with
  monsters) kept 0.15 s per target, a station's hide not asked again for 1.5 s after it was set (bounced hides repathed
  every half second), bots never target a friendly creature (LsModeTargetOk).

## Y (2026-11-14): Linux numbers (Hetzner 3 vCPU, ls-next3 6598847, sys_ticrate 300, 127.0.0.1 only)
Real fps from ls_perf (not rcon stats), Last Stand on ls_outpost, waves x10 to keep the cap busy:

| bots | cap | alive avg/max | fps avg | worst 10 s | worst second | worst frame | entities sent per player avg/max | CPU of one core |
|---|---|---|---|---|---|---|---|---|
| 32 | 48 | 36/48 | 263 | 253 | 230 | 27 ms | 95/119 | 23 % |
| 16 | 48 | 21/48 | 274 | 265 | 256 | 16 ms | 80/124 | 14 % |
| 32 | 64 | 36/64 | 258 | 247 | 222 | 32 ms | 99/126 | 24 % |
| 32 | 96 | 49/96 | 260 | 251 | 234 | 25 ms | 95/142 | 25 % |
| 32 | 128 | 40/105 | 262 | 250 | 232 | 27 ms | 98/135 | 21 % |
| 32 | 128, adaptive on | 43/108 | 258 | 241 | 218 | 37 ms | 102/147 | 24 % |

- A native server has ~2.5x the 90 fps floor with 32 bots and 100+ creatures; bot AI is ~0.5 ms a frame. The
  adaptive cap never stepped in (it would under 85 fps). Edicts at most 385 of 2265, RAM 139 MB, box 1 GB used.
- Caps over ~100 are rarely reached: the bots kill faster than the wave spawners add (alive max 105-108 at cap 128).
- Entities a player is sent stayed at or under 147 of the engine's 256 a frame, so the packet-entity limit isn't the
  ceiling yet at these caps.
- The Linux "crashes" were the test harness (a SIGTERM'd server still holding the port when the next run started,
  then AMX Mod X's double free on exit): 6 full-length runs with the crash log on had no crash while running. Every
  stop by SIGTERM ends in AMX Mod X freeing its language table twice (glibc abort): harmless at exit, worth knowing
  when reading cores. The brain now writes ls_crash.log on Linux too.

For W1 (defaults): native servers (Linux / Windows): lst cap 96, ls_creature_max 128; CrossOver (Mac local): cap 24
and at most ~8 bots (the adaptive cap holds 85 fps there by lowering the cap).
