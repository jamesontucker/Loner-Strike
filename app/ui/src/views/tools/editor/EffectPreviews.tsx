// Engine effects drawn where they'll be in the game, in every view (3D and flat): an env_beam
// as a solid line in its color from its start (or itself) to its end, or a ring of its radius
// when it strikes random spots; an env_laser from itself to what it points at, with a dot where
// its end sprite goes; gibshooter, env_shooter and env_blood as an arrow the way they throw
// (longer the faster). Not pickable: the entity's own box is.

import { useEffect, useMemo } from "react";
import * as THREE from "three";
import type { Vec3 } from "../viewer/coords";
import type { DocEntity } from "./editorApi";
import { keyOf, originOf, useEditor, type BrushGeo } from "./store";

const MATERIAL = new THREE.LineBasicMaterial({ vertexColors: true, transparent: true, opacity: 0.95, depthTest: true });
const SHOOTERS = new Set(["gibshooter", "env_shooter", "env_blood"]);

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

/** "r g b" (0-255) as 0-1, white when it isn't three numbers or is black. */
export function colorOf(text: string | undefined, fallback: Vec3 = [1, 1, 1]): Vec3 {
  const p = (text ?? "").trim().split(/\s+/).slice(0, 3).map(Number);
  if (p.length < 3 || !p.every(Number.isFinite) || p.every((x) => x <= 0)) return fallback;
  return p.map((x) => Math.max(0, Math.min(255, x)) / 255) as Vec3;
}

/** The way a shooter throws: its angles (pitch yaw roll; pitch up is negative) as a unit vector. */
export function throwDir(e: DocEntity): Vec3 {
  const a = (keyOf(e.keys, "angles") ?? "").trim().split(/\s+/).map(Number);
  let pitch = Number.isFinite(a[0]) ? a[0] : 0;
  let yaw = Number.isFinite(a[1]) ? a[1] : 0;
  const angle = Number(keyOf(e.keys, "angle"));
  if (!keyOf(e.keys, "angles") && Number.isFinite(angle)) {
    // the old single angle: -1 up, -2 down
    if (angle === -1) pitch = -90;
    else if (angle === -2) pitch = 90;
    else yaw = angle;
  }
  const p = (pitch * Math.PI) / 180;
  const y = (yaw * Math.PI) / 180;
  return [Math.cos(p) * Math.cos(y), Math.cos(p) * Math.sin(y), -Math.sin(p)];
}

type Seg = { a: Vec3; b: Vec3; c: Vec3 };

/** The lines every effect entity draws (exported for tests). */
export function effectSegments(ents: DocEntity[], brushes: Map<number, BrushGeo>): Seg[] {
  const byName = new Map<string, DocEntity[]>();
  for (const e of ents) {
    const n = keyOf(e.keys, "targetname")?.trim();
    if (n) byName.set(n, [...(byName.get(n) ?? []), e]);
  }
  const at = (name: string | undefined) => {
    const n = name?.trim();
    return n ? (byName.get(n) ?? []).map((e) => spot(e, brushes)).filter((p): p is Vec3 => !!p) : [];
  };
  const out: Seg[] = [];
  const ring = (centre: Vec3, r: number, c: Vec3) => {
    const n = 32;
    for (let i = 0; i < n; i++) {
      const t0 = (i / n) * Math.PI * 2;
      const t1 = ((i + 1) / n) * Math.PI * 2;
      out.push({ a: [centre[0] + Math.cos(t0) * r, centre[1] + Math.sin(t0) * r, centre[2]], b: [centre[0] + Math.cos(t1) * r, centre[1] + Math.sin(t1) * r, centre[2]], c });
    }
  };
  const dot = (p: Vec3, c: Vec3) => {
    const s = 6;
    for (let i = 0; i < 3; i++) {
      const a = [...p] as Vec3;
      const b = [...p] as Vec3;
      a[i] -= s;
      b[i] += s;
      out.push({ a, b, c });
    }
  };
  for (const e of ents) {
    if (out.length > 4000) break;
    const self = spot(e, brushes);
    if (!self) continue;
    const c = colorOf(keyOf(e.keys, "rendercolor"), e.classname === "env_laser" ? [1, 0, 0] : [1, 1, 1]);
    if (e.classname === "env_beam") {
      const starts = keyOf(e.keys, "LightningStart")?.trim() ? at(keyOf(e.keys, "LightningStart")) : [self];
      const ends = at(keyOf(e.keys, "LightningEnd"));
      for (const s of starts) {
        if (ends.length) for (const t of ends) out.push({ a: s, b: t, c });
        else ring(s, Math.max(8, Math.min(4096, Number(keyOf(e.keys, "Radius") ?? 256) || 256)), c);
      }
    } else if (e.classname === "env_laser") {
      const ends = at(keyOf(e.keys, "LaserTarget"));
      for (const t of ends) {
        out.push({ a: self, b: t, c });
        if (keyOf(e.keys, "EndSprite")?.trim()) dot(t, c);
      }
    } else if (SHOOTERS.has(e.classname)) {
      // env_blood's random direction has no arrow
      if (e.classname === "env_blood" && (Number(keyOf(e.keys, "spawnflags") ?? 0) & 1)) continue;
      const d = throwDir(e);
      const speed = Number(keyOf(e.keys, "m_flVelocity") ?? 200) || 200;
      const len = e.classname === "env_blood" ? 48 : Math.max(32, Math.min(256, speed / 3));
      const tip: Vec3 = [self[0] + d[0] * len, self[1] + d[1] * len, self[2] + d[2] * len];
      const col: Vec3 = e.classname === "env_blood" ? [0.9, 0.1, 0.1] : [0.85, 0.55, 0.25];
      out.push({ a: self, b: tip, c: col });
      // the head: two short strokes back from the tip, off to the sides
      const side: Vec3 = Math.abs(d[2]) > 0.9 ? [1, 0, 0] : [-d[1], d[0], 0];
      const sl = Math.hypot(...side) || 1;
      for (const k of [1, -1]) {
        const back = len * 0.2;
        out.push({ a: tip, b: [tip[0] - d[0] * back + (side[0] / sl) * back * 0.6 * k, tip[1] - d[1] * back + (side[1] / sl) * back * 0.6 * k, tip[2] - d[2] * back + (side[2] / sl) * back * 0.6 * k], c: col });
      }
    }
  }
  return out;
}

export function EffectPreviews() {
  const ents = useEditor((s) => s.doc?.entities);
  const brushes = useEditor((s) => s.brushes);
  const hidden = useEditor((s) => s.hidden.entities);
  const obj = useMemo(() => {
    const segs = effectSegments((ents ?? []).filter((e) => !hidden.has(e.id)), brushes);
    if (!segs.length) return null;
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(segs.flatMap((s) => [...s.a, ...s.b]), 3));
    g.setAttribute("color", new THREE.Float32BufferAttribute(segs.flatMap((s) => [...s.c, ...s.c]), 3));
    const l = new THREE.LineSegments(g, MATERIAL);
    l.raycast = () => null;
    l.renderOrder = 3;
    return l;
  }, [ents, brushes, hidden]);
  useEffect(() => () => obj?.geometry.dispose(), [obj]);
  return obj ? <primitive object={obj} /> : null;
}
