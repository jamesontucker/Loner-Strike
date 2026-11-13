// The Studio's server side (tools/studio/*.rs) and what a Playtest borrows from the app.

import { get, post, tryGet } from "../common/api";
import type { Graph } from "./schema";

const S = "/api/tools/studio";

export type Problem = { level: "error" | "warning"; node?: string; pin?: string; edge?: string; message: string };
export type GraphInfo = { id: string; name: string; description?: string; nodes?: number; modified: number; installed: boolean; damaged?: boolean };
export type CompileResult = { ok?: boolean; problems?: Problem[]; output?: string; plugin?: string; error?: string };

export const studioStatus = () => tryGet<{ game?: boolean; compiler?: boolean; graphsDir?: string }>(`${S}/status`);
export const listGraphs = () => get<{ graphs: GraphInfo[] }>(`${S}/graphs`);
export const openGraph = (id: string) => get<{ graph: unknown }>(`${S}/graph/${encodeURIComponent(id)}`);
export const saveGraph = (g: Graph) => post<{ ok?: boolean }>(`${S}/graph/${encodeURIComponent(g.mode.id)}`, { graph: g });
export const deleteGraph = (id: string) => post<{ ok?: boolean }>(`${S}/graph/${encodeURIComponent(id)}/delete`);
export const validate = (g: Graph) => post<{ problems?: Problem[] }>(`${S}/validate`, { graph: g });
export const generate = (g: Graph, debug = false) => post<{ problems?: Problem[]; source?: string; plugin?: string }>(`${S}/generate`, { graph: g, debug });
export const compile = (g: Graph, debug = false) => post<CompileResult>(`${S}/compile`, { graph: g, debug });
export const playtest = (g: Graph, breakpoints: string[]) => post<CompileResult>(`${S}/playtest`, { graph: g, breakpoints });
export const uninstall = (id: string) => post<{ ok?: boolean }>(`${S}/uninstall/${encodeURIComponent(id)}`);

export type DebugEvent = { s: number; t: number; e: string; node?: string; inputs?: Record<string, unknown>; result?: string; message?: string; [k: string]: unknown };
export const debugEvents = (mode: string, from: number) => tryGet<{ events?: DebugEvent[]; next?: number; reset?: boolean }>(`${S}/debug/events?mode=${encodeURIComponent(mode)}&from=${from}`);
export const debugCommand = (b: { mode: string; resume?: boolean; vars?: boolean; breakpoints?: string[] }) => post<{ ok?: boolean }>(`${S}/debug/command`, b);

export const gameState = () => tryGet<{ game_open?: boolean }>("/api/state");
export const launchGame = (b: { map: string; bots: number; mode: string; restart?: boolean }) => post<{ ok?: boolean }>("/api/app/launch", b);
