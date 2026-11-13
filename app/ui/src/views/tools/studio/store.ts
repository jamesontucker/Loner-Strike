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
  debug: DebugState;
  /** "Disable all breakpoints": the marks stay, the game ignores them */
  bpsOff: boolean;
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
  debug: emptyDebug(),
  bpsOff: false,
  open: (g, saved) => set({ graph: g, saved: saved ? g : null, history: emptyHistory(), problems: [], selected: [], playing: null, debug: emptyDebug() }),
  close: () => set({ graph: null, saved: null, history: emptyHistory(), problems: [], selected: [], playing: null, debug: emptyDebug() }),
  edit: (f) => {
    const g = get().graph;
    if (!g) return;
    const next = f(g);
    if (next === g) return;
    set({ graph: next, history: record(get().history, g) });
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
    if (r) set({ graph: r.graph, history: r.history });
  },
  redo: () => {
    const { graph, history } = get();
    const r = graph && redo(history, graph);
    if (r) set({ graph: r.graph, history: r.history });
  },
  markSaved: () => set({ saved: get().graph }),
  setProblems: (problems) => set({ problems }),
  select: (selected) => set({ selected }),
  startPlaying: (mode) => set({ playing: mode, debug: emptyDebug() }),
  stopPlaying: () => set({ playing: null }),
  events: (ev) => set({ debug: applyEvents(get().debug, ev, isEventNode(get().graph)) }),
  setBpsOff: (bpsOff) => set({ bpsOff }),
}));

/** The breakpoints the game should stop at now. */
export const activeBreakpoints = (g: Graph | null, off: boolean) => (off || !g ? [] : g.nodes.filter((n) => n.breakpoint).map((n) => n.id));
