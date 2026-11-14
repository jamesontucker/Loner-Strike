// The map editor's page state: the document summary and brush meshes the app sent (the
// document itself lives in Rust), what's selected, the tool and its settings.

import * as THREE from "three";
import { create } from "zustand";
import type { Vec3 } from "../viewer/coords";
import * as api from "./editorApi";
import type { Checks, CmdResult, Doc, DocChanges, DocState, MeshBrush, MeshFace, MeshUpdate } from "./editorApi";

export type ToolId = "select" | "move" | "rotate" | "scale" | "vertex" | "box" | "shape" | "clip" | "entity" | "texture" | "measure";
export type ShapeKind = "cylinder" | "wedge" | "spike" | "sphere" | "arch" | "cone" | "torus"; // Parity A: cone, torus
export type FaceRef = { brush: number; face: number };
export type Dialog = "paste" | "props" | "report" | null;
export type ViewId = "3d" | "top" | "front" | "side";
/** `face` is the face the sheet shows; `faces` all picked faces (it included). */
export type Selection = { brushes: number[]; entities: number[]; face: FaceRef | null; faces?: FaceRef[] };

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

export const EMPTY_SEL: Selection = { brushes: [], entities: [], face: null, faces: [] };
/** Every picked face (the sheet's face first). */
export const facesOf = (s: Selection): FaceRef[] => (s.faces?.length ? s.faces : s.face ? [s.face] : []);

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
  shape: ShapeKind;
  archArc: number;
  archWall: number;
  thickness: number;
  /** click a grouped thing: the whole group (Hammer's default), or just it */
  selectMode: "groups" | "objects" | "solids"; // P5: solids = single brushes, even inside brush entities (Hammer)
  /** P5: grid snapping turned off until turned on again (Alt still flips it while held) */
  snapOff?: boolean;
  /** hidden for now (this page only; groups have their own saved Hide) */
  hidden: { brushes: Set<number>; entities: Set<number> };
  /** picked corners for the vertex tool */
  verts: { brush: number; p: Vec3 }[];
  /** textures used lately, newest first */
  recentTex: string[];
  /** face sheet: work on several faces as one surface */
  treatAsOne: boolean;
  /** texture tool: a click picks faces, or paints them */
  texClick: "pick" | "paint";
  /** the measure tool's line */
  measure: { a: Vec3; b: Vec3 } | null;
  /** store/go to a camera bookmark (seq changes each time) */
  bookmark: { n: number; store: boolean; seq: number } | null;
  dialog: Dialog;
  /** the last copy, for when the system clipboard can't be read */
  clip: string | null;
  /** when the last autosave went out (for the status bar) */
  autosavedAt: string | null;
  entityClass: string;
  textureLock: boolean;
  /** P5: with texture lock, a stretched brush stretches its textures (on) or repeats them (off) */
  scaleLock: boolean;
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
  clipInfo: { points: number; keep: "back" | "front" | "both"; max?: number } | null;
  doClip: (() => void) | null;
  setClipKeep: ((k: "back" | "front" | "both") => void) | null;

  setNotify: (n: Notify) => void;
  set: (p: Partial<EditorState>) => void;
  select: (s: Selection) => void;
  load: () => Promise<void>;
  /** `after` runs right after the new meshes are set, in the same synchronous step (Parity A:
   *  so an edit's meshes, document patch and status land in one React commit) */
  syncMeshes: (after?: () => void) => Promise<void>;
  refreshDoc: () => Promise<void>;
  /** P5 (perf): patch the document with a command's changes; false when they don't fit (ask for all) */
  applyChanges: (c: DocChanges | null | undefined) => boolean;
  /** P5 (perf): catch up after a change the page didn't make (only what changed, when it can) */
  catchUp: () => Promise<void>;
  refreshChecks: () => void;
  cmd: (body: Record<string, unknown>, opts?: { doc?: boolean; quiet?: boolean }) => Promise<CmdResult>;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  file: (what: "new" | "open" | "import" | "save" | "saveAs" | "export" | "recover" | number) => Promise<void>;
};

function keep(key: string, v: string) {
  try {
    localStorage.setItem(key, v);
  } catch {
    /* private window */
  }
}
function savedStr(key: string, fallback: string) {
  try {
    return localStorage.getItem(key) ?? fallback;
  } catch {
    return fallback;
  }
}
function savedList(key: string): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(key) ?? "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x === "string").slice(0, 16) : [];
  } catch {
    return [];
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
let checksBusy = false; // Checks
let checksAgain = false;
let lastLocalEdit = 0;
/** P5 (perf): when this page last sent an edit (ms). */
export const localEditAt = () => lastLocalEdit;
/** P5 (perf): the document (and limits) the shown checks are for; the same again isn't re-run */
let checksRev: string | null = null;
const CHECKS_DEBOUNCE_MS = 1200;
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
  shape: "cylinder",
  archArc: 180,
  archWall: 16,
  thickness: 8,
  selectMode: ((m) => (m === "objects" || m === "solids" ? m : "groups"))(savedStr("ls.editor.selectMode", "groups")),
  hidden: { brushes: new Set(), entities: new Set() },
  verts: [],
  recentTex: savedList("ls.editor.recentTex"),
  treatAsOne: false,
  texClick: "pick",
  measure: null,
  bookmark: null,
  dialog: null,
  clip: null,
  autosavedAt: null,
  entityClass: "info_player_start",
  textureLock: true,
  scaleLock: true,
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
    if (p.selectMode) keep("ls.editor.selectMode", p.selectMode);
    if (p.texture && p.texture !== get().texture) {
      const recentTex = [p.texture, ...get().recentTex.filter((t) => t.toLowerCase() !== p.texture!.toLowerCase())].slice(0, 16);
      keep("ls.editor.recentTex", JSON.stringify(recentTex));
      set({ ...p, recentTex });
      return;
    }
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
    set({ doc: d, loadError: null, brushes: new Map(), meshRev: null, meshDoc: null, sel: EMPTY_SEL, preview: null, checks: null, verts: [], measure: null, hidden: { brushes: new Set(), entities: new Set() } });
    await get().syncMeshes();
    get().refreshChecks();
  },

  // one mesh request at a time, each picking up from where the last left off
  syncMeshes: (after) => {
    meshChain = meshChain.then(async () => {
      const { meshDoc, meshRev } = get();
      let u: MeshUpdate;
      try {
        u = await api.getMeshes(meshDoc, meshDoc == null ? null : meshRev);
      } catch (e) {
        get().notify(e instanceof Error ? e.message : String(e), true);
        after?.();
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
      const faces = (sel.faces ?? []).filter((f) => next.has(f.brush));
      // picked corners that aren't corners any more drop out
      const verts = get().verts.filter((v) => {
        const b = next.get(v.brush);
        return b && [...b.polys.values()].some((poly) => poly.some((q) => Math.abs(q[0] - v.p[0]) + Math.abs(q[1] - v.p[1]) + Math.abs(q[2] - v.p[2]) < 0.02));
      });
      set({
        brushes: next,
        meshRev: u.revision,
        meshDoc: u.docId,
        preview: null,
        ...(verts.length !== get().verts.length ? { verts } : {}),
        ...(brushes.length !== sel.brushes.length || face !== sel.face || faces.length !== (sel.faces ?? []).length ? { sel: { ...sel, brushes, face, faces } } : {}),
      });
      after?.();
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

  applyChanges: (c) => {
    const d = get().doc;
    if (!c || !d || c.docId !== d.docId || c.from !== d.revision) return false;
    // unchanged brushes and entities keep their objects (memoized views see only what moved)
    const patch = <T extends { id: number }>(list: T[], changed: T[], removed: number[]) => {
      if (!changed.length && !removed.length) return list;
      const gone = new Set(removed);
      const fresh = new Map(changed.map((x) => [x.id, x]));
      const out = list.filter((x) => !gone.has(x.id)).map((x) => fresh.get(x.id) ?? x);
      const have = new Set(out.map((x) => x.id));
      const added = changed.filter((x) => !have.has(x.id));
      return added.length ? [...out, ...added].sort((a, b) => a.id - b.id) : out;
    };
    const { from: _from, brushes, removedBrushes, entities, removedEntities, changes: _c, ...rest } = c;
    void _from;
    void _c;
    const doc: Doc = { ...d, ...rest, brushes: patch(d.brushes, brushes, removedBrushes), entities: patch(d.entities, entities, removedEntities) };
    const ids = new Set(doc.entities.map((e) => e.id));
    const { sel } = get();
    const ents = sel.entities.filter((id) => ids.has(id));
    set({ doc, ...(ents.length !== sel.entities.length ? { sel: { ...sel, entities: ents } } : {}) });
    return true;
  },

  catchUp: async () => {
    const d = get().doc;
    if (!d) return get().refreshDoc();
    const r = await api.getDocSince(d.docId, d.revision);
    if (r.error) return get().notify(r.error, true);
    if ("changes" in r && r.changes === true) {
      if (!get().applyChanges(r as DocChanges)) await get().refreshDoc();
      return;
    }
    const full = r as Doc;
    if (full.docId !== d.docId) return get().load();
    const ids = new Set(full.entities.map((e) => e.id));
    const { sel } = get();
    const ents = sel.entities.filter((id) => ids.has(id));
    set({ doc: full, ...(ents.length !== sel.entities.length ? { sel: { ...sel, entities: ents } } : {}) });
  },

  // checks follow edits, once they settle (P5, perf: 1.2 s, and not again for a revision
  // they've seen: on br_valley a run is about half a second of the app's time)
  refreshChecks: () => {
    // Checks: debounced, one run at a time (another after it when edits came in meanwhile);
    // a run that saw an older document answers `stale` and is dropped (the next run follows)
    if (checksTimer) clearTimeout(checksTimer);
    checksTimer = setTimeout(async () => {
      if (checksBusy) {
        checksAgain = true;
        return;
      }
      // still editing (a command or a drag on its way): wait for it to settle
      if (get().working > 0 || get().preview) return get().refreshChecks();
      const rev = get().doc ? `${get().doc!.docId}:${get().doc!.revision}:${JSON.stringify(api.checkLimits())}` : null;
      if (rev && rev === checksRev) return;
      checksBusy = true;
      try {
        const c = await api.getChecks();
        if (!c.error && !c.stale) {
          set({ checks: c });
          checksRev = rev;
        }
        if (c.stale) checksAgain = true;
      } finally {
        checksBusy = false;
        if (checksAgain) {
          checksAgain = false;
          get().refreshChecks();
        }
      }
    }, CHECKS_DEBOUNCE_MS);
  },

  cmd: async (body, opts = {}) => {
    // P5: "working" lasts until the view and the outliner have caught up (on a big map the
    // meshes take longer than the command), so the status bar's "Working…" covers it all
    // (each step of a drag is quick and the drag shows itself: not counted)
    // P5: every move, turn and stretch follows the scale lock (one place, not every caller)
    if (body.op === "transform" && body.keep_scale === undefined) body = { ...body, keep_scale: !get().scaleLock };
    lastLocalEdit = Date.now(); // P5 (perf): the outside-changes watch lets the page's own edits land first
    const counts = !(body as { drag?: unknown }).drag;
    if (counts) set((s) => ({ working: s.working + 1 }));
    // Parity A: everything an edit changes on the page (its meshes, the document patch, the
    // status, a note, "working") is set in one synchronous step, so React commits it once
    let settled = !counts;
    const settle = () => {
      if (settled) return;
      settled = true;
      set((s) => ({ working: s.working - 1 }));
    };
    try {
      const r: CmdResult = await api.sendCmd(body);
      if (r.error) {
        if (r.state) mergeState(r.state);
        if (!opts.quiet) get().notify(r.error, true);
        set({ preview: null });
        return r;
      }
      let catchUp = false;
      const apply = () => {
        if (r.state) mergeState(r.state);
        if (r.note && body.op !== "group") get().notify(r.note);
        // P5 (perf): the command's own changes; the whole document only when they don't fit
        if (opts.doc !== false && (r.changed || r.docChanged) && !get().applyChanges(r.changes)) catchUp = true;
        if (!catchUp) settle();
      };
      if (r.changed) await get().syncMeshes(apply);
      else apply();
      if (opts.doc !== false) {
        if (catchUp) await get().catchUp();
        get().refreshChecks();
      }
      return r;
    } finally {
      settle();
    }
  },

  undo: async () => {
    set((s) => ({ working: s.working + 1 })); // P5
    // Parity A: the meshes, the document patch, the note and "working" in one React commit
    let settled = false;
    const settle = () => {
      if (settled) return;
      settled = true;
      set((s) => ({ working: s.working - 1 }));
    };
    try {
      const r = await api.undo();
      if (r.error) return get().notify(r.error, true);
      if (!r.changed) {
        if (r.state) mergeState(r.state);
        return;
      }
      let catchUp = false;
      await get().syncMeshes(() => {
        if (r.state) mergeState(r.state);
        if (!get().applyChanges(r.changes)) catchUp = true; // P5 (perf)
        if (r.label) get().notify(`Undid ${r.label.toLowerCase()}`);
        if (!catchUp) settle();
      });
      if (catchUp) await get().catchUp();
      get().refreshChecks();
    } finally {
      settle();
    }
  },
  redo: async () => {
    set((s) => ({ working: s.working + 1 })); // P5
    // Parity A: the meshes, the document patch, the note and "working" in one React commit
    let settled = false;
    const settle = () => {
      if (settled) return;
      settled = true;
      set((s) => ({ working: s.working - 1 }));
    };
    try {
      const r = await api.redo();
      if (r.error) return get().notify(r.error, true);
      if (!r.changed) {
        if (r.state) mergeState(r.state);
        return;
      }
      let catchUp = false;
      await get().syncMeshes(() => {
        if (r.state) mergeState(r.state);
        if (!get().applyChanges(r.changes)) catchUp = true; // P5 (perf)
        if (r.label) get().notify(`Redid ${r.label.toLowerCase()}`);
        if (!catchUp) settle();
      });
      if (catchUp) await get().catchUp();
      get().refreshChecks();
    } finally {
      settle();
    }
  },

  file: async (what) => {
    const labels = { new: "Starting a new map…", open: "Opening…", import: "Importing…", save: "Saving…", saveAs: "Saving…", export: "Exporting…", recover: "Recovering…" } as const;
    // P5 (QA4-A): a file action while another runs (Import clicked during "Starting a new map…")
    // waits for it and then runs, instead of doing nothing; a second one replaces the first
    const busy = get().fileBusy;
    if (busy) {
      fileQueued = what;
      get().notify(`${busy.replace(/…$/, "")} first. ${typeof what === "number" ? "Opening it" : labels[what].replace(/…$/, "")} right after.`);
      return;
    }
    set({ fileBusy: typeof what === "number" ? "Opening…" : labels[what] });
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
      const r =
        typeof what === "number" ? await api.openRecent(what) : what === "recover" ? await api.recover() : what === "new" ? await api.newMap() : what === "open" ? await api.openProject() : await api.importMap();
      if (r.error) return get().notify(r.error, true);
      if ("cancelled" in r && r.cancelled) return;
      await get().load();
      // P5: what an imported map is missing (models, sprites, sounds), said at once
      const notes = (r as { notes?: string[] }).notes;
      if (notes?.length) get().notify(`Imported. ${notes.join(" ")}`, true);
    } finally {
      set({ fileBusy: null });
      const next = fileQueued;
      fileQueued = null;
      if (next !== null) void get().file(next);
    }
  },
}));

/** A file action waiting for the one that's running (P5). */
let fileQueued: Parameters<EditorState["file"]>[0] | null = null;

/** Undo labels, dirty state and the like arrive with every answer. */
function mergeState(st: DocState) {
  const d = useEditor.getState().doc;
  // P5 (perf): not the revision: that's the revision of the content the page holds, moved by
  // the changes (applyChanges) or a whole document, so "what changed since" stays right
  // (nor the document id: another document is loaded whole)
  if (d) {
    const { revision: _r, docId: _i, ...rest } = st;
    void _r;
    void _i;
    useEditor.setState({ doc: { ...d, ...rest } });
  }
}
export { mergeState };

/** Grid snapping (off while Alt is held). */
export function snap(v: number, grid = useEditor.getState().grid) {
  const st = useEditor.getState();
  return st.alt !== !!st.snapOff || grid <= 0 ? Math.round(v * 100) / 100 : Math.round(v / grid) * grid; // P5: snapOff toggle, Alt flips it
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

/** Brushes and entities hidden for now: by this page, or in a hidden group. */
export function isHiddenBrush(b: { id: number; group: number | null; entity: number | null }, s = useEditor.getState(), hiddenGroups?: Set<number>) {
  if (s.hidden.brushes.has(b.id) || (b.entity != null && s.hidden.entities.has(b.entity))) return true;
  return b.group != null && !!hiddenGroups?.has(b.group);
}

/** The group a click picks in "groups" mode: the outermost group the thing is in. */
export function topGroup(g: number | null, doc = useEditor.getState().doc): number | null {
  if (g == null || !doc) return g;
  const parent = new Map(doc.groups.map((x) => [x.id, x.parent]));
  let cur = g;
  for (let i = 0; i < 32; i++) {
    const p = parent.get(cur);
    if (p == null) break;
    cur = p;
  }
  return cur;
}

/** Everything in a group (with groups inside it). */
export function groupMembers(g: number, doc = useEditor.getState().doc): { brushes: number[]; entities: number[] } {
  if (!doc) return { brushes: [], entities: [] };
  const inside = new Set([g]);
  let grew = true;
  while (grew) {
    grew = false;
    for (const x of doc.groups) if (x.parent != null && inside.has(x.parent) && !inside.has(x.id)) {
      inside.add(x.id);
      grew = true;
    }
  }
  const entities = doc.entities.filter((e) => e.group != null && inside.has(e.group)).map((e) => e.id);
  const owned = new Set(entities);
  const brushes = doc.brushes.filter((b) => b.group != null && inside.has(b.group) && (b.entity == null || !owned.has(b.entity))).map((b) => b.id);
  return { brushes, entities };
}

// P5: dev builds let the browser tests drive the store (like __lsEditorGl for draw counts)
if (import.meta.env.DEV) (window as unknown as { __lsEditor?: typeof useEditor }).__lsEditor = useEditor;
