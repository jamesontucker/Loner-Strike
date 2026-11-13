// The map doctor's server side (tools/doctor.rs).

import { getJson, postJson } from "../../../api";

export type FileStatus = "ok" | "elsewhere" | "missing";

export type FileRow = {
  /** the game path ("models/props/barrel.mdl", "mywad.wad") */
  path: string;
  kind: "texture file" | "sky" | "model" | "sprite" | "sound" | "overview" | "file";
  status: FileStatus;
  /** where it is: the game folder (ok) or the other folder it's in (elsewhere) */
  folder: string | null;
  /** every player has it (not listed in the .res) */
  stock: boolean;
  /** texture files: how many of the map's textures come from it */
  textures?: number;
  error?: string;
};

export type Report = {
  name: string;
  game: string;
  title: string;
  size: number | null;
  bsp: { ok: boolean; error: string | null };
  textures: { total: number; embedded: number; missing: string[]; fixable: number; wads: FileRow[] };
  files: FileRow[];
  res: { stockMap: boolean; exists: boolean; listed: number; missing: string[]; broken: string[]; wanted: number };
  nav: { exists: boolean; file?: string; version?: number; areas?: number; places?: number; namedAreas?: number; matchesMap?: boolean | null; error?: string };
  lsv: { exists: boolean; stage?: string; done?: boolean; fits?: boolean | null; error?: string };
  looksIn: string[];
  problems: number;
  gamebanana: { url: string; name: string } | null;
  inUse: boolean;
  error?: undefined;
};

type Err = { error?: string };
const post = <T>(path: string, body: unknown = {}) => postJson<T & Err>(path, body).catch(() => ({ error: "Couldn't reach the app." }) as T & Err);
const base = (map: string) => `/api/tools/doctor/${encodeURIComponent(map)}`;

export const checkMap = async (map: string): Promise<Report> => {
  const r = await getJson<Report | { error: string }>(base(map));
  if (r.error) throw new Error(r.error);
  return r as Report;
};
export const writeRes = (map: string) => post<{ ok?: boolean; message?: string; listed?: number }>(`${base(map)}/res`);
export const copyFiles = (map: string, files: string[]) => post<{ ok?: boolean; message?: string; copied?: string[] }>(`${base(map)}/copy`, { files });
export const learnMap = (map: string, relearn: boolean) => post<{ ok?: boolean; job?: string }>("/api/navs/learn", { maps: [map], relearn });
export const learnResult = () => getJson<{ result?: { message?: string } | null }>("/api/navs/learn").catch(() => null);
