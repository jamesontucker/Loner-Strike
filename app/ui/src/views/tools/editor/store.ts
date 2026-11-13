// The map editor's page state: the document summary and brush meshes the app sent (the
// document itself lives in Rust), what's selected, the tool and its settings.

import * as THREE from "three";
import { create } from "zustand";
import type { Vec3 } from "../viewer/coords";
import * as api from "./editorApi";
import type { Checks, CmdResult, Doc, DocState, MeshBrush, MeshFace, MeshUpdate } from "./editorApi";

export type ToolId = "select" | "move" | "rotate" | "scale" | "box" | "cylinder" | "clip" | "entity" | "texture";
export type ViewId = "3d" | "top" | "front" | "side";
export type Selection = { brushes: number[]; entities: number[]; face: { brush: number; face: number } | null };

/** A brush ready to draw: one geometry, a group per texture, which face each triangle is. */
export type BrushGeo = {
  id: number;
  entity: number | null;
  group: number | null;
  geometry: THREE.BufferGeometry;
  /** the texture of each geometry group (material slot) */
  textures: string[];
  faceOfTri: Int32Array;
  faces: MeshFace[];
  /** each face's corners (GoldSrc), by face number */
  polys: Map<number, Vec3[]>;
  min: Vec3;
  max: Vec3;
};

export const EMPTY_SEL: Selection = { brushes: [], entities: [], face: null };

function buildBrush(b: MeshBrush, pos: Float32Array, uv: Float32Array): BrushGeo {
  // faces sorted by texture so each texture is one draw
  const faces = [...b.faces].sort((x, y) => (x.texture < y.texture ? -1 : x.texture > y.texture ? 1 : x.face - y.face));
  let tris = 0;
  for (const f of faces) tris += Math.max(0, f.count - 2);
  const P = new Float32Array(tris * 9);
  const U = new Float32Array(tris * 6);
  const N = new Float32Array(tris * 9);
  const faceOfTri = new Int32Array(tris);
  const geometry = new THREE.BufferGeometry();
  const textures: string[] = [];
  const min: Vec3 = [Infinity, Infinity, Infinity];
  const max: Vec3 = [-Infinity, -Infinity, -Infinity];
  let t = 0;
  let groupStart = 0;
  let current: string | null = null;
  const put = (k: number, v: number) => {
    P.set(pos.subarray(v * 3, v * 3 + 3), k * 3);
    U.set(uv.subarray(v * 2, v * 2 + 2), k * 2);
    for (let a = 0; a < 3; a++) {
      const x = pos[v * 3 + a];
      if (x < min[a]) min[a] = x;
      if (x > max[a]) max[a] = x;
    }
  };
  const polys = new Map<number, Vec3[]>();
  for (const f of faces) {
    const pts: Vec3[] = [];
    for (let i = 0; i < f.count; i++) pts.push([pos[(f.start + i) * 3], pos[(f.start + i) * 3 + 1], pos[(f.start + i) * 3 + 2]]);
    polys.set(f.face, pts);
    if (current !== null && f.texture !== current) {
      geometry.addGroup(groupStart * 3, (t - groupStart) * 3, textures.length - 1);
      groupStart = t;
    }
    if (f.texture !== current) {
      textures.push(f.texture);
      current = f.texture;
    }
    for (let i = 1; i + 1 < f.count; i++) {
      put(t * 3, f.start);
      put(t * 3 + 1, f.start + i);
      put(t * 3 + 2, f.start + i + 1);
      for (let k = 0; k < 3; k++) N.set(f.normal, (t * 3 + k) * 3);
      faceOfTri[t] = f.face;
      t++;
    }
  }
  if (t > groupStart) geometry.addGroup(groupStart * 3, (t - groupStart) * 3, Math.max(0, textures.length - 1));
  geometry.setAttribute("position", new THREE.BufferAttribute(P, 3));
  geometry.setAttribute("normal", new THREE.BufferAttribute(N, 3));
  geometry.setAttribute("uv", new THREE.BufferAttribute(U, 2));
  geometry.computeBoundingSphere();
  geometry.computeBoundingBox();
  if (!tris) {
    min.fill(0);
    max.fill(0);
  }
  return { id: b.id, entity: b.entity, group: b.group, geometry, textures, faceOfTri, faces: b.faces, polys, min, max };
}

type Notify = (text: string, error?: boolean) => void;

type EditorState = {
  /** null: nothing open (the start screen); undefined: still asking the app */
  doc: Doc | null | undefined;
  loadError: string | null;
  brushes: Map<number, BrushGeo>;
  /** the revision the meshes are up to */
  meshRev: number | null;
  meshDoc: number | null;
  checks: Checks | null;
  sel: Selection;
  tool: ToolId;
  view: ViewId;
  grid: number;
  /** held Alt: snapping off for now */
  alt: boolean;
  texture: string;
  boxHeight: number;
  sides: number;
  thickness: number;
  entityClass: string;
  textureLock: boolean;
  /** a command is on its way (for the busy label) */
  working: number;
  /** a file action is running: its label ("Saving…") */
  fileBusy: string | null;
  /** a gizmo/drag preview: the selection drawn moved by this (GoldSrc), until the app catches up */
  preview: THREE.Matrix4 | null;
  notify: Notify;
  /** where the mouse is on the working plane (GoldSrc), for the status bar */
  cursor: Vec3 | null;
  /** the clip tool's progress, and its actions (set by the 3D view) */
  clipInfo: { points: number; keep: "back" | "front" | "both" } | null;
  doClip: (() => void) | null;
  setClipKeep: ((k: "back" | "front" | "both") => void) | null;

  setNotify: (n: Notify) => void;
  set: (p: Partial<EditorState>) => void;
  select: (s: Selection) => void;
  load: () => Promise<void>;
  syncMeshes: () => Promise<void>;
  refreshDoc: () => Promise<void>;
  refreshChecks: () => void;
  cmd: (body: Record<string, unknown>, opts?: { doc?: boolean; quiet?: boolean }) => Promise<CmdResult>;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  file: (what: "new" | "open" | "import" | "save" | "saveAs" | "export") => Promise<void>;
};

function keep(key: string, v: string) {
  try {
    localStorage.setItem(key, v);
  } catch {
    /* private window */
  }
}
function saved(key: string, fallback: number) {
  try {
    const n = Number(localStorage.getItem(key));
    return Number.isFinite(n) && n > 0 ? n : fallback;
  } catch {
    return fallback;
  }
}

let checksTimer: ReturnType<typeof setTimeout> | null = null;
let meshChain: Promise<void> = Promise.resolve();

export const useEditor = create<EditorState>((set, get) => ({
  doc: undefined,
  loadError: null,
  brushes: new Map(),
  meshRev: null,
  meshDoc: null,
  checks: null,
  sel: EMPTY_SEL,
  tool: "select",
  view: "3d",
  grid: saved("ls.editor.grid", 16),
  alt: false,
  texture: "OUT_WALL5",
  boxHeight: 128,
  sides: 12,
  thickness: 8,
  entityClass: "info_player_start",
  textureLock: true,
  working: 0,
  fileBusy: null,
  preview: null,
  notify: () => {},
  cursor: null,
  clipInfo: null,
  doClip: null,
  setClipKeep: null,

  setNotify: (notify) => set({ notify }),
  set: (p) => {
    if (p.grid) keep("ls.editor.grid", String(p.grid));
    set(p);
  },
  select: (sel) => set({ sel }),

  load: async () => {
    const d = await api.getDoc();
    if (d.error) {
      // nothing open is the normal start, anything else is worth saying
      const none = /No map is open/.test(d.error);
      set({ doc: null, loadError: none ? null : d.error });
      return;
    }
    for (const b of get().brushes.values()) b.geometry.dispose();
    set({ doc: d, loadError: null, brushes: new Map(), meshRev: null, meshDoc: null, sel: EMPTY_SEL, preview: null, checks: null });
    await get().syncMeshes();
    get().refreshChecks();
  },

  // one mesh request at a time, each picking up from where the last left off
  syncMeshes: () => {
    meshChain = meshChain.then(async () => {
      const { meshDoc, meshRev } = get();
      let u: MeshUpdate;
      try {
        u = await api.getMeshes(meshDoc, meshDoc == null ? null : meshRev);
      } catch (e) {
        get().notify(e instanceof Error ? e.message : String(e), true);
        return;
      }
      const next = u.full ? new Map<number, BrushGeo>() : new Map(get().brushes);
      if (u.full) for (const b of get().brushes.values()) b.geometry.dispose();
      for (const id of u.removed) {
        next.get(id)?.geometry.dispose();
        next.delete(id);
      }
      for (const b of u.brushes) {
        next.get(b.id)?.geometry.dispose();
        next.set(b.id, buildBrush(b, u.pos, u.uv));
      }
      // the selection loses what's gone
      const { sel } = get();
      const brushes = sel.brushes.filter((id) => next.has(id));
      const face = sel.face && next.has(sel.face.brush) ? sel.face : null;
      set({ brushes: next, meshRev: u.revision, meshDoc: u.docId, preview: null, ...(brushes.length !== sel.brushes.length || face !== sel.face ? { sel: { ...sel, brushes, face } } : {}) });
    });
    return meshChain;
  },

  refreshDoc: async () => {
    const d = await api.getDoc();
    if (d.error) return get().notify(d.error, true);
    const ids = new Set(d.entities.map((e) => e.id));
    const { sel } = get();
    const entities = sel.entities.filter((id) => ids.has(id));
    set({ doc: d, ...(entities.length !== sel.entities.length ? { sel: { ...sel, entities } } : {}) });
  },

  // checks follow edits, a moment after they settle
  refreshChecks: () => {
    if (checksTimer) clearTimeout(checksTimer);
    checksTimer = setTimeout(async () => {
      const c = await api.getChecks();
      if (!c.error) set({ checks: c });
    }, 350);
  },

  cmd: async (body, opts = {}) => {
    set((s) => ({ working: s.working + 1 }));
    let r: CmdResult;
    try {
      r = await api.sendCmd(body);
    } finally {
      set((s) => ({ working: s.working - 1 }));
    }
    if (r.state) mergeState(r.state);
    if (r.error) {
      if (!opts.quiet) get().notify(r.error, true);
      set({ preview: null });
      return r;
    }
    if (r.note && body.op !== "group") get().notify(r.note);
    if (r.changed) await get().syncMeshes();
    if (opts.doc !== false) {
      await get().refreshDoc();
      get().refreshChecks();
    }
    return r;
  },

  undo: async () => {
    const r = await api.undo();
    if (r.error) return get().notify(r.error, true);
    if (r.state) mergeState(r.state);
    if (!r.changed) return;
    await get().syncMeshes();
    await get().refreshDoc();
    get().refreshChecks();
    if (r.label) get().notify(`Undid ${r.label.toLowerCase()}`);
  },
  redo: async () => {
    const r = await api.redo();
    if (r.error) return get().notify(r.error, true);
    if (r.state) mergeState(r.state);
    if (!r.changed) return;
    await get().syncMeshes();
    await get().refreshDoc();
    get().refreshChecks();
    if (r.label) get().notify(`Redid ${r.label.toLowerCase()}`);
  },

  file: async (what) => {
    const labels = { new: "Starting a new map…", open: "Opening…", import: "Importing…", save: "Saving…", saveAs: "Saving…", export: "Exporting…" } as const;
    if (get().fileBusy) return;
    set({ fileBusy: labels[what] });
    try {
      if (what === "save" || what === "saveAs") {
        const r = await api.saveProject(what === "saveAs");
        if (r.error) return get().notify(r.error, true);
        if (r.cancelled) return;
        if (r.state) mergeState(r.state);
        get().notify(`Saved ${r.file ?? ""}`.trim());
        return;
      }
      if (what === "export") {
        const r = await api.exportMap();
        if (r.error) return get().notify(r.error, true);
        if (!r.cancelled) get().notify(`Exported ${r.file ?? "the .map"}. Open it in any GoldSrc map compiler.`);
        return;
      }
      const r = what === "new" ? await api.newMap() : what === "open" ? await api.openProject() : await api.importMap();
      if (r.error) return get().notify(r.error, true);
      if ("cancelled" in r && r.cancelled) return;
      await get().load();
    } finally {
      set({ fileBusy: null });
    }
  },
}));

/** Undo labels, dirty state and the like arrive with every answer. */
function mergeState(st: DocState) {
  const d = useEditor.getState().doc;
  if (d) useEditor.setState({ doc: { ...d, ...st } });
}

/** Grid snapping (off while Alt is held). */
export function snap(v: number, grid = useEditor.getState().grid) {
  return useEditor.getState().alt || grid <= 0 ? Math.round(v * 100) / 100 : Math.round(v / grid) * grid;
}

/** The selection's bounds from what's drawn (brushes) and the doc (point entities). */
export function selectionBounds(s = useEditor.getState()): [Vec3, Vec3] | null {
  const min: Vec3 = [Infinity, Infinity, Infinity];
  const max: Vec3 = [-Infinity, -Infinity, -Infinity];
  let any = false;
  const add = (p: Vec3) => {
    any = true;
    for (let i = 0; i < 3; i++) {
      min[i] = Math.min(min[i], p[i]);
      max[i] = Math.max(max[i], p[i]);
    }
  };
  for (const id of s.sel.brushes) {
    const b = s.brushes.get(id);
    if (b) {
      add(b.min);
      add(b.max);
    }
  }
  for (const id of s.sel.entities) {
    const e = s.doc?.entities.find((x) => x.id === id);
    if (!e) continue;
    if (e.brushes.length) {
      for (const bid of e.brushes) {
        const b = s.brushes.get(bid);
        if (b) {
          add(b.min);
          add(b.max);
        }
      }
    } else {
      const o = originOf(e.keys);
      if (o) add(o);
    }
  }
  return any ? [min, max] : null;
}

export function keyOf(keys: [string, string][], k: string) {
  return keys.find(([a]) => a === k)?.[1];
}

export function originOf(keys: [string, string][]): Vec3 | null {
  const v = keyOf(keys, "origin");
  if (!v) return null;
  const n = v.trim().split(/\s+/).map(Number);
  return n.length >= 3 && n.slice(0, 3).every(Number.isFinite) ? [n[0], n[1], n[2]] : null;
}

export const isEmptySel = (s: Selection) => !s.brushes.length && !s.entities.length && !s.face;
