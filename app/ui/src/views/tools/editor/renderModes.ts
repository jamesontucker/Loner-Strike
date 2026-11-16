// P5: how each pane draws brushes: Textured, Flat (shaded, a colour per texture), Wireframe,
// or Textured + wire. Picked in each pane's header; flat 2D panes default to wireframe like
// Hammer, the 3D pane to textured. Kept with the layout.

import { useContext } from "react";
import * as THREE from "three";
import { PaneCtx, usePanes } from "./panes";
import type { ViewId } from "./store";

export type RenderMode = "textured" | "flat" | "wire" | "texwire" | "collision" | "bytype" | "missing";
export const RENDER_MODES: { id: RenderMode; label: string }[] = [
  { id: "textured", label: "Textured" },
  { id: "flat", label: "Flat" },
  { id: "wire", label: "Wireframe" },
  { id: "texwire", label: "Textured + wire" },
  { id: "collision", label: "Collision" },
  { id: "bytype", label: "By entity type" },
  { id: "missing", label: "Missing textures" },
];

// ---- the information styles: a color per kind of brush, a legend in the pane's corner ----

type Cat = { id: string; label: string; color: string; opacity?: number };
/** Collision: what blocks what. */
export const COLLISION_CATS: Cat[] = [
  { id: "solid", label: "Solid (world and solid entities)", color: "#8a8f86" },
  { id: "clip", label: "CLIP: players only", color: "#ff8c1a", opacity: 0.5 },
  { id: "bullets", label: "NULL / bullets and players", color: "#e53935", opacity: 0.75 },
  { id: "trigger", label: "Trigger (walk through, sets off)", color: "#9b59ff", opacity: 0.35 },
  { id: "ladder", label: "Ladder", color: "#3fbf5f", opacity: 0.6 },
  { id: "water", label: "Water", color: "#3a8dde", opacity: 0.5 },
  { id: "ghost", label: "Not solid (illusionary)", color: "#c8ccc2", opacity: 0.08 },
];
/** By entity type: what each brush is. */
export const TYPE_CATS: Cat[] = [
  { id: "world", label: "World", color: "#8a8f86" },
  { id: "detail", label: "func_detail", color: "#5f9ea0" },
  { id: "wall", label: "func_wall", color: "#c9a227" },
  { id: "door", label: "Doors, buttons, trains", color: "#4a90e2" },
  { id: "break", label: "Breakables, pushables", color: "#d0605e" },
  { id: "illusion", label: "Illusionary", color: "#c8ccc2", opacity: 0.25 },
  { id: "trigger", label: "Triggers and zones", color: "#9b59ff", opacity: 0.35 },
  { id: "other", label: "Other entities", color: "#7cc576" },
];
export const MISSING_CATS: Cat[] = [
  { id: "missing", label: "Missing or unknown texture", color: "#ff00ff" },
  { id: "ok", label: "Everything else", color: "#4a4e46", opacity: 0.6 },
];
export const legendFor = (m: RenderMode): Cat[] | null => (m === "collision" ? COLLISION_CATS : m === "bytype" ? TYPE_CATS : m === "missing" ? MISSING_CATS : null);
/** Styles that color a brush by what it belongs to (each brush drawn on its own). */
export const byClass = (m: RenderMode) => m === "collision" || m === "bytype";

/** A brush face's kind in a style, from its texture and its entity's class ("" for the world). */
export function categoryOf(m: RenderMode, texture: string, cls: string): string {
  const t = texture.toLowerCase();
  if (m === "collision") {
    if (/^func_illusionary$/.test(cls)) return "ghost";
    if (/^func_ladder$/.test(cls)) return "ladder";
    if (/^func_water$/.test(cls) || t.startsWith("!")) return "water";
    if (/^(trigger_|func_buyzone|func_bomb_target|func_hostage_rescue|func_vip_safetyzone|func_escapezone)/.test(cls) || t === "aaatrigger") return "trigger";
    if (t === "clip") return "clip";
    if (t === "null") return "bullets";
    return "solid";
  }
  if (!cls) return "world";
  if (cls === "func_detail") return "detail";
  if (cls === "func_wall" || cls === "func_wall_toggle") return "wall";
  if (/^(func_door|func_button|func_rot_button|func_train|func_tracktrain|func_plat|func_rotating|momentary_)/.test(cls)) return "door";
  if (/^(func_breakable|func_pushable)/.test(cls)) return "break";
  if (cls === "func_illusionary") return "illusion";
  if (/^(trigger_|func_buyzone|func_bomb_target|func_hostage_rescue|func_vip_safetyzone|func_escapezone|func_ladder)/.test(cls)) return "trigger";
  return "other";
}

const catCache = new Map<string, THREE.MeshLambertMaterial>();
export function categoryMaterial(m: RenderMode, cat: string): THREE.MeshLambertMaterial {
  const key = `${m}:${cat}`;
  let mat = catCache.get(key);
  if (!mat) {
    const c = (legendFor(m) ?? []).find((x) => x.id === cat) ?? { color: "#888", opacity: 1 };
    const op = c.opacity ?? 1;
    mat = new THREE.MeshLambertMaterial({ color: new THREE.Color(c.color), transparent: op < 1, opacity: op, depthWrite: op >= 0.5 });
    catCache.set(key, mat);
  }
  return mat;
}

/** A pane's mode: what was picked for it, else the default for its view. */
export function modeFor(mode: RenderMode | null | undefined, view: ViewId): RenderMode {
  return mode ?? (view === "3d" ? "textured" : "wire");
}

/** The mode of the pane this is drawn in (textured outside panes). */
export function usePaneMode(): RenderMode {
  const p = useContext(PaneCtx);
  const picked = usePanes((s) => (p ? s.modes?.[p.slot] : null));
  return p ? modeFor(picked, p.view) : "textured";
}

export function setPaneMode(slot: number, mode: RenderMode) {
  const st = usePanes.getState();
  const modes = [...(st.modes ?? [null, null, null, null])];
  modes[slot] = mode;
  st.set({ modes });
}

const flatCache = new Map<string, THREE.MeshLambertMaterial>();
/** A soft colour per texture name (the same name, the same colour), lit by the scene. */
export function flatMaterial(texture: string): THREE.MeshLambertMaterial {
  const key = texture.toLowerCase();
  let m = flatCache.get(key);
  if (!m) {
    let h = 2166136261;
    for (let i = 0; i < key.length; i++) h = Math.imul(h ^ key.charCodeAt(i), 16777619);
    const hue = ((h >>> 0) % 360) / 360;
    const sky = /^sky$/.test(key);
    const tool = /^(aaatrigger|null|clip|origin|skip|hint|bevel)$/.test(key);
    const c = new THREE.Color().setHSL(hue, sky ? 0.5 : tool ? 0.6 : 0.22, sky ? 0.7 : tool ? 0.5 : 0.52);
    m = new THREE.MeshLambertMaterial({ color: c, transparent: tool, opacity: tool ? 0.45 : 1 });
    flatCache.set(key, m);
  }
  return m;
}

/** The brush outline's look in a mode. */
export function edgeLook(mode: RenderMode, picked: boolean, tied: boolean): { color: string; opacity: number } {
  if (picked) return { color: "#ffd84a", opacity: 0.95 };
  if (mode === "wire") return { color: tied ? "#62c6d6" : "#d8dccb", opacity: 0.85 };
  if (mode === "texwire") return { color: tied ? "#62c6d6" : "#e6ead8", opacity: 0.6 };
  return { color: tied ? "#62c6d6" : "#10130d", opacity: 0.35 };
}
