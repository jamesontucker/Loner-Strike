// X2's Studio pieces: node help (NodeHelp.tsx + nodehelp.json), the example modes and the tour.
// Run with the other Studio tests: `npm run test:studio`.

import { test } from "node:test";
import assert from "node:assert/strict";
import { exampleSnippet, loadHelp, searchHelp } from "../src/views/tools/studio/NodeHelp";
import { EXAMPLES } from "../src/views/tools/studio/examples";
import { TOUR_STEPS } from "../src/views/tools/studio/Tour";
import { NODE_DEFS, nodeDef } from "../src/views/tools/studio/registry";
import { emptyGraph, parseGraph } from "../src/views/tools/studio/schema";
import { insertSnippet } from "../src/views/tools/studio/graphOps";

test("every node has help, with every pin and setting", async () => {
  const help = await loadHelp();
  for (const d of NODE_DEFS) {
    const h = help[d.type];
    assert.ok(h, `${d.type} has no help`);
    for (const p of [...d.inputs, ...d.outputs]) assert.ok(h.pins.some((x) => x.id === p.id), `${d.type}: pin ${p.id}`);
    for (const p of d.params) assert.ok(h.settings.some((x) => x.id === p.id), `${d.type}: setting ${p.id}`);
    for (const r of h.related) assert.ok(nodeDef(r), `${d.type}: related ${r}`);
  }
});

test("help search: titles first, then what nodes do", async () => {
  const help = await loadHelp();
  const g = searchHelp(help, "gravity");
  assert.ok(g.length >= 3);
  assert.equal(help[g[0]].title.toLowerCase().startsWith("gravity"), true);
  // words anywhere in the text, all of them
  const kill = searchHelp(help, "headshot weapon");
  assert.ok(kill.includes("event.player_killed") && kill.includes("event.player_hurt"));
  assert.deepEqual(searchHelp(help, "zzzz nothing like it"), []);
  assert.equal(searchHelp(help, "").length, NODE_DEFS.length);
});

test("help examples drop into a graph and stay valid", async () => {
  const help = await loadHelp();
  let n = 0;
  for (const [type, h] of Object.entries(help)) {
    const sn = exampleSnippet(type, h);
    if (!sn) continue;
    n++;
    const r = insertSnippet(emptyGraph("help_test", "Help test"), sn, { x: 100, y: 100 });
    assert.ok(r, `${type}: nothing inserted`);
    assert.ok(r.graph.nodes.some((x) => x.type === type), `${type}: its example lacks it`);
    const again = parseGraph(JSON.parse(JSON.stringify(r.graph)));
    assert.ok("graph" in again, `${type}: the graph with its example doesn't parse`);
  }
  assert.ok(n > NODE_DEFS.length * 0.8, `only ${n} examples`);
});

test("the five example modes parse, use known nodes and have notes", () => {
  assert.deepEqual(EXAMPLES.map((e) => e.label), ["Infection", "Gun Game Lite", "King of the Hill", "One in the Chamber", "Juggernaut"]);
  for (const e of EXAMPLES) {
    const r = parseGraph(JSON.parse(e.raw));
    assert.ok("graph" in r, `${e.label}: ${"error" in r ? r.error : ""}`);
    const g = r.graph;
    for (const n of g.nodes) assert.ok(nodeDef(n.type), `${e.label}: unknown node ${n.type}`);
    assert.ok(g.comments.length >= 3, `${e.label}: notes`);
    assert.ok(g.nodes.length >= 25 && g.nodes.length <= 90, `${e.label}: ${g.nodes.length} nodes`);
  }
});

test("the tour: short, every step says something, controls named by stable selectors", () => {
  assert.ok(TOUR_STEPS.length >= 6 && TOUR_STEPS.length <= 12);
  for (const s of TOUR_STEPS) {
    assert.ok(s.title && s.text.length > 40);
    assert.ok(!/colour|behaviour|favourite/i.test(s.text), "US English");
  }
  const at = TOUR_STEPS.map((s) => s.at).filter(Boolean).join(" ");
  for (const want of ["studio-compile", "studio-test", "studio-playtest", "tl-actionbar", "st-canvas", "studio-help-menu"]) assert.ok(at.includes(want), want);
});
