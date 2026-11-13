// The map editor's server side (tools/editor.rs, TOOLS-PLAN 3.8): one open document held
// by the app with its undo history. The page sends commands and asks for what changed by
// revision; files are only ever picked with the system's dialog.

import { postJsonAt } from "../../../api";
import { readLsmb } from "../viewer/bundle";
import type { Vec3 } from "../viewer/coords";

export type DocState = { docId: number; revision: number; name: string; saved: boolean; dirty: boolean; undo: string | null; redo: string | null };

export type DocBrush = { id: number; entity: number | null; group: number | null; min: Vec3; max: Vec3 };
export type DocEntity = { id: number; classname: string; keys: [string, string][]; group: number | null; brushes: number[] };
export type DocGroup = { id: number; name: string; parent: number | null; hidden: boolean; locked: boolean };
export type Stats = { brushes: number; faces: number; entities: number; point_entities: number; brush_entities: number; textures: number };

export type Doc = DocState & {
  world: { wads: string[]; keys: [string, string][] };
  brushes: DocBrush[];
  entities: DocEntity[];
  groups: DocGroup[];
  bounds: [Vec3, Vec3];
  stats: Stats;
};

export type Issue = { level: "info" | "warning" | "error"; code: string; message: string; brush?: number; entity?: number; texture?: string; fix?: string };
export type Checks = { docId: number; revision: number; issues: Issue[]; stats: Stats; texturesChecked: boolean };

export type WadTextures = { name: string; found: boolean; error?: string; textures: { name: string; width: number; height: number }[] };

/** One face of a brush as drawn: a polygon (fan) in the bundle's vertex arrays. */
export type MeshFace = {
  face: number;
  texture: string;
  normal: Vec3;
  start: number;
  count: number;
  /** Valve 220: [u axis x3, u offset, v axis x3, v offset, rotation, u scale, v scale] */
  align?: number[];
};

/** A face's alignment, unpacked. */
export type Align = { u: Vec3; v: Vec3; ou: number; ov: number; rot: number; su: number; sv: number };
export function alignOf(f: MeshFace | undefined): Align | null {
  const a = f?.align;
  if (!a || a.length < 11) return null;
  return { u: [a[0], a[1], a[2]], ou: a[3], v: [a[4], a[5], a[6]], ov: a[7], rot: a[8], su: a[9], sv: a[10] };
}

/** Entity definitions (tools/editor.rs /fgd, from our Counter-Strike FGD). */
export type FgdKey = {
  name: string;
  type: string;
  label: string;
  default?: string;
  help?: string;
  choices?: { value: string; label: string }[];
  flags?: { bit: number; label: string; on: boolean }[];
};
export type FgdClass = { name: string; kind: "point" | "solid"; description: string; size?: number[]; color?: number[]; keys: FgdKey[] };
export type RecentFile = { index: number; name: string; folder: string; project: boolean; found: boolean };
export type Recovery = { found: boolean; name?: string; time?: number; brushes?: number; entities?: number; project?: boolean };
export type MeshBrush = { id: number; entity: number | null; group: number | null; faces: MeshFace[] };
export type MeshUpdate = { docId: number; revision: number; full: boolean; removed: number[]; brushes: MeshBrush[]; pos: Float32Array; uv: Float32Array };

export type CmdResult = {
  error?: string;
  created?: { brushes: number[]; entities: number[] };
  note?: string | null;
  changed?: boolean;
  docChanged?: boolean;
  label?: string | null;
  state?: DocState;
};


type Err = { error?: string };

/** JSON GET; a failure to arrive reads like any other error. */
async function getJson<T>(path: string): Promise<T & Err> {
  try {
    const r = await fetch(path, { credentials: "same-origin" });
    return (await r.json()) as T & Err;
  } catch {
    return { error: "Couldn't reach the app." } as T & Err;
  }
}

async function post<T>(path: string, body: unknown = {}): Promise<T & Err> {
  try {
    return await postJsonAt<T & Err>(path, body, null);
  } catch {
    return { error: "Couldn't reach the app." } as T & Err;
  }
}

const E = "/api/tools/editor";

export const getDoc = () => getJson<Doc>(`${E}/doc`);
export const getChecks = () => getJson<Checks>(`${E}/checks`);
export const getTextures = () => getJson<{ wads: WadTextures[]; noGame?: boolean }>(`${E}/textures`);
export const getGameWads = () => getJson<{ wads: { name: string; game: string }[] }>(`${E}/wads`);
export const texUrl = (name: string) => `${E}/tex/${encodeURIComponent(name)}.png`;

export const sendCmd = (body: Record<string, unknown>) => post<CmdResult>(`${E}/cmd`, body);
export const undo = () => post<CmdResult>(`${E}/undo`);
export const redo = () => post<CmdResult>(`${E}/redo`);
export const newMap = () => post<DocState>(`${E}/new`);
export const openProject = () => post<DocState & { cancelled?: boolean }>(`${E}/open`);
export const importMap = () => post<DocState & { cancelled?: boolean }>(`${E}/import-map`);
export const saveProject = (as = false) => post<{ cancelled?: boolean; saved?: boolean; file?: string; state?: DocState }>(`${E}/save`, { as });
export const copySelection = (brushes: number[], entities: number[]) => post<{ text: string; brushes: number; entities: number }>(`${E}/copy`, { brushes, entities });
export const getFgd = () => getJson<{ classes: FgdClass[]; source: string }>(`${E}/fgd`);
export const getRecent = () => getJson<{ files: RecentFile[] }>(`${E}/recent`);
export const openRecent = (index: number) => post<DocState>(`${E}/open-recent`, { index });
export const autosave = () => post<{ saved: boolean }>(`${E}/autosave`);
export const getRecovery = () => getJson<Recovery>(`${E}/recovery`);
export const recover = () => post<DocState>(`${E}/recover`);
export const discardRecovery = () => post<{ discarded: boolean }>(`${E}/discard-recovery`);
export const exportMap = () => post<{ cancelled?: boolean; exported?: boolean; file?: string }>(`${E}/export-map`);

/** The brush meshes that changed since a revision (all of them without one). */
export async function getMeshes(docId: number | null, since: number | null): Promise<MeshUpdate> {
  const q = new URLSearchParams();
  if (docId != null) q.set("doc", String(docId));
  if (since != null) q.set("since", String(since));
  let r: Response;
  try {
    r = await fetch(`${E}/meshes?${q}`, { credentials: "same-origin" });
  } catch {
    throw new Error("Couldn't reach the app.");
  }
  if (!r.ok) {
    let msg = "Couldn't read the map's shapes.";
    try {
      msg = String((await r.json()).error ?? msg);
    } catch {
      /* keep the plain words */
    }
    throw new Error(msg);
  }
  const b = readLsmb(await r.arrayBuffer(), "The map's shapes couldn't be read.");
  const h = b.header as Partial<MeshUpdate>;
  return {
    docId: Number(h.docId),
    revision: Number(h.revision),
    full: !!h.full,
    removed: Array.isArray(h.removed) ? h.removed : [],
    brushes: Array.isArray(h.brushes) ? h.brushes : [],
    pos: (b.get("pos") as Float32Array | null) ?? new Float32Array(),
    uv: (b.get("uv") as Float32Array | null) ?? new Float32Array(),
  };
}
