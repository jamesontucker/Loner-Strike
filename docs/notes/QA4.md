# QA round 4 — checklist (DRAFT, not run; the user starts it)

Drafted by MCP, 2026-11-14, from docs/notes/QA.md (rounds 1-3) and every agent's notes. It covers what landed since
round 3 (HEAD 1ceff77 then) plus round 3's open items. Same method as before: real routes, real SDHLT through
CrossOver, HLDS sims cloned from `Sim/LSPLAY`, the real client where it says so, no product code changed by QA, bugs
filed under "Requests from QA, round 4" in the owner's notes (editor → P5.md, compile → P4.md, Studio → X.md and
X2.md, Last Stand / components → W1.md, Battle Royale → W3.md, brain/bots → W2.md, DLL/creatures → Y.md, site → Site.md,
legal → Legal.md, crash tracking and MCP → MCP.md).

Before starting: check what's still in flight in the owners' notes. Items marked **(when it lands)** are skipped if
they haven't landed, and noted as "not in this build".

## Rules for every area
- Sims: at most 4 per QA agent, at most 8 on the Mac in all (`pgrep -fl 'Sim\\.*hlds.exe' | wc -l`), only 2 while
  that agent compiles; the load average under 20 before a heavy step; compiles `-threads 6` under `nice -n 15`.
- Stop only your own servers (by PID or your own folder in the pattern). Never attach a debugger to a CrossOver hlds
  (Y.md: the 0xfff4fadc crash). After each run: `~/loner-strike-sim/creatures/crashcheck.sh since <mark>`.
- Delete each sim copy when its area is done. Tests never write into the real game (LS_TEST_GAME → a sim copy).
- A row passes only on evidence: a log line, a route's answer, a screenshot, or a Studio debug event. Write which.

## Areas, sims and order
| # | Area | Sims (folder, port) | Wave |
|---|---|---|---|
| A | Map editor (perf, selection, Checks, Budget, leaks, I/O, new tools) | QA4E 27400 (playtest) | 1 |
| B | Game Mode Studio (Compile, Test run, palette, nodes, security, examples) | QA4S 27402 (playtest/debugger); Test run's own hidden server | 1 |
| C | Last Stand and creatures | QA4L 27404 (16 bots, ls_outpost) | 1 |
| F | Platform: crash tracking, MCP live, site and legal, setup | QA4C 27410 (crash tests); the Linux test machine | 1 |
| D | Battle Royale (squads, downed, revives) | QA4B 27406 (squads), QA4B2 27407 (duos/solo) | 2 |
| E | Deathrun maps and the mode vote | QA4D 27408 | 2 |
| G | Community: share and import | QA4G 27412 (mode installs and play); the user's test GameBanana page | 2 |
| H | Windows (real Windows 10 and 11 PCs; see docs/notes/Windows.md) | local servers on the PC | any (a separate machine) |

Wave 1 runs A, B, C and F side by side (5 sims at most, 6 while a Test run is going). Wave 2 starts D, E and G when
A and F have stopped their sims (4 more). G's offline and malicious-archive rows need no sim and can start in wave 1. The real client is needed in C, D and E (one at a time: the user's game).

---

## A. Map editor (P5, Parity A, Checks, MCP)
Use br_valley v6 (3 MB .map) and ls_outpost; WebKit (the app's engine) first, Chromium second; 1440×900.
| Item | Pass when |
|---|---|
| Big-map perf (PerfAudit #1-#10, 4f578685/7cffb07d, 5c7cb5be, 5bd9f4b9) | br_valley in Quad: panning a 2D view stays ≥ 30 fps in WebKit; a nudge and a drag-move have no frame over 100 ms in the frame-split spec; idle traffic ≤ 2-3 requests a minute (the long polls counted) |
| Selection for agents (10190014, c0db9c22) | Select 3 brushes in the page: `editor_selection` (MCP) lists them within a second with sizes and bounds; "paint these" with `editor_set_texture {selection: true}` changes only those, one undo step; `editor_select` from the agent selects in the page within a second; with the app's network blocked, the page asks at most once a second |
| Checks (f866295a, 12bf42b7, 2158ee2a) | Every kind on its purpose-built map (Checks.md table) is found, Pick/Look go there, each one-click fix works as one undo step; an edit isn't blocked while checks run on br_valley |
| Budget (c6c0c15f, c00088f6) | Bars match the last compile's chart; a red bar makes Playtest and Publish ask first; a made-for Last Stand adds its creature roster |
| Leak tracing (5c1e278f) | A room with a gap: after compile the bar shows, the line goes through the gap, the hole's brushes are listed and picked; closing the gap clears it |
| Entity I/O view (07ab5f76) | ls_outpost's gates and multi_managers show every target/killtarget/master link with delays; clicking a link picks both ends |
| Extrude, vertex split/merge, shear (a0c5f385, 7887a143) | Each refuses a non-convex result with a reason; each is one undo step; shear in Front view leans the box on the grid |
| Primitives cone/torus/arch spirals (d80291a0) | Every setting in Preview matches the placed brushes; all valid, convex, base corners on the grid |
| Face sheet (7e1946f1) | To the view, Pick up, right-click paint, Alt+right-click wrap round an edge (texture runs on) |
| 2D views (e8f90dda) | 8 handles stretch with the edge on the grid; zoom to the cursor |
| Content Browser + credits (9a28bb66, d4b35435) | Drag a model/sprite/sound onto the map; Choose… on an entity key; the credit badge matches `content_credits` |
| Playtest on QA4E | Compile Normal → install into the sim's game (not the real one) → bots load the map |
| Round 3 regressions | The Hammer/J.A.C.K. imports from round 3 still re-import → save → export → compile clean |

### Area A self-check (Parity A, 2026-11-14, before QA starts; browser specs, the frame-split spec, Rust tests)
Run on `tools` at this commit, dev server + production build, WebKit (Playwright) and Chromium, 1440x900, load 10-19.
| Item | Verified here | Evidence | Left for QA / the user |
|---|---|---|---|
| Big-map perf | yes | 2D pan in WebKit 55 fps (editor-flatperf); nudge/drag worst frame 37 / 33 ms WebKit, 17 / 17 ms GPU Chromium (editor-framesplit); idle traffic **failed at 5 requests/min, fixed to 2** (state and selection long polls held 55 s instead of 25 s; editor-idle.spec.ts, both engines) | a check in the built app (a WKWebView host gave nudge p95 26 / worst 37 ms) |
| Selection for agents | partly | `an_agent_paints_what_the_person_selected` (Rust), editor-sync spec in both engines | the live MCP client and the "network blocked" case |
| Checks | yes (specs) | editor-checks / editor-checks-webkit in both suites | every Checks.md map by hand |
| Budget | yes | editor-budget (a red bar asks before Playtest/Publish, a Last Stand roster); after a real Final compile of an ls_outpost copy, `/api/tools/editor/budget`'s last-compile rows equal the compile log's chart row for row (models 3.7 %, planes 1.9, vertexes 3.1, nodes 1.9, faces 2.1, clipnodes 4.6, leaves 1.3, marksurfaces 2.8, surfedges 1.3, edges 1.4, AllocBlock 12.5) | - |
| Leak tracing | yes | editor-leak (the bar, Go to leak, Pick brushes); a real compile (Fast) of a room whose east wall stops 64 units short of the ceiling: "The map leaks", the line's first point (512, 0, 224) is in the gap, the nearest brushes are the ceiling and the short wall (32 u); closed again: the compile finishes, the leak clears | - |
| Entity I/O view | yes (spec) | editor-io in both suites | ls_outpost by eye |
| Extrude, split/merge, shear | yes | Rust extrude/vertex/shear tests, editor-extrude, editor-flat (shear) in both engines | - |
| Primitives | yes | shapes.rs tests (valid, convex, base corners on the grid), editor-shapes in both engines | - |
| Face sheet | yes (spec) | editor-facesheet in both suites | Alt+right-click wrap by hand |
| 2D views | yes | editor-flat in both engines (3/3) | - |
| Content Browser + credits | yes (spec) | editor-content in both suites (one failure under load in a shared run, passes alone) | a GameBanana download |
| Playtest on QA4E | yes (CS 1.6 and CZ, current builds) | an ls_outpost copy compiled **Final** (421 s, no problems, `-threads 6` under the dev server's test-only `LS_COMPILE_THREADS`, nothing installed into the real game); the .bsp into Sim/QA4E (a clone of LSPLAY with the newest green ls-next3 brain, run 43571907279 / `0af2205`, `ls_creature_flee_last` found in its mp.dll, and plugins compiled fresh from tools `7319d3a2`): bots loaded it, the nav was saved (365 areas) and "map knowledge for qa4_outpost" logged in 193 s; **CZ** (Valve's CZ server from SteamCMD in Sim/QA4Z, started the way the app's hidden server is: its server.cfg, `-insecure`, one bot) with the same brain: nav saved and map knowledge in 58 s | F5 from the editor in the built app (it starts the player's own game) |
| Round 3 regressions | partly | br_valley import / autosave / Save as / Recent (editor-bigmap, both engines); rmf/jmf unit tests (9 pass) | QA's real .rmf/.jmf files aren't on this Mac (pmk_m1-3, regamedll_test_map_v8, aim_stadium): re-import, save, export, compile them |
Suites: Chromium 53 passed / 1 skipped (editor-bigmap, needs env; run separately: passes); WebKit 53 passed / 1 skipped.

**Release blocker found and fixed here (Condition Zero learned with Valve's bots):** the hidden server that teaches bots
a map brought Loner-Strike's brain in only when the server had no bot profile; Valve's CZ server ships one, so CZ maps
were learned by Valve's own bots (no map knowledge, a nav from the old brain). Now (navs.rs) the brain goes into every
hidden server whose mp.dll isn't ours, CZ's own bot profile kept, taken from the game being learned or else from the
other game in the same Half-Life folder (`condition_zero_learns_with_our_brain`). On this Mac the player's CZ still has
ReGameDLL 5.30.0.814-dev from about Nov 7 (no brain): setup recorded it as its own then, and updates are offered only
for the game picked at the top, so CZ was never offered one. Not a Steam verify, not deliberate. The app now shows a bar
for the other set-up game when its parts are behind (`/api/app` `game.others`, tests/files-update.spec.ts in both
engines); the player's real CZ install was not touched.

## B. Game Mode Studio (X, X2)
| Item | Pass when |
|---|---|
| Compile button (c0c650c5, b2865b91) | Real amxxpc without installing; a compiler error lands on its node; status icon; Mod+B / F7 |
| Test run (159181be) | On a good example mode: pass with cost against the baseline and every node ran; on a mode with a deliberate run-time error: fail with the error on its node; Cancel stops the hidden server (its process gone); the person's game untouched |
| All Actions palette (5c7cb5be) | Folders and sub-folders, search opens matches, Favorites, Recently used, Context Sensitive from a dropped wire |
| Several event graphs (44dd882a) | Events on a second tab run; copy/paste across tabs keeps inner wires; two of the same event run in tab order |
| Custom events (69a594c2) | Call Custom Event with typed inputs; Set Timer by Event; an `ls_event` entity on the map fires it |
| Timelines (115d83ea, af17cd85, 462b4872) | A looping number track drives a node every tick; Finished fires; reverse works; each key type (Linear, Constant, Ease In/Out/In-Out, Custom tangents) shapes the curve as drawn; a 4 s Timeline lasts 4 s of game time on a sim (Finished's game time minus Play's, ±0.3 s) and each event key fires once |
| Node help (f90f5238) | F1 on any node shows its help; search finds by words; the example drops in |
| New nodes (cd541ea8, 71ca7012, 4d7cf6b1) | Each W1/W3 component node and the round-table/turret nodes does what its help says in a playtest (debug events as evidence) |
| Security (2536e0b9, b2865b91, c0c650c5) | Run Server Command refuses `exec`, `rcon_password`, `quit`, quoted first words, `amx_` lines; Run AMX Command allows only its list; Kick reasons and map names can't inject |
| Example modes (60d61837 Infection; X2's five) **(when they land)** | Each compiles, test-runs to pass and plays 3 rounds with bots on QA4S without run-time errors |
| Debugger | Breakpoints, Resume, live variables (round 3's For Each freeze stays fixed) |
| MCP Studio (d3b922bb) | An agent builds a 3-node mode with `studio_edit`, `studio_check` passes, `studio_test_run` asks first and returns pass/warn/fail |
| Mode settings (361dca58, 77c560a8, 76a4d024) | A mode with a number setting: change it on the Play page (local) and on a remote server's page; set an out-of-range value; Reset one, then Reset all | The new value shows in the game at the next round; out of range warns and is still used; reset puts the default back; the first save on a fresh Linux server works (no SFTP error) |
| Data tables (eaef8b97) | Make a table in the grid; paste CSV; Get Row / Get Cell / For Each Row / Find Rows in a graph; play Gun Game Lite | Values in game match the grid; a bad CSV row is refused with a reason; Gun Game Lite gives the table's guns in order |
| Editing helpers (5fb7762c, 86b8f755, 2e4ff94f, 5a4bbe95, aa1c1970) | Live tweaks during a Playtest; Versions (compare, Restore); Promote to Variable on a pin; bookmarks (Ctrl+Shift+1, Ctrl+1); hold B/S/D and click; drag a variable with Ctrl / Alt / no key; type a value on an unwired pin | A tweak changes the running game at once; Restore is one undo step; Promote names the variable after the pin; bookmarks jump back; quick-add places the node; Ctrl = Get, Alt = Set, no key = a choice; the pin value and Details always match |
| Creature control (6059738b, 462b4872) | On a sim with the current brain: a graph using Creature State, Target, Set Speed, Set Tier, Resume AI, Go To and Creature Idle | State/Target print what the creature does (5 chase, its target); Speed visibly faster; Tier 3 gets the red-orange shell; Resume gives it back its own behavior; Go To walks it there; Creature Idle fires for a stuck one; an older brain logs "need a newer Loner-Strike bot brain" once |
| Bot nature and orders (660e1a72, f3a3ba80) | Bot Skill / Bot Trait print values; Bots: Goal "Hunt it" on a target; the same order with personality influence 0, then 100 | Values match the bot's profile; hunters close on the target (sim: average distance well under the old goal's); at 0 every bot obeys, at 100 some careful ones decline |
| Requirements, screenshots, cover (G's rows) | See G: Requirements in the Studio, Playtest screenshots, Cover card | As in G (not repeated here) |
| Mode list on a running server (6d5eeac4, e7127141) | Add a Studio mode to a running local server, then a Linux one (Community content › tick it › Apply); switch to it at once; remove it; then 60 modes in modes.ini on a sim | `gm_mode <new>` works without a map change; removed: "Unknown mode" at once; an older menu shows "Available after the next map change"; 60 modes list and switch, the vote works; over 128 the log says how many were left out |

## C. Last Stand and creatures (W1, Y)
ls_outpost (32 spawns), 6 Normal bots + the user on the client for the visual rows.
| Item | Pass when |
|---|---|
| Economy and shop (06732e70, be8b2f36) | Bots and players earn kill pay and wave money; bots rifle up by wave 2-3; ls_shop items dearer each wave; perks and the gun upgrade buyable |
| Knocked down (5e5947a8, 16fa01a4) | A downed player crawls, the bleed-out bar shows, a revive by holding USE works, perks are lost; the visuals read on the real client |
| Round table (82c4e4b7) | Counts follow round and team size; types unlock by round; the alive cap follows `ls_creature_cap_now` |
| Turrets (7213bb76) | Bought and placed where the player looks; friendly; shoots creatures, not players |
| Tiers and enrage (a7a7e7dd, 49534c0a) | Tier 2 from round 15, tier 3 from 20 (`lst_jump`), enraged speed/damage under the threshold |
| Fleeing (aeee90ca, Y's flee_hp/flee_home/flee_last) | A hurt spitter runs home and comes back; with 3 or fewer left none flees; no wave hangs over 3 minutes on one creature |
| Wave pace | 6 Normal bots reach wave 7-9; no crash in 30 waves (`crashcheck.sh`) |
| Bench and spending (W1, W2) | 16 bots, Normal, to wave 12; watch money and buys (`lst_debug` or the round table log) | The first bench Mk II around waves 8-10; bots save between waves instead of spending it all; bots repair boards between waves; careful/greedy personalities spend differently (log shows who bought what) |
| Downed bots (W1, W2) | Down a bot near creatures | It crawls, and shoots back with its pistol while downed; it can be revived |

### Area C self-check (W1, 2026-11-14, before QA starts; HLDS sims, no client)
Current builds: brain ls-next3 run 43580692204 (`72cfb7f`) for the first runs and run 43590130925 (`707a12a`) for the
re-runs, each checked inside the copy (`mode revive`/`mode down` in mp.dll); every plugin compiled from tools HEAD
(1e454fbc, c5082d4d) and the repo's configs and maps (scratchpad `fresh.sh`). ls_outpost, 6 Normal bots, adaptive cap on
(as shipped) unless noted. Logs: scratchpad `qa_W1QC_015936.log`, `ls_v5_W1LA_035959.log` (with the map's upgrade spots).
| Item | Result | Evidence | Left for the user |
|---|---|---|---|
| Economy and shop | pass | bots avg $1690 after wave 1, $2585 after 2; 5 of 6 rifles by wave 3; 19 armory buys, armor $650 -> $800 (wave 5) -> $900 (wave 9); perks 14-15 by wave 10; a Mk II at wave 9 (spots run); the box once a round per bot | buying as a person (menu, USE at a machine) |
| Knocked down | pass (server side) | 8-30 downs a run, every one followed by `ls_mode down <id> 1` and later `0`; revives by bots (6-8 a run; the $150 revive pay is set but not logged); `ls_down_list` earlier: sequence 101 held at frame 255, view 14, pistol in hand; perks lost on going down (perk_clear) | **the visuals on the real client** (pose, red pulse, bleed bar, pistol) |
| Round table | pass | counts 28/29/29/30/31 by round, hound pack 6, boss 7 and 10, heavies 11, snark swarm 12; "creature cap now 12 -> 16 -> 20" followed by "N alive at most" in the next wave's line | - |
| Turrets | partly | bots bought and placed 2 from the brain's offer (`Turret: ... placed`); on an older brain the turret went to attack on its own and players' shots didn't hurt it | placing one as a person (kit from the armory, USE where you look) |
| Tiers and enrage | partly | jump to wave 20: `lst_perf` 26 of 26 creatures with a glow shell, all red-orange (tier 3) | tier 2 at wave 15 (the jump run fell before the perf line) and enrage on screen |
| Fleeing and round endings | pass | no wave held by one creature: 0 three-minute cleanups; ranged creatures out of sight sent to a gate (70 in the long run); longest waves 155-265 s | a spitter running home and back, by eye |
| Wave pace | pass (information) | 6 Normal bots fell on wave 11-12 (user: leave difficulty; target 20-30 for human teams); no crash since the mark (`crashcheck.sh since 9260`: empty) | the 30-wave crash run (bots don't get that far; `lst_jump` to 20 ran clean) |

## D. Battle Royale (W3, W1, W2)
br_valley v6; squads on QA4B (16 bots), duos/solo on QA4B2.
| Item | Pass when |
|---|---|
| Squads and duos (5bc0e2df, 14fb4c12, fe158c49) | br_duo and br_squads in the vote and modes.ini; the 30 s lobby (say /squad); squad mates can't hurt each other; markers and radar dots |
| Knocked down in squads (c5ef3a88) | Downed until bled out or revived; out when the whole squad is down |
| Bots reviving (W2/W3) | 4 squad games with bots | Several revives per game (the log counts them); bots revive a knocked squad mate at least half the time it's safe; the reviver gets the $150 revive bonus in modes that pay it (Last Stand; BR sets it to 0, so none there) |
| Bleed-out | A knocked squad member nobody revives | Out after 45 s (the bar runs out), counted as a death |
| Finishing a downed player (fe2c3b71) | In BR an enemy squad finds a knocked player | Their shots finish them (`ls_down_finish` 1); the kill counts for the shooter |
| Loot (5ca3ad76, 060d9a90, 6b4aecdf, 25140068, bed0141b) | Death crates hold the dead player's guns and ammo; armor levels reduce damage; healing items work with the bar; rarity glow colors |
| HUD and drop-in (1d543873, 2c89e395, 83f5f83d) | Players left, kills, zone timer, next-zone distance and radar dot; the drop-in steers and lands without damage |
| Bot layers (decb83fb) | About 3 in 10 bots go below ground, 1 in 4 high; no drop-in deaths in 10 games |

### Area D self-check (W1, 2026-11-14, before QA starts; HLDS sims, no client)
br_valley as in plugins/game, newest ls-next3 brain run 43590130925 (`707a12a`, W2's braver revives) and tools plugins
(c5082d4d, with W1's `ls_mode down` hint), 16 bots in squads of 4, 4 games; an older-brain run of 8 games for the rest
(marked). Logs: scratchpad `qa_W1QB_new.log`, `qa_W1QB.log` (older brain).
| Item | Result | Evidence | Left for the user |
|---|---|---|---|
| Squads and duos | partly | br_duo/br_squads in modes.ini (the vote's list); "squad lobby, 30 s" then "lobby over, 4 squads" (duos: 8 squads) | /squad and party codes, markers and radar dots, no damage between squad mates (not in the log) |
| Knocked down in squads | pass | 34 knock-downs in 4 games: 22 bled out or out with their squad, 7 finished, 3 revived; 0 left hanging | - |
| Bots reviving | **fail** (W2) | 3 of 34 knock-downs revived (9 %; the older brain 8 of 79); W2.md has the numbers | - |
| Finishing a downed player | pass | 7 knocked players finished by an enemy, the kill credited ("out ... by <killer> (weapon_knife, 32 u)"); `ls_down_finish` on with squads | - |
| Loot | partly | per round: death crates 12-15 made, 8-11 searched, 11-25 items taken; armor found at levels 1-3; heals used (bandage, drink) | rarity glow colors and the heal bar on screen |
| HUD and drop-in | partly | 0 drop-in deaths in 12 games (bra.py) | the HUD lines and the drop-in steering on the client |
| Bot layers | pass | 4-8 of 16 went below ground a game (25-50 %; 5-23 % of the time there), 0 drop-in deaths in 12 games | the 1-in-4 high-ground share isn't logged |

## E. Deathrun maps and the mode vote
| Item | Pass when |
|---|---|
| The Deathrun maps that ship (Deathrun.md; sims DR1-DR3 were the agent's) | Each loads, bots run the course and reach the end, traps fire when the terrorist presses them; no crash in 10 rounds |
| Mode vote (83fad7c8) | Battle Royale, Duos and Squads appear in the app's and gm_pubserver's lists, matching votemodes.ini; a vote switches the mode |
| Queued after round 4 (fun_allinone, vehicles) | Not tested: queued for after this round (Deathrun.md) |

### Area E self-check (Parity A, 2026-11-14, before QA starts; sims, no person)
Current builds: bot brain ls-next3 72cfb7f (run 43580692204, its head matched `ls-remote`; checked inside each sim's
mp.dll), plugins compiled fresh from `tools` c8562711 into each sim. Sims QA4D1/QA4D2 (clones of DRPLAY) and QA4V
(clone of BRPLAY), `-insecure`, 8 bots, real time, 15 minutes per map, the shipped deathrun.cfg (brain on:
bot_ls_deathrun 1, bot_ls_fight 1, dr_bot_traps 1). Per round: `drruns.py` on gm_simrec + gm_drrec; crashes:
`crashcheck.sh` around every map. Load 14-26, 7-8 sims on the Mac. Results: ~/loner-strike-sim/results/qa4e/.
Dojo: no dojo rows here; dojo is removed from 1.0 by the user (it isn't in the catalog's Deathrun maps).
| Item | Verified here | Evidence | Left for QA / the user |
|---|---|---|---|
| deathrun_crash_egypt | **yes** | 15 rounds, 20 of 103 runs reach the end (19%), 11 presses, 9 trap deaths, no crash | a person running it |
| deathrun_temple | partly | 11 rounds, traps fire (10 presses, 23 trap deaths), no crash; only 2 of 77 runs reach the end (3%) | Deathrun agent: few finishes (Deathrun.md) |
| deathrun_death | **no** | 45 rounds, no crash; runners die 4-8 s into nearly every round to one trigger_hurt near (-3540, 170, -219) with no press (313 of 314 deaths), 6 finishes (2%), 0 presses | Deathrun agent (same in their DR3 runs at 01:51 and 02:06) |
| deathrun_jigsaw2 | removed by the user | not supported for bots in 1.0 (puzzle-gated: its last door opens on a color sequence); was: 2 rounds in 15 min, both to the round timer, runners stuck at 81% (its puzzles) | - |
| deathrun_starwars | partly | 2 rounds in 15 min (round timer), 7 of 14 runs reach the end (50%) but the rounds don't end; 0 presses (the brain finds 0 traps the terrorist can walk to) | Deathrun agent |
| 10 rounds without a crash | yes on death, temple, egypt | 0 crash reports on all five maps; starwars only reaches 2 rounds in 15 minutes (time-outs) | longer runs of starwars |
| Mode vote lists | **yes** | votemodes.ini, gm_pubserver's DEFAULT_VOTE_MODES and the app's modevote::DEFAULT are the same 17 modes (br, br_duo, br_squads at the end) | the Rules page by eye |
| A vote switches the mode | **yes** | QA4V on classic, the server's votemodes.ini limited to br/br_duo/br_squads, `gm_votemode` over rcon (bots vote): vote 5 "Switching to Battle Royale: Duos on cs_assault", gm_battleroyale running after it; Keep won votes 1, 2, 4 (map vote only) | a person's vote with the menu |
| Mode vote observations | - | (1) a Keep in Battle Royale runs the usual map vote, which offered cs_assault: the map preset then went "Back to Classic", so Keep BR can end BR; (2) a BR mode's map stage offered cs_assault while br_valley was there (no maps/br*.txt list, so every map suits) | the mode vote's owner (83fad7c8) |

## F. Platform: crash tracking, MCP live, site and legal, setup
| Item | Pass when |
|---|---|
| Crash tracking, local (63f17010) | On QA4C: a crash (Y's qa_drop or a forced fault) appears in Servers › Health › Crashes within 2 minutes with time, map, fault and stack; Copy gives the text; the card shows "1 crash in 24 h"; a debugger/stop fault is listed but not counted |
| Crash tracking, Ham Sandwich | Put the official hamsandwich_amxx.dll on QA4C: the warning shows on Health and the badge; Update files puts the fixed one back and the warning goes |
| Crash tracking, Linux **(on the user's Linux test machine, not the production server)** | A crash file appears on the page over SSH; the official .so is replaced on the next start (9d43280b) and the warning clears |
| Crash tracking, Discord | With a test channel's webhook: turning alerts on asks nothing in the app but asks an agent; a new crash posts one plain message with no mentions within 5 minutes; nothing posts for old crashes |
| Crash format with Y **(when Y's capture lands)** | Y's `crashes/*.json` and `modules.json` read as agreed in Y.md; a bad module from the module check shows its own warning |
| MCP live (Claude Code against the built app) | `claude mcp add` with the stdio bridge connects; 170 tools listed; read-only with Allow changes off; every ask-first action shows the native dialog and "Don't allow" leaves things as they were; the activity log lists each call; an agent builds a room, fixes a Checks finding by id, reads the Budget and makes a Studio mode end to end |
| Site (8553153e, 183fc15e, ac912386, e6cd1d71) | Homepage sections, titles and meta as in Site.md; no version numbers; every link works; Windows-first docs; phone width without sideways scroll |
| Legal (91a31d52, 9faeebc8, 302761ea, ee7222f1) | Terms and Privacy linked under Set it up and in About; Publish requires "I have the right to share this"; DMCA agent and legal email right; the warranty panel is bold and bordered |
| Site downloads (R2) | On lonerstrike.com press Download for Windows and for Mac | Both files come from the R2 address with versioned names and open (the DMG mounts, the setup.exe starts) |
| Support Pack and thanks (d073ed9b, f40f709f, b782545b) | The Support Pack section; both checkout buttons (one-time, monthly); finish a test purchase | Each opens the right Polar product; after paying, the thanks page (connect Discord, /link steps, downloads); Terms has "The Support Pack" paragraph dated Nov 14 |
| Made by players page | Open it with an empty list, then with entries; press Open in Loner-Strike | The empty state reads well; Open in Loner-Strike opens the app on that item; the support cards are centered |
| Fresh setup and update | A clean bottle: setup end to end, then an update; the fixed Ham Sandwich is installed on Windows/CrossOver |
| **Update from 0.1.2, Mac (critical)** | Install 0.1.2 (DMG) in a test user account; set it up, make a local server, change some settings, play a match (records), keep a backup; then update to 1.0 through the app's banner (coordinate with P5's dry run in P5.md) | Nothing lost: settings, servers (and their settings), records, backups, recent maps; the AppData folders moved on first start with the old ones gone or explained; the Ham Sandwich check (hamguard) puts ours in on first start; the server starts |
| **Update from 0.1.2, Windows (critical, needs user)** | The same on a real Windows PC with the 0.1.2 NSIS installer, then 1.0's (see also H › Update) | The same as on the Mac; no console windows; the big folders leave Roaming |
| Condition Zero (W2, Y) | Switch the game to Condition Zero; Play with bots; then the CZ update path from 0.1.2 | The hidden server runs our brain (the console's version line); CZ bots use our behavior; updating keeps CZ's settings |
| Update banner | With an update available and a brain update too | One banner for both, one button; nothing asked twice |
| Voice on its file channel (voice-agent; **still needs user**: a voice and mic test) | Voice on; say a bot command in game; open the feed and "Who's here" | Bots answer aloud; the feed shows the lines; who's here lists players; no RCON command or password in any voice log |
| Editor frame time | Open the map editor's performance readout on br_valley | The frame-time number updates while moving and matches the perf spec's range |
| Nav pack | Fresh setup; Maps page | 72 maps with navs from the live pack; no dojo or jigsaw2 |


### Area F self-check (MCP, 2026-11-14, before QA starts; Rust tests, the stdio bridge, the site in two engines)
Run on `tools` at c0266b76. **No sim was started** (a process count read 15 sims, but it counted helper processes: 5 were
running), so every row that needs a running game is left for QA. Crash files read below come from existing sims, so they're **old build**
results for the brain; they only show that the reader handles real files. Newest green ls-next3 at the start: run
43571907279 (head 0af22051); QA runs on that build plus the current `tools` plugins and maps.
| Item | Verified here | Evidence | Left for QA / the user |
|---|---|---|---|
| Crash tracking, local | partly | `crashes::tests` (5 pass); the ignored `real_crash_files` test over the 7 sims with crash logs (LSPLAY, QA4E, W2BN2, W2HUN, W2PLS, X2T, YF1): every file read, each one a debugger/stop fault (0xfff4fadc) so listed but not counted, 0 warnings (old build) | a forced crash on QA4C on the current build: Health › Crashes within 2 min, Copy, "1 crash in 24 h" |
| Crash tracking, Ham Sandwich | partly | `hamguard::tests` (3 pass); the Ham Sandwich in all 16 sims and the real game hashes to the fixed one (d1ebbf716f3a…) | putting the official dll on QA4C: the warning, Update files, the warning goes |
| Crash tracking, Linux | no | - | the user's Linux test machine |
| Crash tracking, Discord | partly | the message test (no mentions, plain text) | **needs user**: a test channel's webhook |
| Crash format with Y | no | Y's capture hasn't landed | when it lands |
| MCP live | partly | the real stdio bridge (`--mcp-stdio`) against the harness on a scratch app dir: initialize (2025-11-25), 174 tools, 10 resources, 5 prompts; maps_list 60; an agent made a box room (hollow → 6 walls), placed a stuck spawn, Checks found it (1 error) and `editor_apply_fix` by id fixed it; Budget 20 bars; selection by agent; a Studio draft built, wired and checked with 0 errors and its code has plugin_init; `studio_compile` asked and "Don't allow" left it as it was; `editor_new` asked before dropping the unsaved map and refused left it; the activity log listed each call (17 done, 2 failed, 1 refused) | `claude mcp add` against the built app with the native dialog; `editor_save` (the harness has no window for the save dialog, so it failed there); the 170 in the row is now 174 |
| Site | yes | 7 pages, 349 links (35 external): no broken local link (the only hits were `/` and anchors built by script); every page at 375 px in Chromium and WebKit without sideways scroll; titles as in Site.md; no version numbers in the copy (the release banner names 1.0.0 on purpose, see Site.md); us-english-check clean | the external links by hand |
| Legal | yes | Terms and Privacy linked in Set it up (Setup.tsx) and About (AppSettings.tsx); Publish's "I have the right to share this" box is required and the button stays off without it; terms/privacy carry DMCA-1070465 and legal@nanoshrineinteractive.com; the warranty panel is bold with a border | - |
| Fresh setup and update | partly | the setup tests (30 pass: foundation, fetch, patch, components, botprofile, waiting for the game per step) | a clean bottle end to end on the current build, then an update |

## G. Community: share and import (T's browser and library, the list service, the publish workflow)
How it works today (docs/COMMUNITY.md, COMMUNITY-SERVER.md, notes/T.md, notes/community.md): files live on
GameBanana (tagged pages); the Loner-Strike index (`dl.lonerstrike.com/community/index.json` + `.sha256`, published from
the list service on the box by the GitHub workflow to R2) adds what GameBanana can't say; Publish builds a package zip
(`<app dir>/tools/community/packages/`), the person uploads it on GameBanana and pastes the link; Try it installs
through the existing installers and notes it in `<app dir>/community/<key>/meta.json`; uids belong to the first
GameBanana member who registered them. Kinds: map, mode (Game Mode Studio graph + its map kit), prefab, spray,
texture. Creature presets aren't shared: the user decided the community shares content and Game Mode
Studio modes only (2026-11-14), so there are no creature-preset rows here.

**Needs the user's OK first:** every row that registers, updates or reports on the live list, or uploads to GameBanana.
Use a test GameBanana page (a private/WIP submission) and test entries only, and clean them up afterwards with the
user (`lsmod.py` pull/delete, and delete the GameBanana page). Rows marked (local) touch nothing outside this computer.
Expected messages are the app's own words; a row fails if a failure shows a raw error, a stack, or nothing.

### Sharing
| Item | Steps | Pass when |
|---|---|---|
| A map with custom content (local) | Make a map in the editor using a custom WAD texture, a model (cycler_sprite), a sound (ambient_generic) and a sprite (env_sprite), compile it into the sim's game, Publish › Check, Make the package | The zip holds the .bsp, nav/knowledge when they fit, a fresh .res and every custom file where the game looks; no stock/Valve file; README and loner-strike.json; credits listed for each file |
| …depending on content not in the pack (local) | Reference a model that isn't on this computer | Check shows a warning naming the missing file ("…isn't on this computer"), the package is still made without it, nothing outside the game folder is pulled in |
| A Studio mode using components/creatures + a kit (local) | A mode with ls_* component nodes, a creature node and two map markers; Publish › A game mode | The package has `modes/<id>/<id>.graph.json` and its kit; no plugin, no .amxx/.sma; the description says what it needs (components, creatures, the bot brain) |
| A mode needing a newer brain (local) | A graph using a node that needs the brain (creatures) | Publish says which brain it needs; the package records it |
| Prefabs (local) | Content › Prefabs › Share on a prefab using a downloaded model | Zip has `.map`, `.json` (with credits), `.png`; the credits name the model's page and author |
| Names with spaces or Unicode (local) | A map named `de_my map`, a mode named "Zoë's Rush", a prefab "Café sign" | A map name with a space is refused with "Map names can only use letters, numbers, _ and -"; the mode and prefab names keep their text in README/json, ids stay plain |
| Huge files and caps (local) | A map with 1.1 GB of custom files; a screenshot over 8 MB; a package zip over 256 MB | "The package would be over 1 GB."; the big screenshot refused by name; nothing half-written left in packages/ |
| Registering (live: user's OK) | Upload the package to the test GameBanana page, paste the link | Listed (or "pending" for a blocked creator); Yours shows it; the moderators' channel gets the card |
| Updating something shared (live) | Upload a new file to the same page, register again; then try registering another member's page declaring the same mode uid | The same entry updates; the other member's is refused: "…belongs to another GameBanana member's creation… remix it" |
| Unlisting/deleting (live) | Moderator pull (`lsmod.py`) or delete the GameBanana page | Gone from the list after the next publish (≤ 15 min + the workflow); Yours says it was pulled |

### Importing
| Item | Steps | Pass when |
|---|---|---|
| Happy path (map) | Community › a listed map › Try it, then Play with bots | Installed through the Get more job; the Maps page shows it with the Community chip; Play starts with bots |
| Happy path (mode with kit) | Try it on a Studio mode | Built with the Studio's compiler on this computer, its kit in the kits folder, Play buttons per mode |
| A mode with risky nodes | Publish a test mode with Run Server Command and Kick nodes; open it in the browser; Try it. Also a mode whose list entry doesn't say (an older entry) | The card and the details show "Runs server commands" and "Can kick players" badges; Try it asks first, naming them, and Cancel installs nothing; the older entry is asked about after the download, before anything installs |
| Requirements in the Studio (local) | In My mode › Details › Requirements pick a default map and two recommended ones (one base game, one community, one only on this computer); add a Spawn Prop with a downloaded model and a Play Sound with a custom sound | The sources show (Base game / Community / Only on this computer) and the local one says "Share this map first so players can get it"; both files are listed, the community model gets a suggested link; components, creatures and the brain show read-only; MCP `studio_requirements` says the same |
| Requirements at Publish (local) | Publish the mode with one file unlinked, then with a file that's on no computer and unlinked | Unlinked: a warning naming it, the package is made; found nowhere: Publish stops and names it ("…Link them in My mode › Details › Requirements first.") |
| Plays on / Needs / Get everything (live: test entries) | Open the published test mode in Community on a computer missing its map and model; Get everything; then Just the mode on another | "Plays on" and "Needs" mark Missing/You have it; Get everything installs the map and the content through their own installers (each with its checks), then the mode (asking about risks first); items not on the list are named with a note; Play starts on the default map |
| A missing file at run time | Delete the mode's custom sound from the game after installing; start the mode | The map loads (nothing precached that isn't there), the log says what's missing, the feature using it does without **(when X's precache guard lands)** |
| Playtest screenshots (local) | Playtest a Studio mode, press F5 three times in game; open Publish › A game mode | The three shots show under "From your playtests" as PNG with their map; picking and unpicking works; with the cover card that's at most 6 in all; the package folder holds the picked ones |
| Cover card (local) | Publish a mode with no pictures; change the name and the line | The cover card is drawn (name, initials emblem, tags, the line, "Made with Loner-Strike") and is the first screenshot; it's drawn again after each change; unticking it leaves it out; offline it still draws (fallback fonts) |
| Missing content | Install a map whose WAD/model/sound isn't in the pack; a mode needing a component/brain this game lacks | The map installs and the Map doctor names what's missing; the mode says what it needs and links to Mods; nothing crashes the game |
| CS 1.6 item into CZ and back | Switch the game to Condition Zero, Try it on a cstrike-only item | Refused or warned in plain words ("made for Counter-Strike 1.6"); a both-games item installs into czero/ |
| Name clash | Install a map with the same name as a map you made; a mode whose id clashes with yours | The installer never writes over existing files (it says which); the mode is refused: the id clashes with a mode you have |
| Older/newer format | Point the app at a test index with `"schema": 2`; a graph file with a newer `version` | "Update Loner-Strike" for the index (the kept copy used); the graph refused with "made by a newer Loner-Strike… Update the app" |
| Corrupt/partial download, then retry | Cut the network mid-download (Wi-Fi off); then a file whose MD5 doesn't match | A plain error, nothing half-installed, retry works once back; the MD5 mismatch is refused, never installed |
| Malicious archives (local, test zips via a local test server or Get more's Add file) | Zips with `../../evil.bsp`, `C:\x.bsp`, `/etc/x`, a symlink, `.dll/.so/.exe/.amxx` inside, a nested zip, a zip bomb (200:1, > 65,536 files) | Each refused or the bad files skipped with a reason; nothing written outside the game folder; the bomb stopped at the caps ("…over 256 MB" / file count) |
| Install then uninstall | Install a map, a mode and a prefab; uninstall each in Installed | Every file the install put in is gone, files it replaced are back, the user's own files untouched (compare a file list before/after); the mode is out of modes.ini |
| Re-import the same item | Try it again on an installed item | Says it's installed (or offers Update); no duplicates |
| While the game is open | Try it/Update for the map being played | The update waits ("pending") and goes in at the next map change; nothing replaced under the running game |

### Moderation and ownership
| Item | Steps | Pass when |
|---|---|---|
| Unlist and list again (live: test entry) | In Community › Yours click Unlist on the test entry; put the shown code in the GameBanana page's description; click again; then List again the same way | First click: the code and how-to, nothing changes; without the code on the page: "The code isn't on the page yet…"; with it: unlisted, gone from the browser after the next publish, the code can come off; List again brings it back; a moderator's hide can't be undone this way ("…only they can change it") |
| Unlisted while installed (live: test entry) | Install the test entry on a second computer, unlist it | Installed shows "The author unlisted this. You can keep yours, or remove it."; no update is offered; Remove still works and leaves nothing behind |
| Withdrawn page (live: test page) | Make the test GameBanana page private; `lsmod.py recheck`; make it public; recheck. Then delete (trash) it; recheck; restore it on GameBanana; recheck | Private: withdrawn (out of the browser, installed copies show "…withdrawn from GameBanana. You can keep yours…", Try it says it can't be downloaded any more), public again: listed by itself. Deleted: withdrawn for review; back on GameBanana: still withdrawn, a "Back on GameBanana" card for the moderators, listed only after `lsmod.py restore` |
| Unlisted and then pulled (live: test entry) | Unlist the test entry, then remove its LonerStrike tag on GameBanana; recheck | Pulled (revoked) even though it was unlisted; installed copies show the pulled note; List again refuses |
| A file that vanished (live: test page) | Open a test entry's page in Community, then remove its file on GameBanana, then Try it | "The file this creation lists isn't on its GameBanana page any more…" (or "The file isn't on GameBanana any more…"), never a raw download error |
| Report (live: test entry) | Report a test entry with each reason and a 500-character note | Sent once; the moderators' channel gets the card; a second quick report is rate-limited politely |
| Taken down after install | Pull a test entry you installed | Installed copy stays and shows the pulled note; Try it is gone; Update never offered |
| Ownership after reinstall | Remove and reinstall the app (keep the GameBanana account), update your test entry | Updating still works: ownership is the GameBanana member, not the app copy |

### Community roles (docs/ROLES.md; **needs user**: the box deploy, the Discord roles and commands, Polar)
Use a test Discord account for the applicant, a second for the owner, and the official test server. Run on the
current builds (gm_extras and gm_vip 1.2 from `tools` HEAD, ls-control unit 4 on the Linux server).
| Item | Steps | Pass when |
|---|---|---|
| Claim | A server on this computer, then a Linux one: Shared control › Loner-Strike roles › Claim; type the owner code in Discord | "Claiming…" then the owner code; `/server claim` says you're the owner; the card shows the owner linked; a stopped server says to start it; a LAN-only one gets the list service's words — **official server claim PASSED, live, by the user on 2026-11-14**: official status and settings through the app-equivalent path at 10:35 |
| Claimed again | Claim the same address from another app; then `lsmod.py roles servers` | The first app's card says the claim ended; the old server isn't official, its moderators are gone (Discord role too), the team channel got the alert |
| /link | Say /link in game; type the code in Discord; a wrong code; one 11 minutes old; /link again for another SteamID; /unlink | The code shows in chat, console and on screen; "Linked…"; "isn't right"; "ran out"; relinking replaces; unlink works; on a server whose app is closed: "This server can't make link codes…" after 10 s |
| Link squatting | Link a SteamID from a community server with account A; then link it on an official server with account B; `lsmod.py roles unlink <steamid>` | B's verified link replaces A's and A gets a DM; a code for a player not on the server is refused; the unlink is in `lsmod.py roles log` |
| Supporters, live — **PASSED, live, by the user on 2026-11-14** | Buy the Support Pack, connect Discord, `/link` in game (the code also shows in the console), then `/link <code>` in Discord; then remove the supporter and unlink | Evidence: a real $3 Polar order counted at 10:16 UTC, the Discord grant at 10:18 (Supporter role on Discord; the box showed lifetime status). `/link`: code shown in game, then in Discord the bot replied "Linked: … STEAM_0:0:4794… (seen on an official server)" at 11:06 (ID shortened here). VIP: users.ini's supporters block got the SteamID with flag "s", the admin entry stayed separate. supporter-remove at 11:10: the Discord role went, the users.ini block emptied at 11:14, the admin kept. `/unlink` at 11:11: 0 links left. Don't ask the user again |
| Supporters (Polar test events) | Monthly grant, revoke; one-time order twice; a full refund of one, a partial refund of the other; a bad signature; the same event again; an older grant after a revoke | Supporter while monthly is granted; 60 days for two one-time orders, 30 after the full refund (the partial one keeps its days, logged in `lsmod.py roles log`); 401; replay ignored; the older event ignored |
| VIP in game | A linked supporter joins the official server; the Admins page's Supporters switches; reserved slot 1 with bots filling the server | [VIP] tag and scoreboard mark, the join message; no gear or money; a non-supporter in the kept slot is kicked unless a bot makes room; a host server with Honor off shows nothing, with it on the supporter within 5 minutes |
| Application, owner (**still needs user**: Apply › Server moderator → approve → invite → redeem in the app → revoke) | Apply › Server moderator for a claimed server (not linked first; then linked); the owner approves by DM; Create moderator invite; accept it | "Link your SteamID first"; the card reaches the owner's DMs (unverified link marked); approve DMs the applicant; the invite comes by DM; accepting gives app access as Moderator, the in-game Moderator role and the Discord role Server Moderator |
| Application, Discord moderators | With Let Discord moderators decide on, a moderator denies with a reason from the team channel; without it, they try | Denied with the reason DMed and the owner's copy edited; without the switch: "You can't decide…"; nobody decides on their own |
| Admin invites | A shared-control Admin (local and Linux server) makes the invite and revokes | Works on their server only; they never see the claim or the switches; on the Linux server it works with the owner's computer off |
| Delegation | Turn on "Let Discord moderators decide"; add a shared-control Admin; each handles one application end to end (invite, redeem, revoke) | Admins act only for their server; Discord moderators decide from the team channel; the log names who did it |
| DMs closed, revoke, unclaim | An applicant with DMs closed; Revoke an active moderator; Unclaim | The invite shows to copy; revoke takes app access, the in-game role and the Discord role; unclaim does the same for every moderator and the claim leaves the Linux server |
| Log | `lsmod.py roles log` | Every claim, link, decision, invite, redemption, revoke and eviction is there |

### Offline
| Item | Steps | Pass when |
|---|---|---|
| Box/R2 down | Block list.lonerstrike.com and dl.lonerstrike.com (hosts file) | The list shows "from earlier" with the kept index; with no kept copy, GameBanana's tag search alone (no badges, modes can't be installed, content can); registering waits and retries |
| GameBanana down | Block gamebanana.com | A plain message, the Installed tab still works, nothing hangs the page |


### Area G self-check (MCP, 2026-11-14, before QA starts; Rust and list service tests)
Run on `tools` at c0266b76. Nothing touched the live list, GameBanana or Discord; those rows are **needs user**.
| Item | Verified here | Evidence | Left for QA / the user |
|---|---|---|---|
| Sharing: maps, modes, prefabs, caps (local) | partly | `tools::community` and `tools::communitylib` (41 pass: builds_a_package_the_installer_reads, mode_packages_carry_one_good_graph, builds_a_mode_package_with_credit, nothing_that_runs_code_is_shared, package_refusals, map_names, uninstalling_a_prefab_removes_only_its_files…) | each row by hand in the app (a real map with custom content, the 1.1 GB case) |
| Requirements (local) | partly | `tools::requirements` (3 pass), the MCP `studio_requirements` tools in the live MCP run's tool list | the panel once X mounts it in My mode › Details; Publish warnings by hand |
| Playtest screenshots, cover card (local) | partly | `tools::playshots` (2 pass: snapshot names, recent snapshots become PNGs) | F5 in a playtest on the current build; the cover card by eye, offline |
| Risky nodes | yes (tests) | a_mode_that_runs_server_commands_shows_it_and_asks_first, risk_words_follow_the_studio, the_install_is_bound_to_what_the_player_saw | the badges and the ask in the app; an older entry asked after the download |
| Index formats, checksum, offline fallback | yes (tests) | index_must_match_its_checksum, index_parsing_refuses_junk_and_newer_formats, unknown_values_from_a_newer_index_keep_the_list, index_falls_back_to_the_kept_copy_then_to_nothing | blocking the box and GameBanana with the hosts file |
| Malicious archives | yes (tests) | `archive::tests` (22 pass: traversal_rules, links_are_never_followed_or_kept, tar_with_link_is_refused, zip_budget_and_traversal, zip_with_too_many_files_is_refused, bomb_ratio_only_stops_bombs, watchdog_stops_runaway_unpacker, nested_failure_leaves_nothing) | the test zips through Get more's Add file |
| Name clash, re-import, game open | partly | mode_clashes_and_remix_ids, previous_version_kept_and_put_back, busy_servers_and_maps_in_use, waiting_updates_and_stale_servers_in_the_library | by hand with a played map |
| Unlisted / withdrawn | yes (tests) | unlisted_and_withdrawn_stay_out_of_lists_and_get_no_updates; list service `test_community.py` 118 pass (unlist code proof, recheck of unlisted entries, withheld fails closed, private auto-relist, deleted held for review) | **needs user**: every live row on a test GameBanana page |
| List service | yes (tests) | test_community 118, test_caps 3, test_modbot 15 pass; test_publish_check OK | **needs user**: deploying e4b55d53 |
| Registering, updating, reports, ownership | partly | trust_rules, uids, remembers_one_registration_per_page, reports_carry_only_the_page_and_the_reason, a_registration_says_only_which_page | **needs user**: live |

## H. Windows (real Windows 10 and 11; release build; docs/notes/Windows.md)
Two PCs if possible: one with a Unicode user name (e.g. `Zoë`) and Documents redirected to OneDrive, one plain. A
Steam library on a second drive. Defender on. Note every prompt, window and message word for word.
| Item | Steps | Pass when |
|---|---|---|
| Install and first run | Run the NSIS setup.exe; open the app; Set it up | No console windows flash; the game is found (also in the second-drive library); setup ends without "os error" |
| Old libraryfolders.vdf | On a PC whose Steam library list is the old format (or edit a copy), Settings › rescan | The game in that library is found |
| Game started during setup | Start setup, start Counter-Strike while it downloads | A plain "close Counter-Strike" message, not "os error 32"; Repair afterwards finishes |
| Read-only config | Mark cstrike\config.cfg and liblist.gam read-only; Repair | Repair finishes; the files are written |
| Ham Sandwich | Replace the module with the official hamsandwich_amxx.dll; press Play with the game closed | Ours is back (hash), `.replaced-<date>` beside it, the quiet toast; with the game open: the "next time" note, nothing broken |
| Browser keys | In the editor (a view, a property field, fly mode), the Studio (Monaco), Plugin Studio, Spray: press F5, Ctrl+R, Ctrl+P, Ctrl+F, F3, F7 | No reload, print dialog, find bar or caret prompt; the editor's own F5 (Playtest) and Ctrl+F (Find) still work |
| Explorer drops | Drag a .map, .bsp and .png from Explorer onto the editor, the sidebar and the Studio | Nothing happens; the app stays |
| Right-click | On a sidebar item, an editor panel, empty space; in a text field; on selected text | No Back/Refresh/Print menu outside text; cut/copy/paste in fields and selections |
| Clipboard | Copy brush text from Notepad, Ctrl+V in the editor; same in the Studio | Pastes (note any permission prompt) |
| Alt in the editor | Tap Alt alone then Q; Alt+1; Alt+right-click a face (Texture tool); Alt-drag a texture | No ding, no stuck menu mode, the action happens |
| Ctrl+1-9 | In the editor and the Studio | Note what it does (bookmarks vs sections) |
| Offline fonts | Unplug the network, start the app | Labels aren't cut off |
| Big folders out of Roaming | Before updating, an older version with voice on, a server made (steamcmd), a backup and a compiled map; note the size of %APPDATA%\Loner-Strike; update and open the app; then, on a PC whose AppData is on another drive or redirected, the same | `backups`, `cache`, `control-log`, `editor\backups`, `smarter-chat`, `steamcmd`, `voice` and `tools\compile`, `tools\recordings`, `tools\sdhlt`, `tools\community\packages`, `tools\wad-backups` are in %LOCALAPPDATA%\gg.lonerstrike.app\data; settings and records stay in %APPDATA%\Loner-Strike; backups restore, voice works without downloading again, the map is still listed as made here; quitting during the copy and opening again finishes it with nothing lost |
| Local server | Make a server, start it | Note the firewall prompt (Private and Public ticked?); a friend on the LAN can join; no console window; the log fills |
| Second server, same port taken | Start another program on 27015 first | A plain "port in use" message, no restart loop |
| Crash dialog | Crash hlds (bad metamod DLL) | Time until the app restarts it; does a dialog cover a full-screen game? |
| Keep servers running | Keep running on, close the app, open it again | The server kept running and is picked up |
| Voice | Voice on for a local server, talk in game, stop the server | ls-voice.exe and ls-intent.exe are gone in Task Manager |
| MCP stdio | `'{"jsonrpc":"2.0","id":1,"method":"ping"}' \| & "$env:LOCALAPPDATA\Loner-Strike\loner-strike.exe" --mcp-stdio` with agents on; then Settings › AI agents › Copy for Claude Code and run it in PowerShell | One JSON line back; `claude mcp list` shows loner-strike Connected |
| Open in Loner-Strike (P5) | After install, on lonerstrike.com › Made by players press Open in Loner-Strike (or Win+R `loner-strike://community/mod/<a listed GameBanana id>`), once with the app closed and once open; then `loner-strike://community/mod/5?install=1` and `loner-strike://settings/mod/5` | The browser asks to open Loner-Strike; the app opens (or comes to the front) on that creation's page in Community, nothing installs; the two bad links do nothing; one app window only. Note the browser's prompt words. After uninstall: the link no longer opens anything |
| Update | One server with Keep running, one without, voice and an MCP bridge on; update in passive mode | Which processes survive; voice comes back; the agent reconnects; latest.json's windows entry is the setup.exe |
| Remove everything | With the window open | Only EBWebView is left for the uninstaller; no failure list otherwise |
| Uninstall | With "Delete app data" ticked | List what's left in %APPDATA%, %LOCALAPPDATA%, firewall rules, the game folder, %SystemDrive%\Loner-Strike, %USERPROFILE%\.loner-strike |
| Spray fonts | Open the spray font picker the first time; a per-user installed font | Time it; the per-user font loads |
| Pointer lock | Unreal preset right-drag; Hammer preset Z; Esc | Esc releases and the camera stops looking around |
| OneDrive Documents | Save a project into Documents (OneDrive, files on demand) | Saves and reopens; no lock errors in the diagnostics |

### H2. Update 0.1.2 → 1.0.0 on a real Windows PC (P5, 2026-11-14)
Before the release is published, with the 1.0.0 build from a manual Release run (Actions artifacts, no release made).
Use a Windows user account made for the test (a fresh profile), not someone's real one.
1. **Install 0.1.2 from the NSIS setup** (`Loner-Strike_0.1.2_x64-setup.exe` from the v0.1.2 release, not the .msi).
   Open it, let setup run on the PC's game, start a game once, make a local server ("Friday Test", a join password,
   20 players), add an admin, turn Keep servers running on, and change two settings (Settings: updates off, phone off).
   Note `%APPDATA%\Loner-Strike` (settings.json, servers.json, record.json, backups\), `%LOCALAPPDATA%\Loner-Strike\Servers`
   and the game's `addons\amxmodx\modules\hamsandwich_amxx.dll` hash. Copy the whole `%APPDATA%\Loner-Strike` aside.
2. **Point 0.1.2 at the test feed:** 0.1.2 only asks `github.com/.../releases/latest/download/latest.json`, so the real
   path can only be tried once 1.0.0 is published. Before that, check the pieces: (a) the run's NSIS setup and
   `.sig` verify against the app's pubkey (`scratchpad rel/verify_sig.py`-style check, or `tauri signer verify`);
   (b) the run's latest.json (or the one tauri-action would write) has `windows-x86_64` and `windows-x86_64-nsis`
   pointing at the `_x64-setup.exe`; 0.1.2 NSIS installs ask for `windows-x86_64-nsis` first.
3. **The update itself** (once published, or run the 1.0.0 setup by hand with `/UPDATE`-like passive install over
   0.1.2: `Loner-Strike_1.0.0_x64-setup.exe /P /UPDATE`): with a server running (Keep running on) and the game closed,
   in 0.1.2 Settings › Check now › Install. Pass when: a passive installer window, no UAC prompt (per-user install),
   the app comes back as 1.0.0 in the same install folder (one entry in Apps & features, version 1.0.0), ls-voice /
   playitd stopped and replaced (no "file in use"), the kept server still running or picked up.
4. **First start of 1.0.0:** pass when `%APPDATA%\Loner-Strike\settings.json` and `servers.json` still hold the step 1
   values (diff against the copy: only added fields), the big folders (backups, cache, steamcmd, voice, tools\compile...)
   moved to `%LOCALAPPDATA%\gg.lonerstrike.app\data` once (Roaming copies gone, nothing in `data\from-roaming` unless
   a file differed), record.json read (Setup shows the game as set up, no reinstall asked), Ham Sandwich: after the
   next Play the module hash is ours and any other copy sits beside it as `.replaced-<date>`, the server, its password,
   admins and settings are as before, Settings › AI agents and the new pages open. Diagnostics: no new errors.
5. **0.1.2 installed from the .msi** (2 downloads so far): its updater falls back to `windows-x86_64` (now the NSIS
   setup), which installs per user beside the per-machine MSI copy. Check what happens; expected: two entries in Apps
   & features. If so, the release notes' known issues say to uninstall the 0.1.2 MSI copy after updating.
6. Mac (done on a throwaway profile, 2026-11-14): 0.1.2's universal app made a profile (settings, an imported server
   with map/maxplayers, prefs), the 1.0.0 bundle replaced it in place and started on the same profile: version 1.0.0,
   settings.json and servers.json unchanged byte for byte, prefs kept (updates off, keep servers on), server listed.

## Live test harness (community roles, the live list; user-approved overnight tests)
`services/serverlist/qa/live.py` (run on the Mac; never prints a token; everything it makes is named QA-/qa- and
recorded in `~/.config/lonerstrike/qa-live.json`, mode 600):
- `discord-setup`: private category **QA-tests** and channel **#qa-live** (only the bot and the owner see them) and the
  role **QA-Tester**. Done on 2026-11-14 (channel 1571131443671728168).
- `discord-check [--target <discord id>]`: the bot posts, edits and deletes in #qa-live and gives QA-Tester to the target
  and takes it back (the same calls the roles service makes for Supporter and Server Moderator). Passed 2026-11-14.
- `community-add mod:<id> --file <file id>`: registers a test page on the live list the way the app does, then
  `community-check`. **Needs a real GameBanana page** (private/WIP, the LonerStrike tag, a package file): agents can't
  upload to GameBanana, so the user makes it and gives the ids.
- `link-add <steamid> <discord id>`: records a test link for cleanup (links are made in game with /link).
- `cleanup`: removes every QA- channel and role, pulls and deletes the recorded test entries, unlinks the recorded links;
  then `verify-clean` (exit 1 and a list when anything is left). Needs the admin tunnel (`ssh -f -N -L
  8100:127.0.0.1:8100 cs@<box>`) and ssh to the box.
- The roles service from a terminal (admin tunnel): `lsmod.py roles servers | log | show <id> | supporters | unlink |
  supporter-remove | supporter-restore`; the server side with Alone Together's token
  (`~/Library/Application Support/Loner-Strike/secrets/roles-loner-strike-alone-together.token`): `GET /roles/server`,
  `/roles/vip`, `/roles/applications`, `POST /roles/server` (settings).

**Rows that need a human** (a bot can't invoke slash commands, press buttons or play): `/link` in game and
`/link code:` in Discord; `/server claim`; Apply › Server moderator (the button, the server pick and the form);
Approve/Deny on a card; accepting an invite in the app; anything said in voice. Everything else (role changes, the
service's answers, the supporter list, users.ini on Alone Together, cleanup) runs unassisted. Alone Together must be
left running and healthy (check `systemctl is-active lonerstrike-loner-strike-alone-together*` at the end).

## Round 3 items to re-check
From QA.md "Round 3": the brain drop crash (30 rounds of drop/drop_all/drop_dead), Last Stand bots buying guns, the
BR start loadout and drop-in, and the Studio's gm_components loading for templates.
