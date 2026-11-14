// The node registry (nodes.json), shared with the app's compiler (tools/studio/graph.rs reads
// the same file): every node's pins, settings, help and category.

import raw from "./nodes.json?raw";

/** exec, player, entity, team, number, bool, text, vector, timer, enum:<id>, struct:<id>,
 *  array<T>, map<K,T>, any, var (the node's variable's type) and, in the registry, $T $K $E $S $F
 *  (worked out from the node's settings: pins.ts). */
export type PinType = string;
export type PinDef = { id: string; type: PinType; label: string; required?: boolean; optional?: boolean; param?: string };
export type ParamDef = {
  id: string;
  label: string;
  /** name: an entity's name; keys: component keys; var: one of the graph's variables;
   *  varvalue: a value of that variable's type */
  kind: "choice" | "number" | "text" | "bool" | "name" | "entname" | "path" | "marker" | "keys" | "var" | "varvalue" | "func" | "timer" | "fn" | "macro" | "struct" | "enum" | "type" | "field" | "enumvalue" | "cases";
  default: string | number | boolean;
  choices?: [string, string][];
  min?: number;
  max?: number;
  varType?: "number" | "bool" | "text" | "struct" | "array";
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
  /** more words search finds it by (gravity -> sv_gravity) */
  keywords?: string;
  /** the cvar a World node reads or sets */
  cvar?: string;
  /** a power tool (server/AMX commands, plugin calls): flagged on community modes */
  advanced?: boolean;
  inputs: PinDef[];
  outputs: PinDef[];
  params: ParamDef[];
};

const data = JSON.parse(raw) as { nodes: NodeDef[]; categories: string[]; pinTypes: Record<string, { label: string; color: string }> };

export const NODE_DEFS = data.nodes;
export const CATEGORIES = data.categories;
export const PIN_TYPES = data.pinTypes;

const BY_TYPE = new Map(NODE_DEFS.map((d) => [d.type, d]));
export const nodeDef = (type: string): NodeDef | undefined => BY_TYPE.get(type);

/** `array<T>` -> { kind: "array", item: "T" }; `map<K,T>` -> { kind: "map", key: "K", item: "T" }. */
export function container(t: string): { kind: "array" | "map"; key?: string; item: string } | null {
  let m = /^array<(.+)>$/.exec(t);
  if (m) return { kind: "array", item: m[1] };
  m = /^map<([^,]+),(.+)>$/.exec(t);
  return m ? { kind: "map", key: m[1], item: m[2] } : null;
}

/** Can a wire carrying `from` go into a pin of type `to`? ("any" is a reroute's pin; a player
 *  is an entity; an array of anything fits any array.) Same rules as graph.rs `compatible`. */
export function compatible(from: PinType, to: PinType): boolean {
  if (from === to || from === "any" || to === "any" || (from === "player" && to === "entity")) return true;
  const a = container(from);
  const b = container(to);
  if (!a || !b || a.kind !== b.kind) return false;
  return (a.key === b.key || a.key === "any" || b.key === "any") && (a.item === b.item || a.item === "any" || b.item === "any");
}

/** A pin type's colour: containers take their item's, enums and structs their own. */
export function typeColor(t: PinType): string {
  const c = container(t);
  if (c) return typeColor(c.item);
  if (t.startsWith("enum:")) return PIN_TYPES.enum.color;
  if (t.startsWith("struct:")) return PIN_TYPES.struct.color;
  return (PIN_TYPES[t] ?? PIN_TYPES.any).color;
}

/** Entity names and creature types; component keys (the app checks the same). */
export const NAME_CHARS = /^[A-Za-z0-9_.-]{0,32}$/;
/** An entity's name as a map gives it (some maps' component names have spaces). */
export const ENT_NAME_CHARS = /^[A-Za-z0-9_. -]{0,32}$/;
/** a model or sound file (models/x.mdl, ambience/x.wav) */
export const PATH_CHARS = /^[A-Za-z0-9_./-]{0,64}$/;
export const KEYS_CHARS = /^[A-Za-z0-9 _.=:,|;@-]*$/;

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
    return `${d.title} ${d.category} ${d.help} ${d.keywords ?? ""} ${d.cvar ?? ""}`.toLowerCase().includes(q);
  }).sort((a, b) => {
    // titles that start with the words first, then by category order, then the registry's order
    // (the everyday nodes come first in each category: Give Weapon before Give Ammo)
    const sa = a.title.toLowerCase().startsWith(q) ? 0 : 1;
    const sb = b.title.toLowerCase().startsWith(q) ? 0 : 1;
    return sa - sb || CATEGORIES.indexOf(a.category) - CATEGORIES.indexOf(b.category);
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
  if (pin === "var" || pin.startsWith("$")) return wire.type !== "exec";
  if (pin.includes("$")) {
    // array<$T>, map<$K,$T>: the same kind of container
    const a = container(pin);
    const b = container(wire.type);
    return !!a && !!b && a.kind === b.kind;
  }
  return wire.dir === "out" ? compatible(wire.type, pin) : compatible(pin, wire.type);
}
