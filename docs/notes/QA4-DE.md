# QA round 4: areas D (Battle Royale) and E (Deathrun, the mode vote)

Fresh QA agent, 2026-11-14 07:45-10:45. Nothing in the product was changed.

**Builds.** Brain: ls-next3 `285b895` (CI run 43604814272, the newest green one; its head matches
`git ls-remote ... refs/heads/ls-next3`). The sim's mp.dll is byte-identical to that run's artifact and has the newest
revive code (`revive-held`, `revive-state` strings) and `bot_ls_deathrun`. Plugins: compiled fresh with amxxpc from a
`git archive` of tools HEAD `314c7f47` (gm_battleroyale, gm_components, gamemodes, gm_pubserver, DeathrunManager,
dr_bot_traps and the rest; only zp/superhero/war3/basebuilder failed to compile for missing third-party includes, none
of them used here). Configs, maps (br_valley) and overviews from the same snapshot.

**Sims.** Two of my own: QA4DE1 (port 27406, BR and the BR/vote edge cases) and QA4DE2 (port 27408, Deathrun and the
vote), both cloned from BRPLAY's files (BRPLAY itself never started), `-insecure`, 16 bots in BR, 8 in Deathrun.
Deathrun maps from the Deathrun agent's copies; **navs from the published nav pack** (dl.lonerstrike.com/navs, what a
player's app installs), plus a second set of runs with the Deathrun agent's own navs for comparison. Both sims are
deleted. A sim-only test plugin (`qa4de_helper`: drive a bot's commands, list squads/downed/armor, set armor levels, a
traced hit with a hit group, knock someone down) lived only in the sims; logs and scripts are in my scratchpad.

Counts: **PASS 28, FAIL 6 (F1-F6; F1 covers two map rows), NEEDS USER 5, INFO 3** (the rows below; temple, starwars and the revive rate are INFO for the user).

---

## FAILs (repro steps first)

### F1. The published nav pack has old Deathrun navs: shipped bots can't play deathrun_death or deathrun_crash_egypt (high)
**Owner:** the Deathrun agent (navs) with whoever publishes the nav pack (MCP).
- `https://dl.lonerstrike.com/navs/list.json` lists deathrun_death, temple, crash_egypt, starwars; their .nav/.lsv
  (last-modified 2026-11-12 07:48 GMT, sha256 matching the list) are the early learned navs in
  `~/loner-strike-sim/navpack/out/` (Nov 12), not the ones the Deathrun agent made and tested on Nov 13
  (`~/loner-strike-sim/navpack-dr/`):

  | map | pack nav: areas / floor | agent nav: areas / floor |
  |---|---|---|
  | deathrun_death | 36 / 0.8 M u² (stops at x -2625) | 481 / 2.9 M u² |
  | deathrun_crash_egypt | 353 / 2.0 M u² | 1109 / 12.2 M u² |
  | deathrun_temple | 173 / 3.0 M u² | 233 / 5.6 M u² |
  | deathrun_starwars | 164 / 1.9 M u² | 209 / 1.2 M u² |
- A fresh install has no nav, so the app's nav pack puts these in (navpack.rs `Step::Add`), and the brain then reads
  the course from them: on deathrun_death "course end at -2750 12 -142, 760 units from the start" (agent nav: -3175
  1062 -143, 1641 units); on crash_egypt "course end at -1725 -3587 2901, 962 units" (it's next to the lasers).
- Same build, same plugins, 8 bots, 10 min each (drruns.py over gm_simrec + gm_drrec):

  | map | pack nav (what ships) | agent nav (Nov 13, not published) |
  |---|---|---|
  | deathrun_death | 31 rounds, **2 of 218 runs reach the end (1%)**, 214 deaths to hazards, every round ends at 48% of the course | 24 rounds, **43 of 169 (25%)** |
  | deathrun_crash_egypt | 25 rounds of ~7 s, **0 of 175 (0%)**; 176 of the deaths are the env_laser at (-1700, -2900, 2900) | 17 rounds, **18 of 119 (15%)**, 3 presses, 3 trap kills |
- Repro: `curl -s https://dl.lonerstrike.com/navs/list.json`, download `deathrun_death.nav`, count its areas (36);
  put the pack's .nav/.lsv beside deathrun_death.bsp in a sim, run deathrun with bots: runners die at the pit every
  round. Swap in `navpack-dr/deathrun_death.{nav,lsv}`: about a quarter finish.
- Fix direction: publish the Nov 13 navs (and .lsv) in the pack (and bump `replaces` so players who already got the old
  ones are upgraded), then re-run QA on the pack files.

### F2. Deathrun rounds aren't ended by the round timer on temple and starwars (medium)
**Owner:** the Deathrun agent (DeathrunManager / dr_bot_traps / round end).
- deathrun.cfg sets `mp_roundtime 5` (checked over rcon: "5"; `mp_round_infinite` "0"), yet with runners stuck alive:
  deathrun_temple (pack nav) one round ran **651 s** of game time to the end of the run; deathrun_starwars (pack nav)
  one round ran **537 s**; temple with the agent nav: one round of **440 s**. Nothing ends them (no win, no draw), so a player waits 9-11 minutes for the next round.
- Repro: deathrun mode, deathrun_temple with the pack nav, 8 bots, 10 min; the gm_drrec file has `R,14.13,start` and
  no end before the run stops at ~665 s.

### F3. Deathrun: the bot activator never presses a trap on deathrun_death and starwars, nor on temple with the shipped nav (low)
**Owner:** the Deathrun agent. QA4 E asks that traps fire when the terrorist presses them.
- 0 presses on deathrun_death in 55 rounds (both navs; "19 traps (3 the terrorist can walk to)"), 0 on temple with the pack nav
  (with the Deathrun agent's nav it pressed 6 times in 7 rounds), 0 on starwars ("20 traps (0 the terrorist can walk to)").
  crash_egypt fired 3 (agent nav). So with the navs that ship the terrorist is a spectator on three of the four maps and runners only die to the course.

### F4. BR squads: when the last squad mate who's up leaves the server, the downed ones aren't taken out (low)
**Owner:** W3 (gm_battleroyale `ls_on_down` / squads).
- QA4 D: "out when the whole squad is down". Repro (br_squads, 16 bots, mid-game): knock 3 of a squad of 4 (MCRocket,
  Condor, Kale at 10:15:56, 10:16:02, 10:16:10), then `kick "Takeshi M."` (the one still up) at 10:16:40. The three
  stay down and bleed out one by one on their own clocks (out at 10:16:41, 10:16:49, 10:16:57, "by self (world)"),
  and the squad still counts as alive until then (`ls_squads_alive` counts downed players). Same with a duo (partner
  kicked: the downed one waited ~30 s more). With an end game of two squads this holds the win for up to 45 s.
- Suggestion: on disconnect (and on a squad mate's death) run the same "nobody up" check that `ls_on_down` does.

### F5. BR: no game summary line when nobody survives (low)
**Owner:** W3. `check_winner()` logs "Battle Royale on <map> won by ... N knocked down, M revived" only for a winner; when
the last players die together it shows the "Nobody survived" toast and ends the round with no log line, so that game's
knocks/revives/below-ground figures are lost (the BR pieces line still comes). Repro: three last players of two squads
killed in the same second (my test at 10:14:04) → no "won by"/summary line for that game.

### F6. BR bots: the high ground is rarely held at the fight start (low; measurement)
**Owner:** W2/W3. QA4 D asks "about 3 in 10 below ground, 1 in 4 high". The fight-start line ("N below ground, N high,
N on the surface") over 20 games: **high 0-3 of 16 (mean ~1, ~6%)**, below ground 0-4 at fight start; over a whole game
2-6 of 16 went below ground (12-38%, passes). W1 said the high share isn't logged, but the fight-start line has it.

### Not a product failure: sim stalls (environment)
Five of my servers froze (0 % CPU, sleeping, no rcon answer, the brain's ls_watchdog silent, so the whole process was
stopped, not a game loop): BR at 08:11, 09:12 and 09:26 late in a game, deathrun_temple and br_valley while loading at
08:18/08:23, starwars at 08:29. Another agent's QA4S stopped logging at the same moments (08:29, 09:11-09:13, 09:26),
the bottle's wineserver restarted at ~08:27, and crashes.log has a winedbg "Internal crash at FFF4FADC" at 08:23 (the
debugger-attach signature from Y.md). I count these as CrossOver/bottle trouble under load, not BR or Deathrun; the
runs were redone. Games cut short by them are left out of the numbers below.

---

## D. Battle Royale

Evidence: qconsole + AMXX logs per session, `knocks.py` (each knock-down's outcome), the helper's lines. Complete games:
**squads 6, duos 3, solo 2**, all br_valley v6, 16 bots.

| Row | Result | Evidence |
|---|---|---|
| Solo, duos, squads start and finish | PASS | modes.ini has br, br_duo, br_squads; solo: 2 games (404 s, 357 s of fighting), 0 knock-downs as it should be; every game logged "game starts, 16 players", the fight start and a "won by" line; squads: "lobby over, 4 squads", duos: "lobby over, 8 squads" |
| The 30 s squad lobby | PASS | "squad lobby, 30 s" then "lobby over" 30 s later every game; damage refused during it (code: PH_LOBBY → HAM_SUPERCEDE) |
| `/squad`, start/invite/leave, party codes | NEEDS USER | bots can't answer the menu: `say /squad` + `menuselect 1` sent to a bot in the lobby left it in no squad (the menu goes to a person) |
| Squad mates can't hurt each other | PASS | traced 30-damage chest hit from a squad mate: hp 100 → 100; from an enemy: 100 → 70 |
| Drop-in, landing without damage | PASS | 0 "died during the drop-in/loot-up" in 17+ games |
| Knocked down in squads, out when the whole squad is down | PASS (but F4) | squads: 55 knock-downs in 6 games, every one ended: 9 revived, 13 finished, 28 bled out, 1 out with the squad, 4 still down on the winning squad at the end; my test: 3 of 4 knocked by traced hits, each "knocked down" with hp 100 (ls_down_hp) |
| Bleed-out after 45 s, counted as out | PASS | all 36 bleed-outs in complete games at 45-46 s after the knock ("out at ... (#N), by self (world)", with a place) |
| Finishing a downed player, the kill to the shooter | PASS | 17 finishes in complete games, credited: e.g. "player 2 out at 18 s (#16), by 14 (weapon_knife, 32 u)" after "knocked down at 15 s by 14" |
| Bots reviving (rate, as information) | INFO | **squads 9 of 55 (16 %)**, duos 2 of 16 (13 %); 11 of 71 in complete games (15 %), 16 of 100 counting the games cut short; 20 % of the knock-downs not finished by enemies. W1's 15 of 53 (28 %) on the same build isn't reproduced. The reviver's $ in BR: `ls_down_revive_pay 0` set by gm_battleroyale (code) |
| Everyone knocked at once | PASS | the last three players of two squads hit in the same second: all out, the round ended ("Nobody survived" path) and the next game's lobby started 10 s later; no stuck state (see F5 for the missing summary) |
| The last squad mate leaving | FAIL → F4 | |
| Loot: death crates | PASS | per game "death crates 8-13 made, 3-9 searched, 5-22 items taken" (BR pieces line); the crate is filled from the victim's carried list (gm_components ~6342) |
| Loot: armor levels reduce damage | PASS | traced 30-damage chest hits: no vest 30, vest 1 25, vest 2 21, vest 3 17 (points 100/120/150); 10-damage head hits (x4): no helmet 40, helmet 1 28, 2 24, 3 18 (W3_HELM_F 0.7/0.6/0.45) |
| Loot: armor found by level | PASS | e.g. "vest 3/0/2 helmet 0/1/2 (levels 1/2/3)"; all three levels seen across games |
| Loot: heals used | PASS | 0-5 bandages, 0-1 medkits, 0-3 drinks a game |
| Heal bar, rarity glow colors | NEEDS USER | client only |
| Match HUD, next-zone dot, kill feed, watching the killer, squad markers | NEEDS USER | client only |
| Late joiners fill a short squad | NEEDS USER | humans only (bots never take that path) |
| Bots below ground | PASS | 2-6 of 16 went below ground per game (12-38 %) |
| Bots on the high ground | FAIL → F6 | |
| Per-round summaries | PASS (but F5) | each game: "won by X (k kills) after N s; P went below ground, Q% of the time; K knocked down, R revived", then "BR pieces last round: ..." at the next game's start |
| br_valley | PASS | loads with its nav and knowledge, zone and supply drops run ("Supply drop N called/landed/opened"), no crash report from my sims in crashes.log, no drop-in deaths |
| Vote during the lobby | PASS | `gm_votemode` at 10:01:43 inside the lobby: the lobby still ended on time, the game and the fight started, the map stage ("Map vote for Battle Royale: Duos: br_valley de_aztec de_inferno de_dust2") ran over the loot-up and the mode stayed br_duo on br_valley |
| Map change mid-round | PASS | `changelevel br_valley` with a player down at 10:17:17: next map, "squad lobby, 30 s", "lobby over, 4 squads", fight start; all 16 up, nobody left downed |

## E. Deathrun

8 bots, the shipped deathrun.cfg (`bot_ls_deathrun 1`), 8-10 min a map, pack navs unless marked. drruns.py output.

| Row | Result | Evidence |
|---|---|---|
| deathrun_death | FAIL → F1 | pack nav: 1 % reach the end (31 rounds); agent nav: 25 %; 0 presses either way (F3) |
| deathrun_crash_egypt | FAIL → F1 | pack nav: 0 % (lasers every round); agent nav: 15 %, traps fire (3 presses) |
| deathrun_temple (for the user's decision) | INFO | **how it plays:** pack nav: 2 rounds in 10 min, runners stuck at 5-10 % of the course for 650 s, 0 finishes, 0 presses. Agent nav (re-run on one sim after the 09:26 stall cut the first one): 7 rounds, the activator springs traps (6 presses, 23 trap kills), rounds of 13-15 s where runners die at 14-20 % of the course, then one round of 440 s with runners stuck at 33 % (F2); 0 of 51 runs reach the end. A believable activator, but runners never finish |
| deathrun_starwars | removed by the user | not supported for bots in 1.0 (user, 2026-11-14); was: how it plays:** pack nav: in the first live round 6 of 15 runners reached the end in ~10 s, the rest stood in the start room for the whole 537-s round (median progress 0 %), 0 presses; agent nav: 7 rounds of 62-65 s, 29 of 50 runs reach the end (58 %), the rounds end with the runners falling (T wins 5), 0 presses (no trap the activator can walk to). Plays like a race with no activator |
| No crash in 10 rounds | PASS | death 31 + 24 rounds, egypt 25 + 17; no crash report from QA4DE2 in crashes.log; temple/starwars don't reach 10 rounds in 10 min (F2) |
| dojo and jigsaw2: out of the rotation | PASS | maps/deathrun.txt has only death, temple, crash_egypt, starwars; with both bsps installed, a Keep-Deathrun map vote offered "deathrun_temple deathrun_crash_egypt deathrun_death deathrun_starwars" |
| dojo and jigsaw2: out of the map download | PASS | catalog/parts/mode-deathrun.json lists the four maps only (cf157dbc, a4714121); no other reference in app/plugins/catalog |
| dojo and jigsaw2: out of the nav pack | PASS | not in list.json; `navs/deathrun_dojo.nav`, `deathrun_jigsaw2.nav/.lsv` → HTTP 404 |
| dojo and jigsaw2: still playable by hand | PASS | `gm_map deathrun_jigsaw2` from Last Stand: "Map preset: Deathrun", DeathrunManager and dr_bot_traps running; deathrun_dojo the same; with no nav "Map timer paused while the bots learn this map" (the normal learn) |

## The mode vote

QA4DE2 with the server's own `data/gamemodes/votemodes.ini` narrowed per test, `gm_votemode` over rcon, bots voting.

| Row | Result | Evidence |
|---|---|---|
| Vote lists match | PASS | votemodes.ini, gm_pubserver DEFAULT_VOTE_MODES and app modevote::DEFAULT: the same 17 modes, br/br_duo/br_squads last |
| Keep Deathrun offers only Deathrun maps | PASS | "Map vote for Deathrun: deathrun_temple deathrun_crash_egypt deathrun_death deathrun_starwars" (dojo, jigsaw2, cs_assault, de_* all installed) |
| Keep Battle Royale offers only BR's list, and BR stays | PASS | four keeps: "br_valley de_dust2 de_inferno de_aztec" and the same set from de_aztec/de_inferno (no cs_assault, which is installed; cs_italy/de_train aren't in the sim); current.txt stayed `br` after moving to de_aztec, br_valley, de_inferno (the QA4 self-check's "Keep BR ends BR" is fixed, a7ef360f) |
| Keep Last Stand offers only its list, and it stays | PASS | "Map vote for Last Stand: ls_outpost de_inferno de_nuke de_aztec de_dust2"; moved to de_dust2, still `laststand` |
| A vote switches the mode | PASS | votemodes "deathrun br" on deathrun_temple: "Map vote for Battle Royale: br_valley de_dust2 de_inferno de_aztec", then "Mapchange to de_aztec" and gm_battleroyale running, current.txt `br` |
| A BR variant stays on a br_ map | PASS | `gm_mode br_duo de_dust2` then `gm_map br_valley` (map preset br_*): still `br_duo` |
| A person's vote with the menu, the Rules page | NEEDS USER | menus only show to people |

Note: when Battle Royale is voted onto de_aztec/de_dust2 it plays with "no map pack, so a zone goes round the spawns"
(by design, P5's list).

## Re-checks of the earlier self-checks
- Area D (W1): knock-downs/finishes/bleed-outs/drop-in/layers agree. The revive rate doesn't (16 % here vs 28 % on
  W1's re-sent-goal run, 9 % before it).
- Area E (Parity A): crash_egypt "19 %" and death's later "27 %" only hold with the Deathrun agent's own navs (F1); with
  the navs players get, both fail. The Keep-BR map bug is fixed. starwars/temple round-timer behavior differs (F2).
