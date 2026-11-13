// The left panel, like Unreal's "My Blueprint": the mode's events, functions and variables, and
// the snippet library. + buttons add; click focuses (or shows the variable in Details); drag a
// variable onto the canvas for Get/Set, a snippet to drop it in.

import { useState, type ReactNode } from "react";
import { useStudio } from "./store";
import { nodeDef, PIN_TYPES } from "./registry";
import { addVariable, makeSnippet, type Snippet } from "./graphOps";
import { DRAG_SNIPPET, DRAG_VAR, type CanvasApi } from "./Canvas";
import { saveSnippet, deleteSnippet } from "./snippets";
import { useAskText, useConfirm } from "../../../ui";

function Section({ title, count, onAdd, addTitle, children, open: start = true }: { title: string; count: number; onAdd?: () => void; addTitle?: string; children: ReactNode; open?: boolean }) {
  const [open, setOpen] = useState(start);
  return (
    <section className="st-mb-section">
      <div className="st-mb-head">
        <button type="button" className="st-mb-toggle" aria-expanded={open} onClick={() => setOpen(!open)}>
          <span className="st-mb-caret">{open ? "▾" : "▸"}</span> {title} <span className="st-muted">{count}</span>
        </button>
        {onAdd && (
          <button type="button" className="st-mb-add" onClick={onAdd} title={addTitle} aria-label={addTitle}>
            +
          </button>
        )}
      </div>
      {open && <div className="st-mb-body">{children}</div>}
    </section>
  );
}

export function MyBlueprint({ api, snippets, reloadSnippets, toast }: { api: CanvasApi | null; snippets: Snippet[]; reloadSnippets: () => void; toast: (text: string, error?: boolean) => void }) {
  const graph = useStudio((s) => s.graph);
  const selected = useStudio((s) => s.selected);
  const selectedVar = useStudio((s) => s.selectedVar);
  const { edit, selectVar, setFuncFocus } = useStudio.getState();
  const askText = useAskText();
  const ask = useConfirm();
  const [filter, setFilter] = useState("");
  if (!graph) return null;
  const events = graph.nodes.filter((n) => nodeDef(n.type)?.event && n.type !== "func.define");
  const funcs = graph.nodes.filter((n) => n.type === "func.define");
  const f = filter.trim().toLowerCase();
  const shown = snippets.filter((s) => !f || `${s.name} ${s.description}`.toLowerCase().includes(f));

  const label = (id: string) => {
    const n = graph.nodes.find((x) => x.id === id)!;
    const d = nodeDef(n.type);
    const extra = n.type === "func.define" ? String(n.params.name ?? "") : n.type === "event.chat_command" ? `!${String(n.params.command ?? "")}` : "";
    return extra ? `${d?.title ?? n.type}: ${extra}` : d?.title ?? n.type;
  };

  const saveSelection = async () => {
    const ids = useStudio.getState().selected;
    if (!ids.length) return toast("Select the nodes to keep first.", true);
    const name = await askText("What's the snippet called?", { label: "Name", placeholder: "My snippet", max: 40 }, { ok: "Save snippet" });
    if (!name?.trim()) return;
    const clean = [...name.trim()].filter((c) => /[\p{L}\p{N} .,!?'()\-:;+&%#@=<>[\]_~$]/u.test(c)).join("").slice(0, 40);
    const snippet = makeSnippet(graph, ids, clean || "My snippet");
    if (!snippet) return toast("Nothing to keep there.", true);
    const id = (clean.toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "") || "snippet").replace(/^([^a-z])/, "s_$1").slice(0, 24);
    const r = await saveSnippet(id.length < 2 ? "my_snippet" : id, snippet);
    if (r.error) return toast(r.error, true);
    toast(`Kept “${clean}” in your snippets.`);
    reloadSnippets();
  };

  return (
    <aside className="st-mb" aria-label="My mode">
      <Section title="Events" count={events.length} onAdd={() => api?.search("Events")} addTitle="Add an event">
        {events.length === 0 && <p className="st-muted st-mb-empty">Nothing starts this mode yet.</p>}
        {events.map((n) => (
          <button key={n.id} type="button" className={`st-mb-item${selected.includes(n.id) ? " on" : ""}`} onClick={() => api?.focus(n.id)}>
            <span className="st-mb-dot ev" /> {label(n.id)}
          </button>
        ))}
      </Section>
      <Section title="Functions" count={funcs.length} onAdd={() => api?.addAtCenter("func.define", { name: `function_${funcs.length + 1}` })} addTitle="Add a function">
        {funcs.length === 0 && <p className="st-muted st-mb-empty">Pieces you call from several places.</p>}
        {funcs.map((n) => (
          <button
            key={n.id}
            type="button"
            className={`st-mb-item${selected.includes(n.id) ? " on" : ""}`}
            onClick={() => {
              api?.focus(n.id);
              setFuncFocus(n.id);
            }}
          >
            <span className="st-mb-dot fn" /> {String(n.params.name ?? n.id)}
          </button>
        ))}
      </Section>
      <Section
        title="Variables"
        count={graph.variables.length}
        onAdd={() => {
          const r = addVariable(graph, { name: `Variable ${graph.variables.length + 1}`, type: "number", scope: "mode", default: 0 });
          edit(() => r.graph);
          selectVar(r.id);
        }}
        addTitle="Add a variable"
      >
        {graph.variables.length === 0 && <p className="st-muted st-mb-empty">Values the mode keeps.</p>}
        {graph.variables.map((v) => (
          <button
            key={v.id}
            type="button"
            draggable
            onDragStart={(e) => {
              e.dataTransfer.setData(DRAG_VAR, v.id);
              e.dataTransfer.effectAllowed = "copy";
            }}
            className={`st-mb-item${selectedVar === v.id ? " on" : ""}`}
            onClick={() => selectVar(v.id)}
            title="Drag onto the canvas for Get or Set"
          >
            <span className="st-mb-pin" style={{ background: PIN_TYPES[v.type].color }} /> {v.name}
            <span className="st-muted st-mb-note">{v.scope === "player" ? "per player" : v.scope === "team" ? "per team" : ""}</span>
          </button>
        ))}
      </Section>
      <Section title="Snippets" count={snippets.length}>
        <input className="st-field st-mb-filter" placeholder="Find a snippet" value={filter} onChange={(e) => setFilter(e.target.value)} aria-label="Find a snippet" />
        {shown.map((s) => (
          <div key={s.id} className="st-mb-snippet">
            <button
              type="button"
              draggable
              onDragStart={(e) => {
                e.dataTransfer.setData(DRAG_SNIPPET, s.id);
                e.dataTransfer.effectAllowed = "copy";
              }}
              className="st-mb-item"
              onClick={() => api?.snippetAtCenter(s)}
              title={`${s.description} (click or drag it in)`}
            >
              <span className="st-mb-dot sn" /> {s.name}
              {s.mine && <span className="st-muted st-mb-note">yours</span>}
            </button>
            {s.mine && (
              <button
                type="button"
                className="st-mb-add"
                title="Delete this snippet"
                aria-label={`Delete ${s.name}`}
                onClick={async () => {
                  if (!(await ask(`Delete your snippet “${s.name}”?`, { ok: "Delete", danger: true }))) return;
                  const r = await deleteSnippet(s.id);
                  if (r.error) toast(r.error, true);
                  reloadSnippets();
                }}
              >
                ×
              </button>
            )}
          </div>
        ))}
        <button type="button" className="btn small st-mb-save" onClick={() => void saveSelection()}>
          Save the selection as a snippet
        </button>
      </Section>
    </aside>
  );
}
