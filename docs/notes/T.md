# Agent T notes: Community (app side)

Earlier notes: ../TOOLS-NOTES.md ("Agent T: Community creations", "Community round 2"). Design: ../COMMUNITY.md.

## 2026-11-12: entry points into Community
- Address `community:browse:<kind>` (map, mode, prefab, spray, texture) opens Tools > Community on Browse
  with that kind picked. `CommunityLink` in `views/tools/community/chip.tsx` (light, no Community chunk) goes there.
- Placed, each a small `(community)`-marked edit:
  - Mods page: "Modes from Community" always shows (own game, this computer), with "Browse community modes"
    and an empty state.
  - Play: "More modes from Community →" under the mode picker and the running game's mode list (own game only).
  - Maps: "More maps from Community" in the toolbar beside Show (own game only).
  - Spray maker: "Sprays from Community" always shows, with "Browse community sprays" and an empty state.
  - Editor prefab library: "Community prefabs…" beside Add from a file.
  - Get more: "Made with Loner-Strike: Community" in the heading (your own game; maps/sprays/textures
    sections pick that kind, others open All).
- Community opens on Browse (first open in a session ignores a remembered tab); Join the Loner-Strike Discord
  on Community and Find servers (`views/DiscordButton.tsx`, the permanent invite).

## Requests
- X (Studio): "Remix of … by …" for graphs in `<app dir>/community/remixes.json`; a Publish… action going to
  `community:publish:mode:<id>`.
- P8: Community moves to ActionBar / MapBrowser when their props are noted.

## Requests from X (2026-11-13)
The user wants Game Mode Studio library items shareable in the Community, data only, alongside
modes. Proposal: a package kind **"studio-library"** holding one file `<id>.lib.json` (schema
`loner-strike/graph-library`, version 1: name, description, category, kind function|macro|struct|enum,
root, itemVersion, then graph data: enums, structs, functions, macros, variables, nodes, edges,
comments). Checks for publish and install: `crate::tools::studio::check_library_item(&Value)` (same
allow-list and graph checks as modes; no events; never code). Installing = saving it into the
player's library: `crate::tools::studio::save_library_item(id, &item)` (picks the version; ids are
2 to 24 small letters, digits, `_`). A whole exported library is schema
`loner-strike/graph-library-set` `{items: [{id, item}]}` if you want to allow packs of several.
Tell me (here or in X.md) if you'd rather a different shape; I can add a route
`POST /api/tools/studio/library/import` taking `{item}` or `{items}` if that's easier for you.

## Requests from X (2026-11-13): mode map kits in community mode packages
Each mode gets a data-only kit (format in X.md, "Mode map kit format"): `<mode id>.kit.json`. For a
Studio mode the Studio makes it from the graph (`crate::tools::studio::kit_of(&Graph) -> Value`; checked
by `crate::tools::studio::check_kit(&Value)`). Please carry it in community mode packages beside the
graph, and when a community mode is installed, call `crate::tools::studio::install_kit(&kit)` (writes
`<app dir>/kits/<id>.kit.json` after checking it). A package without one is fine: install the graph and
the Studio makes the kit itself.

## Rule from Y (2026-11-13): touch only your own server processes
- Never attach a debugger (`winedbg <pid>`, `attach`, DebugBreakProcess, anything that puts a thread into another
  process) to a running hlds under CrossOver: the injected thread starts at 0xfff4fadc and kills the server.
- Stop, signal or inspect only the processes your own script started: by the PID you got when you started it, or by a
  pattern that names your own sim folder (`Sim[\\/]<yours>[\\/]hlds.exe`). Never by name (`hlds.exe`, `winedbg`) and
  never by "the newest process". Before a start, make sure your port is free (`lsof -nP -iUDP:<port>`).
- Stacks and crashes without a debugger: `drive_c/Loner-Strike/Sim/crashes.log` (every crash in the bottle, full
  winedbg report), and with fork builds from ls-creatures `f351765` on, `cstrike/ls_crash*.log` and
  `cstrike/ls_watchdog.log` in the server folder.

## 2026-11-13: ls_modes, mode kits, permanent ids (uid)
- **Publish reads the map's made-for modes**: worldspawn `ls_modes` (ids, or `uid:id` pairs; spaces, commas or
  semicolons) from the compiled .bsp (`community::made_for` / `made_for_refs`), first among the pre-ticked modes
  (still editable). The package's `map.modes` keeps the short ids (older apps) and `map.modeRefs` carries
  `{uid, id, name}` (names from the game's modes.ini). Browse cards show the names when an entry has them.
- **Kits**: a Studio mode's package carries `modes/<id>/<id>.kit.json` when `<graphs>/<id>.kit.json` exists and is a
  kit for that mode. Installing a community mode doesn't copy the downloaded kit: the Studio's compile
  (`register_mode`) writes the kit from the graph itself, so the kit always matches what was built. Removing a
  community mode removes `<app dir>/kits/<id>.kit.json` (kept by the safety net) unless you have a graph of that id.
- **uid**: `Item.uid`, `ModeMeta.uid`, `ModeRef.uid`, `Credit.uid` (= remixOf), `Installed.uid`; UUIDs, lower case,
  anything else read as "" (older entries keep working). Updates match by uid first (the same mode re-uploaded on
  another page still updates), else by page. A remix gets a fresh uid for itself; its credit holds the original's.
  The uid is read from the graph JSON's `mode.uid` (raw JSON, so it works before and after X's struct has it).
  Remix now edits the graph as JSON so fields this app doesn't model survive.
- **List service** (`services/serverlist/community.py`): `uid`, `mode.uid`, `map.modeRefs`, `remixOf` validated
  (UUIDs; modeRefs ≤ 16 with plain mode ids) and copied into the index; test `Validation.test_uids`.

## Requests (2026-11-13)
- X: please keep the graph's permanent id at `mode.uid` (that's what Community reads); if it's elsewhere, tell me.
  A community mode's kit is regenerated by your compile, not taken from the download.
- P5: `ls_modes` read as `uid:id` pairs or bare ids; tell me if the separator changes.
