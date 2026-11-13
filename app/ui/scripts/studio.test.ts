// Game Mode Studio unit tests (kept out of src/ so the app build never sees node:test): the graph schema, wiring rules, copy/paste, undo/redo and what debug
// events mean. Run with `npm run test:studio` (esbuild + node:test, no extra packages).

import { test } from "node:test";
import assert from "node:assert/strict";
import templateRaw from "../src/views/tools/studio/templates/team-dm.graph.json?raw";
import { parseGraph, emptyGraph, type Graph } from "../src/views/tools/studio/schema";
import { addNode, canConnect, connect, copy, emptyHistory, insertReroute, inType, outType, paste, record, redo, remove, undo } from "../src/views/tools/studio/graphOps";
import { NODE_DEFS, nodeDef, searchNodes } from "../src/views/tools/studio/registry";
import { applyEvents, emptyDebug, pathPairs } from "../src/views/tools/studio/debugState";

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
    for (const p of [...d.inputs, ...d.outputs]) assert.ok(["exec", "player", "team", "any"].includes(p.type), `${d.type}.${p.id}`);
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

test("node search follows the wire being dragged", () => {
  const fromPlayer = searchNodes("", { type: "player", dir: "out" }).map((d) => d.type);
  assert.ok(fromPlayer.includes("cond.is_team") && fromPlayer.includes("player.give_weapon"));
  assert.ok(!fromPlayer.includes("score.end_round"));
  const intoPlayer = searchNodes("", { type: "player", dir: "in" }).map((d) => d.type);
  assert.ok(intoPlayer.includes("event.player_killed") && !intoPlayer.includes("event.round_started"));
  assert.equal(searchNodes("give")[0].type, "player.give_weapon");
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
