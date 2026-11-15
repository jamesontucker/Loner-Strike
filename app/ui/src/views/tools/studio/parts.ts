// What the left panel's items can do, the same for each kind: Rename, Duplicate, Delete (saying
// where it's used first), Find uses. A "part" here is a Function, Macro, struct, enum,
// variable (or a Function's local), map marker, or an event node.

import { allVariables } from "./pins";
import { addMarker, freshPartId, remove, removeGraphPart, removeMarker, removeType, removeVariable, uniqueName, updateEnum, updateFunction, updateMacro, updateMarker, updateStruct, updateVariable } from "./graphOps";
import { extractItem, insertItem } from "./library";
import { nodeDef } from "./registry";
import type { GNode, Graph } from "./schema";

export type PartKind = "fn" | "macro" | "struct" | "enum" | "var" | "marker" | "event";
export type PartRef = { kind: PartKind; id: string };

/** What a sidebar id is ("mk:<id>" for markers; an event is its node id). */
export function partOf(g: Graph, id: string): PartRef | null {
  if (id.startsWith("mk:")) return (g.markers ?? []).some((m) => `mk:${m.id}` === id) ? { kind: "marker", id: id.slice(3) } : null;
  if ((g.functions ?? []).some((x) => x.id === id)) return { kind: "fn", id };
  if ((g.macros ?? []).some((x) => x.id === id)) return { kind: "macro", id };
  if ((g.structs ?? []).some((x) => x.id === id)) return { kind: "struct", id };
  if ((g.enums ?? []).some((x) => x.id === id)) return { kind: "enum", id };
  if (allVariables(g).some((x) => x.id === id)) return { kind: "var", id };
  // an event (or an old Function node): the node itself
  if (g.nodes.some((n) => n.id === id)) return { kind: "event", id };
  return null;
}

export const KIND_NAME: Record<PartKind, string> = { fn: "Function", macro: "Macro", struct: "struct", enum: "enum", var: "variable", marker: "map marker", event: "event" };

export function partName(g: Graph, p: PartRef): string {
  switch (p.kind) {
    case "fn":
      return g.functions?.find((x) => x.id === p.id)?.name ?? "";
    case "macro":
      return g.macros?.find((x) => x.id === p.id)?.name ?? "";
    case "struct":
      return g.structs?.find((x) => x.id === p.id)?.name ?? "";
    case "enum":
      return g.enums?.find((x) => x.id === p.id)?.name ?? "";
    case "var":
      return allVariables(g).find((x) => x.id === p.id)?.name ?? "";
    case "marker":
      return g.markers?.find((x) => x.id === p.id)?.label ?? "";
    case "event": {
      const n = g.nodes.find((x) => x.id === p.id);
      return n ? nodeDef(n.type)?.title ?? n.type : "";
    }
  }
}

/** The nodes that use a part (an event: itself), and other places (variables, fields, pins) typed with a struct or enum. */
export function usesOf(g: Graph, p: PartRef): { nodes: GNode[]; others: string[] } {
  const typeRef = p.kind === "struct" ? `struct:${p.id}` : p.kind === "enum" ? `enum:${p.id}` : null;
  const hasType = (t?: string) => !!typeRef && !!t && new RegExp(`(^|[<,])${typeRef.replace(":", "\\:")}([>,]|$)`).test(t);
  const nodes = g.nodes.filter((n) => {
    if (n.graph === p.id && (p.kind === "fn" || p.kind === "macro")) return false;
    const pr = n.params;
    switch (p.kind) {
      case "var":
        return pr.var === p.id;
      case "fn":
        return (n.type === "fn.call" || n.type === "fn.call_pure") && pr.func === p.id;
      case "macro":
        return n.type === "macro.use" && pr.macro === p.id;
      case "struct":
        return pr.struct === p.id || Object.values(pr).some((v) => typeof v === "string" && hasType(v));
      case "enum":
        return pr.enum === p.id || Object.values(pr).some((v) => typeof v === "string" && hasType(v));
      case "marker":
        return pr.marker === p.id;
      case "event":
        return n.id === p.id;
    }
  });
  const others: string[] = [];
  if (typeRef) {
    for (const v of allVariables(g)) if (hasType(v.type)) others.push(`the variable “${v.name}”`);
    for (const s of g.structs ?? []) if (s.id !== p.id && s.fields.some((f) => hasType(f.type))) others.push(`the struct “${s.name}”`);
    for (const f of [...(g.functions ?? []), ...(g.macros ?? [])]) if ([...f.inputs, ...f.outputs].some((m) => hasType(m.type))) others.push(`“${f.name}”'s pins`);
  }
  return { nodes, others };
}

/** "2 nodes on the Event Graph and 1 in Reward" (where the uses are), for the delete question. */
export function whereText(g: Graph, u: { nodes: GNode[]; others: string[] }): string {
  const by = new Map<string, number>();
  for (const n of u.nodes) {
    const eg = (g.eventGraphs ?? []).find((e) => e.id === n.graph);
    const home = !n.graph ? "the Event Graph" : eg ? `the ${eg.name} graph` : [...(g.functions ?? []), ...(g.macros ?? [])].find((x) => x.id === n.graph)?.name ?? "a Function";
    by.set(home, (by.get(home) ?? 0) + 1);
  }
  const parts = [...by].map(([home, c], i) => `${c}${i === 0 ? ` node${c === 1 ? "" : "s"}` : ""} ${home.startsWith("the ") ? "on" : "in"} ${home}`);
  return [...parts, ...u.others].join(", ");
}

export function renamePart(g: Graph, p: PartRef, name: string): Graph {
  switch (p.kind) {
    case "fn":
      return updateFunction(g, p.id, { name });
    case "macro":
      return updateMacro(g, p.id, { name });
    case "struct":
      return updateStruct(g, p.id, { name });
    case "enum":
      return updateEnum(g, p.id, { name });
    case "var":
      return updateVariable(g, p.id, { name });
    case "marker":
      return updateMarker(g, p.id, { label: name });
    case "event":
      return g;
  }
}

/** Removes a part (what used it shows a problem until changed). */
export function deletePart(g: Graph, p: PartRef): Graph {
  switch (p.kind) {
    case "fn":
    case "macro":
      return removeGraphPart(g, p.id);
    case "struct":
    case "enum":
      return removeType(g, p.id);
    case "var":
      return removeVariable(g, p.id);
    case "marker":
      return removeMarker(g, p.id);
    case "event":
      return remove(g, [p.id]);
  }
}

/** A copy beside it (a Function or Macro with its whole graph); the copy's sidebar id, or null. */
export function duplicatePart(g: Graph, p: PartRef): { graph: Graph; id: string } | null {
  const names = (l: { name: string }[] = []) => l.map((x) => x.name);
  switch (p.kind) {
    case "fn":
    case "macro": {
      const x = extractItem(g, p.id);
      if ("error" in x) return null;
      const name = partName(g, p);
      const r = insertItem(g, { id: "__copy", name, description: "", category: "", kind: x.kind, root: p.id, itemVersion: 0, body: x.body });
      // (a copy isn't from the library)
      const strip = <T extends { id: string; from?: unknown }>(l?: T[]) => l?.map((y) => (y.id === r.root ? { ...y, from: undefined } : y));
      return { graph: { ...r.graph, functions: strip(r.graph.functions), macros: strip(r.graph.macros) }, id: r.root };
    }
    case "struct": {
      const s = g.structs!.find((y) => y.id === p.id)!;
      const id = freshPartId(g, "s");
      return { graph: { ...g, structs: [...g.structs!, { ...s, id, from: undefined, name: uniqueName(names(g.structs), s.name) }] }, id };
    }
    case "enum": {
      const e = g.enums!.find((y) => y.id === p.id)!;
      const id = freshPartId(g, "e");
      return { graph: { ...g, enums: [...g.enums!, { ...e, id, from: undefined, name: uniqueName(names(g.enums), e.name) }] }, id };
    }
    case "var": {
      const v = allVariables(g).find((y) => y.id === p.id)!;
      const fn = (g.functions ?? []).find((f) => f.locals?.some((l) => l.id === p.id));
      const id = freshPartId(g, fn ? "l" : "v");
      const copy = { ...v, id, name: uniqueName(allVariables(g).map((y) => y.name), v.name) };
      if (fn) return { graph: { ...g, functions: g.functions!.map((f) => (f.id === fn.id ? { ...f, locals: [...(f.locals ?? []), copy] } : f)) }, id };
      return { graph: { ...g, variables: [...g.variables, copy] }, id };
    }
    case "marker": {
      const m = g.markers!.find((y) => y.id === p.id)!;
      const r = addMarker(g, m.type);
      let id = `${m.id}_2`.slice(0, 24);
      for (let i = 3; (g.markers ?? []).some((y) => y.id === id); i++) id = `${m.id.slice(0, 20)}_${i}`;
      const label = uniqueName((g.markers ?? []).map((y) => y.label), m.label).slice(0, 30);
      const graph = { ...r.graph, markers: r.graph.markers!.map((y) => (y.id === r.id ? { ...m, id, label } : y)) };
      return { graph, id: `mk:${id}` };
    }
    case "event":
      return null;
  }
}
