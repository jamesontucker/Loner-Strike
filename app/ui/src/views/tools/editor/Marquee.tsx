// Box (marquee) select, the same in every view and pane: drag on empty space with a picking
// tool. A see-through rectangle follows the mouse and what it will pick lights up while you
// drag. Drag right = only what's fully inside (solid outline), drag left = anything it touches
// (dashed), unless a mode is fixed in the options. Release: plain replaces the selection,
// Shift adds, Ctrl/⌘ takes away. Esc cancels; a drag under 4 px is a click. The texture tool
// picks faces this way. In the 3D view things behind the camera never count, and "Only what
// you can see" leaves out what's hidden behind walls.

import type { RootState } from "@react-three/fiber";
import { Line } from "@react-three/drei";
import { useMemo } from "react";
import * as THREE from "three";
import { create } from "zustand";
import { useTools } from "../../../stores/tools";
import { gsToThree, type Vec3 } from "../viewer/coords";
import { groupMembers, isHiddenBrush, topGroup, useEditor, type BrushGeo, type FaceRef } from "./store";
import { sharedTriangles } from "./brushBuffers"; // Parity A
import { InstancedMarkers } from "./EntityBatch"; // Parity A
import "./p4.css";

export type MarqueeMode = "auto" | "inside" | "touching";

type Hot = { brushes: Set<number>; entities: Set<number>; faces: FaceRef[] };

type MarqueeState = {
  mode: MarqueeMode;
  /** leave out what's hidden behind other brushes (3D) */
  onlyVisible: boolean;
  /** 3D: a left drag anywhere (not on a picked brush) boxes; the camera turns with the right button */
  anywhere3d: boolean;
  /** what the box would pick right now */
  hot: Hot | null;
  set: (p: Partial<Omit<MarqueeState, "set">>) => void;
};

function saved<T extends string>(key: string, ok: T[], fallback: T): T {
  try {
    const v = localStorage.getItem(key) as T | null;
    return v && ok.includes(v) ? v : fallback;
  } catch {
    return fallback;
  }
}
function keep(key: string, v: string) {
  try {
    localStorage.setItem(key, v);
  } catch {
    /* private window */
  }
}

export const useMarquee = create<MarqueeState>((set) => ({
  mode: saved<MarqueeMode>("ls.editor.marquee", ["auto", "inside", "touching"], "auto"),
  onlyVisible: saved("ls.editor.marqueeVisible", ["1", "0"], "0") === "1",
  anywhere3d: saved("ls.editor.marquee3d", ["1", "0"], "0") === "1",
  hot: null,
  set: (p) => {
    if (p.mode) keep("ls.editor.marquee", p.mode);
    if (p.onlyVisible != null) keep("ls.editor.marqueeVisible", p.onlyVisible ? "1" : "0");
    if (p.anywhere3d != null) keep("ls.editor.marquee3d", p.anywhere3d ? "1" : "0");
    set(p);
  },
}));

type Pt = { x: number; y: number };

/** The view's element (a pane, or the canvas). */
export function viewElement(st: RootState): HTMLElement {
  const pane = st.events.connected as HTMLElement | undefined;
  return pane?.dataset?.pane != null ? pane : st.gl.domElement;
}

/** What a box picks in one view. `inside`: wholly inside; else touching. */
export function pickInBox(st: RootState, el: HTMLElement, a: Pt, b: Pt, inside: boolean, faces: boolean, onlyVisible = false): Hot {
  const s = useEditor.getState();
  const r = el.getBoundingClientRect();
  const lo = { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y) };
  const hi = { x: Math.max(a.x, b.x), y: Math.max(a.y, b.y) };
  const cam = st.camera;
  cam.updateMatrixWorld();
  const persp = cam instanceof THREE.PerspectiveCamera;
  const v = new THREE.Vector3();
  const inv = cam.matrixWorldInverse;
  /** a point on screen, or null behind the camera */
  const scr = (p: Vec3): Pt | null => {
    gsToThree(p, v);
    if (persp && v.clone().applyMatrix4(inv).z > -1) return null;
    v.project(cam);
    return { x: r.left + ((v.x + 1) / 2) * r.width, y: r.top + ((1 - v.y) / 2) * r.height };
  };
  const inRect = (q: Pt) => q.x >= lo.x && q.x <= hi.x && q.y >= lo.y && q.y <= hi.y;
  /** points: all inside (inside) or their screen box crosses the rectangle (touching) */
  const test = (pts: Vec3[]) => {
    const sp = pts.map(scr);
    if (inside) return sp.every((q) => q && inRect(q));
    const ok = sp.filter((q): q is Pt => !!q);
    if (!ok.length) return false;
    const x0 = Math.min(...ok.map((q) => q.x));
    const x1 = Math.max(...ok.map((q) => q.x));
    const y0 = Math.min(...ok.map((q) => q.y));
    const y1 = Math.max(...ok.map((q) => q.y));
    return x0 <= hi.x && x1 >= lo.x && y0 <= hi.y && y1 >= lo.y;
  };
  const corners = (min: Vec3, max: Vec3): Vec3[] => Array.from({ length: 8 }, (_, k) => [k & 1 ? max[0] : min[0], k & 2 ? max[1] : min[1], k & 4 ? max[2] : min[2]] as Vec3);

  const groups = s.doc?.groups ?? [];
  const hiddenGroups = new Set(groups.filter((g) => g.hidden).map((g) => g.id));
  const lockedGroups = new Set(groups.filter((g) => g.locked).map((g) => g.id));
  const locked = (g: number | null) => g != null && (lockedGroups.has(g) || lockedGroups.has(topGroup(g, s.doc) ?? -1));

  // what can be seen (3D, on request): the first thing under a grid of points in the box
  let seen: { brushes: Set<number>; entities: Set<number>; faces: Set<string> } | null = null;
  if (onlyVisible) {
    seen = { brushes: new Set(), entities: new Set(), faces: new Set() };
    const w = hi.x - lo.x;
    const h = hi.y - lo.y;
    const step = Math.max(4, Math.sqrt((w * h) / 3600));
    const rc = new THREE.Raycaster();
    const ndc = new THREE.Vector2();
    for (let y = lo.y + step / 2; y < hi.y; y += step) {
      for (let x = lo.x + step / 2; x < hi.x; x += step) {
        ndc.set(((x - r.left) / r.width) * 2 - 1, -((y - r.top) / r.height) * 2 + 1);
        rc.setFromCamera(ndc, cam);
        for (const i of rc.intersectObjects(st.scene.children, true)) {
          const ud = i.object.userData as { kind?: string; id?: number; sky?: boolean };
          if (ud.kind === "brush" && ud.id != null && !ud.sky && i.object.visible) {
            seen.brushes.add(ud.id);
            const g = s.brushes.get(ud.id);
            const f = g && i.faceIndex != null ? g.faceOfTri[i.faceIndex] : undefined;
            if (f != null) seen.faces.add(`${ud.id}:${f}`);
            break;
          }
          if (ud.kind === "entity" && ud.id != null) {
            seen.entities.add(ud.id);
            break;
          }
        }
      }
    }
  }

  if (faces) {
    const out: FaceRef[] = [];
    const camPos = threeToGsVec(cam.position);
    const look = new THREE.Vector3();
    cam.getWorldDirection(look);
    const lookGs: Vec3 = [look.x, -look.z, look.y];
    for (const br of s.brushes.values()) {
      if (isHiddenBrush(br, s, hiddenGroups) || locked(br.group)) continue;
      for (const f of br.faces) {
        const poly = br.polys.get(f.face);
        if (!poly || poly.length < 3) continue;
        // only faces turned towards the view
        const toCam = persp ? [camPos[0] - poly[0][0], camPos[1] - poly[0][1], camPos[2] - poly[0][2]] : lookGs.map((x) => -x);
        if (f.normal[0] * toCam[0] + f.normal[1] * toCam[1] + f.normal[2] * toCam[2] <= 0) continue;
        if (seen && !seen.faces.has(`${br.id}:${f.face}`)) continue;
        if (test(poly)) out.push({ brush: br.id, face: f.face });
        if (out.length >= 5000) break;
      }
    }
    return { brushes: new Set(), entities: new Set(), faces: out };
  }

  const hitBrushes = new Set<number>();
  const brushOk = new Map<number, boolean>();
  for (const br of s.brushes.values()) {
    if (isHiddenBrush(br, s, hiddenGroups)) continue;
    const ok = test(corners(br.min, br.max)) && (!seen || seen.brushes.has(br.id));
    brushOk.set(br.id, ok);
    if (ok) hitBrushes.add(br.id);
  }
  const brushes = new Set<number>();
  const entities = new Set<number>();
  for (const id of hitBrushes) {
    const br = s.brushes.get(id)!;
    if (br.entity == null && !locked(br.group)) brushes.add(id);
  }
  for (const en of s.doc?.entities ?? []) {
    if (s.hidden.entities.has(en.id) || (en.group != null && hiddenGroups.has(en.group)) || locked(en.group)) continue;
    if (en.brushes.length) {
      const states = en.brushes.map((bid) => brushOk.get(bid)).filter((x) => x != null);
      if (states.length && (inside ? states.every(Boolean) : states.some(Boolean))) entities.add(en.id);
      continue;
    }
    const o = en.keys.find(([k]) => k === "origin")?.[1]?.trim().split(/\s+/).map(Number);
    if (!o || o.length < 3 || !o.every(Number.isFinite)) continue;
    const pts = corners([o[0] - 8, o[1] - 8, o[2] - 8], [o[0] + 8, o[1] + 8, o[2] + 8]);
    if (test(inside ? [[o[0], o[1], o[2]]] : pts) && (!seen || seen.entities.has(en.id))) entities.add(en.id);
  }
  // groups come whole ("Pick groups"): touching one member picks it all; fully inside needs all of it
  if (s.selectMode === "groups" && s.doc) {
    const tops = new Set<number>();
    for (const id of brushes) {
      const t = topGroup(s.brushes.get(id)?.group ?? null, s.doc);
      if (t != null) tops.add(t);
    }
    for (const id of entities) {
      const t = topGroup(s.doc.entities.find((e) => e.id === id)?.group ?? null, s.doc);
      if (t != null) tops.add(t);
    }
    for (const t of tops) {
      const m = groupMembers(t, s.doc);
      const all = m.brushes.every((x) => brushes.has(x)) && m.entities.every((x) => entities.has(x));
      if (inside && !all) {
        m.brushes.forEach((x) => brushes.delete(x));
        m.entities.forEach((x) => entities.delete(x));
      } else {
        m.brushes.forEach((x) => brushes.add(x));
        m.entities.forEach((x) => entities.add(x));
      }
    }
  }
  return { brushes, entities, faces: [] };
}

function threeToGsVec(p: THREE.Vector3): Vec3 {
  return [p.x, -p.z, p.y];
}

/** Starts a box from a press on empty space. `onClick` runs instead when it's a click. */
export function startMarquee(ev: PointerEvent, st: RootState, opts: { faces: boolean; onClick: () => void; controlsOff: (off: boolean) => void }) {
  const el = viewElement(st);
  const start = { x: ev.clientX, y: ev.clientY };
  let div: HTMLDivElement | null = null;
  let last = start;
  let frame = 0;
  let mods = { shift: ev.shiftKey, ctrl: ev.ctrlKey || ev.metaKey };
  opts.controlsOff(true);
  const insideNow = (p: Pt) => {
    const m = useMarquee.getState().mode;
    return m === "inside" ? true : m === "touching" ? false : p.x >= start.x;
  };
  const draw = () => {
    frame = 0;
    if (!div) return;
    const inside = insideNow(last);
    const d = div.style;
    d.left = `${Math.min(start.x, last.x)}px`;
    d.top = `${Math.min(start.y, last.y)}px`;
    d.width = `${Math.abs(last.x - start.x)}px`;
    d.height = `${Math.abs(last.y - start.y)}px`;
    div.classList.toggle("touching", !inside);
    div.classList.toggle("removing", mods.ctrl);
    div.classList.toggle("adding", mods.shift && !mods.ctrl);
    const hot = pickInBox(st, el, start, last, inside, opts.faces);
    useMarquee.getState().set({ hot });
    useTools.getState().setHover(
      `${inside ? "Fully inside" : "Touching"}${mods.ctrl ? ", taking away" : mods.shift ? ", adding" : ""}: ${opts.faces ? `${hot.faces.length} face${hot.faces.length === 1 ? "" : "s"}` : `${hot.brushes.size} brush${hot.brushes.size === 1 ? "" : "es"}, ${hot.entities.size} entit${hot.entities.size === 1 ? "y" : "ies"}`}`,
    );
    st.invalidate();
  };
  const end = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("keydown", key, true);
    window.removeEventListener("keyup", keyUp, true);
    if (frame) cancelAnimationFrame(frame);
    div?.remove();
    opts.controlsOff(false);
    useMarquee.getState().set({ hot: null });
    useTools.getState().setHover(null);
    st.invalidate();
  };
  const move = (m: PointerEvent) => {
    last = { x: m.clientX, y: m.clientY };
    mods = { shift: m.shiftKey, ctrl: m.ctrlKey || m.metaKey };
    if (!div) {
      if (Math.hypot(last.x - start.x, last.y - start.y) < 4) return;
      div = document.createElement("div");
      div.className = "editor-screen-rect";
      document.body.appendChild(div);
    }
    if (!frame) frame = requestAnimationFrame(draw);
  };
  const up = (u: PointerEvent) => {
    const was = div;
    last = { x: u.clientX, y: u.clientY };
    mods = { shift: u.shiftKey || mods.shift, ctrl: u.ctrlKey || u.metaKey || mods.ctrl };
    end();
    if (!was) return opts.onClick();
    const inside = insideNow(last);
    const got = pickInBox(st, el, start, last, inside, opts.faces, useMarquee.getState().onlyVisible && st.camera instanceof THREE.PerspectiveCamera);
    apply(got, opts.faces, mods);
  };
  const key = (k: KeyboardEvent) => {
    if (k.key === "Escape") {
      k.preventDefault();
      k.stopPropagation();
      end();
      return;
    }
    if (k.key === "Shift" || k.key === "Control" || k.key === "Meta") {
      mods = { shift: k.shiftKey, ctrl: k.ctrlKey || k.metaKey };
      if (div && !frame) frame = requestAnimationFrame(draw);
    }
  };
  const keyUp = (k: KeyboardEvent) => {
    if (k.key === "Shift" || k.key === "Control" || k.key === "Meta") {
      mods = { shift: k.shiftKey, ctrl: k.ctrlKey || k.metaKey };
      if (div && !frame) frame = requestAnimationFrame(draw);
    }
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  window.addEventListener("keydown", key, true);
  window.addEventListener("keyup", keyUp, true);
}

function apply(got: Hot, faces: boolean, mods: { shift: boolean; ctrl: boolean }) {
  const s = useEditor.getState();
  const cur = s.sel;
  if (faces) {
    const key = (f: FaceRef) => `${f.brush}:${f.face}`;
    const curFaces = cur.faces?.length ? cur.faces : cur.face ? [cur.face] : [];
    let next: FaceRef[];
    if (mods.ctrl) {
      const drop = new Set(got.faces.map(key));
      next = curFaces.filter((f) => !drop.has(key(f)));
    } else if (mods.shift) {
      const have = new Set(curFaces.map(key));
      next = [...curFaces, ...got.faces.filter((f) => !have.has(key(f)))];
    } else next = got.faces;
    s.select({ brushes: [], entities: [], face: next[0] ?? null, faces: next });
    return;
  }
  let brushes: number[];
  let entities: number[];
  if (mods.ctrl) {
    brushes = cur.brushes.filter((b) => !got.brushes.has(b));
    entities = cur.entities.filter((e) => !got.entities.has(e));
  } else if (mods.shift) {
    brushes = [...new Set([...cur.brushes, ...got.brushes])];
    entities = [...new Set([...cur.entities, ...got.entities])];
  } else {
    brushes = [...got.brushes];
    entities = [...got.entities];
  }
  s.select({ brushes, entities, face: null });
}

// ---- drawing what the box would pick ----------------------------------------------------------

const HOT_MATERIAL = new THREE.MeshBasicMaterial({ color: "#5fc8ff", transparent: true, opacity: 0.35, depthWrite: false, depthTest: false, polygonOffset: true, polygonOffsetFactor: -2, polygonOffsetUnits: -2 });

/** Mounted in each view: what the box would pick, tinted, while dragging. */
export function MarqueeHot() {
  const hot = useMarquee((s) => s.hot);
  const brushes = useEditor((s) => s.brushes);
  const doc = useEditor((s) => s.doc);
  const list = useMemo(() => {
    if (!hot) return [];
    const ids = new Set(hot.brushes);
    for (const e of doc?.entities ?? []) if (hot.entities.has(e.id)) e.brushes.forEach((b) => ids.add(b));
    return [...ids].map((id) => brushes.get(id)).filter((b) => !!b).slice(0, 3000);
  }, [hot, brushes, doc]);
  if (!hot) return null;
  const points = (doc?.entities ?? []).filter((e) => hot.entities.has(e.id) && !e.brushes.length);
  return (
    <>
      {/* Parity A: one shared mesh for all of them (a mesh per brush uploaded every brush's geometry) */}
      {list.length > 0 && <mesh geometry={sharedTriangles(hot, list as BrushGeo[])} material={HOT_MATERIAL} renderOrder={7} raycast={() => null} frustumCulled={false} />}
      {/* Parity A: the point entities in one instanced draw */}
      <InstancedMarkers
        items={points.flatMap((e) => {
          const o = e.keys.find(([k]) => k === "origin")?.[1]?.trim().split(/\s+/).map(Number);
          return o && o.length >= 3 && o.every(Number.isFinite) ? [{ at: [o[0], o[1], o[2]] as Vec3, color: "#ffffff" }] : [];
        })}
        shape="box"
        size={12}
        renderOrder={7}
        material={HOT_MATERIAL}
      />
      {hot.faces.slice(0, 800).map((f) => {
        const poly = brushes.get(f.brush)?.polys.get(f.face);
        return poly ? <Line key={`${f.brush}:${f.face}`} points={[...poly, poly[0]]} color="#5fc8ff" lineWidth={2} depthTest={false} renderOrder={7} /> : null;
      })}
    </>
  );
}

/** The select tools' options for the box. */
export function MarqueeOptions({ texture }: { texture?: boolean }) {
  const m = useMarquee();
  return (
    <>
      <label title="How a box picks: drag right for fully inside, left for touching (Auto), or always one way">
        Box
        <select value={m.mode} onChange={(e) => m.set({ mode: e.target.value as MarqueeMode })}>
          <option value="auto" title="Drag right: only what's fully inside. Drag left: anything it touches.">Auto</option>
          <option value="inside">Fully inside</option>
          <option value="touching">Touching</option>
        </select>
      </label>
      <label className="editor-check" title="In the 3D view, leave out what's hidden behind walls">
        <input type="checkbox" checked={m.onlyVisible} onChange={(e) => m.set({ onlyVisible: e.target.checked })} /> Only what you can see
      </label>
      {!texture && (
        <label className="editor-check" title="3D view: a left drag anywhere draws a box (not just on empty space); turn the camera with the right button, pan with Shift+right">
          <input type="checkbox" checked={m.anywhere3d} onChange={(e) => m.set({ anywhere3d: e.target.checked })} /> Box anywhere in 3D
        </label>
      )}
    </>
  );
}
