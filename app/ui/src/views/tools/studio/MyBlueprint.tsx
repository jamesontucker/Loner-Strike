// The left panel, like Unreal's "My Blueprint": the mode's events, functions and variables, and
// the snippet library. + buttons add; click focuses (or shows the variable in Details); drag a
// variable onto the canvas for Get/Set, a snippet to drop it in.

import { useState, type ReactNode } from "react";
import { useStudio } from "./store";
import { nodeDef, typeColor } from "./registry";
import { addEnum, addFunction, addLocal, addMacro, addStruct, addVariable, makeSnippet, type Snippet } from "./graphOps";
import { DRAG_PART, DRAG_SNIPPET, DRAG_VAR, type CanvasApi } from "./Canvas";
import { typeLabel } from "./pins";
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
  const part = useStudio((s) => s.part);
  const tab = useStudio((s) => s.tab);
  const { edit, selectVar, setFuncFocus, selectPart, openTab } = useStudio.getState();
  const askText = useAskText();
  const ask = useConfirm();
  const [filter, setFilter] = useState("");
  if (!graph) return null;
  const events = graph.nodes.filter((n) => nodeDef(n.type)?.event && !["func.define", "fn.entry", "macro.entry"].includes(n.type));
  const funcs = graph.nodes.filter((n) => n.type === "func.define");
  const functions = graph.functions ?? [];
  const macros = graph.macros ?? [];
  const tabFn = functions.find((x) => x.id === tab);
  const varItem = (v: (typeof graph.variables)[number], local = false) => (
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
      title={`${typeLabel(graph, v.type)}${local ? ", this Function's own" : ""}: drag onto the canvas for Get or Set`}
    >
      <span className={`st-mb-pin${/^array</.test(v.type) ? " arr" : /^map</.test(v.type) ? " map" : ""}`} style={{ background: typeColor(v.type) }} /> {v.name}
      <span className="st-muted st-mb-note">{v.scope === "player" ? "per player" : v.scope === "team" ? "per team" : ""}</span>
    </button>
  );
  const partItem = (id: string, name: string, dot: string, drag: string | null, note = "", openable = false) => (
    <button
      key={id}
      type="button"
      draggable={!!drag}
      onDragStart={(e) => {
        if (!drag) return;
        e.dataTransfer.setData(DRAG_PART, drag);
        e.dataTransfer.effectAllowed = "copy";
      }}
      className={`st-mb-item${part === id ? " on" : ""}${tab === id ? " open" : ""}`}
      onClick={() => selectPart(id)}
      onDoubleClick={() => openable && openTab(id)}
      title={openable ? "Double-click to open its graph; drag onto the canvas to call it" : drag ? "Drag onto the canvas" : undefined}
    >
      <span className={`st-mb-dot ${dot}`} /> {name}
      {note && <span className="st-muted st-mb-note">{note}</span>}
    </button>
  );
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
      <Section
        title="Functions"
        count={functions.length + funcs.length}
        onAdd={() => {
          const r = addFunction(graph);
          edit(() => r.graph);
          selectPart(r.id);
          openTab(r.id);
        }}
        addTitle="Add a Function (its own graph)"
      >
        {functions.length + funcs.length === 0 && <p className="st-muted st-mb-empty">Pieces you call from several places, each on its own graph.</p>}
        {functions.map((f) => partItem(f.id, f.name, "fn", `fn:${f.id}`, f.pure ? "pure" : "", true))}
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
        title="Macros"
        count={macros.length}
        onAdd={() => {
          const r = addMacro(graph);
          edit(() => r.graph);
          selectPart(r.id);
          openTab(r.id);
        }}
        addTitle="Add a Macro"
        open={macros.length > 0}
      >
        {macros.length === 0 && <p className="st-muted st-mb-empty">Pieces copied in where they're used (several flows in and out).</p>}
        {macros.map((m) => partItem(m.id, m.name, "mac", `macro:${m.id}`, "", true))}
      </Section>
      {tabFn && (
        <Section
          title={`Local variables: ${tabFn.name}`}
          count={tabFn.locals?.length ?? 0}
          onAdd={() => {
            const r = addLocal(graph, tabFn.id, { name: `Local ${(tabFn.locals?.length ?? 0) + 1}`, type: "number", scope: "mode", default: 0 });
            edit(() => r.graph);
            selectVar(r.id);
          }}
          addTitle="Add a local variable"
        >
          {!tabFn.locals?.length && <p className="st-muted st-mb-empty">Values only this Function sees, fresh each time it runs.</p>}
          {(tabFn.locals ?? []).map((v) => varItem(v, true))}
        </Section>
      )}
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
        {graph.variables.map((v) => varItem(v))}
      </Section>
      <Section
        title="Structs"
        count={graph.structs?.length ?? 0}
        onAdd={() => {
          const r = addStruct(graph);
          edit(() => r.graph);
          selectPart(r.id);
        }}
        addTitle="Add a struct"
        open={!!graph.structs?.length}
      >
        {!graph.structs?.length && <p className="st-muted st-mb-empty">Records of named values (a Loadout: weapon, ammo).</p>}
        {(graph.structs ?? []).map((st) => partItem(st.id, st.name, "st", `make:${st.id}`, `${st.fields.length} fields`))}
      </Section>
      <Section
        title="Enums"
        count={graph.enums?.length ?? 0}
        onAdd={() => {
          const r = addEnum(graph);
          edit(() => r.graph);
          selectPart(r.id);
        }}
        addTitle="Add an enum"
        open={!!graph.enums?.length}
      >
        {!graph.enums?.length && <p className="st-muted st-mb-empty">Named choices (Class: Scout, Tank, Medic).</p>}
        {(graph.enums ?? []).map((e) => partItem(e.id, e.name, "en", null, `${e.values.length} values`))}
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
