// The quick check's note about something bots could use but aren't told about (graph.rs:
// bot_awareness): its one-click fix and "Don't warn for this one". Both are ordinary edits, so
// Undo takes them back.

import { addNode, freshId } from "./graphOps";
import type { Graph } from "./schema";

/** how the app's note starts (graph.rs: BOTS_NOTE) */
export const BOTS_NOTE = "Bots won't know about";

export const isBotsNote = (message: string) => message.startsWith(BOTS_NOTE);

/** What the fix does for this node, or null when there's no single right one. */
export function botFixLabel(g: Graph, nodeId: string): string | null {
  const n = g.nodes.find((x) => x.id === nodeId);
  if (!n) return null;
  if (n.type === "npc.spawn_use") return "Let bots use it";
  if (n.type === "comp.create" && n.params.kind === "ls_interactable") return "Let bots use it";
  if (n.type === "npc.spawn" || (n.type === "comp.create" && n.params.kind === "ls_npc")) return "Offer it to bots";
  return null;
}

/** The fix: an interactable's own Bots may use (or bots=1 in its keys); for an NPC, a Bots: Offer
 *  ("use": they press E on it) right after it in the flow, its At wired from the NPC. */
export function botFix(g: Graph, nodeId: string): Graph {
  const n = g.nodes.find((x) => x.id === nodeId);
  if (!n) return g;
  const set = (params: Graph["nodes"][number]["params"]) => ({ ...g, nodes: g.nodes.map((x) => (x.id === nodeId ? { ...x, params } : x)) });
  if (n.type === "npc.spawn_use") return set({ ...n.params, bots: true });
  if (n.type === "comp.create" && n.params.kind === "ls_interactable") {
    const keys = String(n.params.keys ?? "")
      .split(/\s+/)
      .filter((kv) => kv && !kv.startsWith("bots="));
    return set({ ...n.params, keys: [...keys, "bots=1"].join(" ") });
  }
  if (n.type !== "npc.spawn" && !(n.type === "comp.create" && n.params.kind === "ls_npc")) return g;
  const added = addNode(g, "bots.offer", { x: n.pos.x + 40, y: n.pos.y + 180 }, n.graph, { kind: "use", cost: 0 });
  let out = added.graph;
  const offer = added.id;
  const used = new Set<string>();
  const wire = (from: { node: string; pin: string }, to: { node: string; pin: string }) => {
    const id = freshId(out, "w", used);
    used.add(id);
    out = { ...out, edges: [...out.edges, { id, from, to }] };
  };
  // what came after the NPC now comes after the offer
  out = { ...out, edges: out.edges.map((e) => (e.from.node === nodeId && e.from.pin === "then" ? { ...e, from: { node: offer, pin: "then" } } : e)) };
  wire({ node: nodeId, pin: "then" }, { node: offer, pin: "exec" });
  wire({ node: nodeId, pin: "out" }, { node: offer, pin: "at" });
  return out;
}

/** "Don't warn for this one": the node keeps a quiet mark (saved with the mode). */
export const quietNode = (g: Graph, nodeId: string): Graph => ({ ...g, nodes: g.nodes.map((x) => (x.id === nodeId ? { ...x, quiet: true } : x)) });
