// Casting: an Entity wire that can't go into a Player pin gets an As Player node put between
// (the refused wire's "Insert As Player"). An ordinary edit, so Undo takes it back.

import { addNode, freshId, inType, outType } from "./graphOps";
import { nodeDef } from "./registry";
import type { Graph } from "./schema";

type Ref = { node: string; pin: string };

/** Is this an Entity output dropped on a Player input (what As Player is for)? */
export const wantsAsPlayer = (g: Graph, from: Ref, to: Ref) => outType(g, from) === "entity" && inType(g, to) === "player";

/** As Player between them: the entity into it, its Player into the pin; in the flow before the
 *  node it feeds when that node is in one (what ran it now runs As Player, Success runs it). */
export function insertAsPlayer(g: Graph, from: Ref, to: Ref): Graph {
  const target = g.nodes.find((n) => n.id === to.node);
  if (!target) return g;
  const added = addNode(g, "cast.player", { x: target.pos.x - 240, y: target.pos.y + 40 }, target.graph);
  let out = added.graph;
  const cast = added.id;
  const used = new Set<string>();
  const wire = (a: Ref, b: Ref) => {
    const id = freshId(out, "w", used);
    used.add(id);
    out = { ...out, edges: [...out.edges, { id, from: a, to: b }] };
  };
  wire(from, { node: cast, pin: "in" });
  wire({ node: cast, pin: "out" }, to);
  const exec = nodeDef(target.type)?.inputs.find((p) => p.type === "exec")?.id;
  if (exec && out.edges.some((e) => e.to.node === target.id && e.to.pin === exec)) {
    out = { ...out, edges: out.edges.map((e) => (e.to.node === target.id && e.to.pin === exec ? { ...e, to: { node: cast, pin: "exec" } } : e)) };
    wire({ node: cast, pin: "ok" }, { node: target.id, pin: exec });
  }
  return out;
}
