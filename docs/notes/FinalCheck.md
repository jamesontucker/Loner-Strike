# Final Studio and editor check (with the performance pass)

P5, prepared 2026-11-14. It runs once X's type system (graph v4, new types, orphaned pins) has landed,
when the coordinator says so. After it: merge `tools` into the main checkout, then the draft build.

## Versions: the latest of every piece (the user's rule)

Everything the check runs is built or refreshed from current `tools` HEAD. A fresh APFS clone of the sim
base gets all of it put in; LSPLAY and BRPLAY are never used as they are. `scripts/finalcheck.mjs` builds
the table at start, puts it at the top of its `summary.md`, and **refuses to run if any row is older than
HEAD** (`--allow-old` exists only for testing the harness).

| Component | Where it comes from | Checked at start |
|---|---|---|
| Source | a worktree of HEAD (`git worktree add --detach`), never the working copy (other agents' edits aren't in it); the UI built there too | the commit |
| Bot brain | the newest green **ls-next3** CI build (the ls22 candidate), from `gh run list -b ls-next3 -s success`, cached in `~/loner-strike-sim/cache/r<run>` | its CI run and head commit; whether it's the branch head; the mp.dll's sha256; **the sim's `ls_mode_ver` read over RCON at start, must be 4+** (an older brain prints its ls_mode usage list for the newer commands: W2's finding) |
| gm_* plugins, gm_components, BR, Last Stand, Deathrun, the game menu | every `.sma` in HEAD's `plugins/scripting`, compiled with amxxpc into the clone | each of our plugins in the sim must be built by this run (file time), else refused |
| Mods' plugins | compiled from HEAD's catalog parts for the mods the base has; their data and configs from HEAD's `mods/` | compile failures refuse the run |
| Studio plugins | HEAD's generator (`finalcheck_install` built in the worktree, so graph v4 after X lands) | made and compiled this run |
| Navs and map knowledge | the live nav pack (`dl.lonerstrike.com/navs/list.json`): every map the sim has in the pack's version gets the pack's .nav and .lsv | every pair's sha256 must match the list |
| App for the editor and Studio specs | the same HEAD worktree: `serve_tools_dev` and Vite run from it (see the run commands) | the commit |

The run's version table (component → commit, CI run or sha256) goes here once the real run starts.

What the user asked for (feature freeze notes):
- every node added this round working in a game;
- realistic load, 16–24 bots: server frame time, CPU and plugin cost against Classic;
- the precache budget warnings;
- big-graph Studio responsiveness;
- janky graphs (infinite spawning on a fast On Tick, nested loops, per-frame effects) hit caps with a
  clear message and never crash, kick or overflow anyone: people may experiment freely.

## What's ready

| Piece | Where |
|---|---|
| The graphs, kept as **version 3** so the run also goes through the v4 migration | `scripts/finalcheck/graphs/` (written by `scripts/finalcheck/make_graphs.py`; don't re-run it once the check has started) |
| Installer: the Studio's own code writes each graph's plugin, include files, modes.ini line and plugin list into a sim copy | `app/src-tauri/src/tools/studio/finalcheck_tests.rs` (`finalcheck_install`, ignored) |
| The game harness: clone, compile, install, Classic vs Studio cost, coverage, the janky set, BR squads | `scripts/finalcheck.mjs` |
| Studio UI on the biggest graph it opens (400 nodes) | `app/ui/tests/studio-final-perf.spec.ts` |
| The node kinds this round added (what coverage counts): 163 since 3782f0ae | `scripts/finalcheck/graphs/new_kinds.json` |

### The graphs

| File | Mode | What |
|---|---|---|
| `cover_w1`, `cover_ec`, `cover_p5`, `cover_3`, `cover_5`, `cover_fx`, `cover_4` | `fc_cover_*` | The node agents' every-node test graphs as HEAD has them: the fewest that hold all 163 new kinds. Unset settings are filled in as the Studio fills them on add, and Objective Marker and Effect Burst get a position. Without that, every_node_3 fails the Studio's check: it was made to compile, not to be a mode. Built with the debugger, so node runs are written down. |
| `perf_tdm`, `perf_ls` | `fc_perf_*` | The Team Deathmatch (7 nodes, v2) and Last Stand (71 nodes) templates, as realistic modes for the cost runs. The cover modes are measured too, as plain builds. |
| `stress_spawn` | `fc_st_spawn` | On Tick at 0 s (every frame, the tick limit at 1000). For each living player: Spawn Entity, Spawn Prop, Spawn Projectile (60 s life, trail), Spawn Trigger Volume, Objective Marker, Loop Sound, Trail. Nothing is removed. Expected caps: entity slots, projectiles 512, trigger volumes 256, markers 256, loop sounds 256, trails 256. |
| `stress_loops` | `fc_st_loops` | Two For Loops at the cap (10,000 each) nested, every second: 100 million runs a second. |
| `stress_loop1` | `fc_st_loop1` | One For Loop at the cap on every tick (what people really make). |
| `stress_fx` | `fc_st_fx` | Every tick, for every player: beam, debug line, light, ring, burst, sprite, tracer, color chat, big HUD, 2D sound, chat. Effect messages a frame at 160. |
| `stress_burst` | `fc_st_burst` | 10,000 × (color chat, big HUD, 2D sound) in one frame at round start. |
| `stress_precache160` | `fc_st_pre160` | 110 sounds and 50 models of its own: the most a mode may load. Played on de_dust2 and on cs_italy. |
| `stress_precache170` | `fc_st_pre170` | 170 files: the Studio must refuse it ("the most is 160"). It isn't installed. |
| `big` | — | The cover graphs side by side, cut to the 400-node cap: 400 nodes, 536 wires. UI only, never compiled. A graph holds at most 400 nodes (graph.rs `MAX_NODES`, schema.ts), so "500+" isn't possible today; see the dry-run findings. |

Every stress graph has every comfort limit (`lim_*`, read from limits.rs) set to its max.

## How each part passes

**Every new node in a game** (`cover`).
- Each `fc_cover_*` mode is switched to with `gm_mode` on de_dust2 with 16 bots.
- Each plays `--secs`, then a round restart and 30 s more.
- The switch must take: current.txt names the mode, `amxx plugins` lists the plugin as running, and the
  bot count is reached.
- Pass:
  - no run-time error and no Host_Error;
  - every new kind that only needs bots ran at least once (from the debugger's
    `data/studio/<mode>.events`);
  - each "never ran" kind is explained: it needs a person (Player Command, Tell Player To Bind, ping),
    a map entity de_dust2 doesn't have, and so on.
- Node errors (the debugger's warnings) are listed for the owner to read.

**Cost against Classic** (`perf`), at 16 and 24 bots, maxplayers 32.
- Classic first, then each `fc_perf_*` and `fc_cover_*` mode as a plain build.
- After the switch, 15 s to settle, then 120 s of samples every 2 s:
  - `stats` gives FPS, from which comes frame time (mean and p95), and the server's CPU;
  - `ps` gives the hlds process's CPU.
- The table gives each mode's difference from Classic at the same bot count. That difference is the
  mode's plugin cost; AMXX has no per-plugin profiler.
- Pass:
  - the realistic modes (`fc_perf_*`) stay within 10% of Classic's frame time and within +15 points of
    process CPU;
  - no mode takes the server under 60 fps at 16 bots.
- A cover mode over that is reported with its busiest node, which the Studio's own Test run shows.

**Janky graphs** (`stress`), 16 bots, 120 s each.
- Pass:
  - the server answers RCON throughout: the longest silence is under 10 s (more than that is a hang);
  - no crash, Host_Error, "no free edicts" or ED_Alloc;
  - no bot dropped;
  - no overflow line;
  - the expected caps show up in the log as the plugin's own lines ("… skipped: the server is out of
    entity slots", "… skipped: too many effects this frame", "… skipped: as many are out as … allows",
    "… not loaded: the mode has loaded 160 files of its own already").
- `fc_st_pre170` passes when the install refuses it with the "the most is 160" message.
- **Overflow needs a real client.** Bots get no network messages, so a reliable-channel overflow can't
  show up with bots only. With `--human` the harness waits for a person to join from the game
  (`connect 127.0.0.1:27520` in the console) and checks they're still connected after every stress run.
  That join is the user's or mine, and the game opening on screen needs the user's OK first.
- **Expected findings to confirm:**
  - `fc_st_loops`: the loop cap is per loop. Nested loops multiply (10,000 × 10,000), and AMXX can't stop
    a running plugin, so this probably hangs the server. If it does, the fix belongs to whoever owns the
    loop nodes (X; gen.rs `flow.for_loop`, LOOP_LIMIT): a budget per event run, not per loop, plus a
    Problems warning for nested capped loops on fast events.
  - `fc_st_pre160` on a busy map: the plugin counts only the mode's own 160 files. The engine's 512
    models or sounds is the real limit, and past it the server stops with a Host_Error. cs_italy plus 160
    shows whether that's reachable. If so, the plugin needs to check the engine's count before each
    precache (gen.rs `lsg_pre_room`).

**Battle Royale squads** (`br`).
- It runs on the same fresh sim, so it gets the same brain, plugins and navs, with the re-check's
  `br-squads16` criteria: `gm_mode br_squads br_valley`, 16 bots, up to 8 minutes.
- Pass: a game starts, then first contact, then a knock, an out or a kill.
- The re-check runner itself now prints the sim's `ls_mode_ver` and takes `RECHECK_BRAIN_DLL` for a
  brain other than the pin. It also fails a step whose mode didn't switch (current.txt is checked after
  `gm_mode`).

**Studio UI, 400 nodes (the cap)** (`studio-final-perf.spec.ts`, WebKit).
- It times open, pan, a node drag, a wire drag and connecting a wire, at zoom 1 and zoomed out.
- Each undo must be painted and must put things back.
- Pass (`FINAL_PERF_ASSERT=1`):
  - opens in under 5 s;
  - a node drag takes under 20 ms a step at zoom 1 and under 34 ms zoomed out;
  - each undo is painted in under 300 ms;
  - no page errors.
- `studio-perf.spec.ts` (every_node, 400 nodes) runs too, for comparison with the PerfAudit numbers.

**Editor and the rest of the Studio.**
- Studio specs:
  - `studio-v3`, `studio-ux`, `studio-ux-x2`, `studio-help`, `studio-npc`, `studio-playtest`, `studio-timeline-layout`;
  - X's new type-system specs, whatever they're called when they land;
  - `studio-perf` and `studio-final-perf`;
  - in Chromium and WebKit.
- Map editor: the coverage sweep as `docs/notes/EditorCoverage.md` "Rerunning before a release"
  describes. That means `editor-coverage.config.ts` over a sim copy, with throwaway app data, and the
  `.lsmap` and `.lin` passes too.
- Other editor specs: `editor-budget` (the map's precache budget warnings), `editor-checks`,
  `editor-checks-webkit`, `editor-stock-entities`, `editor-perf`, `editor-bigmap`, `editor-flatperf`,
  `npc-editor`, `sprite-maker`, `creature-picker`.
- Rust:
  - `cargo test tools::studio` (gen, graph, limits, compat_v3, help, examples, starter, proj, timeline);
  - `cargo test editor`;
  - `generated_plugins_compile_with_amxxpc`.
- **Precache warnings, Studio side** (limits.rs `PRECACHE_WARN` 64, `PRECACHE_MAX` 160):
  - the install table shows each graph's count;
  - `fc_st_pre160` must warn and install;
  - `fc_st_pre170` must be refused;
  - a cover graph over 64 must warn.
- **Precache warnings, editor side:** `editor-budget`.
- **Example modes:** the compat suite (`compat_tests`) builds every template, example mode and library
  premade; Test run on two of them in the app.

## Run commands

Each runs outside the sandbox (CrossOver). The machine's rules: load under 20 (the harness waits), 8 GB
free disk (it stops), at most 3 servers. The harness runs one server (its fresh clone, BR included),
stops it by PID and deletes the clone at the end (`--keep` leaves it).

```sh
cd ~/loner-strike-tools

# 0. after X lands: the graphs stay as they are (v3, on purpose). Check the installer still reads them:
cd app/src-tauri && nice cargo test -j 6 --bin loner-strike finalcheck_install -- --ignored --nocapture; cd -
#    (with no LS_FC_GAME it stops at once; the full run below installs them for real)

# 1. the game half, full length (about 2.5 h: install 10 min, cover 7 x 4 min, cost 2 bot counts x
#    10 modes x 2.5 min, janky 8 x 2.5 min, BR up to 10 min)
node scripts/finalcheck.mjs --out /tmp/ls-finalcheck
#    --human           wait for a person to join before the janky runs (the overflow check)
#    --brain newest|pinned|<mp.dll>   default newest: the newest green ls-next3 build (ls_mode_ver 4+)
#    --steps cover,perf,stress,br --reuse --keep   reruns on the same clone (still version-checked)
#    results: /tmp/ls-finalcheck/summary.md, results.json, a log per run

# 2-4 run from a worktree of HEAD (the app from HEAD, not the working copy):
WT=/tmp/ls-fc-wt; git worktree add --detach $WT HEAD && ln -s ~/loner-strike-tools/app/ui/node_modules $WT/app/ui/
mkdir -p $WT/app/src-tauri/resources/linux/ls-voice $WT/app/src-tauri/resources/linux/ls-intent
(cd $WT/app/ui && npx vite build)
export CARGO_TARGET_DIR=~/loner-strike-tools/app/src-tauri/target LS_APP_DIR=$TMPDIR/fc-appdir LS_DEV_PORT=8391 LS_VITE_PORT=5391

# 2. Studio UI at the cap, in WebKit (own ports, throwaway app data)
cd $WT/app/ui && LS_WEBKIT_ALL=1 FINAL_PERF_ASSERT=1 npx playwright test studio-final-perf studio-perf --project webkit

# 3. the Studio and editor specs (Chromium + WebKit), then the editor coverage sweep (EditorCoverage.md)
cd $WT/app/ui && LS_WEBKIT_ALL=1 npx playwright test studio- editor-budget editor-checks editor-stock-entities \
  editor-perf editor-bigmap editor-flatperf npc-editor sprite-maker creature-picker --project large --project webkit --workers=2

# 4. Rust
cd $WT/app/src-tauri && nice cargo test -j 6 --bin loner-strike tools::studio && nice cargo test -j 6 --bin loner-strike editor

# afterwards
cd ~/loner-strike-tools && git worktree remove $WT; git worktree prune
```

## Dry runs (2026-11-14)

See the end of this file.
