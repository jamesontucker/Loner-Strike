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

Wave 1 runs A, B, C and F side by side (5 sims at most, 6 while a Test run is going). Wave 2 starts D and E when A and
F have stopped their sims (3 more). The real client is needed in C, D and E (one at a time: the user's game).

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

## B. Game Mode Studio (X, X2)
| Item | Pass when |
|---|---|
| Compile button (c0c650c5, b2865b91) | Real amxxpc without installing; a compiler error lands on its node; status icon; Mod+B / F7 |
| Test run (159181be) | On a good example mode: pass with cost against the baseline and every node ran; on a mode with a deliberate run-time error: fail with the error on its node; Cancel stops the hidden server (its process gone); the person's game untouched |
| All Actions palette (5c7cb5be) | Folders and sub-folders, search opens matches, Favorites, Recently used, Context Sensitive from a dropped wire |
| Several event graphs (44dd882a) | Events on a second tab run; copy/paste across tabs keeps inner wires; two of the same event run in tab order |
| Custom events (69a594c2) | Call Custom Event with typed inputs; Set Timer by Event; an `ls_event` entity on the map fires it |
| Timelines **(when it lands)** | A looping number track drives a node every tick; Finished fires; reverse works |
| Node help (f90f5238) | F1 on any node shows its help; search finds by words; the example drops in |
| New nodes (cd541ea8, 71ca7012, 4d7cf6b1) | Each W1/W3 component node and the round-table/turret nodes does what its help says in a playtest (debug events as evidence) |
| Security (2536e0b9, b2865b91, c0c650c5) | Run Server Command refuses `exec`, `rcon_password`, `quit`, quoted first words, `amx_` lines; Run AMX Command allows only its list; Kick reasons and map names can't inject |
| Example modes (60d61837 Infection; X2's five) **(when they land)** | Each compiles, test-runs to pass and plays 3 rounds with bots on QA4S without run-time errors |
| Debugger | Breakpoints, Resume, live variables (round 3's For Each freeze stays fixed) |
| MCP Studio (d3b922bb) | An agent builds a 3-node mode with `studio_edit`, `studio_check` passes, `studio_test_run` asks first and returns pass/warn/fail |

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

## D. Battle Royale (W3, W1, W2)
br_valley v6; squads on QA4B (16 bots), duos/solo on QA4B2.
| Item | Pass when |
|---|---|
| Squads and duos (5bc0e2df, 14fb4c12, fe158c49) | br_duo and br_squads in the vote and modes.ini; the 30 s lobby (say /squad); squad mates can't hurt each other; markers and radar dots |
| Knocked down in squads (c5ef3a88) | Downed until bled out or revived; out when the whole squad is down |
| Bots reviving **(as tuned by W2/W3)** | In 4 squad games, bots revive a knocked squad mate at least half the time it's safe (W3.md had 1 in 4 games) |
| Finishing a downed player **(when it lands: W1's `ls_down_finish`)** | An enemy's shots finish a downed player; the kill counts |
| Loot (5ca3ad76, 060d9a90, 6b4aecdf, 25140068, bed0141b) | Death crates hold the dead player's guns and ammo; armor levels reduce damage; healing items work with the bar; rarity glow colors |
| HUD and drop-in (1d543873, 2c89e395, 83f5f83d) | Players left, kills, zone timer, next-zone distance and radar dot; the drop-in steers and lands without damage |
| Bot layers (decb83fb) | About 3 in 10 bots go below ground, 1 in 4 high; no drop-in deaths in 10 games |

## E. Deathrun maps and the mode vote
| Item | Pass when |
|---|---|
| The Deathrun maps that ship (Deathrun.md; sims DR1-DR3 were the agent's) | Each loads, bots run the course and reach the end, traps fire when the terrorist presses them; no crash in 10 rounds |
| Mode vote (83fad7c8) | Battle Royale, Duos and Squads appear in the app's and gm_pubserver's lists, matching votemodes.ini; a vote switches the mode |
| Queued after round 4 (fun_allinone, vehicles) | Not tested: queued for after this round (Deathrun.md) |

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
| Fresh setup and update | A clean bottle: setup end to end, then an update; the fixed Ham Sandwich is installed on Windows/CrossOver |

## Round 3 items to re-check
From QA.md "Round 3": the brain drop crash (30 rounds of drop/drop_all/drop_dead), Last Stand bots buying guns, the
BR start loadout and drop-in, and the Studio's gm_components loading for templates.
