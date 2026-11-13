// The Plugin Studio's server side (tools/plugins.rs).

import { getJson, postJson } from "../../../api";

export type FolderId = "mine" | "game" | "include";
export type SourceFile = { name: string; size: number; modified: number; readOnly: boolean };
export type Folder = { id: FolderId; label: string; path: string; files: SourceFile[] };
export type Overview = { game: string; compiler: boolean; compilerProblem: string | null; running: boolean; trying: string[]; folders: Folder[] };

export type Param = { label: string; name: string; doc: string };
export type Word = {
  name: string;
  kind: "native" | "forward" | "stock" | "define" | "macro" | "constant" | "enum";
  detail: string;
  params: Param[];
  tag?: string;
  doc: string;
  returns?: string;
  notes?: string[];
  error?: string;
  deprecated?: string;
  value?: string;
  file: string;
  line: number;
};
export type Words = { includes: string[]; symbols: Word[] };

export type Diag = { file: string; inSource: boolean; line: number; endLine: number; severity: "error" | "warning" | "fatal"; code: number; message: string; plain: string };
export type CompileResult = {
  ok: boolean;
  message: string;
  diagnostics: Diag[];
  errors: number;
  warnings: number;
  output: string;
  size: number;
  ms: number;
  tried?: boolean;
  reloaded?: boolean;
  trying?: string[];
};
export type LogLine = { at: string; text: string; error: boolean };

type Err = { error?: string };
const reach = { error: "Couldn't reach the app." };
const post = <T>(path: string, body: unknown = {}) => postJson<T & Err>(path, body).catch(() => reach as T & Err);
const get = <T>(path: string) => getJson<T & Err>(path).catch(() => reach as T & Err);
const q = (folder: FolderId, name: string) => `folder=${encodeURIComponent(folder)}&name=${encodeURIComponent(name)}`;

const sure = <T>(r: T & Err): T => {
  if (r.error) throw new Error(r.error);
  return r;
};

export const overview = async () => sure(await get<Overview>("/api/tools/plugins"));
export const words = async () => sure(await get<Words>("/api/tools/plugins/words"));
export const readFile = async (folder: FolderId, name: string) =>
  sure(await get<{ name: string; folder: FolderId; text: string; encoding: string; readOnly: string | null }>(`/api/tools/plugins/file?${q(folder, name)}`));
export const saveFile = (folder: FolderId, name: string, text: string, encoding = "utf8", create = false) =>
  post<{ ok?: boolean; size?: number; modified?: number }>("/api/tools/plugins/file", { folder, name, text, encoding, create });
export const deleteFile = (folder: FolderId, name: string) => post<{ ok?: boolean }>("/api/tools/plugins/delete", { folder, name });
export const compile = (folder: FolderId, name: string) => post<CompileResult>("/api/tools/plugins/compile", { folder, name });
export const tryIt = (folder: FolderId, name: string) => post<CompileResult>("/api/tools/plugins/try", { folder, name });
export const stopTrying = (name: string, restart: boolean) => post<{ ok?: boolean; message?: string; trying?: string[] }>("/api/tools/plugins/untry", { name, restart });
export const pluginLog = (name: string, cursor: string | null) =>
  get<{ lines: LogLine[]; cursor: string }>(`/api/tools/plugins/log?name=${encodeURIComponent(name)}${cursor ? `&cursor=${encodeURIComponent(cursor)}` : ""}`);
