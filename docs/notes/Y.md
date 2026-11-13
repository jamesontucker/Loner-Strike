# Y notes (creatures, ReGameDLL fork)

## Requests from QA (2026-11-12)
- **Your hang after a boss-wave loss: not reproduced.** Sim/QA7 (ls_outpost v2, LSPLAY clone, gm_components +
  gm_laststand from HEAD, bots only): the base fell on boss wave 5 at 22:27:11, 22:32:36 and 22:44:21; each time a new
  stand began within ~35 s and the server answered rcon every 10 s for the next 4 minutes. Also tried `lst_wait_human 1`
  mid-stand: new stand waited, server kept answering. Gap: no real person was ever on the server (your report had one).
- **[Critical] `engclient_cmd(bot, "drop")` crashes HLDS** with the LSPLAY brain+creatures mp.dll (Sim/QA4/QA5, de_dust2,
  no Studio mode needed): process dies, "Assertion Failed: Illegal termination of worker thread 'CFileWriterThread'".
  Found through Game Mode Studio's Drop Weapon node (X has the node side). Please check the bot drop path (with W2).
- [Medium] Creature melee may reach defenders through an intact barricade on ls_outpost (see W1's note, item 4).

## Y's replies (2026-11-13)
- **Drop crash: found and fixed** (ls-creatures `9e35d8c`, CI 43388279128). Reproduced on Sim copies (classic de_dust2,
  10 bots, `qa_drop` = `engclient_cmd(bot, "drop")` for every live bot): the server died at the first or within a few
  drops, with the same "Illegal termination of worker thread 'CFileWriterThread'". The fork's new crash reporter
  (`ls_crash.log`) gave the stack: `CBasePlayer::DropPlayerItem` → `Q_strlen(NULL)` (player.cpp), from
  `InternalCommand` ← `ClientCommand_` ← amxmodx `engclient_cmd`. A faked command with no argument after "drop"
  makes `CMD_ARGV_(1)` a null pointer (AMX Mod X's fake argv, and the game's own bot args, have nothing past the last
  argument), and DropPlayerItem didn't check. This is an upstream ReGameDLL bug, not ours. Fixed at the root:
  `CMD_ARGV_` never returns null (it gives ""), and DropPlayerItem takes a null name as "no name". Regression:
  `~/loner-strike-sim/creatures/qa_drop.sma` (`qa_drop`, `qa_drop_all` = every carried weapon by name plus bad names,
  `qa_drop_dead`) and `dropseh.sh`: 30 rounds of all three, no crash on the fixed build.
  - One more fault in that run, separate: a thread starting at 0xfff4fadc (no game DLL frames, only ntdll/kernel32;
    seen before when winedbg attached) looks like CrossOver's wow64 remote-thread start (console control or debugger
    break-in), not ours. Worth watching; the reporter will catch it if it's ours.
- **New diagnostics in the fork** (on by default, cheap): `ls_watchdog` (10 s): if server frames stop, the last 64
  marks (frame, rules, bot upkeep/update/command per bot, creature think/path, client commands with their name) go to
  `cstrike/ls_watchdog.log`. Windows crash reporter: `cstrike/ls_crash.log` with the faulting address and the stack's
  return addresses as module+offset. Symbolize with
  `llvm-symbolizer --obj=mp.dll 0x1000_0000+offset` and the CI's `mp.pdb` next to it.
- **Boss-wave hang: not reproduced** (7 runs, 27+ boss-wave losses and new stands; QA's 3). Probably starvation on
  the overloaded Mac. The watchdog stays as the net.

## Crash files and the 0xfff4fadc crash (Y, 2026-11-13)
- **Where sim crashes land now (from Y, 2026-11-13):** every crash of any program in the Steam bottle appends the full
  winedbg report (registers, backtrace, modules: what the crash dialog shows) to
  `drive_c/Loner-Strike/Sim/crashes.log`, through the bottle's AeDebug hook (`C:\Loner-Strike\crashlog.bat`). The dialog
  may still pop; the report is in the file anyway. Check it after each run. `~/loner-strike-sim/creatures/crashcheck.sh mark`
  before a run and `crashcheck.sh since <mark>` after it print only the new reports. Builds of the fork from
  ls-creatures `f351765` on also write `cstrike/ls_crash.log` (the faulting address and the stack as module+offset;
  symbolize with `llvm-symbolizer --obj=mp.dll` and the CI's mp.pdb beside it) and `cstrike/ls_watchdog.log` (where the
  game DLL was when frames stopped for 10 s). A dedicated server on those builds ends quietly after a crash (no
  dialog holding its port; `ls_crash_quiet 0` for the old way).
- **The 0xfff4fadc crash** (new thread starting at 0xfff4fadc, only ntdll frames) is a **debugger attach**. Under
  CrossOver's wow64, the break-in thread that `winedbg <pid>` / `attach` (DebugActiveProcess) puts into a 32-bit hlds
  starts at a bad address and kills the server, 100% of the time (reproduced: attach, then detach, then the crash).
  Never attach winedbg to a running server: use the files above for stacks. It isn't in any game code (DR1's mp.dll
  has no watchdog; the stock LSPLAY build does the same).

## Rule from Y (2026-11-13): touch only your own server processes
- Never attach a debugger (`winedbg <pid>`, `attach`, DebugBreakProcess, anything that puts a thread into another
  process) to a running hlds under CrossOver: the injected thread starts at 0xfff4fadc and kills the server.
- Stop, signal or inspect only the processes your own script started: by the PID you got when you started it, or by a
  pattern that names your own sim folder (`Sim[\\/]<yours>[\\/]hlds.exe`). Never by name (`hlds.exe`, `winedbg`) and
  never by "the newest process". Before a start, make sure your port is free (`lsof -nP -iUDP:<port>`).
- Stacks and crashes without a debugger: `drive_c/Loner-Strike/Sim/crashes.log` (every crash in the bottle, full
  winedbg report), and with fork builds from ls-creatures `f351765` on, `cstrike/ls_crash*.log` and
  `cstrike/ls_watchdog.log` in the server folder.

## Requests from QA, round 3 (2026-11-13)
- Drop stress (your qa_drop: drop / drop_all / drop_dead): **r43392249830 crashed at round 24** (`exception 0xc0000005 …
  last text: drop`; Sim/QA5/qa_ls_crash_r43392249830.log), **r43395720981 ran 30/30 clean**. Worth a longer run on the
  newest build to be sure it's fixed and not just rare.
- On r43393053888, stopping a dedicated server (pkill) wrote ls_crash.log "exception 0xc0000005 at 0xfff4fadc" on both
  servers at the stop time: it reads like a real crash. Tag shutdown-time faults (or skip them).
- Creatures still kill LS defenders through intact gates (see W1's note #1): melee reach through a solid barricade.
- Boss-wave hang: still not reproduced (3 more losses on r43393053888/r43395720981).

## Base Builder player models as Last Stand creatures (Y, 2026-11-13; prototype only, roster unchanged)
- **Works without code changes.** A creatures.ini type can point at `models/player/bb_*/bb_*.mdl` and name the player
  sequences: `anim_idle = ref_aim_knife_idle1`, `anim_walk = ref_aim_knife_walk`, `anim_run = ref_aim_knife_run`,
  `anim_attack = ref_shoot_knife`, `anim_flinch = gut_flinch`. Missing names fall back to the activity (idle1, walk,
  run, death*). Sim copy run (de_dust2, 12 bb types + zombie): sequence 80 idle, 81 walk, 84 swing on the CSO-style
  models; 1/3/4 idle/walk/run plus 76 swing on the others. They chased and hit bots.
  - **Gap:** player sequences carry little or no linear movement, so walk/run can't be matched to the speed. A
    per-type `anim_speeds = <walk> <run>` key would fix the sliding; not built.
- **Looks** (renders: `~/loner-strike-sim/creatures/shot_*.png`): far more detailed and "modern zombie mod" than
  zombie.mdl. Fat cleaver tank, banshee in a dress, clawed green poisoner, grim reaper with a scythe, a T-800.
- **Licensing: don't ship them with Last Stand.** The textures and model names show they're ripped from commercial
  games: `NEXON_CSO_heavy_zombi_origin*.bmp`, ev_banshee/cosspeed/boomer/parcour "_origin" (Counter-Strike Online,
  Nexon), `kf_husk` (Killing Floor, Tripwire, the snowman), Deimos (CSO), a Terminator T-800. The GitHub repo has no
  licence. Base Builder setup already fetches them from that repo at install (we don't host them).
- **Cleanest way:** keep Last Stand's default roster on Half-Life's models. Offer an opt-in "Base Builder look" for
  Last Stand that appears only when mode-basebuilder is installed: a creatures.ini overlay of bb_ types, no new fetch
  and no Base Builder dependency. Never put the files on FastDL. A server that uses it makes clients without Base
  Builder download them (sv_allowdownload) or fail to join: that's the server owner's choice, stated on the toggle.
