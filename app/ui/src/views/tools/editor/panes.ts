// P5: the editor's view layout. One canvas, several panes (drei <View> viewports): Single (one
// view), Quad (3D + Top + Front + Side, like Hammer), 1+3 (a big view and three small ones)
// and Two (side by side). Each pane has its own camera and shows any of the four views.
// The editor store's `view` is the active pane's view, so everything that reads or sets
// `view` (the view buttons, Shift+1-4) works on the pane you last used.

import { createContext, useContext } from "react";
import { create } from "zustand";
import { useEditor, type ViewId } from "./store";

export type LayoutId = "single" | "quad" | "one3" | "two";
export const LAYOUTS: { id: LayoutId; label: string; tip: string }[] = [
  { id: "single", label: "Single", tip: "One view" },
  { id: "quad", label: "Quad", tip: "3D, Top, Front and Side at once, like Hammer" },
  { id: "one3", label: "1+3", tip: "One big view and three small ones" },
  { id: "two", label: "Two", tip: "Two views side by side" },
];

/** How many panes a layout shows. */
export const paneCount = (l: LayoutId) => (l === "single" ? 1 : l === "two" ? 2 : 4);

/** Where the splitters are (0-1 of the width/height). y2: the second split in 1+3's small column. */
export type Split = { x: number; y: number; y2: number };

type PaneState = {
  layout: LayoutId;
  /** what each pane slot shows */
  views: ViewId[];
  /** splitter places, per layout */
  splits: Record<LayoutId, Split>;
  /** the pane you last clicked or hovered with a key press */
  active: number;
  /** a pane blown up to fill the space, for now */
  max: number | null;
  /** the pane under the mouse (for maximise by key) */
  hover: number | null;
  /** zoom flat panes to a box: [min, max] and a counter so the same box can be asked again */
  frame: { min: [number, number, number]; max: [number, number, number]; seq: number } | null;
  set: (p: Partial<Omit<PaneState, "set">>) => void;
};

const KEY = "ls.editor.panes";
const DEFAULT_VIEWS: ViewId[] = ["3d", "top", "front", "side"];

const DEFAULT_SPLITS: Record<LayoutId, Split> = {
  single: { x: 0.5, y: 0.5, y2: 0.667 },
  quad: { x: 0.5, y: 0.5, y2: 0.667 },
  one3: { x: 0.7, y: 0.333, y2: 0.667 },
  two: { x: 0.5, y: 0.5, y2: 0.667 },
};

function load(): Pick<PaneState, "layout" | "views" | "splits"> {
  const d = { layout: "single" as LayoutId, views: [...DEFAULT_VIEWS], splits: { ...DEFAULT_SPLITS } };
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (!v || typeof v !== "object") return d;
    const layout = LAYOUTS.some((l) => l.id === v.layout) ? (v.layout as LayoutId) : d.layout;
    const views = Array.isArray(v.views) && v.views.length === 4 && v.views.every((x: unknown) => DEFAULT_VIEWS.includes(x as ViewId)) ? (v.views as ViewId[]) : d.views;
    const clamp = (n: unknown, f: number) => (typeof n === "number" && n >= 0.1 && n <= 0.9 ? n : f);
    const splits = { ...DEFAULT_SPLITS };
    for (const l of LAYOUTS) {
      const o = v.splits?.[l.id];
      const f = DEFAULT_SPLITS[l.id];
      splits[l.id] = { x: clamp(o?.x, f.x), y: clamp(o?.y, f.y), y2: clamp(o?.y2, f.y2) };
    }
    return { layout, views, splits };
  } catch {
    return d;
  }
}

export const usePanes = create<PaneState>((set, get) => ({
  ...load(),
  active: 0,
  max: null,
  hover: null,
  frame: null,
  set: (p) => {
    set(p);
    const s = get();
    if ("layout" in p || "views" in p || "splits" in p) {
      try {
        localStorage.setItem(KEY, JSON.stringify({ layout: s.layout, views: s.views, splits: s.splits }));
      } catch {
        /* private window */
      }
    }
    // a smaller layout: the active pane must still be there
    if (s.active >= paneCount(s.layout)) set({ active: 0 });
    if (s.max != null && s.max >= paneCount(s.layout)) set({ max: null });
    syncView();
  },
}));

/** The editor's `view` follows the active pane, and setting `view` changes the active pane. */
function syncView() {
  const p = usePanes.getState();
  const v = p.views[p.active];
  if (useEditor.getState().view !== v) useEditor.setState({ view: v });
}
let lastView = useEditor.getState().view;
useEditor.subscribe((s) => {
  if (s.view === lastView) return;
  lastView = s.view;
  const p = usePanes.getState();
  if (p.views[p.active] !== s.view) {
    const views = [...p.views];
    views[p.active] = s.view;
    p.set({ views });
  }
});
// on load, the active pane's view wins
queueMicrotask(syncView);

export function setLayout(layout: LayoutId) {
  usePanes.getState().set({ layout, max: null });
}

/** Blow the pane under the mouse (else the active one) up to fill the space, or put it back. */
export function toggleMaximise(slot?: number) {
  const p = usePanes.getState();
  if (p.layout === "single") return;
  if (p.max != null) return p.set({ max: null });
  const n = slot ?? p.hover ?? p.active;
  p.set({ max: n, active: n });
}

/** Zoom every flat pane to a box (the 3D pane is moved with the viewer's look-at). */
export function frameFlat(min: [number, number, number], max: [number, number, number]) {
  const p = usePanes.getState();
  p.set({ frame: { min, max, seq: (p.frame?.seq ?? 0) + 1 } });
}

/** Inside a pane's part of the scene: which slot and view it is, and whether it's the active one. */
export type PaneInfo = { slot: number; view: ViewId; active: boolean };
export const PaneCtx = createContext<PaneInfo | null>(null);
/** The view of the pane this is drawn in (the editor's view outside any pane). */
export function usePaneView(): ViewId {
  const p = useContext(PaneCtx);
  const v = useEditor((s) => s.view);
  return p?.view ?? v;
}
export function usePaneActive(): boolean {
  return useContext(PaneCtx)?.active ?? true;
}
