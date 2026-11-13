// Edits on the graph JSON, kept pure (graph in, graph out) so undo/redo is a stack of graphs
// and the rules are testable without React (studio.test.ts). The app checks the same rules
// again before compiling (tools/studio/graph.rs).

import { parseGraph, type GComment, type GEdge, type GNode, type Graph } from "./schema";
import { compatible, defaultParams, nodeDef, type PinDef, type PinType } from "./registry";

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
  return n ? nodeDef(n.type)?.outputs.find((p) => p.id === ref.pin) : undefined;
}

export function inPin(g: Graph, ref: PinRef): PinDef | undefined {
  const n = findNode(g, ref.node);
  return n ? nodeDef(n.type)?.inputs.find((p) => p.id === ref.pin) : undefined;
}

/** The type an output really carries: a reroute carries whatever comes into it ("any" when
 *  nothing does yet). */
export function outType(g: Graph, ref: PinRef, seen = new Set<string>()): PinType | undefined {
  const n = findNode(g, ref.node);
  const def = n && nodeDef(n.type);
  if (!def) return undefined;
  if (!def.reroute) return def.outputs.find((p) => p.id === ref.pin)?.type;
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
  if (!def.reroute) return def.inputs.find((p) => p.id === ref.pin)?.type;
  if (seen.has(n.id)) return "any";
  seen.add(n.id);
  const e = g.edges.find((x) => x.from.node === n.id && x.from.pin === "out");
  return e ? inType(g, e.to, seen) : "any";
}

/** Would the flow loop if `from` were wired to `to`? (Only flow wires can loop.) */
function makesLoop(g: Graph, from: string, to: string): boolean {
  const stack = [to];
  const seen = new Set<string>();
  while (stack.length) {
    const cur = stack.pop()!;
    if (cur === from) return true;
    if (seen.has(cur)) continue;
    seen.add(cur);
    for (const e of g.edges) if (e.from.node === cur && outType(g, e.from) === "exec") stack.push(e.to.node);
  }
  return false;
}

export type Check = { ok: true; replaces: string[] } | { ok: false; error: string };

const label = (t: PinType) => ({ exec: "Flow", player: "Player", team: "Team", any: "Anything" })[t];

/** Can this wire be drawn? A new wire into a taken data input (or from a taken flow output)
 *  replaces the old one, as in Unreal's Blueprints: `replaces` lists those. */
export function canConnect(g: Graph, from: PinRef, to: PinRef): Check {
  if (from.node === to.node) return { ok: false, error: "A wire can't go back into the same node." };
  const op = outPin(g, from);
  const ip = inPin(g, to);
  if (!op || !ip) return { ok: false, error: "Wires go from an output (right) to an input (left)." };
  const ft = outType(g, from)!;
  const tt = inType(g, to)!;
  if (!compatible(ft, tt)) return { ok: false, error: `A ${label(ft)} wire can't go into a ${label(tt)} pin.` };
  if (g.edges.some((e) => e.from.node === from.node && e.from.pin === from.pin && e.to.node === to.node && e.to.pin === to.pin)) return { ok: false, error: "Those are already wired." };
  const flow = ft === "exec" || tt === "exec";
  if (flow && makesLoop(g, from.node, to.node)) return { ok: false, error: "That would make the flow loop. Loops aren't possible yet." };
  const toReroute = nodeDef(findNode(g, to.node)!.type)?.reroute;
  const replaces = g.edges
    .filter((e) => (flow && e.from.node === from.node && e.from.pin === from.pin) || ((!flow || toReroute) && e.to.node === to.node && e.to.pin === to.pin))
    .map((e) => e.id);
  return { ok: true, replaces };
}

export function connect(g: Graph, from: PinRef, to: PinRef): { graph: Graph } | { error: string } {
  const c = canConnect(g, from, to);
  if (!c.ok) return { error: c.error };
  const edge: GEdge = { id: freshId(g, "w"), from: { ...from }, to: { ...to } };
  return { graph: { ...g, edges: [...g.edges.filter((e) => !c.replaces.includes(e.id)), edge] } };
}

export function addNode(g: Graph, type: string, pos: { x: number; y: number }): { graph: Graph; id: string } {
  const def = nodeDef(type);
  if (!def) throw new Error(`unknown node ${type}`);
  const id = freshId(g, "n");
  const node: GNode = { id, type, pos: { x: Math.round(pos.x), y: Math.round(pos.y) }, params: defaultParams(def) };
  return { graph: { ...g, nodes: [...g.nodes, node] }, id };
}

export function addComment(g: Graph, around: { x: number; y: number; w: number; h: number }, text = "Comment"): { graph: Graph; id: string } {
  const id = freshId(g, "c");
  const c: GComment = { id, text, pos: { x: Math.round(around.x), y: Math.round(around.y) }, size: { w: Math.max(160, Math.round(around.w)), h: Math.max(80, Math.round(around.h)) } };
  return { graph: { ...g, comments: [...g.comments, c] }, id };
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
  const { graph, id } = addNode(g, "flow.reroute", pos);
  const a: GEdge = { id: freshId(graph, "w"), from: e.from, to: { node: id, pin: "in" } };
  const b: GEdge = { id: freshId(graph, "w", new Set([a.id])), from: { node: id, pin: "out" }, to: e.to };
  return { ...graph, edges: [...graph.edges.filter((x) => x.id !== edgeId), a, b] };
}

export function setParam(g: Graph, node: string, param: string, value: string | number): Graph {
  return { ...g, nodes: g.nodes.map((n) => (n.id === node ? { ...n, params: { ...n.params, [param]: value } } : n)) };
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

// ---- copy and paste: a small graph as JSON text --------------------------------------------

const CLIP = "loner-strike/graph-clip";

/** The chosen nodes and comments, with the wires between them, as text for the clipboard. */
export function copy(g: Graph, ids: Iterable<string>): string | null {
  const keep = new Set(ids);
  const nodes = g.nodes.filter((n) => keep.has(n.id));
  const comments = g.comments.filter((c) => keep.has(c.id));
  if (!nodes.length && !comments.length) return null;
  const edges = g.edges.filter((e) => keep.has(e.from.node) && keep.has(e.to.node));
  return JSON.stringify({ kind: CLIP, nodes, edges, comments });
}

/** Pastes copied text (ours, or a whole graph file) with fresh ids, its top-left at `at`.
 *  Returns null for text that isn't a copied graph. */
export function paste(g: Graph, text: string, at: { x: number; y: number }): { graph: Graph; ids: string[] } | null {
  let v: unknown;
  try {
    v = JSON.parse(text);
  } catch {
    return null;
  }
  let part: { nodes: GNode[]; edges: GEdge[]; comments: GComment[] };
  if (v && typeof v === "object" && (v as { kind?: string }).kind === CLIP) {
    // checked like a graph file: the same shapes
    const r = parseGraph({ schema: "loner-strike/graph", version: 1, mode: { id: "clip", name: "" }, ...(v as object) });
    if ("error" in r) return null;
    part = r.graph;
  } else {
    const r = parseGraph(v);
    if ("error" in r) return null;
    part = r.graph;
  }
  const nodes = part.nodes.filter((n) => nodeDef(n.type));
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
  const newNodes = nodes.map((n) => ({ ...n, id: fresh(n.id, "n"), pos: { x: Math.round(n.pos.x + dx), y: Math.round(n.pos.y + dy) }, breakpoint: undefined }));
  const newComments = part.comments.map((c) => ({ ...c, id: fresh(c.id, "c"), pos: { x: Math.round(c.pos.x + dx), y: Math.round(c.pos.y + dy) } }));
  const newEdges = part.edges
    .filter((e) => remap.has(e.from.node) && remap.has(e.to.node))
    .map((e) => {
      const id = freshId(g, "w", taken);
      taken.add(id);
      return { id, from: { node: remap.get(e.from.node)!, pin: e.from.pin }, to: { node: remap.get(e.to.node)!, pin: e.to.pin } };
    });
  const ids = [...newNodes.map((n) => n.id), ...newComments.map((c) => c.id)];
  return { graph: { ...g, nodes: [...g.nodes, ...newNodes], edges: [...g.edges, ...newEdges], comments: [...g.comments, ...newComments] }, ids };
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
