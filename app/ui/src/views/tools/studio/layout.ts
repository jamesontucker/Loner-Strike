// Auto-layout: left-to-right flow with dagre (MIT). ELK is EPL-2.0 and stays out of the app.

import dagre from "@dagrejs/dagre";
import type { Graph } from "./schema";

/** Node sizes as drawn (roughly): the layout only needs them to keep nodes apart. */
export type Sizes = Map<string, { w: number; h: number }>;

export function autoLayout(g: Graph, sizes: Sizes): Map<string, { x: number; y: number }> {
  const d = new dagre.graphlib.Graph();
  d.setGraph({ rankdir: "LR", nodesep: 40, ranksep: 90, marginx: 20, marginy: 20 });
  d.setDefaultEdgeLabel(() => ({}));
  for (const n of g.nodes) {
    const s = sizes.get(n.id) ?? { w: 220, h: 100 };
    d.setNode(n.id, { width: s.w, height: s.h });
  }
  for (const e of g.edges) d.setEdge(e.from.node, e.to.node);
  dagre.layout(d);
  const out = new Map<string, { x: number; y: number }>();
  for (const n of g.nodes) {
    const p = d.node(n.id);
    const s = sizes.get(n.id) ?? { w: 220, h: 100 };
    if (p) out.set(n.id, { x: Math.round(p.x - s.w / 2), y: Math.round(p.y - s.h / 2) });
  }
  return out;
}
