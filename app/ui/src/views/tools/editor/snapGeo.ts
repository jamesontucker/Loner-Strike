// P5: snapping to brush corners and edges (Grid and snapping > "Snap to brush corners and
// edges"), and Shift held for "drag the gizmo to copy". Used by EditorScene's snapVec/gizmo.

import type { Vec3 } from "../viewer/coords";
import { useP5 } from "./p5store";
import { isHiddenBrush, useEditor } from "./store";

let shift = false;
if (typeof window !== "undefined") {
  window.addEventListener("keydown", (e) => (shift = e.shiftKey), true);
  window.addEventListener("keyup", (e) => (shift = e.shiftKey), true);
  window.addEventListener("pointerdown", (e) => (shift = e.shiftKey), true);
  window.addEventListener("blur", () => (shift = false));
}
/** Shift is down now (gizmo: copy while dragging). */
export const shiftHeld = () => shift;

const d2 = (a: Vec3, b: Vec3) => (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 + (a[2] - b[2]) ** 2;

/** The nearest point on segment ab to p. */
function onSegment(p: Vec3, a: Vec3, b: Vec3): Vec3 {
  const ab: Vec3 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
  const len = ab[0] ** 2 + ab[1] ** 2 + ab[2] ** 2;
  if (len < 1e-9) return a;
  const t = Math.max(0, Math.min(1, ((p[0] - a[0]) * ab[0] + (p[1] - a[1]) * ab[1] + (p[2] - a[2]) * ab[2]) / len));
  return [a[0] + ab[0] * t, a[1] + ab[1] * t, a[2] + ab[2] * t];
}

/** With snapping to geometry on: the nearest brush corner within reach, else the nearest
 *  point on an edge (rounded to whole units); null when nothing is near or it's off.
 *  Alt (snapping off) turns it off too. */
export function snapToGeometry(p: Vec3): Vec3 | null {
  if (!useP5.getState().geoSnap) return null;
  const s = useEditor.getState();
  if (s.alt) return null;
  const reach = Math.max(4, s.grid * 0.75);
  const r2 = reach * reach;
  let best: Vec3 | null = null;
  let bestD = r2;
  let edge: Vec3 | null = null;
  let edgeD = r2;
  for (const b of s.brushes.values()) {
    if (isHiddenBrush(b, s)) continue;
    // quick reject on the brush's box
    if (p[0] < b.min[0] - reach || p[0] > b.max[0] + reach || p[1] < b.min[1] - reach || p[1] > b.max[1] + reach || p[2] < b.min[2] - reach || p[2] > b.max[2] + reach) continue;
    for (const poly of b.polys.values()) {
      for (let i = 0; i < poly.length; i++) {
        const a = poly[i];
        const dc = d2(p, a);
        if (dc < bestD) {
          bestD = dc;
          best = a;
        }
        const q = onSegment(p, a, poly[(i + 1) % poly.length]);
        const de = d2(p, q);
        if (de < edgeD) {
          edgeD = de;
          edge = q;
        }
      }
    }
  }
  if (best) return [...best] as Vec3;
  return edge ? (edge.map((x) => Math.round(x)) as Vec3) : null;
}
