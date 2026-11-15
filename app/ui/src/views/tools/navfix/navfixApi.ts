// The nav fixer's requests (app/src-tauri/src/tools/navfix.rs). The editing session lives in
// the app: every edit, undo and save answers with the whole state, so the page just shows it.

import { post as postJson, ToolsError } from "../common/api";
import { toNav, type Nav } from "../overlays";
import type { Vec3 } from "../viewer/coords";

/** One edit, as the app takes it. */
export type Op =
  | { op: "add"; x: [number, number]; y: [number, number]; z: number; attr?: number; place?: string; snap?: boolean }
  | { op: "delete"; ids: number[] }
  | { op: "reshape"; id: number; x: [number, number]; y: [number, number]; z?: number; snap?: boolean }
  | { op: "link"; a: number; b: number; oneWay?: boolean }
  | { op: "unlink"; a: number; b: number; both?: boolean }
  | { op: "attr"; ids: number[]; set?: number; clear?: number }
  | { op: "place"; ids: number[]; name: string }
  /** several edits as one step: one Undo takes them all back */
  | { op: "batch"; ops: Op[]; label?: string };

export type Problem = { kind: string; text: string; hint: string; areas: number[]; at: Vec3; fix: { label: string; op: Op } | null };

/** Per area (nav file order): 0 bots get there and back, 1 never, 2 only one way (a trap), 3 only by falling in. */
export const REACH_OK = 0;
export const REACH_NEVER = 1;
export const REACH_TRAP = 2;
/** 3: bots only get there by falling or stumbling in (no link leads there), and can walk back out. */
export const REACH_FALLS = 3;

export type FixState = {
  map: string;
  file: string | null;
  nav: Nav;
  reach: number[];
  spawnAreas: number[];
  problems: Problem[];
  moreProblems: number;
  checkNote: string | null;
  dirty: boolean;
  undo: string | null;
  redo: string | null;
  steps: number;
  backup: boolean;
  knowledge: { stage: number; done: boolean; fits: boolean } | null;
  rebuild: { seconds: number; about: string; can: boolean; why: string | null; job: string };
  /** after an edit, undo or save */
  did?: string;
  notes?: string[];
  newId?: number | null;
  saved?: boolean;
  message?: string;
  knowledgeStale?: boolean;
};

/** `answered`: the app said what's wrong in its own words (common/api.ts deadEnd) */
class FixError extends ToolsError {
  constructor(message: string, answered = false) {
    super(message, answered ? 200 : 0, answered);
  }
}

function toState(raw: Record<string, unknown>): FixState {
  if (typeof raw.error === "string") throw new FixError(raw.error, true);
  const nav = toNav((raw.nav ?? {}) as Record<string, unknown>);
  return { ...(raw as unknown as FixState), nav, reach: Array.isArray(raw.reach) ? (raw.reach as number[]) : [], problems: Array.isArray(raw.problems) ? (raw.problems as Problem[]) : [] };
}

const base = (map: string) => `/api/tools/navfix/${encodeURIComponent(map)}`;

export async function openFix(map: string, signal?: AbortSignal): Promise<FixState> {
  let r: Response;
  try {
    r = await fetch(base(map), { credentials: "same-origin", headers: { "X-Panel": "1" }, signal });
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") throw e;
    throw new FixError("Couldn't reach the app. Try again.");
  }
  let raw: Record<string, unknown>;
  try {
    raw = await r.json();
  } catch {
    throw new FixError(r.status === 404 ? "The nav fixer isn't in this version of the app." : "Something went wrong opening the nav. Try again.");
  }
  return toState(raw);
}

async function post(map: string, what: string, body: unknown = {}): Promise<FixState> {
  return toState(await postJson<Record<string, unknown>>(`${base(map)}/${what}`, body));
}

export const editNav = (map: string, op: Op) => post(map, "edit", op);
export const undoNav = (map: string) => post(map, "undo");
export const redoNav = (map: string) => post(map, "redo");
export const saveNav = (map: string) => post(map, "save");
export const reloadNav = (map: string) => post(map, "reload");

/** Starts rebuilding the bots' map knowledge for the saved nav (a setup job). */
export async function rebuildKnowledge(map: string): Promise<string> {
  const r = await postJson<{ ok?: boolean; job?: string }>(`${base(map)}/rebuild`);
  if (r.error || !r.job) throw new FixError(r.error ?? "The rebuild didn't start.");
  return r.job;
}

/** Words for an area's attribute bits. */
export const ATTRS: { bit: keyof Nav["attrBits"]; label: string; help: string }[] = [
  { bit: "crouch", label: "Crouch", help: "Bots crouch here (vents, low gaps)." },
  { bit: "jump", label: "Jump", help: "Bots jump here (onto a box, over a gap)." },
  { bit: "precise", label: "Precise", help: "Bots walk carefully and don't cut corners (narrow beams, ledges)." },
  { bit: "noJump", label: "No jump", help: "Bots never jump here." },
];

export const DIRS = ["north", "east", "south", "west"];
