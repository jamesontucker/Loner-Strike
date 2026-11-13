// Edits on the graph JSON, kept pure (graph in, graph out) so undo/redo is a stack of graphs
// and the rules are testable without React (studio.test.ts). The app checks the same rules
// again before compiling (tools/studio/graph.rs).

import { parseGraph, type GComment, type GEdge, type GFunction, type GMacro, type GMember, type GNode, type GVariable, type Graph } from "./schema";
import { compatible, defaultParams, nodeDef, type PinDef, type PinType } from "./registry";
import { adaptTo, allVariables, pinsOf, typeLabel } from "./pins";

export type PinRef = { node: string; pin: string };

/** A fresh id not used by any node, wire or comment: prefix + the lowest free number. */
export function freshId(g: Graph, prefix: string, taken: Set<string> = new Set()): string {
  const used = new Set<string>([...g.nodes.map((n) => n.id), ...g.edges.map((e) => e.id), ...g.comments.map((c) => c.id), ...taken]);
  for (let i = 1; ; i++) {
    const id = `${prefix}${i}`;
    if (!used.has(id)) return id;
  }
}

const findNode = (g: Graph, id: string) => g.nodes.find((n) => n.id === id);

export function outPin(g: Graph, ref: PinRef): PinDef | undefined {
  const n = findNode(g, ref.node);
  return n ? pinsOf(g, n).outputs.find((p) => p.id === ref.pin) : undefined;
}

export function inPin(g: Graph, ref: PinRef): PinDef | undefined {
  const n = findNode(g, ref.node);
  return n ? pinsOf(g, n).inputs.find((p) => p.id === ref.pin) : undefined;
}

/** The type an output really carries: a reroute carries whatever comes into it ("any" when
 *  nothing does yet). */
export function outType(g: Graph, ref: PinRef, seen = new Set<string>()): PinType | undefined {
  const n = findNode(g, ref.node);
  const def = n && nodeDef(n.type);
  if (!def) return undefined;
  if (!def.reroute) return outPin(g, ref)?.type;
  if (seen.has(n.id)) return "any";
  seen.add(n.id);
  const e = g.edges.find((x) => x.to.node === n.id && x.to.pin === "in");
  return e ? outType(g, e.from, seen) : "any";
}

/** The type an input wants: a reroute wants whatever its output already feeds ("any" if
 *  nothing). */
export function inType(g: Graph, ref: PinRef, seen = new Set<string>()): PinType | undefined {
  const n = findNode(g, ref.node);
  const def = n && nodeDef(n.type);
  if (!def) return undefined;
  if (!def.reroute) return inPin(g, ref)?.type;
  if (seen.has(n.id)) return "any";
  seen.add(n.id);
  const e = g.edges.find((x) => x.from.node === n.id && x.from.pin === "out");
  return e ? inType(g, e.to, seen) : "any";
}

/** Flow into Reset, Open, Close, Toggle or Break only changes the node: it doesn't go on. */
export function passes(g: Graph, node: string, pin: string): boolean {
  const t = findNode(g, node)?.type ?? "";
  return pin === "exec" || !["flow.do_once", "flow.do_n", "flow.gate", "flow.for_loop_break", "flow.for_each_array"].includes(t);
}

/** Would the flow loop if `from` were wired to `to`? (Only flow wires can loop.) */
function makesLoop(g: Graph, from: string, to: PinRef): boolean {
  if (!passes(g, to.node, to.pin)) return false;
  const stack = [to.node];
  const seen = new Set<string>();
  while (stack.length) {
    const cur = stack.pop()!;
    if (cur === from) return true;
    if (seen.has(cur)) continue;
    seen.add(cur);
    for (const e of g.edges) if (e.from.node === cur && outType(g, e.from) === "exec" && passes(g, e.to.node, e.to.pin)) stack.push(e.to.node);
  }
  return false;
}

export type Check = { ok: true; replaces: string[] } | { ok: false; error: string };

/** Settings a generic node (an array's Get, Equal...) takes to fit this wire, or null. */
function adaptFor(g: Graph, from: PinRef, to: PinRef): { node: string; patch: Record<string, string> } | null {
  const a = findNode(g, to.node);
  const b = findNode(g, from.node);
  const ft = outType(g, from) ?? "any";
  const tt = inType(g, to) ?? "any";
  const pa = a && adaptTo(g, a, to.pin, "in", ft);
  if (pa) return { node: a.id, patch: pa };
  const pb = b && adaptTo(g, b, from.pin, "out", tt);
  return pb ? { node: b.id, patch: pb } : null;
}

const withParams = (g: Graph, node: string, patch: Record<string, string>): Graph => ({ ...g, nodes: g.nodes.map((n) => (n.id === node ? { ...n, params: { ...n.params, ...patch } } : n)) });

/** Can this wire be drawn? A new wire into a taken data input (or from a taken flow output)
 *  replaces the old one, as in Unreal's Blueprints: `replaces` lists those. A generic node
 *  (array, map, Equal...) takes the wire's type: `adapt` says how. */
export function canConnect(g: Graph, from: PinRef, to: PinRef): Check & { adapt?: { node: string; patch: Record<string, string> } } {
  if (from.node === to.node) return { ok: false, error: "A wire can't go back into the same node." };
  const op = outPin(g, from);
  const ip = inPin(g, to);
  if (!op || !ip) return { ok: false, error: "Wires go from an output (right) to an input (left)." };
  if ((findNode(g, from.node)?.graph ?? "") !== (findNode(g, to.node)?.graph ?? "")) return { ok: false, error: "A wire can't go from one graph to another." };
  let ft = outType(g, from)!;
  let tt = inType(g, to)!;
  let adapt: { node: string; patch: Record<string, string> } | undefined;
  if (!compatible(ft, tt)) {
    const a = adaptFor(g, from, to);
    const g2 = a && withParams(g, a.node, a.patch);
    if (!a || !g2 || !compatible(outType(g2, from) ?? "any", inType(g2, to) ?? "any")) return { ok: false, error: `A ${typeLabel(g, ft)} wire can't go into a ${typeLabel(g, tt)} pin.` };
    adapt = a;
    ft = outType(g2, from)!;
    tt = inType(g2, to)!;
  } else if (!adapt) {
    // a generic node still on its first setting takes the wire's type
    const a = adaptFor(g, from, to);
    const n = a && findNode(g, a.node);
    const wiredAlready = n && g.edges.some((e) => e.from.node === n.id || e.to.node === n.id);
    if (a && !wiredAlready) adapt = a;
  }
  if (g.edges.some((e) => e.from.node === from.node && e.from.pin === from.pin && e.to.node === to.node && e.to.pin === to.pin)) return { ok: false, error: "Those are already wired." };
  const flow = ft === "exec" || tt === "exec";
  if (flow && makesLoop(g, from.node, to)) return { ok: false, error: "That would make the flow go round in a circle. Use a For Loop, While Loop or Repeat." };
  const toReroute = nodeDef(findNode(g, to.node)!.type)?.reroute;
  const replaces = g.edges
    .filter((e) => (flow && e.from.node === from.node && e.from.pin === from.pin) || ((!flow || toReroute) && e.to.node === to.node && e.to.pin === to.pin))
    .map((e) => e.id);
  return { ok: true, replaces, adapt };
}

export function connect(g: Graph, from: PinRef, to: PinRef): { graph: Graph } | { error: string } {
  const c = canConnect(g, from, to);
  if (!c.ok) return { error: c.error };
  const edge: GEdge = { id: freshId(g, "w"), from: { ...from }, to: { ...to } };
  const g2 = c.adapt ? withParams(g, c.adapt.node, c.adapt.patch) : g;
  return { graph: { ...g2, edges: [...g2.edges.filter((e) => !c.replaces.includes(e.id)), edge] } };
}

/** Adds a node; `home` is the Function or Macro whose graph it goes on (none: the Event Graph). */
export function addNode(g: Graph, type: string, pos: { x: number; y: number }, home?: string, params?: Record<string, string | number | boolean>): { graph: Graph; id: string } {
  const def = nodeDef(type);
  if (!def) throw new Error(`unknown node ${type}`);
  const id = freshId(g, "n");
  const node: GNode = { id, type, pos: { x: Math.round(pos.x), y: Math.round(pos.y) }, params: { ...defaultParams(def), ...params }, ...(home ? { graph: home } : {}) };
  return { graph: { ...g, nodes: [...g.nodes, node] }, id };
}

export function addComment(g: Graph, around: { x: number; y: number; w: number; h: number }, text = "Comment", home?: string): { graph: Graph; id: string } {
  const id = freshId(g, "c");
  const c: GComment = { id, text, pos: { x: Math.round(around.x), y: Math.round(around.y) }, size: { w: Math.max(160, Math.round(around.w)), h: Math.max(80, Math.round(around.h)) }, ...(home ? { graph: home } : {}) };
  return { graph: { ...g, comments: [...g.comments, c] }, id };
}

/** The nodes, wires and comments on one tab ("" the Event Graph), as a graph of their own
 *  (for the canvas and Tidy up). */
export function tabView(g: Graph, tab: string): Graph {
  const nodes = g.nodes.filter((n) => (n.graph ?? "") === tab);
  const ids = new Set(nodes.map((n) => n.id));
  return { ...g, nodes, edges: g.edges.filter((e) => ids.has(e.from.node) && ids.has(e.to.node)), comments: g.comments.filter((c) => (c.graph ?? "") === tab) };
}

/** Removes nodes (with their wires), wires and comments. */
export function remove(g: Graph, ids: Iterable<string>): Graph {
  const gone = new Set(ids);
  return {
    ...g,
    nodes: g.nodes.filter((n) => !gone.has(n.id)),
    comments: g.comments.filter((c) => !gone.has(c.id)),
    edges: g.edges.filter((e) => !gone.has(e.id) && !gone.has(e.from.node) && !gone.has(e.to.node)),
  };
}

/** Splits a wire with a reroute at `pos`. */
export function insertReroute(g: Graph, edgeId: string, pos: { x: number; y: number }): Graph {
  const e = g.edges.find((x) => x.id === edgeId);
  if (!e) return g;
  const { graph, id } = addNode(g, "flow.reroute", pos, findNode(g, e.from.node)?.graph);
  const a: GEdge = { id: freshId(graph, "w"), from: e.from, to: { node: id, pin: "in" } };
  const b: GEdge = { id: freshId(graph, "w", new Set([a.id])), from: { node: id, pin: "out" }, to: e.to };
  return { ...graph, edges: [...graph.edges.filter((x) => x.id !== edgeId), a, b] };
}

export function setParam(g: Graph, node: string, param: string, value: string | number | boolean): Graph {
  return { ...g, nodes: g.nodes.map((n) => (n.id === node ? { ...n, params: { ...n.params, [param]: value } } : n)) };
}

// ---- variables -----------------------------------------------------------------------------

export function addVariable(g: Graph, v: Omit<GVariable, "id">): { graph: Graph; id: string } {
  const id = freshPartId(g, "v");
  return { graph: { ...g, variables: [...g.variables, { ...v, id }] }, id };
}

/** A start value for a type (only numbers, yes/no, text and enums have one). */
export function startValue(type: string): GVariable["default"] {
  if (type === "number" || type.startsWith("enum:")) return 0;
  if (type === "bool") return false;
  if (type === "text") return "";
  return undefined;
}

/** Changes a variable or a Function's local. */
export function updateVariable(g: Graph, id: string, patch: Partial<Omit<GVariable, "id">>): Graph {
  const change = (x: GVariable): GVariable => {
    if (x.id !== id) return x;
    const next = { ...x, ...patch };
    // a new type: the start value starts again (and arrays, maps, structs are for the whole mode)
    if (patch.type && patch.type !== x.type) {
      next.default = startValue(patch.type);
      if (/^(array|map)<|^struct:/.test(patch.type)) next.scope = "mode";
    }
    if (next.default === undefined) delete next.default;
    return next;
  };
  return { ...g, variables: g.variables.map(change), functions: g.functions?.map((f) => (f.locals?.some((x) => x.id === id) ? { ...f, locals: f.locals.map(change) } : f)) };
}

/** Removes a variable (or a local); nodes set to it keep their place and show a problem until another is picked. */
export function removeVariable(g: Graph, id: string): Graph {
  return { ...g, variables: g.variables.filter((x) => x.id !== id), functions: g.functions?.map((f) => (f.locals?.some((x) => x.id === id) ? { ...f, locals: f.locals.filter((x) => x.id !== id) } : f)) };
}

/** A fresh id for a variable, local, Function, Macro, struct or enum. */
export function freshPartId(g: Graph, prefix: string): string {
  const used = new Set([...allVariables(g).map((x) => x.id), ...(g.functions ?? []).map((x) => x.id), ...(g.macros ?? []).map((x) => x.id), ...(g.structs ?? []).map((x) => x.id), ...(g.enums ?? []).map((x) => x.id)]);
  for (let i = 1; ; i++) if (!used.has(`${prefix}${i}`)) return `${prefix}${i}`;
}

export function addLocal(g: Graph, fn: string, v: Omit<GVariable, "id">): { graph: Graph; id: string } {
  const id = freshPartId(g, "l");
  return { graph: { ...g, functions: (g.functions ?? []).map((f) => (f.id === fn ? { ...f, locals: [...(f.locals ?? []), { ...v, id }] } : f)) }, id };
}

export const uniqueName = (taken: string[], base: string) => {
  let name = base;
  for (let n = 2; taken.some((t) => t.toLowerCase() === name.toLowerCase()); n++) name = `${base.slice(0, 26)} ${n}`;
  return name;
};

/** A new Function, with its Function Entry and Return Node on its own graph. */
export function addFunction(g: Graph, pure = false, name?: string): { graph: Graph; id: string } {
  const id = freshPartId(g, "f");
  const taken = [...(g.functions ?? []), ...(g.macros ?? [])].map((x) => x.name);
  const f: GFunction = { id, name: uniqueName(taken, name ?? (pure ? "Pure Function" : "New Function")), ...(pure ? { pure: true } : {}), inputs: [], outputs: [] };
  let g2: Graph = { ...g, functions: [...(g.functions ?? []), f] };
  const e = addNode(g2, "fn.entry", { x: 0, y: 0 }, id);
  const r = addNode(e.graph, "fn.return", { x: 420, y: 0 }, id);
  g2 = r.graph;
  return { graph: { ...g2, edges: [...g2.edges, { id: freshId(g2, "w"), from: { node: e.id, pin: "then" }, to: { node: r.id, pin: "exec" } }] }, id };
}

/** A new Macro, with its Macro Entry and Exit (one flow pin each to start with). */
export function addMacro(g: Graph, name?: string): { graph: Graph; id: string } {
  const id = freshPartId(g, "m");
  const taken = [...(g.functions ?? []), ...(g.macros ?? [])].map((x) => x.name);
  const m: GMacro = { id, name: uniqueName(taken, name ?? "New Macro"), inputs: [{ id: "a1", name: "In", type: "exec" }], outputs: [{ id: "r1", name: "Out", type: "exec" }] };
  const g1: Graph = { ...g, macros: [...(g.macros ?? []), m] };
  const e = addNode(g1, "macro.entry", { x: 0, y: 0 }, id);
  const x = addNode(e.graph, "macro.exit", { x: 420, y: 0 }, id);
  const g2 = x.graph;
  return { graph: { ...g2, edges: [...g2.edges, { id: freshId(g2, "w"), from: { node: e.id, pin: "a1" }, to: { node: x.id, pin: "r1" } }] }, id };
}

export function updateFunction(g: Graph, id: string, patch: Partial<Omit<GFunction, "id">>): Graph {
  return { ...g, functions: (g.functions ?? []).map((f) => (f.id === id ? { ...f, ...patch } : f)) };
}

export function updateMacro(g: Graph, id: string, patch: Partial<Omit<GMacro, "id">>): Graph {
  return { ...g, macros: (g.macros ?? []).map((m) => (m.id === id ? { ...m, ...patch } : m)) };
}

/** Removes a Function or Macro with its graph; calls to it stay and show a problem. */
export function removeGraphPart(g: Graph, id: string): Graph {
  const gone = new Set(g.nodes.filter((n) => n.graph === id).map((n) => n.id));
  const r = remove(g, gone);
  return { ...r, comments: r.comments.filter((c) => c.graph !== id), functions: g.functions?.filter((f) => f.id !== id), macros: g.macros?.filter((m) => m.id !== id) };
}

/** A fresh member id (input, output, field) for a list. */
export function freshMember(list: GMember[], prefix: string): string {
  for (let i = 1; ; i++) if (!list.some((m) => m.id === `${prefix}${i}`)) return `${prefix}${i}`;
}

export function addStruct(g: Graph): { graph: Graph; id: string } {
  const id = freshPartId(g, "s");
  const name = uniqueName((g.structs ?? []).map((x) => x.name), "New Struct");
  return { graph: { ...g, structs: [...(g.structs ?? []), { id, name, fields: [{ id: "f1", name: "Value", type: "number" }] }] }, id };
}

export function updateStruct(g: Graph, id: string, patch: Partial<{ name: string; fields: GMember[] }>): Graph {
  return { ...g, structs: (g.structs ?? []).map((x) => (x.id === id ? { ...x, ...patch } : x)) };
}

export function addEnum(g: Graph): { graph: Graph; id: string } {
  const id = freshPartId(g, "e");
  const name = uniqueName((g.enums ?? []).map((x) => x.name), "New Enum");
  return { graph: { ...g, enums: [...(g.enums ?? []), { id, name, values: ["First", "Second"] }] }, id };
}

export function updateEnum(g: Graph, id: string, patch: Partial<{ name: string; values: string[] }>): Graph {
  return { ...g, enums: (g.enums ?? []).map((x) => (x.id === id ? { ...x, ...patch } : x)) };
}

/** Removes a struct or enum (variables and nodes using it show a problem until changed). */
export function removeType(g: Graph, id: string): Graph {
  return { ...g, structs: g.structs?.filter((x) => x.id !== id), enums: g.enums?.filter((x) => x.id !== id) };
}

export function toggleBreakpoint(g: Graph, node: string): Graph {
  return { ...g, nodes: g.nodes.map((n) => (n.id === node ? { ...n, breakpoint: !n.breakpoint || undefined } : n)) };
}

export function moveItems(g: Graph, moves: Map<string, { x: number; y: number }>): Graph {
  if (!moves.size) return g;
  const at = (id: string, p: { x: number; y: number }) => {
    const m = moves.get(id);
    return m ? { x: Math.round(m.x), y: Math.round(m.y) } : p;
  };
  return { ...g, nodes: g.nodes.map((n) => (moves.has(n.id) ? { ...n, pos: at(n.id, n.pos) } : n)), comments: g.comments.map((c) => (moves.has(c.id) ? { ...c, pos: at(c.id, c.pos) } : c)) };
}

export function resizeComment(g: Graph, id: string, size: { w: number; h: number }, pos?: { x: number; y: number }): Graph {
  return { ...g, comments: g.comments.map((c) => (c.id === id ? { ...c, size: { w: Math.round(size.w), h: Math.round(size.h) }, pos: pos ? { x: Math.round(pos.x), y: Math.round(pos.y) } : c.pos } : c)) };
}

export function setCommentText(g: Graph, id: string, text: string): Graph {
  return { ...g, comments: g.comments.map((c) => (c.id === id ? { ...c, text: text.slice(0, 2000) } : c)) };
}

// ---- Collapse to Function / Macro -----------------------------------------------------------

const cleanName = (s: string, fallback: string) => [...s].filter((c) => /[\p{L}\p{N} .,!?'()\-:;+&%#@=<>[\]_~$]/u.test(c)).join("").trim().slice(0, 30) || fallback;

/** Moves the selected nodes into a new Function (or Macro) of their own and puts a call to it
 *  where they were: wires coming in become its inputs, wires going out its outputs. A Function
 *  has one flow in and at most one flow out (several are fine for a Macro); with no flow at all
 *  it's a pure Function. */
export function collapse(g: Graph, ids: Iterable<string>, kind: "function" | "macro", tab = ""): { graph: Graph; id: string } | { error: string } {
  const pick = new Set(ids);
  const inside = g.nodes.filter((n) => pick.has(n.id) && (n.graph ?? "") === tab && !nodeDef(n.type)?.event && !["fn.return", "macro.exit"].includes(n.type));
  if (!inside.length) return { error: "Select the nodes to collapse first (not events or entry nodes)." };
  const inIds = new Set(inside.map((n) => n.id));
  const comingIn = g.edges.filter((e) => !inIds.has(e.from.node) && inIds.has(e.to.node));
  const goingOut = g.edges.filter((e) => inIds.has(e.from.node) && !inIds.has(e.to.node));
  const isFlow = (e: GEdge) => outType(g, e.from) === "exec";
  const key = (r: PinRef) => `${r.node}.${r.pin}`;
  // flow coming in: one entry point per inside pin it reaches; data coming in: one input per source
  const flowIns = [...new Map(comingIn.filter(isFlow).map((e) => [key(e.to), e.to])).values()];
  const dataIns = [...new Map(comingIn.filter((e) => !isFlow(e)).map((e) => [key(e.from), e.from])).values()];
  const flowOuts = [...new Map(goingOut.filter(isFlow).map((e) => [key(e.from), e.from])).values()];
  const dataOuts = [...new Map(goingOut.filter((e) => !isFlow(e)).map((e) => [key(e.from), e.from])).values()];
  const pure = kind === "function" && !flowIns.length && !flowOuts.length && inside.every((n) => nodeDef(n.type)?.pure);
  if (kind === "function" && !pure && (flowIns.length !== 1 || flowOuts.length > 1)) return { error: "A Function has one flow coming in and at most one going out: select a run of nodes in one flow (or collapse to a Macro)." };
  if (kind === "function" && inside.some((n) => ["timer.delay", "timer.retrigger", "timer.repeat", "timer.countdown", "msg.menu"].includes(n.type))) return { error: "A Function runs straight through: Delay, Repeat, Countdown and Menu can't go in one (collapse to a Macro)." };
  const memberName = (r: PinRef, dir: "in" | "out") => {
    const p = dir === "in" ? outPin(g, r) : outPin(g, r);
    return cleanName(p?.label || typeLabel(g, outType(g, r) ?? "any"), dir === "in" ? "In" : "Out");
  };
  const inputs: GMember[] = [];
  const outputs: GMember[] = [];
  const inMember = new Map<string, string>();
  const outMember = new Map<string, string>();
  if (kind === "macro") {
    flowIns.forEach((r, i) => {
      const id = `a${inputs.length + 1}`;
      inputs.push({ id, name: flowIns.length > 1 ? `In ${i + 1}` : "In", type: "exec" });
      inMember.set(key(r), id);
    });
  }
  dataIns.forEach((r) => {
    const id = `a${inputs.length + 1}`;
    inputs.push({ id, name: memberName(r, "in"), type: outType(g, r) ?? "number" });
    inMember.set(key(r), id);
  });
  if (kind === "macro") {
    flowOuts.forEach((r, i) => {
      const id = `r${outputs.length + 1}`;
      outputs.push({ id, name: flowOuts.length > 1 ? `Out ${i + 1}` : "Out", type: "exec" });
      outMember.set(key(r), id);
    });
  }
  dataOuts.forEach((r) => {
    const id = `r${outputs.length + 1}`;
    outputs.push({ id, name: memberName(r, "out"), type: outType(g, r) ?? "number" });
    outMember.set(key(r), id);
  });
  if (inputs.some((m) => m.type === "any") || outputs.some((m) => m.type === "any")) return { error: "A wire there carries nothing yet (a reroute with nothing in it): wire it up first." };
  // the new part and its graph
  const made = kind === "function" ? addFunction(g, pure) : addMacro(g);
  let g2 = made.graph;
  const part = made.id;
  const entry = g2.nodes.find((n) => n.graph === part && (n.type === "fn.entry" || n.type === "macro.entry"))!;
  const exit = g2.nodes.find((n) => n.graph === part && (n.type === "fn.return" || n.type === "macro.exit"))!;
  // (the starter wire between entry and exit goes; members replace the macro's starter pins)
  g2 = { ...g2, edges: g2.edges.filter((e) => !(e.from.node === entry.id && e.to.node === exit.id)) };
  if (kind === "function") g2 = updateFunction(g2, part, { inputs, outputs });
  else g2 = updateMacro(g2, part, { inputs, outputs });
  const xs = inside.map((n) => n.pos.x);
  const ys = inside.map((n) => n.pos.y);
  const x0 = Math.min(...xs);
  const y0 = Math.min(...ys);
  const x1 = Math.max(...xs);
  g2 = {
    ...g2,
    nodes: g2.nodes.map((n) => {
      if (inIds.has(n.id)) return { ...n, graph: part, pos: { x: n.pos.x - x0 + 300, y: n.pos.y - y0 } };
      if (n.id === entry.id) return { ...n, pos: { x: 0, y: 0 } };
      if (n.id === exit.id) return { ...n, pos: { x: x1 - x0 + 600, y: 0 } };
      return n;
    }),
  };
  // the call where the nodes were
  const callType = kind === "macro" ? "macro.use" : pure ? "fn.call_pure" : "fn.call";
  const call = addNode(g2, callType, { x: x0, y: y0 }, tab || undefined, kind === "macro" ? { macro: part } : { func: part });
  g2 = call.graph;
  const edges: GEdge[] = g2.edges.filter((e) => !comingIn.includes(e) && !goingOut.includes(e));
  const taken = new Set<string>();
  const wire = (from: PinRef, to: PinRef) => {
    const id = freshId(g2, "w", taken);
    taken.add(id);
    edges.push({ id, from, to });
  };
  for (const e of comingIn) {
    if (isFlow(e)) {
      const m = kind === "macro" ? inMember.get(key(e.to))! : "exec";
      wire(e.from, { node: call.id, pin: m });
    } else wire(e.from, { node: call.id, pin: inMember.get(key(e.from))! });
  }
  for (const r of flowIns) wire({ node: entry.id, pin: kind === "macro" ? inMember.get(key(r))! : "then" }, r);
  for (const r of dataIns) for (const e of comingIn.filter((x) => key(x.from) === key(r))) wire({ node: entry.id, pin: inMember.get(key(r))! }, e.to);
  for (const e of goingOut) {
    if (isFlow(e)) wire({ node: call.id, pin: kind === "macro" ? outMember.get(key(e.from))! : "then" }, e.to);
    else wire({ node: call.id, pin: outMember.get(key(e.from))! }, e.to);
  }
  for (const r of flowOuts) wire(r, { node: exit.id, pin: kind === "macro" ? outMember.get(key(r))! : "exec" });
  for (const r of dataOuts) wire(r, { node: exit.id, pin: outMember.get(key(r))! });
  // a pure Function goes straight from entry to return
  if (pure) wire({ node: entry.id, pin: "then" }, { node: exit.id, pin: "exec" });
  return { graph: { ...g2, edges }, id: part };
}

// ---- copy and paste: a small graph as JSON text --------------------------------------------

const CLIP = "loner-strike/graph-clip";

/** The chosen nodes and comments, with the wires between them, as text for the clipboard. */
export function copy(g: Graph, ids: Iterable<string>): string | null {
  const keep = new Set(ids);
  const nodes = g.nodes.filter((n) => keep.has(n.id));
  const comments = g.comments.filter((c) => keep.has(c.id));
  if (!nodes.length && !comments.length) return null;
  const edges = g.edges.filter((e) => keep.has(e.from.node) && keep.has(e.to.node));
  // the variables the nodes use go along, so they work when pasted into another graph
  const used = new Set(nodes.map((n) => n.params.var).filter((x): x is string => typeof x === "string"));
  const variables = g.variables.filter((x) => used.has(x.id));
  return JSON.stringify({ kind: CLIP, nodes, edges, comments, variables });
}

/** Pastes copied text (ours, or a whole graph file) with fresh ids, its top-left at `at`.
 *  Returns null for text that isn't a copied graph. */
export function paste(g: Graph, text: string, at: { x: number; y: number }, home?: string): { graph: Graph; ids: string[] } | null {
  let v: unknown;
  try {
    v = JSON.parse(text);
  } catch {
    return null;
  }
  const part = readPart(v);
  return part ? insertPart(g, part, at, home) : null;
}

export type Part = { nodes: GNode[]; edges: GEdge[]; comments: GComment[]; variables: GVariable[] };

/** A piece of graph from the clipboard, a snippet file or a whole graph file, checked like a
 *  graph file (the same shapes and text rules); null if it isn't one. */
export function readPart(v: unknown): Part | null {
  if (!v || typeof v !== "object") return null;
  const o = v as { kind?: string; schema?: string };
  if (o.kind === CLIP || o.schema === SNIPPET) {
    const { kind: _k, schema: _s, version: _v, name: _n, description: _d, ...rest } = v as Record<string, unknown>;
    void _k, _s, _v, _n, _d;
    const r = parseGraph({ schema: "loner-strike/graph", version: 2, mode: { id: "clip", name: "" }, ...rest });
    return "error" in r ? null : r.graph;
  }
  const r = parseGraph(v);
  return "error" in r ? null : r.graph;
}

/** Adds a piece with fresh ids, its top-left at `at`. Its variables join the graph's: the same
 *  one when the name, type and scope match, a new one otherwise (renamed when the name is
 *  taken). Settings naming a variable or another pasted node (a Function, a timer) follow. */
export function insertPart(g: Graph, part: Part, at: { x: number; y: number }, home?: string): { graph: Graph; ids: string[] } | null {
  // (a graph's own entry and exit nodes don't copy: each Function and Macro has one)
  const nodes = part.nodes.filter((n) => nodeDef(n.type) && !["fn.entry", "macro.entry", "macro.exit"].includes(n.type));
  if (!nodes.length && !part.comments.length) return null;
  const xs = [...nodes.map((n) => n.pos.x), ...part.comments.map((c) => c.pos.x)];
  const ys = [...nodes.map((n) => n.pos.y), ...part.comments.map((c) => c.pos.y)];
  const dx = at.x - Math.min(...xs);
  const dy = at.y - Math.min(...ys);
  const taken = new Set<string>();
  const remap = new Map<string, string>();
  const fresh = (old: string, prefix: string) => {
    const id = freshId(g, prefix, taken);
    taken.add(id);
    remap.set(old, id);
    return id;
  };
  let variables = g.variables;
  const varMap = new Map<string, string>();
  for (const x of part.variables) {
    const same = variables.find((y) => y.name === x.name && y.type === x.type && y.scope === x.scope);
    if (same) {
      varMap.set(x.id, same.id);
      continue;
    }
    let k = 1;
    while (variables.some((y) => y.id === `v${k}`)) k++;
    let name = x.name;
    for (let n = 2; variables.some((y) => y.name === name); n++) name = `${x.name.slice(0, 26)} ${n}`;
    variables = [...variables, { ...x, id: `v${k}`, name }];
    varMap.set(x.id, `v${k}`);
  }
  const pastedIds = new Set(nodes.map((n) => n.id));
  const newNodes = nodes.map((n) => {
    const { graph: _home, ...rest } = n;
    void _home;
    return { ...rest, id: fresh(n.id, "n"), pos: { x: Math.round(n.pos.x + dx), y: Math.round(n.pos.y + dy) }, breakpoint: undefined, ...(home ? { graph: home } : {}) } as GNode;
  });
  for (const n of newNodes) {
    const p = { ...n.params };
    if (typeof p.var === "string" && varMap.has(p.var)) p.var = varMap.get(p.var)!;
    for (const k of ["func", "timer"]) if (typeof p[k] === "string" && pastedIds.has(p[k] as string)) p[k] = remap.get(p[k] as string)!;
    n.params = p;
  }
  const newComments = part.comments.map((c) => {
    const { graph: _home, ...rest } = c;
    void _home;
    return { ...rest, id: fresh(c.id, "c"), pos: { x: Math.round(c.pos.x + dx), y: Math.round(c.pos.y + dy) }, ...(home ? { graph: home } : {}) } as GComment;
  });
  const newEdges = part.edges
    .filter((e) => remap.has(e.from.node) && remap.has(e.to.node))
    .map((e) => {
      const id = freshId(g, "w", taken);
      taken.add(id);
      return { id, from: { node: remap.get(e.from.node)!, pin: e.from.pin }, to: { node: remap.get(e.to.node)!, pin: e.to.pin } };
    });
  const ids = [...newNodes.map((n) => n.id), ...newComments.map((c) => c.id)];
  return { graph: { ...g, variables, nodes: [...g.nodes, ...newNodes], edges: [...g.edges, ...newEdges], comments: [...g.comments, ...newComments] }, ids };
}

// ---- snippets: pieces of graph to drop in --------------------------------------------------

export const SNIPPET = "loner-strike/graph-snippet";

/** A snippet file (shared like any community package: graph data only). */
export type Snippet = { id: string; name: string; description: string; part: Part; mine?: boolean };

export function readSnippet(id: string, v: unknown, mine = false): Snippet | null {
  if (!v || typeof v !== "object" || (v as { schema?: string }).schema !== SNIPPET) return null;
  const part = readPart(v);
  if (!part) return null;
  const o = v as { name?: unknown; description?: unknown };
  const name = typeof o.name === "string" ? o.name : id;
  return { id, name, description: typeof o.description === "string" ? o.description : "", part, mine };
}

/** The chosen nodes and comments as a snippet file. */
export function makeSnippet(g: Graph, ids: Iterable<string>, name: string, description = ""): object | null {
  const text = copy(g, ids);
  if (!text) return null;
  const { kind: _k, ...rest } = JSON.parse(text);
  void _k;
  return { schema: SNIPPET, version: 1, name, description, ...rest };
}

/** Drops a snippet in at `at`, inside a comment box named after it (unless it brings its own). */
export function insertSnippet(g: Graph, s: Snippet, at: { x: number; y: number }, sizes?: (id: string) => { w: number; h: number }, home?: string): { graph: Graph; ids: string[] } | null {
  const r = insertPart(g, s.part, { x: at.x + 30, y: at.y + 50 }, home);
  if (!r) return null;
  if (s.part.comments.length) return r;
  const placed = r.graph.nodes.filter((n) => r.ids.includes(n.id));
  if (!placed.length) return r;
  const size = (id: string) => sizes?.(id) ?? { w: 230, h: 110 };
  const x1 = Math.max(...placed.map((n) => n.pos.x + size(n.id).w));
  const y1 = Math.max(...placed.map((n) => n.pos.y + size(n.id).h));
  const c = addComment(r.graph, { x: at.x, y: at.y, w: x1 - at.x + 30, h: y1 - at.y + 30 }, s.name.replace(/[^\p{L}\p{N} .,!?'()\-:;+&%#@=<>[\]_~$]/gu, "").slice(0, 200), home);
  return { graph: c.graph, ids: [...r.ids, c.id] };
}

// ---- undo and redo -------------------------------------------------------------------------

export type History = { past: Graph[]; future: Graph[] };
const LIMIT = 200;

export const emptyHistory = (): History => ({ past: [], future: [] });

/** Remember `before` (the graph an edit started from). */
export function record(h: History, before: Graph): History {
  const past = h.past.length >= LIMIT ? h.past.slice(1) : h.past.slice();
  past.push(before);
  return { past, future: [] };
}

export function undo(h: History, now: Graph): { history: History; graph: Graph } | null {
  if (!h.past.length) return null;
  const graph = h.past[h.past.length - 1];
  return { graph, history: { past: h.past.slice(0, -1), future: [now, ...h.future] } };
}

export function redo(h: History, now: Graph): { history: History; graph: Graph } | null {
  if (!h.future.length) return null;
  const [graph, ...future] = h.future;
  return { graph, history: { past: [...h.past, now], future } };
}
