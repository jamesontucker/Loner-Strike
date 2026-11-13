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
