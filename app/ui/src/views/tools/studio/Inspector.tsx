// The side panel: the selected node's settings and help (or the mode's name and the problem
// list when nothing is selected).

import { useStudio } from "./store";
import { KEYS_CHARS, NAME_CHARS, nodeDef, type ParamDef } from "./registry";
import { removeVariable, setParam, toggleBreakpoint, updateVariable } from "./graphOps";
import { LABEL_CHARS, type GNode, type GVariable } from "./schema";

type PV = string | number | boolean;

function NumberField({ value, min, max, onChange }: { value: number; min?: number; max?: number; onChange: (v: number) => void }) {
  return (
    <input
      className="st-field st-field-num"
      type="number"
      min={min}
      max={max}
      step="any"
      value={Number(value)}
      onChange={(e) => {
        const n = Number(e.target.value);
        if (Number.isFinite(n)) onChange(Math.min(max ?? 100000, Math.max(min ?? -100000, n)));
      }}
    />
  );
}

/** A text box that only takes the characters `allowed` lets through (the app checks the same). */
function TextField({ value, max, allowed, onChange }: { value: string; max: number; allowed: RegExp; onChange: (v: string) => void }) {
  return (
    <input
      className="st-field"
      value={value}
      maxLength={max}
      onChange={(e) => {
        const v = e.target.value;
        if (allowed.test(v) && v.length <= max) onChange(v);
      }}
    />
  );
}

/** A value for a variable of this type. */
function ValueField({ type, value, onChange }: { type: GVariable["type"]; value: PV | undefined; onChange: (v: PV) => void }) {
  if (type === "number") return <NumberField value={typeof value === "number" ? value : 0} onChange={onChange} />;
  if (type === "bool")
    return (
      <select className="st-field" value={value === true ? "1" : "0"} onChange={(e) => onChange(e.target.value === "1")}>
        <option value="1">Yes</option>
        <option value="0">No</option>
      </select>
    );
  return <TextField value={typeof value === "string" ? value : ""} max={60} allowed={LABEL_CHARS} onChange={onChange} />;
}

function ParamField({ p, value, vars, nodeVar, nodes, onChange }: { p: ParamDef; value: PV; vars: GVariable[]; nodeVar?: GVariable; nodes: GNode[]; onChange: (v: PV) => void }) {
  if (p.kind === "func" || p.kind === "timer") {
    const list = nodes.filter((n) => (p.kind === "func" ? n.type === "func.define" : ["timer.delay", "timer.repeat", "timer.countdown"].includes(n.type)));
    return (
      <select className="st-field" value={String(value)} onChange={(e) => onChange(e.target.value)}>
        <option value="">{list.length ? "Pick one…" : p.kind === "func" ? "Add a Function node first" : "Add a timer first"}</option>
        {list.map((n) => (
          <option key={n.id} value={n.id}>
            {p.kind === "func" ? String(n.params.name ?? n.id) : `${nodeDef(n.type)?.title ?? n.type} (${n.id})`}
          </option>
        ))}
      </select>
    );
  }
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
  if (p.kind === "number") return <NumberField value={Number(value)} min={p.min} max={p.max} onChange={onChange} />;
  if (p.kind === "bool")
    return (
      <select className="st-field" value={value ? "1" : "0"} onChange={(e) => onChange(e.target.value === "1")}>
        <option value="1">Yes</option>
        <option value="0">No</option>
      </select>
    );
  if (p.kind === "name") return <TextField value={String(value)} max={32} allowed={NAME_CHARS} onChange={onChange} />;
  if (p.kind === "keys") return <TextField value={String(value)} max={p.max ?? 120} allowed={KEYS_CHARS} onChange={onChange} />;
  if (p.kind === "var") {
    const list = vars.filter((v) => !p.varType || v.type === p.varType);
    return (
      <select className="st-field" value={String(value)} onChange={(e) => onChange(e.target.value)}>
        <option value="">{list.length ? "Pick one…" : "Add a variable under This mode first"}</option>
        {list.map((v) => (
          <option key={v.id} value={v.id}>
            {v.name} ({v.type === "bool" ? "yes/no" : v.type}
            {v.scope === "player" ? ", per player" : ""})
          </option>
        ))}
      </select>
    );
  }
  if (p.kind === "varvalue") return nodeVar ? <ValueField type={nodeVar.type} value={value === "" ? undefined : value} onChange={onChange} /> : <em className="st-muted">Pick a variable first</em>;
  return <TextField value={String(value)} max={p.max ?? 100} allowed={LABEL_CHARS} onChange={onChange} />;
}

/** The game settings the mode's cfg sets (each map, after base.cfg): Set Setting's choices. */
function ModeSettings() {
  const graph = useStudio((s) => s.graph)!;
  const edit = useStudio((s) => s.edit);
  const choices = nodeDef("game.set_cvar")?.params.find((p) => p.id === "cvar")?.choices ?? [];
  const list = graph.mode.settings ?? [];
  const put = (next: { cvar: string; value: number }[]) => edit((g) => ({ ...g, mode: { ...g.mode, settings: next } }));
  return (
    <>
      <h4 className="st-side-sub">Game settings</h4>
      <p className="st-side-help">Set every map, after the defaults (the mode's own cfg).</p>
      <ul className="st-vars-list">
        {list.map((st, k) => (
          <li key={k} className="st-var">
            <div className="st-var-row">
              <select className="st-field" value={st.cvar} onChange={(e) => put(list.map((x, j) => (j === k ? { ...x, cvar: e.target.value } : x)))} aria-label="Setting">
                {choices.map(([c, l]) => (
                  <option key={c} value={c}>
                    {l}
                  </option>
                ))}
              </select>
              <NumberField value={st.value} min={0} max={100000} onChange={(value) => put(list.map((x, j) => (j === k ? { ...x, value } : x)))} />
            </div>
            <button type="button" className="btn small" onClick={() => put(list.filter((_, j) => j !== k))}>
              Remove
            </button>
          </li>
        ))}
      </ul>
      <button type="button" className="btn small" disabled={list.length >= 32 || !choices.length} onClick={() => put([...list, { cvar: choices[0][0], value: 1 }])}>
        Add a setting
      </button>
    </>
  );
}

/** One variable's details (picked in the left panel). */
function VariableDetails({ id }: { id: string }) {
  const graph = useStudio((s) => s.graph)!;
  const edit = useStudio((s) => s.edit);
  const selectVar = useStudio((s) => s.selectVar);
  const v = graph.variables.find((x) => x.id === id);
  if (!v) return null;
  const used = graph.nodes.filter((n) => n.params.var === id).length;
  return (
    <aside className="st-side" aria-label="Details">
      <p className="st-side-kicker">Details · Variable</p>
      <div className="st-side-fields">
        <label className="st-side-field">
          <span>Name</span>
          <TextField value={v.name} max={30} allowed={LABEL_CHARS} onChange={(name) => edit((g) => updateVariable(g, v.id, { name }))} />
        </label>
        <label className="st-side-field">
          <span>Type</span>
          <select className="st-field" value={v.type} onChange={(e) => edit((g) => updateVariable(g, v.id, { type: e.target.value as GVariable["type"] }))}>
            <option value="number">Number</option>
            <option value="bool">Yes/No</option>
            <option value="text">Text</option>
          </select>
        </label>
        <label className="st-side-field">
          <span>One for</span>
          <select className="st-field" value={v.scope} onChange={(e) => edit((g) => updateVariable(g, v.id, { scope: e.target.value as GVariable["scope"] }))}>
            <option value="mode">The whole mode</option>
            <option value="player">Each player (reset when they join)</option>
            <option value="team">Each team</option>
          </select>
        </label>
        <label className="st-side-field">
          <span>Kept</span>
          <select className="st-field" value={v.reset ?? "never"} onChange={(e) => edit((g) => updateVariable(g, v.id, { reset: e.target.value as "never" | "round" }))}>
            <option value="never">For the map</option>
            <option value="round">Starts again each round</option>
          </select>
        </label>
        <label className="st-side-field">
          <span>Starts at</span>
          <ValueField type={v.type} value={v.default} onChange={(d) => edit((g) => updateVariable(g, v.id, { default: d }))} />
        </label>
      </div>
      <p className="st-side-help">
        Used by {used} node{used === 1 ? "" : "s"}. Drag it from the left onto the canvas for Get or Set.
      </p>
      <button
        type="button"
        className="btn small"
        onClick={() => {
          edit((g) => removeVariable(g, v.id));
          selectVar(null);
        }}
      >
        Remove this variable
      </button>
    </aside>
  );
}

const COMMENT_COLOURS = ["#4a5a6a", "#6a4a8a", "#2f6a4a", "#7a6a2b", "#7a2b2b", "#2f4f7a", "#555555"];

const cleanLabel = (v: string, max: number) => [...v].filter((c) => LABEL_CHARS.test(c)).join("").slice(0, max);

export function Inspector() {
  const graph = useStudio((s) => s.graph);
  const selected = useStudio((s) => s.selected);
  const selectedVar = useStudio((s) => s.selectedVar);
  const problems = useStudio((s) => s.problems);
  const runs = useStudio((s) => s.debug.runs);
  const edit = useStudio((s) => s.edit);
  if (!graph) return null;
  if (selectedVar && graph.variables.some((x) => x.id === selectedVar)) return <VariableDetails id={selectedVar} />;
  const node = selected.length === 1 ? graph.nodes.find((n) => n.id === selected[0]) : undefined;
  const def = node && nodeDef(node.type);

  if (node && def) {
    const mine = problems.filter((p) => p.node === node.id);
    const run = runs[node.id];
    return (
      <aside className="st-side" aria-label="Details">
        <p className="st-side-kicker">Details · {def.category}</p>
        <h3 className="st-side-title">{def.title}</h3>
        <p className="st-side-help">{def.help}</p>
        {def.needs && <p className="st-side-help st-needs">{def.needs === "components" ? "Needs the game mode components (gm_components) on the server; without them this does nothing and Playtest says so." : "Needs Loner-Strike's bot brain with creatures; without it this does nothing and Playtest says so."}</p>}
        {def.params.length > 0 && (
          <div className="st-side-fields">
            {def.params.map((p) => {
              const wiredInstead = def.inputs.some((i) => i.param === p.id && graph.edges.some((e) => e.to.node === node.id && e.to.pin === i.id));
              return (
                <label key={p.id} className="st-side-field">
                  <span>{p.label}</span>
                  {wiredInstead ? <em className="st-muted">From its wire</em> : <ParamField p={p} value={node.params[p.id] ?? p.default} vars={graph.variables} nodeVar={graph.variables.find((x) => x.id === node.params.var)} nodes={graph.nodes} onChange={(v) => edit((g) => setParam(g, node.id, p.id, v))} />}
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
      <aside className="st-side" aria-label="Details">
        <p className="st-side-kicker">Details · Comment</p>
        <p className="st-side-help">Double-click its title bar to change the text. Dragging the title bar moves the box and the nodes inside; inside it, the canvas works as anywhere.</p>
        <div className="st-swatches" role="group" aria-label="Colour">
          {COMMENT_COLOURS.map((c) => (
            <button key={c} type="button" className={`st-swatch${(comment.color ?? "#4a5a6a") === c ? " on" : ""}`} style={{ background: c }} aria-label={`Colour ${c}`} onClick={() => edit((g) => ({ ...g, comments: g.comments.map((x) => (x.id === comment.id ? { ...x, color: c } : x)) }))} />
          ))}
        </div>
      </aside>
    );

  return (
    <aside className="st-side" aria-label="Details">
      <p className="st-side-kicker">Details · Mode</p>
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
      <ModeSettings />
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
