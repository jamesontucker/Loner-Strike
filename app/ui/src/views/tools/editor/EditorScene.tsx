// The editor's 3D view: the brushes and entities the app sent, the tools (draw a box, push
// and pull a face, clip, place entities, paint textures), the W/E/R gizmo, and the cameras:
// one 3D view first (B's Orbit/Fly cameras), flat top/front/side views a key away.
// Everything inside the root group is in GoldSrc units (plan 3.2).

import { Line, MapControls, OrthographicCamera, TransformControls } from "@react-three/drei";
import { Canvas, events, useThree, type EventManager, type RootState, type ThreeEvent } from "@react-three/fiber";
import { createContext, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import * as THREE from "three";
import { useTools } from "../../../stores/tools";
import { Cameras } from "../viewer/Cameras";
import { gsToThree, ROOT_ROTATION, threeToGs, type Vec3 } from "../viewer/coords";
import { MapMaterials } from "../viewer/materials";
import type { Draw } from "../viewer/bundle";
import { texUrl, type DocEntity } from "./editorApi";
import { keyOf, originOf, selectionBounds, snap, useEditor, type BrushGeo, type ViewId } from "./store";

// ---- small helpers ---------------------------------------------------------------------

/** Pointer events measured against the canvas on screen (the app's text size is a CSS zoom). */
function pointerEvents(store: Parameters<typeof events>[0]): EventManager<HTMLElement> {
  const base = events(store);
  return {
    ...base,
    compute(event: Event, state: RootState) {
      const el = state.gl.domElement;
      if (document.pointerLockElement === el) state.pointer.set(0, 0);
      else {
        const r = el.getBoundingClientRect();
        const e = event as PointerEvent;
        state.pointer.set(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
      }
      state.raycaster.setFromCamera(state.pointer, state.camera);
    },
  };
}

const drawOf = (texture: string): Draw => ({ model: 0, texture, start: 0, count: 0, flags: 0 });
const isSkyTex = (t: string) => /^sky$/i.test(t);
/** The axis a view looks along (GoldSrc 0 x, 1 y, 2 z). */
export const viewAxis = (v: ViewId) => (v === "side" ? 0 : v === "front" ? 1 : 2);

const add = (a: Vec3, b: Vec3): Vec3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const mul = (a: Vec3, k: number): Vec3 => [a[0] * k, a[1] * k, a[2] * k];
const dot = (a: Vec3, b: Vec3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

/** The mouse ray in GoldSrc, from a window event. */
function rayAt(e: { clientX: number; clientY: number }, state: RootState): { o: Vec3; d: Vec3 } {
  const r = state.gl.domElement.getBoundingClientRect();
  const ndc = new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
  const rc = new THREE.Raycaster();
  rc.setFromCamera(ndc, state.camera);
  return { o: threeToGs(rc.ray.origin), d: threeToGs(rc.ray.direction) };
}

/** Where a ray meets the plane `p[axis] = at`; null when it runs along it. */
function onPlane(ray: { o: Vec3; d: Vec3 }, axis: number, at: number): Vec3 | null {
  if (Math.abs(ray.d[axis]) < 1e-6) return null;
  const t = (at - ray.o[axis]) / ray.d[axis];
  if (t < 0 && Math.abs(t) > 1e6) return null;
  return add(ray.o, mul(ray.d, t));
}

/** How far along a line (p0 + t n) the mouse ray passes closest. */
function alongLine(ray: { o: Vec3; d: Vec3 }, p0: Vec3, n: Vec3): number | null {
  const w = sub(p0, ray.o);
  const b = dot(n, ray.d);
  const c = dot(ray.d, ray.d);
  const den = c - b * b;
  if (Math.abs(den) < 1e-4) return null;
  return (b * dot(ray.d, w) - c * dot(n, w)) / den;
}

const snapVec = (p: Vec3): Vec3 => [snap(p[0]), snap(p[1]), snap(p[2])];

/** Selection as the commands want it: brushes (with those of picked brush entities) and entities. */
export function selectionIds() {
  const s = useEditor.getState();
  const brushes = new Set(s.sel.brushes);
  for (const id of s.sel.entities) for (const b of s.doc?.entities.find((e) => e.id === id)?.brushes ?? []) brushes.add(b);
  return { brushes: [...brushes], entities: s.sel.entities };
}

/** What a click picks: a brush, or the brush entity it belongs to (like Hammer). */
function pickBrush(id: number, additive: boolean) {
  const s = useEditor.getState();
  const b = s.brushes.get(id);
  const ent = b?.entity ?? null;
  const cur = s.sel;
  if (ent != null) {
    const has = cur.entities.includes(ent);
    const entities = additive ? (has ? cur.entities.filter((x) => x !== ent) : [...cur.entities, ent]) : [ent];
    s.select({ brushes: additive ? cur.brushes : [], entities, face: null });
  } else {
    const has = cur.brushes.includes(id);
    const brushes = additive ? (has ? cur.brushes.filter((x) => x !== id) : [...cur.brushes, id]) : [id];
    s.select({ brushes, entities: additive ? cur.entities : [], face: null });
  }
}

function pickEntity(id: number, additive: boolean) {
  const s = useEditor.getState();
  const cur = s.sel;
  const has = cur.entities.includes(id);
  s.select({ brushes: additive ? cur.brushes : [], entities: additive ? (has ? cur.entities.filter((x) => x !== id) : [...cur.entities, id]) : [id], face: null });
}

function isPicked(b: BrushGeo, sel: { brushes: number[]; entities: number[] }) {
  return sel.brushes.includes(b.id) || (b.entity != null && sel.entities.includes(b.entity));
}

// ---- brushes --------------------------------------------------------------------------

function BrushMesh({ b, mats, picked, hideSky, version }: { b: BrushGeo; mats: MapMaterials; picked: boolean; hideSky: boolean; version: number }) {
  const materials = useMemo(() => b.textures.map((t) => mats.get(drawOf(t))), [b, mats, version]); // eslint-disable-line react-hooks/exhaustive-deps
  const edges = useMemo(() => new THREE.EdgesGeometry(b.geometry, 20), [b]);
  useEffect(() => () => edges.dispose(), [edges]);
  const skyOnly = hideSky && b.textures.every(isSkyTex);
  const tied = b.entity != null;
  return (
    <group>
      {!skyOnly && <mesh geometry={b.geometry} material={materials} userData={{ kind: "brush", id: b.id }} />}
      {skyOnly && <mesh geometry={b.geometry} userData={{ kind: "brush", id: b.id, sky: true }} visible={false} />}
      <lineSegments geometry={edges} renderOrder={picked ? 3 : 1}>
        <lineBasicMaterial color={picked ? "#ffd84a" : tied ? "#62c6d6" : "#10130d"} transparent opacity={picked ? 0.95 : 0.35} depthTest={!picked} />
      </lineSegments>
    </group>
  );
}

function Brushes({ mats, version }: { mats: MapMaterials; version: number }) {
  const brushes = useEditor((s) => s.brushes);
  const sel = useEditor((s) => s.sel);
  const preview = useEditor((s) => s.preview);
  const groups = useEditor((s) => s.doc?.groups);
  const camera = useTools((s) => s.camera);
  const view = useEditor((s) => s.view);
  const hideSky = !(view === "3d" && camera === "fly");
  const hidden = useMemo(() => new Set((groups ?? []).filter((g) => g.hidden).map((g) => g.id)), [groups]);
  const moving = useRef<THREE.Group>(null);
  useEffect(() => {
    const g = moving.current;
    if (!g) return;
    g.matrix.copy(preview ?? new THREE.Matrix4());
    g.matrixWorldNeedsUpdate = true;
  }, [preview]);
  const list = [...brushes.values()].filter((b) => b.group == null || !hidden.has(b.group));
  return (
    <>
      <group>
        {list.filter((b) => !isPicked(b, sel)).map((b) => (
          <BrushMesh key={b.id} b={b} mats={mats} picked={false} hideSky={hideSky} version={version} />
        ))}
      </group>
      <group ref={moving} matrixAutoUpdate={false}>
        {list.filter((b) => isPicked(b, sel)).map((b) => (
          <BrushMesh key={b.id} b={b} mats={mats} picked hideSky={hideSky} version={version} />
        ))}
        <PickedEntities />
      </group>
      <FaceOutline />
    </>
  );
}

/** The face picked with the texture tool, outlined. */
function FaceOutline() {
  const face = useEditor((s) => s.sel.face);
  const brushes = useEditor((s) => s.brushes);
  const pts = face ? brushes.get(face.brush)?.polys.get(face.face) : null;
  if (!pts || pts.length < 3) return null;
  return <Line points={[...pts, pts[0]]} color="#ff8f3a" lineWidth={3} depthTest={false} renderOrder={5} />;
}

// ---- entities -------------------------------------------------------------------------

const PLAYER = /^info_player_|^info_vip_start$|^hostage_entity$|^monster_scientist$/;

function entityLook(e: DocEntity): { size: Vec3; colour: string; up: number } {
  const c = e.classname;
  if (c === "info_player_start") return { size: [32, 32, 72], colour: "#5b8fd6", up: 0 };
  if (c === "info_player_deathmatch") return { size: [32, 32, 72], colour: "#d98a3c", up: 0 };
  if (PLAYER.test(c)) return { size: [32, 32, 72], colour: "#6fc27a", up: 0 };
  if (c.startsWith("light")) {
    const l = (keyOf(e.keys, "_light") ?? "255 255 200").split(/\s+/).map(Number);
    const col = l.length >= 3 && l.slice(0, 3).every(Number.isFinite) ? `rgb(${l[0]},${l[1]},${l[2]})` : "#fff3b0";
    return { size: [16, 16, 16], colour: col, up: 0 };
  }
  if (c.startsWith("ambient") || c.startsWith("env_sound")) return { size: [16, 16, 16], colour: "#b58bd8", up: 0 };
  if (c.startsWith("armoury") || c.startsWith("weapon") || c.startsWith("item")) return { size: [24, 24, 16], colour: "#cfd36a", up: 0 };
  return { size: [16, 16, 16], colour: "#c9c2a8", up: 0 };
}

function yawOf(e: DocEntity): number | null {
  const a = keyOf(e.keys, "angle");
  if (a != null && Number.isFinite(Number(a)) && Number(a) >= 0) return Number(a);
  const v = keyOf(e.keys, "angles")?.trim().split(/\s+/).map(Number);
  return v && v.length === 3 && Number.isFinite(v[1]) ? v[1] : null;
}

function EntityBox({ e, picked }: { e: DocEntity; picked: boolean }) {
  const o = originOf(e.keys);
  const look = entityLook(e);
  const yaw = yawOf(e);
  const [sx, sy, sz] = look.size;
  const outline = useMemo(() => new THREE.EdgesGeometry(new THREE.BoxGeometry(sx, sy, sz)), [sx, sy, sz]);
  useEffect(() => () => outline.dispose(), [outline]);
  if (!o) return null;
  const len = Math.max(look.size[0], 24);
  const dir: Vec3 = yaw == null ? [0, 0, 0] : [Math.cos((yaw * Math.PI) / 180) * len, Math.sin((yaw * Math.PI) / 180) * len, 0];
  return (
    <group position={o}>
      <mesh userData={{ kind: "entity", id: e.id }}>
        <boxGeometry args={look.size} />
        <meshLambertMaterial color={look.colour} transparent opacity={0.85} />
      </mesh>
      <lineSegments geometry={outline} renderOrder={picked ? 3 : 1}>
        <lineBasicMaterial color={picked ? "#ffd84a" : "#11140e"} depthTest={!picked} />
      </lineSegments>
      {yaw != null && <Line points={[[0, 0, 0], dir]} color={picked ? "#ffd84a" : look.colour} lineWidth={2} />}
    </group>
  );
}

function pointEntities(ents: DocEntity[] | undefined) {
  return (ents ?? []).filter((e) => !e.brushes.length && e.classname !== "worldspawn");
}

function Entities() {
  const ents = useEditor((s) => s.doc?.entities);
  const sel = useEditor((s) => s.sel.entities);
  const groups = useEditor((s) => s.doc?.groups);
  const hidden = useMemo(() => new Set((groups ?? []).filter((g) => g.hidden).map((g) => g.id)), [groups]);
  return (
    <>
      {pointEntities(ents)
        .filter((e) => !sel.includes(e.id) && (e.group == null || !hidden.has(e.group)))
        .map((e) => (
          <EntityBox key={e.id} e={e} picked={false} />
        ))}
    </>
  );
}

function PickedEntities() {
  const ents = useEditor((s) => s.doc?.entities);
  const sel = useEditor((s) => s.sel.entities);
  return (
    <>
      {pointEntities(ents)
        .filter((e) => sel.includes(e.id))
        .map((e) => (
          <EntityBox key={e.id} e={e} picked />
        ))}
    </>
  );
}

/** Where an entity is: its origin, or the middle of its brushes. */
function entityCentre(e: DocEntity, brushes: Map<number, BrushGeo>): Vec3 | null {
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
  return Number.isFinite(min[0]) ? mul(add(min, max), 0.5) : null;
}

/** target -> targetname links as lines (brighter for the picked entity). */
function Links() {
  const ents = useEditor((s) => s.doc?.entities);
  const brushes = useEditor((s) => s.brushes);
  const sel = useEditor((s) => s.sel.entities);
  const lines = useMemo(() => {
    const out: { a: Vec3; b: Vec3; hot: boolean }[] = [];
    if (!ents) return out;
    const named = new Map<string, DocEntity[]>();
    for (const e of ents) {
      const n = keyOf(e.keys, "targetname");
      if (n) named.set(n, [...(named.get(n) ?? []), e]);
    }
    for (const e of ents) {
      for (const k of ["target", "killtarget"]) {
        const t = keyOf(e.keys, k);
        if (!t) continue;
        const a = entityCentre(e, brushes);
        for (const to of named.get(t) ?? []) {
          const b = entityCentre(to, brushes);
          if (a && b && out.length < 2000) out.push({ a, b, hot: sel.includes(e.id) || sel.includes(to.id) });
        }
      }
    }
    return out;
  }, [ents, brushes, sel]);
  return (
    <>
      {lines.map((l, i) => (
        <Line key={i} points={[l.a, l.b]} color={l.hot ? "#ffd84a" : "#7fb7c9"} lineWidth={l.hot ? 2.5 : 1} dashed={!l.hot} dashSize={12} gapSize={8} transparent opacity={l.hot ? 1 : 0.6} depthTest={!l.hot} />
      ))}
    </>
  );
}

// ---- tools ----------------------------------------------------------------------------

type Hit = { kind: "brush" | "entity" | "plane"; id: number; face: number | null; point: Vec3; normal: Vec3 };

/** The nearest thing under the mouse that a tool cares about: a brush or an entity, else
 *  the working plane (so a ceiling above the plane still gets picked from below). */
function hitOf(e: ThreeEvent<PointerEvent>): Hit | null {
  let plane: Hit | null = null;
  for (const i of e.intersections) {
    const ud = i.object.userData as { kind?: string; id?: number; sky?: boolean };
    const point = threeToGs(i.point);
    if (ud.kind === "plane") {
      plane ??= { kind: "plane", id: 0, face: null, point, normal: [0, 0, 1] };
    } else if (ud.kind === "brush" && ud.id != null && !ud.sky) {
      const b = useEditor.getState().brushes.get(ud.id);
      const face = b && i.faceIndex != null ? b.faceOfTri[i.faceIndex] ?? null : null;
      const n = i.face?.normal;
      return { kind: "brush", id: ud.id, face, point, normal: n ? [n.x, n.y, n.z] : [0, 0, 1] };
    } else if (ud.kind === "entity" && ud.id != null) {
      return { kind: "entity", id: ud.id, face: null, point, normal: [0, 0, 1] };
    }
  }
  return plane;
}

let dragCounter = 0;

/** Pushes/pulls one face while the mouse moves: one request at a time, the latest distance wins. */
function faceDragger(brush: number, face: number) {
  const drag = `face-${Date.now()}-${++dragCounter}`;
  let sent = 0;
  let want = 0;
  let busy = false;
  let done: (() => void) | null = null;
  const pump = async () => {
    if (busy) return;
    busy = true;
    while (want !== sent) {
      const d = want;
      const r = await useEditor.getState().cmd({ op: "move_face", brush, face, distance: d, drag, lock: useEditor.getState().textureLock }, { doc: false, quiet: true });
      sent = d;
      if (r.error) useTools.getState().setHover(r.error);
    }
    busy = false;
    done?.();
  };
  return {
    to(d: number) {
      want = d;
      void pump();
    },
    async finish() {
      if (busy) await new Promise<void>((res) => (done = res));
      const s = useEditor.getState();
      if (sent !== 0) {
        await s.refreshDoc();
        s.refreshChecks();
      }
    },
  };
}

function ToolLayer({ bounds }: { bounds: [Vec3, Vec3] }) {
  const state = useThree();
  const tool = useEditor((s) => s.tool);
  const view = useEditor((s) => s.view);
  const axis = viewAxis(view);
  const [box, setBox] = useState<{ a: Vec3; b: Vec3 } | null>(null);
  const [clip, setClip] = useState<Vec3[]>([]);
  const clipRef = useRef<Vec3[]>([]);
  clipRef.current = clip;
  const [clipKeep, setClipKeep] = useState<"back" | "front" | "both">("both");
  const live = useRef(state);
  live.current = state;

  // tool changes forget half-made things
  useEffect(() => {
    setClip([]);
    setBox(null);
  }, [tool, view]);

  // clip: Tab picks which side stays, Enter cuts, Esc forgets the points
  useEffect(() => {
    if (tool !== "clip") return;
    const key = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (t && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName))) return;
      if (e.key === "Tab") {
        e.preventDefault();
        setClipKeep((k) => (k === "both" ? "back" : k === "back" ? "front" : "both"));
      } else if (e.key === "Enter") {
        void doClip();
      } else if (e.key === "Escape") setClip([]);
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });
  useEffect(() => {
    useEditor.setState({ clipInfo: tool === "clip" ? { points: clip.length, keep: clipKeep } : null });
  }, [tool, clip.length, clipKeep]);

  const clipPoints = (): [Vec3, Vec3, Vec3] | null => {
    const c = clipRef.current;
    if (c.length < 2) return null;
    const up: Vec3 = [0, 0, 0];
    up[axis] = 64;
    return [c[0], add(c[0], up), c[1]];
  };
  const doClip = async () => {
    const pts = clipPoints();
    const ids = selectionIds().brushes;
    if (!pts) return useEditor.getState().notify("Click two points to place the cut first.", true);
    if (!ids.length) return useEditor.getState().notify("Pick the brushes to cut first.", true);
    const r = await useEditor.getState().cmd({ op: "clip", brushes: ids, points: pts, keep: clipKeep, texture: useEditor.getState().texture });
    if (!r.error) setClip([]);
  };
  useEffect(() => {
    useEditor.setState({ doClip, setClipKeep });
  });

  const controlsOff = (off: boolean) => {
    const c = live.current.controls as unknown as { enabled: boolean } | null;
    if (c) c.enabled = !off;
  };

  const onDown = (e: ThreeEvent<PointerEvent>) => {
    if (e.button !== 0 || (useTools.getState().camera === "fly" && view === "3d")) return;
    e.stopPropagation();
    const hit = hitOf(e);
    const s = useEditor.getState();
    const additive = e.shiftKey || e.ctrlKey || e.metaKey;
    const ev = e.nativeEvent;
    const start = { x: ev.clientX, y: ev.clientY };

    if (tool === "box" || tool === "cylinder") {
      // footprint on the working plane: on top of the face clicked, or at 0
      let at = 0;
      if (hit && hit.kind === "brush" && Math.abs(hit.normal[axis]) > 0.99) at = hit.point[axis];
      const dirUp = hit && hit.kind === "brush" && hit.normal[axis] < -0.99 ? -1 : 1;
      const first = onPlane(rayAt(ev, live.current), axis, at);
      if (!first) return;
      const a = snapVec(first);
      a[axis] = at;
      controlsOff(true);
      let b = a;
      const height = () => s.boxHeight * dirUp;
      const shape = (p: Vec3) => {
        const q: Vec3 = [...p];
        q[axis] = at + height();
        return q;
      };
      setBox({ a, b: shape(a) });
      const move = (m: PointerEvent) => {
        const p = onPlane(rayAt(m, live.current), axis, at);
        if (!p) return;
        b = snapVec(p);
        setBox({ a, b: shape(b) });
        state.invalidate();
      };
      const up = async () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        controlsOff(false);
        setBox(null);
        const end = shape(b);
        const size = sub(end, a).map(Math.abs);
        if (size.filter((x) => x >= 1).length < 3) return;
        if (tool === "box") {
          const r = await s.cmd({ op: "add_box", min: a, max: end, texture: s.texture });
          if (r.created) s.select({ brushes: r.created.brushes, entities: [], face: null });
        } else {
          const u = [0, 1, 2].filter((i) => i !== axis);
          const centre = mul(add(a, end), 0.5);
          const radius = Math.min(size[u[0]], size[u[1]]) / 2;
          if (axis !== 2) return s.notify("Draw cylinders in the 3D or top view: they stand upright.", true);
          const base: Vec3 = [centre[0], centre[1], Math.min(a[2], end[2])];
          const r = await s.cmd({ op: "add_cylinder", base, radius, height: Math.abs(end[2] - a[2]), sides: s.sides, texture: s.texture, grid: s.alt ? 0 : 1 });
          if (r.created) s.select({ brushes: r.created.brushes, entities: [], face: null });
        }
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
      return;
    }

    if (tool === "clip") {
      const p = hit ? hit.point : onPlane(rayAt(ev, live.current), axis, 0);
      if (!p) return;
      const q = snapVec(p);
      setClip((c) => (c.length >= 2 ? [q] : [...c, q]));
      return;
    }

    if (tool === "entity") {
      if (!hit) return;
      const cls = s.entityClass;
      const player = PLAYER.test(cls);
      const off = player ? 36 : 8;
      const n = hit.kind === "brush" ? hit.normal : ([0, 0, 1] as Vec3);
      let o = add(snapVec(hit.point), mul(n, off));
      if (player && Math.abs(n[2]) < 0.5) o = add(snapVec(hit.point), mul(n, 20));
      o = o.map((x) => Math.round(x)) as Vec3;
      void s.cmd({ op: "add_entity", classname: cls, origin: o }).then((r) => {
        if (r.created) s.select({ brushes: [], entities: r.created.entities, face: null });
      });
      return;
    }

    if (tool === "texture") {
      if (!hit || hit.kind !== "brush" || hit.face == null) return;
      const b = s.brushes.get(hit.id);
      if (ev.altKey) {
        // the eyedropper: take this face's texture
        const t = b?.faces.find((f) => f.face === hit.face)?.texture;
        if (t) s.set({ texture: t });
        s.select({ brushes: [], entities: [], face: { brush: hit.id, face: hit.face } });
        return;
      }
      void s.cmd({ op: "set_texture", faces: [[hit.id, e.shiftKey ? null : hit.face]], texture: s.texture });
      s.select({ brushes: [], entities: [], face: { brush: hit.id, face: hit.face } });
      return;
    }

    // select / move / rotate / scale: click picks; on a picked brush in Select, drag pushes/pulls its face
    const b = hit?.kind === "brush" ? s.brushes.get(hit.id) : null;
    const canPush = tool === "select" && b && hit && hit.face != null && isPicked(b, s.sel) && !additive;
    let pushing: ReturnType<typeof faceDragger> | null = null;
    let moved = false;
    if (canPush) controlsOff(true);
    const n = hit?.normal ?? [0, 0, 1];
    const p0 = hit?.point ?? [0, 0, 0];
    const axisAligned = n.filter((x) => Math.abs(x) > 0.999).length === 1;
    const planeD = dot(n, p0);
    const move = (m: PointerEvent) => {
      if (!moved && Math.hypot(m.clientX - start.x, m.clientY - start.y) < 4) return;
      moved = true;
      if (!canPush || !hit || hit.face == null) return;
      pushing ??= faceDragger(hit.id, hit.face);
      let t = alongLine(rayAt(m, live.current), p0, n);
      if (t == null) t = -(m.clientY - start.y);
      // snap where the face ends up (axis-aligned faces land on the grid), not the distance
      const d = axisAligned && !useEditor.getState().alt ? snap(planeD + t) - planeD : snap(t);
      useTools.getState().setHover(`Push/pull ${Math.round(d)} units`);
      pushing.to(d);
    };
    const up = async () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      if (canPush) controlsOff(false);
      if (pushing) {
        await pushing.finish();
        useTools.getState().setHover(null);
        return;
      }
      if (moved) return; // the camera turned
      if (!hit || hit.kind === "plane") {
        if (!additive) s.select({ brushes: [], entities: [], face: null });
      } else if (hit.kind === "brush") pickBrush(hit.id, additive);
      else pickEntity(hit.id, additive);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const onMove = (e: ThreeEvent<PointerEvent>) => {
    const p = onPlane(rayAt(e.nativeEvent, live.current), axis, 0);
    if (!p) return;
    const c = p.map((x) => Math.round(x)) as Vec3;
    const old = useEditor.getState().cursor;
    if (!old || old.some((x, i) => x !== c[i])) useEditor.setState({ cursor: c });
  };

  // the working plane: catches clicks where there's nothing (big, see-through, never drawn over)
  const span = Math.max(16384, ...bounds[1].map((x, i) => Math.abs(x - bounds[0][i]) * 4));
  const planeRot: [number, number, number] = axis === 2 ? [0, 0, 0] : axis === 1 ? [Math.PI / 2, 0, 0] : [0, Math.PI / 2, 0];

  const clipPreview = clip.length >= 2 ? clipPoints() : null;
  return (
    <group onPointerDown={onDown} onPointerMove={onMove}>
      <BrushesWithMats />
      <Entities />
      <Links />
      <mesh rotation={planeRot} userData={{ kind: "plane" }} renderOrder={-1}>
        <planeGeometry args={[span, span]} />
        <meshBasicMaterial transparent opacity={0} depthWrite={false} side={THREE.DoubleSide} />
      </mesh>
      {box && <BoxPreview a={box.a} b={box.b} cylinder={tool === "cylinder"} axis={axis} />}
      {clip.map((p, i) => (
        <mesh key={i} position={p}>
          <sphereGeometry args={[4, 12, 8]} />
          <meshBasicMaterial color="#ff8f3a" depthTest={false} />
        </mesh>
      ))}
      {clipPreview && <ClipPlane pts={clipPreview} keep={clipKeep} />}
    </group>
  );
}

function BrushesWithMats() {
  const m = useContext(MatsCtx);
  return m ? <Brushes mats={m.mats} version={m.version} /> : null;
}

function BoxPreview({ a, b, cylinder, axis }: { a: Vec3; b: Vec3; cylinder: boolean; axis: number }) {
  const min = a.map((x, i) => Math.min(x, b[i])) as Vec3;
  const max = a.map((x, i) => Math.max(x, b[i])) as Vec3;
  const size = sub(max, min).map((x) => Math.max(x, 0.5)) as Vec3;
  const centre = mul(add(min, max), 0.5);
  const sides = useEditor.getState().sides;
  return (
    <group position={centre}>
      {cylinder && axis === 2 ? (
        <mesh rotation={[Math.PI / 2, 0, 0]}>
          <cylinderGeometry args={[Math.min(size[0], size[1]) / 2, Math.min(size[0], size[1]) / 2, size[2], sides, 1]} />
          <meshBasicMaterial color="#ffd84a" wireframe />
        </mesh>
      ) : (
        <mesh>
          <boxGeometry args={size} />
          <meshBasicMaterial color="#ffd84a" transparent opacity={0.25} depthWrite={false} />
        </mesh>
      )}
      <lineSegments>
        <edgesGeometry args={[new THREE.BoxGeometry(...size)]} />
        <lineBasicMaterial color="#ffd84a" depthTest={false} />
      </lineSegments>
    </group>
  );
}

function ClipPlane({ pts, keep }: { pts: [Vec3, Vec3, Vec3]; keep: string }) {
  // the plane through the points, as the core reads them: normal = (p2 - p0) x (p1 - p0)
  const [p0, p1, p2] = pts.map((p) => new THREE.Vector3(...p));
  const n = p2.clone().sub(p0).cross(p1.clone().sub(p0)).normalize();
  const q = new THREE.Quaternion().setFromUnitVectors(new THREE.Vector3(0, 0, 1), n);
  const mid = p0.clone().add(p2).multiplyScalar(0.5);
  const len = Math.max(256, p0.distanceTo(p2) * 2);
  const arrow = mid.clone().addScaledVector(n, 48);
  return (
    <group>
      <mesh position={mid} quaternion={q}>
        <planeGeometry args={[len * 4, len * 4]} />
        <meshBasicMaterial color="#ff8f3a" transparent opacity={0.18} side={THREE.DoubleSide} depthWrite={false} />
      </mesh>
      <Line points={[mid.toArray() as Vec3, arrow.toArray() as Vec3]} color={keep === "front" ? "#ffd84a" : keep === "back" ? "#555" : "#ff8f3a"} lineWidth={3} depthTest={false} />
    </group>
  );
}

// ---- materials ------------------------------------------------------------------------

type Mats = { mats: MapMaterials; version: number };
const MatsCtx = createContext<Mats | null>(null);

/** The textures of the open document (B's MapMaterials, fed from the editor's texture route). */
function Materials({ docId, children }: { docId: number; children: ReactNode }) {
  const invalidate = useThree((s) => s.invalidate);
  const [version, setVersion] = useState(0);
  const mats = useMemo(
    () => new MapMaterials({ tex: async (name) => texUrl(name), lightmap: async () => null }, { textures: true, lighting: false, triggers: true, sky: true }, () => invalidate(), 8),
    // a new document, new textures
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [docId, invalidate],
  );
  useEffect(() => {
    mats.onChange = (remade) => remade && setVersion((v) => v + 1);
    return () => mats.dispose();
  }, [mats]);
  const value = useMemo(() => ({ mats, version }), [mats, version]);
  return <MatsCtx.Provider value={value}>{children}</MatsCtx.Provider>;
}

// ---- the gizmo ------------------------------------------------------------------------

function Gizmo({ pivot }: { pivot: React.MutableRefObject<THREE.Group | null> }) {
  const tool = useEditor((s) => s.tool);
  const sel = useEditor((s) => s.sel);
  const brushes = useEditor((s) => s.brushes);
  const doc = useEditor((s) => s.doc);
  const alt = useEditor((s) => s.alt);
  const grid = useEditor((s) => s.grid);
  const [ready, setReady] = useState<THREE.Group | null>(null);
  const start = useRef<{ pos: THREE.Vector3; q: THREE.Quaternion; s: THREE.Vector3; centre: Vec3 } | null>(null);
  const dragging = useRef(false);
  const mode = tool === "move" ? "translate" : tool === "rotate" ? "rotate" : tool === "scale" ? "scale" : null;
  const bounds = useMemo(() => selectionBounds(), [sel, brushes, doc]); // eslint-disable-line react-hooks/exhaustive-deps
  const centre = bounds ? mul(add(bounds[0], bounds[1]), 0.5) : null;

  useEffect(() => {
    const p = pivot.current;
    if (!p || dragging.current || !centre) return;
    p.position.set(...centre);
    p.quaternion.identity();
    p.scale.set(1, 1, 1);
  });
  useEffect(() => setReady(pivot.current), [mode, !!centre]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!mode || !centre) return null;
  return (
    <>
      {ready && (
        <TransformControls
          object={ready}
          mode={mode}
          space="local"
          size={0.9}
          rotationSnap={alt ? null : Math.PI / 12}
          scaleSnap={alt ? null : 0.125}
          onMouseDown={() => {
            const p = pivot.current!;
            dragging.current = true;
            start.current = { pos: p.position.clone(), q: p.quaternion.clone(), s: p.scale.clone(), centre };
          }}
          onObjectChange={() => {
            const p = pivot.current;
            const st = start.current;
            if (!p || !st || !dragging.current) return;
            if (mode === "translate") {
              // snap the distance moved, not the absolute place
              const d = p.position.clone().sub(st.pos);
              p.position.copy(st.pos).add(new THREE.Vector3(snap(d.x, grid), snap(d.y, grid), snap(d.z, grid)));
            }
            const m0 = new THREE.Matrix4().compose(st.pos, st.q, st.s);
            const m1 = new THREE.Matrix4().compose(p.position, p.quaternion, p.scale);
            useEditor.setState({ preview: m1.multiply(m0.invert()) });
          }}
          onMouseUp={async () => {
            const p = pivot.current;
            const st = start.current;
            dragging.current = false;
            if (!p || !st) return;
            const ids = selectionIds();
            const s = useEditor.getState();
            let body: Record<string, unknown> | null = null;
            if (mode === "translate") {
              const d = p.position.clone().sub(st.pos);
              const t: Vec3 = [d.x, d.y, d.z].map((x) => Math.round(x * 1000) / 1000) as Vec3;
              if (t.some((x) => x !== 0)) body = { op: "transform", ...ids, translate: t, lock: s.textureLock };
            } else if (mode === "rotate") {
              const q = p.quaternion.clone().multiply(st.q.clone().invert());
              const angle = 2 * Math.acos(Math.min(1, Math.abs(q.w)));
              const sgn = q.w < 0 ? -1 : 1;
              const ax = new THREE.Vector3(q.x * sgn, q.y * sgn, q.z * sgn);
              if (ax.lengthSq() > 1e-12 && angle > 1e-4) {
                ax.normalize();
                // snap the axis to x/y/z when it's one of them (it nearly always is)
                const arr = ax.toArray().map((x) => (Math.abs(x) > 0.9999 ? Math.sign(x) : Math.abs(x) < 1e-4 ? 0 : x));
                const deg = Math.round(((angle * 180) / Math.PI) * 1000) / 1000;
                body = { op: "transform", ...ids, rotate: { axis: arr, degrees: deg }, pivot: st.centre, lock: s.textureLock };
              }
            } else {
              const f = [p.scale.x / st.s.x, p.scale.y / st.s.y, p.scale.z / st.s.z].map((x) => Math.round(x * 10000) / 10000);
              if (f.some((x) => x !== 1)) body = { op: "transform", ...ids, scale: f, pivot: st.centre, lock: s.textureLock };
            }
            if (!body) return useEditor.setState({ preview: null });
            const r = await s.cmd(body);
            if (r.error || !r.changed) useEditor.setState({ preview: null });
          }}
        />
      )}
    </>
  );
}

// ---- flat views -----------------------------------------------------------------------

function Flat({ view, bounds }: { view: Exclude<ViewId, "3d">; bounds: [Vec3, Vec3] }) {
  const size = useThree((s) => s.size);
  const camera = useThree((s) => s.camera);
  const ref = useRef<React.ElementRef<typeof MapControls>>(null);
  const [lo, hi] = bounds;
  const centre = mul(add(lo, hi), 0.5);
  const axis = viewAxis(view);
  const u = view === "side" ? 1 : 0;
  const v = view === "top" ? 1 : 2;
  const zoom = Math.min(size.width / Math.max(128, hi[u] - lo[u]), size.height / Math.max(128, hi[v] - lo[v])) * 0.85;
  const eyeGs: Vec3 = [...centre];
  eyeGs[axis] = axis === 1 ? lo[1] - 8192 : hi[axis] + 8192;
  const eye = gsToThree(eyeGs);
  const target = gsToThree(centre);
  const up = view === "top" ? new THREE.Vector3(0, 0, -1) : new THREE.Vector3(0, 1, 0);
  useEffect(() => {
    const c = ref.current;
    if (!c) return;
    c.object.up.copy(up);
    c.object.position.copy(eye);
    c.target.copy(target);
    c.object.lookAt(target);
    c.update();
    // only when the view changes: panning and zooming stay yours
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [view, camera]);
  return (
    <>
      <OrthographicCamera makeDefault position={eye} up={up} zoom={zoom} near={1} far={40000} onUpdate={(c) => c.lookAt(target)} />
      {camera instanceof THREE.OrthographicCamera && <MapControls ref={ref} makeDefault enableRotate={false} screenSpacePanning enableDamping={false} minZoom={zoom / 16} maxZoom={zoom * 60} />}
    </>
  );
}

function GridFor({ view, grid, bounds }: { view: ViewId; grid: number; bounds: [Vec3, Vec3] }) {
  const axis = viewAxis(view);
  // three's grid lies flat in its XZ (GoldSrc's floor); turned for the side views
  const rot: [number, number, number] = axis === 2 ? [0, 0, 0] : axis === 1 ? [Math.PI / 2, 0, 0] : [0, 0, Math.PI / 2];
  const span = Math.max(2048, ...bounds[1].map((x, i) => Math.abs(x - bounds[0][i]) * 2));
  // at most ~400 lines: a fine grid over a big map turns into a grey sheet
  let cell = Math.max(1, grid);
  while (span / cell > 400) cell *= 2;
  const size = Math.ceil(span / (cell * 8)) * cell * 8;
  const helper = useMemo(() => {
    const g = new THREE.GridHelper(size, size / cell, "#5d6a50", "#3a4233");
    (g.material as THREE.Material).transparent = true;
    (g.material as THREE.Material).opacity = 0.7;
    (g.material as THREE.Material).depthWrite = false;
    return g;
  }, [size, cell]);
  useEffect(() => () => helper.dispose(), [helper]);
  return <primitive object={helper} rotation={rot} renderOrder={-2} />;
}

// ---- the canvas -----------------------------------------------------------------------

export function EditorScene({ bounds, docId, onLost }: { bounds: [Vec3, Vec3]; docId: number; onLost: () => void }) {
  const view = useEditor((s) => s.view);
  const grid = useEditor((s) => s.grid);
  const pivot = useRef<THREE.Group | null>(null);
  return (
    <Canvas
      onCreated={({ gl }) => gl.domElement.addEventListener("webglcontextlost", (e) => {
        e.preventDefault();
        // a canvas being put away (view switch, new document) drops its context on purpose
        if (gl.domElement.isConnected) onLost();
      })}
      frameloop="demand"
      dpr={[1, 1.5]}
      linear
      flat
      gl={{ powerPreference: "high-performance", antialias: true }}
      camera={{ fov: 70, near: 2, far: 40000, position: [0, 512, 512] }}
      events={pointerEvents}
    >
      <color attach="background" args={["#22271e"]} />
      <hemisphereLight args={["#ffffff", "#5a5f50", 0.95]} />
      <directionalLight position={[0.4, 1, 0.25]} intensity={1.5} />
      <GridFor view={view} grid={grid} bounds={bounds} />
      <Materials docId={docId}>
        <group rotation={ROOT_ROTATION}>
          <ToolLayer bounds={bounds} />
          {/* what the gizmo moves: in GoldSrc units like everything in here */}
          <group ref={pivot} />
        </group>
      </Materials>
      <Gizmo pivot={pivot} />
      {view === "3d" ? <Cameras bounds={bounds} /> : <Flat key={view} view={view} bounds={bounds} />}
    </Canvas>
  );
}
