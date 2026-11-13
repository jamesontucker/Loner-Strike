// The Plugin Studio's server side (tools/plugins.rs).

import { post as postSafe, tryGet } from "../common/api";

export type FolderId = "mine" | "game" | "include";
export type SourceFile = { name: string; size: number; modified: number; readOnly: boolean; changed?: boolean };
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
/** includes: names for #include <...>; files: include files read; empty: files with nothing usable in them */
export type Words = { includes: string[]; files?: number; unread?: string[]; empty?: string[]; symbols: Word[] };

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
// (the tools' shared client: a save that kept backups offers Undo)
const post = <T>(path: string, body: unknown = {}) => postSafe<T>(path, body);
const get = <T>(path: string) => tryGet<T>(path);
const q = (folder: FolderId, name: string) => `folder=${encodeURIComponent(folder)}&name=${encodeURIComponent(name)}`;

const sure = <T>(r: T & Err): T => {
  if (r.error) throw new Error(r.error);
  return r;
};

export const overview = async () => sure(await get<Overview>("/api/tools/plugins"));
export const words = async () => sure(await get<Words>("/api/tools/plugins/words"));
/** A file as the page opens it. readOnly: why it can't be changed here; reset: Loner-Strike ships a
 *  version of it (differs: the file on disk isn't that version); resetHint: how else to put it back;
 *  history: its path for the safety net's History. */
export type OpenedFile = { name: string; folder: FolderId; text: string; encoding: string; readOnly: string | null; reset: { differs: boolean } | null; resetHint: string | null; history: string };
export const readFile = async (folder: FolderId, name: string) => sure(await get<OpenedFile>(`/api/tools/plugins/file?${q(folder, name)}`));
export const saveFile = (folder: FolderId, name: string, text: string, encoding = "utf8", create = false) =>
  post<{ ok?: boolean; size?: number; modified?: number }>("/api/tools/plugins/file", { folder, name, text, encoding, create });
export const renameFile = (folder: FolderId, name: string, to: string) => post<{ ok?: boolean; name?: string }>("/api/tools/plugins/rename", { folder, name, to });
export const resetFile = (folder: FolderId, name: string) => post<{ ok?: boolean; message?: string }>("/api/tools/plugins/reset", { folder, name });
export const deleteFile = (folder: FolderId, name: string) => post<{ ok?: boolean }>("/api/tools/plugins/delete", { folder, name });
export const compile = (folder: FolderId, name: string) => post<CompileResult>("/api/tools/plugins/compile", { folder, name });
export const tryIt = (folder: FolderId, name: string) => post<CompileResult>("/api/tools/plugins/try", { folder, name });
export const stopTrying = (name: string, restart: boolean) => post<{ ok?: boolean; message?: string; trying?: string[] }>("/api/tools/plugins/untry", { name, restart });
export const pluginLog = (name: string, cursor: string | null) =>
  get<{ lines: LogLine[]; cursor: string }>(`/api/tools/plugins/log?name=${encodeURIComponent(name)}${cursor ? `&cursor=${encodeURIComponent(cursor)}` : ""}`);
