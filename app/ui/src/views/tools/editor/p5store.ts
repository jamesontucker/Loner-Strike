// P5: the editor's "common things" state: which side tab is open (so menus can open one),
// which P5 dialog is up, locked things, grid display and snapping options, and the
// routes for project files, history and the many-things commands (tools/editor/workspace.rs).

import { create } from "zustand";
import { postJsonAt } from "../../../api";
import type { DocState } from "./editorApi";
import { useEditor } from "./store";

export type SideTab = "inspect" | "outline" | "textures" | "checks" | "compile" | "history" | "project" | (string & {});
export type P5Dialog = "palette" | "cheats" | "keys" | "find" | "transform" | "goto" | "grid" | "replaceWith" | "info" | null;

type P5State = {
  tab: SideTab;
  dialog: P5Dialog;
  /** palette opened with text already in it (e.g. "> " for commands only) */
  paletteText: string;
  /** locked for now: can't be picked in the views (this page only) */
  locked: { brushes: Set<number>; entities: Set<number> };
  showGrid: boolean;
  /** degrees the rotate gizmo snaps to (0: free) */
  rotSnap: number;
  /** snap new points to corners/edges of brushes near the mouse */
  geoSnap: boolean;
  /** studio models drawn in place of entity boxes */
  showModels: boolean;
  /** the arch tool's start angle (degrees) */
  archStart: number;
  /** brighter grid lines every 64 and 1024 units (Hammer's highlight lines) */
  gridMajor: boolean;
  set: (p: Partial<Omit<P5State, "set">>) => void;
};

function num(key: string, f: number) {
  try {
    const n = Number(localStorage.getItem(key));
    return localStorage.getItem(key) != null && Number.isFinite(n) ? n : f;
  } catch {
    return f;
  }
}
function keep(key: string, v: string) {
  try {
    localStorage.setItem(key, v);
  } catch {
    /* private window */
  }
}

export const useP5 = create<P5State>((set) => ({
  tab: "inspect",
  dialog: null,
  paletteText: "",
  locked: { brushes: new Set(), entities: new Set() },
  showGrid: num("ls.editor.showGrid", 1) !== 0,
  rotSnap: num("ls.editor.rotSnap", 15),
  geoSnap: num("ls.editor.geoSnap", 0) !== 0,
  showModels: num("ls.editor.showModels", 1) !== 0,
  archStart: 0,
  gridMajor: num("ls.editor.gridMajor", 1) !== 0,
  set: (p) => {
    if (p.gridMajor != null) keep("ls.editor.gridMajor", p.gridMajor ? "1" : "0");
    if (p.showModels != null) keep("ls.editor.showModels", p.showModels ? "1" : "0");
    if (p.showGrid != null) keep("ls.editor.showGrid", p.showGrid ? "1" : "0");
    if (p.rotSnap != null) keep("ls.editor.rotSnap", String(p.rotSnap));
    if (p.geoSnap != null) keep("ls.editor.geoSnap", p.geoSnap ? "1" : "0");
    set(p);
  },
}));

// The entity tool remembers the last class you placed with, across visits.
try {
  const last = localStorage.getItem("ls.editor.lastClass");
  if (last) useEditor.setState({ entityClass: last });
} catch {
  /* private window */
}
let lastClass = useEditor.getState().entityClass;
useEditor.subscribe((s) => {
  if (s.entityClass !== lastClass) {
    lastClass = s.entityClass;
    if (lastClass) keep("ls.editor.lastClass", lastClass);
  }
});

// Locked things (and things in locked groups) never stay picked.
useEditor.subscribe((s, prev) => {
  if (s.sel === prev.sel) return;
  const { locked } = useP5.getState();
  const lockedGroups = new Set((s.doc?.groups ?? []).filter((g) => g.locked).map((g) => g.id));
  if (!locked.brushes.size && !locked.entities.size && !lockedGroups.size) return;
  const ents = new Map((s.doc?.entities ?? []).map((e) => [e.id, e]));
  const brushGroup = new Map((s.doc?.brushes ?? []).map((b) => [b.id, b.group]));
  const okB = (id: number) => !locked.brushes.has(id) && !lockedGroups.has(brushGroup.get(id) ?? -1);
  const okE = (id: number) => !locked.entities.has(id) && !lockedGroups.has(ents.get(id)?.group ?? -1);
  const brushes = s.sel.brushes.filter(okB);
  const entities = s.sel.entities.filter(okE);
  if (brushes.length !== s.sel.brushes.length || entities.length !== s.sel.entities.length) useEditor.setState({ sel: { ...s.sel, brushes, entities } });
});

// ---- routes (tools/editor/workspace.rs) ------------------------------------------------

type Err = { error?: string };
const E = "/api/tools/editor";

async function post<T>(path: string, body: unknown = {}): Promise<T & Err> {
  try {
    return await postJsonAt<T & Err>(path, body, null);
  } catch {
    return { error: "Couldn't reach the app." } as T & Err;
  }
}
async function get<T>(path: string): Promise<T & Err> {
  try {
    const r = await fetch(path, { credentials: "same-origin" });
    return (await r.json()) as T & Err;
  } catch {
    return { error: "Couldn't reach the app." } as T & Err;
  }
}

export type ProjectFile = {
  group: string;
  label: string;
  path: string | null;
  exists: boolean;
  missing: boolean;
  size: number | null;
  modified: number | null;
  note: string;
  openHere: boolean;
  canRename: boolean;
  canDelete: boolean;
};
export type AssetsInfo = { missing: number; toGame: number; game: boolean; saved: boolean; assets: { path: string; kind: string; status: "game" | "beside" | "elsewhere" | "missing"; stock?: boolean; who: string; inProject: boolean }[] };
export type ProjectInfo = { name: string; map: string; project: string | null; folder: string | null; game: boolean; assets?: AssetsInfo };
export type Ids = { brushes: number[]; entities: number[] };
export type Found = Ids & { count: number };
export type FindSpec = { field: "texture" | "classname" | "key" | "value"; find: string; replace?: string; key?: string | null; whole?: boolean; case?: boolean; brushes?: number[] | null; entities?: number[] | null };

export type Cmd2Result = Err & {
  created?: Ids;
  select?: Ids;
  found?: Found;
  replaced?: number;
  moved?: number;
  note?: string;
  changed?: boolean;
  docChanged?: boolean;
  state?: DocState;
};

export const getProject = () => get<{ files: ProjectFile[]; project: ProjectInfo }>(`${E}/project`);
export const projectAction = (action: "open" | "reveal" | "rename" | "delete", path: string, name = "") =>
  post<{ done?: boolean; opened?: boolean; renamed?: string; deleted?: boolean; keptIn?: string; state?: DocState | null }>(`${E}/project/action`, { action, path, name });
/** Copies the models, sprites and sounds the map uses into the game's download folder or next to the project. */
export const copyAssets = (to: "game" | "project") => post<AssetsInfo & { copied: string[]; message: string }>(`${E}/assets/copy`, { to });
export const importRmf = () => post<{ cancelled?: boolean; notes?: string[] }>(`${E}/import-other`);
export const getHistory = () => get<{ undo: string[]; redo: string[]; dirty: boolean }>(`${E}/history`);
export const jumpHistory = (steps: number) => post<{ changed: boolean; steps: number; state?: DocState }>(`${E}/history/jump`, { steps });

/** A many-things command; edits refresh the view like the editor's own commands. */
export async function cmd2(body: Record<string, unknown>, opts: { quiet?: boolean } = {}): Promise<Cmd2Result> {
  const s = useEditor.getState();
  s.set({ working: s.working + 1 });
  let r: Cmd2Result;
  try {
    r = await post<Cmd2Result>(`${E}/cmd2`, body);
  } finally {
    useEditor.setState((x) => ({ working: x.working - 1 }));
  }
  const d = useEditor.getState().doc;
  if (r.state && d) useEditor.setState({ doc: { ...d, ...r.state } });
  if (r.error) {
    if (!opts.quiet) s.notify(r.error, true);
    return r;
  }
  if (r.changed) await useEditor.getState().syncMeshes();
  if (r.changed || r.docChanged) {
    await useEditor.getState().refreshDoc();
    useEditor.getState().refreshChecks();
  }
  return r;
}

/** After an undo/redo-like jump or a file opened from the project list. */
export async function afterJump(state?: DocState | null) {
  const s = useEditor.getState();
  if (state && s.doc) useEditor.setState({ doc: { ...s.doc, ...state } });
  await s.syncMeshes();
  await s.refreshDoc();
  s.refreshChecks();
}
