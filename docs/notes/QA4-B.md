# QA round 4, area B: Game Mode Studio (fresh QA agent, 2026-11-14)

A fresh tester who didn't build the Studio. Every area B row in docs/notes/QA4.md, re-checked independently of the
Parity A self-check, by hand in the page, through the Studio's routes, and in game on sims.

**Counts: 30 rows: PASS 24, FAIL 4, NEEDS USER 2.** Findings F1-F6 below: F1, F2, F4, F5 fail their rows; F3 is a
test-harness safety problem found during row 2; F6 lists low items inside rows that otherwise pass.

## How I tested
- **Code:** `tools` HEAD **314c7f47**, a clean `git archive` snapshot (UI built from it; Rust test binary built from it
  into the shared target, `-j 6`, `nice 15`). Plugins for the sim compiled fresh from the same snapshot
  (`plugins-fresh.sh`: every gm_* and Loner-Strike plugin; only third-party zp50_* failed on missing includes, as
  before).
- **Brain:** newest green ls-next3: `git ls-remote` head **285b895** = CI run **43604814272** (success). Its win32
  mp.dll in sim **QA4S** (an APFS clone of LSPLAY), checked inside: `ls_creature_flee_last`, `ls_creature_ver`,
  `ls_perf` present; the server log says ReGameDLL 5.30.0.1289-dev.
- **Servers:** a Tools dev server on 8241 over QA4S (Compile, Install, Playtest, debugger, mode settings, Requirements,
  publish), one on 8242 for Test run (real game read-only, `LS_TEST_RUN_SOURCE`=QA4S, hidden server in
  `Sim/QA4SRV`; see F3 for the one-line harness patch this needed), a short-lived one on 8243 over a second clone
  (QA4SB) for the For Each breakpoint and mode-reload checks. Vite on 5341 with its own cache dir. Scratch app folders
  only (`LS_APP_DIR`). Hand-started sims with `-insecure`, 8 bots, at most 2 of mine at a time.
- **Browsers:** Chromium and WebKit (Playwright), 1440x900, plus 1024x680. The Studio specs from the snapshot in both.
  Hands-on in Chromium; WebKit through the specs and my own perf script.
- **Evidence:** console prints from test graphs (`[qa_game] ... QA ...`), Studio debug events, route answers,
  screenshots in `~/.playwright-mcp/qa4b/`, sim logs (rcon masked) in `~/loner-strike-sim/results/qa4b-qa/`, and in
  my scratchpad `.../scratchpad/qa4b/` (`qconsole_run3.log`, `qconsole_run5.log`, `testruns.log`, `testruns2.log`,
  `security_static.txt`, `foreach_bp.log`, `events_qa_game.jsonl`, the test graphs and scripts in `bin/`).
- **Test graphs:** `qa_game` (133 nodes: a number mode setting; a 15-row data table with commands in a text column; a
  4 s Timeline with one key of every curve type and three event keys; a looping 1 s Timeline; custom events with
  typed inputs; Set Timer by Event; three map-fired custom events; a second event graph; creatures; bot nodes; Run
  Server/AMX Command fed at run time), `qa_comp` (W1/W3 component nodes on ls_outpost), `qa_ui` (hands-on editing),
  `qa_big` (399 nodes, 760 wires, one tab), `qa_cerr` (compiler error), `qa_req` (missing content), `qa_err`
  (run-time error attempts), 66 static security cases, the five example modes. A copy of de_dust2 with ls_event
  entities (`qa4b_ev`: a trigger_auto, two trigger_multiples on the buyzones' brushes, an `ls_event` with a/b values,
  an info_target `lsev_qa_named`).
- **Environment note:** between 08:14 and 08:29 the CrossOver bottle stalled three times (every sim's log stopped at
  the same second, the wineserver was restarted, a `winedbg --auto 208` from `crashlog.bat` sat attached from 08:23;
  crashes.log `==== crash: pid 208 at 8:23`), and once more at 09:26:35 (QA4S, QA4DE1, QA4DE2 stopped in the same
  second; the wineserver restarted). Not traced to area B; I restarted my own sim each time and re-ran what it cut.

---

## FAILs (most important first)

### F1. A map-fired Custom Event's Player input is always "nobody" (ls_event / lsev_ entities)
Owner: **X** (gen.rs `custom_events_block`, `lsg_ev_use`).
Repro:
1. A mode with a Custom Event "QA Zone", **The map can fire it** ticked, inputs Who (Player) and A (Number); its
   event node → Print String with Name Of(Who).
2. A map with a `trigger_multiple` (here on the CT buyzone's brush) whose target is an `ls_event` with
   `event` = `QA_Zone`, `a` = 3. Play it with bots.
3. Every fire prints `QA map zone who nobody a 3` (17 times in one run as bots walked through;
   `qconsole_run5.log`). The values arrive (`qa_auto a 7 b hello` from a trigger_auto too); only the player is lost.
Likely cause: the hook is `public lsg_ev_use(ent, caller, activator)` on `Ham_Use` and reads the **third** parameter
as the player. HamSandwich hands over CBaseEntity::Use's arguments in C++ order (pActivator first), so the player is
the **second** parameter (the one ham_const.inc calls `idcaller`); the third is the trigger. A func_button pressed
directly passes the player in both, so a button-only test hides this; a trigger chain doesn't. Help and docs say
"Player is whoever set it off".

### F2. Requirements calls Loner-Strike's own maps "Only on this computer" (ls_outpost, br_valley)
Owner: **X / T** (tools/requirements.rs `map_source`; the publish warning).
Repro: Studio › a mode › Mode settings › Requirements › Default map lists `ls_outpost · Only on this computer` and
`br_valley · Only on this computer`; pick ls_outpost: `GET /api/tools/requirements/<id>` answers `"source": "local",
"warning": "Share this map first so players can get it."`; Community › Publish › A game mode shows "! Requirements:
Only on this computer (share them first so players can get them): ls_outpost, br_valley". Both maps ship with
Loner-Strike (`plugins/game/maps/`, copied into every player's game by setup), so every player has them;
`map_source` only knows the stock CS maps (`maps::is_stock_file`). Last Stand and Battle Royale modes are made for
exactly these maps, so their authors are told to "share" maps they can't and needn't share. (The self-check took
this warning as correct.)

### F3. On committed HEAD, a dev-server Test run writes into the real app's `Loner-Strike/Servers/.studio-test`
Owner: whoever owns the **uncommitted** `LS_SERVERS_DIR` hunk in `app/src-tauri/src/servers.rs` (working tree, +57
lines); X / X2 for the docs (X2.md "How I test" and the QA4 self-check rely on `LS_SERVERS_DIR`).
QA4.md and X2.md say Test run's hidden server goes to `Sim/<folder>` with `LS_SERVERS_DIR`. That env var exists only
in the working tree; HEAD's `servers::root_for` ignores it. My first Test runs (Infection, Gun Game Lite and the start
of King of the Hill, 08:07-08:21) therefore synced QA4S into the **real app's** hidden test server
`drive_c/Loner-Strike/Servers/.studio-test` (10 files there changed, its mark is now `test-source`). The real game
folder: 0 files changed (`find -newer`, checked three times). It heals itself: the real app's next Test run sees a
different mark and makes a fresh copy. I deleted nothing there. Then I patched only my snapshot with the same 5-line
`root_for` hunk (a test seam, nothing else) and every later Test run went to `Sim/QA4SRV/.studio-test`.
Fix: commit the `LS_SERVERS_DIR` hunk, or make Test run under `cfg(test)` refuse to use the real servers folder (as
`guard_real_game` does for the game).

### F4. Studio specs: 8 fail in the small (1024x680) project, the library spec fails in every project
Owner: **X** (tests only; the product behaves as designed).
- Small project, run twice: 8 fail. studio-ux.spec.ts (H2, M5/M7/L9, M4/L4/L5/L8, M8/M9, H6) wait for the "This
  mode" heading, but under 1200 px Details starts folded (UX review H4, by design) and studio-ux.spec doesn't open the
  panels the way studio-v3.spec's `beforeEach` does. studio-v3 "address opens the tab" and "several event graphs"
  time out on nodes React Flow culls in the tiny canvas. By hand at 1024x680 the "Details" rail opens it and "This
  mode" shows (`32-small-unfolded.png`), so it's the specs.
- studio-v3 "library: Export all, Import" fails in large, small and WebKit: the spec deletes its items through the
  API, then imports; the page's `library` store still lists the deleted ids, so `itemId(...)` makes
  `pw_one_<tag>_2`. A person deletes in the page (the list reloads), but an agent deleting through MCP while the Studio
  is open gets the same `_2` names. Reload the library before importing, or have the spec reload the page.
- Otherwise: Chromium large 39/40, small 32/40, WebKit 39/40; Studio UI unit tests 46/46; Rust (Studio, MCP,
  requirements, playshots, security) 124/124.

### F5. Data tables: a bad CSV value goes in as 0 without a word
Owner: **X2 / X** (tables.ts `importCsv` / `readCell`).
Repro: a new table (Key text, Value number) › Paste or import CSV › `Key,Value` / `1,10` / `2,abc` / `3,30` /
`4,"x""y"` / `5,50` › Import. Toast: "Added 5 rows (the first row was the column names)." Rows: `2,0` and `4,0`.
QA4's row says "a bad CSV row is refused with a reason". (Quotes in a text cell are caught by the quick check with a
reason, "Row 4, Cmd of “Guns”: text of up to 48 letters...".)

### F6. Small things (low)
1. **Mode settings doesn't show the mode while a Timeline tab is open** (X). Open a Timeline (double-click it in My
   mode), press **Mode settings**: the button shows pressed, Details still says "DETAILS · TIMELINE". From an Event
   Graph tab it works.
2. **Timeline "Tick 0.05 s (default)" runs at 0.1 s** (X2). The plugin calls `set_task(0.05, ...)`; AMX Mod X doesn't
   run tasks faster than 0.1 s. Measured: Update every 0.10-0.12 s of game time; a looping Timeline's Update ran 46-47
   times per 5 s (9.3 a second, not 20). Timing stays right (each tick steps by game time), but the setting promises
   twice the rate. Make 0.1 s the fastest, or say so.
3. **One in the Chamber ends a round twice** when a kill lands in the round-end delay (P5 / X): time up at 08:55:04
   (Round_Draw, Round_End), kevin46 knifes Takeshi M. at 08:55:05, a second Round_Draw + Round_End in the same second
   (`console-one_in_the_chamber.log`). The next round still started normally 11 s later.
4. **Creatures Working says "no" before anything is known** (W1, gm_components `ls_wave_creatures_ok`): read at Round
   Started on ls_outpost it printed `creatures ok no`; Wave 1 then spawned 21 creatures. Help: "False once creatures
   can't spawn".
5. **Multi-select is Cmd-click only** (X). Shift-click and Ctrl-click on a node don't add to the selection on a Mac;
   the "?" sheet doesn't say which key does.
6. **Node help search by words** (X2): "move a player somewhere" finds nothing; "how do I end the round" lists Round
   Ended but not End Round. Single words work well.
7. **Timeline lane clips an overshooting Custom curve** (X2): a Custom key with tout 400 overshoots above the lane and
   the drawn curve is cut off at its top (3.1-3.5 s in `16-timeline.png`); the lane's range doesn't grow to fit.
8. **Publish form fields unstyled** (T): Community › Publish's Name / Made by / About are plain white browser fields,
   About in a monospace font (`22-publish-filled.png`).

---

## All rows

| # | Row | Result | Evidence |
|---|---|---|---|
| 1 | Compile button | **PASS** | Real amxxpc on the five examples ("Done.", 1.8-4.3 s, cached after); Compile (`/check`) installs nothing (no `studio_infection_tour.amxx` after compiling the tour). **A real compiler error lands on its node**: with a one-line `#define set_cvar_float(%1) qa4b_nope(%1)` added to the sim's hamsandwich.inc (restored right after), Compile on `qa_cerr` showed `The compiler said (line 1138): undefined symbol "qa4b_nope"` on the Set Setting node, the status icon a red X, the toast "The compiler found errors" (`04-compiler-error.png`). F7, Cmd+B and Ctrl+B each ran a full check. No graph by itself makes amxxpc fail: bad texts, `%s`, `^`, quotes, emoji, 400-char texts, reserved names (`new`, `plugin_init`, pins `public`/`return`), 390 chained nodes and 40 nested loops are all caught by the quick check or compile clean. |
| 2 | Test run | **PASS** (harness: F3) | All five examples **pass** (Infection 31/41 nodes ran, Gun Game Lite 22/31, King of the Hill 23/30, One in the Chamber 36/57, Juggernaut 32/40; 2:10-2:30 each), `qa_game` passes (60/75). **Cancel** at "Playing de_dust2": stopped in 2.1 s, the hidden hlds and its wrapper gone. The real game: 0 files newer than my start mark. A run-time error couldn't be made: recursion is refused ("This Function ends up calling itself"), loops are capped, every text is checked, so "fail with the error on its node" isn't reachable from a graph (as in the self-check). |
| 3 | All Actions palette | **PASS** | Right-click: 36 folders with counts; Players opens Values (30) / Actions (42); "killed" ranks Player Killed first, Enter places and picks it; star → Favorites at the top, Recently used lists Player Killed; a wire dragged from Attacker to empty space opens Context Sensitive (on, 206 fitting), "give weapon" + Enter places Give Weapon wired to Attacker. |
| 4 | Several event graphs | **PASS** | Sim, three runs: `QA rs ...` (tab 1), then `QA tab1 second round started`, then `QA tab2 round started`: tab order, then top to bottom. Copy/paste across tabs keeping inner wires: the studio-v3 spec passes in Chromium and WebKit. |
| 5 | Custom events, Set Timer by Event, ls_event | **FAIL** (F1) | Call Custom Event with typed inputs: `QA hello n 15 label abc who <bot>` (N = Row Count of the 15-row table). Set Timer by Event 3 s: set at 10.05 → fired 13.06 (another run 10.08 → 13.15). Map: `qa_auto a 7 b hello` (trigger_auto → ls_event with a/b), `QA map named fired` (info_target `lsev_qa_named`), `QA Zone` fires when bots walk in, but its Player is always nobody. |
| 6 | Timelines | **PASS** (F6.2, F6.7) | 4 s Timeline: Play 5.06 → Finished 9.16 (4.10 s); Reverse from end 10.26 → Finished 14.27 (4.01 s); other runs 4.08 / 4.10 s, all within 0.3 s. Event keys at 1, 2, 3 s once each, forward and back. Curves (Update values): Linear 0→100 over 0.5 s (26.8 at 0.13), Constant holds 100 to 1.0, Ease In 4.5 at 1.10 / 39 at 1.31, Ease Out 84 at 1.54, Ease In-Out 2 at 2.08 / 60 at 2.56 / 97 at 2.90, Custom tangent overshoots to 132 at 3.10 (as drawn). A looping 1 s track drove Add To Variable every tick, values wrapping 0-10. The debugger shows a Timeline's progress (`0.56 of 1.00 s, forward; LV: 5.65`). |
| 7 | Node help, the tour | **PASS** (F6.6) | F1 on Give Weapon: its help (pins, settings), focus inside, Escape closes; "Add the example" drops Player Spawned → Give Weapon in a comment box. Tour: 11 steps, each highlighting its real control; it saves nothing. |
| 8 | Variables (Ctrl/Alt drag), functions, macros | **PASS** | New variable: name field focused and selected. Cmd-drag → a Get pill, Alt-drag → Set, plain drop → Get / Set / Cancel (arrows + Enter place, Escape closes). Add a Function / a Macro: tabs with Entry/Return, In/Out; dragged onto the Event Graph they become Call / Macro nodes. |
| 9 | Editing helpers (inline pin values, Promote, bookmarks, quick-add, align) | **PASS** (F6.5) | Inline values: Spare bullets 90 → 31 (Enter) = Details 31; 77 + Escape stays 31; 9999 → 250 (its range, tooltip "0 to 250"); -5 → 0; Details 12 → inline 12; the weapon dropdown matches Details; **each change one undo step** (undos walk 12 → 0 → 250 → 31 → 90). Promote to Variable on Give Money's Amount: variable "Amount", its Get placed clear of other nodes, one undo removes both. Bookmarks Cmd+Shift+1 / Cmd+1 and Ctrl+Shift+2 / Ctrl+2 return to the exact view; View lists them. Quick-add B / S / D + click → Branch / Sequence / Delay. Align top edges and Space evenly across, one undo each. |
| 10 | Versions and Restore | **PASS** | The list shows saves with counts ("8 nodes · 1 removed"); the oldest compares ("Since then: 3 added · 0 changed · 1 removed (Chat Message)") with nodes marked; Restore, then **one undo** returns the newer graph. |
| 11 | Delete dialog | **PASS** | A variable used by a node: "used by 1 node... Find uses", Replace with (none fits, said so), Remove it and its nodes, Keep the nodes; Remove with nodes, then one undo brings both back. |
| 12 | Data tables | **FAIL** (F5) | In game: For Each Row printed all 15 rows in order; Get Row key 2 → `found yes row 2 gun awp`; Find Rows gun = ak47 → 1; Row Count 15. Gun Game Lite gives the table's guns in order (kills per bot walk glock18 → usp → deagle → ... → awp, index 0..13). But a bad CSV value imports as 0 silently. |
| 13 | New nodes (W1/W3 components, round table, turrets) | **PASS** (F6.4; turret placement not seen) | `qa_comp` on ls_outpost (QA4SB): gm_components found 19 components; Round Table On + Start Wave → `Wave 1: 21 creatures against 4 defenders`, Wave Started `1`, This Round `planned 21`, Creatures Alive 20 → 16; Knocked Down On → 15 Knocked Down events with names, revives logged; Give Turret Kit → Turret Kits `2`. Turret Placed didn't fire: my graph set the turret price to 0, which means "off" for bots, my mistake. |
| 14 | Security allowlist | **PASS** | 66 static cases (`security_static.txt`): Run Server Command refuses exec (any case, leading space), rcon_password, quit/QUIT, quoted words, `;`, new lines, tabs, `%`, `^`, amx_ lines, log/file/password/map/alias/meta/amxx/condump/writecfg/kick, sv_cheats, sv_password, hostname, over-long words; allows mp_ rules, sv_gravity/sv_restart, bot_quota, say, humans_join_team. Run AMX Command allows only its list (amx_rcon, amx_ban, amx_exec, amx_plugins refused; amx_cvar only on allowed settings; amx_map only a valid name). Kick reasons with quotes/new lines and map names with `../`, `;`, spaces, new lines, 40 chars refused. **At run time**, commands from a data table and from the hostname (which the quick check can't see): `exec server.cfg`, `quit`, `rcon_password hacked`, `sv_gravity 600; quit`, `amx_cvar rcon_password x`, `mp_logfile 1`, `sv_cheats 1`, `log on`, `changelevel de_dust2`, `sv_password x`, `amx_slay @all` all `ok no`; with hostname set to `hi; quit`, `say hi; quit` and `amx_cvar hi; quit` refused; allowed ones ran (`sv_gravity 700`, `mp_freezetime 1`, `say ...`, `bot_quota 9`). The server stayed up. |
| 15 | Example modes on a sim | **PASS** (F6.3) | QA4S, de_dust2, 8 bots, release builds the Studio installed; 0 run-time errors, 0 crashes. **One in the Chamber** (11 min): rounds of 131, 181, 181 s, a 4th running; no stall (the fix holds); one double round end (F6.3). **Infection** (8 min): 4 rounds ended (all zombie wins, 66-122 s). **Gun Game Lite** (8 min): 5 rounds, each restarted by the mode (no Round_End lines: it restarts the round), guns in table order. **King of the Hill** (8 min): 2 rounds ended by the hill (Terrorists_Win at 09:17:20 and 09:20:24, no bomb), a 3rd running. **Juggernaut**: the first 8-minute play was cut by the 09:26 bottle stall after one round (148 s); re-run 09:30-09:40: 3 rounds ended at 93, 178 and 118 s (20 points), a 4th running, 334 kills. |
| 16 | Playtest + debugger | **PASS** | Playtest from the page (the game's launch answered in the page; the sim stood in for the game): the steps, then the Playtest panel (Playing, Resume, Stop debugging, What happened). F9 on a node: `world_paused` at it, the round clock held (timeLeft 285 throughout), Live values listed the variables, a **live tweak** Loop ticks 696 → 5000 took at once (next prints 5000, 5019, 5067), Resume: `world_resumed paused 3.0`. Events stream session, node_enter with inputs, node_exit with results, sampled (`"skipped"`). |
| 17 | Debugger: breakpoint inside For Each (round 3's freeze) | **PASS** | QA4SB: a breakpoint on the Print inside For Each Row paused per row (4 pauses, each `world_paused` → Resume → `world_resumed paused 2.0`), then all off + Resume: the remaining rows ran (15 of 15 printed), rcon answered (`foreach_bp.log`). |
| 18 | Mode settings (local) | **PASS** | `qa_game`'s number setting (5-120, default 30) set to 7 → the next round printed `hold 7`. "abc" → "Seconds to hold takes a number."; `sv_cheats` → "This mode has no setting called sv_cheats."; 500 kept with "outside what the mode's author suggests (5-120)"; Reset one → 30, Reset all → 30, the file left with only its header. |
| 19 | Mode settings (remote, Alone Together) | **NEEDS USER** | Reaching Alone Together "through the app's path" needs the app's server registration and SSH key in the real app data, which QA rules say not to touch; a dev server with a scratch app folder has no remote servers. X did it on 2026-11-14 with the user's go-ahead (X.md). I didn't touch Alone Together. |
| 20 | Requirements | **FAIL** (F2) | Creatures (zombie) and the brain found from the graph; a missing model and sound listed (`have: false`); Publish with a model found nowhere is stopped ("models/qa_custom_box.mdl isn't on this computer and isn't linked... Link it in My mode › Details › Requirements", Make the package disabled, no package). But Loner-Strike's own maps are called "only on this computer". |
| 21 | Playtest screenshots | **PASS** | Three `de_dust2000N.bmp` in the sim's game folder (what F5 writes) show under "From your playtests" as PNG with their map; pick and unpick work; the package folder holds the cover plus the three (`qa_game-1..4.png`). Pressing F5 in the real client is row 30. |
| 22 | Cover card | **PASS** | 1280x720: name, initials emblem (RM), tags (Creatures, 4 maps), the line, "Made with Loner-Strike"; redrawn after a name change; drawn with every non-localhost request blocked (fonts are local); first in the package. The package has the graph and kit only (no .sma/.amxx), capabilities server_commands and amx_commands. |
| 23 | Creature nodes in game | **PASS** | A zombie spawned at a bot: `state 7 attack target entity 1/4/5/8`, `5 chase`; Go To → `4 guard target 0`; Resume AI → back to `7 attack` / `8 search`; Set Speed 2.5 and Set Tier 3 ran without errors. (Creature Idle didn't come up: never left alone 12 s.) |
| 24 | Bot nodes in game | **PASS** | Bot Nature: `Lancelot skill 100 aggr 54.63 team 25.32 style careful bold no`, `pesto_sux skill 85 aggr 75.31 style knifer bold yes`, `celery_tv skill 100 aggr 86.31 style rusher bold yes` (style and Bold agree); Bots: Goal "Hunt it" on the creature ran without errors. How close hunters stay and influence 0 vs 100 I didn't measure (X did). |
| 25 | MCP Studio | **PASS** (tests) | From the snapshot: `mcp::` + Studio Rust tests 124 passed, including `an_agent_builds_checks_compiles_a_map_and_makes_a_mode`, `compiling_and_playtest`, `risky_actions_ask_the_person_first`, `a_risky_mode_install_names_its_risks_in_the_question`. `studio_test_run` against the built app is area F's live MCP row. |
| 26 | Mode list on a running server | **PASS** (plugin side) | QA4SB running: `gm_mode qa_new` → "Unknown mode"; Install `qa_new`; `gm_modes_reload` → "Modes reloaded: 34"; `gm_mode qa_new` → "Switching to QA New on de_dust2", `amxx plugins` lists it running; Remove from the game; reload → 33, `gm_mode qa_new` → "Unknown mode". The app sends the reload itself only to a server it knows is running (a registered server or the game); my hand-started sim isn't one, so the route said `modes: off`. 60 modes in modes.ini not tried. |
| 27 | Performance on a big graph | **PASS** | `qa_big`, 399 nodes / 760 wires on one tab: Chromium open 0.56 s, fit 0.16 s, pan / drag-everything / drag-one frames p50 17, p95 17 ms, undo 33 ms; WebKit open 1.25 s, fit 0.48 s, pan p50 16 / p95 56 ms, drag-everything p95 19 ms, drag-one p95 19 ms, undo 32 ms. studio-perf spec (every_node): WebKit open 592 ms, undo 47 ms, drags p50 16-17 ms (p95 ≤ 53), debugger stream p95 23 ms; Chromium 17 ms throughout. The 400-node cap holds. |
| 28 | Small window | **PASS** | 1024x680: Details folds to a rail and opens; the canvas is small but usable (Focus helps). |
| 29 | Studio specs | **FAIL** (F4) | See F4. |
| 30 | In the real client: F5 screenshots, Playtest launching the game, creature speed and the tier 3 red-orange shell | **NEEDS USER** | These need eyes on the player's own game, which QA mustn't run. Everything around them passed above (the files F5 writes, the debugger against a running server, Set Speed / Set Tier running). |

Clean-up: sims QA4S, QA4SB and the hidden test server QA4SRV deleted; my dev servers and Vite stopped; the include
I changed for row 1 restored (byte-compared). crashcheck since my start: only the 08:23 pid 208 crash (the bottle
stall above, not one of my servers as far as I can tell). The real game: 0 files changed.
