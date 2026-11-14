// Parity A: the Shape tool's settings beyond the editor store's (sides, arc, wall) and P5's
// start angle: a cone's top size, a torus's tube sides, and add height (arch and torus pieces
// climb: spiral stairs, ramps, slides). Kept between sessions. `currentShape()` gathers every
// setting for the preview and for the add_shape command, so both always agree.

import { create } from "zustand";
import { useP5 } from "./p5store";
import type { ShapeParams } from "./shapePreview";
import { useEditor } from "./store";

type State = {
  /** a cone's top as a share of its base, 0-0.95 (0: a point) */
  coneTop: number;
  /** a torus's tube sides, 3-32 */
  tubeSides: number;
  /** how much higher each arch or torus piece sits than the last */
  addHeight: number;
  set: (p: Partial<Omit<State, "set">>) => void;
};

const KEY = "ls.editor.shapeParams";

function load(): Omit<State, "set"> {
  const d = { coneTop: 0, tubeSides: 8, addHeight: 0 };
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (!v || typeof v !== "object") return d;
    const n = (x: unknown, lo: number, hi: number, f: number) => (typeof x === "number" && Number.isFinite(x) ? Math.max(lo, Math.min(hi, x)) : f);
    return { coneTop: n(v.coneTop, 0, 0.95, 0), tubeSides: Math.round(n(v.tubeSides, 3, 32, 8)), addHeight: n(v.addHeight, -4096, 4096, 0) };
  } catch {
    return d;
  }
}

export const useShapeParams = create<State>((set, get) => ({
  ...load(),
  set: (p) => {
    set(p);
    const { coneTop, tubeSides, addHeight } = get();
    try {
      localStorage.setItem(KEY, JSON.stringify({ coneTop, tubeSides, addHeight }));
    } catch {
      /* private window */
    }
  },
}));

/** Every setting of the shape being drawn (for the preview). */
export function currentShape(): ShapeParams {
  const s = useEditor.getState();
  const x = useShapeParams.getState();
  return { shape: s.shape, sides: s.sides, arc: s.archArc, wall: s.archWall, start: useP5.getState().archStart, top: x.coneTop, tubeSides: x.tubeSides, addHeight: x.addHeight };
}

/** The same, as a hook: the preview redraws when any of them changes. */
export function useCurrentShape(): ShapeParams {
  const shape = useEditor((s) => s.shape);
  const sides = useEditor((s) => s.sides);
  const arc = useEditor((s) => s.archArc);
  const wall = useEditor((s) => s.archWall);
  const start = useP5((s) => s.archStart);
  const top = useShapeParams((s) => s.coneTop);
  const tubeSides = useShapeParams((s) => s.tubeSides);
  const addHeight = useShapeParams((s) => s.addHeight);
  return { shape, sides, arc, wall, start, top, tubeSides, addHeight };
}

/** The add_shape command's settings (beside min, max, texture and grid). */
export function shapeCommand(): Record<string, unknown> {
  const p = currentShape();
  return { shape: p.shape, sides: p.sides, arc: p.arc, wall: p.wall, start: p.start, top: p.top, tube_sides: p.tubeSides, add_height: p.addHeight };
}
