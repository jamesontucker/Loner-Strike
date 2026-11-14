# QA round 4, area C: Last Stand and the creatures (QA4-C, 2026-11-14)

Fresh QA agent; no product code changed. Builds: brain **ls-next3 run 43604814272 (`285b895`)**, the head of
`refs/heads/ls-next3` and the newest green run (checked with `git ls-remote` and `gh run list` at the start; downloaded
myself, sha1 `29973245…`, `mode revive`/`mode down`/`ls_creature_flee_last`/`ls_creature_cap_now` found in the sim's
mp.dll). Plugins compiled fresh from a `git archive` of tools **HEAD 314c7f47**, configs and maps from the same snapshot.
Sims QA4L (27404) and QA4L2 (27405), clones of LSPLAY, `-insecure`, ls_outpost as shipped (no upgrade pack) unless noted,
Normal (`lst_difficulty 1`), bot style 1, adaptive cap on. A test-only probe plugin (`qa4c_probe`, in my scratchpad, not
in the repo) logged what the mode doesn't: money around each revive, each downed bot's crawl distance and pistol shots,
bot money/perks/traits, and each creature's state/enemy/health/look. Sims deleted at the end.

**Totals: 31 rows - 22 PASS, 5 FAIL, 4 NEEDS USER.** Waves reached (information only, difficulty untouched as the user
decided): 6 Normal bots fell on **wave 11** (shipped map) and on **wave 8** (with W1's upgrade-spots pack); 16 bots fell
on **wave 11** (the adaptive cap held them at 12 alive on this loaded Mac, so waves 8-11 took 230-520 s each).

**Environment note for the coordinator (not a product finding of this area):** all CrossOver sims on the Mac froze five
times (08:11, 08:18, 08:23, 08:27, 09:12): every hlds.exe went to 0 % CPU, logs stopped, rcon went silent, and a new
`wineserver` started. Each time it coincided with QA4F's built app (`scratchpad/qa4f/Loner-Strike.app`, run with its own
HOME) starting a server (`Servers/qa4c-crash-test`, `Servers/.nav-learner`) or a crash test (one AeDebug entry, pid 208
at 08:23, winedbg still hung). My own debug app run with a throwaway HOME did **not** replace the wineserver. Stands had
to be restarted four times; other agents' sims (QA4DE*, QA4A, QA4S) froze with mine. Worth a look by F/MCP: if the app
can replace the bottle's wineserver while a game client runs in it, the player's game would freeze too.

## FAILs

### 1. A bought turret does no damage to creatures (owner: Y for the fork's friendly creature, W1 for the turret component)
- Repro (any sim on this brain, rcon): `bot_stop 1`, then during a wave
  `ls_creature spawn turret 269 152 10 0 friendly=1 owner=1 health=100000` (the same line `turret_spawn` sends; 269 152
  is where a bot placed one in the 16-bot stand). Read the turret's `pev_enemy` and that creature's health every 2 s.
- Seen: the turret goes to state 7 (attack) and switches targets every 2-3 s (enemy 235, 239, 219, 230, 221...), but
  **every target stays at full health** (zombie 140/140 for 25 s). Same outside a wave: turret vs zombie at 400 u in the
  open, 24 s, zombie 400/400; turret vs headcrab 25/25 for 17 s, then the turret gives up (state 4, enemy 0). A zombie
  chasing the turret never hurt it either (turret 150/150). With wave creatures it did take damage (110/150), so
  creature-to-turret damage works in a wave, but turret-to-creature never did in 4 tests.
- Friendly side is right: `pev_team` 1, its enemy is always a creature (60+ samples, never a player), bots placed one in
  waves 7, 9 and 10 (`Turret: 4 placed a turret (205) at 269 152 1 in wave 7`).

### 2. The armory's Mk II upgrades the pistol in a bot's hand (owner: W1, gm_laststand `bots_upgrades`)
- On the shipped ls_outpost (no bench) Last Stand buys bots their Mk II at the armory with `ls_bench_use(p, false)`,
  which upgrades the **active** weapon (`bench_why_not`). Bots often hold their pistol there.
- Seen: 16-bot stand: `6 upgraded a DEAGLE to Mk 2 for $3000 in wave 6`, `5 upgraded a DEAGLE to Mk 2 for $3000 in
  wave 8` - both Mk IIs of that stand went on pistols (bot 6 later: `lvl 2 wpn weapon_deagle`). 6-bot stands: M4A1 twice.
- Repro: 16 bots, ls_outpost, stand to wave 8; grep `to Mk`. Fix idea: switch to the primary before the upgrade, as the
  brain does before USE at a real bench (W2.md).

### 3. A boss wedged in a broken gateway is removed as "stuck" (owner: W1 for the cleanup rule, Y if the garg's hull can't pass the gate)
- 6-bot stand, wave 10: `creature 209 (boss) idle out of sight at -628 208 0; sent to the gate at -597 -8` → `West gate
  broken in wave 10 (by boss)` → `creature 209 (boss) stuck at -556 1 3 for 30 s (one of the last); removed`. The garg
  stood in the West gateway (gate at -597 -8) for 30 s, more than 500 u from any defender, and the round ended with the
  boss removed, not killed (no $1000 boss pay). The "busy at a gate" exception only counts standing barricades.
- No hang (the wave ended), so it doesn't fail the round-ending row; seen once in 3 boss waves at 10.

### 4. `ls_creature_alive` stays stale after `ls_creature remove` (owner: Y)
- Repro: idle stand, `ls_creature spawn zombie -300 0 40 0 health=100000` twice → `ls_creature_alive` 2;
  `ls_creature remove zombie` → "2 removed", but `ls_creature_alive` is still **2** after 10 s and `ls_creature list`
  says 0 alive. With `ls_creature kill all` instead it drops to 0. Last Stand uses kill, so the mode isn't affected; a
  Studio mode or plugin reading the cvar after a remove would be.

### 5. Paid traps: no bot used one (owner: W2/W1; low confidence, one stand)
- With W1's upgrade-spots pack (3 `ls_trigger_trap` listed by `ls_comp_list`, ids 31-33), 6 bots, 8 waves: 0
  `switched a trap on` lines (machines were used: 7 perks; the box once). W1/W2 earlier saw 2-10 trap pulls a stand.
  The shipped ls_outpost has no traps, so this only affects packs and Studio maps. Re-run a few pack stands to see if
  it's a regression or chance.

## All rows
| Item | Result | Evidence |
|---|---|---|
| Economy: kill pay, wave money, rifles by wave 2-3 | PASS | 6 bots: avg $1666 after wave 1, $3030 after 2; 6 of 6 rifles by wave 3 (12 guns, 6 rifles); 16 bots: 12 rifles by wave 3, 20 by 5 |
| Shop prices rise each wave | PASS | Armor $700 (w2-3) → $800 (w5) → $900 (w8); Ammo $350 (w3-5) → $400 (w6-8) |
| Perks (armory and machines) | PASS | shipped map: 13 perks in 11 waves (Tough $1500, Light feet $2500 as the 2nd); 16 bots: 33 perks by wave 10; pack: 7 perks at machines by wave 7 |
| Upgrade bench Mk II / III | FAIL (#2) | Mk II bought at waves 6 and 8 (16 bots) and 8 (6 bots), but 2 of 4 went on a Deagle; Mk III never reached (stands fall at 8-11) |
| Mystery box | PASS | rolls in every stand: P90, M3, AK47, DEAGLE, FAMAS, SG552 x2, XM1014, UMP45, $950 each |
| Turrets | FAIL (#1) | placed by bots (waves 7, 9, 10), friendly (team 1, never targets a player), but no damage to creatures |
| Paid traps | FAIL (#5) | 0 uses in one 8-wave pack stand |
| Power-ups | PASS | 14 Airstrike, 11 One-hit kills, 9 Full resupply, 5 Double pay picked up across the runs; an Airstrike right after a revive paid +$400 on top of the $150 |
| Knock-down state (server side) | PASS | `ls_down_list`: `sequence 101 (want 101) gait 0 frame 255 view 14 weapon weapon_usp hp 1 left 29 s` for all 6 bots; 53 samples of downed bots all `perks 0` (perks lost) |
| Knock-down visuals on the real client | NEEDS USER | pose, red pulse, bleed bar, pistol in hand as a person sees them |
| Bleed-out and revives | PASS | 6 bots: 26 downs, 12 revived, 14 bled out at 30.0-30.1 s; 16 bots: 67 downs, 46 revived (69 %), 21 bled out |
| The $150 revive bonus | PASS | probe, reviver's money 50 ms after the revive: +150 in 60 of 63 (+160 twice: a $10 hit in the same tick; +550 once: an Airstrike pickup in the same tick) |
| Round table: counts and health | PASS | 6 defenders: 28/29/29/30/31, hound pack 23 at 6, boss 22 at 7, 38/40, boss 27 at 10, 45 with heavies at 11; health x1.40 → x2.03; 16 defenders: 61/62/64/65/67, 50 (pack), 47 (boss), 82, 87, 57 (boss), 98 at x2.40 → x3.83 - all match the formula in W1.md |
| Specials and bosses | PASS | hound pack 6, babygarg 7, garg 10, gonarch 15 (enraged red, killed), garg 20, heavies from 11, boss again 25 and 30 |
| Tiers 2/3 and enrage | PASS (server) | wave 15: 27-28 of 28 alive with a glow shell, 1-2 red (enraged); wave 20/21/25/30: all alive tier 3 (`fx 19 color 255 70 10 scale 1.08`); gonarch at 415/8511 hp `color 255 0 0` |
| Tier/enrage glow on screen | NEEDS USER | how it reads in the client |
| Alive cap follows the game DLL's | PASS | wave lines say "12 alive at most" while `ls_creature_cap_now` was 12 (16 bots) and 28 when the cap was 48 (6 bots: 16+2×6); wave 21: cap_now 48 → 23 and `cap 23` in `lst_perf`, 28 alive kept (nobody removed); wave 30: cap 23 → 15 → 12 |
| Fleeing and returning | PASS | a hurt spitter at state 6 (146/654 hp) with 5 left; with 3 or fewer left the mode re-orders them (`behave chase` + `target`) every 5 s; note `ls_creature_flee_last` is 0 (off) by default, so it's the mode doing this, not the DLL |
| A spitter running home and back, by eye | NEEDS USER | - |
| Idle ranged creatures sent to a gate; no hangs | PASS | 0 "not killed for 3 minutes" in every run; longest waves 218 s (6 bots), 519 s (16 bots, capped at 12 alive); 33 and 69 creatures sent to a gate in the two long stands, 7 and 31 of them later removed as stuck rather than coming (spitters/assassins at -1400 ±800 sent 3 times without moving) |
| Boss waves end with the boss killed | FAIL (#3) | garg removed as stuck in the West gateway at wave 10 (1 of 3 boss-10 waves seen) |
| Wave pace (information) | PASS (info) | 6 bots: wave 11; with pack: wave 8; 16 bots: wave 11 |
| Bench saving / spending between waves | PASS | avg bot money after each wave $2.1-3.5k (6 bots), $2.3-3.5k (16 bots): they don't spend it all; no bench offer exists on the shipped map, so the brain's bench saving itself wasn't exercised (pack stand fell at 8 before a bench buy) |
| Board repairs between waves | PASS | "bots repaired N barricade health before wave N" every break: 12-3661 hp (16 bots: 2076, 2317, 57, 510, 1756, 1159, 419, 2325, 2977, 3661) |
| Personality differences | PASS (weak) | 16 bots (style 1 mix): bold 6 bots spent $7966 each, 2.5 perks; neither-bold-nor-careful 8 bots $4568, 1.5 perks; careful 2 bots $9200, 3.0 perks; teamwork ≥ 0.5 revived 4.2 each vs 3.1 below |
| Downed bots crawl and shoot back | PASS | 6 bots: 21 of 26 downs fired the pistol (avg 14 shots, up to 36), avg crawl 156 u (up to 456 u); 16 bots: 31 of 67 fired, avg 65 u; lying still for a revive seen (12 s, crawled 35 u) |
| 30-wave crash check | PASS | godmode bots (test plugin), `lst_jump` 15, 20, 21, 25, 30 then 31, 32: each wave at its cap, fps 47-100 (one 47 dip), rcon answered throughout, no AMXX error log, `crashcheck.sh since 9333` empty |
| `ls_creature` controls | FAIL (#4; the rest pass) | state: idle 1, wander 2, guard 4, flee 6, chase 5, attack 7, dead 9; `target <e> 1` → enemy 1, `target <e> 0` → guard, enemy 0; speed ×2 / ×3 on a runner: 425-562 / 708-764 u/s vs ~300, ×0.5 on a zombie 115 vs ~215; tier 2 `255 200 0`, tier 3 `255 70 10` + scale 1.08, tier 0 none, `tier runner 2` by type; `behave all default`: zombie back to wander, runner to chase; bad type/behavior answer with usage; notes: `tier 7` silently becomes 3, `target <e> 999` and `target 99999 1` say nothing |
| Weird cases: everyone downed, map change mid-wave, bot kicked while downed, cap at 128 | PASS | all 6 downed at once → "The base fell on wave 1 (Nobody left standing)", a new stand began 54 s later; `changelevel` mid-wave 2 (a bot down) → clean prepare and wave 1, no errors; bot 3 downed then kicked → `ls_down_list` empty, the new bot in slot 3 alive and not down, no errors; `ls_creature_max 128`, `lst_alive_max 128`, adaptive off: wave line "128 alive at most", 34 extra spawns took it to 128, the next 26 refused with "128 alive already", 364 of 2145 entities, fps 17-27 with 127 alive, adaptive back on → cap_now 12 at once, nothing removed, no crash |
| App Play page for Last Stand | PASS | debug app (04:51 build; Play.tsx unchanged since 01:48) with a throwaway HOME and a sim copy as the game: mode "Last Stand - Hold the base against waves of creatures - Bots play this well"; the map list puts "Maps for Last Stand" first (cs_office, de_aztec, de_dust2, de_inferno, de_nuke, ls_outpost); the button reads "Play Last Stand on ls_outpost" and sends `{"map":"ls_outpost","bots":5,"mode":"laststand","settings":{}}` (launch caught in the page, nothing started); one console 404 for `/thumb/de_dust2.png` (sim copy has no thumbnail) |
| Starting Last Stand from the Play page into the real game; buying, perks, turret placing as a person | NEEDS USER | needs the real client |

## Notes
- After a `changelevel`, the server's `log_amx` lines stopped reaching qconsole.log (they still went to the AMXX log),
  so later evidence comes from `addons/amxmodx/logs`.
- Setting `lst_wait_human 1` mid-wave with no person on silently ends the stand (back to prepare, creatures gone, no
  "held"/"fell" line). Expected from the setting, but it leaves no log line.
- Logs and scripts: scratchpad `qa4c/` (`logs/stand6.log`, `logs/stand16_run3_amxx.log`, `logs/s16.log`,
  `logs/pack_amxx.log`, `logs/jump4.txt`, `logs/ctl1.txt`, `logs/turret2.txt`, `run.sh`, `jump2.sh`, `ctl.sh`,
  `turret.sh`, `probe/qa4c_probe.sma`).
