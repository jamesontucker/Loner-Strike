// What dropping something from My mode onto the canvas makes (Blueprint-style), by what it is
// and the key held: variables (Ctrl or Cmd: Get, Alt: Set, none: ask), Functions and Macros
// (a call), custom events (Ctrl: Call, none: Call or Set Timer by Event), events (its node, or
// the one already on this graph), Timelines (its node, or the one already placed), data tables
// (Get Row, Alt: For Each Row), enums (Switch, Ctrl: a value), structs (Make, Alt: Break), map
// markers (Find Marker). Pure, so it's tested without a page.

import { nodeDef } from "./registry";
import { allVariables } from "./pins";
import type { Graph } from "./schema";

export type Mods = { ctrl: boolean; alt: boolean };
export type Make = { type: string; params: Record<string, string | number | boolean> };
export type DropPlan =
  | { kind: "add"; make: Make }
  | { kind: "choose"; title: string; options: { label: string; make: Make }[] }
  | { kind: "jump"; node: string; message: string }
  | { kind: "error"; message: string };

/** The drag hint a My mode item shows in its tooltip. */
export function dragHint(what: string): string {
  switch (what) {
    case "var":
      return "Drag onto the canvas: Ctrl (⌘): Get · Alt (⌥): Set · onto a pin: a Get wired in · onto a flow wire: a Set in it";
    case "cevent":
      return "Drag onto the canvas: Ctrl (⌘): Call · Alt (⌥): its Custom Event node · or pick Call or Set Timer by Event";
    case "event":
      return "Drag onto the canvas: its event node here (or the one already on this graph)";
    case "tline":
      return "Drag onto the canvas: its Timeline node (or the one already placed)";
    case "table":
      return "Drag onto the canvas: Get Row · Alt (⌥): For Each Row";
    case "enum":
      return "Drag onto the canvas: Switch on it · Ctrl (⌘): one of its values";
    case "struct":
      return "Drag onto the canvas: Make · Alt (⌥): Break";
    default:
      return "Drag onto the canvas to call it";
  }
}

/** The event node already on this tab that matches (same kind and, for named events, the same
 *  name, command or custom event). */
export function sameEventHere(g: Graph, tab: string, type: string, params: Record<string, unknown>): string | null {
  const key = (p: Record<string, unknown>) => JSON.stringify([p.event ?? "", p.name ?? "", p.command ?? ""]);
  const want = key(params);
  return g.nodes.find((n) => n.type === type && (n.graph ?? "") === tab && key(n.params) === want)?.id ?? null;
}

/** What a drop makes. `payload` is "<kind>:<id>"; `tab` the graph tab dropped on. */
export function dropPlan(g: Graph, payload: string, mods: Mods, tab: string): DropPlan {
  const i = payload.indexOf(":");
  const kind = payload.slice(0, i);
  const id = payload.slice(i + 1);
  const inFn = (g.functions ?? []).some((f) => f.id === tab);
  switch (kind) {
    case "var": {
      const v = allVariables(g).find((x) => x.id === id);
      if (!v) return { kind: "error", message: "That variable isn't there any more." };
      const get: Make = { type: "var.get", params: { var: id } };
      const set: Make = { type: "var.set", params: { var: id } };
      if (mods.ctrl) return { kind: "add", make: get };
      if (mods.alt) return { kind: "add", make: set };
      return { kind: "choose", title: v.name, options: [{ label: `Get ${v.name}`, make: get }, { label: `Set ${v.name}`, make: set }] };
    }
    case "fn": {
      const f = (g.functions ?? []).find((x) => x.id === id);
      if (!f) return { kind: "error", message: "That Function isn't there any more." };
      if (id === tab) return { kind: "error", message: "A Function or Macro can't call itself." };
      return { kind: "add", make: { type: f.pure ? "fn.call_pure" : "fn.call", params: { func: id } } };
    }
    case "macro":
      if (id === tab) return { kind: "error", message: "A Function or Macro can't call itself." };
      return { kind: "add", make: { type: "macro.use", params: { macro: id } } };
    case "cevent": {
      const c = (g.customEvents ?? []).find((x) => x.id === id);
      if (!c) return { kind: "error", message: "That custom event isn't there any more." };
      const call: Make = { type: "event.call", params: { event: id } };
      const handler = (): DropPlan => {
        if (inFn) return { kind: "error", message: "Events go on an event graph, not in a Function." };
        const here = sameEventHere(g, tab, "event.custom", { event: id });
        return here ? { kind: "jump", node: here, message: "Already in this graph" } : { kind: "add", make: { type: "event.custom", params: { event: id } } };
      };
      if (mods.ctrl) return { kind: "add", make: call };
      if (mods.alt) return handler();
      const options = [{ label: `Call ${c.name}`, make: call }];
      if (c.inputs.length === 0) options.push({ label: `Set Timer by ${c.name}`, make: { type: "timer.by_event", params: { event: id } } });
      if (!inFn) options.push({ label: `Custom Event: ${c.name}`, make: { type: "event.custom", params: { event: id } } });
      return { kind: "choose", title: c.name, options };
    }
    case "event": {
      // an event entry: one like it on this graph, else a new one with the same settings
      const n = g.nodes.find((x) => x.id === id);
      if (!n) return { kind: "error", message: "That event isn't there any more." };
      if (inFn) return { kind: "error", message: "Events go on an event graph, not in a Function." };
      const here = sameEventHere(g, tab, n.type, n.params);
      if (here) return { kind: "jump", node: here, message: "Already in this graph" };
      return { kind: "add", make: { type: n.type, params: { ...n.params } } };
    }
    case "tline": {
      const placed = g.nodes.find((n) => n.type === "timeline.play" && n.params.timeline === id);
      // (a Timeline keeps its own time: one node plays it, so a second drop finds the first)
      if (placed) return { kind: "jump", node: placed.id, message: "That Timeline is already placed: here it is" };
      return { kind: "add", make: { type: "timeline.play", params: { timeline: id } } };
    }
    case "table":
      return { kind: "add", make: { type: mods.alt ? "table.for_each" : "table.get_row", params: { table: id } } };
    case "enum":
      return { kind: "add", make: mods.ctrl ? { type: "enum.value", params: { enum: id, value: 0 } } : { type: "flow.switch_enum", params: { enum: id } } };
    case "make":
    case "struct":
      return { kind: "add", make: { type: mods.alt ? "struct.break" : "struct.make", params: { struct: id } } };
    case "break":
      return { kind: "add", make: { type: "struct.break", params: { struct: id } } };
    case "marker":
      return { kind: "add", make: { type: "marker.find", params: { marker: id } } };
  }
  return { kind: "error", message: "That can't go on the canvas." };
}

/** A drop's node type exists in the registry (a guard for old payloads). */
export const knownMake = (m: Make) => !!nodeDef(m.type);
