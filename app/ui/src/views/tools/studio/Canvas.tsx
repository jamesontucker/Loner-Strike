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
  type Connection,
  type Edge,
  type EdgeChange,
  type Node,
  type NodeChange,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { useStudio } from "./store";
import { NODE_TYPES } from "./Nodes";
import { Palette, type PaletteAt } from "./Palette";
import { insertSnippet, type Snippet } from "./graphOps";
import { addComment, addNode, canConnect, connect, copy, insertReroute, inType, moveItems, outType, paste, remove, toggleBreakpoint } from "./graphOps";
import { firstPin, nodeDef, PIN_TYPES, type NodeDef, type PinType } from "./registry";
import { pathPairs } from "./debugState";
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
export const DRAG_SNIPPET = "application/x-studio-snippet";

export function Canvas({ toast, onApi, snippets }: { toast: (text: string, error?: boolean) => void; onApi?: (api: CanvasApi) => void; snippets: Snippet[] }) {
  const graph = useStudio((s) => s.graph)!;
  const selected = useStudio((s) => s.selected);
  const problems = useStudio((s) => s.problems);
  const path = useStudio((s) => s.debug.path);
  const playing = useStudio((s) => !!s.playing);
  const { edit, live, commit, select, undo, redo } = useStudio.getState();
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
      const r = insertSnippet(g, sn, at, sizeOf);
      if (!r) return toast("That snippet is empty.", true);
      edit(() => r.graph);
      select(r.ids);
    },
    [edit, select, sizeOf, toast],
  );

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
        select([id]);
        void rf.setCenter(n.pos.x + 110, n.pos.y + 50, { zoom: Math.max(rf.getZoom(), 1), duration: 300 });
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
        edit((g) => {
          const r = addNode(g, type, { x: c.x - 110, y: c.y - 40 });
          return params ? { ...r.graph, nodes: r.graph.nodes.map((n) => (n.id === r.id ? { ...n, params: { ...n.params, ...params } } : n)) } : r.graph;
        });
      },
      snippetAtCenter: (sn) => {
        const c = center();
        dropSnippet(sn, { x: c.x - 200, y: c.y - 120 });
      },
    });
  }, [onApi, rf, select, center, edit, dropSnippet]);

  const sel = useMemo(() => new Set(selected), [selected]);
  const nodes: Node[] = useMemo(
    () => [
      ...graph.comments.map((c) => ({ id: c.id, type: "comment", position: c.pos, data: {}, selected: sel.has(c.id), zIndex: -1, dragHandle: ".st-comment-head", style: { width: c.size.w, height: c.size.h } })),
      ...graph.nodes.map((n) => ({ id: n.id, type: nodeDef(n.type)?.reroute ? "reroute" : "logic", position: n.pos, data: {}, selected: sel.has(n.id) })),
    ],
    [graph.comments, graph.nodes, sel],
  );

  const onPath = useMemo(() => pathPairs(path), [path]);
  const badEdges = useMemo(() => new Set(problems.filter((p) => p.edge && p.level === "error").map((p) => p.edge!)), [problems]);
  const edges: Edge[] = useMemo(
    () =>
      graph.edges.map((e) => {
        const t: PinType = outType(graph, e.from) ?? "any";
        const exec = t === "exec";
        const [a, b] = realEnds(graph, e);
        const lit = playing && onPath.has(`${a}>${b}`);
        const color = badEdges.has(e.id) ? "var(--st-bad)" : exec ? "var(--st-exec)" : PIN_TYPES[t].color;
        return {
          id: e.id,
          source: e.from.node,
          sourceHandle: e.from.pin,
          target: e.to.node,
          targetHandle: e.to.pin,
          selected: selEdges.includes(e.id),
          animated: lit,
          className: `${exec ? "st-edge-exec" : "st-edge-data"}${lit ? " lit" : ""}`,
          style: { stroke: color, strokeWidth: exec ? 3 : 2 },
          markerEnd: exec ? { type: MarkerType.ArrowClosed, color, width: 14, height: 14 } : undefined,
        };
      }),
    [graph, onPath, badEdges, selEdges, playing],
  );

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
    (def: NodeDef) => {
      const at = palette;
      setPalette(null);
      if (!at) return;
      edit((g) => {
        const { graph: g2, id } = addNode(g, def.type, at.wire?.dir === "in" ? { x: at.flow.x - 220, y: at.flow.y - 20 } : { x: at.flow.x, y: at.flow.y - 20 });
        if (!at.wire) return g2;
        const pin = firstPin(def, at.wire.type, at.wire.dir);
        if (!pin) return g2;
        const r = at.wire.dir === "out" ? connect(g2, { node: at.wire.node, pin: at.wire.pin }, { node: id, pin: pin.id }) : connect(g2, { node: id, pin: pin.id }, { node: at.wire.node, pin: at.wire.pin });
        return "error" in r ? g2 : r.graph;
      });
    },
    [palette, edit],
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
    edit((g2) => addComment(g2, box).graph);
  }, [rf, edit, palette]);

  const removeSelected = useCallback(() => {
    const ids = [...useStudio.getState().selected, ...selEdges];
    if (!ids.length) return;
    edit((g) => remove(g, ids));
    select([]);
    setSelEdges([]);
  }, [edit, select, selEdges]);

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
        const r = paste(g, text, rf.screenToFlowPosition(mouse.current));
        if (!r) toast("The clipboard doesn't hold nodes.", true);
        else {
          edit(() => r.graph);
          select(r.ids);
        }
      } else if (mod && k === "a") {
        e.preventDefault();
        const g = useStudio.getState().graph;
        if (g) select([...g.nodes.map((n) => n.id), ...g.comments.map((c) => c.id)]);
      } else if (k === "delete" || k === "backspace") {
        e.preventDefault();
        removeSelected();
      } else if (e.key === " " && !mod) {
        e.preventDefault();
        openPalette(mouse.current.x, mouse.current.y);
      } else if (k === "c" && !mod) {
        commentAround();
      } else if (e.key === "F9") {
        e.preventDefault();
        for (const id of useStudio.getState().selected) if (graph.nodes.some((n) => n.id === id)) edit((g) => toggleBreakpoint(g, id));
      }
    },
    [undo, redo, removeSelected, toast, rf, edit, select, openPalette, commentAround, graph.nodes],
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
        if (e.dataTransfer.types.includes(DRAG_VAR) || e.dataTransfer.types.includes(DRAG_SNIPPET)) {
          e.preventDefault();
          e.dataTransfer.dropEffect = "copy";
        }
      }}
      onDrop={(e) => {
        const flow = rf.screenToFlowPosition({ x: e.clientX, y: e.clientY });
        const v = e.dataTransfer.getData(DRAG_VAR);
        const sn = e.dataTransfer.getData(DRAG_SNIPPET);
        if (!v && !sn) return;
        e.preventDefault();
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
          if (node.type !== "comment") return;
          e.preventDefault();
          openPalette(e.clientX, e.clientY);
        }}
        onPaneClick={() => setPalette(null)}
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
      {palette && <Palette at={palette} onPick={pick} onComment={commentAround} onClose={() => setPalette(null)} />}
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
                edit((g) => {
                  const r = addNode(g, k === "get" ? "var.get" : "var.set", { x: d.flow.x - 20, y: d.flow.y - 20 });
                  return { ...r.graph, nodes: r.graph.nodes.map((n) => (n.id === r.id ? { ...n, params: { ...n.params, var: d.id } } : n)) };
                });
              }}
            >
              <span className="st-palette-title">{k === "get" ? "Get" : "Set"} {graph.variables.find((x) => x.id === varDrop.id)?.name ?? ""}</span>
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
