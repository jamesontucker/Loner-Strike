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

// ---- data tables (tables.ts)
import { addColumn, addTable, importCsv, parseCsv, readCell, removeColumn, retypeColumn, tablePins, tableProblems, toCsv, updateTable } from "../src/views/tools/studio/tables";
import { pinsOf } from "../src/views/tools/studio/pins";

test("data tables: CSV in (commas, tabs, quotes, a header row) and out", () => {
  assert.deepEqual(parseCsv('a,b\n"x, y",2\r\n\n3,"say ""hi"""'), [["a", "b"], ["x, y", "2"], ["3", 'say "hi"']]);
  assert.deepEqual(parseCsv("lvl\tgun\n0\tak47"), [["lvl", "gun"], ["0", "ak47"]]);
  let g = emptyGraph("tb", "Tb");
  const r = addTable(g, "Guns");
  g = updateTable(r.graph, r.id, (t) => retypeColumn({ ...t, columns: [{ id: "c1", name: "Level", type: "number" }, { id: "c2", name: "Gun", type: "text" }], rows: [] }, "c2", "weapon"));
  const t = g.dataTables![0];
  const imp = importCsv(t, "Level,Gun\n0,glock18\n1,weapon_AK47\n2,raygun", false);
  assert.match(imp.message, /Added 3 rows \(the first row was the column names\)/);
  assert.deepEqual(imp.table.rows, [[0, "glock18"], [1, "ak47"], [2, "knife"]]);
  assert.equal(toCsv(imp.table).split("\n")[0], "Level,Gun");
  // a text cell keeps the label characters only
  assert.equal(readCell({ id: "x", name: "X", type: "text" }, 'Dust "II" /x'), "Dust II x");
  // the same key twice is a problem the editor shows
  const dup = { ...imp.table, rows: [...imp.table.rows, [1, "awp"]] };
  assert.ok(tableProblems({ ...g, dataTables: [dup] }, dup).some((m) => m.includes("same key")));
  // columns come and go with their cells; the key moves off a removed column
  const more = addColumn(imp.table, "bool");
  assert.equal(more.rows[0].length, 3);
  const less = removeColumn(more, "c1");
  assert.equal(less.key, "c2");
});

test("data tables: nodes take their pins from the table", () => {
  let g = emptyGraph("tp", "Tp");
  const r = addTable(g, "Guns");
  g = updateTable(r.graph, r.id, (t) => ({ ...t, key: "c1", columns: [{ id: "c1", name: "Level", type: "number" }, { id: "c2", name: "Gun", type: "weapon" }], rows: [[0, "ak47"]] }));
  const add = (type: string, params: Record<string, string | number>) => {
    const x = addNode(g, type, { x: 0, y: 0 }, undefined, { table: r.id, ...params });
    g = x.graph;
    return g.nodes.find((n) => n.id === x.id)!;
  };
  const row = pinsOf(g, add("table.get_row", {}));
  assert.equal(row.inputs.find((p) => p.id === "key")?.type, "number");
  assert.deepEqual(row.outputs.map((p) => p.id), ["found", "row", "c1", "c2"]);
  assert.equal(row.outputs.find((p) => p.id === "c2")?.type, "text");
  assert.equal(pinsOf(g, add("table.get_cell", { column: "c2" })).outputs.find((p) => p.id === "out")?.type, "text");
  assert.equal(pinsOf(g, add("table.find", { column: "c1" })).outputs.find((p) => p.id === "out")?.type, "array<number>");
  assert.equal(pinsOf(g, add("table.keys", {})).outputs.find((p) => p.id === "out")?.type, "array<number>");
  assert.ok(tablePins(g, g.nodes[0]));
});

// ---- polish: promote, align, distribute, linked, drops, quick add, versions
import { align, distribute, linked, promoteToVariable, promotable, QUICK_ADD } from "../src/views/tools/studio/tidyOps";
import { dropPlan, sameEventHere } from "../src/views/tools/studio/dropOps";
import { compareGraphs } from "../src/views/tools/studio/versionOps";
import { addFunction as addFn2, addCustomEvent as addCe2 } from "../src/views/tools/studio/graphOps";

const kills = () => {
  const p = parseGraph(JSON.parse(templateRawForPolish));
  if ("error" in p) throw new Error(p.error);
  return p.graph;
};
import templateRawForPolish from "../src/views/tools/studio/templates/team-dm.graph.json?raw";

test("Promote to Variable: an input gets a Get from its setting, an output feeds a Set run next", () => {
  let g = kills();
  // Give Weapon's Spare bullets (a number with a setting of 90)
  assert.ok(promotable(g, { node: "give_m4", pin: "ammo" }, "in"));
  assert.equal(promotable(g, { node: "give_m4", pin: "exec" }, "in"), null, "flow pins can't");
  let r = promoteToVariable(g, { node: "give_m4", pin: "ammo" }, "in");
  assert.ok(!("error" in r));
  g = r.graph;
  const v = g.variables.find((x) => x.id === r.id)!;
  assert.equal(v.type, "number");
  assert.equal(v.default, 90);
  assert.ok(g.edges.some((e) => e.to.node === "give_m4" && e.to.pin === "ammo" && g.nodes.find((n) => n.id === e.from.node)?.type === "var.get"));
  // an output: Player Killed's Headshot -> a yes/no variable, set right after (Then was free? no: Is CT is there)
  r = promoteToVariable(g, { node: "killed", pin: "headshot" }, "out");
  assert.ok(!("error" in r));
  assert.equal(r.graph.variables.find((x) => x.id === r.id)!.type, "bool");
  assert.ok(r.graph.edges.some((e) => e.from.node === "killed" && e.from.pin === "headshot"));
  // inside a Function: a local of it
  let f = addFn2(emptyGraph("pf", "Pf"));
  const add = addNode(f.graph, "player.set_health", { x: 300, y: 0 }, f.id);
  const loc = promoteToVariable(add.graph, { node: add.id, pin: "amount" }, "in");
  assert.ok(!("error" in loc));
  assert.ok(loc.graph.functions![0].locals?.some((l) => l.id === loc.id));
  void f;
});

test("align, space evenly, select linked", () => {
  const boxes = new Map([
    ["a", { x: 0, y: 0, w: 100, h: 50 }],
    ["b", { x: 50, y: 200, w: 200, h: 50 }],
    ["c", { x: 400, y: 90, w: 100, h: 50 }],
  ]);
  assert.deepEqual([...align(boxes, "left").values()].map((p) => p.x), [0, 0, 0]);
  assert.deepEqual([...align(boxes, "right").values()].map((p) => p.x), [400, 300, 400]);
  assert.deepEqual([...align(boxes, "top").values()].map((p) => p.y), [0, 0, 0]);
  const across = distribute(boxes, true);
  // first and last stay; the middle one sits with equal gaps: (500 - 400) / 2 = 50
  assert.deepEqual([across.get("a")!.x, across.get("b")!.x, across.get("c")!.x], [0, 150, 400]);
  assert.equal(distribute(new Map([...boxes].slice(0, 2)), true).size, 0, "needs three");
  const g = kills();
  assert.deepEqual(new Set(linked(g, ["give_ak"], "up")), new Set(["give_ak", "is_t", "is_ct", "killed"]));
  assert.deepEqual(new Set(linked(g, ["is_t"], "down")), new Set(["is_t", "give_ak"]));
  assert.ok(linked(g, ["give_m4"], "all").includes("give_ak"));
});

test("drops from My mode: Ctrl Get, Alt Set, a choice, events found, tables and enums", () => {
  let g = kills();
  const v = addVariable(g, { name: "Kills", type: "number", scope: "player" });
  g = v.graph;
  const none = { ctrl: false, alt: false };
  assert.deepEqual(dropPlan(g, `var:${v.id}`, { ctrl: true, alt: false }, ""), { kind: "add", make: { type: "var.get", params: { var: v.id } } });
  assert.equal((dropPlan(g, `var:${v.id}`, { ctrl: false, alt: true }, "") as { make: { type: string } }).make.type, "var.set");
  assert.equal(dropPlan(g, `var:${v.id}`, none, "").kind, "choose");
  // an event already on this graph: jump to it
  const ev = dropPlan(g, "event:killed", none, "");
  assert.deepEqual(ev, { kind: "jump", node: "killed", message: "Already in this graph" });
  assert.equal(sameEventHere(g, "other", "event.player_killed", {}), null);
  // a custom event: Ctrl calls, no key offers Call, Set Timer and its event node
  const c = addCe2(g, "Open Gate");
  g = c.graph;
  assert.equal((dropPlan(g, `cevent:${c.id}`, { ctrl: true, alt: false }, "") as { make: { type: string } }).make.type, "event.call");
  const ch = dropPlan(g, `cevent:${c.id}`, none, "");
  assert.ok(ch.kind === "choose" && ch.options.map((o) => o.make.type).join() === "event.call,timer.by_event,event.custom");
  assert.equal((dropPlan(g, "table:dt1", { ctrl: false, alt: true }, "") as { make: { type: string } }).make.type, "table.for_each");
  assert.equal((dropPlan(g, "enum:e1", { ctrl: true, alt: false }, "") as { make: { type: string } }).make.type, "enum.value");
  assert.equal((dropPlan(g, "make:s1", { ctrl: false, alt: true }, "") as { make: { type: string } }).make.type, "struct.break");
  // a Function into itself: refused
  const f = addFn2(g);
  assert.equal(dropPlan(f.graph, `fn:${f.id}`, none, f.id).kind, "error");
  // quick-add keys name real nodes
  for (const q of Object.values(QUICK_ADD)) assert.ok(nodeDef(q.type), q.type);
});

test("version compare: added and changed nodes are marked, removed ones listed", () => {
  const old = kills();
  let now = kills();
  now = { ...now, nodes: [...now.nodes.filter((n) => n.id !== "hello").map((n) => (n.id === "give_m4" ? { ...n, params: { ...n.params, ammo: 10 } } : n.id === "start" ? { ...n, pos: { x: 999, y: 999 } } : n)), { id: "new1", type: "debug.print", pos: { x: 0, y: 0 }, params: {} }] };
  const c = compareGraphs(now, old);
  assert.equal(c.marks.new1, "added");
  assert.equal(c.marks.give_m4, "changed");
  assert.equal(c.marks.start, undefined, "a move isn't a change");
  assert.deepEqual(c.gone.map((x) => x.id), ["hello"]);
});

// ---- Timeline curves (curves.ts): the same as the plugin's
import { valueAt as curveAt, tangent as curveTangent } from "../src/views/tools/studio/curves";

test("Timeline curves: step, eases and custom tangents", () => {
  const k = (t: number, v: number, ease?: string, tin?: number, tout?: number) => ({ t, v, ...(ease ? { ease } : {}), ...(tin !== undefined ? { tin } : {}), ...(tout !== undefined ? { tout } : {}) }) as never;
  const keys = [k(0, 0, "constant"), k(1, 10, "in"), k(2, 20, "out"), k(3, 30, "ease"), k(4, 40, "custom", undefined, 50), k(5, 0, "custom", -10), k(6, 5)];
  assert.equal(curveAt(keys, 0.9), 0, "a step holds");
  assert.equal(curveAt(keys, 1.5), 12.5, "ease in: 10 + 10 * 0.25");
  assert.equal(curveAt(keys, 2.5), 27.5, "ease out: 20 + 10 * 0.75");
  assert.equal(curveAt(keys, 3.5), 35, "in-out: halfway at the middle");
  // custom: it leaves 40 going up at 50 a second, then turns down to 0
  assert.ok(curveAt(keys, 4.1) > 40);
  assert.ok(Math.abs(curveAt(keys, 5) - 0) < 1e-9 && Math.abs(curveAt(keys, 4) - 40) < 1e-9, "it passes through its keys");
  // Auto: through the neighbors
  assert.equal(curveTangent(keys, 6, "in"), (5 - 0) / 1);
  assert.equal(curveTangent(keys, 4, "out"), 50, "its own when set");
  // smooth: no jumps between ticks over the custom part
  let last = curveAt(keys, 4);
  for (let t = 4.05; t <= 5; t += 0.05) {
    const v = curveAt(keys, t);
    assert.ok(Math.abs(v - last) < 6, `a jump at ${t}`);
    last = v;
  }
});

// ---- UX review: a readable first view (H1), new nodes in a free spot (M3)
import { freeSpot, READABLE, startView } from "../src/views/tools/studio/tidyOps";

test("startView: a small graph fits; a big one opens on its first part at a readable zoom", () => {
  const box = (x: number, y: number, w = 230, h = 110) => ({ x, y, w, h });
  assert.deepEqual(startView([], [], { w: 800, h: 600 }), { fit: true });
  assert.deepEqual(startView([box(0, 0), box(300, 0)], [], { w: 800, h: 600 }), { fit: true });
  // 30 nodes in a row, 9000 px wide: fitting it all would be far under READABLE
  const row = Array.from({ length: 30 }, (_, k) => box(k * 300, 50));
  const v = startView(row, [box(-20, 0, 700, 300), box(4000, 0, 700, 300)], { w: 800, h: 600 });
  assert.equal(v.fit, false);
  assert.deepEqual(!v.fit && v.rect, box(-20, 0, 700, 300), "the top-left comment box");
  const n = startView(row, [], { w: 800, h: 600 });
  assert.deepEqual(!n.fit && n.rect, box(0, 50), "no comments: the first node");
  assert.ok(READABLE > 0.3 && READABLE < 1);
});

test("freeSpot: a new node doesn't land on another one of the same graph", () => {
  let g = emptyGraph("fs", "Fs");
  g = addNode(g, "debug.print", { x: 0, y: 0 }).graph;
  const p = freeSpot(g, { x: 20, y: 10 });
  assert.equal(p.x, 20);
  assert.ok(p.y !== 10, "moved off the node");
  const node = g.nodes[0];
  const far = Math.abs(p.y - node.pos.y);
  assert.ok(far >= 110, `clear of it (${far})`);
  // a node of another graph doesn't count
  assert.deepEqual(freeSpot(g, { x: 20, y: 10 }, "fn_other"), { x: 20, y: 10 });
  // promote puts its Get in a free spot too
  const k = kills();
  const r = promoteToVariable(k, { node: "give_m4", pin: "ammo" }, "in");
  assert.ok(!("error" in r));
  const get = r.graph.nodes.find((n) => n.type === "var.get" && n.params.var === r.id)!;
  assert.deepEqual(freeSpot(k, get.pos, get.graph), get.pos, "its spot was free");
});
