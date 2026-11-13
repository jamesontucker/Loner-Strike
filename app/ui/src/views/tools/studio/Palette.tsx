// The node search: right-click the canvas, press Space, or drop a wire on empty space (then it
// only lists nodes that wire can attach to).

import { useEffect, useMemo, useRef, useState } from "react";
import { PIN_TYPES, searchNodes, type NodeDef, type PinType } from "./registry";

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

export function Palette({ at, onPick, onComment, onClose }: { at: PaletteAt; onPick: (def: NodeDef) => void; onComment?: () => void; onClose: () => void }) {
  const [q, setQ] = useState("");
  const [cur, setCur] = useState(0);
  const list = useMemo(() => searchNodes(q, at.wire && { type: at.wire.type, dir: at.wire.dir }).filter((d) => !at.only || d.category === at.only), [q, at.wire, at.only]);
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
        placeholder={at.wire ? `Nodes for this ${PIN_TYPES[at.wire.type].label} wire…` : "Search nodes…"}
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
        {list.map((d, i) => {
          const head = d.category !== lastCat ? d.category : null;
          lastCat = d.category;
          return (
            <div key={d.type}>
              {head && <div className="st-palette-cat">{head}</div>}
              <button type="button" data-i={i} className={`st-palette-item${i === cur ? " cur" : ""}`} onMouseEnter={() => setCur(i)} onClick={() => onPick(d)} title={d.help}>
                <span className="st-palette-title">{d.title}</span>
                <span className="st-palette-help">{d.help}</span>
              </button>
            </div>
          );
        })}
        {!at.wire && onComment && !q && (
          <>
            <div className="st-palette-cat">Notes</div>
            <button type="button" className="st-palette-item" onClick={onComment}>
              <span className="st-palette-title">Comment</span>
              <span className="st-palette-help">A box with a note, around the selected nodes (C).</span>
            </button>
          </>
        )}
      </div>
    </div>
  );
}
