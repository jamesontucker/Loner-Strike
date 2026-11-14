// The node canvas: React Flow showing our graph (store.ts). Every change goes back into the
// graph JSON through graphOps, so undo/redo, saving and the compiler only ever see our format.

import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import {
  Background,
  BackgroundVariant,
  Controls,
  MarkerType,
  MiniMap,
  ReactFlow,
  useReactFlow,
  useStore,
  type Connection,
  type Edge,
  type EdgeChange,
  type Node,
  type NodeChange,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useStudio } from "./store";
import { NODE_TYPES } from "./Nodes";
import { Palette, type PaletteAt, type PaletteItem } from "./Palette";
import { collapse, insertSnippet, tabView, type Snippet } from "./graphOps";
import { addComment, addNode, canConnect, connect, copy, insertReroute, inType, moveItems, outType, paste, remove, toggleBreakpoint } from "./graphOps";
import { compatible, nodeDef, typeColor, type PinType } from "./registry";
import { allVariables, pinsOf } from "./pins";
import { insertItem, type LibItem } from "./library";

/** A library item into the mode, and (for a Function or Macro) a call to it at `at`. */
export function addFromLibrary(g: Graph, it: LibItem, at: { x: number; y: number } | null, home?: string): { graph: Graph; message: string } {
  const r = insertItem(g, it);
  let g2 = r.graph;
  const f = g2.functions?.find((x) => x.id === r.root);
  if (at && (it.kind === "function" || it.kind === "macro") && r.root !== home) {
    const type = it.kind === "macro" ? "macro.use" : f?.pure ? "fn.call_pure" : "fn.call";
    g2 = addNode(g2, type, { x: at.x - 20, y: at.y - 20 }, home, it.kind === "macro" ? { macro: r.root } : { func: r.root }).graph;
  }
  const added = (k: "enums" | "structs" | "functions" | "macros") => (g2[k]?.length ?? 0) - (g[k]?.length ?? 0);
  const n = added("enums") + added("structs") + added("functions") + added("macros") + (g2.variables.length - g.variables.length);
  return { graph: g2, message: n ? `Added “${it.name}” from your library (${n} part${n === 1 ? "" : "s"} with what it needs).` : `“${it.name}” is already in this mode.` };
}
import { pathPairs } from "./debugState";
import { structureOf } from "./nodeView";
import { useContextMenu } from "./ContextMenu";
import type { Graph } from "./schema";

/** The real ends of a wire (through reroutes), for lighting up the path the flow took. */
function realEnds(g: Graph, e: Graph["edges"][number]): [string, string] {
  const isRe = (id: string) => !!nodeDef(g.nodes.find((n) => n.id === id)?.type ?? "")?.reroute;
  let src = e.from.node;
  for (let i = 0; i < 50 && isRe(src); i++) src = g.edges.find((x) => x.to.node === src)?.from.node ?? src;
  let dst = e.to.node;
  for (let i = 0; i < 50 && isRe(dst); i++) dst = g.edges.find((x) => x.from.node === dst)?.to.node ?? dst;
  return [src, dst];
}

const IS_WEBKIT = typeof navigator !== "undefined" && /AppleWebKit/.test(navigator.userAgent) && !/Chrome|Chromium|Edg\//.test(navigator.userAgent);

/** Crisp text and borders at any zoom, on WebKit (macOS) and Chromium (Windows WebView2) alike:
 *  once a pan or zoom settles, the view's offset goes to whole device pixels (no half-pixel
 *  blur, no 1 px jitter between pans), and on WebKit the view is drawn again at its new scale
 *  (WebKit can keep a zoomed layer's old, lower-resolution bitmap). */
function settleViewport(rf: ReturnType<typeof useReactFlow>, box: HTMLDivElement | null, vp: { x: number; y: number; zoom: number }) {
  const dpr = window.devicePixelRatio || 1;
  const x = Math.round(vp.x * dpr) / dpr;
  const y = Math.round(vp.y * dpr) / dpr;
  if (x !== vp.x || y !== vp.y) void rf.setViewport({ x, y, zoom: vp.zoom });
  if (!IS_WEBKIT || !box) return;
  const el = box.querySelector<HTMLElement>(".react-flow__viewport");
  if (!el) return;
  requestAnimationFrame(() => {
    el.style.display = "none";
    void el.offsetHeight;
    el.style.display = "";
  });
}

/** What the panels around the canvas can ask of it. */
export type CanvasApi = {
  focus: (id: string) => void;
  /** the node search at the middle of the view, maybe only one category */
  search: (category?: string) => void;
  /** a node added at the middle of the view */
  addAtCenter: (type: string, params?: Record<string, string | number | boolean>) => void;
  /** a snippet dropped in at the middle of the view */
  snippetAtCenter: (s: Snippet) => void;
};

/** Things dragged from the left panel onto the canvas. */
export const DRAG_VAR = "application/x-studio-variable";
/** a Function, Macro or struct: "fn:<id>", "macro:<id>", "make:<id>", "break:<id>" */
export const DRAG_PART = "application/x-studio-part";
/** a library item's id */
export const DRAG_LIB = "application/x-studio-library";
export const DRAG_SNIPPET = "application/x-studio-snippet";

/** (one data object for every React Flow node: a new {} each render would defeat their memo) */
const NO_DATA = {};
/** Only what's in view (with a margin) is in the page: measured faster on every_node (PerfAudit #4).
 *  (window.__studioAll = true before the Studio opens draws everything, for comparing.) */
const VISIBLE_ONLY = typeof window === "undefined" || !(window as unknown as { __studioAll?: boolean }).__studioAll;

export function Canvas({ toast, onApi, snippets }: { toast: (text: string, error?: boolean) => void; onApi?: (api: CanvasApi) => void; snippets: Snippet[] }) {
  const graph = useStudio((s) => s.graph)!;
  const tab = useStudio((s) => s.tab);
  const view = useMemo(() => tabView(graph, tab), [graph, tab]);
  const selected = useStudio((s) => s.selected);
  const problems = useStudio((s) => s.problems);
  const path = useStudio((s) => s.debug.path);
  const playing = useStudio((s) => !!s.playing);
  const { edit, live, commit, select, undo, redo, openTab } = useStudio.getState();
  const home = tab || undefined;
  const rf = useReactFlow();
  const wrap = useRef<HTMLDivElement>(null);
  const mouse = useRef({ x: 200, y: 200 });
  const [palette, setPalette] = useState<PaletteAt | null>(null);
  const [selEdges, setSelEdges] = useState<string[]>([]);
  const dragBefore = useRef<Graph | null>(null);
  /** nodes riding along with a dragged comment: id -> offset from the comment */
  const riders = useRef(new Map<string, { comment: string; dx: number; dy: number }>());
  const connecting = useRef(false);

  // a variable dropped on the canvas: Get or Set?
  const [varDrop, setVarDrop] = useState<{ left: number; top: number; flow: { x: number; y: number }; id: string } | null>(null);

  const center = useCallback(() => {
    const box = wrap.current?.getBoundingClientRect();
    return box ? rf.screenToFlowPosition({ x: box.left + box.width / 2, y: box.top + box.height / 2 }) : { x: 0, y: 0 };
  }, [rf]);
  const sizeOf = useCallback((id: string) => {
    const n = rf.getNode(id);
    return { w: n?.measured?.width ?? 230, h: n?.measured?.height ?? 110 };
  }, [rf]);
  const dropSnippet = useCallback(
    (sn: Snippet, at: { x: number; y: number }) => {
      const g = useStudio.getState().graph;
      if (!g) return;
      const tab = useStudio.getState().tab;
      // a snippet with events (or, in a Function, a wait) can only go on the Event Graph
      const inFn = (g.functions ?? []).some((f) => f.id === tab);
      const eventOnly = !!tab && sn.part.nodes.some((n) => nodeDef(n.type)?.event || (inFn && ["timer.delay", "timer.retrigger", "timer.repeat", "timer.countdown", "msg.menu"].includes(n.type)));
      let where = at;
      if (eventOnly) {
        // under what's on the Event Graph already
        const ev = tabView(g, "");
        where = ev.nodes.length ? { x: Math.min(...ev.nodes.map((n) => n.pos.x)), y: Math.max(...ev.nodes.map((n) => n.pos.y)) + 260 } : { x: 0, y: 0 };
      }
      const r = insertSnippet(g, sn, where, sizeOf, eventOnly ? undefined : tab || undefined);
      if (!r) return toast("That snippet is empty.", true);
      edit(() => r.graph);
      if (eventOnly) {
        openTab("");
        toast(`“${sn.name}” starts with an event, so it went onto the Event Graph.`);
      }
      select(r.ids);
      if (eventOnly) setTimeout(() => void rf.setCenter(where.x + 300, where.y + 150, { zoom: Math.max(rf.getZoom(), 0.8), duration: 300 }), 120);
    },
    [edit, select, sizeOf, toast, openTab, rf],
  );

  // a new tab: show its graph
  useEffect(() => {
    const t = setTimeout(() => void rf.fitView({ padding: 0.2, maxZoom: 1.1 }), 30);
    return () => clearTimeout(t);
  }, [tab, rf]);

  // (development: tests set the zoom; see scripts and the crispness checks)
  useEffect(() => {
    if (!import.meta.env.DEV) return;
    (window as unknown as { __studioZoom?: (z: number) => void }).__studioZoom = (z) => {
      const n = rf.getNodes().find((x) => x.type === "logic");
      if (n) void rf.setCenter(n.position.x + 110, n.position.y + 50, { zoom: z });
    };
  }, [rf]);

  useEffect(() => {
    onApi?.({
      focus: (id) => {
        const n = useStudio.getState().graph?.nodes.find((x) => x.id === id);
        if (!n) return;
        // (on another tab: that tab first)
        const other = (n.graph ?? "") !== useStudio.getState().tab;
        if (other) openTab(n.graph ?? "");
        select([id]);
        setTimeout(() => void rf.setCenter(n.pos.x + 110, n.pos.y + 50, { zoom: Math.max(rf.getZoom(), 1), duration: 300 }), other ? 80 : 0);
      },
      search: (category) => {
        const box = wrap.current?.getBoundingClientRect();
        if (!box) return;
        const left = Math.max(box.width / 2 - 160, 8);
        const top = Math.max(box.height / 2 - 180, 8);
        setPalette({ left, top, flow: center(), only: category });
      },
      addAtCenter: (type, params) => {
        const c = center();
        edit((g) => addNode(g, type, { x: c.x - 110, y: c.y - 40 }, useStudio.getState().tab || undefined, params).graph);
      },
      snippetAtCenter: (sn) => {
        // (clicked rather than dragged: into free space under everything, then shown)
        const g = useStudio.getState().graph;
        const all = rf.getNodes();
        const at = all.length && g ? { x: Math.min(...all.map((n) => n.position.x)), y: Math.max(...all.map((n) => n.position.y + (n.measured?.height ?? 100))) + 80 } : center();
        dropSnippet(sn, at);
        void rf.setCenter(at.x + 300, at.y + 150, { zoom: Math.max(rf.getZoom(), 0.8), duration: 300 });
      },
    });
  }, [onApi, rf, select, center, edit, dropSnippet, openTab]);

  const sel = useMemo(() => new Set(selected), [selected]);
  // React Flow's node objects are kept while their node, comment and selection are the same, so
  // a drag step hands React Flow new objects only for what moved (PerfAudit #4)
  const rfNodes = useRef(new Map<string, { src: unknown; sel: boolean; node: Node }>());
  const nodes: Node[] = useMemo(() => {
    const cache = rfNodes.current;
    const keep = new Set<string>();
    const out: Node[] = [];
    const get = (id: string, src: unknown, isSel: boolean, make: () => Node) => {
      keep.add(id);
      const c = cache.get(id);
      if (c && c.src === src && c.sel === isSel) return c.node;
      const node = make();
      cache.set(id, { src, sel: isSel, node });
      return node;
    };
    for (const c of view.comments) out.push(get(c.id, c, sel.has(c.id), () => ({ id: c.id, type: "comment", position: c.pos, data: NO_DATA, selected: sel.has(c.id), zIndex: -1, dragHandle: ".st-comment-head", style: { width: c.size.w, height: c.size.h } })));
    for (const n of view.nodes) out.push(get(n.id, n, sel.has(n.id), () => ({ id: n.id, type: nodeDef(n.type)?.reroute ? "reroute" : "logic", position: n.pos, data: NO_DATA, selected: sel.has(n.id) })));
    for (const id of [...cache.keys()]) if (!keep.has(id)) cache.delete(id);
    return out;
  }, [view.comments, view.nodes, sel]);
  // far out, wires are drawn plainly (straight, no arrows)
  const far = useStore((s) => s.transform[2] < 0.35);

  const onPath = useMemo(() => pathPairs(path), [path]);
  const badEdges = useMemo(() => new Set(problems.filter((p) => p.edge && p.level === "error").map((p) => p.edge!)), [problems]);
  // wires: their types and ends from the structure (the same through a drag), and each wire's
  // object kept while nothing about it changed
  const structure = useMemo(() => structureOf(graph)!, [graph]);
  const rfEdges = useRef(new Map<string, { key: string; edge: Edge }>());
  const edges: Edge[] = useMemo(() => {
    const cache = rfEdges.current;
    const keep = new Set<string>();
    const out = view.edges.map((e) => {
        keep.add(e.id);
        const t: PinType = outType(structure, e.from) ?? "any";
        const exec = t === "exec";
        const [a, b] = realEnds(structure, e);
        const lit = playing && onPath.has(`${a}>${b}`);
        const color = badEdges.has(e.id) ? "var(--st-bad)" : exec ? "var(--st-exec)" : typeColor(t);
        const isSel = selEdges.includes(e.id);
        const key = `${e.from.node}.${e.from.pin}>${e.to.node}.${e.to.pin}|${color}|${lit}|${isSel}|${far}`;
        const c = cache.get(e.id);
        if (c && c.key === key) return c.edge;
        const edge: Edge = {
          id: e.id,
          source: e.from.node,
          sourceHandle: e.from.pin,
          target: e.to.node,
          targetHandle: e.to.pin,
          selected: isSel,
          animated: lit,
          className: `${exec ? "st-edge-exec" : "st-edge-data"}${lit ? " lit" : ""}`,
          style: { stroke: color, strokeWidth: exec ? 3 : 2 },
          markerEnd: exec && !far ? { type: MarkerType.ArrowClosed, color, width: 14, height: 14 } : undefined,
          ...(far ? { type: "straight" } : {}),
        };
        cache.set(e.id, { key, edge });
        return edge;
      });
    for (const id of [...cache.keys()]) if (!keep.has(id)) cache.delete(id);
    return out;
  }, [structure, view.edges, onPath, badEdges, selEdges, playing, far]);

  const onNodesChange = useCallback(
    (changes: NodeChange[]) => {
      const moves = new Map<string, { x: number; y: number }>();
      let selection: Set<string> | null = null;
      for (const c of changes) {
        if (c.type === "position" && c.position) {
          moves.set(c.id, c.position);
          for (const [rid, r] of riders.current) if (r.comment === c.id && !moves.has(rid)) moves.set(rid, { x: c.position.x + r.dx, y: c.position.y + r.dy });
        } else if (c.type === "select") {
          selection ??= new Set(useStudio.getState().selected);
          if (c.selected) selection.add(c.id);
          else selection.delete(c.id);
        }
      }
      if (moves.size) live((g) => moveItems(g, moves));
      if (selection) select([...selection]);
    },
    [live, select],
  );

  const onEdgesChange = useCallback((changes: EdgeChange[]) => {
    setSelEdges((cur) => {
      const s = new Set(cur);
      for (const c of changes) if (c.type === "select") (c.selected ? s.add(c.id) : s.delete(c.id));
      return [...s];
    });
  }, []);

  const isValidConnection = useCallback((c: Connection | Edge) => {
    const g = useStudio.getState().graph;
    if (!g || !c.sourceHandle || !c.targetHandle) return false;
    return canConnect(g, { node: c.source, pin: c.sourceHandle }, { node: c.target, pin: c.targetHandle }).ok;
  }, []);

  const onConnect = useCallback(
    (c: Connection) => {
      const g = useStudio.getState().graph;
      if (!g || !c.sourceHandle || !c.targetHandle) return;
      const r = connect(g, { node: c.source, pin: c.sourceHandle }, { node: c.target, pin: c.targetHandle });
      if ("error" in r) toast(r.error, true);
      else edit(() => r.graph);
    },
    [edit, toast],
  );

  const openPalette = useCallback(
    (clientX: number, clientY: number, wire?: PaletteAt["wire"]) => {
      const box = wrap.current?.getBoundingClientRect();
      if (!box) return;
      const left = Math.min(Math.max(clientX - box.left, 8), Math.max(box.width - 330, 8));
      const top = Math.min(Math.max(clientY - box.top, 8), Math.max(box.height - 380, 8));
      setPalette({ left, top, flow: rf.screenToFlowPosition({ x: clientX, y: clientY }), wire });
    },
    [rf],
  );

  // a wire dropped on empty space: offer the nodes it can attach to
  const onConnectEnd = useCallback(
    (event: MouseEvent | TouchEvent, state?: { isValid?: boolean | null; toHandle?: unknown; fromHandle?: { nodeId: string; id?: string | null; type: "source" | "target" } | null }) => {
      connecting.current = false;
      if (!state || state.isValid || state.toHandle || !state.fromHandle?.id) return;
      const target = event.target as HTMLElement | null;
      if (!target?.classList.contains("react-flow__pane")) return;
      const g = useStudio.getState().graph;
      if (!g) return;
      const ref = { node: state.fromHandle.nodeId, pin: state.fromHandle.id };
      const dir = state.fromHandle.type === "source" ? "out" : "in";
      const type = (dir === "out" ? outType(g, ref) : inType(g, ref)) ?? "any";
      const pt = "changedTouches" in event ? event.changedTouches[0] : event;
      openPalette(pt.clientX, pt.clientY, { type, dir, ...ref });
    },
    [openPalette],
  );

  const pick = useCallback(
    (item: PaletteItem) => {
      const at = palette;
      setPalette(null);
      if (!at) return;
      edit((g) => {
        const { graph: g2, id } = addNode(g, item.def.type, at.wire?.dir === "in" ? { x: at.flow.x - 220, y: at.flow.y - 20 } : { x: at.flow.x, y: at.flow.y - 20 }, home, item.params);
        if (!at.wire) return g2;
        const made = g2.nodes.find((n) => n.id === id)!;
        const pins = pinsOf(g2, made);
        const list = at.wire.dir === "out" ? pins.inputs : pins.outputs;
        const wt = at.wire.type;
        const fits = (t: string) => (at.wire!.dir === "out" ? compatible(wt, t) : compatible(t, wt)) && (t === "exec") === (wt === "exec");
        const pin = list.find((p) => p.type === wt) ?? list.find((p) => fits(p.type)) ?? list.find((p) => p.type !== "exec" && wt !== "exec" && /\$|any/.test(nodeDef(made.type)?.[at.wire!.dir === "out" ? "inputs" : "outputs"].find((r) => r.id === p.id)?.type ?? ""));
        if (!pin) return g2;
        const r = at.wire.dir === "out" ? connect(g2, { node: at.wire.node, pin: at.wire.pin }, { node: id, pin: pin.id }) : connect(g2, { node: id, pin: pin.id }, { node: at.wire.node, pin: at.wire.pin });
        return "error" in r ? g2 : r.graph;
      });
    },
    [palette, edit, home],
  );

  const collapseSelected = useCallback(
    (kind: "function" | "macro") => {
      setPalette(null);
      const g = useStudio.getState().graph;
      if (!g) return;
      const r = collapse(g, useStudio.getState().selected, kind, useStudio.getState().tab);
      if ("error" in r) return toast(r.error, true);
      edit(() => r.graph);
      toast(kind === "function" ? "Made a Function: its graph is open in a tab." : "Made a Macro: its graph is open in a tab.");
      openTab(r.id);
      useStudio.getState().selectPart(r.id);
    },
    [edit, toast, openTab],
  );

  const commentAround = useCallback(() => {
    setPalette(null);
    const g = useStudio.getState().graph;
    if (!g) return;
    const ids = new Set(useStudio.getState().selected);
    const inside = rf.getNodes().filter((n) => ids.has(n.id) && n.type !== "comment");
    let box: { x: number; y: number; w: number; h: number };
    if (inside.length) {
      const x0 = Math.min(...inside.map((n) => n.position.x)) - 30;
      const y0 = Math.min(...inside.map((n) => n.position.y)) - 60;
      const x1 = Math.max(...inside.map((n) => n.position.x + (n.measured?.width ?? 220))) + 30;
      const y1 = Math.max(...inside.map((n) => n.position.y + (n.measured?.height ?? 100))) + 30;
      box = { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
    } else {
      const p = palette?.flow ?? rf.screenToFlowPosition(mouse.current);
      box = { x: p.x, y: p.y, w: 320, h: 180 };
    }
    edit((g2) => addComment(g2, box, "Comment", home).graph);
  }, [rf, edit, palette, home]);

  const removeSelected = useCallback(() => {
    const ids = [...useStudio.getState().selected, ...selEdges];
    if (!ids.length) return;
    edit((g) => remove(g, ids));
    select([]);
    setSelEdges([]);
  }, [edit, select, selEdges]);

  /** a copy next to the selection (⌘D, the node menu) */
  const duplicateSelected = useCallback(() => {
    const g = useStudio.getState().graph;
    const ids = useStudio.getState().selected;
    const text = g && copy(g, ids);
    if (!g || !text) return;
    const sel = g.nodes.filter((n) => ids.includes(n.id));
    const at = sel.length ? { x: Math.min(...sel.map((n) => n.pos.x)) + 40, y: Math.min(...sel.map((n) => n.pos.y)) + 40 } : rf.screenToFlowPosition(mouse.current);
    const r = paste(g, text, at, home);
    if (r) {
      edit(() => r.graph);
      select(r.ids);
    }
  }, [rf, home, edit, select]);
  const cm = useContextMenu();
  const MOD = typeof navigator !== "undefined" && /Mac/.test(navigator.platform) ? "⌘" : "Ctrl+";

  const onKey = useCallback(
    async (e: ReactKeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (t.closest("input, textarea, select, .st-palette")) return;
      const mod = e.metaKey || e.ctrlKey;
      const k = e.key.toLowerCase();
      if (mod && k === "z") {
        e.preventDefault();
        if (e.shiftKey) redo();
        else undo();
      } else if (mod && k === "y") {
        e.preventDefault();
        redo();
      } else if (mod && (k === "c" || k === "x")) {
        const g = useStudio.getState().graph;
        const text = g && copy(g, useStudio.getState().selected);
        if (!text) return;
        e.preventDefault();
        try {
          await navigator.clipboard.writeText(text);
          if (k === "x") removeSelected();
        } catch {
          toast("Couldn't use the clipboard.", true);
        }
      } else if (mod && k === "v") {
        e.preventDefault();
        let text = "";
        try {
          text = await navigator.clipboard.readText();
        } catch {
          toast("Couldn't read the clipboard.", true);
          return;
        }
        const g = useStudio.getState().graph;
        if (!g) return;
        const r = paste(g, text, rf.screenToFlowPosition(mouse.current), home);
        if (!r) toast("The clipboard doesn't hold nodes.", true);
        else {
          edit(() => r.graph);
          select(r.ids);
        }
      } else if (mod && k === "a") {
        e.preventDefault();
        const g = useStudio.getState().graph;
        if (g) {
          const v = tabView(g, useStudio.getState().tab);
          select([...v.nodes.map((n) => n.id), ...v.comments.map((c) => c.id)]);
        }
      } else if (k === "delete" || k === "backspace") {
        e.preventDefault();
        removeSelected();
      } else if (e.key === " " && !mod) {
        e.preventDefault();
        openPalette(mouse.current.x, mouse.current.y);
      } else if (mod && k === "d") {
        e.preventDefault();
        duplicateSelected();
      } else if (k === "f" && !mod) {
        // frame the selection (or everything)
        const ids = new Set(useStudio.getState().selected);
        const nodes = rf.getNodes().filter((n) => ids.has(n.id));
        void rf.fitView({ nodes: nodes.length ? nodes : undefined, padding: 0.3, maxZoom: 1.5, duration: 250 });
      } else if (e.key === "Home") {
        e.preventDefault();
        void rf.fitView({ padding: 0.2, maxZoom: 1.1, duration: 250 });
      } else if (k === "q" && !mod) {
        // straighten: each selected node lines its first wired input up with what feeds it
        const g = useStudio.getState().graph;
        const ids = useStudio.getState().selected;
        if (!g || !ids.length) return;
        const moves = new Map<string, { x: number; y: number }>();
        const rowOf = (node: string, pin: string, dir: "in" | "out") => {
          const n = g.nodes.find((x) => x.id === node);
          if (!n) return 0;
          const p = pinsOf(g, n);
          return Math.max(0, (dir === "in" ? p.inputs : p.outputs).findIndex((x) => x.id === pin));
        };
        for (const id of ids) {
          const n = g.nodes.find((x) => x.id === id);
          const e2 = n && g.edges.find((x) => x.to.node === id && !ids.includes(x.from.node));
          const src = e2 && g.nodes.find((x) => x.id === e2.from.node);
          if (!n || !e2 || !src) continue;
          const y = src.pos.y + (rowOf(src.id, e2.from.pin, "out") - rowOf(id, e2.to.pin, "in")) * 22;
          moves.set(id, { x: n.pos.x, y });
        }
        if (moves.size) edit((g2) => moveItems(g2, moves));
      } else if (k === "c" && !mod) {
        commentAround();
      } else if (e.key === "F9") {
        e.preventDefault();
        for (const id of useStudio.getState().selected) if (graph.nodes.some((n) => n.id === id)) edit((g) => toggleBreakpoint(g, id));
      }
    },
    [undo, redo, removeSelected, toast, rf, edit, select, openPalette, commentAround, graph.nodes, home, duplicateSelected],
  );

  return (
    <div
      className="st-canvas"
      ref={wrap}
      tabIndex={0}
      onKeyDown={onKey}
      onMouseMove={(e) => {
        mouse.current = { x: e.clientX, y: e.clientY };
      }}
      onDragOver={(e) => {
        if (e.dataTransfer.types.includes(DRAG_VAR) || e.dataTransfer.types.includes(DRAG_SNIPPET) || e.dataTransfer.types.includes(DRAG_PART) || e.dataTransfer.types.includes(DRAG_LIB)) {
          e.preventDefault();
          e.dataTransfer.dropEffect = "copy";
        }
      }}
      onDrop={(e) => {
        const flow = rf.screenToFlowPosition({ x: e.clientX, y: e.clientY });
        const v = e.dataTransfer.getData(DRAG_VAR);
        const sn = e.dataTransfer.getData(DRAG_SNIPPET);
        const part = e.dataTransfer.getData(DRAG_PART);
        const lib = e.dataTransfer.getData(DRAG_LIB);
        if (!v && !sn && !part && !lib) return;
        e.preventDefault();
        if (lib) {
          const it = useStudio.getState().library.find((x) => x.id === lib);
          if (it) {
            const r = addFromLibrary(useStudio.getState().graph!, it, flow, home);
            edit(() => r.graph);
            toast(r.message);
          }
          return;
        }
        if (part) {
          const [k, pid] = part.split(":");
          const g = useStudio.getState().graph;
          const f = g?.functions?.find((x) => x.id === pid);
          const made: [string, Record<string, string>] | null =
            k === "fn" && f ? [f.pure ? "fn.call_pure" : "fn.call", { func: pid }] : k === "macro" ? ["macro.use", { macro: pid }] : k === "make" ? ["struct.make", { struct: pid }] : k === "break" ? ["struct.break", { struct: pid }] : k === "marker" ? ["marker.find", { marker: pid }] : null;
          if (!made || pid === tab) return toast("A Function or Macro can't call itself.", true);
          edit((g2) => addNode(g2, made[0], { x: flow.x - 20, y: flow.y - 20 }, home, made[1]).graph);
          return;
        }
        if (sn) {
          const found = snippets.find((x) => x.id === sn);
          if (found) dropSnippet(found, flow);
          return;
        }
        const box = wrap.current?.getBoundingClientRect();
        if (box) setVarDrop({ left: e.clientX - box.left, top: e.clientY - box.top, flow, id: v });
      }}
    >
      <ReactFlow
        nodes={nodes}
        edges={edges}
        onlyRenderVisibleElements={VISIBLE_ONLY}
        nodeTypes={NODE_TYPES}
        onNodesChange={onNodesChange}
        onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onConnectStart={() => {
          connecting.current = true;
        }}
        onConnectEnd={onConnectEnd as never}
        isValidConnection={isValidConnection}
        onNodeDragStart={(_, node, dragged) => {
          dragBefore.current = useStudio.getState().graph;
          riders.current.clear();
          const g = useStudio.getState().graph;
          if (!g) return;
          const moving = new Set(dragged.map((d) => d.id));
          for (const c of g.comments.filter((c) => moving.has(c.id))) {
            for (const n of g.nodes) {
              if (moving.has(n.id)) continue;
              if (n.pos.x >= c.pos.x && n.pos.y >= c.pos.y && n.pos.x <= c.pos.x + c.size.w && n.pos.y <= c.pos.y + c.size.h) riders.current.set(n.id, { comment: c.id, dx: n.pos.x - c.pos.x, dy: n.pos.y - c.pos.y });
            }
          }
          void node;
        }}
        onNodeDragStop={() => {
          riders.current.clear();
          if (dragBefore.current) commit(dragBefore.current);
          dragBefore.current = null;
        }}
        onPaneContextMenu={(e) => {
          e.preventDefault();
          openPalette(e.clientX, e.clientY);
        }}
        onNodeContextMenu={(e, node) => {
          // inside a comment box is still empty space
          if (node.type === "comment") {
            e.preventDefault();
            openPalette(e.clientX, e.clientY);
            return;
          }
          // the node's menu works on the selection it's in (or just it)
          const st = useStudio.getState();
          if (!st.selected.includes(node.id)) select([node.id]);
          const n = st.graph?.nodes.find((x) => x.id === node.id);
          const target = n && (n.type === "fn.call" || n.type === "fn.call_pure" ? n.params.func : n.type === "macro.use" ? n.params.macro : null);
          const many = Math.max(1, useStudio.getState().selected.length);
          cm.open(e, nodeDef(n?.type ?? "")?.title ?? "Node", [
            ...(typeof target === "string" && target ? [{ label: "Open its graph", run: () => openTab(target) }, "-" as const] : []),
            { label: many > 1 ? `Duplicate ${many} nodes` : "Duplicate", keys: `${MOD}D`, run: duplicateSelected },
            { label: "Comment around", keys: "C", run: commentAround },
            { label: n?.breakpoint ? "Remove the breakpoint" : "Breakpoint here", keys: "F9", run: () => edit((g) => toggleBreakpoint(g, node.id)) },
            "-",
            { label: many > 1 ? `Delete ${many} nodes` : "Delete", keys: "Del", danger: true, run: removeSelected },
          ]);
        }}
        onEdgeContextMenu={(e, edge) => {
          const p = rf.screenToFlowPosition({ x: e.clientX, y: e.clientY });
          cm.open(e, "Wire", [
            { label: "Add a reroute point", help: "Or double-click the wire", run: () => edit((g) => insertReroute(g, edge.id, { x: p.x - 8, y: p.y - 8 })) },
            "-",
            { label: "Break the wire", keys: "Alt+click", danger: true, run: () => edit((g) => remove(g, [edge.id])) },
          ]);
        }}
        onPaneClick={() => setPalette(null)}
        onNodeDoubleClick={(_, node) => {
          const n = useStudio.getState().graph?.nodes.find((x) => x.id === node.id);
          const target = n && (n.type === "fn.call" || n.type === "fn.call_pure" ? n.params.func : n.type === "macro.use" ? n.params.macro : null);
          if (typeof target === "string" && target) openTab(target);
        }}
        onEdgeClick={(e, edge) => {
          if (e.altKey) edit((g) => remove(g, [edge.id]));
        }}
        onEdgeDoubleClick={(e, edge) => {
          const p = rf.screenToFlowPosition({ x: e.clientX, y: e.clientY });
          edit((g) => insertReroute(g, edge.id, { x: p.x - 8, y: p.y - 8 }));
        }}
        onMoveEnd={(_, vp) => settleViewport(rf, wrap.current, vp)}
        deleteKeyCode={null}
        selectionKeyCode="Shift"
        multiSelectionKeyCode={["Meta", "Control"]}
        connectionRadius={24}
        minZoom={0.2}
        maxZoom={2.5}
        fitView
        fitViewOptions={{ padding: 0.2, maxZoom: 1.1 }}
        proOptions={{ hideAttribution: false }}
      >
        <Background id="minor" variant={BackgroundVariant.Lines} gap={16} lineWidth={1} color="var(--st-grid)" />
        <Background id="major" variant={BackgroundVariant.Lines} gap={128} lineWidth={1} color="var(--st-grid-major)" />
        <Controls showInteractive={false} />
        <MiniMap pannable zoomable className="st-minimap" />
      </ReactFlow>
      {cm.menu}
      {palette && <Palette at={palette} graph={graph} tab={tab} onPick={pick} onComment={commentAround} onCollapse={selected.length ? collapseSelected : undefined} onClose={() => setPalette(null)} />}
      {varDrop && (
        <div className="st-vardrop" style={{ left: varDrop.left, top: varDrop.top }} role="menu">
          {(["get", "set"] as const).map((k) => (
            <button
              key={k}
              type="button"
              className="st-palette-item"
              onClick={() => {
                const d = varDrop;
                setVarDrop(null);
                edit((g) => addNode(g, k === "get" ? "var.get" : "var.set", { x: d.flow.x - 20, y: d.flow.y - 20 }, home, { var: d.id }).graph);
              }}
            >
              <span className="st-palette-title">{k === "get" ? "Get" : "Set"} {allVariables(graph).find((x) => x.id === varDrop.id)?.name ?? ""}</span>
            </button>
          ))}
          <button type="button" className="st-palette-item" onClick={() => setVarDrop(null)}>
            <span className="st-palette-help">Cancel</span>
          </button>
        </div>
      )}
    </div>
  );
}
