// "+ Add pin" and "−" on nodes with a variable number of pins (Sequence, Multi Gate, Make Array,
// Switch on Number, Switch on Text), as in Blueprint: the Details setting (Outputs, Items, Cases)
// changes with them. Taking the last pin off is refused while a wire is on it.
import { nodeDef } from "./registry";
import { pinsOf, switchCases } from "./pins";
import type { Graph } from "./schema";

export type Counted = { kind: "count"; param: string; min: number; max: number } | { kind: "cases"; numbers: boolean };

/** How a node's pins are counted, or null when they're fixed. */
export function countedOf(type: string): Counted | null {
  if (type === "flow.switch_num" || type === "flow.switch_text") return { kind: "cases", numbers: type === "flow.switch_num" };
  if (!["flow.sequence", "flow.multi_gate", "array.make"].includes(type)) return null;
  const p = nodeDef(type)?.params.find((x) => x.id === "count");
  return p ? { kind: "count", param: "count", min: p.min ?? 0, max: p.max ?? 8 } : null;
}

/** The count as the pins show it now (cases: how many), its least and most. */
export function countOf(g: Graph, nodeId: string): { n: number; min: number; max: number } | null {
  const node = g.nodes.find((x) => x.id === nodeId);
  const c = node && countedOf(node.type);
  const def = node && nodeDef(node.type);
  if (!node || !c || !def) return null;
  if (c.kind === "cases") return { n: switchCases(node, def).length, min: 1, max: 16 };
  const v = Number(node.params[c.param] ?? def.params.find((x) => x.id === c.param)?.default ?? c.min);
  return { n: Math.round(Number.isFinite(v) ? v : c.min), min: c.min, max: c.max };
}

const withParams = (g: Graph, nodeId: string, params: Record<string, string | number | boolean>): Graph => ({
  ...g,
  nodes: g.nodes.map((x) => (x.id === nodeId ? { ...x, params: { ...x.params, ...params } } : x)),
});

/** One more pin: the count up one, or a new case after the last (the next number, or "case N"). */
export function addPin(g: Graph, nodeId: string): { graph: Graph } | { error: string } {
  const node = g.nodes.find((x) => x.id === nodeId);
  const c = node && countedOf(node.type);
  const now = countOf(g, nodeId);
  if (!node || !c || !now) return { error: "This node's pins are fixed." };
  if (now.n >= now.max) return { error: `${nodeDef(node.type)?.title ?? "This node"} has ${now.max} at most.` };
  if (c.kind === "count") return { graph: withParams(g, nodeId, { [c.param]: now.n + 1 }) };
  const cases = switchCases(node, nodeDef(node.type)!);
  let next: string;
  if (c.numbers) {
    const nums = cases.map(Number).filter(Number.isFinite);
    next = String(nums.length ? Math.floor(Math.max(...nums)) + 1 : 1);
  } else {
    let k = cases.length + 1;
    while (cases.some((x) => x.toLowerCase() === `case${k}`)) k++;
    next = `case${k}`;
  }
  return { graph: withParams(g, nodeId, { cases: [...cases, next].join(",") }) };
}

/** The last pin off: refused at the least, or while a wire is on a pin that would go. */
export function removePin(g: Graph, nodeId: string): { graph: Graph } | { error: string } {
  const node = g.nodes.find((x) => x.id === nodeId);
  const c = node && countedOf(node.type);
  const now = countOf(g, nodeId);
  if (!node || !c || !now) return { error: "This node's pins are fixed." };
  if (now.n <= now.min) return { error: `${nodeDef(node.type)?.title ?? "This node"} needs at least ${now.min}.` };
  const next = c.kind === "count" ? withParams(g, nodeId, { [c.param]: now.n - 1 }) : withParams(g, nodeId, { cases: switchCases(node, nodeDef(node.type)!).slice(0, -1).join(",") });
  const before = pinsOf(g, node);
  const after = pinsOf(next, next.nodes.find((x) => x.id === nodeId)!);
  const goneIn = before.inputs.filter((p) => !after.inputs.some((q) => q.id === p.id)).map((p) => p.id);
  const goneOut = before.outputs.filter((p) => !after.outputs.some((q) => q.id === p.id)).map((p) => p.id);
  const wired = g.edges.some((e) => (e.to.node === nodeId && goneIn.includes(e.to.pin)) || (e.from.node === nodeId && goneOut.includes(e.from.pin)));
  if (wired) return { error: "The last pin has a wire: take it off first." };
  return { graph: next };
}
