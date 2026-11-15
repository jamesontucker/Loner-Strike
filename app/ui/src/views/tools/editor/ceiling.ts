// Y: entities that hang from a ceiling (ls_barnacle). Placing one by click or drag traces
// straight up from the point to the nearest ceiling and attaches it there, as the game does at
// spawn (creature.cpp CLsBarnacle::Spawn); none above (open sky) leaves it where it was dropped.

import type { Vec3 } from "../viewer/coords";
import { useEditor, type BrushGeo } from "./store";

/** Classes that hang from the ceiling, with their box under the hanging point (the FGD's size). */
export const HANGS: Record<string, { min: Vec3; max: Vec3 }> = {
  ls_barnacle: { min: [-16, -16, -32], max: [16, 16, 0] },
};

const REACH = 4096;
const NOT_SOLID = /^(aaatrigger|clip|origin|hint|skip|null)$/i;

/** Where a ray straight up from `p` meets a brush: its z and whether that face is sky (null: no brush). */
function hitUp(b: BrushGeo, p: Vec3): { z: number; sky: boolean } | null {
  if (p[0] < b.min[0] || p[0] > b.max[0] || p[1] < b.min[1] || p[1] > b.max[1] || b.max[2] <= p[2]) return null;
  let enter = -Infinity;
  let exit = Infinity;
  let enterFace = -1;
  for (const [face, pts] of b.polys) {
    if (pts.length < 3) continue;
    // the face's outward plane (Newell's normal, flipped to point away from the brush's middle)
    let n: Vec3 = [0, 0, 0];
    const c: Vec3 = [0, 0, 0];
    for (let k = 0; k < pts.length; k++) {
      const a = pts[k];
      const q = pts[(k + 1) % pts.length];
      n = [n[0] + (a[1] - q[1]) * (a[2] + q[2]), n[1] + (a[2] - q[2]) * (a[0] + q[0]), n[2] + (a[0] - q[0]) * (a[1] + q[1])];
      for (let i = 0; i < 3; i++) c[i] += a[i] / pts.length;
    }
    const len = Math.hypot(...n);
    if (len < 1e-9) continue;
    n = [n[0] / len, n[1] / len, n[2] / len];
    const mid: Vec3 = [(b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2];
    let d = n[0] * c[0] + n[1] * c[1] + n[2] * c[2];
    if (n[0] * mid[0] + n[1] * mid[1] + n[2] * mid[2] > d) {
      n = [-n[0], -n[1], -n[2]];
      d = -d;
    }
    if (Math.abs(n[2]) < 1e-6) {
      if (n[0] * p[0] + n[1] * p[1] > d + 0.01) return null; // outside a side
      continue;
    }
    const z = (d - n[0] * p[0] - n[1] * p[1]) / n[2];
    if (n[2] < 0) {
      if (z > enter) [enter, enterFace] = [z, face];
    } else exit = Math.min(exit, z);
  }
  if (!(enter < exit) || enter < p[2] - 0.01 || enterFace < 0) return null;
  const tex = b.faces.find((f) => f.face === enterFace)?.texture ?? "";
  return { z: enter, sky: /^sky$/i.test(tex) };
}

/** The ceiling straight above a point (null: none within reach, or open sky). */
export function ceilingAbove(p: Vec3): Vec3 | null {
  let best: { z: number; sky: boolean } | null = null;
  for (const b of useEditor.getState().brushes.values()) {
    if (b.faces.length && b.faces.every((f) => NOT_SOLID.test(f.texture))) continue;
    const h = hitUp(b, p);
    if (h && h.z - p[2] <= REACH && (!best || h.z < best.z)) best = h;
  }
  return best && !best.sky ? [p[0], p[1], Math.round(best.z)] : null;
}

/** For a hanging class placed at a surface point: its origin on the ceiling above, and the box
 *  it takes there (the ghost). Null when the class doesn't hang or there's no ceiling. */
export function hangPlacement(cls: string | undefined, point: Vec3, normal: Vec3): { origin: Vec3; min: Vec3; max: Vec3 } | null {
  const box = cls ? HANGS[cls] : undefined;
  if (!box) return null;
  // a little off the surface it was dropped on, so a click on the ceiling itself finds it
  const from: Vec3 = [point[0] + normal[0] * 2, point[1] + normal[1] * 2, point[2] + normal[2] * 2];
  const o = ceilingAbove(from);
  if (!o) return null;
  return { origin: o, min: o.map((v, i) => v + box.min[i]) as Vec3, max: o.map((v, i) => v + box.max[i]) as Vec3 };
}
