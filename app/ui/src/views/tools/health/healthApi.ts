// The map health check's server side (tools/health.rs): bots play a map on a hidden copy of
// one of your servers for a few minutes, then a report says what went wrong.

import { getJson } from "../../../api";
import { post, type Err } from "../common/api";
import type { Vec3 } from "../viewer/coords";


export type Options = Err & {
  map: string;
  game: string;
  hasNav: boolean;
  hasKnowledge: boolean;
  servers: { id: string; name: string }[];
  server: string | null;
  modes: { id: string; name: string }[];
  mode: string;
  bots: number;
  minutes: number;
  limits: { minutes: [number, number]; bots: [number, number] };
  /** why this map can't be checked (no nav, no server to copy) */
  why: string | null;
  /** why nothing can be checked right now (the game or a server is running, setup is busy) */
  busy: string | null;
  running: boolean;
};

export type Phase = "preparing" | "starting" | "playing" | "reading" | "done" | "failed" | "stopped";
export type Run = {
  id: string;
  map: string;
  mode: string;
  modeName: string;
  minutes: number;
  bots: number;
  server: string;
  phase: Phase;
  detail: string;
  started: number;
  playingSince: number | null;
  error: string | null;
  report: string | null;
};
export type Progress = { run: Run | null; active?: boolean; elapsed?: number; left?: number; fraction?: number };

export type Spot = { at: Vec3; place: string; count: number; seconds: number; drop: number };
export type Finding = { level: "bad" | "warn" | "good"; text: string; spot?: Spot };
export type Report = {
  id: string;
  map: string;
  mode: string;
  modeName: string;
  game: string;
  server: string;
  when: number;
  minutes: number;
  botsWanted: number;
  seconds: number;
  botsSeen: number;
  botMinutes: number;
  kills: number;
  killsPerBotMin: number;
  stuckPct: number;
  stillPct: number;
  coveragePct: number | null;
  navAreas: number | null;
  spawnSitters: number;
  botRounds: number;
  deaths: number;
  worldDeaths: number;
  falls: number;
  fallSpots: Spot[];
  hurtSpots: Spot[];
  stuckSpots: Spot[];
  knowledge: "loaded" | "rebuilt" | "none";
  fatal: string[];
  pluginErrors: number;
  pluginErrorList: { plugin: string; count: number; first: string }[];
  endedEarly: string | null;
  compared: string[];
  findings: Finding[];
  verdict: "good" | "warn" | "bad";
  recording: string | null;
};
export type ReportSummary = { id: string; map: string; mode: string; modeName: string; when: number; minutes: number; bots: number; verdict: Report["verdict"]; headline: string; problems: number };


export const healthOptions = (map: string) => getJson<Options>(`/api/tools/health/options?map=${encodeURIComponent(map)}`);
export const healthProgress = () => getJson<Progress>("/api/tools/health/progress");
export const healthReports = () => getJson<Err & { reports?: ReportSummary[] }>("/api/tools/health/reports");
export const healthReport = (id: string) => getJson<Err & { report?: Report }>(`/api/tools/health/report/${encodeURIComponent(id)}`);
export const startHealth = (req: { map: string; mode: string; minutes: number; bots: number; server: string | null }) => post<{ ok?: boolean; id?: string }>("/api/tools/health/start", req);
export const stopHealth = () => post<{ ok?: boolean }>("/api/tools/health/cancel");

/** The Replay tool's id for a recording file (from agent C's list), or null. */
export async function recordingId(file: string): Promise<string | null> {
  const r = await getJson<{ recordings?: { id: string; file: string }[] }>("/api/tools/recordings").catch(() => null);
  return r?.recordings?.find((x) => x.file === file || x.file.endsWith(`/${file}`) || x.file.endsWith(`\\${file}`))?.id ?? null;
}
