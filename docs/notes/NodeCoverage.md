# Node coverage: every Studio node, run in a real game

The coordinator's sweep (2026-11-14): every node in the Studio's registry (449 types, `make_nodes.py`) built into
small test modes, run on a sim server with bots, and checked against its help text. X did the language, game,
player, world, bot and creature nodes (349 types, below); Y did the components group (105 types: Components,
Upgrades and turrets, Battle Royale, Downed, Waves, Shop, Squads, plus Map markers and Spawn Creature, which both of
us ran), merged at the end from `NodeCoverage-components.md` (7a8715dc).

## Totals

| | PASS | FAIL | NOT RUNTIME-TESTABLE | NOT RUN |
|---|---|---|---|---|
| X: 349 types | 342 (2 after a fix) | 1 (handed on) | 3 | 3 |
| Y: 105 types | see Y's tables below: all ran or refused correctly except Door (fixed, be9b35b9), Buy Perk's Bought (fixed, be9b35b9), Wave Spawn's label (fixed, be9b35b9); not runtime-testable: Barricade Damaged / Broken, Meeting Ended, Shop Buy (a person picks), a real Door | | | |

Over all 449: every node ran except the 3 NOT RUN and Y's 4 not-runtime-testable events; 9 faults were found and
fixed (below), 1 is handed on.

## How (X)

- **Builder** (`scratchpad/nodecov/build.py`): a mode per group. A driver flow (Round Started, a Delay, Pick Player
  of a bot, Pick Entity of a spawn point, a 5000-health zombie for the creature pins, a Repeat timer as a timer) feeds
  every input by type; each tested action hangs off its own Sequence, each event prints all its outputs (`NCEV`), each
  value node's outputs are printed (`NCV`). Disruptive nodes (kill, kick, strip, team moves, map change...) run 50 s
  in on their own chain.
- **Read-backs** (`ncr`): each setter between a getter before and after (health 100 -> 77, gravity 1 -> 0.50, model
  gsg9 -> gign, 17 World settings...), with half a second for what the game reports a frame later.
- **Logic** (`ncl`, `ncd`): the repo's `every_node` / `every_node_3` graphs (variables, arrays, maps, structs, enums,
  functions, macros, data tables, timelines, custom events) with prints added for every value node.
- **Triggers**: a test-only helper plugin (`nchelp.sma`, in the sim only) makes a bot press the map's first button,
  say `/shop` and `/shop 5`, and sets a 4-minute time limit; the round clock gives Round Time Up; cs_office gives
  hostages; Player Killed -> Down Player gives the down events; br_valley's markers (mode `br`) give the marker nodes.
- **Where**: one sim at a time (XNC, a clone of LSPLAY with the ls-next3 brain from CI 43631450756, 8677af9, plugins
  fresh from HEAD, `-insecure`, load under 20, later under 15 and taking turns with Y), maps de_dust2, cs_office,
  de_nuke, de_rats, br_valley. Debug (Playtest) builds; the debugger's event stream collected and analyzed per node
  (`collect.py`, `analyze.py`, `mkdoc.py`). Runs: nc1, nca, ncv, ncr, ncl, ncb, ncc, ncd, ncf (cs_office, de_nuke,
  de_rats), nct, ncw, ncx, nck, ncq, br (markers).
- **Evidence** column: a read-back, the printed value, or the node's own result as the debugger showed it. 94 rows
  show only the node's plain result (`done`, `set`, `told`...): it ran without a refusal; the effect itself wasn't
  read back (most are one-line wrappers of a game call, like the effects and bot orders).

## Faults found, and the fixes

| # | What | Found by | Fix |
|---|---|---|---|
| 1 | A mode that reads the round clock without a Round Time Up event didn't compile (`new ctx` then `#pragma unused ctx`) | ncv build | 7783e9e7, real-amxxpc test |
| 2 | **Long flows crashed**: each node calls the next, so a flow's frames are all on the stack; AMX Mod X's default 4096 cells ran out after about ten Prints ("stack error"; without debug, garbage in `replace` and `set_task` and the flow stopped) | ncv: 28 of 230 nodes ran | 08b63471: `#pragma dynamic` sized by node count; ncv then ran 223/230 |
| 3 | **The Playtest stream lost events**: the debugger's 24000-character buffer went out in one `fputs`, which takes 16383 at most (44 events lost and a line cut in half in one busy half second) | ncv events had a gap | 08b63471: 16000; test |
| 4 | Print, chat, HUD and notices rounded numbers (gravity 0.5 printed `1`); short vectors printed in whole units (Normalize: `-0 -0 0`) | ncr, ncv | 08b63471: two decimals when needed; tests |
| 5 | **Chat Command never ran with arguments** (`/give 5`): AMX Mod X matches a `say /give` registration only against the bare word | ncf/office: `/shop` 6, `/shop 5` 0 | ab946fa2: on all of say, the first word checked; sim after: 15 runs with Arguments `5` |
| 6 | Run Server/AMX Command, Spawn Prop, Call Plugin Function said nothing to the debugger when they worked; the Switches never said which case | nca/nc1 events | ab946fa2: `sent` / `spawned` / `called` / the case; a test that every flow node of the every-node graphs reports a result |
| 7 | (Y) Buy Perk's Bought was always no (not kept) | Y | be9b35b9; test |
| 8 | (Y) A component pin wired with another kind (a barricade into Door) raised gm_components' native error and ended the flow silently | Y | be9b35b9: `lsg_kind_ok` before the call, "Not done: that isn't a Timed door..."; sim (ncq): the error shows and the flow goes on |
| 9 | (Y) Wave Spawned One's entity is the spawner, read as the enemy | Y | be9b35b9: labelled Spawner, help says so |

Handed on:
- **Player Revived never fires for a Studio down** (brain owner): see the row. Bots are sent an `ls_mode` use goal on
  the downed player, but the brain drops a use goal on a player who isn't alive-and-down (gm_components' kind); a
  Studio down is a dead player. Either the brain accepts a dead player for that goal, or the Studio needs a kind the
  brain keeps; a `hold` on the spot was tried and didn't revive either (5 downs), so the code is as it was.
- (Y, W1) Wave Spawned One could give the enemy if gm_components had a forward after the spawn.

Not changed, by design: Round Time Up leaves the game's own time limit off (`mp_round_infinite f`) for the rest of the
round once it fires, as its help says the mode decides what happens then.

## X's nodes

### Advanced

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Run Server Command | `game.run_command` | PASS | 28 (nca, ncd, ncw, ncx) | `sv_gravity 790`: World Gravity read `790` half a second later (ncx; the command runs at the end of the frame, so in the same frame it still read 800, ncw); result `sent` |
| Run AMX Command | `game.amx_command` | PASS | 26 (nca, ncd, ncw) | `amx_say NCAMX`: Ran `yes`, result `sent` (ncw) |
| Call Plugin Function | `amx.call` | PASS | 27 (nca, ncd, ncw) | nchelp.amxx `nc_add(2, 3)`: Result `5`, `called` (ncw) |
| Plugin Is Running | `amx.plugin_running` | PASS | 1 (ncv) | result: `true` |
| Fire Custom Event | `amx.fire` | PASS | 2 (nca) | result: `done` |
| On Custom Event | `amx.on_event` | PASS | 2 (nca) | result: `ran` |

### Arrays

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| For Each | `flow.for_each_array` | PASS | 5 (nc1, ncl) | result: `1 times` |
| Make Array | `array.make` | PASS | 4 (nc1, ncl) | result: `2 items` |
| Players Array | `array.players` | PASS | 3 (nc1) | result: `1 items`; `2 items` |
| Add | `array.add` | PASS | 71 (nc1, ncl) | result: `1 items`; `2 items` |
| Insert | `array.insert` | PASS | 5 (nc1, ncl) | result: `6 items` |
| Remove Item | `array.remove` | PASS | 5 (nc1, ncl) | result: `5 items` |
| Remove Index | `array.remove_index` | PASS | 5 (nc1, ncl) | result: `5 items` |
| Set Array Item | `array.set` | PASS | 5 (nc1, ncl) | result: `5 items` |
| Clear | `array.clear` | PASS | 5 (nc1, ncl) | result: `0 items` |
| Shuffle | `array.shuffle` | PASS | 5 (nc1, ncl) | result: `5 items` |
| Sort | `array.sort` | PASS | 5 (nc1, ncl) | result: `5 items` |
| Get | `array.get` | PASS | 6 (nc1, ncl) | result: `a` |
| Length | `array.length` | PASS | 8 (nc1, ncl) | result: `1.0`; `2.0` |
| Contains | `array.contains` | PASS | 1 (ncl) | result: `false` |
| Find | `array.find` | PASS | 1 (ncl) | result: `-1.0` |
| Random Item | `array.random` | PASS | 2 (ncl) | result: `0.0`; `4 items` |

### Bots

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Bots: Phase | `bots.phase` | PASS | 3 (ncb) | result: `told` |
| Bots: Don't Buy | `bots.no_buy` | PASS | 3 (ncb) | result: `told` |
| Bots: Friends | `bots.friends` | PASS | 3 (ncb) | result: `told` |
| Bots: Tag | `bots.tag` | PASS | 3 (ncb) | result: `told` |
| Bots: Prefer Layer | `bots.layer` | PASS | 3 (ncb) | result: `told` |
| Bots: Fight Creatures | `bots.creature_enemies` | PASS | 3 (ncb) | result: `told` |
| Bots: Goal | `bots.goal` | PASS | 3 (ncb) | result: `told` |
| Bots: Clear Goals | `bots.goal_clear` | PASS | 3 (ncb) | result: `told` |
| Bots: Offer | `bots.offer` | PASS | 3 (ncb) | result: `told` |
| Bots: Clear Offer | `bots.offer_clear` | PASS | 3 (ncb) | result: `told` |
| Bot Go To | `bot.goto` | PASS | 51 (nc1, ncb) | result: `ordered` |
| Bot Hold | `bot.hold` | PASS | 3 (ncb) | result: `ordered` |
| Bot Follow | `bot.follow` | PASS | 3 (ncb) | result: `ordered` |
| Bot Nature | `bot.skill` | PASS | 15 (ncb) | result: `100.0`; `75.0` |
| Bot Trait | `bot.trait` | PASS | 3 (ncb) | result: `false`; `true` |
| Bot Free | `bot.release` | PASS | 3 (ncb) | result: `ordered` |

### Buying

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Buying | `buy.where` | PASS | 50 (nc1, nca) | result: `set` |
| Allow Buying | `buy.allow` | PASS | 50 (nc1, nca) | result: `set` |

### Conditions

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Is Team | `cond.is_team` | PASS | 3 (nca) | result: `false`; `true` |
| Branch | `flow.branch` | PASS | 377 (nc1, nca, ncl) | result: `false`; `true` |
| Compare | `math.compare` | PASS | 13 (nc1, nca, ncv, ncl) | result: `true`; `false` |
| And / Or | `logic.and_or` | PASS | 11 (nc1, ncf/office, ncf/office2, ncv, ncf/nuke, ncf/rats) | result: `true`; `false` |
| Not | `logic.not` | PASS | 2 (ncv, ncl) | result: `false`; `true` |
| Text Is | `text.equals` | PASS | 5 (nc1, ncv, ncl) | result: `false` |
| Is Valid? | `flow.if_valid` | PASS | 7 (ncf/office, ncf/office2, ncf/nuke, ncf/rats) | result: `valid` |
| Equal | `logic.equal` | PASS | 3 (nc1) | result: `true` |
| Select | `logic.select` | PASS | 3 (nc1) | result: `valid` |
| Is Valid | `cond.is_valid` | PASS | 1 (ncv) | result: `true` |

### Creatures

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Creature Killed | `creature.on_killed` | PASS | 3 (ncb) | result: `ran` |
| Creature Health | `creature.health` | PASS | 3 (ncb) | result: `5000.0` |
| Spawn Creature | `creature.spawn` | PASS | 19 (ncf/office, ncf/office2, ncb, ncf/nuke, ncc, ncf/rats) | result: `entity 427`; `entity 404` |
| Creature Hurt | `up.on_creature_hurt` | PASS | 267 (ncb) | result: `ran` |
| Creatures Alive | `creature.count` | PASS | 4 (ncl, ncb) | result: `0.0`; `1.0` |
| For Each Creature | `creature.for_each` | PASS | 27 (ncb, ncd) | result: `1 times`; `0 times` |
| Creatures of Type | `creature.of_type` | PASS | 3 (ncb) | result: `1 items` |
| Nearest Creature | `creature.nearest` | PASS | 4 (ncb, ncc) | result: `null`; `{"ent": 276, "class": "ls_creature", "name": ""}` |
| Creature Type | `creature.type` | PASS | 3 (ncb) | result: `zombie` |
| Idle For | `creature.idle_for` | PASS | 4 (ncb, ncc) | result: `0.0`; `0.21` |
| Can See | `creature.can_see` | PASS | 3 (ncb) | result: `true`; `false` |
| Creature Go To | `creature.go_to` | PASS | 27 (ncb, ncd) | result: `done` |
| Creature Attack | `creature.attack` | PASS | 27 (ncb, ncd) | result: `done` |
| Creature Hold | `creature.hold` | PASS | 27 (ncb, ncd) | result: `done` |
| Creature Resume AI | `creature.resume` | PASS | 27 (ncb, ncd) | result: `done` |
| Creature State | `creature.state` | PASS | 11 (ncb, ncc, ncd) | result: `0.0`; `5.0` |
| Creature Target | `creature.target` | PASS | 4 (ncb, ncc) | result: `null`; `{"id": 5, "name": "pesto_sux^", "team": "T", "hp": 100, "alive": true,` |
| Set Creature Speed | `creature.set_speed` | PASS | 27 (ncb, ncd) | result: `done` |
| Set Creature Tier | `creature.set_tier` | PASS | 27 (ncb, ncd) | result: `done` |
| Creature Flee To | `creature.flee_to` | PASS | 27 (ncb, ncd) | result: `done` |
| Set Creature Health | `creature.set_health` | PASS | 27 (ncb, ncd) | result: `done` |
| Remove Creature | `creature.remove` | PASS | 27 (ncb, ncd) | result: `done` |
| Creature Idle | `creature.on_idle` | PASS | 2 (ncc) | result: `ran` |
| Is Creature | `creature.is` | PASS | 6 (nc1, ncb) | result: `false`; `true` |

### Damage

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Block Damage | `dmg.block` | PASS | 11 (nca) | result: `blocked` |
| Scale Damage | `dmg.scale` | PASS | 123 (nc1, nca) | result: `105.0`; `142.8` |
| Set Damage | `dmg.set` | PASS | 123 (nc1, nca) | result: `5.0`; `1.5` |

### Data tables

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Get Row | `table.get_row` | PASS | 2 (ncd) | result: `true`; `1.0` |
| Get Cell | `table.get_cell` | PASS | 1 (ncd) | result: `glock18` |
| For Each Row | `table.for_each` | PASS | 24 (ncd) | result: `2 times` |
| Find Rows | `table.find` | PASS | 1 (ncd) | result: `1 items` |
| Row Count | `table.count` | PASS | 1 (ncd) | result: `2.0` |
| Row Keys | `table.keys` | PASS | 1 (ncd) | result: `2 items` |

### Effects

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Explosion | `fx.explosion` | PASS | 26 (nca, ncd) | result: `done` |
| Beam | `fx.beam` | PASS | 26 (nca, ncd) | result: `done` |
| Play Sound At | `fx.sound_at` | PASS | 26 (nca, ncd) | result: `done` |
| Play Sound To Player | `fx.sound_to` | PASS | 26 (nca, ncd) | result: `done` |
| Set Light Style | `fx.lights` | PASS | 26 (nca, ncd) | result: `done` |

### Entities

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Entity Class Of | `ent.class` | PASS | 2 (ncv, ncw) | result: `info_player_deathmatch`; `ls_studio_prop` |
| Entity Name Of | `ent.name` | PASS | 1 (ncv) | `` for a spawn point (they have no name) (ncv) |
| Find Entities | `ent.find_all` | PASS | 1 (ncv) | result: `3 items` |
| Spawn Prop | `ent.spawn_prop` | PASS | 27 (nca, ncd, ncw) | w_c4 at a spawn point: `spawned`, its class `ls_studio_prop` (ncw); with no entity wired it refuses as its help says (ncd) |
| Remove Entity | `ent.remove` | PASS | 25 (nca, ncd) | result: `done` |
| Set Entity Position | `ent.set_position` | PASS | 26 (nca, ncd) | result: `done` |
| Set Entity Rotation | `ent.set_angles` | PASS | 26 (nca, ncd) | result: `done` |
| Set Entity Visibility | `ent.set_visible` | PASS | 26 (nca, ncd) | result: `done` |
| Get Entity Transform | `ent.transform` | PASS | 7 (ncv) | result: `-928 -720 176`; `0 140 0` |
| Get Entity Position | `ent.position` | PASS | 1 (ncv) | result: `-928 -720 176` |
| Get Entity Rotation | `ent.rotation` | PASS | 1 (ncv) | result: `0 140 0` |
| Distance Between | `ent.distance` | PASS | 1 (ncv) | result: `0.0` |
| Direction To | `ent.direction` | PASS | 1 (ncv) | result: `0.00 0.00 0.00` |

### Enums

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Switch on Enum | `flow.switch_enum` | PASS | 53 (nc1, ncl) | 53 runs on its case paths (nc1, ncl); it now also tells the debugger the case (ab946fa2) |
| Enum Value | `enum.value` | PASS | 0 | a constant, built in: read through its users, Break Struct `Tank` and the enum Switch's cases (nc1, ncl) |
| Enum To Number | `enum.to_number` | PASS | 5 (nc1, ncl) | result: `1.0` |

### Events

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Round Started | `event.round_started` | PASS | 58 (nc1, nca, ncf/office, ncf/office2, ncv, ncr, ncl, ncb, ncf/nuke, ncc, ncd, nct, ncf/rats, ncw, markers/br_valley, ncx) | result: `ran` |
| Round Ended | `event.round_ended` | PASS | 9 (nc1, nca) | result: `ran` |
| Player Killed | `event.player_killed` | PASS | 85 (nca, ncf/office2, ncf/nuke, ncf/rats, nck) | result: `ran` |
| Player Spawned | `event.player_spawned` | PASS | 218 (nc1, nca, ncd) | result: `ran` |
| Player Joined | `event.player_joined` | PASS | 23 (nc1, nca) | result: `ran` |
| Player Left | `event.player_left` | PASS | 3 (nca) | result: `ran` |
| Bomb Planted | `event.bomb_planted` | PASS | 6 (nc1, nca) | result: `ran` |
| Bomb Defused | `event.bomb_defused` | NOT RUN | 0 | no defuse happened in the de_ runs (de_dust2, de_nuke, de_rats: bombs exploded 5 times); nothing forces one. Testable by chance in a longer run |
| Bomb Exploded | `event.bomb_exploded` | PASS | 5 (nc1, nca) | result: `ran` |
| Player Used | `event.player_used` | PASS | 69 (ncf/office, ncf/office2, ncf/nuke, ncf/rats) | result: `ran` |
| Player Touched | `event.player_touched` | NOT RUNTIME-TESTABLE | 0 | no stock map in the sim set has a trigger_multiple (the helper found none on cs_office, de_nuke, de_rats), and making one at run time hung the server once, so it wasn't forced again while the bottle is fragile; needs an editor map with a named trigger |
| Player Hurt | `event.player_hurt` | PASS | 122 (nc1, nca) | result: `ran` |
| Team Changed | `event.team_changed` | PASS | 31 (nc1, nca) | result: `ran` |
| Map Started | `event.map_started` | PASS | 3 (nc1, nca, ncl) | result: `ran` |
| New Round | `event.new_round` | PASS | 8 (nc1, nca) | result: `ran` |
| Chat Command | `event.chat_command` | PASS (fixed) | 39 (ncf/office2, ncf/rats) | `/shop` ran; `/shop 5` never did until ab946fa2 (AMX Mod X matches a `say /shop` registration only against the bare word); after: 15 runs with Arguments `5` and 18 with none (ncf/rats) |
| Player Said | `event.player_said` | PASS | 64 (ncf/office2, ncf/nuke, ncf/rats) | result: `ran` |
| Freeze Time Ended | `event.freeze_ended` | PASS | 4 (nca, ncd) | result: `ran` |
| Map Ending | `event.map_ending` | PASS | 1 (ncf/nuke) | result: `ran` |
| Weapon Fired | `event.weapon_fired` | PASS | 1560 (nca, ncd) | result: `ran` |
| Reloaded | `event.reloaded` | PASS | 101 (nca, ncd) | result: `ran` |
| Hostage Rescued | `event.hostage_rescued` | PASS | 4 (ncf/office) | result: `ran` |
| VIP Escaped | `event.vip_escaped` | NOT RUNTIME-TESTABLE | 0 | needs an as_ (VIP) map, and CZ bots don't play VIP; none in the sim's map set |
| Player About To Be Hurt | `event.player_about_hurt` | PASS | 265 (nc1, nca) | result: `called` |
| Player Revived | `event.player_revived` | FAIL (handed on) | 0 | never fired: 36 Studio downs (Down Player from Player Killed, de_nuke/de_rats/de_dust2, 12 and 30 s) and no revive; every one timed out (Down Timed Out PASS). Down Player's help says bots come to help: the Studio sends the two nearest bots an ls_mode `use` goal on the downed player, but the brain drops a use goal on a player who isn't alive and down the gm_components way (cs_bot_ls_modes.cpp at 8677af9, the `!IsAlive() / !LsModeIsDown` check), and a Studio down is a dead player. A `hold` goal on the spot was tried: no revive either (5 downs), so it was put back. For the brain's owner; a person holding USE is not affected |
| Downed Player Out | `event.down_timeout` | PASS | 45 (ncf/office2, ncf/nuke, ncf/rats, nck) | result: `called` |
| Round Time Up | `event.time_up` | PASS | 5 (ncf/office, ncf/office2, ncf/nuke, ncf/rats) | result: `called` |
| Custom Event | `event.custom` | PASS | 48 (ncd) | result: `called` |

### Flow

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| For Each Player | `flow.for_each_player` | PASS | 18 (nc1, nca, ncl) | result: `0 times`; `10 times` |
| For Each Entity | `flow.for_each_entity` | PASS | 6 (nc1, nca) | result: `0 times` |
| Pick Random Player | `flow.pick_player` | PASS | 30 (nca, ncf/office, ncf/office2, ncv, ncr, ncl, ncb, ncf/nuke, ncf/rats, ncw, ncx) | result: `picked 4 of 10`; `picked 10 of 10` |
| Pick Random Entity | `flow.pick_entity` | PASS | 34 (nc1, nca, ncf/office, ncf/office2, ncv, ncl, ncb, ncf/nuke, ncc, ncf/rats, ncw) | result: `picked 94 of 20`; `picked 102 of 20` |
| Sequence | `flow.sequence` | PASS | 82 (nc1, nca, ncf/office, ncf/office2, ncl, ncf/nuke, ncf/rats) | result: `ran` |
| Do Once | `flow.do_once` | PASS | 2 (nca) | result: `through`; `closed` |
| Do N | `flow.do_n` | PASS | 1 (nca) | result: `through` |
| Gate | `flow.gate` | PASS | 1 (nca) | result: `closed` |
| Flip Flop | `flow.flipflop` | PASS | 7 (ncf/office, ncf/office2, ncf/nuke, ncf/rats) | result: `A` |
| For Loop | `flow.for_loop` | PASS | 12 (nc1, ncf/office, ncf/office2, ncl, ncf/nuke, ncf/rats) | result: `5 times`; `10 times` |
| For Loop with Break | `flow.for_loop_break` | PASS | 48 (nc1) | result: `4 times` |
| While Loop | `flow.while` | PASS | 12 (nc1, ncf/office, ncf/office2, ncl, ncf/nuke, ncf/rats) | result: `3 times`; `1000 times` |
| Switch on Number | `flow.switch_num` | PASS | 2 (ncl, ncw) | 2 of cases 1,2,3: took the `2` path and said so (ncw) |
| Switch on Text | `flow.switch_text` | PASS | 6 (nc1, ncl, ncw) | `no` of cases yes,no: took the `no` path and said so (ncw) |
| Call Custom Event | `event.call` | PASS | 24 (ncd) | result: `called` |
| Reroute | `flow.reroute` | PASS | 0 | a number through it printed unchanged: `NCW reroute 3` (ncw), `NCV flow.reroute zre out 2` (ncl); it has no run of its own to show |

### Functions

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Function | `func.define` | PASS | 1 (ncl) | result: `called` |
| Call Function | `func.call` | PASS | 1 (ncl) | result: `called` |
| Function Entry | `fn.entry` | PASS | 59 (nc1, ncl, nct) | result: `called` |
| Return Node | `fn.return` | PASS | 44 (nc1, ncl) | result: `returned` |
| Call | `fn.call` | PASS | 34 (nc1, ncl) | result: `called` |
| Call Pure | `fn.call_pure` | PASS | 8 (nc1, ncl) | result: `8.0`; `8` |

### Game

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Set Setting | `game.set_cvar` | PASS | 13 (nc1, nca, ncf/office, ncf/office2, ncf/nuke, ncf/rats) | result: `set` |
| Use Entity | `entity.use` | NOT RUN | 2 (nca) | only the refusal ran ("No entity has that name on this map.", nca); the stock maps used have no door or button by the default name |
| Current Map Name | `game.map_name` | PASS | 1 (ncv) | result: `de_dust2` |
| Next Map | `game.next_map` | PASS | 1 (ncv) | result: `cs_assault` |
| Server Name | `game.server_name` | PASS | 1 (ncv) | result: `sim XNC` |
| Max Players | `game.max_players` | PASS | 1 (ncv) | result: `24.0` |
| Player Count | `game.player_count` | PASS | 1 (ncv) | result: `10.0` |
| Game Time | `game.time` | PASS | 1 (ncv) | result: `16.15` |
| Real Date and Time | `game.clock` | PASS | 2 (ncv) | result: `2026-11-14 13:01`; `1794690176.0` |
| Current Mode | `game.mode_id` | PASS | 1 (ncv) | result: `ncv` |
| Round Number | `game.round_number` | PASS | 1 (ncv) | result: `1.0` |
| Team Score | `game.team_score` | PASS | 1 (ncv) | result: `0.0` |
| Is Freeze Time | `game.freeze_time` | PASS | 1 (ncv) | result: `false` |
| Get Setting | `game.get_setting` | PASS | 2 (ncv) | result: `5.0`; `5` |
| Change Map | `game.change_map` | PASS | 2 (ncf/rats) | result: `done` |
| Restart Round | `game.restart_round` | PASS | 1 (nca) | result: `done` |

### Macros

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Macro Entry | `macro.entry` | PASS | 0 | a macro's body is put in where it's used: Use Macro ran 4 times, `In the macro: 3` (nc1) |
| Macro Exit | `macro.exit` | PASS | 0 | as Macro Entry: Use Macro's Out ran after the body (nc1) |
| Macro | `macro.use` | PASS | 5 (nc1, ncl) | result: `In the macro: 3` |

### Map markers

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Find Marker | `marker.find` | PASS | 2 (markers/br_valley) | result: `-2816 0 471`; `true` |
| Marker Count | `marker.count` | PASS | 1 (markers/br_valley) | result: `40.0` |
| Marker Key | `marker.get` | PASS | 1 (markers/br_valley) | result: `surface` |
| For Each Marker | `marker.for_each` | PASS | 1 (markers/br_valley) | result: `40 times` |

### Maps

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Map Add | `map.add` | PASS | 44 (nc1, ncl) | result: `1 keys`; `2 keys` |
| Map Remove | `map.remove` | PASS | 5 (nc1, ncl) | result: `1 keys` |
| Map Clear | `map.clear` | PASS | 5 (nc1, ncl) | result: `0 keys` |
| Map Find | `map.find` | NOT RUN | 0 | its only use in the test graphs is inside For Each Player of a flow the logic run left out (node cap); not run in this sweep |
| Map Contains | `map.contains` | PASS | 1 (ncl) | result: `true` |
| Map Length | `map.length` | PASS | 1 (ncl) | result: `10.0` |
| Map Keys | `map.keys` | PASS | 0 | Array Length of the keys of a 10-key map printed `10` (ncl) |
| Map Values | `map.values` | PASS | 0 | of an empty number-to-vector map: Array Length `0` (ncl); a filled one's values weren't printed |

### Messages

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Show Message | `msg.broadcast` | PASS | 7 (nc1, nca, ncl) | result: `shown` |
| Show HUD Text | `msg.hud` | PASS | 140 (nc1, nca, ncl) | result: `shown` |
| Chat Message | `msg.chat` | PASS | 2 (nca) | result: `said` |
| Progress Bar | `msg.bar` | PASS | 2 (nca) | result: `shown` |
| Menu | `msg.menu` | NOT RUNTIME-TESTABLE | 2 (nca) | menus show only to people: every run refused it as its help says ("Not shown: nobody there, or a bot."); needs a human player |
| Play Sound | `game.play_sound` | PASS | 3 (nca, ncl) | result: `played` |
| Chat To Team | `msg.team_chat` | PASS | 26 (nca, ncd) | result: `done` |
| Notify Admins | `msg.notify_admins` | PASS | 26 (nca, ncd) | result: `done` |
| Print String | `debug.print` | PASS | 7078 (nc1, nca, ncf/office, ncf/office2, ncv, ncr, ncl, ncb, ncf/nuke, ncc, ncd, nct, ncf/rats, ncw, markers/br_valley, ncx, nck) | result: `Numbers: 5 items, has 7 yes at 0, one is 7 (100%)`; `Numbers: 5 items, has 7 yes at 0, one is 1 (100%)` |

### Numbers

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Math | `math.op` | PASS | 22 (nc1, ncv, ncl) | result: `2.0`; `3.0` |
| Random Number | `math.random` | PASS | 2 (ncv, ncl) | result: `6.0`; `1.0` |
| Math Function | `math.fn` | PASS | 1 (ncv) | result: `0.0` |
| Clamp | `math.clamp` | PASS | 6 (nc1, ncv, ncl) | result: `40.0`; `0.0` |
| Lerp | `math.lerp` | PASS | 1 (ncv) | result: `50.0` |
| In Range | `math.in_range` | PASS | 4 (nc1, ncv) | result: `false`; `true` |
| Number | `math.number` | PASS | 64 (nc1, nca, ncv, ncr, ncl, ncw) | result: `99.0`; `7.0` |

### Players

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Health Of | `player.get_health` | PASS | 3 (ncv, ncr) | result: `100.0`; `77.0` |
| Team Of | `player.get_team` | PASS | 9 (nc1, ncv, ncr) | result: `T`; `CT` |
| Is Alive | `player.is_alive` | PASS | 13 (nc1, ncv, ncr, ncl) | result: `false`; `true` |
| Is Bot | `player.is_bot` | PASS | 4 (nc1, ncv) | result: `true` |
| Name Of | `player.get_name` | PASS | 1 (ncv) | result: `Kalle` |
| Count Alive | `game.count_alive` | PASS | 8 (nc1, ncv, ncl) | result: `10.0`; `7.0` |
| Give Weapon | `player.give_weapon` | PASS | 3 (nca, ncr) | read back player.has_weapon: no -> yes. result: `given`. refusal seen, as its help says: "Not given: that player is dead or already has one." |
| Strip Weapons | `player.strip` | PASS | 49 (nc1, nca) | result: `stripped` |
| Set Health | `player.set_health` | PASS | 2 (nca, ncr) | read back player.get_health: 100 -> 77. result: `set` |
| Set Armor | `player.set_armor` | PASS | 51 (nc1, nca, ncr) | read back player.armor: 0 -> 55. result: `set` |
| Set Team | `player.set_team` | PASS | 50 (nc1, nca, ncr) | read back player.get_team: Counter-Terrorists -> Counter-Terrorists. result: `moved` |
| Set Speed | `player.set_speed` | PASS | 51 (nc1, nca, ncr) | read back player.speed: 250 -> 180. result: `set` |
| Teleport | `player.teleport` | PASS | 50 (nc1, nca) | result: `teleported`. refusal seen, as its help says: "Not moved: no such entity on this map." |
| Give Money | `player.add_money` | PASS | 3 (nca, ncr) | read back player.get_money: 50 -> 550. result: `done` |
| Money Of | `player.get_money` | PASS | 3 (ncv, ncr) | result: `30.0`; `50.0` |
| Add Score | `player.add_frags` | PASS | 4 (nca, ncr, ncl) | read back player.get_frags: 7 -> 9. result: `done` |
| Score Of | `player.get_frags` | PASS | 5 (ncv, ncr) | result: `0.0`; `7.0` |
| Deaths Of | `player.get_deaths` | PASS | 3 (ncv, ncr) | result: `0.0`; `4.0` |
| Give Item | `player.give_item` | PASS | 3 (nca, ncr) | read back player.has_helmet: yes -> yes. result: `done` |
| Drop Weapon | `player.drop_weapon` | PASS | 2 (nca) | result: `done` |
| Set Gravity | `player.set_gravity` | PASS | 3 (nca, ncr) | read back player.gravity: 1 -> 0.50. result: `done` |
| Set Glow | `player.set_glow` | PASS | 5 (nc1, nca) | result: `done` |
| Set Model | `player.set_model` | PASS | 21 (nc1, nca, ncr) | read back player.model: gsg9 -> gign. result: `done` |
| Respawn | `player.respawn` | PASS | 2 (nca, ncr) | read back player.is_alive: no -> yes. result: `respawned`. refusal seen, as its help says: "Nothing done: only a dead player on a team comes back." |
| Kill | `player.kill` | PASS | 2 (nca, ncr) | read back player.is_alive: yes -> no. result: `done` |
| Freeze | `player.freeze` | PASS | 5 (nca, ncw, ncx) | read back: a bot's position `-32 -720 46` at freeze and the same 3 s later, `354 -157 36` 3 s after unfreezing (ncx); a dead player is refused as its help says |
| Set Role | `player.set_role` | PASS | 3 (nca, ncr, ncl) | read back player.is_role: no -> yes. result: `done` |
| Is Role | `player.is_role` | PASS | 3 (ncv, ncr) | result: `false`; `true` |
| Count Role | `game.count_role` | PASS | 2 (ncv, ncl) | result: `0.0` |
| Armor Of | `player.armor` | PASS | 3 (ncv, ncr) | result: `0.0`; `55.0` |
| Has Helmet | `player.has_helmet` | PASS | 3 (ncv, ncr) | result: `false`; `true` |
| Weapon Held | `player.weapon_held` | PASS | 3 (ncv, ncr) | result: `deagle`; `ak47` |
| Ammo Of | `player.ammo` | PASS | 4 (ncv, ncr) | result: `7.0`; `21.0` |
| Has Weapon | `player.has_weapon` | PASS | 5 (ncv, ncr) | result: `false`; `true` |
| Is Admin | `player.is_admin` | PASS | 1 (ncv) | result: `false` |
| Is VIP | `player.is_vip` | PASS | 1 (ncv) | result: `false` |
| Ping | `player.ping` | PASS | 1 (ncv) | result: `0.0` |
| Steam ID | `player.steam_id` | PASS | 1 (ncv) | empty for a bot (they have none), as the node is written; a person's ID needs a human |
| Is Spectator | `player.is_spectator` | PASS | 1 (ncv) | result: `false` |
| Time Connected | `player.time_connected` | PASS | 1 (ncv) | result: `13.0` |
| Player By Name | `player.by_name` | PASS | 1 (ncv) | result: `null` |
| Player By Index | `player.by_index` | PASS | 1 (ncv) | result: `{"id": 1, "name": "wh1z", "team": "CT", "hp": 100, "alive": true, "wea` |
| Gravity Of | `player.gravity` | PASS | 3 (ncv, ncr) | result: `1.0`; `0.5` |
| Speed Of | `player.speed` | PASS | 4 (ncv, ncr, ncw) | result: `250.0`; `180.0` |
| Model Of | `player.model` | PASS | 3 (ncv, ncr) | result: `sas`; `gsg9` |
| Get Transform | `player.transform` | PASS | 5 (ncv) | result: `-374 1695 -91`; `-0 -166 0` |
| Set Transform | `player.set_transform` | PASS | 26 (nca, ncd) | result: `done` |
| Set Rotation | `player.set_rotation` | PASS | 26 (nca, ncd) | result: `done` |
| Look At | `player.look_at` | PASS | 26 (nca, ncd) | result: `done` |
| Slap | `player.slap` | PASS | 25 (nca, ncd) | result: `done` |
| Screen Fade | `player.fade` | PASS | 26 (nca, ncd) | result: `done` |
| Screen Shake | `player.shake` | PASS | 26 (nca, ncd) | result: `done` |
| God Mode | `player.godmode` | PASS | 26 (nca, ncd) | result: `done` |
| No Clip | `player.noclip` | PASS | 26 (nca, ncd) | result: `done` |
| Set Visibility | `player.render` | PASS | 26 (nca, ncd) | result: `done` |
| Set Ammo | `player.set_ammo` | PASS | 27 (nca, ncr, ncd) | read back player.ammo: 30 -> 12. result: `done` |
| Give Ammo | `player.give_ammo` | PASS | 26 (nca, ncd) | result: `done` |
| Remove Weapon | `player.take_weapon` | PASS | 26 (nca, ncr, ncd) | read back player.has_weapon: yes -> no. result: `done` |
| Switch To Weapon | `player.switch_weapon` | PASS | 27 (nca, ncr, ncd) | read back player.weapon_held: ak47 -> ak47. result: `done` |
| Set Kills | `player.set_frags` | PASS | 27 (nca, ncr, ncd) | read back player.get_frags: 0 -> 7. result: `done` |
| Set Deaths | `player.set_deaths` | PASS | 27 (nca, ncr, ncd) | read back player.get_deaths: 0 -> 4. result: `done` |
| Move To Spectators | `player.to_spectators` | PASS | 1 (nca) | result: `done` |
| Kick | `player.kick` | PASS | 1 (nca) | result: `done` |
| Mute Voice | `player.mute` | PASS | 25 (nca, ncd) | result: `done` |
| Gag Chat | `player.gag` | PASS | 25 (nca, ncd) | result: `done` |
| No Fall Damage | `player.no_fall` | PASS | 50 (nc1, nca) | result: `set` |
| Make Downed | `player.down` | PASS | 65 (nca, ncf/office2, ncf/nuke, ncf/rats, nck) | result: `downed` |
| Is Downed | `player.is_downed` | PASS | 1 (ncv) | result: `false` |
| Launch Up | `player.launch` | PASS | 50 (nc1, nca) | result: `done` |
| Is Player | `player.is_player` | PASS | 4 (nc1, ncv) | result: `true`; `false` |
| Set Position | `player.set_position` | PASS | 41 (nc1, ncf/office, ncf/office2, ncl, ncf/nuke, ncf/rats) | result: `done` |
| Set Velocity | `player.set_velocity` | PASS | 41 (nc1, ncf/office, ncf/office2, ncl, ncf/nuke, ncf/rats) | result: `done` |

### Round clock

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Set Round Clock | `clock.set` | PASS | 14 (nc1, nca, ncf/office, ncf/office2, ncl, ncf/nuke, ncf/rats) | result: `180 s left`; `179 s left` |
| Add Round Time | `clock.add` | PASS | 7 (nc1, nca, ncl) | result: `210 s left`; `209 s left` |
| Pause Round Clock | `clock.pause` | PASS | 12 (nc1, nca, ncl) | result: `210 s left`; `209 s left` |
| Round Time Left | `clock.left` | PASS | 6 (nc1, ncv, ncl) | result: `210.0`; `209.99` |

### Saved

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Saved Value | `save.get` | PASS | 2 (ncv, ncl) | result: `0.0`; `1.0` |
| Save Value | `save.set` | PASS | 3 (nca, ncl) | result: `0.0`; `1.0` |
| Add To Saved | `save.add` | PASS | 3 (nca, ncl) | result: `1.0` |

### Scoring

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| End Round | `score.end_round` | PASS | 4 (nc1, nca) | result: `ended` |

### Structs

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Make Struct | `struct.make` | PASS | 2 (nc1, ncl) | read through Break Struct / Format Text: `40 kit Tank 0 0 0` (ncl) |
| Break Struct | `struct.break` | PASS | 4 (nc1, ncl) | result: `Tank` |
| Set Field | `struct.set_field` | PASS | 10 (nc1, ncl) | result: `set` |

### Text

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Text | `text.literal` | PASS | 24 (nc1, ncv, ncr, ncl, ncw) | result: `kill`; `k` |
| Append | `text.append` | PASS | 4 (nc1, ncv, ncl) | result: `bc` |
| Format Text | `text.format` | PASS | 31 (nc1, ncv, ncl) | result: `5 items, has 7 yes at 0, one is 7 (100%)`; `5 items, has 7 yes at 0, one is 1 (100%)` |
| Contains Text | `text.contains` | PASS | 2 (ncv, ncl) | result: `false`; `true` |
| Text Length | `text.length` | PASS | 2 (ncv, ncl) | result: `2.0`; `7.0` |
| Text To Number | `text.to_number` | PASS | 2 (ncv, ncl) | result: `0.0`; `2.0` |
| To Text | `text.from` | PASS | 2 (nc1, ncl) | read through Format Text and Print, which ran with its text (nc1, ncl) |

### Timers

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Delay | `timer.delay` | PASS | 56 (nc1, nca, ncf/office, ncf/office2, ncv, ncr, ncl, ncb, ncf/nuke, ncc, ncd, ncf/rats, ncw, markers/br_valley, ncx) | result: `waiting`; `fired` |
| Repeat | `timer.repeat` | PASS | 31 (nc1, nca, ncf/office, ncf/office2, ncv, ncl, ncb, ncf/nuke, ncf/rats) | result: `started`; `each` |
| Countdown | `timer.countdown` | PASS | 7 (nc1, nca, ncl) | result: `started`; `done` |
| Stop Timers | `timer.stop_all` | PASS | 7 (nc1, nca) | result: `stopped` |
| Stop Timer | `timer.stop` | PASS | 4 (nc1) | result: `stopped` |
| Retriggerable Delay | `timer.retrigger` | PASS | 7 (ncf/office, ncf/office2, ncf/nuke, ncf/rats) | result: `waiting`; `fired` |
| Set Timer by Function | `timer.by_function` | PASS | 6 (nc1, ncl, nct) | result: `started` |
| Set Timer by Event | `timer.by_event` | PASS | 24 (ncd) | result: `started` |
| Timeline | `timeline.play` | PASS | 25 (ncd) | result: `playing`; `0.10 of 4.00 s, forward; Lift: 1.01; Position: 6 0 1; Glow: 241 0 13` |
| Clear Timer | `timer.clear` | PASS | 12 (nc1, ncf/office, ncf/office2, ncl, ncf/nuke, ncf/rats) | result: `cleared` |
| Is Timer Active | `timer.is_active` | PASS | 6 (nc1, ncv, ncl) | result: `true`; `false` |

### Variables

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Get Variable | `var.get` | PASS | 87 (nc1, ncl) | result: `0 items`; `1 items` |
| Set Variable | `var.set` | PASS | 233 (nc1, ncl) | result: `2.00`; `3.00` |
| Add To Variable | `var.add` | PASS | 3 (nc1) | result: `1.00` |

### Vectors

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Make Vector | `vec.make` | PASS | 27 (nc1, ncv, ncl, ncd) | result: `0 0 36`; `0.00 0.00 0.00` |
| Break Vector | `vec.break` | PASS | 3 (ncv) | result: `-374.05`; `1695.5` |
| Vector Math | `vec.op` | PASS | 6 (nc1, ncv, ncl) | result: `875 2212 34`; `-1975 1488 104` |
| Scale Vector | `vec.scale` | PASS | 6 (nc1, ncv, ncl) | result: `0 0 0`; `-748 3391 -183` |
| Vector Length | `vec.length` | PASS | 1 (ncv) | result: `1738.71` |
| Distance | `vec.distance` | PASS | 1 (ncv) | result: `0.0` |
| Direction | `vec.direction` | PASS | 1 (ncv) | result: `0.00 0.00 0.00` |
| Normalize | `vec.normalize` | PASS | 1 (ncv) | result: `-0.21 0.97 -0.05` |
| Position Of Player | `vec.player` | PASS | 51 (nc1, nca, ncf/office, ncf/office2, ncv, ncl, ncb, ncf/nuke, ncf/rats, ncx) | result: `1375 2196 0`; `-1920 1488 85` |
| Position Of Entity | `vec.entity` | PASS | 1 (ncv) | result: `-928 -720 176` |

### World

| Node | Type | Status | Runs | Evidence |
|---|---|---|---|---|
| Gravity | `world.gravity` | PASS | 5 (ncv, ncr, ncw, ncx) | result: `800.0`; `750.0` |
| Set Gravity | `world.set_gravity` | PASS | 27 (nca, ncr, ncd) | read back world.gravity: 800 -> 750. result: `done` |
| Friction | `world.friction` | PASS | 3 (ncv, ncr) | result: `4.0`; `5.0` |
| Set Friction | `world.set_friction` | PASS | 27 (nca, ncr, ncd) | read back world.friction: 4 -> 5. result: `done` |
| Max Speed | `world.max_speed` | PASS | 3 (ncv, ncr) | result: `320.0`; `300.0` |
| Set Max Speed | `world.set_max_speed` | PASS | 27 (nca, ncr, ncd) | read back world.max_speed: 320 -> 300. result: `done` |
| Round Time | `world.round_time` | PASS | 3 (ncv, ncr) | result: `5.0`; `3.0` |
| Set Round Time | `world.set_round_time` | PASS | 27 (nca, ncr, ncd) | read back world.round_time: 5 -> 3. result: `done` |
| Freeze Time | `world.freeze_time` | PASS | 3 (ncv, ncr) | result: `6.0`; `3.0` |
| Set Freeze Time | `world.set_freeze_time` | PASS | 27 (nca, ncr, ncd) | read back world.freeze_time: 6 -> 3. result: `done` |
| Buy Time | `world.buy_time` | PASS | 3 (ncv, ncr) | result: `1.5`; `0.75` |
| Set Buy Time | `world.set_buy_time` | PASS | 27 (nca, ncr, ncd) | read back world.buy_time: 1.50 -> 0.75. result: `done` |
| Friendly Fire | `world.friendly_fire` | PASS | 3 (ncv, ncr) | result: `false`; `true` |
| Set Friendly Fire | `world.set_friendly_fire` | PASS | 27 (nca, ncr, ncd) | read back world.friendly_fire: no -> yes. result: `done` |
| Team Balance | `world.team_balance` | PASS | 3 (ncv, ncr) | result: `true`; `false` |
| Set Team Balance | `world.set_team_balance` | PASS | 27 (nca, ncr, ncd) | read back world.team_balance: yes -> no. result: `done` |
| Team Limit | `world.team_limit` | PASS | 3 (ncv, ncr) | result: `2.0`; `3.0` |
| Set Team Limit | `world.set_team_limit` | PASS | 27 (nca, ncr, ncd) | read back world.team_limit: 2 -> 3. result: `done` |
| Start Money | `world.start_money` | PASS | 3 (ncv, ncr) | result: `800.0`; `1600.0` |
| Set Start Money | `world.set_start_money` | PASS | 27 (nca, ncr, ncd) | read back world.start_money: 800 -> 1600. result: `done` |
| C4 Timer | `world.c4_timer` | PASS | 3 (ncv, ncr) | result: `45.0`; `40.0` |
| Set C4 Timer | `world.set_c4_timer` | PASS | 27 (nca, ncr, ncd) | read back world.c4_timer: 45 -> 40. result: `done` |
| Fall Damage | `world.fall_damage` | PASS | 3 (ncv, ncr) | result: `true`; `false` |
| Set Fall Damage | `world.set_fall_damage` | PASS | 27 (nca, ncr, ncd) | read back world.fall_damage: yes -> no. result: `done` |
| Footsteps | `world.footsteps` | PASS | 3 (ncv, ncr) | result: `true`; `false` |
| Set Footsteps | `world.set_footsteps` | PASS | 27 (nca, ncr, ncd) | read back world.footsteps: yes -> no. result: `done` |
| All Talk | `world.alltalk` | PASS | 3 (ncv, ncr) | result: `false`; `true` |
| Set All Talk | `world.set_alltalk` | PASS | 27 (nca, ncr, ncd) | read back world.alltalk: no -> yes. result: `done` |
| Time Limit | `world.time_limit` | PASS | 3 (ncv, ncr) | result: `20.0`; `25.0` |
| Set Time Limit | `world.set_time_limit` | PASS | 27 (nca, ncr, ncd) | read back world.time_limit: 20 -> 25. result: `done` |
| Win Limit | `world.win_limit` | PASS | 3 (ncv, ncr) | result: `0.0`; `9.0` |
| Set Win Limit | `world.set_win_limit` | PASS | 27 (nca, ncr, ncd) | read back world.win_limit: 0 -> 9. result: `done` |
| Frag Limit | `world.frag_limit` | PASS | 3 (ncv, ncr) | result: `0.0`; `30.0` |
| Set Frag Limit | `world.set_frag_limit` | PASS | 27 (nca, ncr, ncd) | read back world.frag_limit: 0 -> 30. result: `done` |

## The components group (Y), merged from NodeCoverage-components.md

For X to merge into NodeCoverage.md. 105 node types: Components (40), Upgrades and turrets (21), Battle Royale (15),
Downed (8), Waves (8), Shop (4), Squads (4), Map markers (4) and Spawn Creature. The "Maps" category (Add To Map,
Find In Map... the key/value containers, 8) is a language node group, left to X.

## How

X's harness, same way: `scratchpad/nodecov/ncy_build.py <mode>` builds a test mode by hand (each node gets the component
it works on: one of each of the 16 kinds made with Create Component at the CT spawn, the map's own East/West gate, the
map's zone), and after each action prints the value nodes that show what it did (`NCV` lines); every event prints its
outputs (`NCEV`). Compiled with the app (`studio_write_plugin`, debug build) plus X's two pending runtime fixes (stack
size, LSD_BUF 16000), amxxpc, run with `ncy_run.sh` on one sim of my own (YNC, a clone of XNC, `-insecure`, port 27352,
8 bots, `mp_roundtime 9`). Modes: ncy1 on ls_outpost (components, upgrades, downed, waves, shop, squads, turrets),
ncy2 on br_valley (BR, the map's zone), `br` on br_valley (map markers: the map's markers are for the mode `br`),
ncy3/ncy4 on ls_outpost (barricade damage, the checkpoint fix). Results from the server's console (`NCV`/`NCEV`) and the
debugger's events. Test helper `ncyhelp.amxx` (a bot picks item 1 of an open menu) didn't work: bots' menus don't take
`menuselect`.

Legend: **ran** (ran and did what it says, value checked), **refusal** (refused correctly, with the debugger's error),
**failed**, **not runtime-testable** (why).

## Components

| Node | Result | What showed it |
|---|---|---|
| Create Component | ran | all 16 kinds made (capture point, safe zone, shrinking zone, damage zone, loot container, weapon spawner, supply drop, wave spawner, extraction, checkpoint, trap, respawn point, shop, perk machine, upgrade bench, mystery box): "entity N" each |
| Find Component | ran | "East gate" -> 38 (ls_barricade), "West gate" -> 33, br_valley "brzone" -> the safe zone |
| Load Map Pack | refusal | "This map has no such pack." on both maps (no ls_outpost/br_valley pack files exist) |
| Start Component / Stop Component | ran | the shrinker started: Zone Stage Started stage 1, Zone Stage 1; stopped |
| Reset Components | ran | |
| Set Zone / Shrink Zone | ran | Zone Next radius 3000 -> 2000, closing yes, then zone ticks on players outside |
| Skip Zone Stage | ran | the next stage comes at once (br_valley: Zone Stage Started 2x) |
| Zone Stage / Next Zone / In Zone | ran | stage 0 -> 1; secs 10 / 22 / -1 (none planned), radius, closing; in zone yes/no |
| Open Crate | ran | Looted fired (container, player) |
| Roll Loot | ran | item usp / smoke / glock18 / hp:50, rolled yes |
| Open Extraction | ran | Extracted fired |
| Lock Capture Point | ran | Captured / Capture Lost fired around it |
| Fire Trap | ran | "done" (a 0-damage trap: nothing else to see) |
| Set Barricade HP / Repair Barricade / Barricade HP | ran | 800 -> 300 -> 433 (repaired +133), Barricade Repaired fired (hp, player) |
| Door | **failed** (wrong kind) / not runtime-testable (a real door) | no door on these maps and Create Component can't make one; with a barricade wired in, gm_components' native refuses with a native error and **the flow stops there** (no debugger error, the rest of the chain never runs). Studio fix (X): see below |
| Call Supply Drop | ran | "called"; Drop Landed fired |
| Respawn At | ran | "done" (an alive player: nothing to do) |
| Start Wave / Stop Waves | ran | Wave Started wave 1; Wave Spawn; Wave Cleared after the creatures were killed |
| Revive Pay | ran | |
| Events: Captured, Capture Lost, Zone Stage Started, Zone Tick, Extracted, Wave Started, Wave Spawn, Wave Cleared, Looted, Checkpoint Reached, Barricade Repaired, Drop Landed | ran | each fired with its outputs |
| Event: Barricade Damaged / Barricade Broken | not runtime-testable here | needs creatures pressed against a gate; three zombies sent through the West gate (60 hp) didn't touch it in 60 s, Creature Attack doesn't take a barricade, Explosion only hurts players |
| Event: Meeting Ended | not runtime-testable here | needs an ls_meeting_point; none on these maps and Create Component has no meeting point |
| Wave Spawn's Spawned | ran, **label** | the value is the wave spawner that made the enemy, not the enemy (gm_components' forward passes the spawner); the help says "One enemy of a wave came in". Rename to Spawner or pass the creature (X/W1) |

## Upgrades, turrets, downed, shop, squads, waves, creatures

| Node | Result | What showed it |
|---|---|---|
| Set Player Modifier / Player Modifier | ran | speed 1.2 (the debugger shows 1.20; Print shows 1: Print rounds numbers) |
| Give Perk / Has Perk / Perk Count | ran | Tough: has yes, count 1 |
| Buy Perk | **failed** (output) | the perk is bought (Perk Bought fired, $2500, count 1 -> 2), but **Bought is always no**: up.perk_buy isn't in gen.rs's STORE_KINDS, so its result is thrown away and the consumer reads an empty slot |
| Upgrade Weapon / Weapon Level | ran | level 1 -> 2, Weapon Upgraded fired (level 2, $3000) |
| Mystery Box | ran | Box Rolled fired (with Box unwired: the box that's out) |
| Drop Pickup | ran | entity; Pickup Taken fired |
| Turret Setup / Give Turret Kits / Place Turret / Turret Kits / Turret Count | ran | kits 2 -> 1, a turret entity, count 1, Turret Placed fired |
| Events: Perk Bought, Weapon Upgraded, Box Rolled, Pickup Taken, Turret Placed | ran | |
| Downed On / Down Player / Revive / Is Downed / Down Time Left | ran | is downed yes, 30 s left, revived: no; Downed / Got Up / Bled Out fired many times |
| Open Shop / Shop Price | ran | price 200 for "ammo" with items; -1 without items (correct: the shop doesn't sell it) |
| Shop Menu | ran | the menu opens (menu 57 on the bot) |
| Event: Shop Buy | not runtime-testable by bots | a person has to pick from the menu |
| Set Squad / Squad / Squads Alive / Squad Friendly Fire | ran | squad 2/3, alive 5-8 |
| Round Table On / Off, Wave Scale, Wave Cap, Tiers | ran | "Wave 1: 2 creatures against 4 defenders (creature health x0.36, ... 12/20 alive at most)" (scale 0.1/0.3, cap) |
| Wave Info / Creature Pay / Creatures Working | ran | planned 21 (2 scaled), spawned 0..21, type empty (only for boss/pack rounds), pay 75, working yes |
| Spawn Creature | ran | entity each time |

## Battle Royale, map markers

| Node | Result | What showed it |
|---|---|---|
| Lobby / Squad Size / On Lobby End | ran | Lobby Ended fired |
| Match HUD / HUD Text / Feed | ran | |
| Give Healing / Healing Count | ran | 0 -> 2 |
| Set Armor / Make Crate / Glow / Kills / Squads Alive | ran | crate entity, glowed; kills 0/1; squads 3-6 |
| Drop In / On Landed | ran | Landed fired |
| Find Marker / Marker Count / Marker Key / For Each Marker | ran (mode `br`) | 40 br_drop markers, the 2nd at 1280 -3072 28, key layer "surface" (num 0), the loop 40 times |
| the same in another mode | refusal (by design) | br_valley's markers say mode "br": count 0, found no |

## Fixed (mine)

- **Checkpoint Reached fired ~25 times a second** while someone stood in a one-checkpoint course (or a last checkpoint
  that is also the first): each tick completed a lap and reset the player to the start. gm_components now counts it
  again only once the player has left it. bbc7f5b4. Sim ncy4: a bot teleported into a one-checkpoint course and frozen:
  1 event (was thousands in ncy1).

## For X (Studio, gen.rs / runtime; not changed by me: X has these files open)

1. Long flows ran out of stack (every node calls the next): "Run time error 3" and garbage strings after ~20 Prints.
   X's pending `#pragma dynamic` fix cures it (used here).
2. The debugger's buffer (LSD_BUF 24000) is longer than AMX Mod X's string limit (16384): a flush cut the file mid-line
   and dropped ~30-50 events at a time. X's pending 16000 cures it (used here).
3. Buy Perk's Bought output is never stored: add "up.perk_buy" to STORE_KINDS (one line) and a test.
4. A component input wired with the wrong kind (a barricade into Door, a spawn point into Shop...) makes gm_components
   raise a native error, which ends the whole flow silently. Guard like lsg_zone_of: check `ls_comp_kind` first and
   `lsd_error` "That isn't a <kind>" (Door, Fire Trap, Lock Capture Point, Open Extraction, the barricade nodes, the shop
   nodes, Mystery Box; Start/Stop Component on a non-component too).
5. Wave Spawn's Spawned is the spawner (above).
6. Print shows numbers rounded (1.2 -> "1").
