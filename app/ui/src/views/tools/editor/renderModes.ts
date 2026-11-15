// P5: how each pane draws brushes: Textured, Flat (shaded, a colour per texture), Wireframe,
// or Textured + wire. Picked in each pane's header; flat 2D panes default to wireframe like
// Hammer, the 3D pane to textured. Kept with the layout.

import { useContext } from "react";
import * as THREE from "three";
import { PaneCtx, usePanes } from "./panes";
import type { ViewId } from "./store";

export type RenderMode = "textured" | "flat" | "wire" | "texwire";
export const RENDER_MODES: { id: RenderMode; label: string }[] = [
  { id: "textured", label: "Textured" },
  { id: "flat", label: "Flat" },
  { id: "wire", label: "Wireframe" },
  { id: "texwire", label: "Textured + wire" },
];

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
