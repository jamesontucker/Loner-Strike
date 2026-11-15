// My library: Functions, Macros, structs and enums kept to use again in other modes. An item
// holds its part and everything it needs (the structs and enums it uses, the Functions and
// Macros it calls, the mode variables it reads or sets). Adding one to a mode merges what's
// already there (the same struct, the same variable) and renames on a clash; the copy remembers
// the item and version it came from, so the Studio can offer "Update from library" (never on its
// own). Items are graph data only, kept by the app (/api/tools/studio/library) with the same
// checks as graphs, and can be exported and imported as files.

import { freshId, uniqueName } from "./graphOps";
import { allVariables } from "./pins";
import { parseGraph, type GComment, type GEdge, type GEnum, type GFunction, type GMacro, type GMember, type GNode, type GStruct, type GVariable, type Graph } from "./schema";

export const LIBRARY = "loner-strike/graph-library";
/** several items in one file (Export all) */
export const LIBRARY_SET = "loner-strike/graph-library-set";
export type LibKind = "function" | "macro" | "struct" | "enum";

type GTable = NonNullable<Graph["dataTables"]>[number];
export type LibBody = { enums: GEnum[]; structs: GStruct[]; functions: GFunction[]; macros: GMacro[]; variables: GVariable[]; nodes: GNode[]; edges: GEdge[]; comments: GComment[]; dataTables?: GTable[] };
/** builtIn: shipped with the app (library/*.lib.json), read-only: it can be added, not deleted */
export type LibItem = { id: string; name: string; description: string; category: string; kind: LibKind; root: string; itemVersion: number; body: LibBody; builtIn?: boolean };

const TYPE_REF = /(struct|enum):([A-Za-z0-9_]{1,24})/g;
const refs = (t: string) => [...t.matchAll(TYPE_REF)].map((m) => m[2]);

/** What a part needs, with it: its graph, the parts it calls and uses, the mode variables. */
export function extractItem(g: Graph, root: string): { kind: LibKind; body: LibBody } | { error: string } {
  const fns = new Map((g.functions ?? []).map((x) => [x.id, x]));
  const macs = new Map((g.macros ?? []).map((x) => [x.id, x]));
  const sts = new Map((g.structs ?? []).map((x) => [x.id, x]));
  const ens = new Map((g.enums ?? []).map((x) => [x.id, x]));
  const kind: LibKind | null = fns.has(root) ? "function" : macs.has(root) ? "macro" : sts.has(root) ? "struct" : ens.has(root) ? "enum" : null;
  if (!kind) return { error: "That part isn't in this mode." };
  const want = new Set<string>([root]);
  const vars = new Set<string>();
  const queue = [root];
  const add = (id: string) => {
    if (!want.has(id) && (fns.has(id) || macs.has(id) || sts.has(id) || ens.has(id))) {
      want.add(id);
      queue.push(id);
    }
  };
  const addType = (t: string) => refs(t).forEach(add);
  while (queue.length) {
    const id = queue.pop()!;
    const f = fns.get(id) ?? macs.get(id);
    if (f) {
      for (const m of [...f.inputs, ...f.outputs]) addType(m.type);
      for (const l of (f as GFunction).locals ?? []) addType(l.type);
      for (const n of g.nodes.filter((x) => x.graph === id)) {
        for (const k of ["func", "macro", "struct", "enum"]) if (typeof n.params[k] === "string") add(n.params[k] as string);
        if (typeof n.params.of === "string") addType(n.params.of);
        const v = g.variables.find((x) => x.id === n.params.var);
        if (v && !vars.has(v.id)) {
          vars.add(v.id);
          addType(v.type);
        }
      }
    }
    sts.get(id)?.fields.forEach((m) => addType(m.type));
  }
  const graphs = new Set([...want].filter((id) => fns.has(id) || macs.has(id)));
  const nodes = g.nodes.filter((n) => n.graph && graphs.has(n.graph)).map((n) => ({ ...n, breakpoint: undefined }));
  const tables = new Set(nodes.map((n) => n.params.table).filter((t): t is string => typeof t === "string"));
  const ids = new Set(nodes.map((n) => n.id));
  const strip = <T extends { id: string; from?: unknown }>(x: T): T => (x.id === root ? { ...x, from: undefined } : x);
  return {
    kind,
    body: {
      enums: (g.enums ?? []).filter((x) => want.has(x.id)).map(strip),
      structs: (g.structs ?? []).filter((x) => want.has(x.id)).map(strip),
      functions: (g.functions ?? []).filter((x) => want.has(x.id)).map(strip),
      macros: (g.macros ?? []).filter((x) => want.has(x.id)).map(strip),
      variables: g.variables.filter((x) => vars.has(x.id)),
      nodes,
      edges: g.edges.filter((e) => ids.has(e.from.node) && ids.has(e.to.node)),
      comments: g.comments.filter((c) => c.graph && graphs.has(c.graph)),
      ...(tables.size ? { dataTables: (g.dataTables ?? []).filter((t) => tables.has(t.id)) } : {}),
    },
  };
}

/** The item as a file (what the app keeps, what's exported). */
export function itemFile(it: Omit<LibItem, "id" | "itemVersion"> & { itemVersion?: number }) {
  const b = it.body;
  return JSON.parse(
    JSON.stringify({
      schema: LIBRARY,
      version: 1,
      name: it.name,
      description: it.description,
      category: it.category,
      kind: it.kind,
      root: it.root,
      ...(it.itemVersion ? { itemVersion: it.itemVersion } : {}),
      enums: b.enums,
      structs: b.structs,
      functions: b.functions,
      macros: b.macros,
      variables: b.variables,
      nodes: b.nodes,
      edges: b.edges,
      comments: b.comments,
      ...(b.dataTables?.length ? { dataTables: b.dataTables } : {}),
    }),
  );
}

/** A library file read and checked as graph data; null if it isn't one. */
export function readItem(id: string, v: unknown): LibItem | null {
  if (!v || typeof v !== "object") return null;
  const o = v as Record<string, unknown>;
  if (o.schema !== LIBRARY || o.version !== 1) return null;
  const { schema: _s, version: _v, name, description, category, kind, root, itemVersion, ...rest } = o;
  void _s, _v;
  if (typeof name !== "string" || typeof root !== "string" || !["function", "macro", "struct", "enum"].includes(String(kind))) return null;
  const r = parseGraph({ schema: "loner-strike/graph", version: 3, mode: { id: "library", name: "Library" }, ...rest });
  if ("error" in r) return null;
  const g = r.graph;
  return {
    id,
    name,
    description: typeof description === "string" ? description : "",
    category: typeof category === "string" ? category : "",
    kind: kind as LibKind,
    root,
    itemVersion: typeof itemVersion === "number" ? itemVersion : 1,
    body: { enums: g.enums ?? [], structs: g.structs ?? [], functions: g.functions ?? [], macros: g.macros ?? [], variables: g.variables, nodes: g.nodes, edges: g.edges, comments: g.comments, ...(g.dataTables?.length ? { dataTables: g.dataTables } : {}) },
  };
}

const sameMembers = (a: GMember[], b: GMember[]) => a.length === b.length && a.every((m, i) => m.name === b[i].name && m.type === b[i].type);

/** Adds a library item to a mode (or, with `replace`, updates the part that came from it).
 *  What's already in the mode is used again: an enum or struct with the same name and the same
 *  values or fields, a variable with the same name, type and scope, a Function or Macro copied
 *  from the same item and version. Anything else comes in with fresh ids, renamed on a clash. */
export function insertItem(g0: Graph, it: LibItem, replace?: string): { graph: Graph; root: string } {
  let g: Graph = { ...g0, enums: [...(g0.enums ?? [])], structs: [...(g0.structs ?? [])], functions: [...(g0.functions ?? [])], macros: [...(g0.macros ?? [])], variables: [...g0.variables], nodes: [...g0.nodes], edges: [...g0.edges], comments: [...g0.comments] };
  const map = new Map<string, string>();
  const mapType = (t: string) => t.replace(TYPE_REF, (_, k: string, id: string) => `${k}:${map.get(id) ?? id}`);
  const mapMembers = (ms: GMember[]) => ms.map((m) => ({ ...m, type: mapType(m.type) }));
  // (ids handed out here aren't in the graph yet: kept apart so two never match)
  const handed = new Set<string>();
  const fresh = (prefix: string) => {
    const used = new Set([...allVariables(g).map((x) => x.id), ...[...(g.functions ?? []), ...(g.macros ?? []), ...(g.structs ?? []), ...(g.enums ?? [])].map((x) => x.id), ...handed]);
    let i = 1;
    while (used.has(`${prefix}${i}`)) i++;
    handed.add(`${prefix}${i}`);
    return `${prefix}${i}`;
  };
  const mark = (id: string) => (id === it.root ? { from: { lib: it.id, version: it.itemVersion } } : {});
  const b = it.body;
  for (const e of b.enums) {
    const same = !replace || e.id !== it.root ? g.enums!.find((x) => x.name === e.name && JSON.stringify(x.values) === JSON.stringify(e.values)) : undefined;
    if (same) {
      map.set(e.id, same.id);
      continue;
    }
    if (replace && e.id === it.root) {
      map.set(e.id, replace);
      g.enums = g.enums!.map((x) => (x.id === replace ? { ...x, values: e.values, ...mark(e.id) } : x));
      continue;
    }
    const id = fresh("e");
    map.set(e.id, id);
    g.enums!.push({ ...e, id, name: uniqueName(g.enums!.map((x) => x.name), e.name), ...mark(e.id) });
  }
  for (const s of b.structs) {
    const fields = mapMembers(s.fields);
    const same = !replace || s.id !== it.root ? g.structs!.find((x) => x.name === s.name && sameMembers(x.fields, fields)) : undefined;
    if (same) {
      map.set(s.id, same.id);
      continue;
    }
    if (replace && s.id === it.root) {
      map.set(s.id, replace);
      g.structs = g.structs!.map((x) => (x.id === replace ? { ...x, fields, ...mark(s.id) } : x));
      continue;
    }
    const id = fresh("s");
    map.set(s.id, id);
    g.structs!.push({ ...s, id, fields, name: uniqueName(g.structs!.map((x) => x.name), s.name), ...mark(s.id) });
  }
  for (const v of b.variables) {
    const type = mapType(v.type);
    const same = g.variables.find((x) => x.name === v.name && x.type === type && x.scope === v.scope);
    if (same) {
      map.set(v.id, same.id);
      continue;
    }
    const id = fresh("v");
    map.set(v.id, id);
    g.variables.push({ ...v, id, type, name: uniqueName(allVariables(g).map((x) => x.name), v.name) });
  }
  // data tables: one with the same name and columns is used again (its rows are the mode's);
  // another comes in with its rows, a fresh id and a free name
  const tmap = new Map<string, string>();
  if (b.dataTables?.length) {
    g.dataTables = [...(g.dataTables ?? [])];
    for (const t of b.dataTables) {
      const same = g.dataTables.find((x) => x.name === t.name && JSON.stringify(x.columns) === JSON.stringify(t.columns));
      if (same) {
        tmap.set(t.id, same.id);
        continue;
      }
      let i = 1;
      while (g.dataTables.some((x) => x.id === `t${i}`)) i++;
      tmap.set(t.id, `t${i}`);
      g.dataTables.push({ ...t, id: `t${i}`, name: uniqueName(g.dataTables.map((x) => x.name), t.name) });
    }
  }
  // Functions and Macros: the same copy again is used again; the one being updated keeps its id
  const parts: (["fn", GFunction] | ["mac", GMacro])[] = [...b.functions.map((f): ["fn", GFunction] => ["fn", f]), ...b.macros.map((m): ["mac", GMacro] => ["mac", m])];
  const newGraphs = new Set<string>();
  for (const [kind, p] of parts) {
    const list: (GFunction | GMacro)[] = kind === "fn" ? g.functions! : g.macros!;
    const isRoot = p.id === it.root;
    if (replace && isRoot) {
      map.set(p.id, replace);
      newGraphs.add(p.id);
      continue;
    }
    const same = list.find((x) => x.name === p.name && JSON.stringify(x.from ?? null) === JSON.stringify(isRoot ? { lib: it.id, version: it.itemVersion } : p.from ?? null));
    if (same) {
      map.set(p.id, same.id);
      continue;
    }
    map.set(p.id, fresh(kind === "fn" ? "f" : "m"));
    newGraphs.add(p.id);
  }
  // locals get fresh ids too (variable ids are one set)
  for (const f of b.functions.filter((x) => newGraphs.has(x.id))) for (const l of f.locals ?? []) map.set(l.id, fresh("l"));
  const taken = [...(g.functions ?? []), ...(g.macros ?? [])].map((x) => x.name);
  for (const [kind, p] of parts) {
    if (!newGraphs.has(p.id)) continue;
    const id = map.get(p.id)!;
    const isRoot = p.id === it.root;
    const def = {
      ...p,
      id,
      inputs: mapMembers(p.inputs),
      outputs: mapMembers(p.outputs),
      ...(kind === "fn" ? { locals: ((p as GFunction).locals ?? []).map((l) => ({ ...l, id: map.get(l.id)!, type: mapType(l.type) })) } : {}),
      ...mark(p.id),
    };
    if (replace && isRoot) {
      // the update: the old graph goes, the new one comes in under the same id and name
      const gone = new Set(g.nodes.filter((n) => n.graph === id).map((n) => n.id));
      g.nodes = g.nodes.filter((n) => !gone.has(n.id));
      g.edges = g.edges.filter((e) => !gone.has(e.from.node) && !gone.has(e.to.node));
      g.comments = g.comments.filter((c) => c.graph !== id);
      if (kind === "fn") g.functions = g.functions!.map((x) => (x.id === id ? { ...(def as GFunction), name: x.name } : x));
      else g.macros = g.macros!.map((x) => (x.id === id ? { ...(def as GMacro), name: x.name } : x));
    } else {
      const named = { ...def, name: uniqueName(taken, p.name) };
      taken.push(named.name);
      if (kind === "fn") g.functions!.push(named as GFunction);
      else g.macros!.push(named as GMacro);
    }
  }
  // their graphs: fresh node, wire and comment ids, settings pointing at the new ids
  const nodeMap = new Map<string, string>();
  const used = new Set<string>();
  for (const n of b.nodes.filter((x) => x.graph && newGraphs.has(x.graph))) {
    const id = freshId(g, "n", used);
    used.add(id);
    nodeMap.set(n.id, id);
    const params = { ...n.params };
    for (const k of ["func", "macro", "struct", "enum", "var"]) if (typeof params[k] === "string" && map.has(params[k] as string)) params[k] = map.get(params[k] as string)!;
    if (typeof params.of === "string") params.of = mapType(params.of);
    if (typeof params.table === "string" && tmap.has(params.table)) params.table = tmap.get(params.table)!;
    g.nodes.push({ ...n, id, graph: map.get(n.graph!)!, params });
  }
  for (const e of b.edges) {
    const a = nodeMap.get(e.from.node);
    const c = nodeMap.get(e.to.node);
    if (!a || !c) continue;
    const id = freshId(g, "w", used);
    used.add(id);
    g.edges.push({ id, from: { node: a, pin: e.from.pin }, to: { node: c, pin: e.to.pin } });
  }
  for (const c of b.comments.filter((x) => x.graph && newGraphs.has(x.graph))) {
    const id = freshId(g, "c", used);
    used.add(id);
    g.comments.push({ ...c, id, graph: map.get(c.graph!)! });
  }
  // (empty lists stay out of the file)
  for (const k of ["enums", "structs", "functions", "macros"] as const) if (!g[k]?.length) delete g[k];
  return { graph: g, root: map.get(it.root) ?? it.root };
}

/** The item a part came from, when the library has a newer version of it. */
export function newerInLibrary(items: LibItem[], from: { lib: string; version: number } | undefined): LibItem | null {
  if (!from) return null;
  const it = items.find((x) => x.id === from.lib);
  return it && it.itemVersion > from.version ? it : null;
}

/** An id for a new library item from its name. */
export function itemId(name: string, taken: string[]): string {
  let base = (name.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "") || "item").replace(/^([^a-z])/, "l_$1").replace(/^builtin_/, "my_").slice(0, 20);
  if (base.length < 2) base = "item";
  let id = base;
  for (let n = 2; taken.includes(id); n++) id = `${base}_${n}`.slice(0, 24);
  return id;
}

