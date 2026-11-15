// A node's pins as the graph makes them: Function, Macro, struct and switch nodes get theirs from
// the graph, generic nodes (arrays, maps, Equal, Select...) from their settings. The app works
// them out the same way (graph.rs View::make_pins and resolve); the checks there are the ones
// that count.

import { container, nodeDef, PIN_TYPES, type NodeDef, type PinDef, type PinType } from "./registry";
import { allEnums, allStructs, type GMember, type GNode, type Graph } from "./schema";
import { tablePins } from "./tables";

export const BASE_TYPES = [
  "number", "bool", "text", "player", "entity", "team", "vector", "timer",
  // (graph version 4: files, a color, the entity handles, a playing loop; graph.rs BASE_TYPES)
  "model", "sprite", "sound", "decal", "color", "npc", "creature", "projectile", "volume", "objmarker", "component", "soundloop",
] as const;
export const KEY_TYPES = ["text", "number", "player"] as const;

/** A plain type, an enum or (with `structs`) a struct of this graph. */
export function itemTypeOk(g: Graph, t: string, structs: boolean): boolean {
  if ((BASE_TYPES as readonly string[]).includes(t)) return true;
  if (t.startsWith("enum:")) return allEnums(g).some((e) => `enum:${e.id}` === t);
  if (t.startsWith("struct:")) return structs && allStructs(g).some((s) => `struct:${s.id}` === t);
  return false;
}

/** What a variable, a Function's input or output can be (graph.rs type_ok). */
export function typeOk(g: Graph, t: string): boolean {
  const c = container(t);
  if (c?.kind === "array") return itemTypeOk(g, c.item, true);
  if (c?.kind === "map") return (KEY_TYPES as readonly string[]).includes(c.key ?? "") && itemTypeOk(g, c.item, false);
  return itemTypeOk(g, t, true);
}

/** A type's name for people: Number, Array of Players, Loadout... */
export function typeLabel(g: Graph | null, t: string): string {
  const c = container(t);
  if (c) return c.kind === "array" ? `Array of ${typeLabel(g, c.item)}` : `Map of ${typeLabel(g, c.key ?? "text")} to ${typeLabel(g, c.item)}`;
  if (t.startsWith("enum:")) return allEnums(g).find((e) => `enum:${e.id}` === t)?.name ?? "Enum";
  if (t.startsWith("struct:")) return allStructs(g).find((s) => `struct:${s.id}` === t)?.name ?? "Struct";
  return PIN_TYPES[t]?.label ?? t;
}

/** Text, vectors and structs don't last through a Delay unless kept in a variable. */
export const shortLived = (t: string) => t === "text" || t === "vector" || t.startsWith("struct:");

const execPin = (id: string, label: string): PinDef => ({ id, type: "exec", label });
const memberPin = (m: GMember, required: boolean): PinDef => ({ id: m.id, type: m.type, label: m.name, ...(required && m.type !== "exec" ? { required: true } : { optional: true }) });

export const countParam = (n: GNode, def: NodeDef, id: string, max: number) => {
  const v = n.params[id] ?? def.params.find((p) => p.id === id)?.default;
  return typeof v === "number" && Number.isFinite(v) ? Math.min(max, Math.max(0, Math.round(v))) : 0;
};

/** A Switch's cases: words or numbers split by commas, at most 16. */
export function switchCases(n: GNode, def: NodeDef): string[] {
  const v = n.params.cases ?? def.params.find((p) => p.id === "cases")?.default ?? "";
  return String(v)
    .split(",")
    .map((c) => c.trim())
    .filter(Boolean)
    .slice(0, 16);
}

const fnOf = (g: Graph, id: unknown) => (g.functions ?? []).find((f) => f.id === id);
const macroOf = (g: Graph, id: unknown) => (g.macros ?? []).find((m) => m.id === id);
const structOf = (g: Graph, id: unknown) => allStructs(g).find((s) => s.id === id);
const enumOf = (g: Graph, id: unknown) => allEnums(g).find((e) => e.id === id);
/** every variable a node can name: the mode's and every Function's locals */
export const allVariables = (g: Graph) => [...g.variables, ...(g.functions ?? []).flatMap((f) => f.locals ?? [])];

/** A pin type with var, $T, $K, $E, $S and $F worked out ("any" while one isn't set). */
function resolve(g: Graph, n: GNode, def: NodeDef, t: PinType): PinType {
  if (t === "var") return allVariables(g).find((x) => x.id === n.params.var)?.type ?? "any";
  if (!t.includes("$")) return t;
  const p = (k: string) => String(n.params[k] ?? def.params.find((x) => x.id === k)?.default ?? "");
  const item = itemTypeOk(g, p("of"), !t.startsWith("map<")) ? p("of") : "any";
  const key = (KEY_TYPES as readonly string[]).includes(p("key")) ? p("key") : "text";
  const en = enumOf(g, p("enum")) ? `enum:${p("enum")}` : "any";
  const st = structOf(g, p("struct")) ? `struct:${p("struct")}` : "any";
  let field = "any";
  if (t.includes("$F")) {
    const v = allVariables(g).find((x) => x.id === n.params.var);
    const s = v && structOf(g, v.type.replace(/^struct:/, ""));
    field = s?.fields.find((f) => f.id === n.params.field)?.type ?? "any";
  }
  const swap = (x: string, a: string, b: string) => x.split(a).join(b);
  return swap(swap(swap(swap(swap(t, "$T", item), "$K", key), "$E", en), "$S", st), "$F", field);
}

export type Pins = { inputs: PinDef[]; outputs: PinDef[] };

const cache = new WeakMap<Graph, Map<string, Pins>>();

/** A node's pins, types worked out. */
export function pinsOf(g: Graph, n: GNode): Pins {
  let m = cache.get(g);
  if (!m) cache.set(g, (m = new Map()));
  const hit = m.get(n.id);
  if (hit) return hit;
  const def = nodeDef(n.type);
  if (!def) return { inputs: [], outputs: [] };
  let inputs = def.inputs;
  let outputs = def.outputs;
  const home = n.graph;
  switch (def.type) {
    case "fn.entry": {
      const f = fnOf(g, home);
      if (f) outputs = [...outputs, ...f.inputs.map((x) => memberPin(x, false))];
      break;
    }
    case "fn.return": {
      const f = fnOf(g, home);
      if (f) inputs = [...inputs, ...f.outputs.map((x) => memberPin(x, false))];
      break;
    }
    case "table.get_row":
    case "table.get_cell":
    case "table.for_each":
    case "table.find":
    case "table.keys": {
      const r = tablePins(g, n);
      if (r) {
        inputs = inputs.map((p) => ({ ...p, type: r.inType(p.id) ?? p.type }));
        outputs = [...outputs.map((p) => ({ ...p, type: r.outType(p.id) ?? p.type })), ...r.extra];
      }
      break;
    }
    case "timeline.play": {
      const tl = (g.timelines ?? []).find((x) => x.id === n.params.timeline);
      if (tl) outputs = [...outputs, ...tl.tracks.map((t): PinDef => ({ id: t.id, type: t.kind === "event" ? "exec" : t.kind === "float" ? "number" : "vector", label: t.name, optional: true }))];
      break;
    }
    case "event.custom":
    case "event.call": {
      const c = (g.customEvents ?? []).find((x) => x.id === n.params.event);
      if (c && def.type === "event.custom") outputs = [...outputs, ...c.inputs.map((x) => memberPin(x, false))];
      if (c && def.type === "event.call") inputs = [...inputs, ...c.inputs.map((x) => memberPin(x, false))];
      break;
    }
    case "fn.call":
    case "fn.call_pure": {
      const f = fnOf(g, n.params.func);
      if (f) {
        inputs = [...inputs, ...f.inputs.map((x) => memberPin(x, false))];
        outputs = [...outputs, ...f.outputs.map((x) => memberPin(x, false))];
      }
      break;
    }
    case "macro.entry": {
      const mm = macroOf(g, home);
      if (mm) outputs = mm.inputs.map((x) => memberPin(x, false));
      break;
    }
    case "macro.exit": {
      const mm = macroOf(g, home);
      if (mm) inputs = mm.outputs.map((x) => memberPin(x, false));
      break;
    }
    case "macro.use": {
      const mm = macroOf(g, n.params.macro);
      if (mm) {
        inputs = mm.inputs.map((x) => memberPin(x, true));
        outputs = mm.outputs.map((x) => memberPin(x, false));
      }
      break;
    }
    case "struct.make": {
      const s = structOf(g, n.params.struct);
      if (s) inputs = s.fields.map((x) => memberPin(x, false));
      break;
    }
    case "struct.break": {
      const s = structOf(g, n.params.struct);
      if (s) outputs = s.fields.map((x) => memberPin(x, false));
      break;
    }
    case "flow.switch_enum": {
      const e = enumOf(g, n.params.enum);
      if (e) outputs = e.values.map((v, i) => execPin(`v${i}`, v));
      break;
    }
    case "flow.switch_num":
    case "flow.switch_text":
      outputs = [...switchCases(n, def).map((c, i) => execPin(`c${i}`, c)), execPin("default", "Default")];
      break;
    case "flow.sequence":
      outputs = Array.from({ length: Math.max(1, countParam(n, def, "count", 32)) }, (_, i) => execPin(`then${i}`, `Then ${i}`));
      break;
    // W1: Multi Gate's outputs
    case "flow.multi_gate":
      outputs = Array.from({ length: Math.max(1, countParam(n, def, "count", 32)) }, (_, i) => execPin(`then${i}`, `Out ${i}`));
      break;
    case "array.make": {
      const item = def.inputs[0];
      if (item) inputs = Array.from({ length: countParam(n, def, "count", 8) }, (_, i) => ({ ...item, id: `i${i}`, label: `[${i}]` }));
      break;
    }
    case "var.get":
    case "var.set":
    case "var.add":
    case "var.toggle": {
      // Player only for a per-player variable, Team only for a per-team one (a wire already on
      // one keeps it showing, so it can be seen and taken off): UX review M4
      const v = allVariables(g).find((x) => x.id === n.params.var);
      if (v) {
        const wired = (pin: string) => g.edges.some((e) => e.to.node === n.id && e.to.pin === pin);
        inputs = inputs.filter((p) => (p.id !== "player" || v.scope === "player" || wired("player")) && (p.id !== "team" || v.scope === "team" || wired("team")));
      }
      break;
    }
  }
  const out: Pins = {
    inputs: inputs.map((p) => ({ ...p, type: resolve(g, n, def, p.type) })),
    outputs: outputs.map((p) => ({ ...p, type: resolve(g, n, def, p.type) })),
  };
  // orphans: a wire to a pin the node doesn't have any more (an update took it away) stays,
  // on a red pin of its own, until it's re-wired or deleted (graph.rs ORPHAN_PIN)
  for (const e of g.edges) {
    if (e.to.node === n.id && !out.inputs.some((p) => p.id === e.to.pin)) out.inputs.push({ id: e.to.pin, type: "any", label: `Orphaned: ${e.to.pin}`, orphan: true });
    if (e.from.node === n.id && !out.outputs.some((p) => p.id === e.from.pin)) out.outputs.push({ id: e.from.pin, type: "any", label: `Orphaned: ${e.from.pin}`, orphan: true });
  }
  m.set(n.id, out);
  return out;
}

/** Settings a node has a value for that its definition doesn't (removed in an update): kept,
 *  shown greyed until cleared (graph.rs ORPHAN_VALUE). */
export function orphanValues(n: GNode): string[] {
  const def = nodeDef(n.type);
  if (!def) return [];
  return Object.keys(n.params ?? {}).filter((k) => !def.params.some((p) => p.id === k));
}

/** The registry's own pin (its type before it's worked out), for generic nodes. */
export function rawPin(n: GNode, pin: string, dir: "in" | "out"): PinDef | undefined {
  const def = nodeDef(n.type);
  const list = dir === "in" ? def?.inputs : def?.outputs;
  return list?.find((p) => p.id === pin) ?? (n.type === "array.make" && dir === "in" ? def?.inputs[0] : undefined);
}

/** Settings that make a generic node fit a wire of `wire` type on its pin (Holds, Keys), or
 *  null when it doesn't need any (or can't). */
export function adaptTo(g: Graph, n: GNode, pin: string, dir: "in" | "out", wire: string): Record<string, string> | null {
  const raw = rawPin(n, pin, dir)?.type;
  // an enum wire into Switch on Enum (or Enum To Number): the node takes that enum, so its cases
  // fill by themselves (the app's own enums, Kind Of's, too)
  if (raw === "$E" && wire.startsWith("enum:") && itemTypeOk(g, wire, false)) {
    const id = wire.slice(5);
    return n.params.enum !== id ? { enum: id } : null;
  }
  if (!raw || !raw.includes("$T") || wire === "any" || wire === "exec") return null;
  let item: string | undefined;
  let key: string | undefined;
  if (raw === "$T") item = wire;
  else {
    const a = container(raw);
    const b = container(wire);
    if (!a || !b || a.kind !== b.kind) return null;
    item = b.item;
    if (a.key === "$K") key = b.key;
  }
  const isMap = n.type.startsWith("map.");
  if (!item || !itemTypeOk(g, item, !isMap)) return null;
  const patch: Record<string, string> = {};
  if (n.params.of !== item) patch.of = item;
  if (key && n.params.key !== key) patch.key = key;
  return Object.keys(patch).length ? patch : null;
}

/** The functions and macros a Call or Macro node can be set to, for the palette and Details. */
export function callables(g: Graph) {
  return {
    functions: g.functions ?? [],
    macros: g.macros ?? [],
  };
}

/** The node title as drawn: a Call shows its Function, a Make its struct... */
export function nodeTitle(g: Graph, n: GNode): string {
  const def = nodeDef(n.type);
  if (!def) return n.type;
  switch (n.type) {
    case "fn.call":
    case "fn.call_pure":
      return fnOf(g, n.params.func)?.name ?? "Call (pick a Function)";
    case "fn.entry":
      return fnOf(g, n.graph)?.name ?? def.title;
    case "timeline.play": {
      const tl = (g.timelines ?? []).find((x) => x.id === n.params.timeline);
      return tl ? `Timeline: ${tl.name}` : def.title;
    }
    case "table.get_row":
    case "table.get_cell":
    case "table.for_each":
    case "table.find":
    case "table.count":
    case "table.keys": {
      const t = (g.dataTables ?? []).find((x) => x.id === n.params.table);
      return t ? `${def.title}: ${t.name}` : def.title;
    }
    case "event.custom":
    case "event.call":
    case "timer.by_event": {
      const c = (g.customEvents ?? []).find((x) => x.id === n.params.event);
      if (!c) return def.title;
      return n.type === "event.custom" ? `Custom Event: ${c.name}` : n.type === "event.call" ? `Call ${c.name}` : `Set Timer by ${c.name}`;
    }
    case "macro.use":
      return macroOf(g, n.params.macro)?.name ?? "Macro (pick one)";
    case "macro.entry":
      return `${macroOf(g, n.graph)?.name ?? "Macro"}: In`;
    case "macro.exit":
      return `${macroOf(g, n.graph)?.name ?? "Macro"}: Out`;
    case "struct.make":
    case "struct.break": {
      const s = structOf(g, n.params.struct);
      return s ? `${n.type === "struct.make" ? "Make" : "Break"} ${s.name}` : def.title;
    }
    case "flow.switch_enum": {
      const e = enumOf(g, n.params.enum);
      return e ? `Switch on ${e.name}` : def.title;
    }
    case "enum.value": {
      const e = enumOf(g, n.params.enum);
      return e ? `${e.name}: ${e.values[Number(n.params.value ?? 0)] ?? "?"}` : def.title;
    }
    case "var.get":
    case "var.set": {
      const v = allVariables(g).find((x) => x.id === n.params.var);
      return v ? `${n.type === "var.get" ? "Get" : "Set"} ${v.name}` : def.title;
    }
  }
  return def.title;
}
