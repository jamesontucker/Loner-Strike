// The node registry (nodes.json), shared with the app's compiler (tools/studio/graph.rs reads
// the same file): every node's pins, settings, help and category.

import raw from "./nodes.json?raw";

export type PinType = "exec" | "player" | "entity" | "team" | "number" | "bool" | "text" | "any" | "var";
export type PinDef = { id: string; type: PinType; label: string; required?: boolean; optional?: boolean; param?: string };
export type ParamDef = {
  id: string;
  label: string;
  /** name: an entity's name; keys: component keys; var: one of the graph's variables;
   *  varvalue: a value of that variable's type */
  kind: "choice" | "number" | "text" | "bool" | "name" | "keys" | "var" | "varvalue";
  default: string | number | boolean;
  choices?: [string, string][];
  min?: number;
  max?: number;
  varType?: "number" | "bool" | "text";
};
export type NodeDef = {
  type: string;
  title: string;
  category: string;
  help: string;
  event?: boolean;
  reroute?: boolean;
  /** no flow pins: worked out where its value is used */
  pure?: boolean;
  /** "components" (gm_components) or "creatures" (the bot brain's creatures) */
  needs?: string;
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

/** Can a wire carrying `from` go into a pin of type `to`? ("any" is a reroute's pin; a player
 *  is an entity.) Same rules as graph.rs `compatible`. */
export const compatible = (from: PinType, to: PinType) => from === to || from === "any" || to === "any" || (from === "player" && to === "entity");

/** Entity names and creature types; component keys (the app checks the same). */
export const NAME_CHARS = /^[A-Za-z0-9_.-]{0,32}$/;
export const KEYS_CHARS = /^[A-Za-z0-9 _.=:,-]*$/;

export function defaultParams(def: NodeDef): Record<string, string | number | boolean> {
  return Object.fromEntries(def.params.map((p) => [p.id, p.default]));
}

/** What the node search lists. `wire`: dragging a wire of that type from an output (`dir:
 *  "out"`, so the node needs a matching input) or from an input (`dir: "in"`). */
export function searchNodes(query: string, wire?: { type: PinType; dir: "in" | "out" }): NodeDef[] {
  const q = query.trim().toLowerCase();
  return NODE_DEFS.filter((d) => {
    if (wire) {
      const pins = wire.dir === "out" ? d.inputs : d.outputs;
      if (!pins.some((p) => fits(wire, p.type))) return false;
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
  return pins.find((p) => p.type === type) ?? pins.find((p) => fits({ type, dir }, p.type));
}

/** Can a pin of type `pin` take a wire being dragged (a variable's pin can take anything until
 *  its variable is picked). */
function fits(wire: { type: PinType; dir: "in" | "out" }, pin: PinType): boolean {
  if (pin === "var") return wire.type !== "exec";
  return wire.dir === "out" ? compatible(wire.type, pin) : compatible(pin, wire.type);
}
