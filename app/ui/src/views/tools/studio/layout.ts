// Auto-layout ("Tidy up"): left-to-right flow with dagre (MIT; ELK is EPL-2.0 and stays out of
// the app). Comment boxes are dagre clusters: the nodes inside a box stay together and the box
// is fitted round them afterwards, so boxes never end up on top of other nodes.

import dagre from "@dagrejs/dagre";
import { nodeDef } from "./registry";
import type { Graph } from "./schema";

/** Node sizes as drawn: the layout only needs them to keep nodes apart. */
export type Sizes = Map<string, { w: number; h: number }>;
export type Layout = { nodes: Map<string, { x: number; y: number }>; comments: Map<string, { x: number; y: number; w: number; h: number }> };

/** A node's size without a browser (templates, tests): close to how it's drawn. */
export function estimateSize(type: string, params: Record<string, unknown> = {}): { w: number; h: number } {
  const d = nodeDef(type);
  if (!d) return { w: 220, h: 100 };
  if (d.reroute) return { w: 16, h: 16 };
  const rows = Math.max(d.inputs.length, d.outputs.length, 1);
  return { w: 230, h: 34 + rows * 22 + 8 + (d.params.length || Object.keys(params).length ? 26 : 0) };
}

/** The comments a node sits in (its centre inside the box), smallest box first. */
function boxOf(g: Graph, id: string, sizes: Sizes): string | null {
  const n = g.nodes.find((x) => x.id === id);
  if (!n) return null;
  const s = sizes.get(id) ?? { w: 220, h: 100 };
  const cx = n.pos.x + s.w / 2;
  const cy = n.pos.y + s.h / 2;
  const inside = g.comments.filter((c) => cx >= c.pos.x && cy >= c.pos.y && cx <= c.pos.x + c.size.w && cy <= c.pos.y + c.size.h);
  inside.sort((a, b) => a.size.w * a.size.h - b.size.w * b.size.h);
  return inside[0]?.id ?? null;
}

const PAD = { side: 24, top: 46, bottom: 22 };

export function autoLayout(g: Graph, sizes: Sizes): Layout {
  const d = new dagre.graphlib.Graph({ compound: true });
  d.setGraph({ rankdir: "LR", nodesep: 40, ranksep: 90, marginx: 20, marginy: 20 });
  d.setDefaultEdgeLabel(() => ({}));
  const members = new Map<string, string[]>();
  for (const n of g.nodes) {
    const s = sizes.get(n.id) ?? { w: 220, h: 100 };
    d.setNode(n.id, { width: s.w, height: s.h });
  }
  for (const c of g.comments) {
    const inside = g.nodes.filter((n) => boxOf(g, n.id, sizes) === c.id).map((n) => n.id);
    if (!inside.length) continue;
    members.set(c.id, inside);
    // (the cluster's label space makes room for the comment's title bar)
    d.setNode(c.id, { paddingTop: PAD.top, paddingLeft: PAD.side, paddingRight: PAD.side, paddingBottom: PAD.bottom });
    for (const id of inside) d.setParent(id, c.id);
  }
  for (const e of g.edges) d.setEdge(e.from.node, e.to.node);
  dagre.layout(d);
  const nodes = new Map<string, { x: number; y: number }>();
  for (const n of g.nodes) {
    const p = d.node(n.id);
    const s = sizes.get(n.id) ?? { w: 220, h: 100 };
    if (p) nodes.set(n.id, { x: Math.round(p.x - s.w / 2), y: Math.round(p.y - s.h / 2) });
  }
  // each box fitted round its nodes
  const comments = new Map<string, { x: number; y: number; w: number; h: number }>();
  for (const [cid, ids] of members) {
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const id of ids) {
      const p = nodes.get(id);
      const s = sizes.get(id) ?? { w: 220, h: 100 };
      if (!p) continue;
      x0 = Math.min(x0, p.x);
      y0 = Math.min(y0, p.y);
      x1 = Math.max(x1, p.x + s.w);
      y1 = Math.max(y1, p.y + s.h);
    }
    if (x0 === Infinity) continue;
    comments.set(cid, { x: Math.round(x0 - PAD.side), y: Math.round(y0 - PAD.top), w: Math.round(x1 - x0 + PAD.side * 2), h: Math.round(y1 - y0 + PAD.top + PAD.bottom) });
  }
  return { nodes, comments };
}

/** The graph laid out (positions and boxes changed, nothing else). */
export function applyLayout(g: Graph, l: Layout): Graph {
  return {
    ...g,
    nodes: g.nodes.map((n) => (l.nodes.has(n.id) ? { ...n, pos: l.nodes.get(n.id)! } : n)),
    comments: g.comments.map((c) => {
      const b = l.comments.get(c.id);
      return b ? { ...c, pos: { x: b.x, y: b.y }, size: { w: b.w, h: b.h } } : c;
    }),
  };
}

/** Laid out with estimated sizes (templates, and graphs made without a browser). */
export function tidyEstimated(g: Graph): Graph {
  const sizes: Sizes = new Map(g.nodes.map((n) => [n.id, estimateSize(n.type, n.params)]));
  return applyLayout(g, autoLayout(g, sizes));
}
