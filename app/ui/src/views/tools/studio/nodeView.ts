// What each node on the canvas needs, worked out once per graph change and shared, so a node
// re-renders only when something about it changes (docs/notes/PerfAudit.md #4 and #8):
// - the "structure" graph: the same object while only positions change (a drag), so pins,
//   titles and settings lines aren't worked out again for every node on every drag step;
// - per node: which pins are wired, its problems, and a key of the values the debugger last saw
//   on its outputs (so a debugger batch re-renders only the nodes it touched).

import { pinsOf } from "./pins";
import type { Problem } from "./studioApi";
import type { GNode, Graph } from "./schema";
import type { DebugState } from "./debugState";

let lastGraph: Graph | null = null;
let lastStructure: Graph | null = null;

/** Same nodes (ids, kinds, settings, graphs, breakpoints), wires and parts: only positions moved. */
function sameStructure(a: Graph, b: Graph): boolean {
  if (a.edges !== b.edges || a.functions !== b.functions || a.macros !== b.macros || a.structs !== b.structs || a.enums !== b.enums || a.variables !== b.variables || a.markers !== b.markers || a.mode !== b.mode || a.comments.length !== b.comments.length) return false;
  if (a.nodes.length !== b.nodes.length) return false;
  for (let i = 0; i < a.nodes.length; i++) {
    const x = a.nodes[i], y = b.nodes[i];
    if (x === y) continue;
    if (x.id !== y.id || x.type !== y.type || x.params !== y.params || x.graph !== y.graph || x.breakpoint !== y.breakpoint) return false;
  }
  return true;
}

/** The graph for everything but positions: kept while a drag only moves things. */
export function structureOf(g: Graph | null): Graph | null {
  if (!g) return (lastGraph = lastStructure = null);
  if (g === lastGraph) return lastStructure;
  if (!lastGraph || !lastStructure || !sameStructure(lastGraph, g)) lastStructure = g;
  lastGraph = g;
  return lastStructure;
}

type Wiring = { wired: Map<string, string>; outs: Map<string, { pin: string; to: string; toPin: string }[]> };
const wiringCache = new WeakMap<Graph, Wiring>();
/** Per node: its wired pins as one key ("i:a|o:then"), and where each output goes. */
export function wiringOf(s: Graph): Wiring {
  let w = wiringCache.get(s);
  if (w) return w;
  const sets = new Map<string, string[]>();
  const outs = new Map<string, { pin: string; to: string; toPin: string }[]>();
  const add = (id: string, k: string) => (sets.get(id) ?? sets.set(id, []).get(id)!).push(k);
  for (const e of s.edges) {
    add(e.to.node, `i:${e.to.pin}`);
    add(e.from.node, `o:${e.from.pin}`);
    (outs.get(e.from.node) ?? outs.set(e.from.node, []).get(e.from.node)!).push({ pin: e.from.pin, to: e.to.node, toPin: e.to.pin });
  }
  const wired = new Map<string, string>();
  for (const [id, l] of sets) wired.set(id, l.sort().join("|"));
  w = { wired, outs };
  wiringCache.set(s, w);
  return w;
}

const pinCache = new WeakMap<Graph, Map<string, { n: GNode; pins: ReturnType<typeof pinsOf> }>>();
/** A node's pins (per structure, and per the node's own settings). */
export function pinsFor(s: Graph, n: GNode): ReturnType<typeof pinsOf> {
  let m = pinCache.get(s);
  if (!m) pinCache.set(s, (m = new Map()));
  const c = m.get(n.id);
  if (c && c.n.params === n.params && c.n.type === n.type) return c.pins;
  const pins = pinsOf(s, n);
  m.set(n.id, { n, pins });
  return pins;
}

const EMPTY: Problem[] = [];
const problemCache = new WeakMap<Problem[], Map<string, Problem[]>>();
/** A node's own problems (the same array until the problems change). */
export function problemsFor(all: Problem[], id: string): Problem[] {
  let m = problemCache.get(all);
  if (!m) {
    m = new Map();
    for (const p of all) if (p.node) (m.get(p.node) ?? m.set(p.node, []).get(p.node)!).push(p);
    problemCache.set(all, m);
  }
  return m.get(id) ?? EMPTY;
}

/** What the game last saw on each of a node's outputs (through the nodes they feed), as one key
 *  "pin\u0001label: value\u0002..." so a node re-renders only when its own watch values change. */
const byId = new WeakMap<Graph, Map<string, GNode>>();
const watchCache = new WeakMap<DebugState["runs"], { s: Graph; keys: Map<string, string> }>();
export function watchKey(s: Graph | null, runs: DebugState["runs"], id: string): string {
  if (!s) return "";
  // (worked out once per debugger batch and structure, then just looked up: every store change
  // runs every node's selector)
  let c = watchCache.get(runs);
  if (!c || c.s !== s) watchCache.set(runs, (c = { s, keys: new Map() }));
  const hit = c.keys.get(id);
  if (hit !== undefined) return hit;
  const k = watchKeyOf(s, runs, id);
  c.keys.set(id, k);
  return k;
}

function watchKeyOf(s: Graph, runs: DebugState["runs"], id: string): string {
  const outs = wiringOf(s).outs.get(id);
  if (!outs) return "";
  let nodes = byId.get(s);
  if (!nodes) byId.set(s, (nodes = new Map(s.nodes.map((n) => [n.id, n]))));
  let key = "";
  const seen = new Set<string>();
  for (const o of outs) {
    if (seen.has(o.pin)) continue;
    const r = runs[o.to];
    if (!r?.inputs) continue;
    const to = nodes.get(o.to);
    const label = to && pinsFor(s, to).inputs.find((q) => q.id === o.toPin)?.label;
    const v = label !== undefined ? r.inputs[label] : undefined;
    if (v === undefined) continue;
    seen.add(o.pin);
    key += `${o.pin}\u0001${JSON.stringify(v)}\u0002`;
  }
  return key;
}
