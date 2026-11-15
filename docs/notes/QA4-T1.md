# QA round 4, T1: Creature Studio, Plugin Studio, Map simulator, Replay and Demos (a fresh QA tester, 2026-11-14)

## What was tested and how
- **Builds.** Brain: ls-next3 run 43629835474, head `bdf4e8d`. That is the same head as `git ls-remote`, and the newest
  green run. I checked it inside the sim: the console says "ReGameDLL version: 5.30.0.1301-dev", mp.dll has the
  `ls_creature*` commands, and the hidden simulator copy's mp.dll had the same hash (`e429b769`). Plugins were compiled
  fresh into the sim from a `git archive` of tools HEAD `98b24f60` (`plugins-fresh.sh`). Every gm_* plugin and
  gamemodes built. 116 third-party plugins (zp50_*, sh_*, war3ft, basebuilder…) failed on missing includes and kept
  their old builds. QA4-A already reported that `plugins-fresh.sh` problem.
- **Dev servers.** The Rust Tools routes came from the shared target, built at `e315c267`. The tree then also held
  other agents' uncommitted `servers.rs` and `tools/mod.rs`.
  - Dev server 1 ran over the sim (`LS_TEST_GAME` = Sim/QA4T1), with its own `LS_APP_DIR`, TMPDIR, `LS_SERVERS_DIR` =
    Sim/QA4T1SRV, Vite cache and HMR off.
  - Dev server 2 ran over the real Half-Life folder, **read-only, for the Map simulator only**. The simulator needs a
    Steam-found install. It copied its hidden server from the sim (servers.json in my scratch app folder) into
    Sim/QA4T1SRV. **The real game had no file newer than my start mark at the end.**
- **Browsers.** Chromium and WebKit through Playwright, at 1440×900, 1024×680 and 800×600. I used the repo's specs
  plus my own scripts (untracked, removed after).
- **Sim.** One sim at a time, all with `-insecure`:
  1. Sim/QA4T1, a clone of LSPLAY on port 27015 (the dev server's game port), Last Stand on ls_outpost with 4 bots.
  2. After that was stopped, the simulator's own hidden server: two runs, plus one run I stopped.

  The load stayed under 20, except one reading of 21.4 as the first simulation started. `crashcheck.sh since 9333`
  came back empty.
- **Evidence** is in my scratchpad: `/private/tmp/claude-501/-Users-jameson/a1c85d27-df33-445d-9b50-85a20aff9c24/scratchpad/qa4t1/`.
  `shots/` holds the screenshots, `logs/` the dev server, sim and plugin-compile logs, and `app/` the reports and
  recordings. The rcon replies quoted here were read through a helper that never prints the password.
- **Not cleaned up:** the safety hook blocks `rm -rf` outside the working folder, even from inside the Sim folder.
  `Sim/QA4T1` (1.4 GB, an APFS clone) and `Sim/QA4T1SRV` (1.1 GB, the simulator's kept hidden copy) are still there and
  need deleting by hand. Both servers are stopped.

**Counts: 62 rows. PASS 52, FAIL 6, NEEDS USER 4. On top of the rows there are 20 UX/UI items: 2 high, 5 medium, 13 low.**

## FAILs (most important first)

### T1-1. The Waves tab's "mode's own mix" for Last Stand is out of date, and saving waves cuts Last Stand's round table to 4 types (medium-high). Owner: Creature Studio (`creatures.rs` `laststand_default`, fa206a37, agent Z) against W1's round table (`gm_components` `ls_wave_table`, 82c4e4b7)
`laststand_default()` still copies the old `pick_type`: zombie, runner from wave 2, brute from 3, spitter from 4, and a
boss every 5. Last Stand's real built-in is now the round table. It uses about 18 types, has roles, a themed round every
6 and bosses from round 10. The tab presents the old mix as "the mode's own" ("Start from the mode's own mix"; "Not saved
yet: the mode uses its own"). The first Save writes `[laststand]`, and Last Stand then reads waves.ini instead of the round
table.

Repro on a sim:
1. Before saving, gm_laststand logs "creature types for this map: zombie babycrab headcrab runner spitter brute vortigaunt
   assassin soldier controller stukabat hound snark babygarg boss gonarch turret".
2. In Waves, add one type (`qa_runner_`) at weight 50 on wave 1 and click Save waves.
3. After a map change the log says "creature types for this map: zombie qa_runner_ runner brute spitter boss turret".

Every other type, special round and heavy is gone, though the user changed one cell. Expected: the default shows the round
table as it really is, or the tab says that saving replaces the round table. Better still, a waves.ini line could override
only some waves.

### T1-2. An empty or non-number value in a creature's number field gives no inline error, and Save says "Something went wrong. Try again." (medium-low). Owner: Creature Studio UI (`TypeForm.tsx` `Num`, 5eef3137, agent Z)
Clearing a field, or typing letters (`abc`) into Health, stores NaN. No message appears under the control. React logs
"Received NaN for the value attribute". Save stays enabled. The request sends `"hp": null`, the server answers 422, and the
only feedback is a toast: "Something went wrong. Try again." Nothing says which field is wrong.
Repro: Creature Studio › any type › clear Health › Tab › Save. Seen in Chromium and WebKit. Screenshot `shots/creatures-hp-abc.png`.
Expected: "Type a number" under the field, and Save blocked like the range errors are.

### T1-3. "Spawn one in front of me" shows the brain's raw console line for a type the map didn't load, and the advice in the code is wrong for Last Stand (low-medium). Owner: Creature Studio (`creatures.rs` `spawn`, fa206a37, agent Z); the wording is the brain's (Y)
`spawn()` looks for "isn't precached" or "not precached". The brain says "type 'X' **wasn't** precached when the map
loaded - add it to ls_creature_precache …", so the toast is the raw line: "The game said: ls_creature: type 'qa_runner_'
wasn't precached …".

The plain-words message the code would show says "set ls_creature_precache to all … and change map". In Last Stand,
gm_laststand overwrites `ls_creature_precache` at map start. I set it to `all` and changed map: the cvar was "" again and
the type was still "(not precached)". In Last Stand, the way that works is adding the type to the mode's waves. After
that it was precached, and spawned with hp 77, chasing a bot.

Expected:
- Match "wasn't precached" too.
- In a mode that sets the precache list (Last Stand), say "add it to this mode's waves (Waves tab), then change map".

### T1-4. Names that Windows reserves are accepted when renaming plugins and demos; a demo rename can also make a name the game can't play (low; medium on Windows). Owners: Plugin Studio `plugins.rs` `safe_file` (agent R); Demos `demos.rs` `safe_name` / `rename` (17554ae0, agent U)
- `POST /api/tools/plugins/rename` to `con.sma` returns `ok`. Compiling it would write `con.amxx`. On Windows (the main
  platform), CON/PRN/AUX/NUL/COM1-9/LPT1-9 can't be file names, with or without an extension.
- `POST /api/tools/demos/rename` to `CON` (→ `CON.dem`) and `con` both return `ok`.
- Demos rename also accepts `a;quit`. The file becomes `a;quit.dem`, which the Watch button then refuses
  (`playable: false`). Yet the refusal message for bad names says "Use letters, numbers, - and _". Rename should use the
  same rule as `playable_name`.

This is the same class as QA4-A's A-2 (map names). One shared "safe file name" check would cover them all.

### T1-5. The Map simulator's shortest run gives a false "Worth a look" on de_dust2 (low-medium). Owner: Map simulator (`health.rs` findings, 76f1bbb5, agent F)
Run: de_dust2, Classic, 6 bots, 2 min (options the page offers). The verdict is **warn**: "Bots only used 17% of the map.
Parts of it may be hard to reach, or not in the nav." The run recorded 1 min 53 s, 6.8 bot-minutes and 1 round. The
coverage finding fires once bot-minutes reach `0.15 × bots × minutes` (here 2), so short runs nearly always trip it on a
stock map.

The 5-minute, 12-bot run on de_nuke covered 33% and said "No problems found". Expected: scale the coverage threshold with
bot-minutes, or don't judge coverage on runs this short. The same goes for "Nobody went to: Catwalk, Long, OutsideLong,
Pit" in the fights section, which the notes say is meant only for runs of 4+ minutes. Screenshot `shots/health-cancelled-webkit.png`.

### T1-6. A truncated demo looks like a normal one (low). Owner: Demos (`demos.rs` detail, agent U)
`truncated.dem` is the first 1.5 MB of a 3.7 MB demo, with no directory at the end. It is listed with its map and no
problem. Length is blank, the detail is `frames: 0, seconds: 0, problem: null`, and Watch is enabled. Empty and garbage
files do say "Can't be read". Expected: "This demo looks cut short (still being recorded, or damaged)". The same text fits
an HLTV demo that's still being written. Screenshot `shots/demos-panel-truncated-chromium.png`.

## Rows

### Setup, Rust and specs
| Check | Result | Evidence |
|---|---|---|
| Current brain and plugins | PASS | See the top of this file: run 43629835474 = ls-remote head `bdf4e8d`, version 1301-dev in the sim console, plugins compiled from `98b24f60` |
| Rust tests for creatures, plugins, health, replay, demos, overlays | PASS | 71 passed, 0 failed. With `LS_TEST_GAME` = the sim: `real_compile` (amxxpc "Done."), `real_includes`, `real_demos`, `real_recording`, `real_sweep`, `real_know_bundle` and `sequences_of_real_models` all pass |
| Repo specs: tools.spec, the five pages, focus mode, Creature Studio and Plugin Studio tests | PASS | 24/24 across Chromium large, Chromium small and WebKit |
| Repo specs: sim-report.spec | PASS | 6 passed, 2 skipped (no "worth a look" finding with a spot in my reports), in Chromium and WebKit |
| Repo specs: p10.spec demos | PASS | 2/2, in Chromium and WebKit |
| The real game untouched | PASS | `find <Half-Life> -newer <start mark>` found nothing |

### Creature Studio
| Check | Result | Evidence |
|---|---|---|
| Types list, form, 3D preview, tint, model and sound pickers, waves render | PASS | tools.spec in 3 projects; `shots/creatures-chromium-1440x900.png` |
| New type from a template | PASS | "QA Runner!" became `qa_runner_` (see UX L2), "yours" tag, form opens |
| Out-of-range numbers say so under the control | PASS | -5 → "Keep it between 1 and 100000: the game would use 1."; 1e9 → "…would use 100000." |
| Empty or letters in a number field | **FAIL** | T1-2 |
| Save writes creatures.ini in place, only what changed | PASS | The file had `[qa_runner_] base = runner, name = Runner (copy), hp = 77` and `[zombie] hp = 150`, under the header |
| Double-clicking Save | PASS | The second click is blocked while saving; one write |
| Save and apply reaches the running game | PASS | rcon `ls_creature types`: zombie hp 150, qa_runner_ hp 77 (not precached) |
| Spawn one in front of me (a built-in, standing still) | PASS | rcon `ls_creature list`: "#196 zombie alive hp 150 idle at -1488 912 0". The API says "A runner is in the game at a Terrorist spawn." (no host to aim with) |
| Spawn a type the map didn't load | **FAIL** | T1-3 |
| Server checks: name `all`, model `../../../evil.mdl`, `;` in a value, 57 types | PASS | Each refused with its own words; save refused "Not saved: all: … (and 2 more to fix)"; "The game holds 48 types at most (16 built in + 32 of yours): remove 9." |
| Waves save, and Last Stand reads it | PASS | waves.ini `[laststand]` written; after a map change Last Stand loaded qa_runner_; a spawn gave "qa_runner_ alive hp 77 chase -> Andre F." |
| Waves "mode's own mix" | **FAIL** | T1-1 |
| Game not running | PASS | Live, spawn and reload all say "Your game isn't running a map. Start one (Play), then try again." |
| Missing or bad model | PASS | "Not in your game's folders." with the stand-in picked; `../../etc/passwd` → "Not a model path." |
| WebKit flow (new type, save, apply, spawn, waves) | PASS | Same results; zombie hp 151 applied in game |
| Small windows (1024×680, 800×600) | PASS | No sideways scroll (see UX H1 for layout) |
| The creature on screen: tint, tier shells, enrage, spawn where the host looks | NEEDS USER | Needs the real client (gm_aim answers only with a person in game) |

### Plugin Studio
| Check | Result | Evidence |
|---|---|---|
| Start page, read-only shipped plugins, hover docs, F12, "/" menu, Help | PASS | tools.spec in 3 projects |
| New plugin from a template | PASS | `qa_hello_chromium.sma` in My plugins |
| Compile errors in plain words, on their line | PASS | 2.1 s; "Line 22 Error "nope_func" isn't defined anywhere…", a squiggle on line 22, raw compiler output in its tab. One wording issue: UX L1 |
| Compile clean; rapid clicks | PASS | "Compiled with no problems. 3.1 KB."; the 2nd and 3rd clicks were blocked while busy |
| Try it in my game | PASS | plugins-studio.ini got `qa_hello_chromium.amxx debug`, the .amxx is in plugins/, the map restarted, rcon `qa_ping_chromium` answered "QA4T1 pong from qa_hello_chromium" |
| Game log follows the plugin | PASS | Its `log_amx` lines appeared within 5 s |
| A run-time error | PASS | "Run time error 4: index out of bounds … qa_hello_chromium.sma::cmd_boom (line 13)" in Game log; `shots/plugins-runtime-error-chromium.png` |
| Take it out | PASS | The ini and the .amxx are gone; "It stops with the next map" (UX L13) |
| Try it with the game stopped | PASS | "…is in your game. It loads when you start a map (Play)." |
| Unsaved edits | PASS | Kept per file across switching (Not saved dot); the crumb/Back asks "Leave without saving your changes?". The dev harness's own sidebar doesn't ask, but the app's sections use `confirmLeave` |
| A copy under a shipped name | PASS | "gamemodes is one of Loner-Strike's own plugins, so the Plugin Studio won't put a plugin by that name in your game…" |
| Rename rules | **FAIL** | `../x.sma`, `a b.sma`, `x.txt`, a 70-character name refused; `con.sma` accepted (T1-4) |
| A 2.5 MB file, a symlink, odd file names | PASS | "That file is too big: 2.5 MB — the most is 2.0 MB. Open it in another editor."; the symlink gives "That isn't a plugin source file."; `bad name.sma` and `évil.sma` are hidden (UX L9) |
| No game install record (missing setup) | PASS | Compile and Try are disabled with "Couldn't find your game's install to run the compiler in." (UX L10) |
| WebKit full flow | PASS | Same results, "QA4T1 pong from qa_hello_webkit" in game |

### Map simulator (`health`; the brief said "map")
| Check | Result | Evidence |
|---|---|---|
| Start, progress and a double click | PASS | The second Run click is blocked; "Starting the hidden server…" then "6 bots playing de_dust2 · 1 min 57 s left", with ETA and % |
| Run on a hidden copy with the current brain | PASS | The hidden copy's mp.dll hash and version match the sim; 6 of 6 bots; report saved with a recording |
| Report contents | PASS | Map layers, rounds and sides (T 1 · CT 0), fights by place, numbers, map knowledge "Loaded", plugin errors 0 |
| Finding on the shortest run | **FAIL** | T1-5 |
| Stop mid-run (WebKit) | PASS | Stop at 45 s: "The simulation of de_dust2 was stopped.", progress phase `stopped`, no hlds left (`pgrep` 0), no report |
| Second start, bad map, bad mode, no X-Panel | PASS | "A simulation is already running…", "That isn't a map name.", "Pick one of the server's game modes.", 403 |
| A longer run | PASS | de_nuke, 5 min, 12 bots: "No problems found", 26 kills, 33% coverage, 3 rounds, 1 stuck spot |
| Watch in Replay (header and spot) | PASS | The spot (t = 32 s, bot 11) opened Replay at 0:26 following pesto_sux (bot 11), 3 s before |
| Map picture after leaving and coming back | PASS | The `<image>` is still there (spec and my script) |
| The app's "no server, made from your game" path | NEEDS USER | Needs SteamCMD's `.base` and the built app; this run copied the sim as its source |

### Replay
| Check | Result | Evidence |
|---|---|---|
| List and the size cap | PASS | 340 MB file: card disabled with "This recording is too big to open here (340 MB; the limit is 200 MB)…" |
| Garbage, empty, a map not installed | PASS | Garbage opens to "This file isn't a recording…"; empty is left out; missing map gives "That map isn't on this computer." (UX L8) |
| Playback | PASS | 4× default, 16×, Space pauses (clock held), scrub to the end, Follow, heat map options, top-down; a short recording after a long one starts at 0:00 / 0:18. Chromium and WebKit |
| HTML in a player name | PASS | `<img src=x onerror=alert(1)>` as a name: no dialog, shown as text |
| Open a recording… (system file dialog) | NEEDS USER | The native picker |

### Demos
| Check | Result | Evidence |
|---|---|---|
| List of good, empty, garbage, 1.2 GB and unicode-named demos | PASS | Map, length, size; "Can't be read" on the bad ones; `shots/demos-chromium-1440x900.png` |
| Over the 1 GB cap | PASS | "This demo is too big to look inside." (header still read: 1:05) |
| Truncated demo | **FAIL** | T1-6 |
| A name the game can't be told to play | PASS | Watch disabled, "Rename to my_demo.dem" offered |
| Rename | PASS | Existing name: "There's already a demo called NAS_Auto_Demo.dem."; `../escape`, `a/b`, `.hidden`, blanks refused; undo toast. Reserved and unplayable names are T1-4 |
| Delete | PASS | "Delete empty.dem? It can't be brought back." Cancel keeps it |
| Search, filters, keyboard | PASS | "dust2" finds TopFrag; Arrow and Enter open the side panel |
| Show my path in Replay (66 MB, 27 min) | PASS | Replay opened in ~6 s, 1 player, 26:35. Chromium and WebKit |
| Watch (starts the game with +playdemo), Show in Finder | NEEDS USER | They start the player's game or open Finder |

## UX/UI review
Screenshots are in `/private/tmp/claude-501/-Users-jameson/a1c85d27-df33-445d-9b50-85a20aff9c24/scratchpad/qa4t1/shots/`.

### High
- **H1. At small window sizes the editor and form sit below the fold** (Plugin Studio, Creature Studio).
  - At 800×600, Plugin Studio shows the header, blurb, focus tip and the whole file list; the code editor is off screen.
    In Creature Studio the 3D preview comes first and the types list and form follow it.
    Shots: `plugins-small-webkit-800x600.png`, `creatures-small-webkit-800x600.png`.
  - Suggestion: under ~1000 px, fold the file list and types list into a drawer or select, show the editor and form
    first, and collapse the title and blurb into one line.
- **H2. Toasts cover the bottom action bar.** After each compile or save, "Undo that save?" and the result toast sit on
  top of Try it / More / Save (and Save waves) for several seconds. Shots: `plugins-errors-chromium.png`,
  `waves-saved-chromium.png`. Suggestion: lift the toast stack above the action bar's height, or move it top-right on
  tool pages.

### Medium
- **M1. Text contrast is below WCAG AA in all five tools.**
  - Muted notes measure 3.62:1 (the simulator's intro, row notes, "Nothing here yet…").
  - Selected segmented and tab labels (yellow on olive) measure 2.79:1: "Problems", "All", "5 min", "Creature types".
  - The "built in" and "Half-Life's own" tags measure 2.79-2.90:1. Small buttons are 4.24:1.
  - Suggestion: raise the muted color and the selected label until both reach 4.5:1.
- **M2. Replay's recordings can't be told apart.**
  - Each card shows only the map and a date, and three cards read "br_valley · 25 min".
  - A demo path says "0 bots · 0 kills", and its header says "1 players".
  - Shot: `replay-list-chromium.png`.
  - Suggestion: show where a recording came from ("Map simulator, de_nuke 5 min, 12 bots" or "Demo TopFrag-DUST2only"),
    count players for demo paths, fix the plural, and add delete/rename.
- **M3. The Creature types list doesn't work from the keyboard.** It is `role=listbox`, but the arrow keys don't move
  through it, and in WebKit Tab skips its buttons. Suggestion: give it a roving tabindex with Up/Down/Home/End, the way
  the Demos table does.
- **M4. Dialogs close on error and drop what was typed.** In a Demos rename to a taken name, the dialog closes and a toast
  says "There's already a demo called …". Suggestion: keep the sheet open with the error under the field. The same goes
  for the new-type name in Creature Studio.
- **M5. The Map simulator forgets the picked settings after Stop.** I ran 5 min / 6 bots, stopped it, and the form came
  back as 5 min / 12 bots. Shot: `health-cancelled-webkit.png`. Suggestion: keep the last choices.

### Low
- **L1.** A plain-words compiler error shows the compiler's placeholder: ""-unknown-" is an array: use one element
  (-unknown-[0])…" for `new x = "a"`. Suggestion: when the name is `-unknown-`, say "this value" and explain array
  assignment (`plugins-errors-chromium.png`).
- **L2.** The new-type name is changed without saying so: "QA Runner!" becomes `qa_runner_`. Every copy is shown as
  "Runner (copy)", so the list holds look-alike names. Suggestion: show the cleaned name live in the dialog, and number
  the copies.
- **L3.** The "New from a template…" menu has blurbs for the first 7 templates only (Headcrab…Gonarch have none).
- **L4.** "Saved the waves for laststand." uses the mode's id. It should say "Last Stand".
- **L5.** Plugin Studio's file Filter box is bright white, unlike every other dark search field.
- **L6.** Small layout issues:
  - The page header's "Docs ↗" button sits a few pixels higher than History and Focus, on every tool.
  - In Demos, a 0-byte file shows as "1 KB".
  - At 800 px the Demos "?" help wraps onto a line of its own.
- **L7.** The Demos HLTV filter with none says only "No demos match." Suggestion: say that HLTV demos come from a
  server's HLTV (Servers › HLTV), with a link.
- **L8.** Replay errors that retrying can't fix still offer "Try again" ("This file isn't a recording", "That map isn't
  on this computer"). Suggestion: offer "Back to recordings", and "Get the map" for the missing map.
- **L9.** Plugin Studio hides files whose names it can't use (`bad name.sma`, `évil.sma`) without saying so.
  Suggestion: add a note such as "2 files hidden: rename them to use letters, numbers, - and _".
- **L10.** "Couldn't find your game's install to run the compiler in." doesn't say what to do. Suggestion: "Set up your
  game on the Setup page first."
- **L11.** Disabled primary buttons look nearly enabled: Run the simulation with no map picked, and Watch in the demo
  panel for an unplayable name (`demos-hltv-empty-chromium.png`). Suggestion: lower their opacity or remove the fill.
- **L12.** Game log shows an AMX Mod X "performance issue. Function … executed more than 1.1ms." line for every call
  (the `debug` flag). Suggestion: hide these or explain them once.
- **L13.** Try it restarts the map, but Take it out says "It stops with the next map". Suggestion: restart the map the
  same way, or offer a button to do it.

## Requests (for the owners' notes)
- Agent Z / Y.md: T1-1, T1-2, T1-3; UX M3, L2, L3, L4.
- W1.md: T1-1 (decide what the Studio's "Last Stand default" should be: the round table, or a warning before replacing it).
- Agent R (Plugin Studio): T1-4 (plugins); UX L1, L5, L9, L10, L12, L13.
- Agent U / P10.md: T1-4 (demos), T1-6; UX M4, L6, L7, L11.
- Agent F (Map simulator, health.rs): T1-5; UX M5.
- Shared shell/theme (P8/P1): UX H1, H2, M1, L6 (Docs button).
- Replay (agent C / B): UX M2, L8.

## Fixed (2026-11-14)
- **T1-1** (73bb3fef): the Waves tab starts from Last Stand's real round table: every type by round, with `own_rounds = 1`
  so the mode keeps its own boss waves, hound/snark rounds and heavies. gm_components reads `own_rounds` and takes 16
  types a line. Saving with one type added keeps all 18 (Rust test). Added "Reset to Last Stand's own" and the line "Saving
  makes this game's Last Stand use your list". The save message now reads "Saved Last Stand's waves." (L4).
  - Sim check (Sim/T1FIX, brain run 43634247724, 1306-dev; plugins from 9fcb59db plus this gm_components): "creature
    types for this map: zombie qa_runner_ runner babycrab headcrab spitter brute vortigaunt assassin soldier controller
    stukabat hound snark babygarg boss gonarch turret".
  - On the very first load after a server start, gm_laststand reads `lst_waves` before the cvar exists. So it takes types
    up to wave 10 only, and controller and stukabat come in from the next map. This was there before the fix.
  - Pack rounds and heavies in a long run weren't watched.
- **T1-2** (73bb3fef): an empty or non-number field shows "Type a number." under it. Save is blocked with the count to fix,
  and no request goes out.
- **T1-3** (73bb3fef): the toast matches the brain's "wasn't precached". In Last Stand it says to add the type to its
  waves, save, then change map (anywhere else it names the cvar).
  - Sim check: the spawn was refused with that text. After following it, "qa_other spawned".
- **T1-4** (d1812542, 8bf396a2): Plugin Studio names and Demos rename refuse CON/PRN/AUX/NUL/COM1-9/LPT1-9 in any
  case, with or without an extension. They use `setup::record::windows_unsafe`. Demos rename uses Watch's rule (no `;`).
- **T1-5** (389ba573): map coverage is judged only on runs of 3+ minutes and 20+ bot-minutes, with a lower bar under 40
  bot-minutes. "Nobody went to" shows only on runs of 4+ minutes.
- **T1-6** (d1812542, 8bf396a2): a demo with no directory at its end shows "This demo looks cut short (still being
  recorded, or damaged)." and Watch is off.
- **UX:**
  - H1: Creature Studio 73bb3fef, Plugin Studio 8bf396a2.
  - M2, M5: 389ba573.
  - M3: 73bb3fef.
  - M4: 8bf396a2 (shared dialog `check`), plus Creature Studio's new-type name in 73bb3fef.
  - L1, L5, L7, L9, L10, L12, L13: d1812542 and 8bf396a2.
  - L2, L3, L4, L6 (Docs button): 73bb3fef. L6 (Demos): 8bf396a2.
  - L8, L11 (simulator): 389ba573. L11 (Demos): 8bf396a2.
- **Creature Studio scope** (73bb3fef, 5d9516a4):
  - A line names what the Studio edits and who it reaches.
  - Docs gained "How creatures work" (docs.html#creatures-how).
  - Publish now warns: "Custom creatures aren't included yet: players need to make <name> themselves". Definitions
    aren't packaged.
- Not done here: H2 (toasts) and M1 (contrast) belong to the app-wide pass.
- Left behind: `Sim/T1FIX` (1.4 GB, an APFS clone, stopped). The safety hook blocks deleting it from here, so delete it
  by hand.

## Shared creatures (1.0, 2026-11-14): QA rows
| Check | Result | Evidence |
|---|---|---|
| Publish a Studio mode that spawns your own type | PASS | `creature_round_trip` (ignored test, `tools/creature_roundtrip.rs`): package `qa_rt-mode-loner-strike.zip` has `creatures/creatures.ini` with `[gb1234567.qa_crawler]` (base runner, hp 77); the packaged graph's spawn names `gb1234567.qa_crawler`; index field `creatures: [{name: gb1234567.qa_crawler, model: false, sounds: 0}]` |
| Install it on a clean game | PASS | Unpacked with `archive::extract_strict`, block `; >>> from mod-9990001` written to Sim/T1CPT's creatures.ini (there was no file before); the graph installed through the Studio's compile route (amxxpc "Done.") |
| The creature spawns in a sim | PASS | Sim/T1CPT (brain ls-next3 run 43638373487, 1308-dev; plugins fresh from 7f3fe82e), `-insecure`, de_dust2, `gm_mode qa_rt`: `ls_creature list` shows 5 × `gb1234567.qa_crawler alive hp 77 chase`; `ls_creature types` lists it (houndeye model, from runner) |
| Uninstall removes only that block | PASS | With the player's own `[qa_mine]` above and `[qa_mine2]` below the block, `uninstall_pack` left both and their notes as they were and took out only the marked block (no files to remove) |
| Update, Go back, failed update, ref counts, server carry-over, refusals | PASS (Rust) | creaturepack, creaturekind, creaturesrv, archive and community tests (see the agents' commits 9f5c69c1…4d583fd2, 54e534ff, ac8e1c3a) |
| Picker, Publish listing, Used by, creature card | PASS (Playwright) | creature-picker, community-creatures, community-creature-kind specs in Chromium and WebKit |
| A real GameBanana publish, Get, update and Go back | NEEDS USER | Needs a real upload and the list service deployed |
| A real Linux server getting a block | NEEDS USER | Unit-level only (the upload filter passes creatures.ini, models/, sound/) |

## NPCs and interactables (1.0, 2026-11-14): QA rows
| Check | Result | Evidence |
|---|---|---|
| The Quest Giver installs and runs | PASS | Sim/T1NPC (brain ls-next3 run 43648864150, 1310-dev; plugins fresh from b5bb94c7 plus 801fbeed's gm_components), `-insecure`, de_dust2, 5 bots, debug build of the example: Old Sam spawned (entity 236), the note (237) |
| Talked To NPC fires from a use | PASS | `ls_npc_use 1 236` → events `talk ran`, `ask asked` |
| A choice flows | PASS | `ls_npc_pick 1 0` → `ask reply 1` → `take true` → `thanks said`; a second bot answered by itself → `ask reply 2` → `later said` |
| The interactable fires Used Interactable and the reward | PASS | `ls_npc_use 1 237` → `used ran` → `if true` → `pay done` → `yay said` |
| Bots don't shoot NPCs or get stuck on them | PASS (Y) | Y's brain check: 16 bots, 16 NPCs, 0 hits, nobody stuck; unkillable NPCs take no damage, killable ones only when the mode allows |
| Plain-map I/O (greeting, targets) | PASS (Rust/FGD) | `target` is a target_destination, shown in the I/O panel (npc-editor spec) |
| Editor icons, preview, Checks; Studio nodes and the example | PASS (Playwright) | npc-editor and studio-npc specs, Chromium and WebKit |
| Talking through the menu, timeouts and walking away with a real player | NEEDS USER | Menus are drawn only for people; bots answer by themselves |
| Bots using an interactable on their own ("Bots may use") | NEEDS USER / W2 | W2 owns the bot side; not watched in this sim |
| The Quest Giver, bring it back (after 35360b25's fix) | PASS | Sim/T1QG (brain run 43648864150, 1310-dev: the newest green; the ls-next3 head run was still building; plugins fresh from 35360b25), rcon-driven with a bot: talk → Yes (`take true`, `thanks`); talk → hint (`still`); use note → `got true`, `found`; use again → `meh`; talk → `reward`, `pay done` ($1000), both flags cleared; another player's use → `meh` |

## Projectiles (1.0, 2026-11-14): QA rows
| Check | Result | Evidence |
|---|---|---|
| Two Spawn Projectile nodes, each with its own On Hit | PASS | Sim/T1PJ (brain run 43651450817, 1314-dev; plugins fresh from 95595e10), `-insecure`, de_dust2, 5 bots, debug build of a two-projectile graph: every rocket hit ran `boom` and every orb hit ran `bonk` (9 each over the spawns), never crossed |
| Hits: world, entity, player with hit zone | PASS | `ls_proj_test`: world hits with their normals, a `func_breakable` (entity 17), players with hit groups 1, 3, 6, 7 |
| Kill credit to the owner | PASS | log: `"[LtD] Tilted<10><BOT><CT>" killed "chameleon_cs<8>..." with "ls_projectile"` (and pesto_sux); same-team hits did no damage with friendly fire off |
| A hit on the spot it was spawned at | NOTE | One test projectile fired from a hand-typed point reported a world hit at its own start (probably inside a brush there); not seen from players' eyes |
| Bots treating projectiles as dangerous | NEEDS BRAIN | Not built; bots ignore projectiles |

## Send my creatures, NPC placement (1.0, 2026-11-14): QA rows
| Check | Result | Evidence |
|---|---|---|
| A pushed creature spawns on the server | PASS | Sim/T1SV (brain run 43669651332, 1316-dev; plugins fresh), `-insecure`: Send put `qa_crawler` in an `own:creatures` block after the server's own `[srv_own]`; on the server (Classic, types loaded with the map) `ls_creature spawn qa_crawler` → "qa_crawler alive hp 77 chase -> <a bot>"; the server's own type spawned too |
| Remove takes only that block off | PASS | after Remove the server's creatures.ini is byte-identical to before the send (`cmp`); the message now says "Removed from 1 server." (b69956eb) |
| NPC and interactable spawn right next to a bot | PASS | plugins from b69956eb, Last Stand on de_dust2: `ls_npc_spawn_test` at three bots' feet, NPC and interactable, default, `fail` and `near:16`: all made, moved 0 (players don't block) |
| A spawn that has to move, and Studio's "If the spot is blocked" / Spawned-Failed outputs | NOT YET | the move is covered by a Rust test only; the node option waits for FX's shared helper |
| Note | — | Last Stand and a mode's base.cfg set `ls_creature_precache` at map start: a pushed type a server's mode doesn't list isn't loaded there (add it to that mode's waves, or the cvar for other modes) |
