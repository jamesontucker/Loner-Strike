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
