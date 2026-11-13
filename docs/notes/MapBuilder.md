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
