// P5: mode map kits in the editor (format: docs/notes/X.md "Mode map kit format"; the app reads
// them from <app dir>/kits, tools/editor/kits.rs). A kit's markers are `ls_marker` entities
// (`kind` = marker id, `mode` = mode id) with the kit's keys; its components are the `ls_*`
// entities. The map says which modes it's made for in the worldspawn key `ls_modes`.
// Also: the creature types for ls_creature and wave spawners (built-in + creatures.ini).

import { create } from "zustand";
import type { Vec3 } from "../viewer/coords";
import type { DocEntity, FgdClass, FgdKey, Issue } from "./editorApi";
import { keyOf, useEditor } from "./store";

export type KitKey = { name: string; type: "number" | "text" | "bool" | "choice"; default?: unknown; help?: string; min?: number; max?: number; choices?: [string, string][] };
export type KitMarker = {
  id: string;
  label: string;
  description?: string;
  type: "point" | "area" | "path";
  required: { min: number; max: number | null };
  keys: KitKey[];
  draw: { colour?: string; icon?: string; shape?: string; size?: Vec3; radius?: number };
};
export type KitComponent = { classname: string; min: number; max: number | null; note?: string };
export type Kit = {
  mode: { id: string; name: string; description?: string };
  source?: string;
  markers: KitMarker[];
  components: KitComponent[];
  creatures: string[];
  notes?: string;
};
export type KitsReply = { folder: string; kits: Kit[]; bad: { file: string; error: string }[]; madeFor: string[]; key: string; needs: Issue[]; error?: string };

export const MODES_KEY = "ls_modes";

type KitState = { kits: Kit[]; bad: KitsReply["bad"]; folder: string; loaded: boolean; creatures: Creature[]; set: (p: Partial<KitState>) => void };
export const useKits = create<KitState>((set) => ({ kits: [], bad: [], folder: "", loaded: false, creatures: [], set }));

/** Reads the installed kits (and what the map's modes still need). */
export async function loadKits(): Promise<KitsReply | null> {
  try {
    const r = (await (await fetch("/api/tools/editor/kits")).json()) as KitsReply;
    if (r.error) return null;
    useKits.getState().set({ kits: r.kits, bad: r.bad, folder: r.folder, loaded: true });
    return r;
  } catch {
    return null;
  }
}

export const kitOf = (mode: string | undefined) => useKits.getState().kits.find((k) => k.mode.id === mode);

/** The modes the map is made for. */
export function madeForOf(world: { keys: [string, string][] } | undefined): string[] {
  const v = (world ? keyOf(world.keys, MODES_KEY) ?? "" : "").split(/[\s,;]+/).filter((x) => /^[a-z][a-z0-9_]{0,23}$/.test(x));
  return [...new Set(v)];
}

/** Sets the modes the map is made for (one undo step; saved in the project and the map). */
export async function setMadeFor(ids: string[]) {
  const s = useEditor.getState();
  const d = s.doc;
  if (!d) return;
  const keys = d.world.keys.filter(([k]) => k !== MODES_KEY);
  if (ids.length) keys.push([MODES_KEY, ids.join(" ")]);
  await s.cmd({ op: "set_world", keys });
}

const keyText = (k: KitKey) => (k.default == null ? "" : typeof k.default === "boolean" ? (k.default ? "1" : "0") : String(k.default));

/** The keys a new marker gets: kind, mode, its keys' defaults, an area's size/radius, a path's spot. */
export function markerKeys(kit: Kit, m: KitMarker, existing: DocEntity[] = []): [string, string][] {
  const keys: [string, string][] = [
    ["kind", m.id],
    ["mode", kit.mode.id],
  ];
  for (const k of m.keys) {
    const v = keyText(k);
    if (v !== "") keys.push([k.name, v]);
  }
  if (m.type === "area") {
    if (m.draw.radius) keys.push(["radius", String(m.draw.radius)]);
    else keys.push(["size", (m.draw.size ?? [256, 256, 128]).join(" ")]);
  }
  if (m.type === "path") {
    // the next spot of the newest path
    const spots = existing.filter((e) => e.classname === "ls_marker" && keyOf(e.keys, "kind") === m.id);
    const path = spots.length ? Math.max(...spots.map((e) => Number(keyOf(e.keys, "path") ?? 1) || 1)) : 1;
    const order = Math.max(0, ...spots.filter((e) => (Number(keyOf(e.keys, "path") ?? 1) || 1) === path).map((e) => Number(keyOf(e.keys, "order")) || 0)) + 1;
    keys.push(["path", String(path)], ["order", String(order)]);
  }
  return keys;
}

/** What to place for a `place` id ("kit:<mode>:<marker or component>" or "<mode>:<id>"). */
export function kitPlacement(id: string): { classname: string; keys: [string, string][]; name: string } | null {
  const m = /^(?:kit:)?([a-z][a-z0-9_]*):([a-z0-9_]+)$/.exec(id);
  if (!m) return null;
  const kit = kitOf(m[1]);
  if (!kit) return null;
  const marker = kit.markers.find((x) => x.id === m[2]);
  if (marker) return { classname: "ls_marker", keys: markerKeys(kit, marker, useEditor.getState().doc?.entities ?? []), name: `${kit.mode.name}: ${marker.label}` };
  const comp = kit.components.find((c) => c.classname === m[2]);
  if (comp) return { classname: comp.classname, keys: [], name: `${kit.mode.name}: ${comp.classname}` };
  return null;
}

/** The marker an ls_marker entity is (its kit and the kit's marker), when a kit has it. */
export function markerOf(e: DocEntity): { kit: Kit; marker: KitMarker } | null {
  if (e.classname !== "ls_marker") return null;
  const kind = keyOf(e.keys, "kind")?.trim();
  const mode = keyOf(e.keys, "mode")?.trim();
  const kits = useKits.getState().kits;
  const order = mode && mode !== "any" ? kits.filter((k) => k.mode.id === mode) : kits;
  for (const kit of order) {
    const marker = kit.markers.find((m) => m.id === kind);
    if (marker) return { kit, marker };
  }
  return null;
}

/** How an ls_marker is drawn: its kit's colour. */
export function markerColour(e: DocEntity): string {
  return markerOf(e)?.marker.draw.colour || "#e8c547";
}

/** An entity form for ls_marker from its kit: kind and mode, the marker's own keys typed. */
export function markerDef(e: DocEntity, base: FgdClass | undefined): FgdClass | undefined {
  if (e.classname !== "ls_marker") return undefined;
  const kits = useKits.getState().kits;
  const found = markerOf(e);
  const kinds = kits.flatMap((k) => [...k.markers.map((m) => ({ value: m.id, label: `${m.label} (${k.mode.name})` })), ...k.components.map((c) => ({ value: c.classname, label: `${c.classname} (${k.mode.name})` }))]);
  const uniq = [...new Map(kinds.map((c) => [c.value, c])).values()];
  const keys: FgdKey[] = [
    { name: "kind", type: "choices", label: "Marks", help: "The kit's marker (or an ls_ component the mode's components make of it).", choices: uniq },
    { name: "mode", type: "choices", label: "For mode", default: "any", help: "Only this mode uses it; Any: every mode with this marker.", choices: [{ value: "any", label: "Any mode" }, ...kits.map((k) => ({ value: k.mode.id, label: k.mode.name }))] },
  ];
  if (found) {
    const m = found.marker;
    for (const k of m.keys) {
      const label = k.name.replace(/_/g, " ").replace(/^./, (c) => c.toUpperCase());
      if (k.type === "choice") keys.push({ name: k.name, type: "choices", label, default: keyText(k), help: k.help, choices: (k.choices ?? []).map(([value, l]) => ({ value, label: l })) });
      else if (k.type === "bool") keys.push({ name: k.name, type: "choices", label, default: keyText(k), help: k.help, choices: [{ value: "1", label: "Yes" }, { value: "0", label: "No" }] });
      else if (k.type === "number") keys.push({ name: k.name, type: "float", label, default: keyText(k), help: [k.help, k.min != null && k.max != null ? `${k.min} to ${k.max}.` : ""].filter(Boolean).join(" ") });
      else keys.push({ name: k.name, type: "string", label, default: keyText(k), help: k.help });
    }
    if (m.type === "area") {
      keys.push({ name: "size", type: "string", label: "Box size (x y z)", default: m.draw.radius ? "" : (m.draw.size ?? [256, 256, 128]).join(" "), help: "The area: a box round the marker. Leave empty to use a radius." });
      keys.push({ name: "radius", type: "float", label: "Radius", default: m.draw.radius ? String(m.draw.radius) : "", help: "The area: a round one this far from the marker (instead of a box)." });
    }
    if (m.type === "path") {
      keys.push({ name: "path", type: "integer", label: "Path", default: "1", help: "Which path this spot is on (1, 2, …)." });
      keys.push({ name: "order", type: "integer", label: "Order", default: "1", help: "Its place along the path (1, 2, 3, …)." });
    }
  }
  keys.push({ name: "targetname", type: "target_source", label: "Name", help: "Optional, for the mode's plugin." });
  keys.push({ name: "angle", type: "angle", label: "Facing", help: "Which way it faces." });
  return {
    name: "ls_marker",
    kind: "point",
    description: found ? `${found.kit.mode.name}: ${found.marker.label}. ${found.marker.description ?? ""}`.trim() : base?.description || "A spot a game mode uses (mode map kits).",
    keys,
  };
}

// ---- creature types ------------------------------------------------------------------------

export type Creature = { name: string; display: string; model: string; builtin: boolean };

let creaturesAsked: Promise<Creature[]> | null = null;
/** Every creature type: the built-in ones and the user's creatures.ini (community ones are installed there). */
export function loadCreatures(): Promise<Creature[]> {
  creaturesAsked ??= fetch("/api/tools/creatures")
    .then((r) => r.json())
    .then((r: { types?: (Raw & { builtin?: boolean })[]; templates?: Raw[]; error?: string }) => {
      const out = new Map<string, Creature>();
      for (const t of r.templates ?? []) if (t?.name) out.set(t.name, { name: t.name, display: t.display || t.name, model: t.model ?? "", builtin: true });
      // (the file's types, built-in ones it changes included)
      for (const t of r.types ?? []) {
        if (t?.name) out.set(t.name, { name: t.name, display: t.display || t.name, model: t.model || out.get(t.name)?.model || "", builtin: t.builtin ?? out.get(t.name)?.builtin ?? false });
      }
      const list = [...out.values()];
      useKits.getState().set({ creatures: list });
      return list;
    })
    .catch(() => {
      creaturesAsked = null;
      return [];
    });
  return creaturesAsked;
}
type Raw = { name: string; display?: string; model?: string };

/** ls_creature's Type as a list of every installed creature type (others kept as typed). */
export function creatureDef(def: FgdClass | undefined, creatures: Creature[]): FgdClass | undefined {
  if (!def || def.name !== "ls_creature" || !creatures.length) return def;
  return {
    ...def,
    keys: def.keys.map((k) => (k.name === "type" ? { ...k, choices: creatures.map((c) => ({ value: c.name, label: `${c.display}${c.builtin ? "" : " (yours)"}` })) } : k)),
  };
}
