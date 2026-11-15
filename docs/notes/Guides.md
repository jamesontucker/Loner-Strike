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

## Waiting on
- Spawn Projectile, Spawn NPC and Spawn Interactable: the creature fixer is adding the same setting and outputs.
  The guide says they're coming and to check their output with Is Valid? until then. When they commit, add them to
  the node list at the top of "When the spot is taken" and drop that sentence.

## Types and casting (2026-11-15, after e5c437a7)
site/reference/guides/types.html: the type menu's groups, one Number with # marks, Weapon / Hit Zone / Damage Kind into
Switch on Enum, Color and its "Color (wins over the setting)" pins, Transform, files (picked, never made from text),
handles and Sound Loop, upcasts, casts with Success / Failed, Kind Of into Switch, Insert As, orphaned pins and Refresh
nodes. Checked against the committed catalog, schema.ts, graph.rs / Canvas.tsx strings and StudioCompat.md. No color
table columns or color mode settings (not in the app). All seven guides are written; no stubs left.
The reference was regenerated from the committed catalog in the same commit (Colors and Files pages are new).
