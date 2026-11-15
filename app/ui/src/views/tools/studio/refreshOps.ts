// "Refresh nodes" (View menu): the graph against the node definitions this app has. A node's pins
// always come from the current definitions, so refreshing changes nothing on its own; it lists
// what an update left behind (StudioCompat.md): orphaned wires and settings, which stay until
// they're re-wired, cleared or deleted, and older nodes that still work but have a replacement.

import { nodeDef } from "./registry";
import { orphanValues, pinsOf } from "./pins";
import type { Graph } from "./schema";

export type RefreshItem = { node: string; what: "wire" | "setting" | "deprecated"; text: string };

export function refreshReport(g: Graph): RefreshItem[] {
  const out: RefreshItem[] = [];
  for (const n of g.nodes) {
    const def = nodeDef(n.type);
    if (!def) {
      out.push({ node: n.id, what: "deprecated", text: `${n.type}: this app doesn't have that node` });
      continue;
    }
    const p = pinsOf(g, n);
    for (const pin of [...p.inputs, ...p.outputs].filter((x) => x.orphan)) out.push({ node: n.id, what: "wire", text: `${def.title}: a wire to "${pin.id}", which was removed` });
    for (const k of orphanValues(n)) out.push({ node: n.id, what: "setting", text: `${def.title}: a value for "${k}", which was removed` });
    const dep = (def as { deprecated?: string }).deprecated;
    if (dep) out.push({ node: n.id, what: "deprecated", text: `${def.title} is from an older version (${dep} does it now)` });
  }
  return out;
}

/** The report as one line for a toast. */
export function refreshSummary(items: RefreshItem[]): string {
  if (!items.length) return "Every node is up to date: nothing was removed by an update.";
  const count = (w: RefreshItem["what"]) => items.filter((i) => i.what === w).length;
  const parts = [
    count("wire") && `${count("wire")} orphaned wire${count("wire") > 1 ? "s" : ""}`,
    count("setting") && `${count("setting")} orphaned setting${count("setting") > 1 ? "s" : ""}`,
    count("deprecated") && `${count("deprecated")} older node${count("deprecated") > 1 ? "s" : ""}`,
  ].filter(Boolean);
  return `Nodes refreshed: ${parts.join(", ")}. They're marked red on the canvas and listed in Problems: ${items.slice(0, 3).map((i) => i.text).join("; ")}${items.length > 3 ? "; ..." : ""}`;
}
