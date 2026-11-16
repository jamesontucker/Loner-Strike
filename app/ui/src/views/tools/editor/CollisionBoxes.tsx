// A model entity's Collision box (editor/collision.rs makes it when the map compiles), drawn as
// a wireframe in the views while the entity is picked: orange for Box, players only, red for Box,
// players and bullets. The box is the model's bounds, scaled, turned by its yaw (the box round
// the turned box) and moved to its origin, as the compile does.

import { useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import type { Vec3 } from "../viewer/coords";
import type { DocEntity } from "./editorApi";
import { loadModel } from "./EntityModels";
import { keyOf, originOf, useEditor } from "./store";
import { usePaneMode } from "./renderModes";

export const COLLISION_KEY = "ls_collision";
/** The colors of the collision kinds (the Collision drawing style uses them too). */
export const COLLISION_COLOR: Record<string, Vec3> = { box: [1, 0.55, 0.1], box_bullets: [0.95, 0.2, 0.2] };
const MATERIAL = new THREE.LineBasicMaterial({ vertexColors: true, depthTest: false, transparent: true, opacity: 0.95 });

/** The box in the map for a model whose bounds are `b`. */
export function collisionBox(e: DocEntity, b: [Vec3, Vec3]): [Vec3, Vec3] | null {
  const o = originOf(e.keys);
  if (!o) return null;
  const s = Number(keyOf(e.keys, "scale")) > 0 ? Number(keyOf(e.keys, "scale")) : 1;
  const ang = keyOf(e.keys, "angles")?.trim().split(/\s+/).map(Number);
  const yaw = ((ang && Number.isFinite(ang[1]) ? ang[1] : Number(keyOf(e.keys, "angle")) || 0) * Math.PI) / 180;
  const [c, sn] = [Math.cos(yaw), Math.sin(yaw)];
  const lo: Vec3 = [Infinity, Infinity, Infinity];
  const hi: Vec3 = [-Infinity, -Infinity, -Infinity];
  for (const x of [b[0][0], b[1][0]])
    for (const y of [b[0][1], b[1][1]])
      for (const z of [b[0][2], b[1][2]]) {
        const p: Vec3 = [x * s * c - y * s * sn, x * s * sn + y * s * c, z * s];
        for (let i = 0; i < 3; i++) {
          lo[i] = Math.min(lo[i], p[i]);
          hi[i] = Math.max(hi[i], p[i]);
        }
      }
  return [lo.map((v, i) => Math.floor(o[i] + v)) as Vec3, hi.map((v, i) => Math.ceil(o[i] + v)) as Vec3];
}

function edges([a, b]: [Vec3, Vec3]): number[] {
  const p = (x: number, y: number, z: number) => [x ? b[0] : a[0], y ? b[1] : a[1], z ? b[2] : a[2]];
  const out: number[] = [];
  const line = (u: number[], v: number[]) => out.push(...u, ...v);
  for (const z of [0, 1]) {
    line(p(0, 0, z), p(1, 0, z));
    line(p(1, 0, z), p(1, 1, z));
    line(p(1, 1, z), p(0, 1, z));
    line(p(0, 1, z), p(0, 0, z));
  }
  for (const [x, y] of [[0, 0], [1, 0], [1, 1], [0, 1]]) line(p(x, y, 0), p(x, y, 1));
  return out;
}

export function CollisionBoxes() {
  const sel = useEditor((s) => s.sel.entities);
  const doc = useEditor((s) => s.doc);
  // (the Collision drawing style shows every model's box, not just the picked ones)
  const all = usePaneMode() === "collision";
  const picked = useMemo(() => (doc?.entities ?? []).filter((e) => (all || sel.includes(e.id)) && COLLISION_COLOR[keyOf(e.keys, COLLISION_KEY) ?? ""] && /\.mdl$/i.test(keyOf(e.keys, "model") ?? "")), [doc, sel, all]);
  const [bounds, setBounds] = useState<Map<string, [Vec3, Vec3]>>(new Map());
  useEffect(() => {
    let live = true;
    for (const e of picked) {
      const m = keyOf(e.keys, "model")!;
      if (bounds.has(m)) continue;
      void loadModel(m).then((g) => live && g && setBounds((old) => new Map(old).set(m, g.bundle.bounds as [Vec3, Vec3])));
    }
    return () => {
      live = false;
    };
  }, [picked]); // eslint-disable-line react-hooks/exhaustive-deps
  const obj = useMemo(() => {
    const pos: number[] = [];
    const col: number[] = [];
    for (const e of picked) {
      const b = bounds.get(keyOf(e.keys, "model")!);
      const box = b && collisionBox(e, b);
      if (!box) continue;
      const seg = edges(box);
      pos.push(...seg);
      const c = COLLISION_COLOR[keyOf(e.keys, COLLISION_KEY)!];
      for (let i = 0; i < seg.length / 3; i++) col.push(...c);
    }
    if (!pos.length) return null;
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
    g.setAttribute("color", new THREE.Float32BufferAttribute(col, 3));
    const l = new THREE.LineSegments(g, MATERIAL);
    l.raycast = () => null;
    l.renderOrder = 4;
    return l;
  }, [picked, bounds]);
  useEffect(() => () => obj?.geometry.dispose(), [obj]);
  return obj ? <primitive object={obj} /> : null;
}
