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
Comfort limits are `#define`s in coverage.pawn / coverage_npc.pawn (attachments 64, looping animations 64, map entity
kinds hooked 96, NPC moves waiting 32): past one the call is skipped, logged, and the debugger says why. When X's
Limits registry lands they can become mode settings. Engine limit enforced: at most 24 Big HUD Texts / Colored Chat
lines per server frame (the rest skipped, logged once).

## Tutor box on CS 1.6 (feasibility only)
The CS 1.6 client.dll has the tutor window classes and handles TutorText / TutorLine / TutorState / TutorClose, and
ReGameDLL registers those messages in both games. But CS 1.6 ships none of the files the box needs
(resource/TutorScheme.res, resource/UI/TutorTextWindow.res, gfx/career/*.tga: CZ has them), so on 1.6 it would need those
sent as downloads. The visual can't be verified without a real client. Not built.

## Requests
None blocking.
