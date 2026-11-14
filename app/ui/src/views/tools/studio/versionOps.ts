// Version history's pure part: how the graph now differs from an older save, and the marks the
// canvas shows for it (VersionHistory.tsx; tested in scripts/studio-help.test.ts).

import { create } from "zustand";
import { nodeDef } from "./registry";
import type { Graph } from "./schema";

export type Marks = Record<string, "added" | "changed">;

/** The canvas marks while a version is looked at (read by each node). */
export const useMarks = create<{ marks: Marks; set: (m: Marks) => void }>((set) => ({ marks: {}, set: (marks) => set({ marks }) }));

/** How the graph now differs from an older one: nodes added since, changed since, and gone. */
export function compareGraphs(now: Graph, old: Graph): { marks: Marks; gone: { id: string; title: string }[] } {
  const marks: Marks = {};
  const before = new Map(old.nodes.map((n) => [n.id, n]));
  for (const n of now.nodes) {
    const o = before.get(n.id);
    if (!o) marks[n.id] = "added";
    else if (JSON.stringify({ ...o, pos: 0 }) !== JSON.stringify({ ...n, pos: 0 })) marks[n.id] = "changed";
  }
  const ids = new Set(now.nodes.map((n) => n.id));
  const gone = old.nodes.filter((n) => !ids.has(n.id)).map((n) => ({ id: n.id, title: nodeDef(n.type)?.title ?? n.type }));
  return { marks, gone };
}

