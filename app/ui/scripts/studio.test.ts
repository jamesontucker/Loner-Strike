// Game Mode Studio unit tests (kept out of src/ so the app build never sees node:test): the graph schema, wiring rules, copy/paste, undo/redo and what debug
// events mean. Run with `npm run test:studio` (esbuild + node:test, no extra packages).

import { test } from "node:test";
import { readFileSync, writeFileSync } from "node:fs";
import assert from "node:assert/strict";
import templateRaw from "../src/views/tools/studio/templates/team-dm.graph.json?raw";
import { parseGraph, emptyGraph, type Graph } from "../src/views/tools/studio/schema";
import { insertSnippet, makeSnippet, readSnippet } from "../src/views/tools/studio/graphOps";
import { addVariable, addNode, canConnect, connect, copy, emptyHistory, insertReroute, inType, outType, paste, record, redo, remove, undo } from "../src/views/tools/studio/graphOps";
import { NODE_DEFS, compatible, matchRank, nodeDef, searchNodes } from "../src/views/tools/studio/registry";
import { addEnum, addFunction, addMacro, addStruct, collapse, removeGraphPart, tabView, updateFunction, updateStruct } from "../src/views/tools/studio/graphOps";
import { pinsOf, typeLabel, typeOk } from "../src/views/tools/studio/pins";
import { extractItem, insertItem, itemFile, newerInLibrary, readItem } from "../src/views/tools/studio/library";
import { applyEvents, emptyDebug, pathPairs } from "../src/views/tools/studio/debugState";
import { tidyEstimated } from "../src/views/tools/studio/layout";
import lastStandRaw from "../src/views/tools/studio/templates/last-stand.graph.json?raw";
import royaleRaw from "../src/views/tools/studio/templates/battle-royale.graph.json?raw";
import runDialogRaw from "../src/views/tools/studio/library/run_dialog.lib.json?raw";

const template = (): Graph => {
  const r = parseGraph(JSON.parse(templateRaw));
  if ("error" in r) throw new Error(r.error);
  return r.graph;
};

test("the example graph parses and round-trips", () => {
  const g = template();
  assert.equal(g.mode.id, "rifle_reward");
  const again = parseGraph(JSON.parse(JSON.stringify(g)));
  assert.ok("graph" in again);
  assert.deepEqual("graph" in again && again.graph, g);
});

test("schema refuses other files, newer versions and unsafe text", () => {
  assert.ok("error" in parseGraph({ schema: "x" }));
  const newer = parseGraph({ schema: "loner-strike/graph", version: 7 });
  assert.ok("error" in newer && newer.error.includes("newer"));
  const g = template();
  for (const name of ["a */ b", 'q"uote', "line\nbreak", "caret ^", "back\\slash"]) {
    assert.ok("error" in parseGraph({ ...g, mode: { ...g.mode, name } }), name);
  }
  assert.ok("error" in parseGraph({ ...g, mode: { ...g.mode, id: "Bad Id" } }));
  const n = { ...g.nodes[1], params: { text: "x); server_cmd(\"quit" } };
  assert.ok("error" in parseGraph({ ...g, nodes: [n] }));
  assert.ok("error" in parseGraph({ ...g, comments: [{ ...g.comments[0], color: "red;background:url(x)" }] }));
  assert.ok("graph" in parseGraph({ ...g, mode: { ...g.mode, name: "Rifles & More (v2)!" } }));
});

test("every node in the registry is well formed", () => {
  for (const d of NODE_DEFS) {
    assert.ok(d.title && d.category && d.help, d.type);
    for (const p of [...d.inputs, ...d.outputs]) assert.ok(["exec", "player", "entity", "team", "number", "bool", "text", "any", "var", "vector", "timer", "$T", "$K", "$E", "$S", "$F", "array<$T>", "map<$K,$T>", "array<$K>", "array<player>", "array<entity>"].includes(p.type), `${d.type}.${p.id}`);
  }
  for (const t of ["event.round_started", "event.player_killed", "cond.is_team", "player.give_weapon", "score.end_round"]) assert.ok(nodeDef(t), t);
});

test("incompatible wires are refused, compatible ones replace the old wire", () => {
  let g = template();
  // a player into a flow input; a flow into a player pin; a player into a team pin
  assert.equal(canConnect(g, { node: "killed", pin: "victim" }, { node: "give_m4", pin: "exec" }).ok, false);
  assert.equal(canConnect(g, { node: "killed", pin: "then" }, { node: "give_m4", pin: "player" }).ok, false);
  assert.equal(canConnect(g, { node: "killed", pin: "victim" }, { node: "is_ct", pin: "team" }).ok, false);
  assert.equal(canConnect(g, { node: "give_m4", pin: "player" }, { node: "is_ct", pin: "player" }).ok, false, "inputs aren't outputs");
  // victim instead of attacker into give_m4: replaces the attacker wire
  const r = connect(g, { node: "killed", pin: "victim" }, { node: "give_m4", pin: "player" });
  assert.ok("graph" in r);
  g = (r as { graph: Graph }).graph;
  const into = g.edges.filter((e) => e.to.node === "give_m4" && e.to.pin === "player");
  assert.equal(into.length, 1);
  assert.equal(into[0].from.pin, "victim");
  // a flow back to the start would loop
  const loop = canConnect(g, { node: "give_ak", pin: "then" }, { node: "is_ct", pin: "exec" });
  assert.equal(loop.ok, false);
});

test("reroutes carry the type that comes in", () => {
  let g = template();
  const w = g.edges.find((e) => e.from.pin === "attacker" && e.to.node === "give_ak")!;
  g = insertReroute(g, w.id, { x: 800, y: 200 });
  const re = g.nodes.find((n) => n.type === "flow.reroute")!;
  assert.equal(outType(g, { node: re.id, pin: "out" }), "player");
  assert.equal(inType(g, { node: re.id, pin: "in" }), "player");
  assert.equal(canConnect(g, { node: re.id, pin: "out" }, { node: "is_t", pin: "exec" }).ok, false);
  assert.equal(g.edges.filter((e) => e.to.node === "give_ak" && e.to.pin === "player").length, 1);
});

test("a mode setting on a variable is kept with the graph (label, range, choices, group)", () => {
  const g = emptyGraph("koth", "King");
  const withSetting = { ...g, variables: [{ id: "hold", name: "Hold", type: "number", scope: "mode", default: 30, setting: { label: "Seconds to win", min: 5, max: 120, group: "Rules" } }] };
  const r = parseGraph(withSetting);
  if ("error" in r) assert.fail(r.error);
  assert.equal(r.graph.variables[0].setting?.label, "Seconds to win");
  assert.equal(r.graph.variables[0].setting?.max, 120);
  const bad = parseGraph({ ...withSetting, variables: [{ ...withSetting.variables[0], setting: { label: "x", choices: Array(13).fill("a") } }] });
  assert.ok("error" in bad, "13 choices");
});

test("mode settings may have digits after the first letter (mp_give_player_c4), not first", () => {
  const g = emptyGraph("koth", "King");
  const ok = parseGraph({ ...g, mode: { ...g.mode, settings: [{ cvar: "mp_give_player_c4", value: 0 }] } });
  assert.ok(!("error" in ok), "mp_give_player_c4");
  const bad = parseGraph({ ...g, mode: { ...g.mode, settings: [{ cvar: "4mp", value: 0 }] } });
  assert.ok("error" in bad, "a digit first");
});

test("version 1 and 2 files open as version 3; variables are checked", () => {
  const g = template();
  const r = parseGraph({ ...g, version: 1, variables: undefined });
  assert.ok("graph" in r && r.graph.version === 3 && r.graph.variables.length === 0 && !r.graph.functions);
  const r2 = parseGraph({ ...g, version: 2 });
  assert.ok("graph" in r2 && r2.graph.version === 3);
  assert.ok("error" in parseGraph({ ...g, version: 4 }));
  assert.ok("error" in parseGraph({ ...g, variables: [{ id: "v1", name: "Kills", type: "number", scope: "player", default: "lots" }] }));
  assert.ok("error" in parseGraph({ ...g, variables: [{ id: "v1", name: "a */ b", type: "number", scope: "mode", default: 0 }] }));
  assert.ok("graph" in parseGraph({ ...g, variables: [{ id: "v1", name: "Kills", type: "number", scope: "player", default: 0 }] }));
});

test("a variable's pins take its type", () => {
  let g = template();
  const v = addVariable(g, { name: "Kills", type: "number", scope: "player", default: 0 });
  g = v.graph;
  const s = addNode(g, "var.set", { x: 0, y: 500 });
  g = { ...s.graph, nodes: s.graph.nodes.map((n) => (n.id === s.id ? { ...n, params: { ...n.params, var: v.id } } : n)) };
  assert.equal(inType(g, { node: s.id, pin: "value" }), "number");
  // a player can't go into a number
  assert.equal(canConnect(g, { node: "killed", pin: "victim" }, { node: s.id, pin: "value" }).ok, false);
  const h = addNode(g, "player.get_health", { x: 0, y: 600 });
  g = h.graph;
  assert.equal(canConnect(g, { node: h.id, pin: "out" }, { node: s.id, pin: "value" }).ok, true);
  // a player is an entity
  const c = addNode(g, "comp.crate_open", { x: 300, y: 600 });
  assert.equal(canConnect(c.graph, { node: "killed", pin: "victim" }, { node: c.id, pin: "container" }).ok, true);
});

test("node search follows the wire being dragged", () => {
  const fromPlayer = searchNodes("", { type: "player", dir: "out" }).map((d) => d.type);
  assert.ok(fromPlayer.includes("cond.is_team") && fromPlayer.includes("player.give_weapon"));
  assert.ok(!fromPlayer.includes("score.end_round"), "end round");
  assert.ok(searchNodes("", { type: "number", dir: "out" }).some((d) => d.type === "math.compare"), "compare");
  const intoPlayer = searchNodes("", { type: "player", dir: "in" }).map((d) => d.type);
  assert.ok(intoPlayer.includes("event.player_killed") && !intoPlayer.includes("event.round_started"), "into");
  assert.ok(searchNodes("give").slice(0, 3).some((d) => d.type === "player.give_weapon"));
});

test("copy and paste gives fresh ids and keeps inner wires", () => {
  const g = template();
  const text = copy(g, ["killed", "is_ct", "give_m4"])!;
  const r = paste(g, text, { x: 0, y: 1000 })!;
  assert.equal(r.ids.length, 3);
  assert.ok(r.ids.every((id) => !g.nodes.some((n) => n.id === id)));
  const pasted = new Set(r.ids);
  const inner = r.graph.edges.filter((e) => pasted.has(e.from.node) && pasted.has(e.to.node));
  assert.equal(inner.length, 4);
  assert.equal(Math.min(...r.graph.nodes.filter((n) => pasted.has(n.id)).map((n) => n.pos.y)), 1000);
  assert.equal(paste(g, "not json", { x: 0, y: 0 }), null);
  assert.equal(paste(g, JSON.stringify({ kind: "loner-strike/graph-clip", nodes: [{ id: "a", type: "msg.broadcast", params: { text: "*/" } }], edges: [], comments: [] }), { x: 0, y: 0 }), null);
});

test("undo and redo walk the history", () => {
  let g = emptyGraph("abc", "ABC");
  let h = emptyHistory();
  const a = addNode(g, "event.round_started", { x: 0, y: 0 });
  h = record(h, g);
  g = a.graph;
  const b = addNode(g, "score.end_round", { x: 300, y: 0 });
  h = record(h, g);
  g = b.graph;
  const u = undo(h, g)!;
  assert.equal(u.graph.nodes.length, 1);
  const u2 = undo(u.history, u.graph)!;
  assert.equal(u2.graph.nodes.length, 0);
  assert.equal(undo(u2.history, u2.graph), null);
  const r = redo(u2.history, u2.graph)!;
  assert.equal(r.graph.nodes.length, 1);
  assert.equal(remove(g, [a.id]).nodes.length, 1);
});

test("debug events become runs, a path, a pause and live values", () => {
  const g = template();
  const isEvent = (id: string) => !!nodeDef(g.nodes.find((n) => n.id === id)!.type)?.event;
  let s = emptyDebug();
  s = applyEvents(s, [
    { s: 1, t: 1, e: "session", map: "de_dust2" },
    { s: 2, t: 5, e: "node_enter", node: "killed", inputs: {} },
    { s: 3, t: 5, e: "node_enter", node: "is_ct", inputs: { Player: { name: "Bot", team: "T" } } },
    { s: 4, t: 5, e: "node_exit", node: "is_ct", result: "false" },
    { s: 5, t: 5, e: "node_enter", node: "is_t" },
    { s: 6, t: 5, e: "node_exit", node: "is_t", result: "true" },
    { s: 7, t: 5, e: "breakpoint_hit", node: "give_ak", inputs: { Player: null } },
    { s: 8, t: 5, e: "world_paused", node: "give_ak" },
    { s: 9, t: 5, e: "vars", round: 2, phase: "live", timeLeft: 95, aliveT: 3, aliveCT: 4 },
    { s: 10, t: 5, e: "var_player", p: { id: 1, name: "Bot", team: "T", hp: 100, alive: true, weapon: "ak47", bot: true } },
  ], isEvent, 1000);
  assert.equal(s.map, "de_dust2");
  assert.equal(s.runs.is_ct.result, "false");
  assert.equal(s.runs.is_t.result, "true");
  assert.deepEqual(s.path, ["killed", "is_ct", "is_t", "give_ak"]);
  assert.ok(pathPairs(s.path).has("is_t>give_ak"));
  assert.equal(s.paused?.node, "give_ak");
  assert.equal(s.vars?.aliveCT, 4);
  assert.equal(s.vars?.players[0].weapon, "ak47");
  s = applyEvents(s, [
    { s: 11, t: 6, e: "vars", round: 2, phase: "live", timeLeft: 90, aliveT: 3, aliveCT: 4 },
    { s: 12, t: 6, e: "var_value", v: "v1", p: 0, value: 3 },
    { s: 13, t: 6, e: "var_value", v: "v2", p: 1, value: true },
  ], isEvent, 1000);
  assert.deepEqual(s.vars?.values, [{ v: "v1", p: 0, value: 3 }, { v: "v2", p: 1, value: true }]);
  s = applyEvents(s, [
    { s: 11, t: 9, e: "world_resumed", paused: 4 },
    { s: 12, t: 9, e: "node_enter", node: "give_ak" },
    { s: 13, t: 9, e: "node_error", node: "give_ak", message: "Not given" },
    { s: 14, t: 9, e: "node_enter", node: "start" },
  ], isEvent, 2000);
  assert.equal(s.paused, null);
  assert.equal(s.runs.give_ak.error, "Not given");
  assert.deepEqual(s.path, ["start"], "a new event starts a new path");
  s = applyEvents(s, [{ s: 1, t: 0, e: "session", map: "cs_office" }], isEvent);
  assert.deepEqual(s.runs, {});
});

test("copy and paste between graphs brings the variables along", () => {
  let from = template();
  const v = addVariable(from, { name: "Kills", type: "number", scope: "player", default: 0 });
  from = v.graph;
  const a = addNode(from, "var.add", { x: 0, y: 900 });
  from = { ...a.graph, nodes: a.graph.nodes.map((n) => (n.id === a.id ? { ...n, params: { ...n.params, var: v.id } } : n)) };
  const text = copy(from, [a.id])!;
  // into a graph that already has a variable called v1 (another one)
  const to = { ...emptyGraph("other", "Other"), variables: [{ id: "v1", name: "Lives", type: "number" as const, scope: "mode" as const, default: 3 }] };
  const r = paste(to, text, { x: 0, y: 0 })!;
  assert.equal(r.graph.variables.length, 2);
  const kills = r.graph.variables.find((x) => x.name === "Kills")!;
  assert.notEqual(kills.id, "v1");
  assert.equal(r.graph.nodes[0].params.var, kills.id);
  // pasted again: the same variable, not a second one
  const r2 = paste(r.graph, text, { x: 0, y: 300 })!;
  assert.equal(r2.graph.variables.length, 2);
});

test("templates are laid out: no comment box over another, boxes round their nodes", () => {
  for (const raw of [templateRaw, lastStandRaw, royaleRaw]) {
    const p = parseGraph(JSON.parse(raw));
    if ("error" in p) assert.fail(p.error);
    const g = p.graph;
    // tidying again changes nothing: they ship tidied
    assert.deepEqual(tidyEstimated(g), g);
    const boxes = g.comments.map((c) => ({ x0: c.pos.x, y0: c.pos.y, x1: c.pos.x + c.size.w, y1: c.pos.y + c.size.h }));
    for (let i = 0; i < boxes.length; i++)
      for (let j = i + 1; j < boxes.length; j++) {
        const a = boxes[i], b = boxes[j];
        assert.ok(a.x1 <= b.x0 || b.x1 <= a.x0 || a.y1 <= b.y0 || b.y1 <= a.y0, `${g.mode.id}: boxes ${i} and ${j} overlap`);
      }
  }
});

test("snippets drop in wrapped in a comment, variables merged or renamed", () => {
  const raw = JSON.parse(readFileSync("src/views/tools/studio/snippets/team_score_win.snippet.json", "utf8"));
  const sn = readSnippet("team_score_win", raw)!;
  assert.ok(sn && sn.part.variables.length === 1);
  // a graph that already has "Team score", but as text: the snippet's gets another name
  const g = { ...emptyGraph("x", "X"), variables: [{ id: "v1", name: "Team score", type: "text" as const, scope: "mode" as const, default: "" }] };
  const r = insertSnippet(g, sn, { x: 0, y: 0 })!;
  assert.equal(r.graph.variables.length, 2);
  const added = r.graph.variables.find((v) => v.id !== "v1")!;
  assert.equal(added.name, "Team score 2");
  assert.ok(r.graph.nodes.filter((n) => n.type.startsWith("var.")).every((n) => n.params.var === added.id));
  assert.equal(r.graph.comments.length, 1);
  assert.equal(r.graph.comments[0].text, sn.name);
  // into a graph that already has the same variable: that one is used, not a new one
  const once = insertSnippet(emptyGraph("y", "Y"), sn, { x: 0, y: 0 })!;
  const twice = insertSnippet(once.graph, sn, { x: 0, y: 900 })!;
  assert.equal(twice.graph.variables.length, 1);
  // a saved selection reads back as a snippet
  const made = makeSnippet(r.graph, r.ids, "Mine");
  assert.ok(readSnippet("mine", made));
});

// ---- graph version 3

test("Functions: their own graph with entry and return; calls take their pins", () => {
  let g = emptyGraph("fx", "Fx");
  const f = addFunction(g);
  g = f.graph;
  const entry = g.nodes.find((n) => n.graph === f.id && n.type === "fn.entry")!;
  const ret = g.nodes.find((n) => n.graph === f.id && n.type === "fn.return")!;
  assert.ok(entry && ret && g.edges.some((e) => e.from.node === entry.id && e.to.node === ret.id));
  g = updateFunction(g, f.id, { inputs: [{ id: "a1", name: "Who", type: "player" }], outputs: [{ id: "r1", name: "Score", type: "number" }] });
  assert.deepEqual(pinsOf(g, g.nodes.find((n) => n.id === entry.id)!).outputs.map((p) => p.id), ["then", "a1"]);
  const call = addNode(g, "fn.call", { x: 0, y: 0 }, undefined, { func: f.id });
  g = call.graph;
  const pins = pinsOf(g, g.nodes.find((n) => n.id === call.id)!);
  assert.deepEqual(pins.inputs.map((p) => `${p.id}:${p.type}`), ["exec:exec", "a1:player"]);
  assert.deepEqual(pins.outputs.map((p) => `${p.id}:${p.type}`), ["then:exec", "r1:number"]);
  // tabs: the Event Graph doesn't show the Function's nodes; wires can't cross
  assert.equal(tabView(g, "").nodes.length, 1);
  assert.equal(tabView(g, f.id).nodes.length, 2);
  assert.equal(canConnect(g, { node: entry.id, pin: "a1" }, { node: call.id, pin: "a1" }).ok, false);
  // removing it takes its graph along
  const gone = removeGraphPart(g, f.id);
  assert.equal(gone.nodes.length, 1);
  assert.ok(!gone.functions?.length);
});

test("generic nodes take the type of the first wire, arrays only fit arrays", () => {
  let g = { ...emptyGraph("ax", "Ax"), variables: [{ id: "v1", name: "Names", type: "array<text>", scope: "mode" as const }] };
  const get = addNode(g, "var.get", { x: 0, y: 0 }, undefined, { var: "v1" });
  g = get.graph;
  const len = addNode(g, "array.length", { x: 200, y: 0 });
  g = len.graph;
  const r = connect(g, { node: get.id, pin: "out" }, { node: len.id, pin: "array" });
  assert.ok("graph" in r);
  if ("graph" in r) {
    assert.equal(r.graph.nodes.find((n) => n.id === len.id)!.params.of, "text");
    // a number into an array pin: no
    const num = addNode(r.graph, "math.number", { x: 0, y: 100 });
    assert.equal(canConnect(num.graph, { node: num.id, pin: "out" }, { node: len.id, pin: "array" }).ok, false);
  }
  assert.ok(compatible("array<any>", "array<number>") && !compatible("array<number>", "array<text>") && !compatible("number", "array<number>"));
});

test("a body wired into its loop's Break isn't a flow loop", () => {
  let g = emptyGraph("lx", "Lx");
  const ev = addNode(g, "event.round_started", { x: 0, y: 0 });
  const loop = addNode(ev.graph, "flow.for_loop_break", { x: 200, y: 0 });
  const br = addNode(loop.graph, "flow.branch", { x: 400, y: 0 });
  g = br.graph;
  for (const [a, ap, b, bp] of [[ev.id, "then", loop.id, "exec"], [loop.id, "body", br.id, "exec"]] as const) {
    const r = connect(g, { node: a, pin: ap }, { node: b, pin: bp });
    assert.ok("graph" in r);
    if ("graph" in r) g = r.graph;
  }
  assert.ok(canConnect(g, { node: br.id, pin: "true" }, { node: loop.id, pin: "break" }).ok);
  assert.equal(canConnect(g, { node: br.id, pin: "true" }, { node: loop.id, pin: "exec" }).ok, false);
});

test("Collapse to Function: the selection moves to a Function, a call takes its place", () => {
  let g = template();
  const before = g.nodes.length;
  // the attacker's weapon and health: one flow in, one out
  const pick = g.nodes.filter((n) => n.type === "player.give_weapon").map((n) => n.id).slice(0, 1);
  const r = collapse(g, pick, "function");
  if ("error" in r) assert.fail(r.error);
  g = r.graph;
  const f = g.functions!.find((x) => x.id === r.id)!;
  assert.ok(f.inputs.some((m) => m.type === "player"), JSON.stringify(f));
  assert.equal(g.nodes.filter((n) => !n.graph).length, before - pick.length + 1);
  const call = g.nodes.find((n) => n.type === "fn.call")!;
  assert.equal(call.params.func, r.id);
  // every wire stays inside one graph
  for (const e of g.edges) {
    const a = g.nodes.find((n) => n.id === e.from.node)!;
    const b = g.nodes.find((n) => n.id === e.to.node)!;
    assert.equal(a.graph ?? "", b.graph ?? "", e.id);
  }
  // and the Function checks as data (LS_COLLAPSE_OUT: written for the app's checks too)
  assert.ok("graph" in parseGraph(JSON.parse(JSON.stringify(g))));
  if (process.env.LS_COLLAPSE_OUT) writeFileSync(process.env.LS_COLLAPSE_OUT, JSON.stringify(g, null, 1));
  // nothing to collapse
  assert.ok("error" in collapse(g, [], "macro"));
});

test("Macros, structs and enums give their nodes pins", () => {
  let g = emptyGraph("mx", "Mx");
  const m = addMacro(g);
  g = m.graph;
  const use = addNode(g, "macro.use", { x: 0, y: 0 }, undefined, { macro: m.id });
  g = use.graph;
  assert.deepEqual(pinsOf(g, g.nodes.find((n) => n.id === use.id)!).inputs.map((p) => p.type), ["exec"]);
  const s = addStruct(g);
  g = updateStruct(s.graph, s.id, { fields: [{ id: "f1", name: "Ammo", type: "number" }, { id: "f2", name: "Tag", type: "text" }] });
  const mk = addNode(g, "struct.make", { x: 0, y: 0 }, undefined, { struct: s.id });
  g = mk.graph;
  const mp = pinsOf(g, g.nodes.find((n) => n.id === mk.id)!);
  assert.deepEqual(mp.inputs.map((p) => p.type), ["number", "text"]);
  assert.equal(mp.outputs[0].type, `struct:${s.id}`);
  const e = addEnum(g);
  const sw = addNode(e.graph, "flow.switch_enum", { x: 0, y: 0 }, undefined, { enum: e.id });
  assert.deepEqual(pinsOf(sw.graph, sw.graph.nodes.find((n) => n.id === sw.id)!).outputs.map((p) => p.label), ["First", "Second"]);
  assert.equal(typeLabel(sw.graph, `array<struct:${s.id}>`), "Array of New Struct");
  assert.ok(typeOk(sw.graph, `map<player,enum:${e.id}>`) && !typeOk(sw.graph, `map<player,struct:${s.id}>`) && !typeOk(sw.graph, "array<array<number>>"));
});

// ---- My library

test("a library item brings what it needs, merges what's there, renames clashes, updates in place", () => {
  // a mode: enum Class, struct Kit (with a Class), variable Kits, Function Give (takes a Kit, calls Helper, sets Kits)
  let g: Graph = {
    ...emptyGraph("src", "Src"),
    enums: [{ id: "e1", name: "Class", values: ["Scout", "Tank"] }],
    structs: [{ id: "s1", name: "Kit", fields: [{ id: "f1", name: "Ammo", type: "number" }, { id: "f2", name: "Class", type: "enum:e1" }] }],
    variables: [{ id: "v1", name: "Kits", type: "array<struct:s1>", scope: "mode" }],
  };
  const helper = addFunction(g, false, "Helper");
  g = helper.graph;
  const give = addFunction(g, false, "Give");
  g = updateFunction(give.graph, give.id, { inputs: [{ id: "a1", name: "Kit", type: "struct:s1" }] });
  const call = addNode(g, "fn.call", { x: 200, y: 0 }, give.id, { func: helper.id });
  const push = addNode(call.graph, "array.add", { x: 400, y: 0 }, give.id, { of: "struct:s1" });
  const kits = addNode(push.graph, "var.get", { x: 300, y: 100 }, give.id, { var: "v1" });
  g = kits.graph;
  const entry = g.nodes.find((n) => n.graph === give.id && n.type === "fn.entry")!;
  const ret = g.nodes.find((n) => n.graph === give.id && n.type === "fn.return")!;
  g = { ...g, edges: g.edges.filter((e) => e.from.node !== entry.id) };
  const wire = (a: string, ap: string, b: string, bp: string) => {
    const r = connect(g, { node: a, pin: ap }, { node: b, pin: bp });
    if ("error" in r) assert.fail(r.error);
    g = r.graph;
  };
  wire(entry.id, "then", call.id, "exec");
  wire(call.id, "then", push.id, "exec");
  wire(push.id, "then", ret.id, "exec");
  wire(kits.id, "out", push.id, "array");
  wire(entry.id, "a1", push.id, "item");
  const hp = g.nodes.find((n) => n.graph === helper.id && n.type === "fn.entry")!;
  const hpr = addNode(g, "debug.print", { x: 200, y: 0 }, helper.id, { text: "helper ran", chat: false });
  g = { ...hpr.graph, edges: hpr.graph.edges.filter((e) => e.from.node !== hp.id) };
  wire(hp.id, "then", hpr.id, "exec");
  const x = extractItem(g, give.id);
  if ("error" in x) assert.fail(x.error);
  assert.equal(x.kind, "function");
  assert.deepEqual(x.body.functions.map((f) => f.name).sort(), ["Give", "Helper"]);
  assert.deepEqual([x.body.structs.length, x.body.enums.length, x.body.variables.length], [1, 1, 1]);
  const file = itemFile({ name: "Give", description: "Gives a kit", category: "Kits", kind: x.kind, root: give.id, body: x.body, itemVersion: 1 });
  const it = readItem("give_kit", JSON.parse(JSON.stringify(file)));
  assert.ok(it);
  // another mode that already has the same Class, and a different "Kit"
  let other: Graph = {
    ...emptyGraph("dst", "Dst"),
    enums: [{ id: "e7", name: "Class", values: ["Scout", "Tank"] }],
    structs: [{ id: "s7", name: "Kit", fields: [{ id: "q", name: "Other", type: "text" }] }],
  };
  const r = insertItem(other, it!);
  other = r.graph;
  assert.equal(other.enums!.length, 1, "the same enum is used again");
  assert.deepEqual(other.structs!.map((s) => s.name), ["Kit", "Kit 2"], "a clash is renamed");
  const kit2 = other.structs!.find((s) => s.name === "Kit 2")!;
  assert.equal(kit2.fields[1].type, "enum:e7", "types point at the mode's own");
  const root = other.functions!.find((f) => f.id === r.root)!;
  assert.deepEqual(root.from, { lib: "give_kit", version: 1 });
  assert.equal(root.inputs[0].type, `struct:${kit2.id}`);
  const inner = other.nodes.find((n) => n.graph === root.id && n.type === "fn.call")!;
  assert.equal(other.functions!.find((f) => f.id === inner.params.func)!.name, "Helper");
  assert.ok("graph" in parseGraph(JSON.parse(JSON.stringify(other))));
  if (process.env.LS_LIB_OUT) {
    // a mode that uses it: each round, every living player gets a Kit through the library's Give
    let m: Graph = { ...other, mode: { id: "libuse", name: "Library use", description: "" } };
    const ev = addNode(m, "event.round_started", { x: 0, y: 600 });
    const each = addNode(ev.graph, "flow.for_each_player", { x: 200, y: 600 });
    const mk = addNode(each.graph, "struct.make", { x: 200, y: 800 }, undefined, { struct: kit2.id });
    const n30 = addNode(mk.graph, "math.number", { x: 0, y: 800 }, undefined, { value: 30 });
    const giveCall = addNode(n30.graph, "fn.call", { x: 400, y: 600 }, undefined, { func: root.id });
    const kitsVar = other.variables.find((v) => v.name === "Kits")!;
    const getK = addNode(giveCall.graph, "var.get", { x: 400, y: 800 }, undefined, { var: kitsVar.id });
    const len = addNode(getK.graph, "array.length", { x: 600, y: 800 }, undefined, { of: `struct:${kit2.id}` });
    const pr = addNode(len.graph, "debug.print", { x: 800, y: 600 }, undefined, { text: "kits given:", chat: false });
    m = pr.graph;
    for (const [a, ap, b, bp] of [[ev.id, "then", each.id, "exec"], [each.id, "each", giveCall.id, "exec"], [n30.id, "out", mk.id, "f1"], [mk.id, "out", giveCall.id, "a1"], [each.id, "then", pr.id, "exec"], [getK.id, "out", len.id, "array"], [len.id, "out", pr.id, "value"]]) {
      const r = connect(m, { node: a, pin: ap }, { node: b, pin: bp });
      if ("error" in r) assert.fail(`${a}.${ap} -> ${b}.${bp}: ${r.error}`);
      m = r.graph;
    }
    writeFileSync(process.env.LS_LIB_OUT, JSON.stringify(m, null, 1));
  }
  // the same item again: nothing new
  const again = insertItem(other, it!);
  assert.equal(again.graph.functions!.length, other.functions!.length);
  // version 2 (Give gains an output): "Update from library" replaces it in place
  const v2 = { ...it!, itemVersion: 2, body: { ...it!.body, functions: it!.body.functions.map((f) => (f.id === give.id ? { ...f, outputs: [{ id: "r1", name: "Count", type: "number" }] } : f)) } };
  assert.equal(newerInLibrary([v2], root.from)?.itemVersion, 2);
  const up = insertItem(other, v2, root.id);
  const root2 = up.graph.functions!.find((f) => f.id === root.id)!;
  assert.equal(root2.outputs.length, 1);
  assert.deepEqual(root2.from, { lib: "give_kit", version: 2 });
  assert.equal(up.graph.functions!.length, other.functions!.length, "no second copy");
  assert.equal(up.graph.nodes.filter((n) => n.graph === root.id && n.type === "fn.entry").length, 1);
});

test("UX review H5: search puts title matches first (killed: Player Killed before Bot Trait)", () => {
  const titles = searchNodes("killed").map((d) => d.title);
  assert.equal(titles[0], "Player Killed");
  assert.ok(titles.indexOf("Player Killed") < titles.indexOf("Bot Trait") || !titles.includes("Bot Trait"));
  assert.equal(searchNodes("give w")[0].title, "Give Weapon");
  assert.equal(matchRank("Player Killed", "player killed"), 0);
  assert.ok(matchRank("Player Killed", "player") < matchRank("Player Killed", "killed"));
  assert.ok(matchRank("Bot Trait", "skilled", "skill skilled") > matchRank("Player Killed", "killed"));
});

test("UX review M4: Get/Set show Player and Team only for a per-player or per-team variable", () => {
  for (const scope of ["mode", "player", "team"] as const) {
    let g = template();
    g = { ...g, variables: [...g.variables, { id: "vx", name: "X", type: "number", scope, default: 0 }] };
    for (const type of ["var.get", "var.set", "var.add"]) {
      const r = addNode(g, type, { x: 0, y: 0 }, undefined, { var: "vx" });
      const ids = pinsOf(r.graph, r.graph.nodes.find((n) => n.id === r.id)!).inputs.map((p) => p.id);
      assert.equal(ids.includes("player"), scope === "player", `${type} ${scope} player`);
      assert.equal(ids.includes("team"), scope === "team", `${type} ${scope} team`);
    }
  }
});

test("StudioPerfAudit #5: a sampled run's count includes the runs before it that weren't written", () => {
  const d = applyEvents(emptyDebug(), [
    { s: 1, t: 1, e: "node_enter", node: "a" },
    { s: 2, t: 2, e: "node_enter", node: "a", skipped: 30 },
  ], () => false);
  assert.equal(d.runs.a.count, 32);
});

test("the built-in Run Dialog brings its Dialog table; adding it again uses the same table", () => {
  const it = readItem("builtin_run_dialog", JSON.parse(runDialogRaw));
  assert.ok(it, "the item reads");
  assert.equal(it!.kind, "macro");
  assert.equal(it!.body.dataTables?.[0].name, "Dialog");
  assert.equal(it!.body.dataTables?.[0].columns.length, 15);
  const g0 = emptyGraph("talk", "Talk");
  const r = insertItem(g0, it!);
  assert.equal(r.graph.dataTables?.length, 1);
  const t = r.graph.dataTables![0];
  assert.equal(t.rows.length, 5);
  // every Get Row in the copy reads the table that came in
  const rows = r.graph.nodes.filter((n) => n.type === "table.get_row");
  assert.ok(rows.length > 0 && rows.every((n) => n.params.table === t.id));
  assert.ok("graph" in parseGraph(JSON.parse(JSON.stringify(r.graph))), "the mode still parses");
  // the same again: a second macro, the one table (its rows are the mode's)
  const twice = insertItem(r.graph, it!);
  assert.equal(twice.graph.dataTables?.length, 1);
  // a mode that has another table called Dialog: the item's comes in under a free name
  const other = { ...g0, dataTables: [{ ...t, id: "t1", columns: t.columns.slice(0, 3), rows: [] }] };
  const r2 = insertItem(other, it!);
  assert.deepEqual(r2.graph.dataTables!.map((x) => x.name), ["Dialog", "Dialog 2"]);
  assert.ok(r2.graph.nodes.filter((n) => n.type === "table.get_row").every((n) => n.params.table === r2.graph.dataTables![1].id));
  // and a Function or Macro that reads a table takes it with it when saved to the library
  const back = extractItem(r.graph, r.root);
  assert.ok(!("error" in back) && back.body.dataTables?.[0].id === t.id);
});
