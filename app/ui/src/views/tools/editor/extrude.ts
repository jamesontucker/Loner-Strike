// Parity A: TrenchBroom-style brush editing from the views.
// - Extrude: Shift-drag a face of a picked brush (Select tool, 3D view) pulls a new brush out of
//   it, as far as the mouse goes along the face's normal; the face's far side lands on the grid
//   when the face is axis-aligned. The new brush is picked when you let go; a press that doesn't
//   move is still a Shift-click.
// - Split: Ctrl/⌘-drag the middle handle of an edge or a face (Vertex tool) adds a corner there
//   and drags it out, keeping the brush convex (the core refuses dents and says why).
// Both send one command per step with a drag key, so a whole drag is one undo step.

import type { RootState } from "@react-three/fiber";
import * as THREE from "three";
import { useTools } from "../../../stores/tools";
import { threeToGs, type Vec3 } from "../viewer/coords";
import { viewElement } from "./Marquee";
import { snap, useEditor } from "./store";

const add = (a: Vec3, b: Vec3): Vec3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const mul = (a: Vec3, k: number): Vec3 => [a[0] * k, a[1] * k, a[2] * k];
const dot = (a: Vec3, b: Vec3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

/** The mouse ray in GoldSrc. */
function ray(e: { clientX: number; clientY: number }, st: RootState): { o: Vec3; d: Vec3 } {
  const r = viewElement(st).getBoundingClientRect();
  const ndc = new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
  const rc = new THREE.Raycaster();
  rc.setFromCamera(ndc, st.camera);
  return { o: threeToGs(rc.ray.origin), d: threeToGs(rc.ray.direction) };
}

/** How far along the line p0 + t·n the ray passes closest (null when they run side by side). */
function along(r: { o: Vec3; d: Vec3 }, p0: Vec3, n: Vec3): number | null {
  const w = sub(p0, r.o);
  const b = dot(n, r.d);
  const c = dot(r.d, r.d);
  const den = c - b * b;
  if (Math.abs(den) < 1e-4) return null;
  return (b * dot(r.d, w) - c * dot(n, w)) / den;
}

function onPlane(r: { o: Vec3; d: Vec3 }, p0: Vec3, n: Vec3): Vec3 | null {
  const den = dot(r.d, n);
  if (Math.abs(den) < 1e-6) return null;
  return add(r.o, mul(r.d, dot(sub(p0, r.o), n) / den));
}

let seq = 0;

/** One request at a time; the newest value wins. */
function stepper<T>(send: (v: T, drag: string) => Promise<{ error?: string; created?: { brushes: number[] } }>, same: (a: T, b: T) => boolean, zero: T) {
  const drag = `pa-${Date.now()}-${++seq}`;
  let sent = zero;
  let want = zero;
  let busy = false;
  let done: (() => void) | null = null;
  let error: string | null = null;
  let made: number | null = null;
  const run = async () => {
    if (busy) return;
    busy = true;
    while (!same(want, sent)) {
      const v = want;
      const r = await send(v, drag);
      sent = v;
      error = r.error ?? null;
      if (r.created?.brushes?.length) made = r.created.brushes[0];
      useTools.getState().setHover(r.error ?? null);
    }
    busy = false;
    done?.();
  };
  return {
    to(v: T) {
      want = v;
      void run();
    },
    async finish() {
      if (busy) await new Promise<void>((res) => (done = res));
      return { moved: !same(sent, zero), error, made };
    },
  };
}

type FaceHit = { id: number; face: number; point: Vec3; normal: Vec3 };

/** Shift-drag on a picked brush's face: pull out a new brush. */
export function startExtrude(ev: PointerEvent, st: RootState, hit: FaceHit, controlsOff: (off: boolean) => void, onClick: () => void) {
  const n = hit.normal;
  const p0 = hit.point;
  const axisAligned = n.filter((x) => Math.abs(x) > 0.999).length === 1;
  const planeD = dot(n, p0);
  const start = { x: ev.clientX, y: ev.clientY };
  let moved = false;
  controlsOff(true);
  const step = stepper<number>(
    (d, drag) => useEditor.getState().cmd({ op: "extrude_face", brush: hit.id, face: hit.face, distance: d, drag }, { doc: false, quiet: true }),
    (a, b) => a === b,
    0,
  );
  const move = (m: PointerEvent) => {
    if (!moved && Math.hypot(m.clientX - start.x, m.clientY - start.y) < 4) return;
    moved = true;
    let t = along(ray(m, st), p0, n);
    if (t == null) t = -(m.clientY - start.y);
    // the new brush's far side on the grid (axis-aligned faces), else whole grid steps out
    const d = Math.max(0, axisAligned && !useEditor.getState().alt ? snap(planeD + t) - planeD : snap(t));
    useTools.getState().setHover(d > 0 ? `Extrude ${Math.round(d * 100) / 100} units` : "Drag outwards to pull out a new brush");
    step.to(d);
  };
  const up = async () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    controlsOff(false);
    if (!moved) return onClick();
    const r = await step.finish();
    const s = useEditor.getState();
    useTools.getState().setHover(null);
    if (r.moved) {
      await s.refreshDoc();
      s.refreshChecks();
      // (the state after the refresh: a drag back onto the face leaves no brush)
      const now = useEditor.getState();
      if (r.made != null && now.doc?.brushes.some((b) => b.id === r.made)) now.select({ brushes: [r.made], entities: [], face: null });
    }
    if (r.error && !r.made) s.notify(r.error, true);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}

/** Ctrl/⌘-drag on an edge's or face's middle handle: a new corner, dragged out. `axis`: the
 *  flat view's depth axis (the corner stays in the view's plane), or null in 3D (a plane facing
 *  the camera through the handle). */
export function startSplit(ev: PointerEvent, st: RootState, brush: number, at: Vec3, axis: number | null, controlsOff: (off: boolean) => void) {
  const r0 = ray(ev, st);
  const n: Vec3 = axis == null ? mul(r0.d, -1) : ([0, 1, 2].map((i) => (i === axis ? 1 : 0)) as Vec3);
  const startAt = onPlane(r0, at, n);
  if (!startAt) return;
  controlsOff(true);
  const step = stepper<Vec3>(
    (d, drag) => useEditor.getState().cmd({ op: "add_vertex", brush, at, delta: d, drag }, { doc: false, quiet: true }),
    (a, b) => a[0] === b[0] && a[1] === b[1] && a[2] === b[2],
    [0, 0, 0],
  );
  let last: Vec3 = [0, 0, 0];
  const move = (m: PointerEvent) => {
    const q = onPlane(ray(m, st), at, n);
    if (!q) return;
    // the new corner lands on the grid
    const target = add(at, sub(q, startAt)).map((x) => snap(x)) as Vec3;
    if (axis != null) target[axis] = at[axis];
    last = sub(target, at);
    useTools.getState().setHover(`New corner at ${target.map((x) => Math.round(x * 100) / 100).join(" ")}`);
    step.to(last);
  };
  const up = async () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    controlsOff(false);
    const r = await step.finish();
    const s = useEditor.getState();
    useTools.getState().setHover(null);
    if (r.moved && !r.error) {
      useEditor.setState({ verts: [{ brush, p: add(at, last) }] });
      await s.refreshDoc();
      s.refreshChecks();
    } else if (r.error) s.notify(r.error, true);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}

/** Merge the picked corners of each brush into one (at their middle). */
export async function mergePickedCorners() {
  const s = useEditor.getState();
  const by = new Map<number, Vec3[]>();
  for (const v of s.verts) by.set(v.brush, [...(by.get(v.brush) ?? []), v.p]);
  const groups = [...by.entries()].filter(([, ps]) => ps.length >= 2);
  if (!groups.length) return s.notify("Pick two or more corners of one brush to merge.", true);
  const next: { brush: number; p: Vec3 }[] = [];
  for (const [brush, picks] of groups) {
    const r = await s.cmd({ op: "merge_vertices", brush, picks });
    if (r.error) return;
    next.push({ brush, p: mul(picks.reduce((a, p) => add(a, p), [0, 0, 0] as Vec3), 1 / picks.length) });
  }
  useEditor.setState({ verts: next });
}
