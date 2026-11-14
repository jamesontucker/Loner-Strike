# Node coverage: the components group (Y)

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
