// Parity A (PerfAudit.md #1-#3): the map's brushes drawn the way TrenchBroom and Radiant do it.
// - One set of GPU buffers for the whole map, shared by every pane (never merged per pane): one
//   triangle pool per texture, and outline pools (world brushes, brush-entity brushes, sky-only).
// - Each brush owns a range in its pools. An edit rewrites only the touched brushes' ranges in
//   place (BufferAttribute update ranges): a one-brush nudge uploads a few hundred floats, not
//   the map. A pool only grows (and is re-uploaded) when it runs out of room.
// - Brushes leave the pools while picked, hidden or dimmed by the cordon (their range is filled
//   with NaN: the GPU drops those triangles) and come back the same way.
// - The selection is an overlay: the picked brushes in one mesh per texture plus one outline
//   draw, built from each brush's cached data when the selection changes (and disposed when it's
//   replaced), drawn on top and moved by the gizmo's preview.
// - Picking: one object per pane tests every visible brush's box, then the brush itself
//   (pickRay.ts), handing back a stand-in mesh per brush with `{ kind: "brush", id }` as before.

import * as THREE from "three";
import { create } from "zustand";
import { brushRaycast } from "./pickRay";
import { redrawAll } from "./paneRedraw";
import { isHiddenBrush, useEditor, type BrushGeo } from "./store";

const isSky = (t: string) => /^sky$/i.test(t);
const skyOnly = (b: BrushGeo) => b.textures.length > 0 && b.textures.every(isSky);
const HUGE = new THREE.Sphere(new THREE.Vector3(), 1e7);

const edgeCache = new WeakMap<BrushGeo, Float32Array>();
/** A brush's outline segments, cached with its mesh. */
export function edgesOf(b: BrushGeo): Float32Array {
  let e = edgeCache.get(b);
  if (!e) {
    const g = new THREE.EdgesGeometry(b.geometry, 20);
    e = (g.getAttribute("position").array as Float32Array).slice();
    g.dispose();
    edgeCache.set(b, e);
  }
  return e;
}

/** A growable vertex pool: ranges handed out to brushes, rewritten in place. */
export class Pool {
  geometry: THREE.BufferGeometry;
  private arrays: Float32Array[];
  private cap: number;
  private used = 0;
  /** freed ranges by size (brushes come and go in the same sizes) */
  private free = new Map<number, number[]>();
  /** changes when the geometry object is replaced (it grew) */
  gen = 0;
  /** many ranges changed before the next upload: send the whole buffer once instead */
  private full = false;
  constructor(
    readonly key: string,
    private readonly sizes: number[],
    private readonly names: string[],
    cap: number,
  ) {
    this.cap = Math.max(64, cap);
    this.arrays = sizes.map((s) => new Float32Array(this.cap * s).fill(NaN));
    this.geometry = this.makeGeometry();
  }
  private makeGeometry() {
    const g = new THREE.BufferGeometry();
    this.names.forEach((n, i) => {
      const a = new THREE.BufferAttribute(this.arrays[i], this.sizes[i]);
      a.setUsage(THREE.DynamicDrawUsage);
      a.onUpload(() => (this.full = false));
      g.setAttribute(n, a);
    });
    g.boundingSphere = HUGE;
    g.setDrawRange(0, this.used);
    return g;
  }
  /** A range of `n` vertices. */
  alloc(n: number): number {
    const f = this.free.get(n)?.pop();
    if (f != null) return f;
    if (this.used + n > this.cap) this.grow(this.used + n);
    const s = this.used;
    this.used += n;
    this.geometry.setDrawRange(0, this.used);
    return s;
  }
  private grow(need: number) {
    let cap = this.cap;
    while (cap < need) cap *= 2;
    this.arrays = this.arrays.map((old, i) => {
      const a = new Float32Array(cap * this.sizes[i]).fill(NaN);
      a.set(old.subarray(0, this.used * this.sizes[i]));
      return a;
    });
    this.cap = cap;
    // a new geometry (uploaded whole once); the old one's GPU buffers are freed
    this.geometry.dispose();
    this.geometry = this.makeGeometry();
    this.gen++;
    structural = true;
  }
  /** Copy vertices in (one source array per attribute, `n` vertices each). */
  write(start: number, src: Float32Array[]) {
    src.forEach((s, i) => this.arrays[i].set(s, start * this.sizes[i]));
    this.touch(start, src[0].length / this.sizes[0]);
  }
  release(start: number, n: number) {
    this.hide(start, n);
    let b = this.free.get(n);
    if (!b) this.free.set(n, (b = []));
    b.push(start);
  }
  /** The range drops out of the picture (NaN positions: the GPU skips those triangles). */
  hide(start: number, n: number) {
    this.arrays[0].fill(NaN, start * this.sizes[0], (start + n) * this.sizes[0]);
    this.touch(start, n);
  }
  touch(start: number, n: number) {
    this.names.forEach((name, i) => {
      const a = this.geometry.getAttribute(name) as THREE.BufferAttribute;
      if (a.array !== this.arrays[i]) return;
      if (!this.full) {
        a.addUpdateRange(start * this.sizes[i], n * this.sizes[i]);
        // thousands of small uploads (a big selection coming back) cost more than one whole one
        if (a.updateRanges.length > 256) this.full = true;
      }
      if (this.full) a.clearUpdateRanges();
      a.needsUpdate = true;
    });
  }
  dispose() {
    this.geometry.dispose();
  }
}

type Range = { pool: Pool; start: number; count: number };
type Rec = { b: BrushGeo; ranges: Range[]; shown: boolean; shape: string };

const recs = new Map<number, Rec>();
const triPools = new Map<string, Pool>();
const edgePools = { world: null as Pool | null, tied: null as Pool | null, sky: null as Pool | null };
let structural = false;
let lastBrushes: Map<number, BrushGeo> | null = null;
let lastKey = "";

/** Every visible brush (picked and dimmed ones too): what clicks can find. */
export const pickable = new Set<number>();

function triPool(texture: string, cap: number): Pool {
  let p = triPools.get(texture);
  if (!p) {
    triPools.set(texture, (p = new Pool(texture, [3, 3, 2], ["position", "normal", "uv"], cap)));
    structural = true;
  }
  return p;
}
function edgePool(which: keyof typeof edgePools, cap: number): Pool {
  let p = edgePools[which];
  if (!p) {
    edgePools[which] = p = new Pool(which, [3], ["position"], cap);
    structural = true;
  }
  return p;
}
const edgeKind = (b: BrushGeo): keyof typeof edgePools => (skyOnly(b) ? "sky" : b.entity != null ? "tied" : "world");

/** The brush's layout in the pools: its textures, triangle counts and outline length. */
function shapeOf(b: BrushGeo): string {
  return `${b.geometry.groups.map((g) => `${b.textures[g.materialIndex ?? 0] ?? ""}:${g.count}`).join(",")}|${edgeKind(b)}:${edgesOf(b).length}`;
}

/** The brush's data written into its ranges (same layout). */
function writeInto(b: BrushGeo, ranges: Range[]) {
  const pos = b.geometry.getAttribute("position").array as Float32Array;
  const nor = b.geometry.getAttribute("normal").array as Float32Array;
  const uv = b.geometry.getAttribute("uv").array as Float32Array;
  b.geometry.groups.forEach((g, i) => {
    ranges[i].pool.write(ranges[i].start, [pos.subarray(g.start * 3, (g.start + g.count) * 3), nor.subarray(g.start * 3, (g.start + g.count) * 3), uv.subarray(g.start * 2, (g.start + g.count) * 2)]);
  });
  const e = edgesOf(b);
  const last = ranges[b.geometry.groups.length];
  if (e.length && last) last.pool.write(last.start, [e]);
}

function place(b: BrushGeo, sizeHint?: Map<string, number>): Range[] {
  const out: Range[] = [];
  const pos = b.geometry.getAttribute("position").array as Float32Array;
  const nor = b.geometry.getAttribute("normal").array as Float32Array;
  const uv = b.geometry.getAttribute("uv").array as Float32Array;
  for (const g of b.geometry.groups) {
    const t = b.textures[g.materialIndex ?? 0] ?? "";
    const pool = triPool(t, sizeHint?.get(t) ?? 1024);
    const start = pool.alloc(g.count);
    pool.write(start, [pos.subarray(g.start * 3, (g.start + g.count) * 3), nor.subarray(g.start * 3, (g.start + g.count) * 3), uv.subarray(g.start * 2, (g.start + g.count) * 2)]);
    out.push({ pool, start, count: g.count });
  }
  const e = edgesOf(b);
  if (e.length) {
    const k = edgeKind(b);
    const pool = edgePool(k, sizeHint?.get(`edges:${k}`) ?? 4096);
    const start = pool.alloc(e.length / 3);
    pool.write(start, [e]);
    out.push({ pool, start, count: e.length / 3 });
  }
  return out;
}

function unplace(r: Rec) {
  for (const x of r.ranges) x.pool.release(x.start, x.count);
  r.ranges = [];
}
function hideRec(r: Rec) {
  for (const x of r.ranges) x.pool.hide(x.start, x.count);
  r.shown = false;
}

// ---- the selection overlay ------------------------------------------------------------------

export type Overlay = { meshes: { texture: string; geometry: THREE.BufferGeometry }[]; edges: THREE.BufferGeometry; ids: number[] };

function buildOverlay(list: BrushGeo[]): Overlay {
  const parts = new Map<string, { n: number; chunks: { b: BrushGeo; start: number; count: number }[] }>();
  let edgeLen = 0;
  for (const b of list) {
    for (const g of b.geometry.groups) {
      const t = b.textures[g.materialIndex ?? 0] ?? "";
      let p = parts.get(t);
      if (!p) parts.set(t, (p = { n: 0, chunks: [] }));
      p.chunks.push({ b, start: g.start, count: g.count });
      p.n += g.count;
    }
    edgeLen += edgesOf(b).length;
  }
  const meshes: Overlay["meshes"] = [];
  for (const [texture, p] of parts) {
    const P = new Float32Array(p.n * 3);
    const N = new Float32Array(p.n * 3);
    const U = new Float32Array(p.n * 2);
    let at = 0;
    for (const c of p.chunks) {
      P.set((c.b.geometry.getAttribute("position").array as Float32Array).subarray(c.start * 3, (c.start + c.count) * 3), at * 3);
      N.set((c.b.geometry.getAttribute("normal").array as Float32Array).subarray(c.start * 3, (c.start + c.count) * 3), at * 3);
      U.set((c.b.geometry.getAttribute("uv").array as Float32Array).subarray(c.start * 2, (c.start + c.count) * 2), at * 2);
      at += c.count;
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(P, 3));
    geometry.setAttribute("normal", new THREE.BufferAttribute(N, 3));
    geometry.setAttribute("uv", new THREE.BufferAttribute(U, 2));
    geometry.boundingSphere = HUGE;
    meshes.push({ texture, geometry });
  }
  const E = new Float32Array(edgeLen);
  let ei = 0;
  for (const b of list) {
    const e = edgesOf(b);
    E.set(e, ei);
    ei += e.length;
  }
  const edges = new THREE.BufferGeometry();
  edges.setAttribute("position", new THREE.BufferAttribute(E, 3));
  edges.boundingSphere = HUGE;
  return { meshes, edges, ids: list.map((b) => b.id) };
}

function disposeOverlay(o: Overlay | null) {
  if (!o) return;
  o.meshes.forEach((m) => m.geometry.dispose());
  o.edges.dispose();
}

// ---- what the panes read -----------------------------------------------------------------------

type BufState = {
  /** changes when a pool's geometry object or the set of pools changes (panes re-render) */
  version: number;
  overlay: Overlay | null;
  /** brushes dimmed by the cordon: drawn one by one with the dimming (EditorScene's BrushMesh) */
  dimmed: BrushGeo[];
};
export const useBrushBuffers = create<BufState>(() => ({ version: 0, overlay: null, dimmed: [] }));

export function triangleFor(texture: string) {
  return triPools.get(texture) ?? null;
}
export function allTrianglePools(): Pool[] {
  return [...triPools.values()];
}
export function outlinePools() {
  return edgePools;
}

let lastOverlayKey = "";
let lastDimKey = "";

/** Bring the buffers up to date with the editor's state (cheap when little changed). */
export function syncBrushBuffers() {
  const s = useEditor.getState();
  const groups = s.doc?.groups;
  const hiddenGroups = new Set((groups ?? []).filter((g) => g.hidden).map((g) => g.id));
  const c = s.doc?.editor?.cordon;
  const dimOn = !!(c?.on && c.boxes.length);
  const dimmed = (b: BrushGeo) => dimOn && !c!.boxes.some((x) => b.min.every((v, i) => v < x.max[i]) && b.max.every((v, i) => v > x.min[i]));
  const selB = new Set(s.sel.brushes);
  const selE = new Set(s.sel.entities);
  const picked = (b: BrushGeo) => selB.has(b.id) || (b.entity != null && selE.has(b.entity));
  const key = `${s.sel.brushes.length}:${s.sel.entities.join(",")}|${dimOn ? JSON.stringify(c!.boxes) : ""}`;
  // nothing that matters changed: the same brushes, selection, hidden things and cordon
  if (s.brushes === lastBrushes && key === lastKey + "" && lastSel === s.sel && lastHidden === s.hidden && lastGroups === groups) return;
  const first = recs.size === 0;
  // first build: size each pool once instead of growing it
  let hint: Map<string, number> | undefined;
  if (first) {
    hint = new Map();
    for (const b of s.brushes.values()) {
      for (const g of b.geometry.groups) {
        const t = b.textures[g.materialIndex ?? 0] ?? "";
        hint.set(t, (hint.get(t) ?? 0) + g.count);
      }
      const k = `edges:${edgeKind(b)}`;
      hint.set(k, (hint.get(k) ?? 0) + edgesOf(b).length / 3);
    }
  }
  const pickedList: BrushGeo[] = [];
  const dimList: BrushGeo[] = [];
  pickable.clear();
  for (const b of s.brushes.values()) {
    const vis = !isHiddenBrush(b, s, hiddenGroups);
    if (vis) pickable.add(b.id);
    const isPicked = vis && picked(b);
    const isDim = vis && !isPicked && dimmed(b);
    if (isPicked) pickedList.push(b);
    if (isDim) dimList.push(b);
    const want = vis && !isPicked && !isDim;
    const r = recs.get(b.id);
    if (r && r.b === b && r.shown === want) continue;
    if (!r) {
      // a new brush: its ranges for good (hidden until it's shown)
      const rec: Rec = { b, ranges: place(b, hint), shown: true, shape: shapeOf(b) };
      if (!want) hideRec(rec);
      recs.set(b.id, rec);
      continue;
    }
    if (r.b !== b) {
      const shape = shapeOf(b);
      if (shape === r.shape) {
        // same layout (a move, a turn, a texture shift): rewritten in place
        r.b = b;
        if (want) writeInto(b, r.ranges);
        else hideRec(r);
        r.shown = want;
        continue;
      }
      unplace(r);
      r.b = b;
      r.shape = shape;
      r.ranges = place(b);
      r.shown = true;
      if (!want) hideRec(r);
      continue;
    }
    // shown again (unpicked, unhidden): its data back in its own ranges
    if (want) writeInto(b, r.ranges);
    else hideRec(r);
    r.shown = want;
  }
  for (const [id, r] of recs) {
    if (!s.brushes.has(id)) {
      unplace(r);
      recs.delete(id);
    }
  }
  lastBrushes = s.brushes;
  lastKey = key;
  lastSel = s.sel;
  lastHidden = s.hidden;
  lastGroups = groups;
  // the overlay: rebuilt only when the picked brushes (or their meshes) change
  const oKey = pickedList.map((b) => `${b.id}:${geoId(b)}`).join(",");
  const st = useBrushBuffers.getState();
  let overlay = st.overlay;
  if (oKey !== lastOverlayKey) {
    lastOverlayKey = oKey;
    const old = overlay;
    overlay = pickedList.length ? buildOverlay(pickedList) : null;
    // freed once the panes have drawn the new one (a pane drawing a disposed geometry would
    // upload it again, and nothing would free that copy)
    if (old) setTimeout(() => disposeOverlay(old), 1000);
  }
  const dKey = dimList.map((b) => `${b.id}:${geoId(b)}`).join(",");
  const dimChanged = dKey !== lastDimKey;
  lastDimKey = dKey;
  if (structural || overlay !== st.overlay || dimChanged) {
    structural = false;
    useBrushBuffers.setState({ version: st.version + 1, overlay, ...(dimChanged ? { dimmed: dimList } : {}) });
  }
  redrawAll();
}
let lastSel: unknown = null;
let lastHidden: unknown = null;
let lastGroups: unknown = null;
const geoIds = new WeakMap<BrushGeo, number>();
let nextGeo = 1;
function geoId(b: BrushGeo) {
  let i = geoIds.get(b);
  if (!i) geoIds.set(b, (i = nextGeo++));
  return i;
}

/** Start over (a new document): every pool and range is dropped and freed. */
export function resetBrushBuffers() {
  for (const p of triPools.values()) p.dispose();
  triPools.clear();
  for (const k of Object.keys(edgePools) as (keyof typeof edgePools)[]) {
    edgePools[k]?.dispose();
    edgePools[k] = null;
  }
  recs.clear();
  pickable.clear();
  disposeOverlay(useBrushBuffers.getState().overlay);
  lastBrushes = null;
  lastOverlayKey = "";
  lastDimKey = "";
  structural = true;
  useBrushBuffers.setState({ version: useBrushBuffers.getState().version + 1, overlay: null, dimmed: [] });
}

// follow the editor: brushes, selection, hidden things, groups, cordon
let lastDocId: unknown = null;
useEditor.subscribe((s, p) => {
  const docId = (s.doc as { docId?: unknown } | null)?.docId ?? null;
  if (s.meshDoc !== p.meshDoc || (docId !== lastDocId && s.brushes.size === 0)) {
    lastDocId = docId;
    if (s.meshDoc !== p.meshDoc) resetBrushBuffers();
  }
  if (s.brushes !== p.brushes || s.sel !== p.sel || s.hidden !== p.hidden || s.doc?.groups !== p.doc?.groups || s.doc?.editor?.cordon !== p.doc?.editor?.cordon) syncBrushBuffers();
});

// ---- picking --------------------------------------------------------------------------------

const proxies = [new WeakMap<BrushGeo, THREE.Mesh>(), new WeakMap<BrushGeo, THREE.Mesh>()];
function proxy(b: BrushGeo, sky: boolean, parent: THREE.Object3D): THREE.Mesh {
  const map = proxies[sky ? 1 : 0];
  let m = map.get(b);
  if (!m) {
    m = new THREE.Mesh(b.geometry);
    m.userData = sky ? { kind: "brush", id: b.id, sky: true } : { kind: "brush", id: b.id };
    m.raycast = brushRaycast(b);
    map.set(b, m);
  }
  (m as { parent: THREE.Object3D | null }).parent = parent;
  m.matrixWorld.copy(parent.matrixWorld);
  return m;
}

const inv = new THREE.Matrix4();
const ray = new THREE.Ray();
const box = new THREE.Box3();

/** One pane's picking object: put it inside the GoldSrc root group. */
export function makePicker(hideSky: () => boolean): THREE.Object3D {
  const o = new THREE.Object3D();
  o.raycast = (rc, out) => {
    const s = useEditor.getState();
    inv.copy(o.matrixWorld).invert();
    ray.copy(rc.ray).applyMatrix4(inv);
    const hs = hideSky();
    for (const id of pickable) {
      const b = s.brushes.get(id);
      if (!b) continue;
      box.min.set(b.min[0] - 0.5, b.min[1] - 0.5, b.min[2] - 0.5);
      box.max.set(b.max[0] + 0.5, b.max[1] + 0.5, b.max[2] + 0.5);
      if (!ray.intersectsBox(box)) continue;
      const m = proxy(b, hs && skyOnly(b), o);
      m.raycast(rc, out);
    }
  };
  return o;
}

/** Brushes' world-space positions at the moment (for tests and tools): how many vertex slots are in use. */
export function bufferStats() {
  let tri = 0;
  for (const p of triPools.values()) tri += p.geometry.drawRange.count;
  return { pools: triPools.size, triangleVerts: tri, brushes: recs.size };
}

// ---- the box select's highlight ------------------------------------------------------------

let hotKey: unknown = null;
let hotGeo: THREE.BufferGeometry | null = null;
/** All of `list`'s triangles in one geometry, shared by every pane for the same `key` (the box
 *  select's hot set): one draw and one upload instead of a mesh per brush (PerfAudit #3). */
export function sharedTriangles(key: object, list: BrushGeo[]): THREE.BufferGeometry {
  if (key === hotKey && hotGeo) return hotGeo;
  const old = hotGeo;
  if (old) setTimeout(() => old.dispose(), 1000);
  let n = 0;
  for (const b of list) n += (b.geometry.getAttribute("position").array as Float32Array).length;
  const P = new Float32Array(n);
  let at = 0;
  for (const b of list) {
    const a = b.geometry.getAttribute("position").array as Float32Array;
    P.set(a, at);
    at += a.length;
  }
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.BufferAttribute(P, 3));
  g.boundingSphere = HUGE;
  hotKey = key;
  hotGeo = g;
  return g;
}
