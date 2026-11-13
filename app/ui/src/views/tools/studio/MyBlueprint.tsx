// The left panel, like Unreal's "My Blueprint": the mode's events, functions and variables, and
// the snippet library. + buttons add; click focuses (or shows the variable in Details); drag a
// variable onto the canvas for Get/Set, a snippet to drop it in.

import { useEffect, useRef, useState, type ReactNode } from "react";
import { useStudio } from "./store";
import { nodeDef, typeColor } from "./registry";
import { addEnum, addFunction, addLocal, addMacro, addMarker, addStruct, addVariable, makeSnippet, type Snippet } from "./graphOps";
import { addFromLibrary, DRAG_LIB, DRAG_PART, DRAG_SNIPPET, DRAG_VAR, type CanvasApi } from "./Canvas";
import { extractItem, itemFile, itemId, LIBRARY_SET, readItem, type LibItem } from "./library";
import { deleteItem, saveItem } from "./libraryApi";
import { LABEL_CHARS } from "./schema";
import { Sheet } from "../../../ui";
import { isMenuKey, useContextMenu, type MenuItem } from "./ContextMenu";
import { deletePart, duplicatePart, KIND_NAME, partName, partOf, renamePart, usesOf, whereText, type PartRef } from "./parts";
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
          <button
            type="button"
            className="st-mb-add"
            onClick={() => {
              // (what was just added shows)
              setOpen(true);
              onAdd();
            }}
            title={addTitle}
            aria-label={addTitle}
          >
            +
          </button>
        )}
      </div>
      {open && <div className="st-mb-body">{children}</div>}
    </section>
  );
}

const clean = (s: string, max: number) => [...s].filter((c) => LABEL_CHARS.test(c)).join("").slice(0, max);

/** "Save to my library": a name, a short description and a category for the part. */
function SaveToLibrary({ partId, onDone, toast }: { partId: string; onDone: () => void; toast: (text: string, error?: boolean) => void }) {
  const graph = useStudio((s) => s.graph)!;
  const library = useStudio((s) => s.library);
  const start = [...(graph.functions ?? []), ...(graph.macros ?? []), ...(graph.structs ?? []), ...(graph.enums ?? [])].find((x) => x.id === partId);
  // saving a part that came from the library again: the same item, a new version
  const fromItem = start && "from" in start && start.from ? library.find((x) => x.id === start.from!.lib) : undefined;
  const [name, setName] = useState(fromItem?.name ?? start?.name ?? "");
  const [description, setDescription] = useState(fromItem?.description ?? "");
  const [category, setCategory] = useState(fromItem?.category ?? "General");
  const [busy, setBusy] = useState(false);
  const save = async () => {
    const x = extractItem(graph, partId);
    if ("error" in x) return toast(x.error, true);
    const id = fromItem?.id ?? itemId(name, library.map((l) => l.id));
    setBusy(true);
    const r = await saveItem(id, { ...itemFile({ name: name.trim(), description, category: category.trim() || "General", kind: x.kind, root: partId, body: x.body }), ...(graph.mode.uid ? { fromMode: graph.mode.uid } : {}) });
    setBusy(false);
    if (r.error) return toast(r.error, true);
    // this copy is now that version of the item
    useStudio.getState().edit((g) => {
      const mark = { from: { lib: id, version: r.version ?? 1 } };
      const put = <T extends { id: string }>(l?: T[]) => l?.map((p) => (p.id === partId ? { ...p, ...mark } : p));
      return { ...g, functions: put(g.functions), macros: put(g.macros), structs: put(g.structs), enums: put(g.enums) };
    });
    await useStudio.getState().reloadLibrary();
    toast(`Kept “${name.trim()}” in your library${(r.version ?? 1) > 1 ? ` (version ${r.version})` : ""}.`);
    onDone();
  };
  return (
    <Sheet label="Save to my library" onClose={onDone}>
      <div className="st-libform">
        <h3>Save to my library</h3>
        <p className="st-muted">Use it in your other modes: it brings the structs, enums, Functions and variables it needs.</p>
        <label className="st-side-field">
          <span>Name</span>
          <input className="st-field" value={name} maxLength={40} onChange={(e) => setName(clean(e.target.value, 40))} autoFocus />
        </label>
        <label className="st-side-field">
          <span>Description</span>
          <input className="st-field" value={description} maxLength={160} onChange={(e) => setDescription(clean(e.target.value, 160))} placeholder="What it does" />
        </label>
        <label className="st-side-field">
          <span>Category</span>
          <input className="st-field" value={category} maxLength={30} list="st-lib-cats" onChange={(e) => setCategory(clean(e.target.value, 30))} />
          <datalist id="st-lib-cats">
            {[...new Set(library.map((l) => l.category))].map((c) => (
              <option key={c} value={c} />
            ))}
          </datalist>
        </label>
        <div className="st-libform-buttons">
          <button type="button" className="btn" onClick={onDone}>
            Cancel
          </button>
          <button type="button" className="btn primary" disabled={!name.trim() || busy} onClick={() => void save()}>
            {busy ? "Saving…" : fromItem ? "Save a new version" : "Save"}
          </button>
        </div>
      </div>
    </Sheet>
  );
}

/** My library: what you kept to reuse; double-click or drag one in, export, import, delete. */
function LibrarySection({ api, toast }: { api: CanvasApi | null; toast: (text: string, error?: boolean) => void }) {
  const library = useStudio((s) => s.library);
  const [q, setQ] = useState("");
  const file = useRef<HTMLInputElement>(null);
  const ask = useConfirm();
  const cm = useContextMenu();
  useEffect(() => {
    void useStudio.getState().reloadLibrary();
  }, []);
  const f = q.trim().toLowerCase();
  const shown = library.filter((it) => !f || `${it.name} ${it.description} ${it.category} ${it.kind}`.toLowerCase().includes(f));
  const add = (it: LibItem) => {
    const g = useStudio.getState().graph;
    if (!g) return;
    const r = addFromLibrary(g, it, null);
    useStudio.getState().edit(() => r.graph);
    toast(r.message);
    void api;
  };
  const download = (data: object, name: string) => {
    const blob = new Blob([JSON.stringify(data, null, 1)], { type: "application/json" });
    const a = document.createElement("a");
    a.href = URL.createObjectURL(blob);
    a.download = name;
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  };
  const dropItem = async (it: LibItem) => {
    if (!(await ask(`Delete “${it.name}” from your library? Modes that use it keep their copy.`, { ok: "Delete", danger: true }))) return;
    const r = await deleteItem(it.id);
    if (r.error) toast(r.error, true);
    await useStudio.getState().reloadLibrary();
  };
  const exportItem = (it: LibItem) => download(itemFile(it), `${it.id}.lib.json`);
  // the whole library in one file (Import takes it back, item by item)
  const exportAll = () => download({ schema: LIBRARY_SET, version: 1, items: library.map((it) => ({ id: it.id, item: itemFile(it) })) }, "my-studio-library.lib.json");
  const importFiles = async (files: FileList | null) => {
    for (const fl of [...(files ?? [])]) {
      let v: unknown;
      try {
        v = JSON.parse(await fl.text());
      } catch {
        toast(`${fl.name} isn't a library file.`, true);
        continue;
      }
      // one item, or a whole library exported at once
      const set = v && typeof v === "object" && (v as { schema?: string }).schema === LIBRARY_SET ? ((v as { items?: { id?: string; item?: unknown }[] }).items ?? []).slice(0, 500) : [{ id: fl.name.replace(/\.lib\.json$|\.json$/, ""), item: v }];
      let n = 0;
      for (const x of set) {
        const it = readItem("import", x.item);
        if (!it) {
          toast(`${fl.name}: that isn't a Game Mode Studio library item.`, true);
          continue;
        }
        const taken = useStudio.getState().library.map((l) => l.id);
        const id = itemId(typeof x.id === "string" ? x.id : it.name, taken);
        const r = await saveItem(id, itemFile({ ...it, itemVersion: undefined }));
        if (r.error) toast(r.error, true);
        else n++;
        await useStudio.getState().reloadLibrary();
      }
      if (n) toast(`Imported ${n} item${n === 1 ? "" : "s"} into your library.`);
    }
    await useStudio.getState().reloadLibrary();
  };
  let cat = "";
  return (
    <Section title="My library" count={library.length}>
      <input className="st-field st-mb-filter" placeholder="Find in my library" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Find in my library" />
      {library.length === 0 && <p className="st-muted st-mb-empty">Right-click a Function, Macro, struct or enum: Save to my library. Then use it in any mode.</p>}
      {shown.map((it) => {
        const head = it.category !== cat ? it.category : null;
        cat = it.category;
        return (
          <div key={it.id}>
            {head && <div className="st-mb-cat">{head}</div>}
            <div className="st-mb-snippet">
              <button
                type="button"
                draggable
                onDragStart={(e) => {
                  e.dataTransfer.setData(DRAG_LIB, it.id);
                  e.dataTransfer.effectAllowed = "copy";
                }}
                className="st-mb-item"
                onDoubleClick={() => add(it)}
                onContextMenu={(e) =>
                  cm.open(e, it.name, [
                    { label: "Add to this mode", run: () => add(it) },
                    { label: "Export as a file", run: () => exportItem(it) },
                    "-",
                    { label: "Delete from my library…", danger: true, run: () => void dropItem(it) },
                  ])
                }
                onKeyDown={(e) => {
                  if (e.key === "Delete" || e.key === "Backspace") {
                    e.preventDefault();
                    void dropItem(it);
                  }
                }}
                onMouseUp={(e) => e.button === 0 && e.currentTarget.focus({ preventScroll: true })}
                title={`${it.description || it.name} · version ${it.itemVersion} (double-click or drag it in)`}
              >
                <span className={`st-mb-dot ${it.kind === "function" ? "fn" : it.kind === "macro" ? "mac" : it.kind === "struct" ? "st" : "en"}`} /> {it.name}
                <span className="st-muted st-mb-note">v{it.itemVersion}</span>
              </button>
              <button type="button" className="st-mb-add" title="Export as a file" aria-label={`Export ${it.name}`} onClick={() => exportItem(it)}>
                ⤓
              </button>
              <button
                type="button"
                className="st-mb-add"
                title="Delete from my library"
                aria-label={`Delete ${it.name}`}
                onClick={() => void dropItem(it)}
              >
                ×
              </button>
            </div>
          </div>
        );
      })}
      <div className="st-mb-row">
        <button type="button" className="btn small" onClick={() => file.current?.click()}>
          Import
        </button>
        <button type="button" className="btn small" disabled={!library.length} onClick={exportAll} title="Every item in one file">
          Export all
        </button>
      </div>
      {cm.menu}
      <input ref={file} type="file" accept=".json,application/json" multiple hidden onChange={(e) => void importFiles(e.target.files).then(() => (e.target.value = ""))} />
    </Section>
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
  const cm = useContextMenu();
  const [saving, setSaving] = useState<string | null>(null);
  if (!graph) return null;
  const now = () => useStudio.getState().graph!;
  const findUses = (p: PartRef) => {
    const g = now();
    const u = usesOf(g, p);
    const name = partName(g, p);
    if (!u.nodes.length) return toast(u.others.length ? `“${name}” has no nodes using it; it's in ${u.others.join(", ")}.` : `Nothing uses “${name}” yet.`);
    api?.focus(u.nodes[0].id);
    const home = u.nodes[0].graph ?? "";
    useStudio.getState().select(u.nodes.filter((n) => (n.graph ?? "") === home).map((n) => n.id));
    toast(`“${name}”: ${whereText(g, u)}.`);
  };
  const del = async (p: PartRef) => {
    const g = now();
    const name = partName(g, p);
    if (p.kind === "event") {
      edit((x) => deletePart(x, p));
      return;
    }
    const u = usesOf(g, p);
    const used = u.nodes.length + u.others.length;
    const q = used
      ? `Delete the ${KIND_NAME[p.kind]} “${name}”? It's used by ${whereText(g, u)}; those show a problem until changed.`
      : `Delete the ${KIND_NAME[p.kind]} “${name}”?${p.kind === "fn" || p.kind === "macro" ? " Its graph goes too." : ""}`;
    if (!(await ask(q, { ok: "Delete", danger: true }))) return;
    edit((x) => deletePart(x, p));
    selectPart(null);
    selectVar(null);
  };
  const rename = async (p: PartRef) => {
    const g = now();
    const name = await askText(`Rename the ${KIND_NAME[p.kind]}`, { label: "Name", placeholder: partName(g, p), max: 30 }, { ok: "Rename" });
    const clean = [...(name ?? "").trim()].filter((c) => LABEL_CHARS.test(c)).join("").slice(0, 30);
    if (!clean) return;
    edit((x) => renamePart(x, p, clean));
  };
  const duplicate = (p: PartRef) => {
    const r = duplicatePart(now(), p);
    if (!r) return toast("That can't be copied.", true);
    edit(() => r.graph);
    if (p.kind === "var") selectVar(r.id);
    else selectPart(r.id);
  };
  const dropSnippet = async (s: Snippet) => {
    if (!(await ask(`Delete your snippet “${s.name}”?`, { ok: "Delete", danger: true }))) return;
    const r = await deleteSnippet(s.id);
    if (r.error) toast(r.error, true);
    reloadSnippets();
  };
  /** The same menu for every item of this mode. */
  const itemMenu = (sid: string): (MenuItem | "-")[] => {
    const p = partOf(now(), sid);
    if (!p) return [];
    const libOk = p.kind === "fn" || p.kind === "macro" || p.kind === "struct" || p.kind === "enum";
    if (p.kind === "event")
      return [
        { label: "Find on the graph", run: () => api?.focus(p.id) },
        "-",
        { label: "Delete", keys: "Del", danger: true, run: () => void del(p) },
      ];
    return [
      { label: "Rename…", run: () => void rename(p) },
      { label: "Duplicate", run: () => duplicate(p) },
      { label: "Find uses", run: () => findUses(p) },
      { label: "Save to my library…", disabled: !libOk, help: libOk ? undefined : "Functions, Macros, structs and enums", run: () => setSaving(p.id) },
      "-",
      { label: "Delete…", keys: "Del", danger: true, run: () => void del(p) },
    ];
  };
  /** Right-click, Shift+F10 / the menu key, Delete or Backspace on an item. */
  const itemKeys = (sid: string, label: string) => ({
    // (WebKit doesn't give a clicked button the keyboard: it needs it for Delete)
    onMouseUp: (e: React.MouseEvent<HTMLElement>) => e.button === 0 && e.currentTarget.focus({ preventScroll: true }),
    onContextMenu: (e: React.MouseEvent) => cm.open(e, label, itemMenu(sid)),
    onKeyDown: (e: React.KeyboardEvent<HTMLElement>) => {
      if (isMenuKey(e)) {
        e.preventDefault();
        cm.openAt(e.currentTarget, label, itemMenu(sid));
      } else if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        e.stopPropagation();
        const p = partOf(now(), sid);
        if (p) void del(p);
      }
    },
  });
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
      {...itemKeys(v.id, v.name)}
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
      {...itemKeys(id, name)}
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
      {cm.menu}
      {saving && <SaveToLibrary partId={saving} onDone={() => setSaving(null)} toast={toast} />}
      <Section title="Events" count={events.length} onAdd={() => api?.search("Events")} addTitle="Add an event">
        {events.length === 0 && <p className="st-muted st-mb-empty">Nothing starts this mode yet.</p>}
        {events.map((n) => (
          <button key={n.id} type="button" className={`st-mb-item${selected.includes(n.id) ? " on" : ""}`} onClick={() => api?.focus(n.id)} {...itemKeys(n.id, label(n.id))}>
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
            {...itemKeys(n.id, String(n.params.name ?? n.id))}
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
      <Section
        title="Map markers"
        count={graph.markers?.length ?? 0}
        onAdd={() => {
          const r = addMarker(graph);
          edit(() => r.graph);
          selectPart(`mk:${r.id}`);
        }}
        addTitle="Add a map marker"
        open={!!graph.markers?.length}
      >
        {!graph.markers?.length && <p className="st-muted st-mb-empty">What a map for this mode places (creature spawns, a base, a patrol path). They make the mode's map kit for the map editor.</p>}
        {(graph.markers ?? []).map((m) => (
          <button
            key={m.id}
            type="button"
            draggable
            onDragStart={(e) => {
              e.dataTransfer.setData(DRAG_PART, `marker:${m.id}`);
              e.dataTransfer.effectAllowed = "copy";
            }}
            className={`st-mb-item${part === `mk:${m.id}` ? " on" : ""}`}
            onClick={() => selectPart(`mk:${m.id}`)}
            {...itemKeys(`mk:${m.id}`, m.label)}
            title="Drag onto the canvas for Find Marker"
          >
            <span className="st-mb-dot mk" style={{ background: m.colour || undefined }} /> {m.label}
            <span className="st-muted st-mb-note">{m.type}</span>
          </button>
        ))}
        <button type="button" className={`st-mb-item${part === "mk:" ? " on" : ""}`} onClick={() => selectPart("mk:")}>
          <span className="st-mb-dot mk" /> Map advice
          <span className="st-muted st-mb-note">size, players</span>
        </button>
      </Section>
      <LibrarySection api={api} toast={toast} />
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
              onContextMenu={(e) =>
                cm.open(e, s.name, [
                  { label: "Add to the graph", run: () => api?.snippetAtCenter(s) },
                  ...(s.mine
                    ? (["-", { label: "Delete…", danger: true, run: () => void dropSnippet(s) }] as const)
                    : [{ label: "Delete", disabled: true, help: "Built-in snippets stay", run: () => {} }]),
                ])
              }
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
                onClick={() => void dropSnippet(s)}
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
