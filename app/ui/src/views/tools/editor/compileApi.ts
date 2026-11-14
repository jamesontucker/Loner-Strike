// The map compiler's server side (tools/compile.rs): SDHLT downloaded on first use, one job at a
// time (a download or a compile) whose output the page streams, and the leak line for the 3D view.
// Also the app's own routes a playtest goes through (the map doctor's .res, nav learning, the
// game launch), so the panel needs nothing else.

import { create } from "zustand";
import { getJsonAt, postJsonAt } from "../../../api";
import type { Vec3 } from "../viewer/coords";

export type Preset = "fast" | "normal" | "final";

export type CompileStatus = {
  installed: boolean;
  version: string;
  bytes: number;
  url: string;
  licence: string;
  folder: string;
  canRun: boolean;
  cantRun: string | null;
  suggestName: string | null;
  busy: boolean;
  /** this computer's cores, and how many the compiler uses unless "all cores" is on */
  cores?: number;
  threads?: number;
  error?: string;
};

export type Problem = {
  level: "error" | "warning";
  kind: "leak" | "limit" | "texture" | "wad" | "brush" | "other";
  message: string;
  detail: string;
  help?: string;
  brush?: number;
  entity?: number;
  at?: Vec3;
};

export type CompileResult = { map: string; bsp: string; bytes: number; preset: Preset; installed: boolean; backup?: string | null; hasNav?: boolean };

export type Job = {
  id: number;
  kind: "setup" | "compile";
  running: boolean;
  stage: string;
  label: string;
  fraction: number;
  seconds: number;
  problems: Problem[];
  leak: { points: Vec3[]; pointfile: string } | null;
  /** a compile: what it made; the download: {installed: true} */
  result: (Partial<CompileResult> & { installed?: boolean }) | null;
  error: string | null;
  stopped: boolean;
};

export type Progress = { job: Job | null; lines?: string[]; from?: number | null; next?: number; partial?: string };

export type Cordon = { min: Vec3; max: Vec3 };

const C = "/api/tools/compile";
const NOT_REACHED = "Couldn't reach the app. Try again.";

const get = <T,>(path: string) => getJsonAt<T>(path, null).catch(() => ({ error: NOT_REACHED }) as T);
const post = <T,>(path: string, body: unknown = {}) => postJsonAt<T>(path, body, null).catch(() => ({ error: NOT_REACHED }) as T);

export const getStatus = () => get<CompileStatus>(`${C}/status`);
export const setUpCompiler = () => post<{ ok?: boolean; error?: string; installed?: boolean }>(`${C}/setup`);
export type CordonOptions = { seal: "sky" | "black"; keepLights: boolean; spawn: boolean; spawnAt?: Vec3 };
export const startCompile = (b: { preset: Preset; name?: string; cordon?: Cordon; cordons?: Cordon[]; cordonOptions?: CordonOptions; install?: boolean; allCores?: boolean }) => post<{ ok?: boolean; error?: string; map?: string }>(`${C}/start`, b);
export const getProgress = (from?: number) => get<Progress & { error?: string }>(`${C}/progress${from != null ? `?from=${from}` : ""}`);
export const cancelCompile = () => post<{ ok?: boolean; error?: string }>(`${C}/cancel`);

// ---- what a playtest borrows from the rest of the app -------------------------------------

export type Mode = { id: string; name: string };
export const getModes = () => get<{ modes?: Mode[]; mode?: string; error?: string }>("/api/app/start-options");
export const writeRes = (map: string) => post<{ ok?: boolean; error?: string }>(`/api/tools/doctor/${encodeURIComponent(map)}/res`);
export const learnNav = (map: string) => post<{ ok?: boolean; error?: string }>("/api/navs/learn", { maps: [map], relearn: true });
export const learnResult = () => get<{ result?: { message?: string; learned?: string[] } | null }>("/api/navs/learn");
export const gameState = () => get<{ online?: boolean; game_open?: boolean; error?: string }>("/api/state");
export const launchGame = (b: { map: string; bots: number; mode?: string; restart?: boolean }) => post<{ ok?: boolean; error?: string }>("/api/app/launch", b);

// ---- the leak line, for the 3D view (LeakLine.tsx draws it) -------------------------------

/** P5: what the app works out about the leak (GET /api/tools/editor/leak). */
export type LeakInfo = {
  leaked: boolean;
  pointfile?: string;
  from?: "compile" | "file";
  points?: Vec3[];
  hole?: { at: Vec3; index: number } | null;
  nearest?: { brush: number; distance: number }[];
  brushEntitiesNear?: { brush: number; entity: number; classname: string; distance: number }[];
  start?: { entity: number; classname: string } | null;
  length?: number;
  advice?: string;
  error?: string;
};

type LeakState = {
  points: Vec3[] | null;
  visible: boolean;
  /** P5: the hole and the brushes round it */
  info: LeakInfo | null;
  /** P5: bumped to fly the 3D camera along the line */
  fly: number;
  set: (points: Vec3[] | null) => void;
  toggle: (on?: boolean) => void;
  /** P5: ask the app again (after a compile, a pointfile loaded, the map changed) */
  refresh: () => Promise<LeakInfo | null>;
};

export const useLeak = create<LeakState>((set) => ({
  points: null,
  visible: true,
  info: null,
  fly: 0,
  set: (points) => set({ points, visible: true, ...(points ? {} : { info: null }) }),
  toggle: (on) => set((s) => ({ visible: on ?? !s.visible })),
  refresh: async () => {
    const r = await get<LeakInfo>("/api/tools/editor/leak");
    if (r.error) return null;
    if (!r.leaked) set({ points: null, info: null });
    else set((s) => ({ points: r.points ?? null, info: r, visible: s.points ? s.visible : true }));
    return r;
  },
}));

/** P5: a pointfile (.lin or .pts) from another compile, picked in the file dialog. */
export const loadPointfile = () => post<LeakInfo & { cancelled?: boolean }>("/api/tools/editor/leak/load");
export const clearLeak = () => post<LeakInfo>("/api/tools/editor/leak/clear");
