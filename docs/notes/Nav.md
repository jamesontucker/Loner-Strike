# Nav (2026-11-13): map tools feedback pass

The user's notes from a real-app pass: tool names, how much the health check tells you, the nav
fixer's camera and editing, and whether custom callouts work end to end.

## 1. Names (commit 258cea9d)

- **Map doctor** stays: it reads a map's *files* without playing it (missing textures, models,
  sounds, sky, the .res download list, the nav and knowledge files, the overview) and fixes them.
  Its blurb now says "without playing it".
- **Map health check → Map simulator**: what it really does is run a bot match on a hidden copy
  of a server and report on it. The page says "Simulate a match", "Run the simulation", "Earlier
  simulations"; the doctor's link says "Simulate a match".
- Changed: registry, both pages, Replay's empty state, Community/Publish copy and the "Checked
  with bots" help, community.rs tips and warnings, health.rs messages, the MCP tool titles and
  descriptions (tool names stay `health_*` so agents keep working), docs/MCP.md, docs/COMMUNITY.md,
  site/docs.html (section `#map-simulator`, an empty `#health-check` anchor kept for old links),
  site/index.html, docs/notes/Site.md. The id `health`, routes and folders keep the old name.
- Not changed: TOOLS-PLAN.md / TOOLS-NOTES.md (history); mcp/routes.rs has one "health check" in
  a route list but had someone else's uncommitted work in it, so it's left (see Requests).

## 2. Map simulator vs the sim pipeline (commit 258cea9d)

What the report had: falls, deaths to the map, stuck spots (lists), % stuck / standing still,
kills a bot-minute, coverage % of nav areas, spawn sitters, map knowledge, console/plugin errors,
comparison with other maps. No picture of the map; nothing about rounds, sides or fights.

What the sim pipeline (analyze.py, sweepreport.py, the trace scripts) can produce on top:
heatmaps, choke points, unreachable areas, side balance, round times, plus behavior metrics
(reaction time, strafing, packs, view turns, shots per kill).

**Added** (all from the same recording, no extra run time; computed in `health.rs::insights`):
- **The map**: the radar maker's top-down picture of the map (clean style, bombsites, rescue
  zones, spawns) with one layer at a time: *Where bots went* (time per 128-unit cell),
  *Where they died* (enemy kills, victim spot), *Where kills came from* (killer spot),
  *Where nobody went* (nav areas no bot walked over). Pins for falls, stuck spots, deaths to
  the map and the busiest fights; a readout of the cell under the pointer; zoom and drag
  (the radar maker's PanZoom). Grids scale past 128 units so a side is at most 80 cells.
- **Rounds and sides**: rounds won by each side (gm_simrec now writes the winner on the round
  end row, `R,t,end,1|2|0`, from the T/CT win sounds; replay.rs reads it into `Round.winner`),
  kills by side, round length (middle, shortest, longest) and a strip of each round colored by
  winner. A finding when one side wins 75%+ of 8+ decided rounds.
- **Where fights happen** (choke points as bots play the map): deaths to enemies grouped by
  place, with the T/CT split; and the named places nobody went to (also a finding on runs of
  4+ minutes).
- Coverage now samples every 0.2 s instead of 0.5 s (fewer small areas missed at a run).
- Old reports still open: the new fields are optional and the page says what's missing.
- Tests: `rounds_sides_fights_and_places_nobody_went`, `one_sided_maps_and_places_nobody_went_are_said`;
  `report_from_recording` (ignored) turns any gm_simrec .csv into a report for the dev harness.
  UI: tests/sim-report.spec.ts (Chromium and WebKit via `LS_BROWSER=webkit`, both sizes).
- Tried for real: a 7-minute de_dust2 run, 12 bots, on a throwaway clone of Sim/B (port 27377,
  deleted after): 4 rounds recorded with winners (T 3, CT 1), 32 enemy kills, fight spots,
  408 nav areas never walked; the recorder compiles with the server's amxxpc.

**Left out, and why:**
- Behavior metrics (reaction time, strafing, packs, view turns, shots per kill): they judge the
  bot brain, not the map; a mapper can't act on them. They stay in analyze.py.
- True "unreachable" areas (no path from any spawn): that's a nav graph question, answered
  better and instantly by the Nav fixer's reach check than by a run. The simulator shows "never
  visited", which also catches areas that are reachable but pointless.
- Side balance over many runs / A-B comparisons: one run has 4-10 rounds, so the finding only
  fires on a big lean and says it's a hint. Comparing builds is the pipeline's job.
- Bomb plants/defuses by site: the recorder logs them (E rows), but bots' site choice is the
  brain's habit more than the map's; worth adding later as a layer if mappers ask.
- Fight spots on a nav without place names group by 256-unit squares, so they read as "the
  spot at x y z" and split up more; naming places in Callouts makes them read well.

## 3. Nav fixer UX (commits 86a9c702, f9260d74)

Friction found on de_dust2 and cs_rats2: scroll barely moved and zoomed to the map's middle;
top-down reset to the whole map (and back reset 3D); two-way links drew as spokes over every big
area; Draw/Link only showed as a pressed button; D or L then an immediate click acted in the old
mode (a real bug: the 3D view's settings arrived a frame late, now read at the moment of the
press); a top-down box started over a void silently did nothing.

- Camera (new navfix/FixCamera.tsx; the map viewer's camera and editor/NavCamera.tsx are
  untouched): scroll zooms to the point under the mouse and stops 12 units short; drag turns;
  right/middle/Shift-drag pans at the grabbed point's speed; double-click goes to a spot; F frames
  the picked areas (else what's under the mouse); glides instead of jumps; near plane scales.
- 3D and top-down share the point looked at and how much map is in view, both ways.
- Split view (3D / Top-down / Split, keys O / T / S, remembered; single 3D stays default): hover
  outlined in both views plus a marker in the other, shared picks, F moves both. Costs a second
  WebGL canvas (textures load twice on big maps).
- Draw and link: a mode chip ("Drawing areas" / "Linking", Both ways / One way, Done (Esc)) and
  a colored border; a cursor hint (box size and snap, "12 ↔ 34 · both ways", "already linked");
  8-unit grid plus snapping to nearby area edges, a snapped edge turns blue, Alt turns it off;
  links of hovered/picked areas drawn through walls: white both ways, orange arrow one way, pink
  drops in, with a Links: Picked / All switch and legend; picked areas outlined through roofs;
  add/delete area toasts with Undo (not on links: clicking along a beam stacked toasts). The `?`
  sheet lists the new keys.
- Left: top-down draws on the first surface seen from above, so under roofs/tunnels/cs_rats2's
  furniture use 3D or Split; a proper fix needs a height-slice control.
- Tests: tests/navfix-ux.spec.ts (mode chip and Esc, F, view switching keeps the spot, split
  hover and picks, undo after drawing): Chromium large+small 10/10 on each map, WebKit 5/5 on
  each (tests/webkit.config.ts); editor-nav.spec.ts and tools.spec.ts navfix checks pass. Nothing
  was saved to real navs.

## 4. Custom callouts (commits 182d0052, f68c8c77)

**They didn't work in game before this pass.** The Callouts tool saved names into the .nav
correctly, but ReGameDLL keeps a nav's place name only if BotChatter.db has a `Place <Name>`
entry (nav_file.cpp `PlaceDirectory::Load` -> `NameToID`); anything else becomes "no place". So
no Location message, no radio and no gm_botchat use, even for the editor's own list (Catwalk,
Long, Pit...): every BotChatter.db had only the 104 stock places.

Path: Callouts UI -> `POST /api/tools/callouts/:map` writes `maps/<map>.nav` (.nav.bak once)
-> the game loads the place table and checks each name against BotChatter.db -> Location
message `#Name` -> radio, scoreboard, and gm_botchat ("catwalk" in chat and "go catwalk").
Servers get the nav via `navs::copy_with`.

Tested on cs_office (scratch game copy + clone Sim/CO7 on 27373, deleted): UI flow in Chromium
and WebKit (box -> Catwalk, bomb sites and spawns suggestion, rename CTSpawn -> ZorkBase, save,
reload); the saved nav reads back with navpack/navarea.py (912 areas, 5 places, corners and
knowledge fit). In game before the fix: 377 samples standing in Catwalk, 0 `#Catwalk`. After:
`#Catwalk` 65 (and 160 on a second run), `#ZorkBase` 20; bots answer "was catwalk" to "where u
at" (`C,794.56,10,1,was catwalk`) and take "go catwalk" as an order.

Fixed: saving adds new names to BotChatter.db (cstrike and czero; the old file goes to History
first, and the save message lists the names it added); a server that gets a nav gets its names in its
BotChatter.db; the radio mark only counts names with a voice line; suggestions no longer
overwrite unsaved names. Tests: 2 Rust, tests/callouts.spec.ts (a `webkit` project in
playwright.config.ts for the specs it lists).

Left: navs that arrive other ways (downloaded packs) don't add names to the local game; a
server sent a custom-named nav before this fix needs the nav sent again. Real fix for the fork
owner: on an unknown place name, add it as a place with no voice line instead of dropping it
(repro: nav with place `ZorkBase` on CS 1.6 with the stock BotChatter.db -> no Location
message; add `Place ZorkBase`/`End` and restart -> it appears). Not checked: what the client
shows for an unknown `#Name` on the scoreboard.

## 5. After the user's test of the map simulator

- **No server on this computer**: the simulator makes a hidden temporary server from the game's
  own files, built the way a new server is with the "Your game" template:
  `servers::start_hidden_server` (Valve's download in `.base`, shared and kept; a copy of it;
  Loner-Strike plus the mods the game has; the engine; the game's mode settings). It runs as the
  setup job, with its steps shown as the run's progress, and it's deleted after the run (the
  `.health-check` folder). The setup page says so in one line. It takes about 4 minutes before
  bots start, because it installs every mod the game has. A faster version could install only
  the chosen mode's component. `LS_SERVERS_DIR` points servers and hidden servers at a test
  folder; `LS_KEEP_SIM_SERVER` keeps the temporary server to look into a failed run.
  `pkill_hidden` now matches only this servers folder's `.health-check`, not every one.
- **Map picture gone after coming back**: the picture's object URL was cached in TanStack
  Query and revoked when the page closed, so the cached URL was dead on return. Now the bytes
  are cached and each visit makes its own URL. Tested by leaving and reopening the report twice
  and via another tool (the blob URL must fetch).
- **Stuck false positives**: stuck was "slower than 20 u/s for 8 s with no enemy in view", which
  matched holding, camping, buying and defusing. Now it means trying to move: movement keys
  held (forward, back, strafe), no enemy in view, alive, in play, and never more than 160 units
  from where it began (pushing into a wall, or pacing between a few spots) for 8 s or more.
  Pauses in pressing of up to 1 s don't break a stretch (replay.rs, so Replay's stuck heat
  follows it too). Checked against the brain trace on the user's 5-minute de_dust2 run:
  all 21 old stuck spans had no movement keys pressed, and the brain was in Hide, Buy or
  DefuseBomb during them. The new rule finds none ("No problems found"). On cs_rats2 v5 it still
  finds real ones (bot 7 in Hunt with a goal, holding strafe at about 20 u/s for 88 s at
  -638 632).
- Findings and spot lists with a place have **Watch in Replay**: it opens the kept recording 3 s
  before the moment, following that bot (`replay:<id>@<time>,<player>`). The spot keeps when and
  who (`Spot.t`, `Spot.bot`) from the recording that's kept, which is now chosen by the number
  of samples in both places.

## Requests

- Owner of app/src-tauri/src/mcp/routes.rs: line ~725's route list says "health check"; it can
  say "map simulator" when you next commit that file.
