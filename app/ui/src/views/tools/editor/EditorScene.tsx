// The editor's 3D view: the brushes and entities the app sent, the tools (draw a box, push
// and pull a face, clip, place entities, paint textures), the W/E/R gizmo, and the cameras:
// one 3D view first (B's Orbit/Fly cameras), flat top/front/side views a key away.
// Everything inside the root group is in GoldSrc units (plan 3.2).

import { Line, MapControls, OrthographicCamera, PerspectiveCamera, TransformControls } from "@react-three/drei";
import { PaneView } from "./PaneView"; // P5: drei View with the off-screen test fixed
import { Canvas, useThree, type RootState, type ThreeEvent } from "@react-three/fiber";
import { createContext, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import * as THREE from "three";
import { shapeOutline } from "./shapePreview";
import { shapeCommand, useCurrentShape } from "./shapeParams"; // Parity A
import { useTools } from "../../../stores/tools";
import { Cameras } from "../viewer/Cameras";
import { gsToThree, ROOT_ROTATION, threeToGs, type Vec3 } from "../viewer/coords";
import { MapMaterials } from "../viewer/materials";
import { pointerEvents } from "../viewer/Scene";
import type { Draw } from "../viewer/bundle";
import { texUrl, type DocEntity } from "./editorApi";
import { facesOf, groupMembers, keyOf, originOf, selectionBounds, snap, topGroup, useEditor, type BrushGeo, type FaceRef, type ViewId } from "./store";
import { LeakLine } from "./LeakLine";
import { CheckMarkers } from "./CheckMarkers"; // Checks
import { DIM_MATERIAL, P4SceneParts } from "./P4Scene"; // P4: cordon, drag and drop, 3D box select
import { useCordonDim } from "./extras";
import { MarqueeHot, startMarquee, useMarquee } from "./Marquee";
// P5: panes (quad view): one canvas, a drei <View> per pane
import { PaneBoxes } from "./Layout";
import { paneCount, PaneCtx, usePaneActive, usePanes, usePaneView } from "./panes";
import { useP5 } from "./p5store";
import { duplicateWith } from "./commands";
import { shiftHeld, snapToGeometry } from "./snapGeo";
import { edgeLook, flatMaterial, usePaneMode } from "./renderModes";
import { EntityModelMesh, SpriteGlow, useEntityModel } from "./EntityModels";
import { MergedBrushes } from "./MergedBrushes";
import { brushRaycast } from "./pickRay";
import { KitOverlay } from "./KitOverlay"; // P5: mode kit areas and paths
import { FlatHandles, startFlatMove } from "./FlatHandles"; // Parity A: 2D handles and moves
import { FlatGrid } from "./FlatGrid"; // Parity A
import { EntityBatch, LinkBatch } from "./EntityBatch"; // Parity A: entities in a few draws
import { QuietWhileDragging, wrapRootInvalidate } from "./paneRedraw"; // Parity A: redraw only the panes that changed
import { startExtrude, startSplit } from "./extrude"; // Parity A: Shift-drag extrude, Ctrl-drag split
import { DropTarget } from "./DropTarget"; // P5: a dragged texture's face
import { TriadFeed } from "./AxisTriad"; // P5: the axes in each view's corner
import { markerColour, useKits } from "./kits";
import { useNav } from "./navigation";
import { NavCamera } from "./NavCamera";
import { Bookmarks, MeasureLine, sameP, SelectRect, TexHandles, useVisibleBrush, VertexHandles } from "./SceneExtras";

// ---- small helpers ---------------------------------------------------------------------

const drawOf = (texture: string): Draw => ({ model: 0, texture, start: 0, count: 0, flags: 0 });
const isSkyTex = (t: string) => /^sky$/i.test(t);
/** The axis a view looks along (GoldSrc 0 x, 1 y, 2 z). */
export const viewAxis = (v: ViewId) => (v === "side" ? 0 : v === "front" ? 1 : 2);

const add = (a: Vec3, b: Vec3): Vec3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const sub = (a: Vec3, b: Vec3): Vec3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const mul = (a: Vec3, k: number): Vec3 => [a[0] * k, a[1] * k, a[2] * k];
const dot = (a: Vec3, b: Vec3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];

/** The mouse ray in GoldSrc, from a window event. */
export function rayAt(e: { clientX: number; clientY: number }, state: RootState): { o: Vec3; d: Vec3 } {
  // P5: in a pane, the pane's own box (its events are connected to it), else the canvas
  const pane = state.events.connected as HTMLElement | undefined;
  const r = (pane?.dataset?.pane != null ? pane : state.gl.domElement).getBoundingClientRect();
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

// P5: a brush corner/edge nearby wins when "snap to corners and edges" is on
const snapVec = (p: Vec3): Vec3 => snapToGeometry(p) ?? [snap(p[0]), snap(p[1]), snap(p[2])];
const cross = (a: Vec3, b: Vec3): Vec3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm2 = (a: Vec3): Vec3 => {
  const l = Math.hypot(a[0], a[1], a[2]) || 1;
  return [a[0] / l, a[1] / l, a[2] / l];
};

/** Selection as the commands want it: brushes (with those of picked brush entities) and entities. */
export function selectionIds() {
  const s = useEditor.getState();
  const brushes = new Set(s.sel.brushes);
  for (const id of s.sel.entities) for (const b of s.doc?.entities.find((e) => e.id === id)?.brushes ?? []) brushes.add(b);
  return { brushes: [...brushes], entities: s.sel.entities };
}

/** Adds or toggles a whole group (Pick groups mode). */
function pickGroup(g: number, additive: boolean) {
  const s = useEditor.getState();
  const m = groupMembers(g);
  const cur = s.sel;
  const all = m.brushes.every((b) => cur.brushes.includes(b)) && m.entities.every((e) => cur.entities.includes(e));
  if (!additive) return s.select({ brushes: m.brushes, entities: m.entities, face: null });
  if (all) return s.select({ brushes: cur.brushes.filter((b) => !m.brushes.includes(b)), entities: cur.entities.filter((e) => !m.entities.includes(e)), face: null });
  s.select({ brushes: [...new Set([...cur.brushes, ...m.brushes])], entities: [...new Set([...cur.entities, ...m.entities])], face: null });
}

/** What a click picks: a brush, or the brush entity it belongs to (like Hammer), or its group. */
function pickBrush(id: number, additive: boolean) {
  const s = useEditor.getState();
  const b = s.brushes.get(id);
  const ent0 = b?.entity != null ? s.doc?.entities.find((e) => e.id === b.entity) : null;
  const g = s.selectMode === "groups" ? topGroup(ent0?.group ?? b?.group ?? null) : null;
  if (g != null) return pickGroup(g, additive);
  const ent = s.selectMode === "solids" ? null : b?.entity ?? null; // P5: "Pick brushes" takes the brush itself
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
  const g = s.selectMode === "groups" ? topGroup(s.doc?.entities.find((e) => e.id === id)?.group ?? null) : null;
  if (g != null) return pickGroup(g, additive);
  const cur = s.sel;
  const has = cur.entities.includes(id);
  s.select({ brushes: additive ? cur.brushes : [], entities: additive ? (has ? cur.entities.filter((x) => x !== id) : [...cur.entities, id]) : [id], face: null });
}

function isPicked(b: BrushGeo, sel: { brushes: number[]; entities: number[] }) {
  return sel.brushes.includes(b.id) || (b.entity != null && sel.entities.includes(b.entity));
}

// ---- brushes --------------------------------------------------------------------------

function BrushMesh({ b, mats, picked, hideSky, version, dim }: { b: BrushGeo; mats: MapMaterials; picked: boolean; hideSky: boolean; version: number; dim?: boolean }) {
  const mode = usePaneMode(); // P5: textured / flat / wire / textured + wire, per pane
  const materials = useMemo(() => (mode === "flat" ? b.textures.map(flatMaterial) : b.textures.map((t) => mats.get(drawOf(t)))), [b, mats, version, mode]); // eslint-disable-line react-hooks/exhaustive-deps
  const edges = useMemo(() => new THREE.EdgesGeometry(b.geometry, 20), [b]);
  const look = edgeLook(mode, picked, b.entity != null);
  useEffect(() => () => edges.dispose(), [edges]);
  const skyOnly = hideSky && b.textures.every(isSkyTex);
  const raycast = brushRaycast(b); // P5: picks on edges too (pickRay.ts)
  return (
    <group>
      {!skyOnly && <mesh geometry={b.geometry} material={materials} userData={{ kind: "brush", id: b.id }} visible={mode !== "wire"} raycast={raycast} />}
      {skyOnly && <mesh geometry={b.geometry} userData={{ kind: "brush", id: b.id, sky: true }} visible={false} raycast={raycast} />}
      {dim && !skyOnly && <mesh geometry={b.geometry} material={DIM_MATERIAL} renderOrder={2} raycast={() => null} />}
      <lineSegments geometry={edges} renderOrder={picked ? 3 : 1}>
        <lineBasicMaterial color={look.color} transparent opacity={look.opacity} depthTest={!picked && mode !== "wire"} />
      </lineSegments>
    </group>
  );
}

/** P5: dev builds can draw brushes one by one (?nomerge) to compare speeds. */
const NO_MERGE = import.meta.env.DEV && typeof location !== "undefined" && location.search.includes("nomerge");

function Brushes({ mats, version }: { mats: MapMaterials; version: number }) {
  const brushes = useEditor((s) => s.brushes);
  const sel = useEditor((s) => s.sel);
  const preview = useEditor((s) => s.preview);
  const groups = useEditor((s) => s.doc?.groups);
  const camera = useTools((s) => s.camera);
  const view = usePaneView(); // P5
  const hideSky = !(view === "3d" && camera === "fly");
  const visible = useVisibleBrush();
  const dim = useCordonDim(); // P4: outside the cordon
  const moving = useRef<THREE.Group>(null);
  useEffect(() => {
    const g = moving.current;
    if (!g) return;
    g.matrix.copy(preview ?? new THREE.Matrix4());
    g.matrixWorldNeedsUpdate = true;
  }, [preview]);
  void groups;
  // P5: the unpicked brushes are drawn merged (MergedBrushes); brushes dimmed by the cordon keep
  // their own mesh for the dimming. Rebuilt only when these change.
  const mode = usePaneMode();
  const cordon = useEditor((s) => s.doc?.editor?.cordon);
  const { merged, own, picked } = useMemo(() => {
    const merged: BrushGeo[] = [];
    const own: BrushGeo[] = [];
    const picked: BrushGeo[] = [];
    for (const b of brushes.values()) {
      if (!visible(b)) continue;
      if (isPicked(b, sel)) picked.push(b);
      else if (dim?.(b)) own.push(b);
      else merged.push(b);
    }
    return { merged, own, picked };
  }, [brushes, sel, visible, cordon]); // eslint-disable-line react-hooks/exhaustive-deps
  return (
    <>
      <group>
        {NO_MERGE ? (
          merged.map((b) => <BrushMesh key={b.id} b={b} mats={mats} picked={false} hideSky={hideSky} version={version} />)
        ) : (
          <MergedBrushes list={merged} mats={mats} version={version} mode={mode} hideSky={hideSky} />
        )}
        {own.map((b) => (
          <BrushMesh key={b.id} b={b} mats={mats} picked={false} hideSky={hideSky} version={version} dim />
        ))}
      </group>
      <group ref={moving} matrixAutoUpdate={false}>
        {picked.map((b) => (
          <BrushMesh key={b.id} b={b} mats={mats} picked hideSky={hideSky} version={version} />
        ))}
        <PickedEntities />
      </group>
      <FaceOutline />
    </>
  );
}

/** The faces picked with the texture tool, outlined (the sheet's face brighter). */
function FaceOutline() {
  const sel = useEditor((s) => s.sel);
  const brushes = useEditor((s) => s.brushes);
  const faces = facesOf(sel).slice(0, 400);
  return (
    <>
      {faces.map((f, i) => {
        const pts = brushes.get(f.brush)?.polys.get(f.face);
        if (!pts || pts.length < 3) return null;
        return <Line key={`${f.brush}-${f.face}`} points={[...pts, pts[0]]} color={i === 0 ? "#ff8f3a" : "#e0b060"} lineWidth={i === 0 ? 3 : 2} depthTest={false} renderOrder={5} />;
      })}
    </>
  );
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
  if (c === "ls_marker") return { size: [24, 24, 24], colour: markerColour(e), up: 0 }; // P5: its kit's colour
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
  const model = useEntityModel(e); // P5: the studio model in place of the box (View > Models)
  if (!o) return null;
  const len = Math.max(look.size[0], 24);
  const dir: Vec3 = yaw == null ? [0, 0, 0] : [Math.cos((yaw * Math.PI) / 180) * len, Math.sin((yaw * Math.PI) / 180) * len, 0];
  return (
    <group position={o}>
      <mesh userData={{ kind: "entity", id: e.id }} visible={!model}>
        <boxGeometry args={look.size} />
        <meshLambertMaterial color={look.colour} transparent opacity={0.85} />
      </mesh>
      {model && <EntityModelMesh got={model} yaw={yaw ?? 0} body={Number(keyOf(e.keys, "body") ?? 0) || 0} skin={Number(keyOf(e.keys, "skin") ?? 0) || 0} />}
      <SpriteGlow e={e} />
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
  const hiddenHere = useEditor((s) => s.hidden.entities);
  useKits((k) => k.kits); // P5: markers take their kit's colour when the kits arrive
  const hidden = useMemo(() => new Set((groups ?? []).filter((g) => g.hidden).map((g) => g.id)), [groups]);
  // Parity A: drawn batched (a few draws for all of them); the picked ones stay EntityBox
  const list = useMemo(() => pointEntities(ents).filter((e) => !sel.includes(e.id) && !hiddenHere.has(e.id) && (e.group == null || !hidden.has(e.group))), [ents, sel, hiddenHere, hidden]);
  return <EntityBatch ents={list} look={entityLook} yaw={yawOf} />;
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
  // Parity A: the ordinary links in one dashed draw; the picked entity's few stay thick lines
  const cold = useMemo(() => lines.filter((l) => !l.hot), [lines]);
  return (
    <>
      <LinkBatch lines={cold} />
      {lines.filter((l) => l.hot).map((l, i) => (
        <Line key={i} points={[l.a, l.b]} color={l.hot ? "#ffd84a" : "#7fb7c9"} lineWidth={l.hot ? 2.5 : 1} dashed={!l.hot} dashSize={12} gapSize={8} transparent opacity={l.hot ? 1 : 0.6} depthTest={!l.hot} />
      ))}
    </>
  );
}

// ---- tools ----------------------------------------------------------------------------

type Hit = { kind: "brush" | "entity" | "plane"; id: number; face: number | null; point: Vec3; normal: Vec3 };

/** The nearest thing under the mouse that a tool cares about: a brush or an entity, else
 *  the working plane (so a ceiling above the plane still gets picked from below). */
type Handle = { kind: string; id: number; face?: number; p?: Vec3; ps?: Vec3[] };
/** A handle under the mouse (vertex corners, texture knobs): they're drawn on top, so they win. */
function handleOf(e: ThreeEvent<PointerEvent>): Handle | null {
  for (const i of e.intersections) {
    const ud = i.object.userData as Handle;
    if (ud.kind === "vert" || ud.kind?.startsWith("tex-")) return ud;
  }
  return null;
}

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

/** Sends the latest value of a drag, one request at a time (vertex moves, texture drags). */
function pump<T>(send: (v: T, drag: string) => Promise<{ error?: string }>, same: (a: T, b: T) => boolean, zero: T) {
  const drag = `drag-${Date.now()}-${++dragCounter}`;
  let sent = zero;
  let want = zero;
  let busy = false;
  let done: (() => void) | null = null;
  let error: string | null = null;
  const run = async () => {
    if (busy) return;
    busy = true;
    while (!same(want, sent)) {
      const v = want;
      const r = await send(v, drag);
      sent = v;
      error = r.error ?? null;
      if (r.error) useTools.getState().setHover(r.error);
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
      return { moved: !same(sent, zero), error };
    },
  };
}

/** A point on the plane through `p0` with normal `n`, under the mouse. */
function onAnyPlane(ray: { o: Vec3; d: Vec3 }, p0: Vec3, n: Vec3): Vec3 | null {
  const den = dot(ray.d, n);
  if (Math.abs(den) < 1e-6) return null;
  const t = dot(sub(p0, ray.o), n) / den;
  return add(ray.o, mul(ray.d, t));
}

function ToolLayer({ bounds }: { bounds: [Vec3, Vec3] }) {
  const state = useThree();
  const tool = useEditor((s) => s.tool);
  const view = usePaneView(); // P5: this pane's view
  const active = usePaneActive(); // P5: only the active pane answers clip keys
  const axis = viewAxis(view);
  const [box, setBox] = useState<{ a: Vec3; b: Vec3 } | null>(null);
  const [rect, setRect] = useState<{ a: Vec3; b: Vec3 } | null>(null);
  const [clip, setClip] = useState<Vec3[]>([]);
  const clipRef = useRef<Vec3[]>([]);
  clipRef.current = clip;
  const [clipKeep, setClipKeep] = useState<"back" | "front" | "both">("both");
  // P5: the clip's next point follows the cursor, so the cut shows before the click; in the 3D
  // view a third point tilts the plane (any angle), in the flat views two points cut straight
  // through the view, as in Hammer
  const [clipHover, setClipHover] = useState<Vec3 | null>(null);
  const clipMax = view === "3d" ? 3 : 2;
  const live = useRef(state);
  live.current = state;

  // tool changes forget half-made things
  useEffect(() => {
    setClip([]);
    setBox(null);
    setRect(null);
    if (tool !== "measure") useEditor.setState({ measure: null });
    if (tool !== "vertex") useEditor.setState({ verts: [] });
  }, [tool, view]);

  // clip: Tab picks which side stays, Enter cuts, Esc forgets the points
  useEffect(() => {
    if (tool !== "clip" || !active) return;
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
    if (active) useEditor.setState({ clipInfo: tool === "clip" ? { points: clip.length, keep: clipKeep, max: clipMax } : null });
  }, [tool, clip.length, clipKeep, active, clipMax]);

  const clipPoints = (c: Vec3[] = clipRef.current): [Vec3, Vec3, Vec3] | null => {
    if (c.length < 2) return null;
    if (c.length >= 3) {
      // three points on the brushes: the core's normal is (p2 - p0) x (p1 - p0); a degenerate
      // (in-line) third point falls back to the straight cut
      const n = cross(sub(c[2], c[0]), sub(c[1], c[0]));
      if (Math.hypot(...n) > 1e-6) return [c[0], c[1], c[2]];
    }
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
    if (active) useEditor.setState({ doClip, setClipKeep });
  });

  const controlsOff = (off: boolean) => {
    const c = live.current.controls as unknown as { enabled: boolean } | null;
    if (c) c.enabled = !off;
  };

  const onDown = (e: ThreeEvent<PointerEvent>) => {
    // P5: Hammer's face painting with the Texture tool: right-click paints the face with the
    // current texture, Alt+right-click also carries the picked face's alignment onto it. Only a
    // click (no drag), so right-drag still looks around.
    if (e.button === 2 && tool === "texture") {
      const hit = hitOf(e);
      if (!hit || hit.kind !== "brush" || hit.face == null) return;
      e.stopPropagation(); // (once, not for every object under the pointer)
      const ev = e.nativeEvent;
      // (the Unreal key set locks the pointer on a right press, so clientX stands still: the
      // movement is summed instead, and a click is short)
      const start = { t: performance.now(), alt: ev.altKey, moved: 0 };
      const target: [number, number] = [hit.id, hit.face];
      const move = (m: PointerEvent) => (start.moved += Math.abs(m.movementX) + Math.abs(m.movementY));
      const up = (u: PointerEvent) => {
        window.removeEventListener("pointerup", up, true);
        window.removeEventListener("pointermove", move, true);
        if (u.button !== 2 || start.moved > 4 || performance.now() - start.t > 450) return;
        const s = useEditor.getState();
        const src = s.sel.face;
        if ((start.alt || u.altKey) && src && !(src.brush === target[0] && src.face === target[1])) {
          void s.cmd({ op: "wrap_texture", from: [src.brush, src.face], faces: [target], texture: s.texture });
        } else {
          void s.cmd({ op: "set_texture", faces: [target], texture: s.texture });
        }
      };
      window.addEventListener("pointermove", move, true);
      window.addEventListener("pointerup", up, true);
      return;
    }
    if (e.button !== 0 || (useTools.getState().camera === "fly" && view === "3d")) return;
    e.stopPropagation();
    const hit = hitOf(e);
    const s = useEditor.getState();
    const additive = e.shiftKey || e.ctrlKey || e.metaKey;
    const ev = e.nativeEvent;
    const start = { x: ev.clientX, y: ev.clientY };

    const handle = handleOf(e);

    // Parity A: Ctrl/⌘-drag an edge's or face's middle splits it (a new corner)
    if (tool === "vertex" && handle?.kind === "vert" && handle.p && (handle.ps?.length ?? 0) > 1 && (e.ctrlKey || e.metaKey) && !e.shiftKey) {
      return startSplit(ev, live.current, handle.id, handle.p, view === "3d" ? null : axis, controlsOff);
    }
    if (tool === "vertex" && handle?.kind === "vert" && handle.p) {
      // click picks a corner (Shift adds); dragging moves the picked corners with snapping.
      // P4: edge and face handles pick all their corners at once
      const p = handle.p;
      const ps: Vec3[] = handle.ps?.length ? handle.ps : [p];
      const isPick = (v: { brush: number; p: Vec3 }) => v.brush === handle.id && ps.some((q) => sameP(v.p, q));
      const has = ps.every((q) => s.verts.some((v) => v.brush === handle.id && sameP(v.p, q)));
      let picks = s.verts;
      if (additive) picks = has ? s.verts.filter((v) => !isPick(v)) : [...s.verts.filter((v) => !isPick(v)), ...ps.map((q) => ({ brush: handle.id, p: q }))];
      else if (!has) picks = ps.map((q) => ({ brush: handle.id, p: q }));
      s.set({ verts: picks });
      if (additive && has) return;
      controlsOff(true);
      const n: Vec3 = view === "3d" ? [0, 0, 1] : (([0, 1, 2].map((i) => (i === axis ? 1 : 0)) as unknown) as Vec3);
      const startAt = onAnyPlane(rayAt(ev, live.current), p, n);
      const sender = pump<Vec3>(
        (d, drag) => useEditor.getState().cmd({ op: "move_vertices", picks: picks.map((v) => [v.brush, v.p]), delta: d, drag }, { doc: false, quiet: true }),
        (a, b) => sameP(a, b),
        [0, 0, 0],
      );
      let last: Vec3 = [0, 0, 0];
      const move = (m: PointerEvent) => {
        if (!startAt) return;
        const q = onAnyPlane(rayAt(m, live.current), p, n);
        if (!q) return;
        // the dragged corner lands on the grid (an edge or face moves by whole grid steps)
        const target = ps.length > 1 ? add(p, snapVec(sub(q, startAt))) : snapVec(add(p, sub(q, startAt)));
        for (let i = 0; i < 3; i++) if (n[i]) target[i] = p[i];
        last = sub(target, p);
        useTools.getState().setHover(`Corner to ${target.map((x) => Math.round(x * 100) / 100).join(" ")}`);
        sender.to(last);
      };
      const up = async () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        controlsOff(false);
        const r = await sender.finish();
        useTools.getState().setHover(null);
        if (r.moved && !r.error) {
          useEditor.setState({ verts: picks.map((v) => ({ brush: v.brush, p: add(v.p, last) })) });
          await useEditor.getState().refreshDoc();
          useEditor.getState().refreshChecks();
        } else if (r.error) useEditor.getState().notify(r.error, true);
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
      return;
    }

    if (handle?.kind?.startsWith("tex-") && handle.face != null) {
      // texture handles: the arrow ends stretch, the ring turns (round the face's middle)
      const b = s.brushes.get(handle.id);
      const f = b?.faces.find((x) => x.face === handle.face);
      const poly = b?.polys.get(handle.face);
      const al = f?.align;
      if (!b || !f || !poly || !al) return;
      controlsOff(true);
      const c = mul(poly.reduce((acc, q) => add(acc, q), [0, 0, 0] as Vec3), 1 / poly.length);
      const n = f.normal;
      const start = onAnyPlane(rayAt(ev, live.current), c, n);
      const faces = facesOf(s.sel).map((x) => [x.brush, x.face]);
      const one = s.treatAsOne;
      const u: Vec3 = [al[0], al[1], al[2]];
      const vax: Vec3 = [al[4], al[5], al[6]];
      const [su0, sv0] = [al[9], al[10]];
      const kind = handle.kind;
      let sentRot = 0;
      const sender = pump<number>(
        (x, drag) => {
          // turning is relative: send what's new since the last step; scale is a plain number
          const body = kind === "tex-rot" ? { op: "align", faces, action: "rotate", degrees: x - sentRot, drag, one } : { op: "align", faces, action: "set", ...(kind === "tex-su" ? { su: x } : { sv: x }), drag, one };
          if (kind === "tex-rot") sentRot = x;
          return useEditor.getState().cmd(body, { doc: false, quiet: true });
        },
        (a, b2) => Math.abs(a - b2) < 1e-6,
        kind === "tex-rot" ? 0 : kind === "tex-su" ? su0 : sv0,
      );
      const move = (m: PointerEvent) => {
        if (!start) return;
        const q = onAnyPlane(rayAt(m, live.current), c, n);
        if (!q) return;
        const d0 = sub(start, c);
        const d1 = sub(q, c);
        if (kind === "tex-rot") {
          let ang = (Math.atan2(dot(n, cross(d0, d1)), dot(d0, d1)) * 180) / Math.PI;
          if (!useEditor.getState().alt) ang = Math.round(ang / 15) * 15;
          useTools.getState().setHover(`Turn ${Math.round(ang)}°`);
          sender.to(ang);
        } else {
          const axisV = kind === "tex-su" ? u : vax;
          const a0 = dot(d0, axisV) / (Math.hypot(...axisV) || 1);
          const a1 = dot(d1, axisV) / (Math.hypot(...axisV) || 1);
          if (Math.abs(a0) < 1) return;
          let k = (kind === "tex-su" ? su0 : sv0) * (a1 / a0);
          k = useEditor.getState().alt ? Math.round(k * 1000) / 1000 : Math.round(k * 20) / 20;
          if (Math.abs(k) < 0.05) k = k < 0 ? -0.05 : 0.05;
          useTools.getState().setHover(`Scale ${k}`);
          sender.to(k);
        }
      };
      const up = async () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        controlsOff(false);
        await sender.finish();
        useTools.getState().setHover(null);
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
      return;
    }

    if (tool === "measure") {
      const first = hit ? hit.point : onPlane(rayAt(ev, live.current), axis, 0);
      if (!first) return;
      const a = snapVec(first);
      controlsOff(true);
      useEditor.setState({ measure: { a, b: a } });
      const move = (m: PointerEvent) => {
        const ray = rayAt(m, live.current);
        // along the view's plane through the first point (or onto what's under the mouse)
        const q = onAnyPlane(ray, a, view === "3d" ? (Math.abs(ray.d[2]) > 0.3 ? [0, 0, 1] : norm2([ray.d[0], ray.d[1], 0])) : (([0, 1, 2].map((i) => (i === axis ? 1 : 0)) as unknown) as Vec3));
        if (q) useEditor.setState({ measure: { a, b: snapVec(q) } });
        state.invalidate();
      };
      const up = () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        controlsOff(false);
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
      return;
    }

    if (tool === "box" || tool === "shape") {
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
        } else if (s.shape !== "cylinder") {
          if (axis !== 2 && s.shape !== "wedge") return s.notify("Draw this shape in the 3D or top view: it stands upright.", true);
          const min = a.map((x, i) => Math.min(x, end[i]));
          const max = a.map((x, i) => Math.max(x, end[i]));
          const r = await s.cmd({ op: "add_shape", ...shapeCommand(), min, max, texture: s.texture, grid: s.alt ? 0 : 1 }); // Parity A: every shape setting
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
      setClip((c) => (c.length >= clipMax ? [q] : [...c, q]));
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
      if (!hit || hit.kind !== "brush" || hit.face == null) {
        // P4: a box from empty space picks faces
        if (!(view === "3d" && useTools.getState().camera === "fly")) {
          startMarquee(ev, live.current, { faces: true, controlsOff, onClick: () => !additive && s.select({ brushes: [], entities: [], face: null, faces: [] }) });
        }
        return;
      }
      const b = s.brushes.get(hit.id);
      const ref: FaceRef = { brush: hit.id, face: hit.face };
      if (ev.altKey || useP5.getState().eyedrop) {
        // the eyedropper: take this face's texture (Alt+click, or the face sheet's Pick up)
        useP5.getState().set({ eyedrop: false });
        const t = b?.faces.find((f) => f.face === hit.face)?.texture;
        if (t) s.set({ texture: t });
        s.select({ brushes: [], entities: [], face: ref, faces: [ref] });
        return;
      }
      const cur = facesOf(s.sel);
      const picked = cur.some((f) => f.brush === ref.brush && f.face === ref.face);
      if (s.texClick === "paint") {
        void s.cmd({ op: "set_texture", faces: [[hit.id, e.shiftKey ? null : hit.face]], texture: s.texture });
        s.select({ brushes: [], entities: [], face: ref, faces: [ref] });
        return;
      }
      if (additive) {
        const faces = picked ? cur.filter((f) => !(f.brush === ref.brush && f.face === ref.face)) : [...cur, ref];
        s.select({ brushes: [], entities: [], face: faces[0] ?? null, faces });
        return;
      }
      if (!picked) s.select({ brushes: [], entities: [], face: ref, faces: [ref] });
      // dragging a picked face slides its texture (all picked faces), snapped to the grid
      const f = b?.faces.find((x) => x.face === hit.face);
      const al = f?.align;
      if (!f || !al) return;
      controlsOff(true);
      const faces = (picked ? cur : [ref]).map((x) => [x.brush, x.face]);
      const p0 = hit.point;
      const n = f.normal;
      const u: Vec3 = [al[0], al[1], al[2]];
      const vax: Vec3 = [al[4], al[5], al[6]];
      const [su, sv] = [al[9] || 1, al[10] || 1];
      let sentUV: [number, number] = [0, 0];
      const sender = pump<[number, number]>(
        (uv, drag) => {
          const step = { op: "align", faces, action: "shift", u: uv[0] - sentUV[0], v: uv[1] - sentUV[1], drag };
          sentUV = uv;
          return useEditor.getState().cmd(step, { doc: false, quiet: true });
        },
        (x, y) => x[0] === y[0] && x[1] === y[1],
        [0, 0],
      );
      const move = (m: PointerEvent) => {
        if (Math.hypot(m.clientX - start.x, m.clientY - start.y) < 3) return;
        const q = onAnyPlane(rayAt(m, live.current), p0, n);
        if (!q) return;
        const d = sub(q, p0);
        // the texture follows the mouse: its offset goes the other way
        let du = -dot(d, u) / su;
        let dv = -dot(d, vax) / sv;
        if (!useEditor.getState().alt) {
          const g = useEditor.getState().grid;
          du = Math.round(du / g) * g;
          dv = Math.round(dv / g) * g;
        } else {
          du = Math.round(du);
          dv = Math.round(dv);
        }
        useTools.getState().setHover(`Slide ${du}, ${dv}`);
        sender.to([du, dv]);
      };
      const up = async () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        controlsOff(false);
        await sender.finish();
        useTools.getState().setHover(null);
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
      return;
    }

    // select / move / rotate / scale: click picks; on a picked brush in Select, drag pushes/pulls its face
    const b = hit?.kind === "brush" ? s.brushes.get(hit.id) : null;
    const canPush = tool === "select" && b && hit && hit.face != null && isPicked(b, s.sel) && !additive;
    // Parity A: Shift-drag a face of a picked brush pulls out a new brush (TrenchBroom's extrude)
    if (tool === "select" && view === "3d" && b && hit && hit.face != null && isPicked(b, s.sel) && e.shiftKey && !e.ctrlKey && !e.metaKey && useTools.getState().camera !== "fly") {
      return startExtrude(ev, live.current, { id: hit.id, face: hit.face, point: hit.point, normal: hit.normal }, controlsOff, () => pickBrush(hit.id, true));
    }
    // P4: dragging from empty space draws a box (every view; see Marquee.tsx). In flat views and
    // in 3D with "Box anywhere", from anywhere but a picked brush (that's push/pull).
    const flyNow = view === "3d" && useTools.getState().camera === "fly";
    // (flat views, like Hammer: from anywhere but a picked brush; a click still picks what's under it)
    const anywhere = (view !== "3d" || useMarquee.getState().anywhere3d) && !canPush;
    if (!flyNow && (!hit || hit.kind === "plane" || anywhere) && (tool === "select" || tool === "move" || tool === "rotate" || tool === "scale" || tool === "vertex")) {
      startMarquee(ev, live.current, {
        faces: false,
        controlsOff,
        onClick: () => {
          if (!hit || hit.kind === "plane") {
            if (!additive) s.select({ brushes: [], entities: [], face: null });
          } else if (hit.kind === "brush") pickBrush(hit.id, additive);
          else pickEntity(hit.id, additive);
        },
      });
      return;
    }
    // Parity A: in a flat view, dragging a picked brush moves the selection (Hammer), a click still picks
    if (canPush && view !== "3d") {
      return startFlatMove(ev, live.current, view, () => pickBrush(hit.id, additive));
    }
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
    if (tool === "clip" && clip.length >= 1 && clip.length < clipMax) {
      const h = hitOf(e);
      const p = h ? h.point : onPlane(rayAt(e.nativeEvent, live.current), axis, 0);
      if (p) {
        const q = snapVec(p);
        if (!clipHover || q.some((x, i) => x !== clipHover[i])) setClipHover(q);
      }
    } else if (clipHover) setClipHover(null);
    const p = onPlane(rayAt(e.nativeEvent, live.current), axis, 0);
    if (!p) return;
    const c = p.map((x) => Math.round(x)) as Vec3;
    const old = useEditor.getState().cursor;
    if (!old || old.some((x, i) => x !== c[i])) useEditor.setState({ cursor: c });
  };

  // the working plane: catches clicks where there's nothing (big, see-through, never drawn over)
  const span = Math.max(16384, ...bounds[1].map((x, i) => Math.abs(x - bounds[0][i]) * 4));
  const planeRot: [number, number, number] = axis === 2 ? [0, 0, 0] : axis === 1 ? [Math.PI / 2, 0, 0] : [0, Math.PI / 2, 0];

  const clipShown = clipHover && clip.length >= 1 && clip.length < clipMax ? [...clip, clipHover] : clip;
  const clipPreview = clipShown.length >= 2 ? clipPoints(clipShown) : null;
  return (
    <group onPointerDown={onDown} onPointerMove={onMove}>
      <BrushesWithMats />
      <Entities />
      <Links />
      <mesh rotation={planeRot} userData={{ kind: "plane" }} renderOrder={-1}>
        <planeGeometry args={[span, span]} />
        <meshBasicMaterial transparent opacity={0} depthWrite={false} side={THREE.DoubleSide} />
      </mesh>
      {box && <BoxPreview a={box.a} b={box.b} shape={tool === "shape"} axis={axis} />}
      {rect && <SelectRect a={rect.a} b={rect.b} axis={axis} />}
      {tool === "vertex" && <VertexHandles ids={selectionIds().brushes} />}
      <TexHandles />
      <MeasureLine />
      <KitOverlay />
      <DropTarget />
      <LeakLine />
      <CheckMarkers /> {/* Checks: the live checks' findings */}
      <P4SceneParts />
      <MarqueeHot />
      {clip.map((p, i) => (
        <mesh key={i} position={p}>
          <sphereGeometry args={[4, 12, 8]} />
          <meshBasicMaterial color="#ff8f3a" depthTest={false} />
        </mesh>
      ))}
      {clipPreview && <ClipPlane pts={clipPreview} keep={clipKeep} />}
      {clipPreview && <ClipCut pts={clipPreview} ids={selectionIds().brushes} />}
      {clipHover && clip.length >= 1 && clip.length < clipMax && (
        <mesh position={clipHover}>
          <sphereGeometry args={[3, 10, 6]} />
          <meshBasicMaterial color="#ff8f3a" transparent opacity={0.6} depthTest={false} />
        </mesh>
      )}
    </group>
  );
}

function BrushesWithMats() {
  const m = useContext(MatsCtx);
  return m ? <Brushes mats={m.mats} version={m.version} /> : null;
}

function BoxPreview({ a, b, shape, axis }: { a: Vec3; b: Vec3; shape: boolean; axis: number }) {
  const min = a.map((x, i) => Math.min(x, b[i])) as Vec3;
  const max = a.map((x, i) => Math.max(x, b[i])) as Vec3;
  const size = sub(max, min).map((x) => Math.max(x, 0.5)) as Vec3;
  const centre = mul(add(min, max), 0.5);
  const s = useEditor.getState();
  const params = useCurrentShape(); // Parity A: every shape setting (cone top, tube sides, add height too)
  // shapes stand upright: drawn in a side view (other than a wedge) the core refuses them, so
  // the ghost stays a red box there instead of pretending
  const upright = axis === 2 || s.shape === "wedge";
  const segs = useMemo(
    () => (shape && upright ? shapeOutline(min, max, params) : null),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [shape, upright, ...min, ...max, ...Object.values(params)],
  );
  const colour = shape && !upright ? "#e05a4a" : "#ffd84a";
  return (
    <>
      <group position={centre}>
        {!segs && (
          <mesh>
            <boxGeometry args={size} />
            <meshBasicMaterial color={colour} transparent opacity={0.25} depthWrite={false} />
          </mesh>
        )}
        <lineSegments>
          <edgesGeometry args={[new THREE.BoxGeometry(...size)]} />
          <lineBasicMaterial color={colour} depthTest={false} transparent opacity={segs ? 0.3 : 1} />
        </lineSegments>
      </group>
      {segs && (
        <lineSegments renderOrder={5}>
          <bufferGeometry>
            <bufferAttribute attach="attributes-position" args={[new Float32Array(segs.flat(2)), 3]} />
          </bufferGeometry>
          <lineBasicMaterial color={colour} depthTest={false} />
        </lineSegments>
      )}
    </>
  );
}

/** Where the cut goes through the picked brushes: the plane against each face's edges. */
function ClipCut({ pts, ids }: { pts: [Vec3, Vec3, Vec3]; ids: number[] }) {
  const brushes = useEditor((s) => s.brushes);
  const segs = useMemo(() => {
    const [p0, p1, p2] = pts;
    const nRaw = cross(sub(p2, p0), sub(p1, p0));
    const len = Math.hypot(...nRaw);
    if (len < 1e-9) return new Float32Array();
    const n = mul(nRaw, 1 / len);
    const d = dot(n, p0);
    const out: number[] = [];
    for (const id of ids.slice(0, 512)) {
      const b = brushes.get(id);
      if (!b) continue;
      for (const poly of b.polys.values()) {
        const cut: Vec3[] = [];
        for (let i = 0; i < poly.length; i++) {
          const a = poly[i];
          const c = poly[(i + 1) % poly.length];
          const da = dot(n, a) - d;
          const dc = dot(n, c) - d;
          if ((da < 0 && dc > 0) || (da > 0 && dc < 0)) cut.push(add(a, mul(sub(c, a), da / (da - dc))));
          else if (Math.abs(da) < 1e-6) cut.push(a);
        }
        if (cut.length >= 2) out.push(...cut[0], ...cut[1]);
      }
    }
    return new Float32Array(out);
  }, [pts, ids, brushes]);
  if (!segs.length) return null;
  return (
    <lineSegments renderOrder={6}>
      <bufferGeometry>
        <bufferAttribute attach="attributes-position" args={[segs, 3]} />
      </bufferGeometry>
      <lineBasicMaterial color="#ff8f3a" depthTest={false} />
    </lineSegments>
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
  const copying = useRef<Promise<void> | null>(null); // P5
  const copyKey = useRef<string | null>(null); // P5: joins the copy and its move in one undo step
  const rotSnap = useP5((x) => x.rotSnap); // P5
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
          rotationSnap={alt || !rotSnap ? null : (rotSnap * Math.PI) / 180} /* P5: Grid and snapping */
          scaleSnap={alt ? null : 0.125}
          onMouseDown={() => {
            const p = pivot.current!;
            dragging.current = true;
            // P5: Shift-drag copies: the copies move, the originals stay
            copyKey.current = shiftHeld() ? `copy-${Date.now()}` : null;
            copying.current = copyKey.current ? duplicateWith([0, 0, 0], copyKey.current) : null;
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
            if (copying.current) await copying.current; // P5
            copying.current = null;
            const merge = copyKey.current ? { merge: copyKey.current } : {}; // P5: one "Copy" step with the copy
            copyKey.current = null;
            if (!p || !st) return;
            const ids = selectionIds();
            const s = useEditor.getState();
            let body: Record<string, unknown> | null = null;
            if (mode === "translate") {
              const d = p.position.clone().sub(st.pos);
              const t: Vec3 = [d.x, d.y, d.z].map((x) => Math.round(x * 1000) / 1000) as Vec3;
              if (t.some((x) => x !== 0)) body = { op: "transform", ...ids, translate: t, lock: s.textureLock, ...merge };
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
                body = { op: "transform", ...ids, rotate: { axis: arr, degrees: deg }, pivot: st.centre, lock: s.textureLock, ...merge };
              }
            } else {
              const f = [p.scale.x / st.s.x, p.scale.y / st.s.y, p.scale.z / st.s.z].map((x) => Math.round(x * 10000) / 10000);
              if (f.some((x) => x !== 1)) body = { op: "transform", ...ids, scale: f, pivot: st.centre, lock: s.textureLock, ...merge };
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
  // P5: the first zoom only (a pane's size can read 0 while panes move; refits come from frameFlat)
  const zoom0 = Math.min(size.width / Math.max(128, hi[u] - lo[u]), size.height / Math.max(128, hi[v] - lo[v])) * 0.85;
  const [zoom] = useState(() => (Number.isFinite(zoom0) && zoom0 > 0 ? zoom0 : 0.5));
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
  // P5: zoom to a box when asked (frame selection / frame all), and follow "look at" requests
  const frame = usePanes((s) => s.frame);
  const lookReq = useTools((s) => s.lookAt);
  const invalidate = useThree((s) => s.invalidate);
  const fit = (min: Vec3, max: Vec3, zoomToo: boolean) => {
    const c = ref.current;
    if (!c || !(c.object instanceof THREE.OrthographicCamera)) return;
    const mid = mul(add(min, max), 0.5);
    const t = gsToThree(mid);
    const off = c.object.position.clone().sub(c.target);
    c.target.copy(t);
    c.object.position.copy(t).add(off);
    if (zoomToo) {
      const el = (c as unknown as { domElement?: HTMLElement }).domElement;
      const w = el?.clientWidth || size.width;
      const h = el?.clientHeight || size.height;
      c.object.zoom = Math.min(w / Math.max(64, max[u] - min[u]), h / Math.max(64, max[v] - min[v])) * 0.8;
      c.object.updateProjectionMatrix();
    }
    c.update();
    invalidate();
  };
  useEffect(() => {
    if (frame) fit(frame.min, frame.max, true);
  }, [frame]); // eslint-disable-line react-hooks/exhaustive-deps
  // P5: fit the map to this pane once its controls are up and its box is known
  const ctl = useThree((s) => s.controls);
  useEffect(() => {
    const id = requestAnimationFrame(() => fit(lo, hi, true));
    return () => cancelAnimationFrame(id);
  }, [ctl]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (lookReq) fit(lookReq.point, lookReq.point, false);
  }, [lookReq]); // eslint-disable-line react-hooks/exhaustive-deps
  return (
    <>
      <OrthographicCamera makeDefault position={eye} up={up} zoom={zoom} near={1} far={40000} onUpdate={(c) => c.lookAt(target)} />
      {camera instanceof THREE.OrthographicCamera && <MapControls ref={ref} makeDefault enableRotate={false} zoomToCursor screenSpacePanning enableDamping={false} minZoom={zoom / 16} maxZoom={zoom * 60} mouseButtons={{ LEFT: -1 as unknown as THREE.MOUSE, MIDDLE: THREE.MOUSE.PAN, RIGHT: THREE.MOUSE.PAN }} />}
    </>
  );
}

function GridFor({ view, grid, bounds, major }: { view: ViewId; grid: number; bounds: [Vec3, Vec3]; major?: boolean }) {
  const axis = viewAxis(view);
  // three's grid lies flat in its XZ (GoldSrc's floor); turned for the side views
  const rot: [number, number, number] = axis === 2 ? [0, 0, 0] : axis === 1 ? [Math.PI / 2, 0, 0] : [0, 0, Math.PI / 2];
  const span = Math.max(2048, ...bounds[1].map((x, i) => Math.abs(x - bounds[0][i]) * 2));
  // at most ~400 lines: a fine grid over a big map turns into a grey sheet
  let cell = Math.max(1, grid);
  while (span / cell > 400) cell *= 2;
  const size = Math.ceil(span / (cell * 8)) * cell * 8;
  const helper = useMemo(() => {
    const g = new THREE.GridHelper(size, size / cell, major ? "#8a9a74" : "#5d6a50", major ? "#56634a" : "#3a4233");
    (g.material as THREE.Material).transparent = true;
    (g.material as THREE.Material).opacity = major ? 0.8 : 0.7;
    (g.material as THREE.Material).depthWrite = false;
    return g;
  }, [size, cell, major]);
  useEffect(() => () => helper.dispose(), [helper]);
  return <primitive object={helper} rotation={rot} renderOrder={major ? -1 : -2} />;
}

// ---- the canvas -----------------------------------------------------------------------

export function EditorScene({ bounds, docId, onLost }: { bounds: [Vec3, Vec3]; docId: number; onLost: () => void }) {
  // P5: one canvas behind the pane boxes; each pane is a drei <View> drawn into its box
  const layout = usePanes((s) => s.layout);
  const views = usePanes((s) => s.views);
  const max = usePanes((s) => s.max);
  const active = usePanes((s) => s.active);
  const wrap = useRef<HTMLDivElement>(null);
  const t0 = useRef<HTMLDivElement | null>(null);
  const t1 = useRef<HTMLDivElement | null>(null);
  const t2 = useRef<HTMLDivElement | null>(null);
  const t3 = useRef<HTMLDivElement | null>(null);
  const tracks = [t0, t1, t2, t3];
  const n = paneCount(layout);
  return (
    <div ref={wrap} className="ed-panes">
      <div className="ed-panes-canvas">
        <Canvas
          onCreated={(state) => {
            const { gl } = state;
            wrapRootInvalidate(state); // Parity A: the canvas's own invalidate redraws every pane
            // P5: dev builds let the perf test read draw counts
            if (import.meta.env.DEV) (window as unknown as { __lsEditorGl?: THREE.WebGLRenderer }).__lsEditorGl = gl;
            gl.domElement.addEventListener("webglcontextlost", (e) => {
            e.preventDefault();
            // a canvas being put away (new document) drops its context on purpose
            if (gl.domElement.isConnected) onLost();
            });
          }}
          frameloop="demand"
          dpr={[1, 1.5]}
          linear
          flat
          gl={{ powerPreference: "high-performance", antialias: true, preserveDrawingBuffer: true /* Parity A: panes not redrawn keep their picture */ }}
          camera={{ fov: 70, near: 2, far: 40000, position: [0, 512, 512] }}
          events={pointerEvents}
          eventSource={wrap as React.MutableRefObject<HTMLElement>}
        >
          <Redraw />
          <Materials docId={docId}>
            {Array.from({ length: n }, (_, i) => (
              <PaneView key={`${i}-${views[i]}`} track={tracks[i] as React.MutableRefObject<HTMLElement | null>} index={i + 1} visible={max == null || max === i}>
                <PaneCtx.Provider value={{ slot: i, view: views[i], active: n === 1 || active === i }}>
                  <PaneScene view={views[i]} bounds={bounds} />
                </PaneCtx.Provider>
              </PaneView>
            ))}
          </Materials>
        </Canvas>
      </div>
      <PaneBoxes tracks={tracks} />
    </div>
  );
}

/** P5: one pane's scene: its own lights, grid, camera, tools and gizmo. */
function PaneScene({ view, bounds }: { view: ViewId; bounds: [Vec3, Vec3] }) {
  const grid = useEditor((s) => s.grid);
  const showGrid = useP5((s) => s.showGrid);
  const gridMajor = useP5((s) => s.gridMajor);
  const pivot = useRef<THREE.Group | null>(null);
  return (
    <>
      <color attach="background" args={["#22271e"]} />
      <hemisphereLight args={["#ffffff", "#5a5f50", 0.95]} />
      <directionalLight position={[0.4, 1, 0.25]} intensity={1.5} />
      {showGrid && (view === "3d" ? <GridFor view={view} grid={grid} bounds={bounds} /> : <FlatGrid view={view} grid={grid} major={gridMajor} bounds={bounds} />) /* Parity A: the flat views' grid follows the zoom */}
      {view !== "3d" && <Crosshair view={view} />}
      {view !== "3d" && <FlatHandles view={view} /> /* Parity A: Hammer's 8 handles */}
      <group rotation={ROOT_ROTATION}>
        <ToolLayer bounds={bounds} />
        {/* what the gizmo moves: in GoldSrc units like everything in here */}
        <group ref={pivot} />
      </group>
      <Gizmo pivot={pivot} />
      <TriadFeed />
      <QuietWhileDragging /> {/* Parity A: no hover rays while the camera is dragged */}
      {view === "3d" ? (
        <>
          <Persp bounds={bounds} />
          <Bookmarks />
        </>
      ) : (
        <Flat key={view} view={view} bounds={bounds} />
      )}
    </>
  );
}

/** P5: a 3D pane's own camera; B's orbit/fly cameras start once it's the one in use. */
function Persp({ bounds }: { bounds: [Vec3, Vec3] }) {
  const ref = useRef<THREE.PerspectiveCamera>(null);
  const cam = useThree((s) => s.camera);
  const nav = useNav((s) => s.preset); // P5: Unreal / Blender / Hammer navigation
  return (
    <>
      <PerspectiveCamera ref={ref} makeDefault fov={70} near={2} far={40000} position={[0, 512, 512]} />
      {ref.current && cam === ref.current && (nav === "blender" ? <Cameras bounds={bounds} /> : <NavCamera key={nav} bounds={bounds} preset={nav} />)}
    </>
  );
}

/** P5: pane boxes move without the canvas resizing: draw again when they do. */
function Redraw() {
  const invalidate = useThree((s) => s.invalidate);
  const layout = usePanes((s) => s.layout);
  const split = usePanes((s) => s.splits[s.layout]);
  const max = usePanes((s) => s.max);
  useEffect(() => {
    invalidate();
    // the views read their boxes on the next frame: once more after that
    const id = requestAnimationFrame(() => invalidate());
    return () => cancelAnimationFrame(id);
  }, [layout, split, max, invalidate]);
  return null;
}

/** P5: a cross-hair through the mouse's place in the flat views (GoldSrc units). */
function Crosshair({ view }: { view: Exclude<ViewId, "3d"> }) {
  const cursor = useEditor((s) => s.cursor);
  if (!cursor) return null;
  const axis = viewAxis(view);
  const [u, v] = [0, 1, 2].filter((i) => i !== axis);
  const far = 65536;
  const at = (i: number, val: number): Vec3 => {
    const p: Vec3 = [cursor[0], cursor[1], cursor[2]];
    p[i] = val;
    return p;
  };
  return (
    <group rotation={ROOT_ROTATION}>
      <Line points={[at(u, -far), at(u, far)]} color="#8fa0b0" lineWidth={1} transparent opacity={0.35} depthTest={false} renderOrder={6} />
      <Line points={[at(v, -far), at(v, far)]} color="#8fa0b0" lineWidth={1} transparent opacity={0.35} depthTest={false} renderOrder={6} />
    </group>
  );
}
