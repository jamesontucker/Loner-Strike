// The nav fixer's view state that both 3D views share (split view): what's under the mouse,
// the box being drawn, focus requests, and where the camera was (so switching between 3D and
// top-down keeps your place). Kept out of React state so hovering doesn't re-render the page.

import { create } from "zustand";
import * as THREE from "three";
import type { Vec3 } from "../viewer/coords";

export type ViewKind = "orbit" | "top";
/** what a left click does: pick, draw a box, or link two areas */
export type Mode = "select" | "draw" | "link";
export type Layout = ViewKind | "split";
export type LinkShow = "picked" | "all";

/** The box being drawn: its size and which edges snapped to a neighbor's edge. */
export type DraftInfo = { w: number; h: number; z: number; snapped: boolean } | null;

type FixView = {
  /** the page's mode, set as soon as it changes: the views read it on a press (their own
   *  props can be a frame or two behind, and a quick D-then-drag must draw) */
  mode: Mode;
  busy: boolean;
  /** the area under the mouse (in either view) */
  hoverArea: number | null;
  /** the point under the mouse, GoldSrc units */
  hoverPoint: Vec3 | null;
  /** which view the mouse is in (the other one shows a marker there) */
  hoverView: ViewKind | null;
  /** the mouse on the page, for the hint next to it */
  mouse: { x: number; y: number } | null;
  draft: DraftInfo;
  /** move both cameras to a point; `at` orders it against the camera memory */
  focus: { point: Vec3; size: number; at: number } | null;
  setHover: (area: number | null, point: Vec3 | null, view: ViewKind | null, mouse: { x: number; y: number } | null) => void;
  setDraft: (d: DraftInfo) => void;
  look: (point: Vec3, size?: number) => void;
  /** the 3D view flies (hold right mouse + WASD) instead of orbiting; kept on this computer */
  fly: boolean;
  setFly: (fly: boolean) => void;
};

const FLY_KEY = "ls.navfix.fly";
function savedFly(): boolean {
  try {
    return localStorage.getItem(FLY_KEY) === "1";
  } catch {
    return false;
  }
}

export const useFix = create<FixView>((set, get) => ({
  mode: "select",
  busy: false,
  hoverArea: null,
  hoverPoint: null,
  hoverView: null,
  mouse: null,
  draft: null,
  focus: null,
  setHover: (area, point, view, mouse) => {
    const s = get();
    if (s.hoverArea === area && s.hoverView === view && s.hoverPoint === point && s.mouse?.x === mouse?.x && s.mouse?.y === mouse?.y) return;
    set({ hoverArea: area, hoverPoint: point, hoverView: view, mouse });
  },
  setDraft: (draft) => {
    const d = get().draft;
    if (d === draft || (d && draft && d.w === draft.w && d.h === draft.h && d.snapped === draft.snapped && d.z === draft.z)) return;
    set({ draft });
  },
  look: (point, size = 96) => set({ focus: { point, size, at: performance.now() } }),
  fly: savedFly(),
  setFly: (fly) => {
    try {
      localStorage.setItem(FLY_KEY, fly ? "1" : "0");
    } catch {
      /* private window */
    }
    set({ fly });
  },
}));

/** Where the camera was last put (by you or a focus), in three's units (y up): the point it
 *  looks at, how far back it is (top-down: the same amount of map in view), and the 3D
 *  view's direction. One per map. */
export const viewMemory: { map: string | null; target: THREE.Vector3 | null; dist: number; dir: THREE.Vector3 | null; at: number; from: ViewKind | null } = {
  map: null,
  target: null,
  dist: 0,
  dir: null,
  at: 0,
  from: null,
};

/** `touch`: false when a view only says where it opened (so a focus waiting for the other
 *  view in split still counts as newer) */
export function remember(map: string, from: ViewKind, target: THREE.Vector3, dist: number, dir?: THREE.Vector3, touch = true) {
  const fresh = viewMemory.map !== map;
  if (fresh) viewMemory.dir = null;
  viewMemory.map = map;
  viewMemory.target = target.clone();
  viewMemory.dist = dist;
  if (dir) viewMemory.dir = dir.clone().normalize();
  if (touch) viewMemory.at = performance.now();
  else if (fresh) viewMemory.at = 0;
  viewMemory.from = from;
}

const KEY = "ls.navfix.layout";
export function savedLayout(): Layout {
  try {
    const v = localStorage.getItem(KEY);
    return v === "top" || v === "split" ? v : "orbit";
  } catch {
    return "orbit";
  }
}
export function saveLayout(l: Layout) {
  try {
    localStorage.setItem(KEY, l);
  } catch {
    /* private window */
  }
}
const LINKS = "ls.navfix.links";
export function savedLinks(): LinkShow {
  try {
    return localStorage.getItem(LINKS) === "all" ? "all" : "picked";
  } catch {
    return "picked";
  }
}
export function saveLinks(l: LinkShow) {
  try {
    localStorage.setItem(LINKS, l);
  } catch {
    /* private window */
  }
}

// the UX test (tests/navfix-ux.spec.ts) reads where the cameras are and what's hovered
if (import.meta.env.DEV) (window as unknown as { __lsNavfix?: unknown }).__lsNavfix = { useFix, viewMemory };
