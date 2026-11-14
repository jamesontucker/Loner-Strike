// The node search: right-click the canvas, press Space, or drop a wire on empty space (then it
// only lists nodes that wire can attach to). Besides the registry's nodes it lists the mode's own
// Functions, Macros, structs and enums (Call X, Make X, Switch on X), and only what fits the tab.

import { useEffect, useMemo, useRef, useState } from "react";
import { CATEGORIES, compatible, defaultParams, matchRank, nodeDef, searchNodes, type NodeDef, type PinType } from "./registry";
import { pinsOf, typeLabel } from "./pins";
import type { Graph } from "./schema";

export type PaletteAt = {
  /** screen position, relative to the canvas box */
  left: number;
  top: number;
  /** where a new node goes (graph units) */
  flow: { x: number; y: number };
  /** dropped from a wire: its type and which way it points */
  wire?: { type: PinType; dir: "in" | "out"; node: string; pin: string };
  /** only this category (the left panel's + buttons) */
  only?: string;
};

/** One line of the palette: a node with maybe some settings already made. */
export type PaletteItem = { def: NodeDef; title: string; help: string; category: string; params?: Record<string, string | number | boolean> };

const LATENT = ["timer.delay", "timer.retrigger", "timer.repeat", "timer.countdown", "msg.menu"];
/** made with their Function or Macro, or listed per Function (Call X) */
const HIDDEN = ["fn.entry", "macro.entry", "fn.call", "fn.call_pure", "macro.use"];

/** What can go on this tab. */
function fitsTab(g: Graph, tab: string, d: NodeDef): boolean {
  if (HIDDEN.includes(d.type)) return false;
  const inFn = (g.functions ?? []).some((f) => f.id === tab);
  const inMacro = (g.macros ?? []).some((m) => m.id === tab);
  if (d.type === "fn.return") return inFn;
  if (d.type === "macro.exit") return inMacro;
  if (d.event) return !tab || (g.eventGraphs ?? []).some((e) => e.id === tab);
  if (inFn && LATENT.includes(d.type)) return false;
  return true;
}

/** The mode's own: Call each Function, each Macro, Make and Break each struct, each enum. */
function ownItems(g: Graph, tab: string): PaletteItem[] {
  const out: PaletteItem[] = [];
  const add = (type: string, title: string, help: string, category: string, params: Record<string, string | number | boolean>) => {
    const def = nodeDef(type);
    if (def) out.push({ def, title, help, category, params });
  };
  for (const f of g.functions ?? []) {
    if (f.id === tab) continue;
    add(f.pure ? "fn.call_pure" : "fn.call", f.name, `Call this mode's Function${f.pure ? " (pure)" : ""}.`, "Functions", { func: f.id });
  }
  for (const m of g.macros ?? []) {
    if (m.id === tab) continue;
    add("macro.use", m.name, "Use this mode's Macro (copied in when the mode is made).", "Macros", { macro: m.id });
  }
  for (const t of g.timelines ?? []) {
    add("timeline.play", `Timeline: ${t.name}`, `Plays ${t.name}: its tracks' values every tick, its events at their moments.`, "Timelines", { timeline: t.id });
  }
  for (const c of g.customEvents ?? []) {
    add("event.custom", `Custom Event: ${c.name}`, `Runs when ${c.name} is called (or the map fires it).`, "Custom events", { event: c.id });
    add("event.call", `Call ${c.name}`, `Runs ${c.name} now${c.inputs.length ? " with these values" : ""}.`, "Custom events", { event: c.id });
    if (!c.inputs.length) add("timer.by_event", `Set Timer by ${c.name}`, `Runs ${c.name} after some seconds (again and again if Looping).`, "Custom events", { event: c.id });
  }
  for (const s of g.structs ?? []) {
    add("struct.make", `Make ${s.name}`, `A ${s.name} from its fields.`, "Structs", { struct: s.id });
    add("struct.break", `Break ${s.name}`, `A ${s.name}'s fields.`, "Structs", { struct: s.id });
  }
  for (const e of g.enums ?? []) {
    add("flow.switch_enum", `Switch on ${e.name}`, `Goes to the output for the ${e.name}.`, "Enums", { enum: e.id });
    e.values.forEach((v, i) => add("enum.value", `${e.name}: ${v}`, `The ${e.name} value ${v}.`, "Enums", { enum: e.id, value: i }));
  }
  return out;
}

/** Does an item have a pin a wire of this type can attach to? */
function itemFits(g: Graph, it: PaletteItem, wire: { type: PinType; dir: "in" | "out" }, k: number): boolean {
  const node = { id: `__palette${k}`, type: it.def.type, pos: { x: 0, y: 0 }, params: { ...defaultParams(it.def), ...it.params } };
  const pins = pinsOf(g, node);
  const list = wire.dir === "out" ? pins.inputs : pins.outputs;
  return list.some((p) => (wire.dir === "out" ? compatible(wire.type, p.type) : compatible(p.type, wire.type)) && (p.type === "exec") === (wire.type === "exec"));
}

const FAVS = "ls.studio.palette.favs";
const RECENT = "ls.studio.palette.recent";
function loadList(key: string): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(key) ?? "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x === "string").slice(0, 60) : [];
  } catch {
    return [];
  }
}
function saveList(key: string, v: string[]) {
  try {
    localStorage.setItem(key, JSON.stringify(v));
  } catch {
    /* private window */
  }
}
/** An item's identity (its node type and the settings it comes with). */
export const itemKey = (it: PaletteItem) => `${it.def.type}:${JSON.stringify(it.params ?? {})}`;

/** A big category splits into sub-folders: Events, Values (no flow), Actions. */
function subOf(it: PaletteItem, big: boolean): string {
  if (!big) return "";
  if (it.def.event) return "Events";
  if (it.def.pure) return "Values";
  return "Actions";
}

type Row = { kind: "folder"; id: string; label: string; depth: number; open: boolean; count: number } | { kind: "item"; it: PaletteItem; depth: number; i: number; folder: string };

export function Palette({
  at,
  graph,
  tab,
  onPick,
  onComment,
  onCollapse,
  onClose,
}: {
  at: PaletteAt;
  graph: Graph;
  tab: string;
  onPick: (item: PaletteItem) => void;
  onComment?: () => void;
  onCollapse?: (kind: "function" | "macro") => void;
  onClose: () => void;
}) {
  const [q, setQ] = useState("");
  // (null: the default place, worked out with the rows, so Enter right after typing places the
  // best match: an effect would come a frame late)
  const [curSet, setCur] = useState<number | null>(null);
  const [context, setContext] = useState(true);
  const [favs, setFavs] = useState(() => loadList(FAVS));
  const recent = useMemo(() => loadList(RECENT), []);
  const [opened, setOpened] = useState<Record<string, boolean>>({});
  const wire = context ? at.wire : undefined;
  const list = useMemo(() => {
    const words = q.trim().toLowerCase();
    const reg: PaletteItem[] = searchNodes(q, wire && { type: wire.type, dir: wire.dir })
      .filter((d) => fitsTab(graph, tab, d))
      .map((d) => ({ def: d, title: d.title, help: d.help, category: d.category }));
    const own = ownItems(graph, tab).filter((it, k) => (!words || `${it.title} ${it.category}`.toLowerCase().includes(words)) && (!wire || itemFits(graph, it, wire, k)));
    const all = [...own, ...reg].filter((it) => !at.only || it.category === at.only);
    // searching: the best matches first (their folder goes first too, so Enter places the top
    // one); otherwise by category. (stable: the search's own order otherwise)
    const rank = (it: PaletteItem) => (words ? matchRank(it.title, words, it.def.keywords) : 0);
    return all
      .map((it, k) => ({ it, k, r: rank(it) }))
      .sort((a, b) => a.r - b.r || CATEGORIES.indexOf(a.it.category) - CATEGORIES.indexOf(b.it.category) || a.k - b.k)
      .map((x) => x.it);
  }, [q, wire, at.only, graph, tab]);
  const searching = q.trim() !== "";
  // folders: open while searching (those with matches), when the list is short, or when opened
  const rows = useMemo(() => {
    const out: Row[] = [];
    let i = 0;
    const byKey = new Map(list.map((it) => [itemKey(it), it]));
    const openAll = searching || list.length <= 40 || !!at.only;
    const isOpen = (id: string, dflt: boolean) => opened[id] ?? dflt;
    const section = (id: string, label: string, items: PaletteItem[], dflt: boolean) => {
      if (!items.length) return;
      const open = isOpen(id, dflt);
      out.push({ kind: "folder", id, label, depth: 0, open, count: items.length });
      if (open) for (const it of items) out.push({ kind: "item", it, depth: 1, i: i++, folder: id });
    };
    if (!searching && !at.only) {
      section("fav", "Favorites", favs.map((k) => byKey.get(k)).filter((x): x is PaletteItem => !!x), true);
      section("recent", "Recently used", recent.map((k) => byKey.get(k)).filter((x): x is PaletteItem => !!x).slice(0, 10), true);
    }
    // (searching: folders in the order of their best match; else the usual order)
    const cats: string[] = [];
    for (const it of list) if (!cats.includes(it.category)) cats.push(it.category);
    if (!searching) cats.sort((a, b) => CATEGORIES.indexOf(a) - CATEGORIES.indexOf(b));
    for (const c of cats) {
      const items = list.filter((it) => it.category === c);
      const id = `cat:${c}`;
      const open = isOpen(id, openAll);
      out.push({ kind: "folder", id, label: c, depth: 0, open, count: items.length });
      if (!open) continue;
      const big = items.length > 24;
      const subs: string[] = [];
      for (const it of items) if (!subs.includes(subOf(it, big))) subs.push(subOf(it, big));
      for (const sub of subs) {
        const inSub = items.filter((it) => subOf(it, big) === sub);
        if (!sub) {
          for (const it of inSub) out.push({ kind: "item", it, depth: 1, i: i++, folder: id });
          continue;
        }
        const sid = `${id}/${sub}`;
        const sopen = isOpen(sid, openAll);
        out.push({ kind: "folder", id: sid, label: sub, depth: 1, open: sopen, count: inSub.length });
        if (sopen) for (const it of inSub) out.push({ kind: "item", it, depth: 2, i: i++, folder: sid });
      }
    }
    return out;
  }, [list, favs, recent, opened, searching, at.only]);
  const box = useRef<HTMLDivElement>(null);
  // searching: the cursor on the best match (Enter places it), not its folder
  const cur = curSet ?? (searching ? Math.max(0, rows.findIndex((r) => r.kind === "item")) : 0);
  useEffect(() => {
    const away = (e: PointerEvent) => {
      if (box.current && !box.current.contains(e.target as Node)) onClose();
    };
    window.addEventListener("pointerdown", away, true);
    return () => window.removeEventListener("pointerdown", away, true);
  }, [onClose]);
  useEffect(() => {
    box.current?.querySelector<HTMLElement>(`[data-row="${cur}"]`)?.scrollIntoView({ block: "nearest" });
  }, [cur]);
  const toggle = (id: string, open?: boolean) => setOpened((o) => ({ ...o, [id]: open ?? !(rows.find((r) => r.kind === "folder" && r.id === id) as { open: boolean } | undefined)?.open }));
  const pick = (it: PaletteItem) => {
    const k = itemKey(it);
    saveList(RECENT, [k, ...recent.filter((x) => x !== k)].slice(0, 10));
    onPick(it);
  };
  const star = (it: PaletteItem) => {
    const k = itemKey(it);
    const next = favs.includes(k) ? favs.filter((x) => x !== k) : [...favs, k];
    setFavs(next);
    saveList(FAVS, next);
  };
  return (
    <div className="st-palette" ref={box} style={{ left: at.left, top: at.top }} role="dialog" aria-label="Add a node">
      <div className="st-palette-head">
        <input
          className="st-palette-search"
          autoFocus
          placeholder={wire ? `Nodes for this ${typeLabel(graph, wire.type)} wire…` : "Search all actions…"}
          value={q}
          onChange={(e) => (setQ(e.target.value), setCur(null))}
          onKeyDown={(e) => {
            e.stopPropagation();
            const row = rows[cur];
            if (e.key === "Escape") onClose();
            else if (e.key === "ArrowDown") {
              e.preventDefault();
              setCur(Math.min(cur + 1, rows.length - 1));
            } else if (e.key === "ArrowUp") {
              e.preventDefault();
              setCur(Math.max(cur - 1, 0));
            } else if (e.key === "ArrowRight" && row?.kind === "folder" && !row.open) {
              e.preventDefault();
              toggle(row.id, true);
            } else if (e.key === "ArrowLeft" && row) {
              const fid = row.kind === "folder" ? (row.open ? row.id : null) : row.folder;
              if (fid) {
                e.preventDefault();
                toggle(fid, false);
                const at2 = rows.findIndex((r) => r.kind === "folder" && r.id === fid);
                if (at2 >= 0) setCur(at2);
              }
            } else if (e.key === "Enter" && row) {
              e.preventDefault();
              if (row.kind === "item") pick(row.it);
              else toggle(row.id);
            }
          }}
        />
        {at.wire && (
          <label className="st-palette-context" title="Only nodes that can attach to the wire you're dragging">
            <input type="checkbox" checked={context} onChange={(e) => (setContext(e.target.checked), setCur(null))} /> Context Sensitive
          </label>
        )}
      </div>
      <div className="st-palette-list" role="tree">
        {rows.length === 0 && <div className="st-palette-none">No node matches{wire ? " this wire" : ""}.</div>}
        {rows.map((r, n) =>
          r.kind === "folder" ? (
            <button
              key={r.id}
              type="button"
              role="treeitem"
              aria-expanded={r.open}
              data-row={n}
              className={`st-palette-folder${n === cur ? " cur" : ""}`}
              style={{ paddingLeft: 8 + r.depth * 14 }}
              onMouseEnter={() => setCur(n)}
              onClick={() => toggle(r.id)}
            >
              <span className="st-palette-caret">{r.open ? "▾" : "▸"}</span> {r.label} <span className="st-palette-count">{r.count}</span>
            </button>
          ) : (
            <div key={`${r.folder}|${itemKey(r.it)}`} className={`st-palette-row${n === cur ? " cur" : ""}`} role="treeitem" style={{ paddingLeft: 8 + r.depth * 14 }} onMouseEnter={() => setCur(n)}>
              <button type="button" data-row={n} className="st-palette-item" onClick={() => pick(r.it)} title={r.it.help}>
                <span className="st-palette-title">{r.it.title}</span>
                <span className="st-palette-help">{r.it.help}</span>
              </button>
              <button type="button" className={`st-palette-star${favs.includes(itemKey(r.it)) ? " on" : ""}`} onClick={() => star(r.it)} title={favs.includes(itemKey(r.it)) ? "Take out of Favorites" : "Add to Favorites"} aria-label={favs.includes(itemKey(r.it)) ? `Take ${r.it.title} out of Favorites` : `Add ${r.it.title} to Favorites`}>
                {favs.includes(itemKey(r.it)) ? "★" : "☆"}
              </button>
            </div>
          ),
        )}
        {!at.wire && !q && (onComment || onCollapse) && (
          <>
            <div className="st-palette-cat">Selection</div>
            {onComment && (
              <button type="button" className="st-palette-item" onClick={onComment}>
                <span className="st-palette-title">Comment</span>
                <span className="st-palette-help">A box with a note, around the selected nodes (C).</span>
              </button>
            )}
            {onCollapse && (
              <>
                <button type="button" className="st-palette-item" onClick={() => onCollapse("function")}>
                  <span className="st-palette-title">Collapse to Function</span>
                  <span className="st-palette-help">The selected nodes become a Function of their own, called from here.</span>
                </button>
                <button type="button" className="st-palette-item" onClick={() => onCollapse("macro")}>
                  <span className="st-palette-title">Collapse to Macro</span>
                  <span className="st-palette-help">The selected nodes become a Macro (several flows in and out, Delays allowed).</span>
                </button>
              </>
            )}
          </>
        )}
      </div>
    </div>
  );
}
