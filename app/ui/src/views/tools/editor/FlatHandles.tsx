// Parity A: Hammer's selection box in the Top, Front and Side views. Eight square handles on
// the picked things' box (corners and edge middles): drag one to stretch the selection, the
// moved edge landing on the grid (Alt or Snap off: free). Dragging a picked brush in a flat
// view moves the selection in the view's plane, its corner landing on the grid
// (`startFlatMove`, called from the select tool). Both show the change live (the editor's
// `preview` matrix) and send one undoable transform at the end. ⌘/Ctrl-drag an edge's middle
// handle shears (Hammer's skew): that edge slides along itself on the grid, the opposite edge
// stays (a box becomes a slanted wall; the core refuses shears steeper than 8:1).

import { useFrame, useThree, type RootState } from "@react-three/fiber";
import { useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { useTools } from "../../../stores/tools";
import { gsToThree, ROOT_ROTATION, type Vec3 } from "../viewer/coords";
import { viewElement } from "./Marquee";
import { selectionBounds, snap, useEditor, type ViewId } from "./store";

type FlatView = Exclude<ViewId, "3d">;
/** The axis a flat view looks along, and its two screen axes (right, up) in GoldSrc. */
export function flatAxes(view: FlatView): { axis: number; u: number; v: number } {
  return view === "top" ? { axis: 2, u: 0, v: 1 } : view === "front" ? { axis: 1, u: 0, v: 2 } : { axis: 0, u: 1, v: 2 };
}

/** How close (screen pixels) the mouse must be to a handle's middle to grab it. */
const GRAB = 7;
/** A handle's size in pixels. */
const SIZE = 8;

/** The GoldSrc point under a window position, in a flat view (the depth axis is left as the camera's). */
export function flatPoint(st: RootState, el: HTMLElement, x: number, y: number): Vec3 {
  const r = el.getBoundingClientRect();
  const ndc = new THREE.Vector3(((x - r.left) / r.width) * 2 - 1, -((y - r.top) / r.height) * 2 + 1, 0);
  ndc.unproject(st.camera);
  return [ndc.x, -ndc.z, ndc.y];
}

function toScreen(st: RootState, el: HTMLElement, p: Vec3): { x: number; y: number } {
  const r = el.getBoundingClientRect();
  const q = gsToThree(p).project(st.camera);
  return { x: r.left + ((q.x + 1) / 2) * r.width, y: r.top + ((1 - q.y) / 2) * r.height };
}

function selectedBrushCount(): number {
  const s = useEditor.getState();
  let n = s.sel.brushes.length;
  for (const id of s.sel.entities) n += s.doc?.entities.find((e) => e.id === id)?.brushes.length ?? 0;
  return n;
}

function ids() {
  const s = useEditor.getState();
  const brushes = new Set(s.sel.brushes);
  for (const id of s.sel.entities) for (const b of s.doc?.entities.find((e) => e.id === id)?.brushes ?? []) brushes.add(b);
  return { brushes: [...brushes], entities: s.sel.entities };
}

/** Scale about `pivot` (GoldSrc), as the editor's preview matrix wants it. */
function scaleAbout(f: Vec3, pivot: Vec3): THREE.Matrix4 {
  const t = new THREE.Matrix4().makeTranslation(pivot[0], pivot[1], pivot[2]);
  const s = new THREE.Matrix4().makeScale(f[0], f[1], f[2]);
  const back = new THREE.Matrix4().makeTranslation(-pivot[0], -pivot[1], -pivot[2]);
  return t.multiply(s).multiply(back);
}

const round = (x: number) => Math.round(x * 10000) / 10000;

/** One handle: which side of the box it pulls on each screen axis (-1 min, 0 none, 1 max). */
type Grip = { su: -1 | 0 | 1; sv: -1 | 0 | 1 };
const GRIPS: Grip[] = [
  { su: -1, sv: -1 },
  { su: 0, sv: -1 },
  { su: 1, sv: -1 },
  { su: 1, sv: 0 },
  { su: 1, sv: 1 },
  { su: 0, sv: 1 },
  { su: -1, sv: 1 },
  { su: -1, sv: 0 },
];

function cursorFor(g: Grip): string {
  if (g.su && g.sv) return g.su === g.sv ? "nesw-resize" : "nwse-resize";
  return g.su ? "ew-resize" : "ns-resize";
}

/** The selection box's handles in one flat view. Mount inside the pane's scene. */
export function FlatHandles({ view }: { view: FlatView }) {
  const st = useThree();
  const live = useRef(st);
  live.current = st;
  const tool = useEditor((s) => s.tool);
  const sel = useEditor((s) => s.sel);
  const brushes = useEditor((s) => s.brushes);
  const doc = useEditor((s) => s.doc);
  const [drag, setDrag] = useState<[Vec3, Vec3] | null>(null);
  const on = tool === "select" || tool === "scale";
  const box = useMemo(() => (on ? selectionBounds() : null), [on, sel, brushes, doc]); // eslint-disable-line react-hooks/exhaustive-deps
  const canStretch = useMemo(() => on && selectedBrushCount() > 0, [on, sel, doc]); // eslint-disable-line react-hooks/exhaustive-deps
  const { axis, u, v } = flatAxes(view);
  const shown = drag ?? box;

  // grab a handle before anything else in this pane sees the press
  useEffect(() => {
    if (!box || !canStretch) return;
    const near = (ev: { clientX: number; clientY: number }): Grip | null => {
      const s = live.current;
      const el = viewElement(s);
      const r = el.getBoundingClientRect();
      if (ev.clientX < r.left || ev.clientX > r.right || ev.clientY < r.top || ev.clientY > r.bottom) return null;
      const [lo, hi] = box;
      for (const g of GRIPS) {
        const p: Vec3 = [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2];
        if (g.su) p[u] = g.su < 0 ? lo[u] : hi[u];
        if (g.sv) p[v] = g.sv < 0 ? lo[v] : hi[v];
        const q = toScreen(s, el, p);
        if (Math.abs(q.x - ev.clientX) <= GRAB && Math.abs(q.y - ev.clientY) <= GRAB) return g;
      }
      return null;
    };
    let hoverCursor = false;
    const hover = (ev: PointerEvent) => {
      if (ev.buttons) return;
      const el = viewElement(live.current);
      const g = near(ev);
      if (g) {
        // ⌘/Ctrl on an edge's middle: shear (the edge slides along itself)
        const shearing = (ev.ctrlKey || ev.metaKey) && !g.su !== !g.sv;
        el.style.cursor = shearing ? (g.sv ? "ew-resize" : "ns-resize") : cursorFor(g);
        hoverCursor = true;
      } else if (hoverCursor) {
        el.style.cursor = "";
        hoverCursor = false;
      }
    };
    const down = (ev: PointerEvent) => {
      if (ev.button !== 0) return;
      const g = near(ev);
      if (!g) return;
      ev.stopPropagation();
      ev.preventDefault();
      if ((ev.ctrlKey || ev.metaKey) && !g.su !== !g.sv) shear(ev, g, box);
      else stretch(ev, g, box);
    };
    // Parity A (Hammer's skew): the dragged edge slides along itself by whole grid steps, the
    // opposite edge stays; one "Shear" step
    const shear = (ev0: PointerEvent, g: Grip, b0: [Vec3, Vec3]) => {
      const s = live.current;
      const el = viewElement(s);
      const controls = s.controls as unknown as { enabled: boolean } | null;
      if (controls) controls.enabled = false;
      const [lo0, hi0] = b0;
      const axis = g.sv ? v : u; // the edge's place across
      const along = g.sv ? u : v; // the way it slides
      const side = g.sv || g.su;
      const edge = side < 0 ? lo0[axis] : hi0[axis];
      const pivot: Vec3 = [0, 0, 0];
      pivot[axis] = side < 0 ? hi0[axis] : lo0[axis];
      const span = edge - pivot[axis];
      const start = flatPoint(s, el, ev0.clientX, ev0.clientY);
      let factor = 0;
      const move = (m: PointerEvent) => {
        const p = flatPoint(s, el, m.clientX, m.clientY);
        const d = snap(p[along] - start[along]);
        factor = Math.abs(span) > 1e-6 ? d / span : 0;
        const sh = new THREE.Matrix4().identity();
        sh.elements[axis * 4 + along] = factor; // column-major: row `along`, column `axis`
        const t = new THREE.Matrix4().makeTranslation(pivot[0], pivot[1], pivot[2]);
        const back = new THREE.Matrix4().makeTranslation(-pivot[0], -pivot[1], -pivot[2]);
        useEditor.setState({ preview: t.multiply(sh).multiply(back) });
        setDrag([lo0, hi0]);
        useTools.getState().setHover(`Shear ${Math.round(d * 100) / 100} units`);
        s.invalidate();
      };
      const up = async () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        if (controls) controls.enabled = true;
        setDrag(null);
        useTools.getState().setHover(null);
        if (factor === 0) return useEditor.setState({ preview: null });
        const e = useEditor.getState();
        const r = await e.cmd({ op: "shear", ...ids(), axis, along, factor, pivot, lock: e.textureLock, keep_scale: !e.scaleLock });
        if (r.error || !r.changed) useEditor.setState({ preview: null });
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
    };
    const stretch = (ev0: PointerEvent, g: Grip, b0: [Vec3, Vec3]) => {
      const s = live.current;
      const el = viewElement(s);
      const controls = s.controls as unknown as { enabled: boolean } | null;
      if (controls) controls.enabled = false;
      const [lo0, hi0] = b0;
      const start = flatPoint(s, el, ev0.clientX, ev0.clientY);
      let f: Vec3 = [1, 1, 1];
      let pivot: Vec3 = [...lo0];
      const move = (m: PointerEvent) => {
        const p = flatPoint(s, el, m.clientX, m.clientY);
        const lo: Vec3 = [...lo0];
        const hi: Vec3 = [...hi0];
        for (const [ax, side] of [
          [u, g.su],
          [v, g.sv],
        ] as const) {
          if (!side) continue;
          const edge0 = side < 0 ? lo0[ax] : hi0[ax];
          const at = snap(edge0 + p[ax] - start[ax]);
          if (side < 0) lo[ax] = Math.min(at, hi0[ax] - 1);
          else hi[ax] = Math.max(at, lo0[ax] + 1);
        }
        f = [0, 1, 2].map((i) => (hi[i] - lo[i]) / Math.max(1e-6, hi0[i] - lo0[i])) as Vec3; // exact: the core tidies the result
        pivot = [0, 1, 2].map((i) => (i === u ? (g.su < 0 ? hi0[i] : lo0[i]) : i === v ? (g.sv < 0 ? hi0[i] : lo0[i]) : lo0[i])) as Vec3;
        useEditor.setState({ preview: scaleAbout(f, pivot) });
        setDrag([lo, hi]);
        const size = [u, v].map((i) => Math.round((hi[i] - lo[i]) * 100) / 100);
        useTools.getState().setHover(`Size ${size[0]} × ${size[1]} (${/Mac/.test(navigator.platform) ? "⌘" : "Ctrl"}-drag an edge's middle to shear)`);
        s.invalidate();
      };
      const up = async () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        if (controls) controls.enabled = true;
        setDrag(null);
        useTools.getState().setHover(null);
        if (f.every((x) => x === 1)) return useEditor.setState({ preview: null });
        const e = useEditor.getState();
        const r = await e.cmd({ op: "transform", ...ids(), scale: f, pivot, lock: e.textureLock });
        if (r.error || !r.changed) useEditor.setState({ preview: null });
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
    };
    window.addEventListener("pointerdown", down, true);
    window.addEventListener("pointermove", hover, true);
    // dev builds: tests find the handles on screen (tests/editor-flat.spec.ts)
    const hooks = (window as unknown as { __lsFlatHandles?: Record<string, () => { x: number; y: number }[]> }).__lsFlatHandles ??= {};
    if (import.meta.env.DEV) {
      hooks[view] = () => {
        const s = live.current;
        const el = viewElement(s);
        const [lo, hi] = box;
        return GRIPS.map((g) => {
          const p: Vec3 = [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, (lo[2] + hi[2]) / 2];
          if (g.su) p[u] = g.su < 0 ? lo[u] : hi[u];
          if (g.sv) p[v] = g.sv < 0 ? lo[v] : hi[v];
          return toScreen(s, el, p);
        });
      };
    }
    return () => {
      if (import.meta.env.DEV) delete hooks[view];
      window.removeEventListener("pointerdown", down, true);
      window.removeEventListener("pointermove", hover, true);
      if (hoverCursor) viewElement(live.current).style.cursor = "";
    };
  }, [box, canStretch, u, v, view]);

  // handles keep their size on screen whatever the zoom
  const group = useRef<THREE.Group>(null);
  useFrame(({ camera }) => {
    const k = SIZE / ((camera as THREE.OrthographicCamera).zoom || 1);
    group.current?.children.forEach((c) => {
      if (c.userData.handle) c.scale.setScalar(k);
    });
  });

  if (!shown || !on) return null;
  const [lo, hi] = shown;
  // drawn in front of everything in this view
  const front = view === "front" ? lo[axis] - 4 : hi[axis] + 4;
  const corner = (a: number, b: number): Vec3 => {
    const p: Vec3 = [0, 0, 0];
    p[u] = a;
    p[v] = b;
    p[axis] = front;
    return p;
  };
  const rect = [corner(lo[u], lo[v]), corner(hi[u], lo[v]), corner(hi[u], hi[v]), corner(lo[u], hi[v])];
  const outline = new Float32Array([0, 1, 1, 2, 2, 3, 3, 0].flatMap((i) => rect[i]));
  return (
    <group rotation={ROOT_ROTATION} ref={group}>
      <lineSegments renderOrder={20}>
        <bufferGeometry key={outline.join(",")}>
          <bufferAttribute attach="attributes-position" args={[outline, 3]} />
        </bufferGeometry>
        <lineBasicMaterial color="#ffd84a" depthTest={false} transparent opacity={0.9} />
      </lineSegments>
      {canStretch &&
        GRIPS.map((g, i) => {
          const p = corner(g.su < 0 ? lo[u] : g.su > 0 ? hi[u] : (lo[u] + hi[u]) / 2, g.sv < 0 ? lo[v] : g.sv > 0 ? hi[v] : (lo[v] + hi[v]) / 2);
          const rot: [number, number, number] = axis === 2 ? [0, 0, 0] : axis === 1 ? [Math.PI / 2, 0, 0] : [0, Math.PI / 2, 0];
          return (
            <mesh key={i} position={p} rotation={rot} renderOrder={21} userData={{ handle: true }} scale={1}>
              <planeGeometry args={[1, 1]} />
              <meshBasicMaterial color="#ffd84a" depthTest={false} side={THREE.DoubleSide} />
            </mesh>
          );
        })}
    </group>
  );
}

/** Drag the picked things in a flat view's plane: their box's corner lands on the grid. A press
 *  that doesn't move is a click (`onClick`). Called from the select tool on a picked brush. */
export function startFlatMove(ev: PointerEvent, st: RootState, view: FlatView, onClick: () => void) {
  const el = viewElement(st);
  const { u, v } = flatAxes(view);
  const b0 = selectionBounds();
  if (!b0) return onClick();
  const controls = st.controls as unknown as { enabled: boolean } | null;
  if (controls) controls.enabled = false;
  const start = flatPoint(st, el, ev.clientX, ev.clientY);
  const sx = ev.clientX;
  const sy = ev.clientY;
  let moved = false;
  let d: Vec3 = [0, 0, 0];
  const move = (m: PointerEvent) => {
    if (!moved && Math.hypot(m.clientX - sx, m.clientY - sy) < 4) return;
    moved = true;
    const p = flatPoint(st, el, m.clientX, m.clientY);
    d = [0, 0, 0];
    for (const ax of [u, v]) d[ax] = round(snap(b0[0][ax] + p[ax] - start[ax]) - b0[0][ax]);
    useEditor.setState({ preview: new THREE.Matrix4().makeTranslation(d[0], d[1], d[2]) });
    useTools.getState().setHover(`Move ${d[u]}, ${d[v]}`);
    st.invalidate();
  };
  const up = async () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    if (controls) controls.enabled = true;
    useTools.getState().setHover(null);
    if (!moved) return onClick();
    if (d.every((x) => x === 0)) return useEditor.setState({ preview: null });
    const e = useEditor.getState();
    const r = await e.cmd({ op: "transform", ...ids(), translate: d, lock: e.textureLock });
    if (r.error || !r.changed) useEditor.setState({ preview: null });
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}
