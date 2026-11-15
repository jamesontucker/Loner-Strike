# EC: engine coverage nodes (2026-11-14)

Built from docs/notes/EngineCoverage.md (items 1-8, 10-16, 19, 22, 24) plus NPC moves. Codegen in
`app/src-tauri/src/tools/studio/gen/coverage.rs` (+ `coverage.pawn`, `coverage_npc.pawn`, `coverage_vote.pawn`),
hooked into gen.rs with one-line calls (`coverage::...`, marked `// EC`), graph.rs (`problems`), mod.rs (gm_botnav in
plugins.ini for NPC moves). Registry block `# ==== EC:` in make_nodes.py. Tests: `gen/coverage/tests.rs`
(codegen, hooks only when used, hostile text, Problems, amxxpc both builds), graph `testdata/every_node_ec.graph.json`
(made by `make_every_node_ec.py`).

## Nodes
Deal Damage, Get/Set Entity Property, Big HUD Text, Colored Chat, Player Movement State, Push, Jump Rules, Grenade
Thrown, Grenade Exploded, Set Grenade Fuse, Play Animation, Attach To, Detach, On Map Entity Fired, Remove Map
Objectives, Map Type, Weapon Switched, Voice Rules, Voice Override, Play Radio, In Map Zone, Entered/Left Map Zone, Show
Info Window, NPC Move To, NPC Follow Path, NPC Stop, Round End Rules, Has Admin Flag, Start Vote, Vote Count, Hostage
Follow, Hostage Stop, Is Following, Add Bots, Remove Bots.

## Plugins
- gm_components: `ls_npc_move`, `ls_npc_move_path` (path_track chain), `ls_npc_move_stop`, `ls_npc_moving`, forwards
  `ls_on_npc_arrived(npc, point, last)`, `ls_on_npc_move_failed(npc, point)`; behavior 4 while moving.
- gm_botnav: `botnav_route` native and `botnav_route_cf` public (callfunc): A* crossings pulled straight for a
  person-sized box. No brain change was needed (creatures' nav natives only move ls_creature entities).
- gm_pubserver: `public gm_pubserver_voting()` so Start Vote never opens over the map or mode vote.

## Limits
Comfort limits are mode settings in limits.rs / limits.ts (X's registry): Attached things at once (64), Looping
animations at once (64), NPC moves at once (32), Map entity kinds heard (96); arrays hold the max, the value is read
with limit_expr; past one the call is skipped, logged, and the debugger says why. Big HUD Text, Colored Chat and Play
Radio go through the shared per-frame message budget (lsg_msg_room, the messages limit).

## Tutor box on CS 1.6 (feasibility only)
The CS 1.6 client.dll has the tutor window classes and handles TutorText / TutorLine / TutorState / TutorClose, and
ReGameDLL registers those messages in both games. But CS 1.6 ships none of the files the box needs
(resource/TutorScheme.res, resource/UI/TutorTextWindow.res, gfx/career/*.tga: CZ has them), so on 1.6 it would need those
sent as downloads. The visual can't be verified without a real client. Not built.

## Sim (2026-11-14, one sim EC1, -insecure, de_dust2, 10 bots, 5 min)
Brain: ls-next3 892b97a (run 43669651332, the branch head). Plugins: gm_components, gm_botnav, gm_pubserver, gamemodes
compiled from tools HEAD; the test mode compiled from a graph (scratchpad ec_sim.graph.json) with the current generator.
All seen in the console, no run time errors: the NPC was made, glowed (Get Entity Property read renderfx 19), walked to
its point (Arrived), then ran a 2-point path (At point twice, Finished); Map Type "bomb"; Deal Damage (health 97 after
3); Push (vertical speed 250); Weapon Switched (177); Entered/Left Map Zone (45/42); Grenade Thrown and Exploded (4/4,
flashbangs); Start Vote with bots voting (yes 10, no, tie across rounds); Add Bots and Remove Bots. Not seen in the
sim (no event on de_dust2 or no person to see it): On Map Entity Fired, Big HUD Text and Colored Chat on screen, Play
Radio's sound, the info window, hostages, voice. Sim copy deleted.

## Requests
None blocking.
