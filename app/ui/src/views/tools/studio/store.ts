// The Studio page's state: the open graph with its undo/redo stack, the problems the app found,
// what's selected, and the Playtest's debug state.

import { create } from "zustand";
import { emptyHistory, record, redo, undo, type History } from "./graphOps";
import type { Graph } from "./schema";
import type { Problem } from "./studioApi";
import { applyEvents, emptyDebug, type DebugState } from "./debugState";
import type { DebugEvent } from "./studioApi";
import { nodeDef } from "./registry";

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
  const there = new Set([...(g.functions ?? []).map((f) => f.id), ...(g.macros ?? []).map((m) => m.id)]);
  const next = tabs.filter((t) => there.has(t));
  return next.length === tabs.length && (!tab || there.has(tab)) ? {} : { tabs: next, tab: tab && there.has(tab) ? tab : "" };
}

const isEventNode = (g: Graph | null) => (id: string) => {
  const n = g?.nodes.find((x) => x.id === id);
  return !!(n && nodeDef(n.type)?.event);
};

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
  tab: "",
  tabs: [],
  part: null,
  openTab: (id) => {
    const { tabs } = get();
    set({ tab: id, tabs: !id || tabs.includes(id) ? tabs : [...tabs, id], selected: [] });
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
  selectVar: (selectedVar) => set(selectedVar ? { selectedVar, selected: [], part: null } : { selectedVar }),
  setFuncFocus: (funcFocus) => set({ funcFocus }),
  setOutput: (output) => set({ output }),
  open: (g, saved) => set({ selectedVar: null, funcFocus: null, output: null, tab: "", tabs: [], part: null, graph: g, saved: saved ? g : null, history: emptyHistory(), problems: [], selected: [], playing: null, debug: emptyDebug() }),
  close: () => set({ graph: null, saved: null, tab: "", tabs: [], part: null, history: emptyHistory(), problems: [], selected: [], playing: null, debug: emptyDebug() }),
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
  select: (selected) => set(selected.length ? { selected, selectedVar: null, funcFocus: null } : { selected }),
  startPlaying: (mode) => set({ playing: mode, playingSince: Date.now(), debug: emptyDebug() }),
  stopPlaying: () => set({ playing: null }),
  events: (ev) => set({ debug: applyEvents(get().debug, ev, isEventNode(get().graph)) }),
  setBpsOff: (bpsOff) => set({ bpsOff }),
}));

/** The breakpoints the game should stop at now. */
export const activeBreakpoints = (g: Graph | null, off: boolean) => (off || !g ? [] : g.nodes.filter((n) => n.breakpoint).map((n) => n.id));
