// How nodes look on the canvas: a logic node (header, pins, settings, last result), a reroute
// dot and a comment box. They read the graph, problems and debug state from the store by id.

import { memo, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Handle, NodeResizer, Position, type NodeProps } from "@xyflow/react";
import { useStudio } from "./store";
import { container, nodeDef, typeColor, type NodeDef, type ParamDef, type PinDef } from "./registry";
import { outType, resizeComment, setCommentText, setParam, toggleBreakpoint } from "./graphOps";
import { LABEL_CHARS } from "./schema";
import { allVariables, nodeTitle, typeLabel } from "./pins";
import { showValue } from "./Inspector";
import type { GNode, GVariable, Graph } from "./schema";
import { pinsFor, problemsFor, structureOf, watchKey, wiringOf } from "./nodeView";
import { useMarks } from "./versionOps";

const CAT_CLASS: Record<string, string> = { Events: "ev", Conditions: "cond", Numbers: "num", Text: "num", Variables: "var", Timers: "timer", Players: "player", Messages: "msg", Scoring: "score", Components: "comp", "Map markers": "comp", Creatures: "creature", Flow: "flow", Arrays: "var", Maps: "var", Structs: "var", Enums: "var", Vectors: "num" };

/** How a node is drawn, as in Blueprints: events red, values (pure) green, functions purple,
 *  flow control grey, actions blue. */
export function kindOf(def: NodeDef): "event" | "pure" | "func" | "flow" | "action" {
  if (def.type.startsWith("func.") || def.type.startsWith("fn.") || def.type.startsWith("macro.")) return "func";
  if (def.event) return "event";
  if (def.pure) return "pure";
  if (["Flow", "Conditions", "Timers"].includes(def.category)) return "flow";
  return "action";
}

/** One line saying what a node is set to ("AK-47, 90 bullets"). */
export function paramSummary(def: NodeDef, n: GNode, wired: (pin: string) => boolean, vars: GVariable[] = [], g: Graph | null = null): string {
  return def.params
    .filter((p) => !def.inputs.some((i) => i.param === p.id && wired(i.id)))
    // (typed next to its pin: UX review H6)
    .filter((p) => !(INLINE.includes(p.kind) && def.inputs.some((i) => i.param === p.id)))
    // (what the title already says)
    .filter((p) => !["fn", "macro", "struct", "enum", "enumvalue"].includes(p.kind) && !(p.kind === "var" && (n.type === "var.get" || n.type === "var.set")))
    .map((p) => {
      const v = n.params[p.id] ?? p.default;
      if (p.kind === "type") return typeLabel(g, String(v));
      if (p.kind === "cases") return "";
      if (p.kind === "field") {
        const sv = vars.find((x) => x.id === n.params.var);
        const st = g?.structs?.find((x) => `struct:${x.id}` === sv?.type);
        return `.${st?.fields.find((f) => f.id === v)?.name ?? "?"}`;
      }
      if (p.kind === "choice") return p.choices?.find(([k]) => k === v)?.[1] ?? String(v);
      if (p.kind === "number") return `${p.label}: ${v}`;
      if (p.kind === "bool") return `${p.label}: ${v ? "yes" : "no"}`;
      if (p.kind === "var") return vars.find((x) => x.id === v)?.name ?? "(pick a variable)";
      if (p.kind === "path") return v ? String(v).replace(/^.*\//, "") : "";
      if (p.kind === "name" || p.kind === "entname") return v ? `${p.label.replace(/^.*named$/i, "at")} ${String(v)}` : "";
      if (p.kind === "keys") return v ? String(v) : "";
      if (p.kind === "marker") return g?.markers?.find((m) => m.id === v)?.label ?? "(pick a marker)";
      if (p.kind === "varvalue") return v === "" ? "" : `= ${typeof v === "boolean" ? (v ? "yes" : "no") : String(v)}`;
      return `“${String(v)}”`;
    })
    .filter(Boolean)
    .join(" · ");
}

/** Settings that can be typed right next to their (unwired) input pin, as in Unreal. */
const INLINE: ParamDef["kind"][] = ["number", "bool", "text", "choice"];

/** A pin's value typed on the node (UX review H6): the node's own setting, so Details shows the
 *  same. Numbers and text are kept on Enter or leaving the box (one undo step); Escape undoes. */
function PinValue({ node, p, value }: { node: string; p: ParamDef; value: string | number | boolean }) {
  const edit = useStudio((s) => s.edit);
  const put = (v: string | number | boolean) => v !== value && edit((g) => setParam(g, node, p.id, v));
  const [draft, setDraft] = useState(String(value));
  const [typing, setTyping] = useState(false);
  const cancelled = useRef(false);
  useEffect(() => {
    if (!typing) setDraft(String(value));
  }, [value, typing]);
  const stop = { onPointerDown: (e: { stopPropagation: () => void }) => e.stopPropagation(), onDoubleClick: (e: { stopPropagation: () => void }) => e.stopPropagation() };
  if (p.kind === "bool") return <input type="checkbox" className="st-pin-value nodrag" checked={!!value} onChange={(e) => put(e.target.checked)} aria-label={p.label} {...stop} />;
  if (p.kind === "choice")
    return (
      <select className="st-pin-value st-pin-choice nodrag nowheel" value={String(value)} onChange={(e) => put(e.target.value)} aria-label={p.label} title={p.choices?.find(([k]) => k === String(value))?.[1]} {...stop}>
        {p.choices?.map(([k, l]) => (
          <option key={k} value={k}>
            {l}
          </option>
        ))}
      </select>
    );
  const num = p.kind === "number";
  const keep = () => {
    setTyping(false);
    if (cancelled.current) {
      cancelled.current = false;
      return setDraft(String(value));
    }
    if (num) {
      const n = Number(draft);
      if (draft.trim() === "" || !Number.isFinite(n)) return setDraft(String(value));
      const v = Math.min(p.max ?? 100000, Math.max(p.min ?? -100000, n));
      setDraft(String(v));
      put(v);
    } else {
      const t = [...draft].filter((c) => LABEL_CHARS.test(c)).join("").slice(0, 100);
      setDraft(t);
      put(t);
    }
  };
  return (
    <input
      className={`st-pin-value nodrag ${num ? "st-pin-num" : "st-pin-text"}`}
      type={num ? "number" : "text"}
      step="any"
      value={draft}
      aria-label={p.label}
      title={num && (p.min !== undefined || p.max !== undefined) ? `${p.min ?? ""} to ${p.max ?? ""}` : undefined}
      onFocus={() => setTyping(true)}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={keep}
      onKeyDown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") e.currentTarget.blur();
        else if (e.key === "Escape") {
          cancelled.current = true;
          e.currentTarget.blur();
        }
      }}
      {...stop}
    />
  );
}

function Pin({ pin, side, color, wired, problem, watch, value }: { pin: PinDef; side: "in" | "out"; color: string; wired: boolean; problem?: string; watch?: string; value?: ReactNode }) {
  const exec = pin.type === "exec";
  const shape = container(pin.type)?.kind;
  return (
    <div className={`st-pin st-pin-${side}${problem ? " bad" : ""}${watch ? " watched" : ""}`} title={problem ?? watch}>
      <Handle
        type={side === "in" ? "target" : "source"}
        position={side === "in" ? Position.Left : Position.Right}
        id={pin.id}
        aria-label={`${pin.label || (pin.type === "exec" ? "Flow" : "Value")} ${side === "in" ? "input" : "output"}`}
        className={`st-handle ${exec ? "st-handle-exec" : "st-handle-data"}${shape ? ` st-handle-${shape}` : ""}${wired ? " wired" : ""}`}
        style={exec ? undefined : { borderColor: color, background: wired ? color : undefined }}
      />
      {pin.label && <span className="st-pin-label">{pin.label}</span>}
      {value}
    </div>
  );
}

export const LogicNode = memo(function LogicNode({ id, selected }: NodeProps) {
  // (each piece by this node's id, so a drag step or a debugger batch re-renders only the nodes
  // it touches; docs/notes/PerfAudit.md #4, #8)
  const n = useStudio((s) => s.graph?.nodes.find((x) => x.id === id));
  const graph = useStudio((s) => structureOf(s.graph));
  const mine = useStudio((s) => problemsFor(s.problems, id));
  const run = useStudio((s) => s.debug.runs[id]);
  const watched = useStudio((s) => watchKey(structureOf(s.graph), s.debug.runs, id));
  const paused = useStudio((s) => s.debug.paused?.node === id);
  const playing = useStudio((s) => !!s.playing);
  const bpsOff = useStudio((s) => s.bpsOff);
  const mark = useMarks((m) => m.marks[id]);
  const edit = useStudio((s) => s.edit);
  const wiredKey = graph ? wiringOf(graph).wired.get(id) ?? "" : "";
  const def = n && nodeDef(n.type);
  // (titles, pins and the settings line: only when the node or the structure changes)
  const shape = useMemo(() => {
    if (!n || !graph || !def) return null;
    const wired = new Set(wiredKey.split("|"));
    const wiredIn = (pin: string) => wired.has(`i:${pin}`);
    return { wired, title: nodeTitle(graph, n), summary: paramSummary(def, n, wiredIn, allVariables(graph), graph), pins: pinsFor(graph, n) };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [n?.type, n?.params, n?.graph, graph, wiredKey, def]);
  const watchOuts = useMemo(() => {
    const m = new Map<string, unknown>();
    for (const part of watched.split("\u0002")) {
      if (!part) continue;
      const [pin, v] = part.split("\u0001");
      m.set(pin, JSON.parse(v));
    }
    return m;
  }, [watched]);
  if (!n || !graph) return null;
  if (!def || !shape) return <div className="st-node st-node-unknown">Unknown node: {n.type}</div>;
  const errors = mine.filter((p) => p.level === "error");
  const wiredIn = (pin: string) => shape.wired.has(`i:${pin}`);
  const wiredOut = (pin: string) => shape.wired.has(`o:${pin}`);
  const pinProblem = (pin: string) => mine.find((p) => p.pin === pin)?.message;
  const summary = shape.summary;
  const pins = shape.pins;
  // watch values: what the game last saw on a pin (the node's inputs as it ran; an output as the
  // nodes it feeds saw it)
  const watchIn = (p: PinDef) => {
    const v = run?.inputs?.[p.label];
    return v === undefined ? undefined : `${p.label || "Value"}: ${showValue(v)}`;
  };
  const watchOut = (p: PinDef) => {
    if (!watchOuts.has(p.id)) return undefined;
    return `${p.label || "Value"}: ${showValue(watchOuts.get(p.id))}`;
  };
  const failed = run?.result === "false";
  // an unwired input with its own setting: typed right there (UX review H6)
  const inlineOf = (ip: PinDef) => {
    if (!ip.param || wiredIn(ip.id)) return undefined;
    const p = def.params.find((x) => x.id === ip.param && INLINE.includes(x.kind));
    return p ? <PinValue node={id} p={p} value={n.params[p.id] ?? p.default} /> : undefined;
  };
  const cls = [
    "st-node",
    mark ? `st-mark-${mark}` : "",
    `st-kind-${kindOf(def)}`,
    `st-cat-${CAT_CLASS[def.category] ?? "flow"}`,
    selected ? "selected" : "",
    errors.length ? "has-error" : mine.length ? "has-warning" : "",
    paused ? "paused" : "",
    run?.error ? "ran-error" : failed ? "ran-false" : "",
  ].join(" ");
  const rows = Math.max(pins.inputs.length, pins.outputs.length);
  // a whole-mode variable's Get: a small pill (its name and the value), as in Unreal (UX review M4)
  if (n.type === "var.get" && !pins.inputs.length && pins.outputs.length === 1 && !run) {
    const op = pins.outputs[0];
    return (
      <div className={`${cls} st-node-pill`}>
        <span className="st-node-title">{shape.title.replace(/^Get /, "")}</span>
        {mine.length > 0 && (
          <span className={`st-node-badge ${errors.length ? "err" : "warn"}`} title={mine.map((p) => p.message).join("\n")}>
            {errors.length ? "!" : "?"}
          </span>
        )}
        <Pin pin={{ ...op, label: "" }} side="out" color={typeColor(op.type)} wired={wiredOut(op.id)} watch={watchOut(op)} />
      </div>
    );
  }
  return (
    <div className={cls}>
      {run && playing && <div key={run.at} className="st-flash" />}
      <div className="st-node-head">
        <button
          type="button"
          className={`st-bp${n.breakpoint ? " on" : ""}${bpsOff ? " off" : ""}`}
          title={n.breakpoint ? "Breakpoint: the world pauses here (click to remove)" : "Add a breakpoint (F9)"}
          aria-label="Breakpoint"
          onClick={(e) => {
            e.stopPropagation();
            edit((g) => toggleBreakpoint(g, id));
          }}
        />
        <span className="st-node-title">{shape.title}</span>
        {mine.length > 0 && (
          <span className={`st-node-badge ${errors.length ? "err" : "warn"}`} title={mine.map((p) => p.message).join("\n")}>
            {errors.length ? "!" : "?"}
          </span>
        )}
      </div>
      <div className="st-node-pins">
        {Array.from({ length: rows }, (_, i) => {
          const ip = pins.inputs[i];
          const op = pins.outputs[i];
          return (
            <div className="st-pin-row" key={i}>
              {ip ? <Pin pin={ip} side="in" color={typeColor(ip.type)} wired={wiredIn(ip.id)} problem={pinProblem(ip.id)} watch={watchIn(ip)} value={inlineOf(ip)} /> : <span />}
              {op ? <Pin pin={op} side="out" color={typeColor(op.type)} wired={wiredOut(op.id)} watch={watchOut(op)} /> : <span />}
            </div>
          );
        })}
      </div>
      {summary && <div className="st-node-params">{summary}</div>}
      {paused && <div className="st-node-run paused">Paused here</div>}
      {!paused && run && (
        <div className={`st-node-run${run.error ? " err" : failed ? " false" : ""}`} title={run.inputs ? JSON.stringify(run.inputs, null, 1) : undefined}>
          {run.error ?? (run.result ? `Last: ${run.result}` : "Ran")} <span className="st-node-count">{run.count}×</span>
        </div>
      )}
      {errors.length > 0 && !run && <div className="st-node-problem">{errors[0].message}</div>}
    </div>
  );
});

export const RerouteNode = memo(function RerouteNode({ id, selected }: NodeProps) {
  const graph = useStudio((s) => structureOf(s.graph));
  const bad = useStudio((s) => problemsFor(s.problems, id).length > 0);
  if (!graph) return null;
  const t = outType(graph, { node: id, pin: "out" }) ?? "any";
  const color = typeColor(t);
  return (
    <div className={`st-reroute${selected ? " selected" : ""}${bad ? " bad" : ""}${t === "exec" ? " exec" : ""}`} style={{ borderColor: color }} title="Reroute">
      <Handle type="target" position={Position.Left} id="in" className="st-handle st-handle-reroute" />
      <Handle type="source" position={Position.Right} id="out" className="st-handle st-handle-reroute" />
    </div>
  );
});

/** The graph when a comment's resize began (one undo step for the whole resize). */
let resizeBefore: Graph | null = null;

export const CommentNode = memo(function CommentNode({ id, selected }: NodeProps) {
  const c = useStudio((s) => s.graph?.comments.find((x) => x.id === id));
  const live = useStudio((s) => s.live);
  const commit = useStudio((s) => s.commit);
  const edit = useStudio((s) => s.edit);
  const [editing, setEditing] = useState(false);
  if (!c) return null;
  return (
    <div className={`st-comment${selected ? " selected" : ""}`} style={{ width: c.size.w, height: c.size.h, ...(c.color ? { ["--st-comment" as string]: c.color } : {}) }}>
      <NodeResizer
        isVisible={selected}
        minWidth={160}
        minHeight={80}
        onResizeStart={() => {
          resizeBefore = useStudio.getState().graph;
        }}
        onResize={(_, p) => live((g) => resizeComment(g, id, { w: p.width, h: p.height }, { x: p.x, y: p.y }))}
        onResizeEnd={() => {
          if (resizeBefore) commit(resizeBefore);
          resizeBefore = null;
        }}
      />
      <div className="st-comment-head" onDoubleClick={() => setEditing(true)} title="Double-click to edit">
        {editing ? (
          <textarea
            className="st-comment-edit nodrag"
            autoFocus
            defaultValue={c.text}
            maxLength={2000}
            onBlur={(e) => {
              const text = e.currentTarget.value;
              setEditing(false);
              if (text !== c.text) edit((g) => setCommentText(g, id, text));
            }}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Escape" || (e.key === "Enter" && !e.shiftKey)) e.currentTarget.blur();
            }}
          />
        ) : (
          <span>{c.text || "Comment"}</span>
        )}
      </div>
    </div>
  );
});

export const NODE_TYPES = { logic: LogicNode, reroute: RerouteNode, comment: CommentNode };
