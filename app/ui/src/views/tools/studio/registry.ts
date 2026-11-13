// The node registry (nodes.json), shared with the app's compiler (tools/studio/graph.rs reads
// the same file): every node's pins, settings, help and category.

import raw from "./nodes.json?raw";

export type PinType = "exec" | "player" | "team" | "any";
export type PinDef = { id: string; type: PinType; label: string; required?: boolean; param?: string };
export type ParamDef = {
  id: string;
  label: string;
  kind: "choice" | "number" | "text";
  default: string | number;
  choices?: [string, string][];
  min?: number;
  max?: number;
};
export type NodeDef = {
  type: string;
  title: string;
  category: string;
  help: string;
  event?: boolean;
  reroute?: boolean;
  inputs: PinDef[];
  outputs: PinDef[];
  params: ParamDef[];
};

const data = JSON.parse(raw) as { nodes: NodeDef[]; categories: string[]; pinTypes: Record<PinType, { label: string; color: string }> };

export const NODE_DEFS = data.nodes;
export const CATEGORIES = data.categories;
export const PIN_TYPES = data.pinTypes;

const BY_TYPE = new Map(NODE_DEFS.map((d) => [d.type, d]));
export const nodeDef = (type: string): NodeDef | undefined => BY_TYPE.get(type);

/** Can a wire carrying `from` go into a pin of type `to`? ("any" is a reroute's pin.) */
export const compatible = (from: PinType, to: PinType) => from === to || from === "any" || to === "any";

export function defaultParams(def: NodeDef): Record<string, string | number> {
  return Object.fromEntries(def.params.map((p) => [p.id, p.default]));
}

/** What the node search lists. `wire`: dragging a wire of that type from an output (`dir:
 *  "out"`, so the node needs a matching input) or from an input (`dir: "in"`). */
export function searchNodes(query: string, wire?: { type: PinType; dir: "in" | "out" }): NodeDef[] {
  const q = query.trim().toLowerCase();
  return NODE_DEFS.filter((d) => {
    if (wire) {
      const pins = wire.dir === "out" ? d.inputs : d.outputs;
      if (!pins.some((p) => compatible(wire.type, p.type))) return false;
    }
    if (!q) return true;
    return `${d.title} ${d.category} ${d.help}`.toLowerCase().includes(q);
  }).sort((a, b) => {
    // titles that start with the words first, then by category order
    const sa = a.title.toLowerCase().startsWith(q) ? 0 : 1;
    const sb = b.title.toLowerCase().startsWith(q) ? 0 : 1;
    return sa - sb || CATEGORIES.indexOf(a.category) - CATEGORIES.indexOf(b.category) || a.title.localeCompare(b.title);
  });
}

/** The first pin on `def` a wire of `type` can attach to (from an output: an input; from an
 *  input: an output). */
export function firstPin(def: NodeDef, type: PinType, dir: "in" | "out"): PinDef | undefined {
  const pins = dir === "out" ? def.inputs : def.outputs;
  return pins.find((p) => p.type === type) ?? pins.find((p) => compatible(type, p.type));
}
