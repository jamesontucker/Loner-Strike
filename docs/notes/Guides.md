# Guides (technical writer): the reference's hand-written guides

## Done (2026-11-14)
Six of the seven guides in the reference's Guides section, as pages in `site/reference/guides/`:
events-flow, variables, spawning, limits, bots, sharing. Types and casting stays a stub until the Studio type
system lands (docs/notes/StudioTypes.md).

How it fits the generator (scripts/gen-reference.mjs):
- A guide is "written" when its page has text between `<!-- GUIDE:BEGIN … -->` and `<!-- GUIDE:END -->`.
  Re-running the script keeps that text and rebuilds the frame (top bar, search, sidebar with every guide,
  On this page from the `<h2 id>`s), the landing card (badge "guide" instead of "soon"), the docs.html entry
  (a short section with the guide's sections, replacing the stub) and search entries (one per guide and one
  per section, `c: "guide"`, category Guides).
- Node links are `<a class="node" data-node="<type>" href="…">`; the href is set from the catalog each run.
  `guideProblems()` lists links to nodes the catalog doesn't have.
- Links between guides (`href="types.html"`) point at the docs stub while that guide isn't written.
- guides.css (written by the script) styles node links and chains (`<p class="chain">` with `<span class="to">→</span>`).

Checked: Chromium and WebKit at 1440 and 390 (no sideways scroll, no console errors), site spec 34/34
(`npx playwright test -c tests/site.config.ts`, new site-guides.spec.ts included). Re-running the script twice
leaves the guide pages and docs.html byte-identical.

## Spawn outcome update (2026-11-14, after c0543a94)
The Spawning guide's "When the spot is taken" now documents If the spot is blocked (the five choices), Max distance,
Retry every / Retry tries, and Then / Spawned / Failed / Entity / Success / Moved / Failure reason, linked to the five
nodes that have them (Spawn Prop, Spawn Creature, Spawn Entity, Spawn Weapon On Ground, Create Component), with a
fallback chain and a Retry example; Limits' "What's skipped" points there.

## Spawn outcome, the last three (2026-11-15, after 4a11ca7b)
Spawn NPC, Spawn Interactable and Spawn Projectile added to "When the spot is taken", with their defaults (NPC: try a
nearby spot; Interactable: don't spawn if blocked; Projectile: always spawn, it isn't solid).

## Types and casting (2026-11-15, after e5c437a7)
site/reference/guides/types.html: the type menu's groups, one Number with # marks, Weapon / Hit Zone / Damage Kind into
Switch on Enum, Color and its "Color (wins over the setting)" pins, Transform, files (picked, never made from text),
handles and Sound Loop, upcasts, casts with Success / Failed, Kind Of into Switch, Insert As, orphaned pins and Refresh
nodes. Checked against the committed catalog, schema.ts, graph.rs / Canvas.tsx strings and StudioCompat.md. No color
table columns or color mode settings (not in the app). All seven guides are written; no stubs left.
The reference was regenerated from the committed catalog in the same commit (Colors and Files pages are new).

## Release text pass (2026-11-15)
- docs.html (Game Mode Studio): pins (square file pins, kind letters, #), Over 650 nodes, a Types row, files as values
  and picker previews, variable types, a new "Types and casting" h2 (#studio-types) linking the guide, table columns
  (models, sprites, sounds; 16 columns), files travel with a shared mode, Orphaned pins in "Why won't it compile?",
  2000 nodes and the Limits settings in "Good to know", Publish's Requirements covering file variables and table cells.
- Guides: Limits (the table from HEAD's limits: Effect messages a frame max 64, Loop steps a frame / in a Function,
  Visible things at once, Things in one trigger volume; the breaking points: 128-slot reserve, things in sight, the
  per-player byte budget and state messages that wait, loops spread over frames; the aggregated log lines),
  Events and flow (loops spread over frames), Variables (types, file columns, 16 columns), Sharing (files in
  variables and table cells travel).
- Studio tour, The canvas step: square file pins, kind letters, the cast offered on a dropped Entity wire.
- Example mode and starter premade descriptions: read, nothing outdated (no colors, paths or text weapons in them;
  Weapon Pickup's Weapon input is still text, and a Weapon value goes into it).
