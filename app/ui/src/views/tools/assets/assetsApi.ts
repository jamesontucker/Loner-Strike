// The texture and model browser's server side (tools/assets.rs). Files are named by game
// folder + file name (WADs) or folder + path under models/ (models); new files come from the
// system's file dialog, never a path from the page.

import { useQuery } from "@tanstack/react-query";
import { getJson, postJson } from "../../../api";
import { readLsmb } from "../viewer/bundle";

export type WadInfo = { folder: string; name: string; size: number; textures?: number; custom: boolean; stock: boolean; error?: string };
export type WadTexture = { name: string; width: number; height: number; kind: "texture" | "decal" };
export type Usage = {
  maps: string[];
  /** texture name (lower case) → indexes into maps */
  textures: Record<string, number[]>;
  /** [map index, name, width, height] */
  embedded: [number, string, number, number][];
  wads: Record<string, number[]>;
  models: Record<string, number[]>;
};
export type ModelKind = "player" | "view" | "carried" | "world" | "hostage" | "prop";
export type ModelItem = { folder: string; path: string; name: string; kind: ModelKind; size: number; imported: boolean };
export type ModelInfo = { name: string; bones: number; sequences: number; textures: number; skins: number; bodyparts: number; maps: string[] };

type Err = { error?: string };

/** JSON routes answer {error} with 200: that's a failure too. */
async function json<T>(path: string): Promise<T> {
  const v = await getJson<T & Err>(path);
  if (v && typeof v === "object" && typeof (v as Err).error === "string") throw new Error((v as Err).error);
  return v;
}

/** A POST whose failure to arrive reads like any other error. */
const post = <T>(path: string, body: unknown = {}) => postJson<T & Err>(path, body).catch(() => ({ error: "Couldn't reach the app." }) as T & Err);

const enc = encodeURIComponent;

export const wadTextureUrl = (folder: string, wad: string, tex: string) => `/api/tools/assets/wad/${enc(folder)}/${enc(wad)}/tex/${enc(tex)}.png`;
/** A map's own texture (served by the map viewer's route). */
export const mapTextureUrl = (map: string, tex: string) => `/api/tools/map/${enc(map)}/tex/${enc(tex)}.png`;
const modelQuery = (m: { folder: string; path: string }) => `folder=${enc(m.folder)}&path=${enc(m.path)}`;
export const modelTextureUrl = (m: { folder: string; path: string }, i: number) => `/api/tools/assets/model/tex?${modelQuery(m)}&i=${i}`;

export const useWads = () => useQuery({ queryKey: ["tools", "assets", "wads"], queryFn: () => json<{ wads: WadInfo[]; game: string }>("/api/tools/assets/wads") });
export const useWadTextures = (wad: WadInfo | null) =>
  useQuery({
    queryKey: ["tools", "assets", "wad", wad?.folder, wad?.name, wad?.size],
    queryFn: () => json<{ textures: WadTexture[] }>(`/api/tools/assets/wad/${enc(wad!.folder)}/${enc(wad!.name)}`).then((r) => r.textures),
    enabled: !!wad,
  });
export const useUsage = () => useQuery({ queryKey: ["tools", "assets", "usage"], queryFn: () => json<Usage>("/api/tools/assets/usage"), staleTime: 60_000 });
export const useModels = () => useQuery({ queryKey: ["tools", "assets", "models"], queryFn: () => json<{ models: ModelItem[] }>("/api/tools/assets/models").then((r) => r.models) });
export const useModelInfo = (m: ModelItem | null) =>
  useQuery({ queryKey: ["tools", "assets", "modelinfo", m?.folder, m?.path], queryFn: () => json<ModelInfo>(`/api/tools/assets/model/info?${modelQuery(m!)}`), enabled: !!m });

// ---- the model bundle (LSMB, see assets.rs model_bundle) ------------------------------------

export type ModelDraw = { part: number; model: number; mesh: number; skinRef: number; start: number; count: number };
export type ModelTexture = { name: string; width: number; height: number; flags: number; masked: boolean; additive: boolean; chrome: boolean; fullbright: boolean };
export type ModelSequence = { name: string; fps: number; frames: number; loop: boolean; activity: number; group: number };
export type ModelBundle = {
  name: string;
  positions: Float32Array;
  normals: Float32Array;
  uv: Float32Array;
  local: Float32Array;
  localNormals: Float32Array;
  bone: Uint16Array;
  normalBone: Uint16Array;
  indices: Uint32Array;
  draws: ModelDraw[];
  textures: ModelTexture[];
  skins: number[][];
  bones: { name: string; parent: number }[];
  sequences: ModelSequence[];
  bodyparts: { name: string; base: number; models: { name: string; triangles: number }[] }[];
  bounds: [[number, number, number], [number, number, number]];
};

const DAMAGED = "This model couldn't be read. The file may be damaged.";

async function fetchBin(path: string, signal?: AbortSignal): Promise<ArrayBuffer> {
  let r: Response;
  try {
    r = await fetch(path, { credentials: "same-origin", signal });
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") throw e;
    throw new Error("Couldn't reach the app. Try again.");
  }
  if (!r.ok) {
    let msg = "";
    try {
      msg = String((await r.json()).error ?? "");
    } catch {
      /* no words */
    }
    throw new Error(msg || DAMAGED);
  }
  return r.arrayBuffer();
}

export async function getModel(m: { folder: string; path: string }, signal?: AbortSignal): Promise<ModelBundle> {
  const { header: h, get } = readLsmb(await fetchBin(`/api/tools/assets/model?${modelQuery(m)}`, signal), DAMAGED);
  const f32 = (n: string) => {
    const a = get(n);
    if (!(a instanceof Float32Array)) throw new Error(DAMAGED);
    return a;
  };
  const u16 = (n: string) => {
    const a = get(n);
    if (!(a instanceof Uint16Array)) throw new Error(DAMAGED);
    return a;
  };
  const indices = get("indices");
  if (!(indices instanceof Uint32Array)) throw new Error(DAMAGED);
  const positions = f32("positions");
  const verts = positions.length / 3;
  for (let i = 0; i < indices.length; i++) if (indices[i] >= verts) throw new Error(DAMAGED);
  const draws = (Array.isArray(h.draws) ? h.draws : []) as ModelDraw[];
  if (draws.some((d) => d.start < 0 || d.count < 0 || d.start + d.count > indices.length)) throw new Error(DAMAGED);
  const b = h.bounds as ModelBundle["bounds"] | undefined;
  return {
    name: String(h.name ?? ""),
    positions,
    normals: f32("normals"),
    uv: f32("uv"),
    local: f32("local"),
    localNormals: f32("localNormals"),
    bone: u16("bone"),
    normalBone: u16("normalBone"),
    indices,
    draws,
    textures: (h.textures ?? []) as ModelTexture[],
    skins: (h.skins ?? []) as number[][],
    bones: (h.bones ?? []) as ModelBundle["bones"],
    sequences: (h.sequences ?? []) as ModelSequence[],
    bodyparts: (h.bodyparts ?? []) as ModelBundle["bodyparts"],
    bounds: b && b.length === 2 ? b : [[-16, -16, 0], [16, 16, 72]],
  };
}

/** One sequence's frames: per frame, per bone, x y z and a quaternion (x y z w). */
export type Animation = { fps: number; frames: number; bones: number; loop: boolean; data: Float32Array };

export async function getAnimation(m: { folder: string; path: string }, seq: number, signal?: AbortSignal): Promise<Animation> {
  const msg = "That animation couldn't be read.";
  const { header: h, get } = readLsmb(await fetchBin(`/api/tools/assets/model/anim?${modelQuery(m)}&seq=${seq}`, signal), msg);
  const data = get("frames");
  const frames = Number(h.frames) || 0, bones = Number(h.bones) || 0;
  if (!(data instanceof Float32Array) || data.length !== frames * bones * 7) throw new Error(msg);
  return { fps: Number(h.fps) || 30, frames, bones, loop: !!h.loop, data };
}

// ---- actions ----------------------------------------------------------------------------------

export type ExportSource = { kind: "wad"; folder: string; wad: string; tex: string } | { kind: "map"; map: string; tex: string } | { kind: "model"; folder: string; path: string; i: number };
export const exportPng = (s: ExportSource) => post<{ ok?: boolean; cancelled?: boolean; message?: string }>("/api/tools/assets/export", s);
export const pickPicture = () => post<{ cancelled?: boolean; name?: string; width?: number; height?: number; suggested?: string }>("/api/tools/assets/texture/pick");
export type TextureChoice = { name: string; size: number; wad: string };
export const previewTexture = (c: TextureChoice) => post<{ png?: string; width?: number; height?: number }>("/api/tools/assets/texture/preview", c);
export const saveTexture = (c: TextureChoice) => post<{ ok?: boolean; wad?: string; folder?: string; texture?: string; message?: string }>("/api/tools/assets/texture/save", c);
export const addOwnFile = (kind: "wad" | "model") => post<{ ok?: boolean; cancelled?: boolean; job?: string; folder?: string; path?: string; message?: string }>("/api/tools/assets/add-file", { kind });
export const removeModel = (m: { folder: string; path: string }) => post<{ ok?: boolean }>("/api/tools/assets/model/remove", { folder: m.folder, path: m.path });
