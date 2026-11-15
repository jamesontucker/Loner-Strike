// Studio polish: Promote to Variable, align and distribute, select linked nodes, bookmarks and
// the quick-add keys (hold a key and click empty space). Pure, so they're tested without a page
// (scripts/studio-help.test.ts).

import { addLocal, addNode, addVariable, connect, inType, outType, startValue, uniqueName, type PinRef } from "./graphOps";
import { allVariables, pinsOf, typeOk } from "./pins";
import { nodeDef } from "./registry";
import { estimateSize } from "./layout";
import type { GVariable, Graph } from "./schema";

// ---- Promote to Variable --------------------------------------------------------------------

/** Can this pin become a variable (a value pin of a type a variable can hold)? */
export function promotable(g: Graph, ref: PinRef, dir: "in" | "out"): { type: string } | null {
  const t = dir === "in" ? inType(g, ref) : outType(g, ref);
  if (!t || t === "exec" || t === "any" || t.includes("$")) return null;
  return typeOk(g, t) ? { type: t } : null;
}

/** A variable from a pin, wired in: an input gets a Get (starting at the pin's own setting), an
 *  output feeds a Set (run right after the node when its Then is free). Inside a Function, a
 *  local of it. Returns the new graph and the variable's id, or an error to show. */
export function promoteToVariable(g: Graph, ref: PinRef, dir: "in" | "out"): { graph: Graph; id: string; message: string } | { error: string } {
  const n = g.nodes.find((x) => x.id === ref.node);
  const p = promotable(g, ref, dir);
  if (!n || !p) return { error: "That pin can't be a variable (flow pins and pins of any type can't)." };
  const pins = pinsOf(g, n);
  const pin = (dir === "in" ? pins.inputs : pins.outputs).find((x) => x.id === ref.pin);
  const fn = n.graph && (g.functions ?? []).some((f) => f.id === n.graph) ? n.graph : undefined;
  const name = uniqueName(
    allVariables(g).map((v) => v.name),
    (pin?.label || "Value").replace(/[^\p{L}\p{N} .,!?'()\-:;+&%#@=<>[\]_~$]/gu, "").slice(0, 26) || "Value",
  );
  // an input with a setting of the same name starts the variable at that value
  let start = startValue(p.type);
  const own = dir === "in" ? n.params[pin?.param ?? ""] : undefined;
  if (own !== undefined && ((p.type === "number" && typeof own === "number") || (p.type === "bool" && typeof own === "boolean") || (p.type === "text" && typeof own === "string"))) start = own;
  const v: Omit<GVariable, "id"> = { name, type: p.type, scope: "mode", ...(start !== undefined ? { default: start as GVariable["default"] } : {}) };
  const made = fn ? addLocal(g, fn, v) : addVariable(g, v);
  let g2 = made.graph;
  const home = n.graph;
  if (dir === "in") {
    const get = addNode(g2, "var.get", freeSpot(g2, { x: n.pos.x - 260, y: n.pos.y }, home), home, { var: made.id });
    const c = connect(get.graph, { node: get.id, pin: "out" }, ref);
    if ("error" in c) return { error: c.error };
    g2 = c.graph;
  } else {
    const set = addNode(g2, "var.set", freeSpot(g2, { x: n.pos.x + 280, y: n.pos.y }, home), home, { var: made.id });
    let c = connect(set.graph, ref, { node: set.id, pin: "value" });
    if ("error" in c) return { error: c.error };
    // run it right after the node, when the node's Then goes nowhere yet
    const then = pinsOf(c.graph, n).outputs.find((o) => o.id === "then" && o.type === "exec");
    if (then && !c.graph.edges.some((e) => e.from.node === n.id && e.from.pin === "then")) {
      const c2 = connect(c.graph, { node: n.id, pin: "then" }, { node: set.id, pin: "exec" });
      if (!("error" in c2)) c = c2;
    }
    g2 = c.graph;
  }
  return { graph: g2, id: made.id, message: `${fn ? "Local variable" : "Variable"} “${name}” made and wired in.` };
}

/** The nearest spot to `want` (looking down, then up) where a new node doesn't sit on another
 *  node of the same graph (UX review M3). */
export function freeSpot(g: Graph, want: { x: number; y: number }, home?: string, size = { w: 230, h: 110 }): { x: number; y: number } {
  const others = g.nodes.filter((o) => (o.graph ?? "") === (home ?? "")).map((o) => ({ ...o.pos, ...estimateSize(o.type, o.params) }));
  const clear = (p: { x: number; y: number }) => others.every((o) => p.x + size.w + 16 <= o.x || o.x + o.w + 16 <= p.x || p.y + size.h + 16 <= o.y || o.y + o.h + 16 <= p.y);
  for (let k = 0; k < 40; k++) {
    for (const dy of k ? [k * 40, -k * 40] : [0]) {
      const p = { x: want.x, y: want.y + dy };
      if (clear(p)) return p;
    }
  }
  return want;
}

// ---- align and distribute -------------------------------------------------------------------

export type Box = { x: number; y: number; w: number; h: number };
export type AlignHow = "left" | "right" | "top" | "bottom" | "center" | "middle";

/** New positions that line the boxes up: left/right edges, top/bottom edges, centers across
 *  (center) or down (middle). */
export function align(boxes: Map<string, Box>, how: AlignHow): Map<string, { x: number; y: number }> {
  const all = [...boxes.values()];
  const out = new Map<string, { x: number; y: number }>();
  if (all.length < 2) return out;
  const left = Math.min(...all.map((b) => b.x));
  const right = Math.max(...all.map((b) => b.x + b.w));
  const top = Math.min(...all.map((b) => b.y));
  const bottom = Math.max(...all.map((b) => b.y + b.h));
  const cx = all.reduce((s, b) => s + b.x + b.w / 2, 0) / all.length;
  const cy = all.reduce((s, b) => s + b.y + b.h / 2, 0) / all.length;
  for (const [id, b] of boxes) {
    const p = { x: b.x, y: b.y };
    if (how === "left") p.x = left;
    if (how === "right") p.x = right - b.w;
    if (how === "top") p.y = top;
    if (how === "bottom") p.y = bottom - b.h;
    if (how === "center") p.x = cx - b.w / 2;
    if (how === "middle") p.y = cy - b.h / 2;
    out.set(id, { x: Math.round(p.x), y: Math.round(p.y) });
  }
  return out;
}

/** Even gaps between the boxes, across or down, the first and last staying where they are. */
export function distribute(boxes: Map<string, Box>, across: boolean): Map<string, { x: number; y: number }> {
  const list = [...boxes.entries()].sort((a, b) => (across ? a[1].x - b[1].x : a[1].y - b[1].y));
  const out = new Map<string, { x: number; y: number }>();
  if (list.length < 3) return out;
  const size = (b: Box) => (across ? b.w : b.h);
  const start = across ? list[0][1].x : list[0][1].y;
  const last = list[list.length - 1][1];
  const end = across ? last.x + last.w : last.y + last.h;
  const gap = (end - start - list.reduce((s, [, b]) => s + size(b), 0)) / (list.length - 1);
  let at = start;
  for (const [id, b] of list) {
    out.set(id, across ? { x: Math.round(at), y: b.y } : { x: b.x, y: Math.round(at) });
    at += size(b) + gap;
  }
  return out;
}

// ---- select linked --------------------------------------------------------------------------

/** The nodes wired to these, following wires upstream (what feeds them), downstream, or both. */
export function linked(g: Graph, ids: string[], which: "up" | "down" | "all"): string[] {
  const seen = new Set(ids);
  const todo = [...ids];
  while (todo.length) {
    const id = todo.pop()!;
    for (const e of g.edges) {
      const next = (which !== "up" && e.from.node === id && e.to.node) || (which !== "down" && e.to.node === id && e.from.node);
      if (next && !seen.has(next)) {
        seen.add(next);
        todo.push(next);
      }
    }
  }
  return [...seen];
}

// ---- bookmarks (kept per mode in this browser: a view, not part of the mode) -----------------

export type Bookmark = { tab: string; x: number; y: number; zoom: number; name: string };
const BM = "ls.studio.bookmarks.";

export function loadBookmarks(uid: string): (Bookmark | null)[] {
  try {
    const v = JSON.parse(localStorage.getItem(BM + uid) ?? "[]");
    return Array.from({ length: 9 }, (_, i) => (v[i] && typeof v[i].x === "number" ? (v[i] as Bookmark) : null));
  } catch {
    return Array(9).fill(null);
  }
}

export function saveBookmarks(uid: string, list: (Bookmark | null)[]) {
  try {
    localStorage.setItem(BM + uid, JSON.stringify(list));
  } catch {
    /* private window: they last for this visit */
  }
}

// ---- quick-add keys: hold one and click empty space --------------------------------------------

export const QUICK_ADD: Record<string, { type: string; params?: Record<string, string | number | boolean> }> = {
  b: { type: "flow.branch" },
  s: { type: "flow.sequence" },
  d: { type: "timer.delay" },
  f: { type: "flow.for_each_player" },
  l: { type: "flow.for_loop" },
  g: { type: "flow.gate" },
  o: { type: "flow.do_once" },
  n: { type: "flow.do_n" },
  r: { type: "timer.repeat" },
  p: { type: "debug.print" },
  w: { type: "flow.while" },
};

/** "B + click: Branch" lines for help and the tour. */
export const quickAddList = () =>
  Object.entries(QUICK_ADD)
    .filter(([, q]) => nodeDef(q.type))
    .map(([k, q]) => ({ key: k.toUpperCase(), title: nodeDef(q.type)!.title }));

// ---- a readable first view (UX review H1) ----------------------------------------------------

export type Rect = { x: number; y: number; w: number; h: number };
/** Below this the text on nodes can't be read. */
export const READABLE = 0.6;

/** What to show first on a graph: all of it when that's readable; else its first part (the
 *  comment box nearest the top left, or the first node) at a readable zoom. */
export function startView(nodes: Rect[], comments: Rect[], canvas: { w: number; h: number }): { fit: true } | { fit: false; rect: Rect } {
  const all = [...nodes, ...comments];
  if (!all.length) return { fit: true };
  const x0 = Math.min(...all.map((r) => r.x)),
    y0 = Math.min(...all.map((r) => r.y));
  const x1 = Math.max(...all.map((r) => r.x + r.w)),
    y1 = Math.max(...all.map((r) => r.y + r.h));
  const zoom = Math.min(canvas.w / ((x1 - x0) * 1.2), canvas.h / ((y1 - y0) * 1.2));
  if (zoom >= READABLE) return { fit: true };
  const first = [...(comments.length ? comments : nodes)].sort((a, b) => a.y - b.y || a.x - b.x)[0];
  return { fit: false, rect: first };
}
