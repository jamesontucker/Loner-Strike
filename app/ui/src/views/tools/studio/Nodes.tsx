// How nodes look on the canvas: a logic node (header, pins, settings, last result), a reroute
// dot and a comment box. They read the graph, problems and debug state from the store by id.

import { memo, useState } from "react";
import { Handle, NodeResizer, Position, type NodeProps } from "@xyflow/react";
import { useStudio } from "./store";
import { nodeDef, PIN_TYPES, type NodeDef, type PinDef } from "./registry";
import { inType, outType, resizeComment, setCommentText, toggleBreakpoint } from "./graphOps";
import type { GNode, GVariable, Graph } from "./schema";

const CAT_CLASS: Record<string, string> = { Events: "ev", Conditions: "cond", Numbers: "num", Variables: "var", Timers: "timer", Players: "player", Messages: "msg", Scoring: "score", Components: "comp", Creatures: "creature", Flow: "flow" };

/** One line saying what a node is set to ("AK-47, 90 bullets"). */
export function paramSummary(def: NodeDef, n: GNode, wired: (pin: string) => boolean, vars: GVariable[] = []): string {
  return def.params
    .filter((p) => !def.inputs.some((i) => i.param === p.id && wired(i.id)))
    .map((p) => {
      const v = n.params[p.id] ?? p.default;
      if (p.kind === "choice") return p.choices?.find(([k]) => k === v)?.[1] ?? String(v);
      if (p.kind === "number") return `${p.label}: ${v}`;
      if (p.kind === "bool") return `${p.label}: ${v ? "yes" : "no"}`;
      if (p.kind === "var") return vars.find((x) => x.id === v)?.name ?? "(pick a variable)";
      if (p.kind === "name") return v ? `${p.label.replace(/^.*named$/i, "at")} ${String(v)}` : "";
      if (p.kind === "keys") return v ? String(v) : "";
      if (p.kind === "varvalue") return v === "" ? "" : `= ${typeof v === "boolean" ? (v ? "yes" : "no") : String(v)}`;
      return `“${String(v)}”`;
    })
    .filter(Boolean)
    .join(" · ");
}

function Pin({ pin, side, color, wired, problem }: { pin: PinDef; side: "in" | "out"; color: string; wired: boolean; problem?: string }) {
  const exec = pin.type === "exec";
  return (
    <div className={`st-pin st-pin-${side}${problem ? " bad" : ""}`} title={problem}>
      <Handle
        type={side === "in" ? "target" : "source"}
        position={side === "in" ? Position.Left : Position.Right}
        id={pin.id}
        className={`st-handle ${exec ? "st-handle-exec" : "st-handle-data"}${wired ? " wired" : ""}`}
        style={exec ? undefined : { borderColor: color, background: wired ? color : undefined }}
      />
      {pin.label && <span className="st-pin-label">{pin.label}</span>}
    </div>
  );
}

export const LogicNode = memo(function LogicNode({ id, selected }: NodeProps) {
  const n = useStudio((s) => s.graph?.nodes.find((x) => x.id === id));
  const graph = useStudio((s) => s.graph);
  const problems = useStudio((s) => s.problems);
  const run = useStudio((s) => s.debug.runs[id]);
  const paused = useStudio((s) => s.debug.paused?.node === id);
  const playing = useStudio((s) => !!s.playing);
  const bpsOff = useStudio((s) => s.bpsOff);
  const edit = useStudio((s) => s.edit);
  if (!n || !graph) return null;
  const def = nodeDef(n.type);
  if (!def) return <div className="st-node st-node-unknown">Unknown node: {n.type}</div>;
  const mine = problems.filter((p) => p.node === id);
  const errors = mine.filter((p) => p.level === "error");
  const wiredIn = (pin: string) => graph.edges.some((e) => e.to.node === id && e.to.pin === pin);
  const wiredOut = (pin: string) => graph.edges.some((e) => e.from.node === id && e.from.pin === pin);
  const pinProblem = (pin: string) => mine.find((p) => p.pin === pin)?.message;
  const summary = paramSummary(def, n, wiredIn, graph.variables);
  const failed = run?.result === "false";
  const cls = [
    "st-node",
    `st-cat-${CAT_CLASS[def.category] ?? "flow"}`,
    selected ? "selected" : "",
    errors.length ? "has-error" : mine.length ? "has-warning" : "",
    paused ? "paused" : "",
    run?.error ? "ran-error" : failed ? "ran-false" : "",
  ].join(" ");
  const rows = Math.max(def.inputs.length, def.outputs.length);
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
        <span className="st-node-title">{def.title}</span>
        {mine.length > 0 && (
          <span className={`st-node-badge ${errors.length ? "err" : "warn"}`} title={mine.map((p) => p.message).join("\n")}>
            {errors.length ? "!" : "?"}
          </span>
        )}
      </div>
      <div className="st-node-pins">
        {Array.from({ length: rows }, (_, i) => {
          const ip = def.inputs[i];
          const op = def.outputs[i];
          return (
            <div className="st-pin-row" key={i}>
              {ip ? <Pin pin={ip} side="in" color={PIN_TYPES[inType(graph, { node: id, pin: ip.id }) ?? ip.type].color} wired={wiredIn(ip.id)} problem={pinProblem(ip.id)} /> : <span />}
              {op ? <Pin pin={op} side="out" color={PIN_TYPES[outType(graph, { node: id, pin: op.id }) ?? op.type].color} wired={wiredOut(op.id)} /> : <span />}
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
  const graph = useStudio((s) => s.graph);
  const bad = useStudio((s) => s.problems.some((p) => p.node === id));
  if (!graph) return null;
  const t = outType(graph, { node: id, pin: "out" }) ?? "any";
  const color = PIN_TYPES[t].color;
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
