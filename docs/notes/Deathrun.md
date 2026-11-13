# Deathrun agent notes

## Requests from Y
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

## Queued after QA round 4 (user, 2026-11-13): fun_allinone with smart bots
The user's favorite fun map, https://gamebanana.com/mods/93446 (fun_allinone by natko, CS 1.6). Get it through Get more /
GameBanana; never ship it. Test case for general fun-map skills, not map-specific code:
- Glass spawn room race; the first out picks the game in a room of 6 "SECURE ACCESS" doors (func_button -> multi_manager ->
  func_door + 56 trigger_changetarget rewiring 18 trigger_teleports).
- Six arenas: HE only, Para (24 trigger_push), Knives, AWPs, Shotguns, Pistols (walk into one of 14 trigger_multiple ->
  game_player_equip spots to pick a pistol; player_weaponstrip on entry).
Brain work, all general: read the map's I/O wiring (button -> manager -> door/teleport/equip) so bots know what a button
does; a chooser behavior for the race winner (random or by preference, maybe what humans played last); teleports whose
destination changes (learn every destination, follow the active one); arena awareness after strip/equip (HE-only fighting
with real grenade arcs, para floating/falling, walking to a chosen equip spot). Check against other fun/jail maps too.
Nav generation must work on it first (user, 2026-11-13): learn a clean nav for fun_allinone with the app's normal
nav learn (not hand-edited), covering the glass room, the chooser room and all six arenas, with every teleport destination
linked, ladders joined, no unreachable or trapped areas in playable space, and the 6,000-area map-knowledge limit
respected. Fix the learner generally (seeding every arena, teleport destinations, trigger_push areas, water) if it
misses parts, and run the nav fixer's reach check on the result.

## Queued after QA round 4 (user, 2026-11-13): vehicles and other interactive func_ entities
General fun-map skills, alongside the fun_allinone work. Bots should understand and use the interactive entities fun maps
are built from, learned from the map's own entities (no per-map code):
- Vehicles: func_vehicle (CS 1.6 drivable cars and boats: get in with USE, steer, exit; drive toward goals, run people
  over, avoid water/edges) and func_tracktrain (drivable trains and carts on path_track).
- Mounted guns: func_tank / func_tankmortar / func_tankrocket / func_tanklaser with func_tankcontrols (man a turret,
  aim it, leave it).
- Lifts and movers: func_plat, func_train lifts, func_door used as an elevator, func_rotating platforms, func_conveyor,
  func_pushable crates (push to climb or block).
- Buttons and valves: func_button, func_rot_button, momentary_rot_button (hold to turn), multisource-gated doors.
- Hazards and movement: trigger_push / trigger_gravity, func_water (swim, breathe), trigger_hurt, func_breakable to
  break through, ladders.
Nav learn has to cover areas reached only by riding or pushing something (link ride start -> ride end). Test on a few
real fun maps with vehicles and turrets from GameBanana (pick popular ones), with the same "general, not overfitted"
check as deathrun.
