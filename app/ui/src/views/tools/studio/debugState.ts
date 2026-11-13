// What the Playtest's debug events (tools/studio/debug.rs) mean for the editor: which node ran
// last and with what, the path the flow took, where the world is paused and the live values
// read while it is. Pure, so it's tested without a game (studio.test.ts).

import type { DebugEvent } from "./studioApi";

export type NodeRun = {
  count: number;
  /** "true" / "false" for a condition, "given", "shown"... */
  result?: string;
  error?: string;
  inputs?: Record<string, unknown>;
  /** when it last ran (Date.now()) */
  at: number;
};

export type PlayerVar = { id: number; name: string; team: string; hp: number; alive: boolean; weapon: string; bot: boolean };
export type Vars = { round: number; phase: string; timeLeft: number; aliveT: number; aliveCT: number; players: PlayerVar[]; at: number };

export type DebugState = {
  runs: Record<string, NodeRun>;
  /** the nodes the latest flow went through, in order (starts again at each event) */
  path: string[];
  paused: { node: string; inputs?: Record<string, unknown> } | null;
  vars: Vars | null;
  log: DebugEvent[];
  map?: string;
  /** events seen since the Playtest started */
  seen: number;
};

export const emptyDebug = (): DebugState => ({ runs: {}, path: [], paused: null, vars: null, log: [], seen: 0 });

const LOG = 80;

/** Applies a batch of events. `isEvent` tells event nodes (where a new flow starts). */
export function applyEvents(s: DebugState, events: DebugEvent[], isEvent: (node: string) => boolean, now = Date.now()): DebugState {
  if (!events.length) return s;
  let runs = s.runs;
  let path = s.path;
  let paused = s.paused;
  let vars = s.vars;
  let map = s.map;
  let pendingPlayers: PlayerVar[] | null = null;
  const touch = (node: string, f: (r: NodeRun) => NodeRun) => {
    runs = { ...runs, [node]: f(runs[node] ?? { count: 0, at: now }) };
  };
  for (const ev of events) {
    const node = typeof ev.node === "string" ? ev.node : undefined;
    switch (ev.e) {
      case "session":
        // a new map: what ran before is history
        runs = {};
        path = [];
        paused = null;
        vars = null;
        map = typeof ev.map === "string" ? ev.map : undefined;
        break;
      case "node_enter":
        if (!node) break;
        touch(node, (r) => ({ ...r, count: r.count + 1, at: now, inputs: ev.inputs, error: undefined, result: undefined }));
        path = isEvent(node) || !path.length ? [node] : [...path, node];
        if (paused?.node === node) paused = null;
        break;
      case "node_exit":
        if (node) touch(node, (r) => ({ ...r, result: typeof ev.result === "string" ? ev.result : undefined, at: now }));
        break;
      case "node_error":
        if (node) touch(node, (r) => ({ ...r, error: typeof ev.message === "string" ? ev.message : "Something went wrong here.", at: now }));
        break;
      case "breakpoint_hit":
        if (!node) break;
        paused = { node, inputs: ev.inputs };
        touch(node, (r) => ({ ...r, inputs: ev.inputs, at: now }));
        path = isEvent(node) || !path.length ? [node] : [...path, node];
        break;
      case "world_resumed":
        paused = null;
        break;
      case "vars":
        pendingPlayers = [];
        vars = {
          round: Number(ev.round) || 0,
          phase: String(ev.phase ?? ""),
          timeLeft: Number(ev.timeLeft) || 0,
          aliveT: Number(ev.aliveT) || 0,
          aliveCT: Number(ev.aliveCT) || 0,
          players: pendingPlayers,
          at: now,
        };
        break;
      case "var_player":
        if (pendingPlayers && ev.p && typeof ev.p === "object") pendingPlayers.push(ev.p as PlayerVar);
        break;
    }
  }
  const log = [...s.log, ...events.filter((e) => e.e !== "var_player" && e.e !== "commands")].slice(-LOG);
  return { runs, path, paused, vars, log, map, seen: s.seen + events.length };
}

/** The wires to light up: between consecutive nodes on the latest path. */
export function pathPairs(path: string[]): Set<string> {
  const out = new Set<string>();
  for (let i = 1; i < path.length; i++) out.add(`${path[i - 1]}>${path[i]}`);
  return out;
}

/** One line for the event log. */
export function describe(ev: DebugEvent, title: (node: string) => string): string {
  const n = ev.node ? title(ev.node) : "";
  switch (ev.e) {
    case "session": return `Map ${String(ev.map ?? "")} loaded: the debugger is connected.`;
    case "node_enter": return `${n} ran`;
    case "node_exit": return `${n}: ${String(ev.result ?? "done")}`;
    case "node_error": return `${n}: ${String(ev.message ?? "error")}`;
    case "breakpoint_hit": return `Paused at ${n}`;
    case "world_paused": return "The world is paused";
    case "world_resumed": return `Resumed (paused for ${Number(ev.paused ?? 0).toFixed(1)} s)`;
    case "vars": return `Round ${String(ev.round)}, ${String(ev.phase)}`;
    default: return ev.e;
  }
}
