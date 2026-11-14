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

// ---- the delete question (deleteOps.ts)
import { addCustomEvent, addFunction, addNode, addVariable, updateCustomEvent, updateFunction } from "../src/views/tools/studio/graphOps";
import { removeBridged, removeKeepNodes, removeWithNodes, replaceRefs, replacements, usesOfRef } from "../src/views/tools/studio/deleteOps";
import type { Graph as G } from "../src/views/tools/studio/schema";

/** event -> Set var -> Print (fed by a Get of the var), and a second number variable and a text one. */
function varGraph() {
  let g: G = emptyGraph("del_test", "Delete test");
  const a = addVariable(g, { name: "Score", type: "number", scope: "player" });
  const b = addVariable(a.graph, { name: "Points", type: "number", scope: "player" });
  const c = addVariable(b.graph, { name: "Label", type: "text", scope: "player" });
  const d = addVariable(c.graph, { name: "Total", type: "number", scope: "mode" });
  g = d.graph;
  const ev = addNode(g, "event.player_spawned", { x: 0, y: 0 });
  const set = addNode(ev.graph, "var.set", { x: 300, y: 0 }, undefined, { var: a.id, value: 1 });
  const get = addNode(set.graph, "var.get", { x: 300, y: 200 }, undefined, { var: a.id });
  const pr = addNode(get.graph, "debug.print", { x: 600, y: 0 });
  g = pr.graph;
  const E = (id: string, f: [string, string], t: [string, string]) => ({ id, from: { node: f[0], pin: f[1] }, to: { node: t[0], pin: t[1] } });
  g = { ...g, edges: [E("w1", [ev.id, "then"], [set.id, "exec"]), E("w2", [ev.id, "player"], [set.id, "player"]), E("w3", [set.id, "then"], [pr.id, "exec"]), E("w4", [get.id, "out"], [pr.id, "value"]), E("w5", [ev.id, "player"], [get.id, "player"])] };
  return { g, v: a.id, other: b.id, text: c.id, mode: d.id, ev: ev.id, set: set.id, get: get.id, pr: pr.id };
}

test("delete a variable: its uses, what fits in its place", () => {
  const t = varGraph();
  assert.equal(usesOfRef(t.g, { kind: "var", id: t.v }).length, 2);
  // same type and scope only (not the text one, not the mode one)
  assert.deepEqual(replacements(t.g, { kind: "var", id: t.v }).map((x) => x.id), [t.other]);
});

test("delete a variable: Replace with switches every use, wires stay", () => {
  const t = varGraph();
  const g = replaceRefs(t.g, { kind: "var", id: t.v }, t.other);
  assert.ok(!g.variables.some((x) => x.id === t.v));
  assert.equal(g.nodes.filter((n) => n.params.var === t.other).length, 2);
  assert.equal(g.edges.length, t.g.edges.length);
});

test("delete a variable with its nodes: the flow is joined round the Set, the Get's wire goes", () => {
  const t = varGraph();
  const g = removeWithNodes(t.g, { kind: "var", id: t.v });
  assert.ok(!g.nodes.some((n) => n.id === t.set || n.id === t.get));
  assert.ok(g.edges.some((e) => e.from.node === t.ev && e.from.pin === "then" && e.to.node === t.pr && e.to.pin === "exec"), "event -> print");
  assert.ok(!g.edges.some((e) => e.to.node === t.pr && e.to.pin === "value"));
  assert.equal(new Set(g.edges.map((e) => e.id)).size, g.edges.length, "wire ids stay unique");
  // keep the nodes: as before
  const k = removeKeepNodes(t.g, { kind: "var", id: t.v });
  assert.equal(k.nodes.length, t.g.nodes.length);
  assert.ok(!k.variables.some((x) => x.id === t.v));
});

test("removeBridged joins a chain of removed nodes", () => {
  let g: G = emptyGraph("chain", "Chain");
  const ids: string[] = [];
  for (const type of ["event.round_started", "debug.print", "debug.print", "debug.print"]) {
    const r = addNode(g, type, { x: ids.length * 300, y: 0 });
    g = r.graph;
    ids.push(r.id);
  }
  g = { ...g, edges: [0, 1, 2].map((i) => ({ id: `w${i + 1}`, from: { node: ids[i], pin: "then" }, to: { node: ids[i + 1], pin: "exec" } })) };
  const out = removeBridged(g, [ids[1], ids[2]]);
  assert.deepEqual(out.edges.map((e) => [e.from.node, e.to.node]), [[ids[0], ids[3]]]);
});

test("delete a Function: its calls go (flow joined) or follow a Function with the same pins", () => {
  let g: G = emptyGraph("fns", "Fns");
  const f1 = addFunction(g, false, "Reward");
  g = updateFunction(f1.graph, f1.id, { inputs: [{ id: "a1", name: "Who", type: "player" }] });
  const f2 = addFunction(g, false, "Bonus");
  g = updateFunction(f2.graph, f2.id, { inputs: [{ id: "b9", name: "Player", type: "player" }] });
  const ev = addNode(g, "event.player_spawned", { x: 0, y: 0 });
  const call = addNode(ev.graph, "fn.call", { x: 300, y: 0 }, undefined, { func: f1.id });
  const pr = addNode(call.graph, "debug.print", { x: 600, y: 0 });
  g = { ...pr.graph, edges: [...pr.graph.edges, { id: "x1", from: { node: ev.id, pin: "then" }, to: { node: call.id, pin: "exec" } }, { id: "x2", from: { node: ev.id, pin: "player" }, to: { node: call.id, pin: "a1" } }, { id: "x3", from: { node: call.id, pin: "then" }, to: { node: pr.id, pin: "exec" } }] };
  const ref = { kind: "fn" as const, id: f1.id };
  assert.deepEqual(replacements(g, ref).map((x) => x.id), [f2.id]);
  const rep = replaceRefs(g, ref, f2.id);
  assert.ok(rep.edges.some((e) => e.to.node === call.id && e.to.pin === "b9"), "the wire follows to the other Function's pin");
  assert.ok(!rep.functions?.some((f) => f.id === f1.id) && !rep.nodes.some((n) => n.graph === f1.id), "its graph went");
  const gone = removeWithNodes(g, ref);
  assert.ok(!gone.nodes.some((n) => n.id === call.id));
  assert.ok(gone.edges.some((e) => e.from.node === ev.id && e.to.node === pr.id));
});

test("delete a Custom Event: Call nodes and its event nodes go", () => {
  let g: G = emptyGraph("ces", "Ces");
  const c = addCustomEvent(g, "Open Gate");
  g = updateCustomEvent(c.graph, c.id, { inputs: [] });
  const on = addNode(g, "event.custom", { x: 0, y: 300 }, undefined, { event: c.id });
  const ev = addNode(on.graph, "event.round_started", { x: 0, y: 0 });
  const call = addNode(ev.graph, "event.call", { x: 300, y: 0 }, undefined, { event: c.id });
  g = { ...call.graph, edges: [{ id: "q1", from: { node: ev.id, pin: "then" }, to: { node: call.id, pin: "exec" } }] };
  const ref = { kind: "cevent" as const, id: c.id };
  assert.equal(usesOfRef(g, ref).length, 2);
  const out = removeWithNodes(g, ref);
  assert.ok(!out.nodes.some((n) => n.id === on.id || n.id === call.id));
  assert.ok(!(out.customEvents ?? []).length);
});

test("setting names may hold digits after the first letter (mp_give_player_c4)", () => {
  const base = emptyGraph("cvars", "Cvars");
  const ok = parseGraph({ ...base, mode: { ...base.mode, settings: [{ cvar: "mp_give_player_c4", value: 0 }] } });
  assert.ok("graph" in ok);
  for (const bad of ["4mp", "mp-x", "MP_X", "mp_x;exec"]) {
    const r = parseGraph({ ...base, mode: { ...base.mode, settings: [{ cvar: bad, value: 0 }] } });
    assert.ok("error" in r, bad);
  }
});
