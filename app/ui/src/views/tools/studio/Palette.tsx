// The node search: right-click the canvas, press Space, or drop a wire on empty space (then it
// only lists nodes that wire can attach to). Besides the registry's nodes it lists the mode's own
// Functions, Macros, structs and enums (Call X, Make X, Switch on X), and only what fits the tab.

import { useEffect, useMemo, useRef, useState } from "react";
import { CATEGORIES, compatible, defaultParams, nodeDef, searchNodes, type NodeDef, type PinType } from "./registry";
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
  if (d.event) return !tab;
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
  const [cur, setCur] = useState(0);
  const list = useMemo(() => {
    const words = q.trim().toLowerCase();
    const reg: PaletteItem[] = searchNodes(q, at.wire && { type: at.wire.type, dir: at.wire.dir })
      .filter((d) => fitsTab(graph, tab, d))
      .map((d) => ({ def: d, title: d.title, help: d.help, category: d.category }));
    const own = ownItems(graph, tab).filter((it, k) => (!words || `${it.title} ${it.category}`.toLowerCase().includes(words)) && (!at.wire || itemFits(graph, it, at.wire, k)));
    const all = [...own, ...reg].filter((it) => !at.only || it.category === at.only);
    return all.sort((a, b) => CATEGORIES.indexOf(a.category) - CATEGORIES.indexOf(b.category));
  }, [q, at.wire, at.only, graph, tab]);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => setCur(0), [q]);
  useEffect(() => {
    const away = (e: PointerEvent) => {
      if (box.current && !box.current.contains(e.target as Node)) onClose();
    };
    window.addEventListener("pointerdown", away, true);
    return () => window.removeEventListener("pointerdown", away, true);
  }, [onClose]);
  useEffect(() => {
    box.current?.querySelector<HTMLElement>(`[data-i="${cur}"]`)?.scrollIntoView({ block: "nearest" });
  }, [cur]);
  let lastCat = "";
  return (
    <div className="st-palette" ref={box} style={{ left: at.left, top: at.top }} role="dialog" aria-label="Add a node">
      <input
        className="st-palette-search"
        autoFocus
        placeholder={at.wire ? `Nodes for this ${typeLabel(graph, at.wire.type)} wire…` : "Search nodes…"}
        value={q}
        onChange={(e) => setQ(e.target.value)}
        onKeyDown={(e) => {
          e.stopPropagation();
          if (e.key === "Escape") onClose();
          else if (e.key === "ArrowDown") {
            e.preventDefault();
            setCur((c) => Math.min(c + 1, list.length - 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setCur((c) => Math.max(c - 1, 0));
          } else if (e.key === "Enter" && list[cur]) onPick(list[cur]);
        }}
      />
      <div className="st-palette-list">
        {list.length === 0 && <div className="st-palette-none">No node matches{at.wire ? " this wire" : ""}.</div>}
        {list.map((it, i) => {
          const head = it.category !== lastCat ? it.category : null;
          lastCat = it.category;
          return (
            <div key={`${it.def.type}:${JSON.stringify(it.params ?? {})}`}>
              {head && <div className="st-palette-cat">{head}</div>}
              <button type="button" data-i={i} className={`st-palette-item${i === cur ? " cur" : ""}`} onMouseEnter={() => setCur(i)} onClick={() => onPick(it)} title={it.help}>
                <span className="st-palette-title">{it.title}</span>
                <span className="st-palette-help">{it.help}</span>
              </button>
            </div>
          );
        })}
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
