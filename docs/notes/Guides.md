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

## Not in the guides because it isn't in the app yet
- The spawn nodes' "If the spot is blocked" setting (five options) and Spawned / Failed / Failure Reason on every
  spawn node: only Spawn Trigger Volume has the outcome pins so far (785fb399); QA4-T1 lists the rest as "waits for
  FX's shared helper". The Spawning guide's "When the spot is taken" section covers what exists (Find Free Spot
  Near, an empty output checked with Is Valid?, Spawn Trigger Volume's outcome). Add the options there when they land.
