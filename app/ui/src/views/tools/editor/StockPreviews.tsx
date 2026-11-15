// The stock entities' shapes drawn where they act in the game (EngineCoverage item 9), in every
// view: a train's track as solid lines from each path_track (and path_corner) to its next stop,
// with an arrow showing the way, and branches in another color; a func_tracktrain to its first
// stop; a turret's aim as an arc (how far it turns either way from the middle of its aim) with a
// line for the middle; a trigger_setorigin to what it moves and where to. Not pickable: the
// entity's own box is. (The faint dashed lines for every link are EditorScene's Links.)

import { useEffect, useMemo } from "react";
import * as THREE from "three";
import type { Vec3 } from "../viewer/coords";
import type { DocEntity } from "./editorApi";
import { keyOf, originOf, useEditor, type BrushGeo } from "./store";

const MATERIAL = new THREE.LineBasicMaterial({ vertexColors: true, transparent: true, opacity: 0.9, depthTest: true });
const TRACK: Vec3 = [1, 0.55, 0.1];
const BRANCH: Vec3 = [0.35, 0.85, 1];
const AIM: Vec3 = [1, 0.25, 0.25];
const MOVE: Vec3 = [0.75, 0.5, 1];
const PATHS = new Set(["path_track", "path_corner"]);
const TANKS = new Set(["func_tank", "func_tankmortar", "func_tanklaser"]);

type Seg = { a: Vec3; b: Vec3; c: Vec3 };

/** An entity's spot: its origin, or the middle of its brushes. */
function spot(e: DocEntity, brushes: Map<number, BrushGeo>): Vec3 | null {
  if (!e.brushes.length) return originOf(e.keys);
  const min: Vec3 = [Infinity, Infinity, Infinity];
  const max: Vec3 = [-Infinity, -Infinity, -Infinity];
  for (const id of e.brushes) {
    const b = brushes.get(id);
    if (!b) continue;
    for (let i = 0; i < 3; i++) {
      min[i] = Math.min(min[i], b.min[i]);
      max[i] = Math.max(max[i], b.max[i]);
    }
  }
  return Number.isFinite(min[0]) ? (min.map((x, i) => (x + max[i]) / 2) as Vec3) : null;
}

const num = (e: DocEntity, k: string) => {
  const n = Number(keyOf(e.keys, k));
  return Number.isFinite(n) ? n : 0;
};

/** The lines the stock entities draw (exported for tests). */
export function stockSegments(ents: DocEntity[], brushes: Map<number, BrushGeo>): Seg[] {
  const byName = new Map<string, DocEntity[]>();
  for (const e of ents) {
    const n = keyOf(e.keys, "targetname")?.trim();
    if (n) byName.set(n, [...(byName.get(n) ?? []), e]);
  }
  const named = (name: string | undefined, ok: (c: string) => boolean = () => true) => {
    const n = name?.trim();
    return n ? (byName.get(n) ?? []).filter((e) => ok(e.classname)) : [];
  };
  const out: Seg[] = [];
  // a line with an arrowhead halfway along, pointing from a to b
  const arrow = (a: Vec3, b: Vec3, c: Vec3) => {
    out.push({ a, b, c });
    const d: Vec3 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    const len = Math.hypot(...d);
    if (len < 1) return;
    const u = d.map((x) => x / len) as Vec3;
    const mid: Vec3 = [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];
    const side: Vec3 = Math.abs(u[2]) > 0.9 ? [1, 0, 0] : [-u[1], u[0], 0];
    const sl = Math.hypot(...side) || 1;
    const h = Math.min(16, len / 4);
    for (const k of [1, -1]) out.push({ a: mid, b: [mid[0] - u[0] * h + (side[0] / sl) * h * 0.6 * k, mid[1] - u[1] * h + (side[1] / sl) * h * 0.6 * k, mid[2] - u[2] * h + (side[2] / sl) * h * 0.6 * k], c });
  };
  for (const e of ents) {
    if (out.length > 6000) break;
    const self = spot(e, brushes);
    if (!self) continue;
    if (PATHS.has(e.classname)) {
      // the next stop of the same kind (a path_corner's next is a path_corner)
      for (const t of named(keyOf(e.keys, "target"), (c) => c === e.classname)) {
        const p = spot(t, brushes);
        if (p) arrow(self, p, TRACK);
      }
      if (e.classname === "path_track")
        for (const t of named(keyOf(e.keys, "altpath"), (c) => c === "path_track")) {
          const p = spot(t, brushes);
          if (p) arrow(self, p, BRANCH);
        }
    } else if (e.classname === "func_tracktrain" || e.classname === "func_train" || e.classname === "func_guntarget") {
      for (const t of named(keyOf(e.keys, "target"), (c) => PATHS.has(c))) {
        const p = spot(t, brushes);
        if (p) out.push({ a: self, b: p, c: TRACK });
      }
    } else if (TANKS.has(e.classname)) {
      // its aim: the middle (its yaw) and how far it turns either way, as an arc on the floor of its pivot
      const ang = (keyOf(e.keys, "angles") ?? "").trim().split(/\s+/).map(Number);
      const yaw = Number.isFinite(ang[1]) ? ang[1] : 0;
      const range = Math.max(0, Math.min(180, num(e, "yawrange")));
      const maxR = num(e, "maxRange");
      const r = maxR > 0 ? Math.min(maxR, 512) : 192;
      const at = (deg: number): Vec3 => {
        const t = (deg * Math.PI) / 180;
        return [self[0] + Math.cos(t) * r, self[1] + Math.sin(t) * r, self[2]];
      };
      arrow(self, at(yaw), AIM);
      if (range > 0) {
        const n = Math.max(4, Math.round(range / 7.5));
        for (let i = 0; i < n * 2; i++) out.push({ a: at(yaw - range + (i * range) / n), b: at(yaw - range + ((i + 1) * range) / n), c: AIM });
        if (range < 180) {
          out.push({ a: self, b: at(yaw - range), c: AIM });
          out.push({ a: self, b: at(yaw + range), c: AIM });
        }
      }
    } else if (e.classname === "trigger_setorigin") {
      const to = named(keyOf(e.keys, "copypointer"))
        .map((t) => spot(t, brushes))
        .filter((p): p is Vec3 => !!p);
      for (const t of named(keyOf(e.keys, "target"))) {
        const p = spot(t, brushes);
        if (!p) continue;
        out.push({ a: self, b: p, c: MOVE });
        for (const q of to) arrow(p, q, MOVE);
      }
    }
  }
  return out;
}

export function StockPreviews() {
  const ents = useEditor((s) => s.doc?.entities);
  const brushes = useEditor((s) => s.brushes);
  const hidden = useEditor((s) => s.hidden.entities);
  const obj = useMemo(() => {
    const segs = stockSegments((ents ?? []).filter((e) => !hidden.has(e.id)), brushes);
    if (!segs.length) return null;
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(segs.flatMap((s) => [...s.a, ...s.b]), 3));
    g.setAttribute("color", new THREE.Float32BufferAttribute(segs.flatMap((s) => [...s.c, ...s.c]), 3));
    const l = new THREE.LineSegments(g, MATERIAL);
    l.raycast = () => null;
    l.renderOrder = 3;
    l.userData = { kind: "stock-previews", segments: segs.length };
    return l;
  }, [ents, brushes, hidden]);
  useEffect(() => () => obj?.geometry.dispose(), [obj]);
  return obj ? <primitive object={obj} /> : null;
}
