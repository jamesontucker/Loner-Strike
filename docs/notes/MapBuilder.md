# MapBuilder notes (ls_outpost, the Last Stand map)
Older notes are in ../TOOLS-NOTES.md ("Map builder: ls_outpost ..."). Source: maps-src/ls_outpost (build_map.py →
ls_outpost.map), shipped files in plugins/game/.

## 2026-11-12 — v2 fixes after the user's playtest ("huge improvement")
- **Barricades z-fought** (frame and boards flickering against each other and the gate's jambs). In build_map.py
  nothing in a gate now shares a visible plane: the frame (two posts) sits 4 units inside the wall's outer face,
  12 deep; the boards are 6 thick, 3 units inside the frame's faces, their ends buried 4 units in the posts; the rail
  stops 1 unit under the lintel; every face that touches a jamb, the floor, the lintel or a post is NULL (box(...,
  hidden=)). Four boards 10 high with 16-unit gaps (was 5 with 6-8). The CLIP blocker is never drawn. Ladder art sits
  1 unit off its backing.
- **Creatures climbed the supply-yard ladder.** The map no longer relies on "creatures can't climb": the ladder volume
  starts 76 up (above the head of anything standing on the ground) over a 40-high crate step at its foot. Players and
  bots hop onto the step (40 < a jump) and grab the ladder; creatures step 18 and don't jump, so they can't reach it.
  Y is also making creatures ignore ladders.
- Compiled through Q's pipeline (Normal): 196 brushes, no leak, no warnings. Nav re-learned (the step is new walkable
  ground); reach check below.
- Nav: 320 areas, all reachable, 0 problems; the ladder joins the step (bottom 75) to the firing step (top 110). Overview and .res redone.

## Rule from Y (2026-11-13): touch only your own server processes
- Never attach a debugger (`winedbg <pid>`, `attach`, DebugBreakProcess, anything that puts a thread into another
  process) to a running hlds under CrossOver: the injected thread starts at 0xfff4fadc and kills the server.
- Stop, signal or inspect only the processes your own script started: by the PID you got when you started it, or by a
  pattern that names your own sim folder (`Sim[\\/]<yours>[\\/]hlds.exe`). Never by name (`hlds.exe`, `winedbg`) and
  never by "the newest process". Before a start, make sure your port is free (`lsof -nP -iUDP:<port>`).
- Stacks and crashes without a debugger: `drive_c/Loner-Strike/Sim/crashes.log` (every crash in the bottle, full
  winedbg report), and with fork builds from ls-creatures `f351765` on, `cstrike/ls_crash*.log` and
  `cstrike/ls_watchdog.log` in the server folder.

## 2026-11-13 — crenellated parapet (user request)
- The parapet along the firing step (west, south, east walls and over the three gatehouses) is now merlons and
  crenels: the wall runs to a sill 24 above the step (z 136, half the 48-unit parapet), merlons rise to 160, with a
  48-wide gap every ~112 units (one centred over each gate). Shoot down through a gap, step aside behind a merlon.
  The north wall (keep, armory) stays plain. Merlons are world brushes on the wall (the compiler merges the touching
  faces: nothing coplanar to flicker).
- Creature-proof: the sill is 136 above the ground outside (they step 18); CLIP from the sill up over every
  crenellated wall keeps players and bots inside (bullets and sight pass CLIP).
- Compiled through Q's pipeline (Normal): 229 brushes, no leak, no warnings. Nav re-learned: 366 areas, all
  reachable, 0 problems, the ladder still joined. Overview and .res redone; outputs in plugins/game.
- Bots (6, Normal, creatures, 7 min): waves 1-4 held; 62 of 1276 CT shots were fired from the firing step, by 5 of
  the 6 bots.
