// The Studio's server side (tools/studio/*.rs) and what a Playtest borrows from the app.

import { get, post, tryGet } from "../common/api";
import { sizeError, type Graph } from "./schema";

const S = "/api/tools/studio";

export type Problem = { level: "error" | "warning"; node?: string; pin?: string; edge?: string; message: string };
export type GraphInfo = { id: string; name: string; description?: string; nodes?: number; modified: number; installed: boolean; damaged?: boolean };
export type CompileResult = { ok?: boolean; problems?: Problem[]; output?: string; plugin?: string; installedAs?: string; error?: string; cached?: boolean; secs?: number };

export const studioStatus = () => tryGet<{ game?: boolean; compiler?: boolean; graphsDir?: string }>(`${S}/status`);
export const listGraphs = () => get<{ graphs: GraphInfo[] }>(`${S}/graphs`);
export const openGraph = (id: string) => get<{ graph: unknown }>(`${S}/graph/${encodeURIComponent(id)}`);
// (too big: refused here with why, as the server would; a saved file always opens again)
export const saveGraph = (g: Graph) => {
  const big = sizeError(g);
  return big ? Promise.resolve({ error: big } as { ok?: boolean; error?: string }) : post<{ ok?: boolean }>(`${S}/graph/${encodeURIComponent(g.mode.id)}`, { graph: g });
};
export const deleteGraph = (id: string) => post<{ ok?: boolean }>(`${S}/graph/${encodeURIComponent(id)}/delete`);
export const validate = (g: Graph) => post<{ problems?: Problem[] }>(`${S}/validate`, { graph: g });
export const generate = (g: Graph, debug = false) => post<{ problems?: Problem[]; source?: string; plugin?: string }>(`${S}/generate`, { graph: g, debug });
export const openAsCode = (g: Graph) => post<{ ok?: boolean; folder?: string; name?: string; problems?: Problem[] }>(`${S}/source`, { graph: g });
export const compile = (g: Graph, debug = false) => post<CompileResult>(`${S}/compile`, { graph: g, debug });
/** Compile (the full check): the real compiler, nothing installed or started. */
export const checkCompile = (g: Graph) => post<{ ok?: boolean; problems?: Problem[]; compiler?: Problem[]; output?: string; stopped?: boolean; cached?: boolean; secs?: number; error?: string }>(`${S}/check`, { graph: g });
/** Test run (the real-world check): a hidden server plays the mode with bots. */
export type TestStep = { label: string; at: number; secs?: number | null };
export type TestResult = {
  verdict: "pass" | "warn" | "fail";
  summary: string[];
  problems: Problem[];
  ran?: string[];
  neverRan?: string[];
  cost?: { baseline?: { cpu: number; fps: number } | null; mode?: { cpu: number; fps: number } | null; nodeRunsPerSec?: number; busiest?: { node: string; runs: number }[] };
  maps?: string[];
  log: string;
};
export type TestRunState = { mode: string; phase: "running" | "done" | "failed" | "stopped"; steps: TestStep[]; started: number; result?: TestResult | null; error?: string | null };
export const startTestRun = (g: Graph) => post<{ ok?: boolean; error?: string }>(`${S}/testrun`, { graph: g });
export const testRunProgress = () => tryGet<{ run?: TestRunState | null; now?: number }>(`${S}/testrun`);
export const cancelTestRun = () => post<{ ok?: boolean }>(`${S}/testrun/cancel`);
export const playtest = (g: Graph, breakpoints: string[]) => post<CompileResult>(`${S}/playtest`, { graph: g, breakpoints });
export const switchOpenGame = (mode: string, map: string) => post<{ ok?: boolean }>(`${S}/switch`, { mode, map });
export const uninstall = (id: string) => post<{ ok?: boolean }>(`${S}/uninstall/${encodeURIComponent(id)}`);

export type DebugEvent = { s: number; t: number; e: string; node?: string; inputs?: Record<string, unknown>; result?: string; message?: string; [k: string]: unknown };
export const debugEvents = (mode: string, from: number) => tryGet<{ events?: DebugEvent[]; next?: number; reset?: boolean }>(`${S}/debug/events?mode=${encodeURIComponent(mode)}&from=${from}`);
export const debugCommand = (b: { mode: string; resume?: boolean; vars?: boolean; breakpoints?: string[]; set?: { var: string; who: number; value: string | number | boolean; graph: unknown } }) =>
  post<{ ok?: boolean; error?: string }>(`${S}/debug/command`, b);

export const gameState = () => tryGet<{ game_open?: boolean }>("/api/state");
export const launchGame = (b: { map: string; bots: number; mode: string; restart?: boolean }) => post<{ ok?: boolean }>("/api/app/launch", b);
