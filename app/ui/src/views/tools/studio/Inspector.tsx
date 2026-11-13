// The side panel: the selected node's settings and help (or the mode's name and the problem
// list when nothing is selected).

import { useStudio } from "./store";
import { nodeDef, type ParamDef } from "./registry";
import { setParam, toggleBreakpoint } from "./graphOps";
import { LABEL_CHARS } from "./schema";

function ParamField({ p, value, onChange }: { p: ParamDef; value: string | number; onChange: (v: string | number) => void }) {
  if (p.kind === "choice")
    return (
      <select className="st-field" value={String(value)} onChange={(e) => onChange(e.target.value)}>
        {p.choices?.map(([k, l]) => (
          <option key={k} value={k}>
            {l}
          </option>
        ))}
      </select>
    );
  if (p.kind === "number")
    return (
      <input
        className="st-field st-field-num"
        type="number"
        min={p.min}
        max={p.max}
        step="any"
        value={Number(value)}
        onChange={(e) => {
          const n = Number(e.target.value);
          if (Number.isFinite(n)) onChange(Math.min(p.max ?? n, Math.max(p.min ?? n, n)));
        }}
      />
    );
  return (
    <input
      className="st-field"
      value={String(value)}
      maxLength={p.max ?? 100}
      onChange={(e) => {
        const v = e.target.value;
        if (LABEL_CHARS.test(v)) onChange(v);
      }}
    />
  );
}

const cleanLabel = (v: string, max: number) => [...v].filter((c) => LABEL_CHARS.test(c)).join("").slice(0, max);

export function Inspector({ focus }: { focus: (id: string) => void }) {
  const graph = useStudio((s) => s.graph);
  const selected = useStudio((s) => s.selected);
  const problems = useStudio((s) => s.problems);
  const runs = useStudio((s) => s.debug.runs);
  const edit = useStudio((s) => s.edit);
  if (!graph) return null;
  const node = selected.length === 1 ? graph.nodes.find((n) => n.id === selected[0]) : undefined;
  const def = node && nodeDef(node.type);

  if (node && def) {
    const mine = problems.filter((p) => p.node === node.id);
    const run = runs[node.id];
    return (
      <aside className="st-side">
        <h3 className="st-side-title">{def.title}</h3>
        <p className="st-side-help">{def.help}</p>
        {def.params.length > 0 && (
          <div className="st-side-fields">
            {def.params.map((p) => {
              const wiredInstead = def.inputs.some((i) => i.param === p.id && graph.edges.some((e) => e.to.node === node.id && e.to.pin === i.id));
              return (
                <label key={p.id} className="st-side-field">
                  <span>{p.label}</span>
                  {wiredInstead ? <em className="st-muted">From its wire</em> : <ParamField p={p} value={typeof node.params[p.id] === "boolean" ? p.default : (node.params[p.id] as string | number | undefined) ?? p.default} onChange={(v) => edit((g) => setParam(g, node.id, p.id, v))} />}
                </label>
              );
            })}
          </div>
        )}
        {!def.reroute && (
          <label className="st-side-check">
            <input type="checkbox" checked={!!node.breakpoint} onChange={() => edit((g) => toggleBreakpoint(g, node.id))} />
            <span>Breakpoint: pause the world when this runs (F9)</span>
          </label>
        )}
        {mine.length > 0 && (
          <ul className="st-problems">
            {mine.map((p, i) => (
              <li key={i} className={p.level}>
                {p.message}
              </li>
            ))}
          </ul>
        )}
        {run && (
          <div className="st-side-run">
            <h4>Last time it ran</h4>
            <div>
              {run.count}× · {run.error ? <span className="st-bad-text">{run.error}</span> : run.result ?? "ran"}
            </div>
            {run.inputs && Object.keys(run.inputs).length > 0 && <Values values={run.inputs} />}
          </div>
        )}
      </aside>
    );
  }

  const comment = selected.length === 1 ? graph.comments.find((c) => c.id === selected[0]) : undefined;
  if (comment)
    return (
      <aside className="st-side">
        <h3 className="st-side-title">Comment</h3>
        <p className="st-side-help">Double-click its title bar to change the text. Dragging it moves the nodes inside.</p>
      </aside>
    );

  return (
    <aside className="st-side">
      <h3 className="st-side-title">This mode</h3>
      <div className="st-side-fields">
        <label className="st-side-field">
          <span>Name</span>
          <input className="st-field" value={graph.mode.name} maxLength={40} onChange={(e) => edit((g) => ({ ...g, mode: { ...g.mode, name: cleanLabel(e.target.value, 40) } }))} />
        </label>
        <label className="st-side-field">
          <span>Description</span>
          <input className="st-field" value={graph.mode.description} maxLength={60} onChange={(e) => edit((g) => ({ ...g, mode: { ...g.mode, description: cleanLabel(e.target.value, 60) } }))} />
        </label>
        <div className="st-side-field">
          <span>Id</span>
          <code className="st-muted">{graph.mode.id}</code>
        </div>
      </div>
      <h4 className="st-side-sub">Problems</h4>
      {problems.length === 0 ? (
        <p className="st-muted">None. Ready to compile.</p>
      ) : (
        <ul className="st-problems">
          {problems.map((p, i) => (
            <li key={i} className={p.level}>
              {p.node ? (
                <button type="button" className="st-link" onClick={() => focus(p.node!)}>
                  {nodeDef(graph.nodes.find((n) => n.id === p.node)?.type ?? "")?.title ?? p.node}
                </button>
              ) : null}{" "}
              {p.message}
            </li>
          ))}
        </ul>
      )}
      <p className="st-side-help st-keys">
        Right-click or Space: add a node · drag from a pin to empty space: nodes for that wire · double-click a wire: reroute point · C: comment · F9:
        breakpoint · ⌘Z / ⇧⌘Z: undo / redo · ⌘C / ⌘V: copy / paste
      </p>
    </aside>
  );
}

/** A node's input values as the game saw them (players with team, health, weapon). */
export function Values({ values }: { values: Record<string, unknown> }) {
  return (
    <dl className="st-values">
      {Object.entries(values).map(([k, v]) => (
        <div key={k}>
          <dt>{k}</dt>
          <dd>{showValue(v)}</dd>
        </div>
      ))}
    </dl>
  );
}

export function showValue(v: unknown): string {
  if (v == null) return "nobody";
  if (typeof v === "object") {
    const p = v as { name?: string; team?: string; hp?: number; alive?: boolean; weapon?: string };
    if (typeof p.name === "string") return `${p.name} (${p.team ?? "?"}, ${p.alive ? `${p.hp} hp${p.weapon ? `, ${p.weapon}` : ""}` : "dead"})`;
    return JSON.stringify(v);
  }
  return String(v);
}
