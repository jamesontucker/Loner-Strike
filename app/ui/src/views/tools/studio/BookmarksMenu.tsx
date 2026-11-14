// The View menu: Tidy up, the minimap, and bookmarks: up to nine named views of a big graph to
// jump between (Ctrl+Shift+1-9 keeps the view, Ctrl+1-9 goes back; on a Mac, Cmd). Bookmarks are
// kept per mode in this browser: a view, not part of the mode.

import { useEffect, useRef, useState } from "react";
import { useReactFlow } from "@xyflow/react";
import { useStudio } from "./store";
import { loadBookmarks, saveBookmarks, type Bookmark } from "./tidyOps";
import { usePanels } from "./panels";
import { useAskText } from "../../../ui";

const MOD = typeof navigator !== "undefined" && /Mac/.test(navigator.platform) ? "⌘" : "Ctrl+";

export function ViewMenu({ onTidy }: { onTidy: () => void }) {
  const graph = useStudio((s) => s.graph);
  const rf = useReactFlow();
  const [open, setOpen] = useState(false);
  const [list, setList] = useState<(Bookmark | null)[]>([]);
  const box = useRef<HTMLDivElement>(null);
  const minimap = usePanels((p) => p.minimap);
  const askText = useAskText();
  const uid = graph?.mode.uid || graph?.mode.id || "";
  useEffect(() => {
    if (!open) return;
    setList(loadBookmarks(uid));
    const away = (e: PointerEvent) => box.current && !box.current.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc);
    };
  }, [open, uid]);
  const tabName = (t: string) => (t ? (graph?.functions?.find((f) => f.id === t)?.name ?? graph?.macros?.find((m) => m.id === t)?.name ?? graph?.eventGraphs?.find((e) => e.id === t)?.name ?? "a graph") : "Event Graph");
  const set = async (i: number) => {
    const vp = rf.getViewport();
    setOpen(false);
    const name = await askText(`Name bookmark ${i + 1}`, { label: "Name", placeholder: `Bookmark ${i + 1}`, max: 30 }, { ok: "Keep this view" });
    if (name === null) return;
    const next = [...list];
    next[i] = { tab: useStudio.getState().tab, x: vp.x, y: vp.y, zoom: vp.zoom, name: name.trim().slice(0, 30) || `Bookmark ${i + 1}` };
    saveBookmarks(uid, next);
    setList(next);
  };
  const go = (b: Bookmark) => {
    const other = b.tab !== useStudio.getState().tab;
    if (other) useStudio.getState().openTab(b.tab);
    setTimeout(() => void rf.setViewport({ x: b.x, y: b.y, zoom: b.zoom }, { duration: 250 }), other ? 120 : 0);
    setOpen(false);
  };
  const clear = (i: number) => {
    const next = [...list];
    next[i] = null;
    saveBookmarks(uid, next);
    setList(next);
  };
  const free = list.findIndex((b) => !b);
  return (
    <div className="st-help-menu" ref={box}>
      <button type="button" className="btn" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)} title={`Tidy up, the minimap, and bookmarks (${MOD}Shift+1-9 keeps a view, ${MOD}1-9 jumps back)`} data-testid="studio-view-menu">
        View ▾
      </button>
      {open && (
        <div className="st-help-menu-list st-bookmarks" role="menu" aria-label="View">
          <button type="button" role="menuitem" onClick={() => (setOpen(false), onTidy())}>
            Tidy up <span className="st-muted">nodes left to right</span>
          </button>
          <button type="button" role="menuitemcheckbox" aria-checked={minimap} onClick={() => usePanels.getState().set({ minimap: !minimap })}>
            {minimap ? "✓ " : ""}Show the minimap
          </button>
          <div className="st-menu-head">Bookmarks</div>
          {list.map((b, i) =>
            b ? (
              <div key={i} className="st-bookmark-row">
                <button type="button" role="menuitem" onClick={() => go(b)}>
                  {b.name} <span className="st-muted">· {tabName(b.tab)}</span> <kbd>{MOD}{i + 1}</kbd>
                </button>
                <button type="button" className="st-bookmark-x" aria-label={`Clear bookmark ${i + 1}`} title="Clear it" onClick={() => clear(i)}>
                  ×
                </button>
              </div>
            ) : null,
          )}
          {free >= 0 ? (
            <button type="button" role="menuitem" onClick={() => void set(free)}>
              Keep this view as a bookmark… <kbd>{MOD}Shift+{free + 1}</kbd>
            </button>
          ) : (
            <p className="st-muted st-menu-note">All nine are kept: clear one first.</p>
          )}
        </div>
      )}
    </div>
  );
}
