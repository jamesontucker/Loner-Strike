// Deleting something the graph uses (a variable, a Function's local, a Function, a Custom Event):
// what uses it, what could take its place, and the three ways to delete it (Replace with…,
// Remove it and its nodes with the flow joined up round them, Remove and keep the nodes). Each
// returns the whole new graph, so the page records it as one undo step.

import { remove, removeCustomEvent, removeGraphPart, removeVariable } from "./graphOps";
import { allVariables } from "./pins";
import { nodeDef } from "./registry";
import type { GEdge, GNode, Graph } from "./schema";

export type DeleteKind = "var" | "fn" | "cevent";
export type DeleteRef = { kind: DeleteKind; id: string };

const FN_NODES = ["fn.call", "fn.call_pure", "timer.by_function"];
const CE_NODES = ["event.call", "timer.by_event", "event.custom"];

/** The param a node names it by. */
const paramOf = (k: DeleteKind) => (k === "var" ? "var" : k === "fn" ? "func" : "event");

/** The nodes that use it (a Function's own graph doesn't count). */
export function usesOfRef(g: Graph, r: DeleteRef): GNode[] {
  const types = r.kind === "var" ? null : r.kind === "fn" ? FN_NODES : CE_NODES;
  const p = paramOf(r.kind);
  return g.nodes.filter((n) => n.params[p] === r.id && (types ? types.includes(n.type) : true) && !(r.kind === "fn" && n.graph === r.id));
}

export function refName(g: Graph, r: DeleteRef): string {
  if (r.kind === "var") return allVariables(g).find((v) => v.id === r.id)?.name ?? "";
  if (r.kind === "fn") return g.functions?.find((f) => f.id === r.id)?.name ?? "";
  return g.customEvents?.find((c) => c.id === r.id)?.name ?? "";
}

/** The Function a local belongs to (null: a mode variable). */
const ownerOf = (g: Graph, id: string) => (g.functions ?? []).find((f) => f.locals?.some((l) => l.id === id))?.id ?? null;

const sig = (l: { type: string }[]) => l.map((m) => m.type).join(",");

/** What could take its place: a variable of the same type and scope (a local: one of the same
 *  Function's, or a mode variable); a Function with the same pins (and as pure); a Custom Event
 *  with the same inputs. */
export function replacements(g: Graph, r: DeleteRef): { id: string; name: string }[] {
  if (r.kind === "var") {
    const v = allVariables(g).find((x) => x.id === r.id);
    if (!v) return [];
    const owner = ownerOf(g, r.id);
    return allVariables(g)
      .filter((x) => x.id !== r.id && x.type === v.type && x.scope === v.scope)
      .filter((x) => {
        const o = ownerOf(g, x.id);
        return o === null || o === owner;
      })
      .map((x) => ({ id: x.id, name: x.name }));
  }
  if (r.kind === "fn") {
    const f = g.functions?.find((x) => x.id === r.id);
    if (!f) return [];
    return (g.functions ?? []).filter((x) => x.id !== r.id && !!x.pure === !!f.pure && sig(x.inputs) === sig(f.inputs) && sig(x.outputs) === sig(f.outputs)).map((x) => ({ id: x.id, name: x.name }));
  }
  const c = g.customEvents?.find((x) => x.id === r.id);
  if (!c) return [];
  return (g.customEvents ?? []).filter((x) => x.id !== r.id && sig(x.inputs) === sig(c.inputs)).map((x) => ({ id: x.id, name: x.name }));
}

/** Pin ids that follow the thing (a Function's or Custom Event's members), old -> new, by place. */
function pinMap(g: Graph, r: DeleteRef, to: string): Map<string, string> {
  const m = new Map<string, string>();
  if (r.kind === "var") return m;
  const list = r.kind === "fn" ? g.functions ?? [] : g.customEvents ?? [];
  const a = list.find((x) => x.id === r.id) as { inputs: { id: string }[]; outputs?: { id: string }[] } | undefined;
  const b = list.find((x) => x.id === to) as { inputs: { id: string }[]; outputs?: { id: string }[] } | undefined;
  if (!a || !b) return m;
  a.inputs.forEach((x, i) => b.inputs[i] && m.set(x.id, b.inputs[i].id));
  (a.outputs ?? []).forEach((x, i) => b.outputs?.[i] && m.set(x.id, b.outputs[i].id));
  return m;
}

/** Removes the thing itself (nodes that used it are left as they are). */
function removeThing(g: Graph, r: DeleteRef): Graph {
  return r.kind === "var" ? removeVariable(g, r.id) : r.kind === "fn" ? removeGraphPart(g, r.id) : removeCustomEvent(g, r.id);
}

/** Every Get, Set, Add (or call, or Custom Event node) switches to `to`; wires on its pins follow. */
export function replaceRefs(g: Graph, r: DeleteRef, to: string): Graph {
  const uses = new Set(usesOfRef(g, r).map((n) => n.id));
  const pins = pinMap(g, r, to);
  const p = paramOf(r.kind);
  const mv = (end: GEdge["from"]) => (uses.has(end.node) && pins.has(end.pin) ? { ...end, pin: pins.get(end.pin)! } : end);
  const g2: Graph = {
    ...g,
    nodes: g.nodes.map((n) => (uses.has(n.id) ? { ...n, params: { ...n.params, [p]: to } } : n)),
    edges: g.edges.map((e) => ({ ...e, from: mv(e.from), to: mv(e.to) })),
  };
  return removeThing(g2, r);
}

/** Removes nodes, joining the flow round each: what ran into a removed node now runs into what
 *  came after it (its Then, or its only flow output). Data wires from them just go. */
export function removeBridged(g: Graph, ids: Iterable<string>): Graph {
  let cur = g;
  for (const id of ids) {
    const n = cur.nodes.find((x) => x.id === id);
    if (!n) continue;
    const d = nodeDef(n.type);
    const execIns = new Set((d?.inputs ?? []).filter((p) => p.type === "exec").map((p) => p.id));
    const execOuts = (d?.outputs ?? []).filter((p) => p.type === "exec").map((p) => p.id);
    // (its Then; or, with one flow output only, that one)
    const via = execOuts.includes("then") ? "then" : execOuts.length === 1 ? execOuts[0] : null;
    const next = via ? cur.edges.find((e) => e.from.node === id && e.from.pin === via) : undefined;
    const ins = cur.edges.filter((e) => e.to.node === id && (execIns.has(e.to.pin) || e.to.pin === "exec"));
    const added: GEdge[] = [];
    if (next) {
      const taken = new Set(cur.edges.map((e) => e.id));
      for (const e of ins) {
        let k = 1;
        while (taken.has(`w${k}`)) k++;
        taken.add(`w${k}`);
        added.push({ id: `w${k}`, from: e.from, to: next.to });
      }
    }
    cur = remove(cur, [id]);
    cur = { ...cur, edges: [...cur.edges, ...added] };
  }
  return cur;
}

/** Removes it and every node that uses it, the flow joined up round them. */
export function removeWithNodes(g: Graph, r: DeleteRef): Graph {
  return removeThing(removeBridged(g, usesOfRef(g, r).map((n) => n.id)), r);
}

/** Removes it; the nodes stay and show a problem until changed (as before). */
export function removeKeepNodes(g: Graph, r: DeleteRef): Graph {
  return removeThing(g, r);
}
