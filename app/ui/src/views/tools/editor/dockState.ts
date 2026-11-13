// P5: where the editor's panels live. Each panel is in one place: a dock area (left, right,
// right-lower, bottom: stacked as tabs), a floating window inside the editor, a window of its
// own (popped out), or closed. Sizes and places are kept on this computer; three ready-made
// layouts (Default, Hammer, Unreal-like) and Reset.

import { create } from "zustand";
import { useP5 } from "./p5store";

export type AreaId = "left" | "right" | "right2" | "bottom";
export const AREAS: { id: AreaId; label: string }[] = [
  { id: "left", label: "Left" },
  { id: "right", label: "Right" },
  { id: "right2", label: "Right, lower" },
  { id: "bottom", label: "Bottom" },
];
export type Float = { id: string; x: number; y: number; w: number; h: number };
type Area = { tabs: string[]; active: string | null };
export type DockLayout = {
  areas: Record<AreaId, Area>;
  sizes: { left: number; right: number; bottom: number; rightSplit: number };
  floats: Float[];
  popped: string[];
};
export type DockPresetId = "default" | "hammer" | "unreal";

const area = (tabs: string[], active = tabs[0] ?? null): Area => ({ tabs, active });
const SIZES = { left: 300, right: 320, bottom: 240, rightSplit: 0.45 };
export const DOCK_PRESETS: Record<DockPresetId, { label: string; tip: string; layout: DockLayout }> = {
  default: {
    label: "Default",
    tip: "Everything on the right as tabs",
    layout: { areas: { left: area([]), right: area(["inspect", "outline", "textures", "library", "checks", "compile", "project", "history"]), right2: area([]), bottom: area([]) }, sizes: { ...SIZES }, floats: [], popped: [] },
  },
  hammer: {
    label: "Hammer",
    tip: "Properties and textures on the right, checks and compiling below, with the four views",
    layout: { areas: { left: area([]), right: area(["inspect", "textures", "library"]), right2: area(["outline", "history"]), bottom: area(["checks", "compile", "project"]) }, sizes: { ...SIZES, bottom: 200 }, floats: [], popped: [] },
  },
  unreal: {
    label: "Unreal-like",
    tip: "Outliner above details on the right, a content drawer (textures, library, project) below",
    layout: { areas: { left: area([]), right: area(["outline"]), right2: area(["inspect", "history"]), bottom: area(["textures", "library", "project", "checks", "compile"]) }, sizes: { ...SIZES, right: 340, bottom: 260, rightSplit: 0.42 }, floats: [], popped: [] },
  },
};

type DockState = DockLayout & {
  /** a tab is being dragged (the drop zones show) */
  dragging: string | null;
  set: (p: Partial<DockLayout & { dragging: string | null }>) => void;
};

const KEY = "ls.editor.dock";
const clone = (l: DockLayout): DockLayout => JSON.parse(JSON.stringify(l));

function load(): DockLayout {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null") as DockLayout | null;
    if (!v || typeof v !== "object" || !v.areas) return clone(DOCK_PRESETS.default.layout);
    const out = clone(DOCK_PRESETS.default.layout);
    for (const a of AREAS) {
      const x = v.areas[a.id];
      if (x && Array.isArray(x.tabs)) out.areas[a.id] = { tabs: x.tabs.filter((t) => typeof t === "string"), active: typeof x.active === "string" ? x.active : null };
    }
    const n = (x: unknown, lo: number, hi: number, f: number) => (typeof x === "number" && x >= lo && x <= hi ? x : f);
    out.sizes = { left: n(v.sizes?.left, 180, 900, SIZES.left), right: n(v.sizes?.right, 200, 900, SIZES.right), bottom: n(v.sizes?.bottom, 100, 800, SIZES.bottom), rightSplit: n(v.sizes?.rightSplit, 0.15, 0.85, SIZES.rightSplit) };
    out.floats = Array.isArray(v.floats) ? v.floats.filter((f) => f && typeof f.id === "string").map((f) => ({ id: f.id, x: n(f.x, 0, 4000, 80), y: n(f.y, 0, 3000, 80), w: n(f.w, 200, 2000, 360), h: n(f.h, 150, 2000, 480) })) : [];
    // windows don't survive a reload: those panels come back as floats
    for (const id of Array.isArray(v.popped) ? v.popped : []) if (typeof id === "string" && !out.floats.some((f) => f.id === id)) out.floats.push({ id, x: 120, y: 120, w: 380, h: 520 });
    out.popped = [];
    return out;
  } catch {
    return clone(DOCK_PRESETS.default.layout);
  }
}

export const useDock = create<DockState>((set, get) => ({
  ...load(),
  dragging: null,
  set: (p) => {
    set(p);
    if (Object.keys(p).some((k) => k !== "dragging")) {
      const s = get();
      try {
        localStorage.setItem(KEY, JSON.stringify({ areas: s.areas, sizes: s.sizes, floats: s.floats, popped: s.popped }));
      } catch {
        /* private window */
      }
    }
  },
}));

/** Takes a panel out of wherever it is (areas, floats, windows). */
function without(l: DockLayout, id: string): DockLayout {
  const areas = { ...l.areas };
  for (const a of AREAS) {
    const x = areas[a.id];
    if (!x.tabs.includes(id)) continue;
    const tabs = x.tabs.filter((t) => t !== id);
    areas[a.id] = { tabs, active: x.active === id ? tabs[Math.max(0, x.tabs.indexOf(id) - 1)] ?? null : x.active };
  }
  return { ...l, areas, floats: l.floats.filter((f) => f.id !== id), popped: l.popped.filter((p) => p !== id) };
}

export function whereIs(id: string, l: DockLayout = useDock.getState()): AreaId | "float" | "window" | null {
  for (const a of AREAS) if (l.areas[a.id].tabs.includes(id)) return a.id;
  if (l.popped.includes(id)) return "window";
  if (l.floats.some((f) => f.id === id)) return "float";
  return null;
}

/** Puts a panel in an area as its open tab (before `before`, else at the end). */
export function dockTo(id: string, to: AreaId, before?: string) {
  const l = without(useDock.getState(), id);
  const tabs = [...l.areas[to].tabs];
  const i = before ? tabs.indexOf(before) : -1;
  if (i >= 0) tabs.splice(i, 0, id);
  else tabs.push(id);
  useDock.getState().set({ ...l, areas: { ...l.areas, [to]: { tabs, active: id } } });
}

export function floatPanel(id: string, x = 140, y = 120, w = 380, h = 520) {
  const l = without(useDock.getState(), id);
  const maxX = Math.max(0, window.innerWidth - 200);
  const maxY = Math.max(0, window.innerHeight - 120);
  useDock.getState().set({ ...l, floats: [...l.floats, { id, x: Math.min(maxX, Math.max(0, x)), y: Math.min(maxY, Math.max(0, y)), w, h }] });
}

export function closePanel(id: string) {
  useDock.getState().set(without(useDock.getState(), id));
}

/** Marks a panel as in its own window (Dock.tsx opens it). */
export function popOut(id: string) {
  const l = without(useDock.getState(), id);
  useDock.getState().set({ ...l, popped: [...l.popped, id] });
}

/** Shows a panel: its tab comes forward, a float comes to the front, a closed one opens on the right. */
export function showPanel(id: string) {
  const s = useDock.getState();
  const at = whereIs(id, s);
  if (at === "float") {
    const f = s.floats.find((x) => x.id === id)!;
    s.set({ floats: [...s.floats.filter((x) => x.id !== id), f] });
    return;
  }
  if (at === "window") {
    focusWindow(id);
    return;
  }
  if (at) {
    s.set({ areas: { ...s.areas, [at]: { ...s.areas[at], active: id } } });
    return;
  }
  dockTo(id, s.areas.right.tabs.length || !s.areas.left.tabs.length ? "right" : "left");
}

export function applyDockPreset(p: DockPresetId) {
  useDock.getState().set(clone(DOCK_PRESETS[p].layout));
}

/** Open windows by panel (Dock.tsx keeps it). */
export const panelWindows = new Map<string, Window>();
function focusWindow(id: string) {
  panelWindows.get(id)?.focus();
}

// Menus and old code ask for a side tab through p5store's `tab`: show that panel.
let lastTab = useP5.getState().tab;
useP5.subscribe((s) => {
  if (s.tab === lastTab) return;
  lastTab = s.tab;
  showPanel(s.tab);
});
