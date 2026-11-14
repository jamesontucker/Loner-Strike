# The combined re-check (prepared by P5, 2026-11-14; not run yet)

One build from one commit, every automatic check, one summary table, then what only the user can
check. **Don't run it until the coordinator says so.**

## How to run it
```
node scripts/recheck.mjs --dry-run      # what it would do
node scripts/recheck.mjs                # HEAD; or --commit <sha>
```
- Run it **outside a sandbox**. A sandboxed shell can't stop CrossOver processes, so the sim's
  hlds.exe would be left running, and the failure is silent.
- It needs `GITLEAKS=<path>` (the audit's copy) unless gitleaks is on the PATH. It never installs it.
- Other options: `--only rust,ui,…`, `--out DIR`, `--target DIR` (cargo target, default
  `~/Library/Caches/loner-strike-recheck-target`), `--game DIR`, `--sim-base NAME` (default
  `LSPLAY`), `--keep`, and the ports `--vite-port 5390 --dev-port 8390 --sim-port 27490`.
- It stops, marking the rest SKIPPED, whenever free disk is under 8 GB. That check runs before each
  step and each sim step.
- `<out>/summary.md` is the table. Each step's full output is in `<out>/<step>.log`, and the failed
  Playwright tests are in `ui-failed.txt`.

## What it does
| Step | What | Notes |
|---|---|---|
| worktree | A detached `git worktree` of the commit, with `node_modules` linked from this checkout | Nothing in a working tree is used or changed. It's removed at the end. |
| rust | `cargo test --bin loner-strike` (the whole suite, `-j 6`, `nice`) | One build, in `--target`. The Playwright dev server uses the same build. |
| node | `tsc --noEmit` and every `app/ui/scripts/test-*.mjs` | Includes the panel-GET header check. |
| ui | Every Playwright spec in Chromium (`large` 1440x900, `small` 1024x680) and WebKit (`LS_WEBKIT_ALL=1`: all specs, not only the listed ones) | Runs against the tools dev server over a **full APFS clone of the game** (`Sim/RECHECK-HL`), never the real folder or the small test game. The people save bar, the map browser keys and the Creature and Plugin Studio specs failed on the small one. App data goes to `<out>/app-data` (`LS_APP_DIR`; test builds refuse to start if any app path is outside it). |
| list | `python3 -m unittest discover -s services/serverlist` | The list service: community, roles, creatures, formats, modbot. |
| modfiles | `scripts/mod-files-check.py` | Base Builder's SQL on SQLite, and the lang files. |
| english | `scripts/us-english-check.py` | |
| gitleaks | `gitleaks git --log-opts="-1 <commit>"` | Over the commit itself. For the release, also over the orphan commit (P5.md recipe). |
| sims | One CrossOver server at a time, `-insecure`, 5 bots: a clone of `Sim/<sim-base>` | The commit's plugins are compiled fresh (amxxpc, for every `.amxx` the server has whose source is in the commit), with its mods' data and configs and its gamemodes configs. A step's map the sim lacks is cloned from the game, with what its `.res` lists. Each step switches the mode as the app does (`gm_mode`), waits, and reads the log. Every step fails on "Run time error", "Host_Error", a segfault, "failed to load", or a server that stops answering. |

### Sim steps
| Step | Mode / map | Pass when | Covers |
|---|---|---|---|
| classic | classic / de_dust2, 120 s | 1+ kill | The classic modes still play (QA4-CORE) |
| csdm | csdm / de_dust2, 90 s | 5+ kills | Respawn modes |
| gungame | gungame / de_dust2, 90 s | 3+ kills, no "Invalid multi-lingual line" | caca2004 |
| basebuilder | basebuilder / bb_aztec, 120 s | No "SQL-LOG", no "syntax error" | ee9818d3 |
| bb-precache | basebuilder / bb_dust_iplay, 90 s | The server still answers (no Host_Error over the 512 limit) | 56f3ca19 |
| laststand | laststand / ls_outpost, 180 s | No errors, still answering | QA4-C and W1 fixes (waves, turrets, boss in the gateway) |
| br | br / br_valley, 180 s | 1+ kill | QA4-D and W2 (squad out, revives, high ground) |
| deathrun | deathrun / deathrun_temple, 150 s | 1+ round ended | 584eb81d (time-up ends the round) |
| zombie | zombie / zm_deko2, 120 s | 1+ round ended | Zombie Plague |
| modevote | `gm_votemode`, 60 s | "Map vote for" in the log | a7ef360f (mode vote map lists) |

Bot behavior numbers (revive rates, BR high ground, Last Stand difficulty) are W1's and W2's A/B
sims (`~/loner-strike-sim`), not this re-check.

## Only the user can check these (the real client, accounts, hardware)
- **The game itself:** starting a game from Play and playing a first match. Seeing in the client
  that turrets do damage, the Mk II bench upgrades on a gun, and a person revives a downed squad
  mate (QA4-C, QA4-D). Kreedz climbing (timer, checkpoints). Zombie Plague Advanced rounds. A spray
  picked in the game's Options after the Spray maker's Put back (QA4-T2 F3).
- **Playtest in the client:** Game Mode Studio playtest and Creature Studio's live list in a running
  game (QA4-B, QA4-T1). The top strip's picture during a real map change (QA4-CORE F1).
- **Setup on a real game:** "Use Loner-Strike's" on a game with a foreign ReGameDLL (QA4-CORE F4).
  It can also be tried on a copy. The 0.1.2 → 1.0 update through a signed feed (QA4-F).
- **Accounts and networks:** Play with friends (playit.gg, router UPnP). Find servers with live
  internet servers. The live Community list (register, moderator queue, unlisting). Community roles:
  a Polar purchase, `/link`, the supporter VIP pull and remove (QA4-G, roles).
- **Hardware and dialogs:** voice commands (microphone, model download, a dedicated server). Backup
  to a file and restore from one (native dialogs).
- **Windows:** the QA4.md area H rows (a real PC): setup, servers, updates, Local AppData moves.
- **Linux server:** ls-control roles unit, crash records, an RCON-masked console on a real box (Y).
- **GameBanana:** uploading a package and the AI Usage section (Publish only prepares the text).

## Commits since QA round 4 began, and how each is verified
Made with `python3 scripts/recheck-list.py`. Run it again just before the re-check: other agents are
still committing. It counts from 6eb8e379 (the QA4 checklist draft). Notes-only commits are left out.
A row's tests are the ones the commit added or touched. Where it has none of its own, the row says
which suite, sim step or user check covers it. Every row is also covered by the full Rust and
Playwright runs.

### GameBanana publish

| Commit | What | Verified by |
|---|---|---|
| b2378de8 | Publish and GameBanana's rules: a credits block for GameBanana's Authors module (one line per person with their roles: the publisher, a remix's… | Rust `gamebanana_credits_and_ai_disclosure`, `publish_hears_which_ai_and_what_it_changed`; Playwright `community-gb-credits.spec.ts` |

### QA4-CORE

| Commit | What | Verified by |
|---|---|---|
| 1aeec7f1 | QA4-CORE fixes: the top strip's map picture (name tile underneath, the radar right away while GameBanana is looked up, asks again with ?v=); every… | Rust `a_kept_regamedll_reaches_the_done_screen`, `test_builds_keep_every_app_path_in_the_test_folder`, `which_server_code_is_kept`; Playwright `qa4-core.spec.ts`; node `test-panel-gets.mjs` |
| 56f3ca19 | Game modes: a map with more models than the engine can load with a mode can't hang the server (preset skips it, mode and map picks refuse it, vote… | (none of its own) the full suites |

### Editor coverage

| Commit | What | Verified by |
|---|---|---|
| e140b681 | Editor coverage sweep: every command, key, dialog, panel and tool of the map editor run once with its effect and Undo checked (Chromium, WebKit… | Playwright `editor-coverage-commands.spec.ts`, `editor-coverage-dialogs.spec.ts`, `editor-coverage-file.spec.ts` (+4) |

### Node coverage

| Commit | What | Verified by |
|---|---|---|
| 08b63471 | Studio: long flows no longer crash, the Playtest stream no longer loses events, numbers and short vectors print with decimals (node coverage sweep) | Rust `event_buffer_fits_one_fputs`, `printed_numbers_keep_decimals`, `short_vectors_as_text_keep_decimals` (+1) |
| 7783e9e7 | Studio: a mode that reads the round clock (Round Time Left, Set Round Clock...) with no Round Time Up event compiles again - its empty time-up… | (none of its own) the full suites |

### Plugin import and export

| Commit | What | Verified by |
|---|---|---|
| bbcde2ad | Plugin Studio: Community doesn't take plugins, and the save bar says so (post the .sma on GameBanana or AlliedModders; share a mode from Game Mode… | Rust `export_carries_own_includes_and_import_never_overwrites`; Playwright `plugins-share-io.spec.ts` |

### Contrast

| Commit | What | Verified by |
|---|---|---|
| 48e3f86d | App-wide contrast to WCAG AA (QA4-T1 M1, QA4-T2 UX-H1): panels a shade darker and text tokens lighter, same hues (selected tabs 2.8 -> 4.6:1,… | (none of its own) the full Playwright run (screenshots by eye) |

### Base Builder

| Commit | What | Verified by |
|---|---|---|
| ee9818d3 | Base Builder: player data saves on SQLite again (a continued query string lost its space before WHERE; NOW() in the clan query);… | (none of its own) mod-files-check.py; sims basebuilder, bb-precache |

### Gun Game

| Commit | What | Verified by |
|---|---|---|
| caca2004 | Gun Game: language file without the // first line (Invalid multi-lingual line on every map) | (none of its own) mod-files-check.py; sim gungame |

### Share and remix

| Commit | What | Verified by |
|---|---|---|
| 1d74d66e | Docs: shared creatures (How creatures work: sharing, Share/Remix, installed blocks, servers; Share and remix maps: a map's custom creatures go… | (none of its own) Rust mapsource tests; community-remix.spec.ts |
| 54e534ff | Community's creature kind: Creature Studio's Share… packs one of your types (its custom bases, models and sounds, named for your GameBanana… | Rust `a_creature_package_has_the_list_services_layout`, `get_update_go_back_and_remix_with_blocks`, `install_namespace_comes_only_from_the_lists_verified_uploader` (+2); Playwright `community-creature-kind.spec.ts` |
| 644032a3 | Community list, creature security mirrors: every brain key that names a file is path-checked (model/models, sounds and snd_* relative to sound/,… | list service `test_creatures.py` |
| a3bd28c5 | Shared creatures, the core (tools/creaturepack.rs): find the creature types a Studio graph, a map's or prefab's entities (keys from… | Rust `blocks_split_and_join_back_the_same`, `detection_in_graphs_entities_waves_and_bases`, `install_and_uninstall_keep_the_players_part_and_count_users` (+6) |
| 31f1cb19 | Community list: custom creatures in packages, and a creature kind. Maps, modes and prefabs may carry creatures/creatures.ini with their models and… | list service `test_community.py`, `test_creatures.py` |
| 8441e600 | Share and remix, security review: one sanitizer for both ways (Publish's shared project and Remix's import): editor settings from an allowlist… | Rust `a_published_project_carries_no_path_from_the_authors_computer`, `a_remix_takes_no_path_from_the_package` |
| 8afc8d30 | Share and remix maps: Publish › A map can include the editable source (off by default; the map's project from the editor's recent list): source/… | Rust `a_package_with_its_editable_source_and_a_remix_credit`, `a_remix_comes_back_with_its_credit_and_never_overwrites`, `malformed_source_is_refused_with_a_reason` (+2); Playwright `community-remix.spec.ts` |
| 7b6e1b2b | List service: map packages may carry their editable source (source/: .lsmap, .map, the map's own .wad, a .txt; the project checked as JSON, the… | list service `test_community.py`, `test_formats.py` |

### Creature sharing

| Commit | What | Verified by |
|---|---|---|
| 3110edc9 | Shared creatures: an ignored QA round trip on real game folders (publish a Studio mode that spawns the author's own type, unpack the package as… | Rust `creature_round_trip` |
| 4d583fd2 | Community downloads unpack strictly: only a zip, every entry name plain ASCII (letters, digits, spaces, _ . / -) with no part that climbs out, is… | (none of its own) Rust creaturepack and community tests; list-service test_creatures.py |
| 7f3fe82e | Community list: package entry names follow the app's strict archive rule: plain ASCII letters, digits, _ . space and - only, no part ending in a… | list service `test_creatures.py` |
| 2b58fc69 | Shared creatures named for the uploader's GameBanana member id (gb<id>.<type>, exactly the community list's rule; people see the type and its… | Rust `a_stock_only_map_ships_no_game_files`; Playwright `community-creatures.spec.ts` |
| 5e3fccdf | Shared creature labels: 'by <author>' only from a package its maker uploaded (verified member id = the type's gb<id>); a type carried in someone… | (none of its own) Rust creaturepack and community tests; list-service test_creatures.py |
| 99460133 | Shared creatures read '<type> by <author>' (the community library's author, else the member id) in the pickers and Creature Studio's installed… | Playwright `creature-picker.spec.ts` |
| 70d475c0 | Community list: shared creatures are named after the uploader's GameBanana member id (gb<id>.<name>); the namespaces table, its migration and… | list service `test_community.py`, `test_creatures.py` |
| 640c0eb6 | Creature Studio: the running game's installed shared types (<author>.<type>) show in the live list | (none of its own) Rust creaturepack and community tests; list-service test_creatures.py |
| 212c68b3 | Shared creatures, pickers and Used by: one creature type picker (built-in, yours, installed <author>.<type>; any name can still be typed, with a… | Rust `names_used_by_and_unknown_types`, `save_keeps_installed_blocks_byte_identical`; Playwright `creature-picker.spec.ts`, `editor-kits.spec.ts` |
| ac8e1c3a | Shared creatures reach servers: a community mode put on a server brings its creature block (with its models and sounds) from this computer's game… | Rust `a_map_brings_the_players_own_types_by_their_names_with_their_bases`, `a_mode_to_a_server_carries_its_block_and_files_and_takes_them_back`, `shared_creatures_go_up_with_the_server_folder` |
| 3515626b | Shared creatures travel with maps, modes and prefabs: Publish finds the custom creatures a map's entities, a mode's graph or a prefab name (with… | Rust `maps_and_modes_carry_their_custom_creatures`; Playwright `community-creatures.spec.ts` |
| 49f15523 | Community list, namespace claims: taken only when an entry is written as listed, inside the same transaction as the write (after the blocked and… | list service `test_creatures.py` |
| f98f2152 | Shared creatures, security: a package only replaces a file its own earlier version wrote (another package's or the player's stays; a shared file… | Rust `a_hostile_pack_cant_name_a_path_through_any_key`, `a_package_with_twin_entries_is_refused`, `one_package_never_replaces_anothers_file` (+2) |
| 80d144de | Community list, creature review fixes: an author part is bound to one GameBanana member (first to publish; another member whose name slugs the… | list service `test_community.py`, `test_creatures.py` |
| 3ee2674c | Community list: a shared creature's author part must be the publisher's full slug (the app no longer shortens it); tests | list service `test_creatures.py` |
| 2c4f6ec2 | Community list: shared creatures use canonical keys only (ls, sounds, models, display, health, detect, fov, behavior, guard_radius refused);… | list service `test_creatures.py` |
| 0573d2cc | Last Stand round table: creature type names up to 31 characters (shared creatures are named <author>.<type>): wave rows, boss, stand-ins and the… | (none of its own) Rust creaturepack and community tests; list-service test_creatures.py |
| 5d9516a4 | Publish a Game Mode Studio mode: custom creature types aren't in the package (only their names), so its Requirements and the Publish check now say… | Rust `custom_creatures_are_named_as_not_included` |

### QA4-T2

| Commit | What | Verified by |
|---|---|---|
| dbea3d2c | QA4-T2 low items: Nav fixer's Fix N is one edit and one Undo, and Draw area and Link wait for the nav; Callouts keeps 'N picked' in the status… | Rust `a_batch_is_one_step`, `radar_settings_errors_are_words`, `stock_maps_keep_their_res_and_copies_dedupe`; Playwright `qa4-t2-fixes.spec.ts` |
| 9fcb59db | Tools UX from QA4-T2: toasts stack above the save bar on every page with one (tools and Studios), and a save's message and its Undo are one toast;… | Playwright `qa4-t2-fixes.spec.ts` |
| c50168f2 | Spray maker: Put back gives the old spray back writable when it was (QA4-T2 F3), like the toast's Undo, so picking a spray in the game's Options… | Rust `put_back_restores_a_writable_spray_writable` |
| a7782410 | Map tools find maps the game downloaded from servers (QA4-T2 F2): cstrike_downloads and czero_downloads maps are in the Tools map list ("From a… | Rust `finds_maps_the_game_downloaded`, `maps_the_game_downloaded_are_in_every_tool` |
| b6e1d39f | Stock files are only Valve's own (QA4-T2): doctor, the .res, publish packaging and the source's custom-only WAD share one check built from Steam's… | Rust `czero_texture_files_are_stock_on_czero_maps_only`, `stock_with_czero_installed` |

### QA4-T1

| Commit | What | Verified by |
|---|---|---|
| 519418de | Last Stand: creature names read from the game hold 31 characters (shared <author>.<type> names) | (none of its own) the T1 specs (qa4-t1-plugins-demos.spec.ts, creatures-qa4.spec.ts, sim-replay-qa4.spec.ts) |
| 5cd70475 | Last Stand: the first map after a server start loads every creature type of the endless table (lst_waves isn't registered yet at precache: treat… | (none of its own) the T1 specs (qa4-t1-plugins-demos.spec.ts, creatures-qa4.spec.ts, sim-replay-qa4.spec.ts) |
| 73bb3fef | Creature Studio, QA4-T1 fixes: Waves start from Last Stand's real round table (all its types by round, own_rounds keeps its boss, pack and heavy… | Rust `saving_waves_keeps_every_last_stand_type`, `spawn_of_a_type_the_map_did_not_load`; Playwright `creatures-qa4.spec.ts` |
| 8bf396a2 | Plugin Studio and Demos UI (QA4-T1): rename and new-name dialogs stay open on an error, with the reason under the field and what was typed kept… | Playwright `qa4-t1-plugins-demos.spec.ts` |
| d1812542 | Plugin Studio and Demos (QA4-T1 T1-4, T1-6): names Windows keeps for devices (CON, NUL, COM1...) are refused in Plugin Studio names and Demos… | Rust `slow_function_notes_are_marked`, `unnamed_array_value` |
| 389ba573 | QA4 T1-5, M5, M2, L8, L11: the map simulator only judges how much of the map bots used on runs of 3+ minutes and 20+ bot-minutes, with a lower bar… | Rust `coverage_judged_by_run_length`, `sources_tell_recordings_apart`; Playwright `sim-replay-qa4.spec.ts` |

### QA4-A (map editor)

| Commit | What | Verified by |
|---|---|---|
| f7d1ddd5 | QA4-A fixes: CS 1.6's own models, sprites, sounds and gfx count as stock without Condition Zero (a list of what Valve's dedicated server ships in… | Rust `cs16_stock_without_czero`; Playwright `editor-filequeue.spec.ts`; node `test-mapname.mjs` |
| 5c3dee1a | Map editor (QA4-A A-1, A-5): the Budget's last compile is the one this document ran, whatever name it was compiled as (each compile notes the… | Rust `last_compile_is_the_documents_own`; Playwright `editor-budget.spec.ts`, `editor-flat.spec.ts` |
| 88e7c630 | Condition Zero learns maps with Loner-Strike's brain: the hidden server brings our mp.dll into every server whose DLL isn't ours (Valve's CZ… | Rust `condition_zero_learns_with_our_brain`; Playwright `files-update.spec.ts` |
| 6ea98b5c | Map editor per-edit cost and QA4 area A self-check: the editor's top component reads only the document id (it re-rendered everything on every… | Playwright `editor-framesplit.spec.ts`, `editor-idle.spec.ts` |

### QA4-B (Game Mode Studio)

| Commit | What | Verified by |
|---|---|---|
| b7ddd814 | Site: fresh map editor and Game Mode Studio screenshots (br_valley, Quad on ls_outpost, Checks, a new Budget section, King of the Hill, the start… | (none of its own) the studio Playwright specs; a playtest by the user |
| 56aba2a0 | QA4-G F6: a shared Studio mode says what it needs: requirements::mode_needs (Loner-Strike's components and which ones, creature types, the… | Rust `a_mode_package_says_what_it_needs`, `needs_list_components_creatures_and_the_brain_versions`, `the_brain_here_from_its_mp_dll`; Playwright `community-needs.spec.ts`; list service `test_community.py` |
| 76f1cfce | One in the Chamber: a round is decided once (QA4-B: a kill in the round-end delay after time ran out ended it again) - a Round Over flag, reset… | (none of its own) the studio Playwright specs; a playtest by the user |
| 8f9bcad3 | Components: Creatures Working (ls_wave_creatures_ok) says whether this server can spawn creatures until a wave table is set up, then whether they… | (none of its own) the studio Playwright specs; a playtest by the user |
| 05c31fb8 | Studio Timelines: the tick says what runs (QA4-B) - 0.1 s is the default and the fastest, AMX Mod X's shortest repeat (0.05 s really ran at 0.1);… | (none of its own) the studio Playwright specs; a playtest by the user |
| 9fba3676 | Requirements and the community page: the maps Loner-Strike ships (ls_outpost, br_valley: plugins/game/maps, in every player's game) are every… | Rust `loner_maps_are_the_shipped_ones` |
| 2377d95f | Studio Test run: with no server on this computer it makes a hidden temporary server from the game's own files again (servers::start_hidden_server… | (none of its own) the studio Playwright specs; a playtest by the user |
| 18d7d6c8 | Studio: a map-fired Custom Event knows who set it off (QA4-B: Player was always nobody) - Ham_Use's first value is the activator (ham_const.inc's… | Rust `map_fired_custom_events_know_who` |
| cb35b8d9 | Node help search: words match at a word's start ("move" no longer finds Remove ...), so 'move a player somewhere' lists Teleport; test | (none of its own) the studio Playwright specs; a playtest by the user |
| f700e50d | QA4-B F4 and F6: Export all / Import read the library from the app first (deleted elsewhere, an import came back as <id>_2); Mode settings shows… | Playwright `studio-ux-x2.spec.ts`, `studio-ux.spec.ts`, `studio-v3.spec.ts` |
| 0183a72a | QA4-B F5: a CSV import never turns a bad value into something else: every row with a value its column can't take (not a number, out of range, not… | (none of its own) the studio Playwright specs; a playtest by the user |
| 6464df1a | Studio snippets: laid out as Tidy up lays them out now (the tidy script's output, unchanged by running it again) | (none of its own) the studio Playwright specs; a playtest by the user |
| 01a834a6 | Map simulator after the user's test (Nav's uncommitted work from 2026-11-13, recovered and committed by X for QA4-B): with no server on this… | Rust `stuck_is_trying_to_move_and_going_nowhere`, `tests_never_save_into_the_real_game`; Playwright `callouts.spec.ts`, `sim-report.spec.ts` |
| 4983def5 | One in the Chamber: a round can't stall any more (QA4: a round with a few players left, out of bullets and far apart, went on for minutes) - a… | (none of its own) the studio Playwright specs; a playtest by the user |
| 7f934a17 | Studio debugger: a timer (Delay, Repeat, Countdown) that comes due while a breakpoint has the world paused keeps its schedule after Resume - it… | Rust `timers_keep_their_schedule_across_a_pause` |
| ec184f06 | Studio Playtest builds: a node's runs are written at most 4 times a second (2 a half second; was 8 a quarter); a Timeline's progress note shares… | (none of its own) the studio Playwright specs; a playtest by the user |
| 38949bf8 | Studio canvas perf (StudioPerfAudit #1-#4): the wire layer on its own compositing layer (WebKit repainted the whole graph-sized layer on every… | Playwright `studio-perf.spec.ts` |
| aa486daa | Flaky Studio specs, the real races: Find uses picked the uses 120 ms later, after the user could already have picked a variable (studio-help… | Playwright `studio-help.spec.ts`, `studio-playtest.spec.ts`, `studio-v3.spec.ts` |
| 0d5c6598 | Studio version history: versions() only for tests (no unused warning) | (none of its own) the studio Playwright specs; a playtest by the user |
| e64863ce | Studio version history (StudioPerfAudit #7): the newest save kept whole, so a save diffs against it with no replay; older saves as changes back… | Rust `a_big_history_drops_its_oldest_past_the_size_cap`, `a_version_one_file_is_read_and_kept_going`, `history_bench` (+1) |
| 0ad5fff2 | Studio Playtest builds and Get Row (StudioPerfAudit #5, #6): a node's run is written at most 8 times a quarter second (the rest counted and… | Rust `get_row_search_by_kind` |
| 92a639ec | Studio: no 'symbol is never used' warning for a node with several flow inputs (a Timeline's Play) when only another input (Play from Start) is… | (none of its own) the studio Playwright specs; a playtest by the user |
| aa1c1970 | Studio UX review (X): H6 inline pin values - an unwired input with its own setting (a number, yes/no, short text or a choice) is typed right on… | Playwright `studio-ux.spec.ts` |
| 5a4bbe95 | Studio UX review (X): M4 a whole-mode variable's Get is a pill; M5 a placed node is picked and a new variable, Function, Macro, struct, enum,… | Playwright `studio-help.spec.ts`, `studio-ux.spec.ts`, `studio-v3.spec.ts` |
| 86b8f755 | Studio UX review (X2): H1 a big graph opens readable on its first part; H3 Requirements compact, recommended maps as chips with a search; H4 side… | Playwright `studio-help.spec.ts`, `studio-ux-x2.spec.ts`, `studio-v3.spec.ts` |
| 2e4ff94f | Studio UX review (X): H2 the mode's own Details come back (empty canvas, Escape, a Mode settings button); H5 node search ranks title matches first… | Rust `nothing_happens_yet`; Playwright `studio-ux.spec.ts` |
| e7127141 | Game Mode Menu: 128 modes (was 32), with a spare slot for a mode taken off while it plays and a log line when modes.ini lists more; the Studio's… | Rust `max_modes_matches_the_menu` |
| 462b4872 | Studio: Timelines run by real game time (each tick steps by the game time since the last, at most 0.5 s after a hitch; a 4 s Timeline took ~9 s on… | (none of its own) the studio Playwright specs; a playtest by the user |
| af17cd85 | Studio Timelines: key interpolation - Linear, Constant (a step), Ease In, Ease Out, Ease In-Out and Custom (a cubic Hermite curve by the keys'… | Rust `bad_tangents_and_eases_are_refused`, `curves_compile_with_amxxpc`, `every_curve_checks_clean_and_compiles_to_small_stocks`; Playwright `studio-help.spec.ts`, `studio-v3.spec.ts` |
| 589ec377 | App: small Docs links to the matching docs section on every Tools page (but the Studio, which has its Help menu), Admins, Shared control,… | (none of its own) the studio Playwright specs; a playtest by the user |
| 2b804682 | Docs screenshots: editor face sheet, I/O chain and leak line (ls_outpost), mode settings, crash tracking, publishing a mode, roles (claim,… | (none of its own) the studio Playwright specs; a playtest by the user |
| 6d5eeac4 | Game Mode Menu: gm_modes_reload reads modes.ini again on the spot (picks kept by id; a mode taken off while playing stays listed, not pickable,… | Rust `replies` |
| 5fb7762c | Studio polish: Promote to Variable (right-click a pin; a Local in a Function), align and space evenly, select what feeds it / what it feeds /… | Rust `diffs_apply_back_to_the_same_graph`, `history_keeps_the_last_fifty_and_skips_the_same`; Playwright `studio-help.spec.ts` |
| c8562711 | Studio: the Battle Royale template's snapshot with the 45 s bleed-out | (none of its own) the studio Playwright specs; a playtest by the user |
| a6115803 | Battle Royale: knocked players bleed out after 45 s by default (br_bleed in the plugin, br.cfg and the Studio template; user-approved), so squad… | (none of its own) the studio Playwright specs; a playtest by the user |
| 76a4d024 | Studio: Mode settings on a Linux server - the first save makes data/studio_settings there (failed before); removing a mode also removes its… | (none of its own) the studio Playwright specs; a playtest by the user |
| 660e1a72 | Studio: Bot Nature and Bot Trait (the brain's skill, aggression, teamwork, greed, playstyle, traits), Bots: Goal 'Hunt it' (W2's hunt goal;… | (none of its own) the studio Playwright specs; a playtest by the user |
| 7ed53fd8 | Docs: the Game Mode Studio for 1.0 - make your first mode, the tour, the node list and node help, event graphs and custom events, Timelines, data… | (none of its own) the studio Playwright specs; a playtest by the user |
| eaef8b97 | Studio: data tables and live tweaks. Data tables (My mode > Data tables): typed columns (number, text, yes/no, enum, weapon, entity name), a key… | Rust `a_clean_table_checks_clean_and_bad_cells_are_named`, `live_tweak_values_are_checked_by_type`, `table_nodes_check_clean_and_generate` (+4); Playwright `studio-v3.spec.ts` |
| 11d282ec | Docs for 1.0: Last Stand upgrades (perks, bench, box, turrets, traps, power-ups) and knock-downs; Battle Royale loot, healing and squads; Studio… | (none of its own) the studio Playwright specs; a playtest by the user |
| 77c560a8 | Studio mode settings: remote servers through the usual remote file path, hosts trusted (out-of-range kept with a warning, the plugin uses the… | (none of its own) the studio Playwright specs; a playtest by the user |
| 2ec9c5dd | Studio: models and sounds a graph names are precached only if the file exists (a missing one used to stop the map loading), logged, and what uses… | Rust `named_files_are_loaded_only_if_there` |
| ca220014 | Studio examples: King of the Hill, One in the Chamber and Juggernaut take the bomb away at Round Started (Remove Weapon c4), so bots play the mode… | (none of its own) the studio Playwright specs; a playtest by the user |
| 361dca58 | Studio mode settings (Instance Editable): a variable marked 'Setting (hosts can change it)' becomes a per-server cvar the mode reads at map start… | Rust `values_are_checked_against_the_manifest` |
| 65a38d15 | Site: Made by players, a view-only peek at Community on the homepage (maps, Studio modes, prefabs: GameBanana pictures, author, kind, modes,… | (none of its own) the studio Playwright specs; a playtest by the user |
| 96531dde | Studio example Juggernaut: the round ends once (a Round Over flag), the Juggernaut variable resets each round (no two at once), and bots are sent… | (none of its own) the studio Playwright specs; a playtest by the user |
| 96b757e2 | Mode requirements: what a Studio mode plays on and needs. requirements.rs keeps the default and recommended maps and content links beside the… | Rust `a_modes_page_says_what_it_plays_on_and_needs`, `bad_requests_are_refused`, `content_is_found_in_any_node_setting` (+1); list service `test_community.py` |
| d5de5709 | Studio: a cost hint for Timelines (copies x ticks a second: one per player in a Function with a Player input is up to 32 copies; warns over 200… | Rust `timeline_cost_counts_copies_and_ticks` |
| f3a3ba80 | Studio: Personality influence (0-100, 50 as before) on Bots: Goal, Offer, Phase, Prefer Layer and Bot Follow, sent as W2's ls_mode_ver 2 argument;… | (none of its own) the studio Playwright specs; a playtest by the user |
| bc82e620 | Studio node help: Set Setting and Remove Weapon say the bomb can't be turned off with a setting (game.cfg sets mp_give_player_c4 again) and to… | (none of its own) the studio Playwright specs; a playtest by the user |
| 7c7e0585 | Studio: a better delete question for variables, locals, Functions and Custom Events in use - how many nodes use it and where, Find uses, and… | Playwright `studio-help.spec.ts` |
| 85017e2d | Studio: Get Entity Transform (position, rotation, velocity, box, on ground) for any entity, Get Entity Position / Rotation, Distance Between,… | (none of its own) the studio Playwright specs; a playtest by the user |
| 6059738b | Studio creature control: For Each Creature, Creatures of Type, Nearest Creature, Creature Type, Idle For, Can See, Go To (guard or resume),… | Rust `ammo_reads_the_active_item`, `the_bomb_setting_is_allowed_and_the_list_still_holds` |
| 798ea682 | Studio example One in the Chamber: a player who joins mid-round no longer keeps the round from ending (still in = lives left and alive or coming… | (none of its own) the studio Playwright specs; a playtest by the user |
| 115d83ea | Studio Timelines: values over time (number, position, color tracks) and events at set moments, a Timeline node (Play, Play from Start, Stop,… | Playwright `studio-v3.spec.ts` |
| 6ca119bc | Studio examples: undo King of the Hill's bomb setting (it went into 26b00e76 half-done; the Studio doesn't allow mp_give_player_c4 yet); notes to X2 | (none of its own) the studio Playwright specs; a playtest by the user |
| 26b00e76 | Studio example Gun Game Lite: 14 guns before the knife (a game took 40 s with 12 bots), 2 seconds of God Mode on spawn | (none of its own) the studio Playwright specs; a playtest by the user |

### QA4-C / W1 (Last Stand, creatures)

| Commit | What | Verified by |
|---|---|---|
| 377a0a42 | Setup copy: mention Last Stand, Battle Royale and making; nav pack maps tip; Make tip; Privacy policy wording; Mac docs link | (none of its own) sim laststand; the client part by the user |
| c51cf442 | QA4-C fixes: bots upgrade their primary at the bench/armory (a person still upgrades what they hold); a wedged boss or heavy is never just removed… | (none of its own) sim laststand; the client part by the user |
| 2b7e651f | Downed state: revive goals re-sent every 3 s where the downed player lies now; with squads to every living squad mate within 2500 u (Last Stand:… | (none of its own) sim laststand; the client part by the user |
| a7ef360f | Mode vote: keeping a mode with maps of its own (Battle Royale, Last Stand, ...) offers only that mode's maps (it used the everyday map vote, and a… | (none of its own) sim laststand; the client part by the user |
| fe2c3b71 | Downed state: with ls_down_finish a downed player isn't hidden from bots (enemies can finish them; BR test: nobody was ever finished); Last Stand… | (none of its own) sim laststand; the client part by the user |
| e0d92b1d | Downed state: a revive bonus - getting a teammate up pays ls_down_revive_pay (once a downing, not for getting yourself up, bots too, '+$N… | (none of its own) sim laststand; the client part by the user |
| f38dd978 | Last Stand economy and knock-downs: perks first (no box or turret offers to a bot with fewer than 2 perks, the box once a round), the bench before… | (none of its own) sim laststand; the client part by the user |

### QA4-D / W2 (Battle Royale, downed)

| Commit | What | Verified by |
|---|---|---|
| 23e73883 | Battle Royale: bots meant for the high ground take only guns and crates up there in the loot-up (none up there: no gun goal, the brain takes them… | (none of its own) sim br; the client part by the user |
| 0ec9a400 | Battle Royale: a squad with nobody standing is out at once - its downed players go, the kill to whoever knocked them (QA4-DE F4); the game's… | (none of its own) sim br; the client part by the user |
| 7ce4d619 | For W2: ls_mode down <id> 1/0 when a player goes down / gets up, dies, respawns or leaves; a bot that hasn't come within 300 u of a gate 9 s after… | (none of its own) sim br; the client part by the user |

### QA4-E (Deathrun, mode vote)

| Commit | What | Verified by |
|---|---|---|
| 584eb81d | Deathrun: rounds end at time-up with the runners losing (mp_roundover 2, as deathrun servers play it; with no bomb or hostages a round never ended… | (none of its own) sims deathrun, modevote |
| e7c0e35f | Deathrun: drop deathrun_starwars from the mode's map download and rotation (user: not supported for bots in 1.0, still playable by hand); QA rows… | (none of its own) sims deathrun, modevote |
| a4714121 | Deathrun: drop deathrun_jigsaw2 from the mode's map download and rotation (user: not supported for bots in 1.0, puzzle-gated like dojo); QA4 row… | (none of its own) sims deathrun, modevote |
| cf157dbc | Deathrun: drop deathrun_dojo from the mode's map download and rotation (its next area needs the activator to trigger a trap; not supported for… | (none of its own) sims deathrun, modevote |

### QA4-F / Y (crashes, servers, setup, updates)

| Commit | What | Verified by |
|---|---|---|
| 72d42c46 | Crashes (QA4-F F1, F2, F3, F9): ls_crash.log is read as one crash per entry (each counted and posted to Discord once; an older brain's double… | Rust `a_crash_log_holds_one_entry_per_crash` |
| e315c267 | Servers and the nav learner (QA4-F F6, F8): on a Mac, ending a server or the hidden learner also ends its Windows-side hlds.exe (by its own… | Rust `wine_side_ends_a_leftover_server`, `wine_side_is_found_by_its_windows_path` |
| ae80eca1 | Update path fixes (QA4-F F5, F7): a server made before 1.0 gets panel_rcon.txt from its own rcon_password (else a new one) at app start and before… | Rust `a_stripped_server_download_is_never_taken_for_whole`, `panel_rcon_for_servers_made_before_1_0` |
| 3f22e988 | Servers: relocalizebugfix is no longer installed and is taken out of Metamod's list (built for ReHLDS API 2, it never loaded on ours: 'fail' in… | Rust `relocalizebugfix_taken_out` |
| 567e6f8d | Servers: the engine install takes any progress (a test runs it on a copy of a server: our ReHLDS in, the replaced engine kept in .valve-engine) | Rust `engine_install_on_a_copy` |
| 54e0addd | Foundation: a network test that the pinned ReHLDS comes through the app's download path (mirror first, SHA-256 checked) with both engines | Rust `pinned_rehlds_downloads_with_both_engines` |
| 3f1ee9e7 | Pin ReHLDS 3.15.0.896-ls8 (RCON password masked in the console and logs, sv_rcon_mask_password) | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |
| 1d05fb58 | Tests: a documentation address (203.0.113.10, RFC 5737) instead of the real box's IP in servers.rs and hltv.rs | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |
| b217e6e3 | Voice files channel: gm_extras looks every 0.2 s and says the voice feed's value in status.txt; ls-voice says plainly when the game doesn't answer… | Rust `a_folder_that_cant_be_written_says_so_once` |
| e6f2b14a | Site #get UX pass: Download is the primary box (this computer's button gold, the other an outlined button that's clearly clickable; Windows by… | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |
| 90ecb741 | No RCON from ls-voice: its voice feed, its lines for the bots and its look at who's here go through files gm_extras reads and writes… | Rust `requests_and_status_through_the_folder` |
| d83b320f | Roles /link: requests and answers go through files (gm_extras writes lslink/requests.txt, the relays answer in lslink/answers.txt), so it works… | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |
| 2c0deffe | gm_vip: the kept slots set again after AMX Mod X's adminslots resets sv_visiblemaxplayers on every connect and leave | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |
| dac22813 | gm_vip: the kept slots also set 5 and 15 s into a map and when someone joins | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |
| 2532f29f | gm_vip: the kept slots set again every round (a map change put sv_visiblemaxplayers back to -1 on prod) | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |
| 973d27b6 | Roles relay: read /link lines from qconsole.log (servers run with -condebug and log files are off on prod), else the newest logs/*.log | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |
| 78455587 | Roles: lsmod.py roles supporter-remove / supporter-restore (logged; the Discord Supporter role goes at once and the hourly check never re-adds it;… | list service `test_roles.py` |
| d073ed9b | Thanks page for the Support Pack: Thanks, you're a Supporter; connect Discord on the Polar receipt (with the invite); /link in game then /link… | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |
| b782545b | Terms: the Support Pack paragraph (an optional purchase: a Discord Supporter role and cosmetic perks on official servers, no app features or… | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |
| b9b7e501 | Site get section: Download for Windows and Download for Mac (this computer's highlighted, both kept), fixed-name copies on dl.lonerstrike.com… | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |
| d21fbb4e | ls-control: community roles run as their own unit (ls-control roles, lonerstrike-<id>-control-roles) with internet for the list service; the… | Rust `the_spool_passes_on_only_an_admins_calls` |
| b4091837 | Community roles on Linux servers: ls-control relays /link (curl with its settings on stdin), keeps supporters in users.ini, makes application… | Rust `community_roles_for_admins_here`, `curl_gets_everything_on_stdin_quoted`, `link_lines_and_supporters` (+1) |
| b9fa4f70 | Community roles, the host's app: claim (ls_claim over RCON, token kept 0600, copied to a Linux server's ls-control), settings, owner code, unclaim… | Rust `community_roles_for_admins_of_their_server_only`, `fail_words_and_status`, `link_lines` (+3) |
| 5bde6476 | Windows: big folders (backups, cache, steamcmd, voice and chat models, compile work, recordings, logs) in Local AppData, moved once from Roaming… | Rust `a_bad_copy_keeps_roaming`, `a_copy_is_checked_before_roaming_goes`, `a_link_stops_the_copy` (+4) |
| d1d38e35 | Open in Loner-Strike: loner-strike://community/<mod/spray>/<id> links (deep-link plugin; checked strictly, kept for the app's page, which opens… | Rust `a_link_waits_until_the_page_asks`, `only_a_community_creation_link_is_understood` |
| 7319d3a2 | App fonts work offline: Barlow Semi Condensed and Big Shoulders Stencil bundled (OFL texts beside them), no Google Fonts; Bahnschrift… | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |
| cded6d33 | QA4: areas F and G self-check (MCP live over the stdio bridge, crash files, site at phone width, legal, community and list service tests); ignored… | Rust `real_crash_files` |
| 1dca9abb | Windows, round 2 (docs/notes/Windows.md): crashed servers exit at once (SetErrorMode at start, inherited by every child: no 'has stopped working'… | Rust `a_port_another_program_holds_isnt_free`, `a_reused_pid_started_later_isnt_ours`, `ends_only_the_process_it_started` (+3) |
| 273c64e1 | Windows audit (docs/notes/Windows.md): saves retry on Windows' in-use errors (32/33, not only access denied); setup copies over read-only files… | Rust `both_libraryfolders_formats`, `lock_errors_that_clear_by_themselves`, `read_only_files_can_be_made_writable` (+3); node `test-windows.mjs` |
| d559c7e8 | Play shows 'Restored Loner-Strike's fixed Ham Sandwich' as a quiet toast when the start put it back; map editor frame-time readout (View > Show… | node `test-frametime.mjs` |
| 9be4db0b | Fixed Ham Sandwich kept in place on this computer: before every Play and every local Windows/Mac server start the module's hash is checked against… | Rust `an_install_that_ships_a_ham_module_keeps_ours`, `ours_is_left_alone_and_no_module_is_nothing_to_do`, `the_official_module_is_replaced_before_play_and_kept_once` |
| 7aa95dc5 | Linux servers: crash records - lscrashinfo writes crashes/modules.json before every start (Ham Sandwich fixed / official / unknown) and… | (none of its own) the Rust suite; Windows and Linux-server parts by the user (QA4.md H) |

### QA4-G (Community share and import)

| Commit | What | Verified by |
|---|---|---|
| e0ece72a | Community list, creature files: a creature-carrying package ships none of the games' own files (the app's stock lists, copied to stock/ and… | list service `test_creatures.py` |
| 564fc781 | Community list, creatures.ini rule as the app packs it: model lists models/....mdl paths and snd_* keys .wav paths relative to sound/ (several,… | list service `test_creatures.py` |
| f2d1c7d4 | Caps test: the screenshot message names the file now (QA4-G F3) | (none of its own) list-service tests; the live list by the user |
| c1f298ac | QA4-G fixes: map packages carry no game files (a test publishes a stock-only map, desert sky and hostage.mdl, with CS 1.6 only and with both… | Rust `a_stock_only_map_ships_no_game_files`, `missing_files_are_named`, `screenshots_named_and_other_game_asked` |
| 98b24f60 | List service: a GET (or anything but a POST) on /roles/polar/webhook says 'This endpoint takes Polar's webhook POSTs.' (405), not 'server isn't… | list service `test_roles.py` |
| 79962a3d | Community roles UI: Loner-Strike roles card on a server's Shared control (claim, Discord owner code, applications/Discord-moderators/supporters… | Rust `roles_routes_ask_before_access`; node `test-roles.mjs` |
| 2b28faaa | List service: roles.db turned on in the unit, installed, backed up daily; env example for POLAR_WEBHOOK_SECRET, DISCORD_SERVERMOD_DECIDERS,… | list service `test_backup.py` |
| 8bdaecb4 | Community roles, service side: server claim (ls_claim via A2S), Steam-Discord link codes (/link, /unlink, /server claim), Polar supporter webhook… | list service `test_roles.py` |
| e4b55d53 | Community security review of 4a178c08: unlisted entries are re-checked like listed ones (untagged, flagged, a virus result or GameBanana… | list service `test_community.py` |
| 55fe7184 | Site: Made by players intro and empty state reworded (what it is, where it's hosted, how to be first); link says How sharing works | (none of its own) list-service tests; the live list by the user |
| 186df8cb | Site: Made by players shows only explicitly listed creations (unlisted, withdrawn, hidden, pending, revoked, unknown states and anything on the… | (none of its own) list-service tests; the live list by the user |
| 4a178c08 | Community: unlisting and withdrawn pages. The author unlists (and lists again) from Yours with the page's own proof, as for registering: the list… | Rust `unlisted_and_withdrawn_stay_out_of_lists_and_get_no_updates`; list service `test_community.py` |
| 710b6701 | Community risk confirmation bound to what was shown (security review): the page sends back the exact risk codes its question named; Try it always… | Rust `a_risky_mode_install_names_its_risks_in_the_question`, `the_install_is_bound_to_what_the_player_saw` |
| 540e430e | Community: a mode's risky nodes in plain words - a badge each on the card and the details ('Runs server commands', 'Runs AMX Mod X commands',… | Rust `a_mode_that_runs_server_commands_shows_it_and_asks_first`, `risk_words_follow_the_studio`; list service `test_community.py` |

### Community roles and Support Pack

| Commit | What | Verified by |
|---|---|---|
| 62af673f | Site: drop the 'yours for good' promise for Supporter perks (user) | (none of its own) list-service test_roles.py; live purchase and /link rows by the user |
| bb48b6b5 | Site: drop 'in gold' from the Support Pack perks | (none of its own) list-service test_roles.py; live purchase and /link rows by the user |
| 59baed0f | lsmod.py roles official: the server argument no longer overwrites --server | (none of its own) list-service test_roles.py; live purchase and /link rows by the user |
| f40f709f | Site: the Support Pack's checkout links (one-time and monthly); the monthly line shows now | (none of its own) list-service test_roles.py; live purchase and /link rows by the user |
| 478f6b6b | Site: the Loner-Strike Support Pack (what you get first: the Supporter role, [VIP] tag, reserved slot and join message, the apps; perks for good;… | (none of its own) list-service test_roles.py; live purchase and /link rows by the user |
| 9205c8c8 | Support section and docs match the final Polar description (side project, Supporter for good, Discord on the receipt page, /link in game then… | (none of its own) list-service test_roles.py; live purchase and /link rows by the user |
| b1599ec9 | Roles: the Supporter role's givers recorded (role_granted: Polar and our bot), so stale roles go once they don't qualify (an old account after a… | list service `test_roles.py` |
| ced20c90 | Roles: our bot keeps the Discord Supporter role as our rules say (back after Polar's revoke for supporters for good, off after a full refund; on… | list service `test_roles.py` |
| fa2c70e8 | Roles: only POLAR_SUPPORTER_PRODUCTS (the one-time and monthly support products) make a Supporter, from orders and grants; other products logged… | list service `test_roles.py` |
| 44ed9b88 | Site Support: the monthly line links to POLAR_CHECKOUT_MONTHLY and stays hidden until it's set; docs don't mention monthly yet | (none of its own) list-service test_roles.py; live purchase and /link rows by the user |
| a73809f6 | Roles: subscription payments count like one-time donations (same minimum; with POLAR_SUPPORTER_DAYS 0 the first makes a supporter for good and… | list service `test_roles.py` |
| 486611e8 | Roles: each donation keeps the Discord account it counted for (refunds and account changes work that account out again); a donation counts only… | list service `test_roles.py` |
| d90ed2c0 | Roles: Polar order.refunded (a full refund of a one-time donation takes its days off, worked out again from the donations that count; partial… | list service `test_roles.py` |
| 009c4339 | Roles: one SteamID form per account (STEAM_0, old rows made canonical at start), unclaim and re-claim revoke like revoke() and list who for the… | list service `test_roles.py` |
| 447dec74 | Roles: a server claimed again starts from nothing (not official, default settings, open applications closed, moderators revoked, the old server… | list service `test_roles.py` |

### Site and docs

| Commit | What | Verified by |
|---|---|---|
| 5e5202a3 | Site and editor: TrenchBroom import said where the importers are listed (docs feature list, the editor's start buttons, the import page's heading… | (none of its own) by eye (site not deployed) |
| 76d50844 | Public-repo cleanups: no home paths in health.rs and the docs (relative or generic paths); the Discord IDs in community.env.example blank with how… | (none of its own) by eye (site not deployed) |
| 2f0e24c2 | Site: support cards all the same size, last row centered (no stretched card) | (none of its own) by eye (site not deployed) |

### Other

| Commit | What | Verified by |
|---|---|---|
| b8ee5f19 | Release prep: no box address or owner id defaults in the live QA harness (LS_BOX required), made-up Discord ids in modbot tests, gitleaks:allow on… | list service `test_modbot.py` |
| 7319babb | QA: live test harness (services/serverlist/qa/live.py: private QA-tests category and #qa-live channel, QA-Tester role, bot checks, test list… | (none of its own) the full suites |
| c5ef240d | Editor small-window fixes from Parity A's sweep: in short panes (a small window's 1+3 or Quad) the axes triad is smaller, and gone in the… | Playwright `editor-content.spec.ts`, `tools.spec.ts` |
| 0ccbc425 | Editor specs (extrude, 2D views, shapes): run at 1440x900 in every project; the small project's 1024x680 leaves 2D panes about 120 px tall, too… | Playwright `editor-extrude.spec.ts`, `editor-flat.spec.ts`, `editor-shapes.spec.ts` |
| 38459fdb | Play: one updates bar for both games (Updates waiting: Counter-Strike 1.6 (6 parts) and Condition Zero (9 parts)); Update all does each game in… | Playwright `files-update.spec.ts` |
| afa84c2d | Publishing a mode: playtest screenshots (F5, the game's own screenshot key, during a playtest: the form lists the last two weeks'… | Rust `recent_snapshots_become_pngs`, `snapshot_names`; node `test-cover.mjs` |
| 5c2b80b6 | plugins.ini: Loner-Strike's own plugins run with AMX Mod X's debug flag (run time errors name the line), added to lines already there | (none of its own) the full suites |
