// Smaller pieces of the editor's 3D view: vertex handles, texture drag handles, the measure
// line, the box-select rectangle and camera bookmarks.
// Everything except Bookmarks sits inside the root group, in GoldSrc units.

import { Html, Line } from "@react-three/drei";
import { useThree } from "@react-three/fiber";
import { useEffect, useMemo } from "react";
import * as THREE from "three";
import type { Vec3 } from "../viewer/coords";
import { alignOf } from "./editorApi";
import { facesOf, isHiddenBrush, useEditor, type BrushGeo } from "./store";

const add = (a: Vec3, b: Vec3): Vec3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const mul = (a: Vec3, k: number): Vec3 => [a[0] * k, a[1] * k, a[2] * k];
const len = (a: Vec3) => Math.hypot(a[0], a[1], a[2]);
const norm = (a: Vec3): Vec3 => {
  const l = len(a) || 1;
  return [a[0] / l, a[1] / l, a[2] / l];
};
export const sameP = (a: Vec3, b: Vec3) => Math.abs(a[0] - b[0]) + Math.abs(a[1] - b[1]) + Math.abs(a[2] - b[2]) < 0.02;

/** A brush's corners (each once). */
export function cornersOf(b: BrushGeo): Vec3[] {
  const out: Vec3[] = [];
  for (const poly of b.polys.values()) for (const p of poly) if (!out.some((q) => sameP(q, p))) out.push(p);
  return out;
}

/** The corners of the picked brushes, as handles to click and drag (vertex tool). */
export function VertexHandles({ ids }: { ids: number[] }) {
  const brushes = useEditor((s) => s.brushes);
  const verts = useEditor((s) => s.verts);
  const camera = useThree((s) => s.camera);
  // a size that stays readable: a share of the brush, at least a few units
  const handles = useMemo(() => {
    const out: { brush: number; p: Vec3; r: number }[] = [];
    for (const id of ids) {
      const b = brushes.get(id);
      if (!b) continue;
      const r = Math.max(2, Math.min(8, Math.min(...b.max.map((x, i) => x - b.min[i])) / 10));
      for (const p of cornersOf(b)) out.push({ brush: id, p, r });
    }
    return out.slice(0, 4000);
  }, [ids, brushes]);
  const ortho = camera instanceof THREE.OrthographicCamera;
  return (
    <>
      {handles.map((h, i) => {
        const on = verts.some((v) => v.brush === h.brush && sameP(v.p, h.p));
        return (
          <mesh key={i} position={h.p} userData={{ kind: "vert", id: h.brush, p: h.p }} renderOrder={6}>
            <boxGeometry args={[h.r * (ortho ? 1.5 : 1), h.r * (ortho ? 1.5 : 1), h.r * (ortho ? 1.5 : 1)]} />
            <meshBasicMaterial color={on ? "#ff4d2e" : "#ffd84a"} depthTest={false} />
          </mesh>
        );
      })}
    </>
  );
}

/** The picked face's texture axes, with handles: the arrow ends stretch, the ring turns. */
export function TexHandles() {
  const sel = useEditor((s) => s.sel);
  const brushes = useEditor((s) => s.brushes);
  const tool = useEditor((s) => s.tool);
  const f = facesOf(sel)[0];
  const b = f ? brushes.get(f.brush) : null;
  const face = b?.faces.find((x) => x.face === f!.face);
  const poly = b?.polys.get(f?.face ?? -1);
  const a = alignOf(face);
  if (tool !== "texture" || !face || !poly || !a) return null;
  const c = mul(poly.reduce((acc, p) => add(acc, p), [0, 0, 0] as Vec3), 1 / poly.length);
  const lift = mul(face.normal, 1.5);
  const size = Math.max(16, Math.min(96, Math.max(...poly.map((p) => len([p[0] - c[0], p[1] - c[1], p[2] - c[2]]))) * 0.5));
  const centre = add(c, lift);
  const u = add(centre, mul(norm(a.u), size));
  const v = add(centre, mul(norm(a.v), size));
  const ring = add(centre, mul(norm(add(norm(a.u), mul(norm(a.v), -1))), size * 0.8));
  const knob = (p: Vec3, kind: string, colour: string, title: string) => (
    <mesh position={p} userData={{ kind, id: f!.brush, face: f!.face }} renderOrder={8}>
      <sphereGeometry args={[Math.max(2.5, size / 14), 14, 10]} />
      <meshBasicMaterial color={colour} depthTest={false} />
      <Html center distanceFactor={undefined} style={{ pointerEvents: "none" }} zIndexRange={[5, 0]}>
        <span className="editor-handle-tip">{title}</span>
      </Html>
    </mesh>
  );
  return (
    <group>
      <Line points={[centre, u]} color="#ff6b5a" lineWidth={2.5} depthTest={false} renderOrder={7} />
      <Line points={[centre, v]} color="#5ab8ff" lineWidth={2.5} depthTest={false} renderOrder={7} />
      {knob(u, "tex-su", "#ff6b5a", "stretch")}
      {knob(v, "tex-sv", "#5ab8ff", "stretch")}
      {knob(ring, "tex-rot", "#ffd84a", "turn")}
    </group>
  );
}

/** The measure tool's line and its length. */
export function MeasureLine() {
  const m = useEditor((s) => s.measure);
  const tool = useEditor((s) => s.tool);
  if (!m || tool !== "measure") return null;
  const d = Math.hypot(m.b[0] - m.a[0], m.b[1] - m.a[1], m.b[2] - m.a[2]);
  const mid = mul(add(m.a, m.b), 0.5);
  return (
    <group>
      <Line points={[m.a, m.b]} color="#7dffb0" lineWidth={2.5} depthTest={false} renderOrder={9} />
      {[m.a, m.b].map((p, i) => (
        <mesh key={i} position={p} renderOrder={9}>
          <sphereGeometry args={[3, 10, 8]} />
          <meshBasicMaterial color="#7dffb0" depthTest={false} />
        </mesh>
      ))}
      <Html position={mid} center style={{ pointerEvents: "none" }}>
        <span className="editor-measure">{Math.round(d * 100) / 100} u</span>
      </Html>
    </group>
  );
}

/** The 2D views' box-select rectangle (corners on the working plane). */
export function SelectRect({ a, b, axis }: { a: Vec3; b: Vec3; axis: number }) {
  const [u, v] = [0, 1, 2].filter((i) => i !== axis);
  const at = (pu: number, pv: number): Vec3 => {
    const p: Vec3 = [0, 0, 0];
    p[u] = pu;
    p[v] = pv;
    p[axis] = a[axis];
    return p;
  };
  const pts = [at(a[u], a[v]), at(b[u], a[v]), at(b[u], b[v]), at(a[u], b[v]), at(a[u], a[v])];
  return <Line points={pts} color="#ffd84a" lineWidth={1.5} dashed dashSize={8} gapSize={6} depthTest={false} renderOrder={10} />;
}

/** Whether a brush is drawn (not hidden here or in a hidden group). */
export function useVisibleBrush() {
  const hidden = useEditor((s) => s.hidden);
  const groups = useEditor((s) => s.doc?.groups);
  const hiddenGroups = useMemo(() => new Set((groups ?? []).filter((g) => g.hidden).map((g) => g.id)), [groups]);
  return useMemo(() => (b: BrushGeo) => !isHiddenBrush(b, { ...useEditor.getState(), hidden }, hiddenGroups), [hidden, hiddenGroups]);
}

// ---- camera bookmarks -------------------------------------------------------------------

type Mark = { pos: number[]; quat: number[]; target: number[] | null };
let handled = 0;

function marksKey() {
  return `ls.editor.cams.${useEditor.getState().doc?.name ?? ""}`;
}
function readMarks(): Record<string, Mark> {
  try {
    const v = JSON.parse(localStorage.getItem(marksKey()) ?? "{}");
    return v && typeof v === "object" ? v : {};
  } catch {
    return {};
  }
}

/** Ctrl+1-9 store the 3D camera, 1-9 go back to it (inside the Canvas, outside the root group). */
export function Bookmarks() {
  const camera = useThree((s) => s.camera);
  const controls = useThree((s) => s.controls) as unknown as { target?: THREE.Vector3; update?: () => void } | null;
  const invalidate = useThree((s) => s.invalidate);
  const req = useEditor((s) => s.bookmark);
  const view = useEditor((s) => s.view);
  useEffect(() => {
    if (!req || req.seq <= handled || view !== "3d" || !(camera instanceof THREE.PerspectiveCamera)) return;
    // going to a mark waits for the orbit/fly controls, so they don't undo the jump
    if (!req.store && !controls) return;
    handled = req.seq;
    const s = useEditor.getState();
    const marks = readMarks();
    if (req.store) {
      marks[req.n] = { pos: camera.position.toArray(), quat: camera.quaternion.toArray(), target: controls?.target ? controls.target.toArray() : null };
      try {
        localStorage.setItem(marksKey(), JSON.stringify(marks));
      } catch {
        /* private window */
      }
      s.notify(`Camera ${req.n} stored. Press ${req.n} to come back here.`);
      return;
    }
    const m = marks[req.n];
    if (!m) return s.notify(`No camera ${req.n} yet. Ctrl+${req.n} stores one.`);
    camera.position.fromArray(m.pos);
    camera.quaternion.fromArray(m.quat);
    if (controls?.target) {
      if (m.target) controls.target.fromArray(m.target);
      else controls.target.copy(camera.position).add(camera.getWorldDirection(new THREE.Vector3()).multiplyScalar(256));
      controls.update?.();
    }
    invalidate();
  }, [req, view, camera, controls, invalidate]);
  return null;
}
