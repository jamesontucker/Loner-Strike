// The Studio page's state: the open graph with its undo/redo stack, the problems the app found,
// what's selected, and the Playtest's debug state.

import { create } from "zustand";
import { emptyHistory, record, redo, undo, type History } from "./graphOps";
import type { Graph } from "./schema";
import type { Problem, TestRunState } from "./studioApi";
import { applyEvents, emptyDebug, type DebugState } from "./debugState";
import type { DebugEvent } from "./studioApi";
import { nodeDef } from "./registry";
import type { LibItem } from "./library";
import { loadLibrary } from "./libraryApi";

type Studio = {
  graph: Graph | null;
  /** the graph as last saved (to tell unsaved changes) */
  saved: Graph | null;
  history: History;
  problems: Problem[];
  selected: string[];
  /** a Playtest is going in this mode: the debug link is read */
  playing: string | null;
  /** when it started (Date.now()), for "waiting for the game" */
  playingSince: number;
  debug: DebugState;
  /** "Disable all breakpoints": the marks stay, the game ignores them */
  bpsOff: boolean;
  /** the variable shown in Details (picked in the left panel) */
  selectedVar: string | null;
  /** the Function looked at (its name joins the crumbs) */
  funcFocus: string | null;
  /** the compiler's last words (Results panel) */
  output: string | null;
  /** the full check (Compile: the real compiler) and what it found */
  full: FullCheck;
  setFull: (f: FullCheck) => void;
  /** a request to show the Results panel on a tab (n changes each time) */
  resultsAsk: { tab: ResultsTab; n: number };
  showResults: (tab: ResultsTab) => void;
  /** the test run in progress or the last one (as the app reports it), and the graph it tested */
  testRun: TestRunState | null;
  testKey: string;
  setTestRun: (r: TestRunState | null, key?: string) => void;
  /** the graph tab shown: "" the Event Graph, else a Function's or Macro's id */
  tab: string;
  /** the tabs open after the Event Graph, in order */
  tabs: string[];
  /** a Function, Macro, struct or enum shown in Details (picked in the left panel) */
  part: string | null;
  openTab: (id: string) => void;
  closeTab: (id: string) => void;
  moveTab: (id: string, before: string | null) => void;
  selectPart: (id: string | null) => void;
  /** My library: reusable Functions, Macros, structs and enums */
  library: LibItem[];
  reloadLibrary: () => Promise<void>;
  selectVar: (id: string | null) => void;
  setFuncFocus: (id: string | null) => void;
  setOutput: (o: string | null) => void;
  open: (g: Graph, saved: boolean) => void;
  close: () => void;
  /** an edit that can be undone */
  edit: (f: (g: Graph) => Graph) => void;
  /** a change shown while it happens (dragging), recorded once at the end with `commit` */
  live: (f: (g: Graph) => Graph) => void;
  commit: (before: Graph) => void;
  undo: () => void;
  redo: () => void;
  markSaved: () => void;
  setProblems: (p: Problem[]) => void;
  select: (ids: string[]) => void;
  startPlaying: (mode: string) => void;
  stopPlaying: () => void;
  events: (ev: DebugEvent[]) => void;
  setBpsOff: (v: boolean) => void;
};

/** The tabs still there after a change (a removed Function's tab closes). */
function keepTabs(g: Graph, tab: string, tabs: string[]) {
  const there = new Set([...(g.functions ?? []).map((f) => f.id), ...(g.macros ?? []).map((m) => m.id), ...(g.timelines ?? []).map((t) => t.id)]);
  const egs = new Set((g.eventGraphs ?? []).map((e) => e.id));
  const next = tabs.filter((t) => there.has(t));
  const ok = (t: string) => !t || there.has(t) || egs.has(t);
  return next.length === tabs.length && ok(tab) ? {} : { tabs: next, tab: ok(tab) ? tab : "" };
}

const isEventNode = (g: Graph | null) => (id: string) => {
  const n = g?.nodes.find((x) => x.id === id);
  return !!(n && nodeDef(n.type)?.event);
};

/** The full check (Compile): "none" not run for this graph, "running", or what the compiler said
 *  ("ok", "warn", "error"; "failed" when it couldn't run). `key` is the graph it checked
 *  (`checkKey`): an edit since then makes it stale. */
export type ResultsTab = "problems" | "output" | "testrun";
export type FullCheck = { state: "none" | "running" | "ok" | "warn" | "error" | "failed"; key: string; problems: Problem[]; message?: string };
export const NO_FULL: FullCheck = { state: "none", key: "", problems: [] };

/** What the compiler sees of a graph: everything but where things sit on the canvas. */
const keyCache = new WeakMap<Graph, string>();
export function checkKey(g: Graph): string {
  let k = keyCache.get(g);
  if (k === undefined) {
    k = JSON.stringify({ ...g, nodes: g.nodes.map(({ pos: _p, ...n }) => n), comments: [] });
    keyCache.set(g, k);
  }
  return k;
}

export const useStudio = create<Studio>((set, get) => ({
  graph: null,
  saved: null,
  history: emptyHistory(),
  problems: [],
  selected: [],
  playing: null,
  playingSince: 0,
  debug: emptyDebug(),
  bpsOff: false,
  selectedVar: null,
  funcFocus: null,
  output: null,
  full: NO_FULL,
  resultsAsk: { tab: "problems", n: 0 },
  testRun: null,
  testKey: "",
  tab: "",
  tabs: [],
  part: null,
  openTab: (id) => {
    const { tabs, graph } = get();
    // (the extra event graphs always have their tabs: they aren't opened or closed)
    const eg = (graph?.eventGraphs ?? []).some((e) => e.id === id);
    set({ tab: id, tabs: !id || eg || tabs.includes(id) ? tabs : [...tabs, id], selected: [] });
  },
  closeTab: (id) => {
    const { tabs, tab } = get();
    const i = tabs.indexOf(id);
    const next = tabs.filter((t) => t !== id);
    set({ tabs: next, tab: tab === id ? (next[Math.min(i, next.length - 1)] ?? "") : tab });
  },
  moveTab: (id, before) => {
    const rest = get().tabs.filter((t) => t !== id);
    const at = before ? rest.indexOf(before) : rest.length;
    rest.splice(at < 0 ? rest.length : at, 0, id);
    set({ tabs: rest });
  },
  selectPart: (part) => set(part ? { part, selected: [], selectedVar: null } : { part }),
  library: [],
  reloadLibrary: async () => {
    try {
      set({ library: await loadLibrary() });
    } catch {
      set({ library: [] });
    }
  },
  selectVar: (selectedVar) => set(selectedVar ? { selectedVar, selected: [], part: null } : { selectedVar }),
  setFuncFocus: (funcFocus) => set({ funcFocus }),
  setOutput: (output) => set({ output }),
  setFull: (full) => set({ full }),
  setTestRun: (testRun, key) => set(key === undefined ? { testRun } : { testRun, testKey: key }),
  showResults: (tab) => set({ resultsAsk: { tab, n: get().resultsAsk.n + 1 } }),
  open: (g, saved) => set({ selectedVar: null, funcFocus: null, output: null, full: NO_FULL, testRun: null, testKey: "", tab: "", tabs: [], part: null, graph: g, saved: saved ? g : null, history: emptyHistory(), problems: [], selected: [], playing: null, debug: emptyDebug() }),
  close: () => set({ full: NO_FULL, graph: null, saved: null, tab: "", tabs: [], part: null, history: emptyHistory(), problems: [], selected: [], playing: null, debug: emptyDebug() }),
  edit: (f) => {
    const g = get().graph;
    if (!g) return;
    const next = f(g);
    if (next === g) return;
    set({ graph: next, history: record(get().history, g), ...keepTabs(next, get().tab, get().tabs) });
  },
  live: (f) => {
    const g = get().graph;
    if (g) set({ graph: f(g) });
  },
  commit: (before) => {
    if (get().graph !== before) set({ history: record(get().history, before) });
  },
  undo: () => {
    const { graph, history } = get();
    const r = graph && undo(history, graph);
    if (r) set({ graph: r.graph, history: r.history, ...keepTabs(r.graph, get().tab, get().tabs) });
  },
  redo: () => {
    const { graph, history } = get();
    const r = graph && redo(history, graph);
    if (r) set({ graph: r.graph, history: r.history, ...keepTabs(r.graph, get().tab, get().tabs) });
  },
  markSaved: () => set({ saved: get().graph }),
  setProblems: (problems) => set({ problems }),
  // (nothing picked: Details goes back to the mode itself, not the last My mode item)
  select: (selected) => set(selected.length ? { selected, selectedVar: null, funcFocus: null } : { selected, part: null, selectedVar: null }),
  startPlaying: (mode) => set({ playing: mode, playingSince: Date.now(), debug: emptyDebug() }),
  stopPlaying: () => set({ playing: null }),
  events: (ev) => set({ debug: applyEvents(get().debug, ev, isEventNode(get().graph)) }),
  setBpsOff: (bpsOff) => set({ bpsOff }),
}));

/** The breakpoints the game should stop at now. */
export const activeBreakpoints = (g: Graph | null, off: boolean) => (off || !g ? [] : g.nodes.filter((n) => n.breakpoint).map((n) => n.id));
