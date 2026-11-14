// Bookmarks: up to nine views of a big graph to jump between (Ctrl+Shift+1-9 keeps the view,
// Ctrl+1-9 goes back; on a Mac, Cmd). Kept per mode in this browser: a view, not part of the mode.

import { useEffect, useRef, useState } from "react";
import { useReactFlow } from "@xyflow/react";
import { useStudio } from "./store";
import { loadBookmarks, saveBookmarks, type Bookmark } from "./tidyOps";

const MOD = typeof navigator !== "undefined" && /Mac/.test(navigator.platform) ? "⌘" : "Ctrl+";

export function BookmarksMenu() {
  const graph = useStudio((s) => s.graph);
  const rf = useReactFlow();
  const [open, setOpen] = useState(false);
  const [list, setList] = useState<(Bookmark | null)[]>([]);
  const box = useRef<HTMLDivElement>(null);
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
  const set = (i: number) => {
    const vp = rf.getViewport();
    const next = [...list];
    next[i] = { tab: useStudio.getState().tab, x: vp.x, y: vp.y, zoom: vp.zoom, name: list[i]?.name || `Bookmark ${i + 1}` };
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
  return (
    <div className="st-help-menu" ref={box}>
      <button type="button" className="btn" aria-haspopup="menu" aria-expanded={open} onClick={() => setOpen(!open)} title={`Bookmarks: ${MOD}Shift+1-9 keeps this view, ${MOD}1-9 jumps back`} data-testid="studio-bookmarks">
        Bookmarks
      </button>
      {open && (
        <div className="st-help-menu-list st-bookmarks" role="menu" aria-label="Bookmarks">
          {list.map((b, i) => (
            <div key={i} className="st-bookmark-row">
              {b ? (
                <button type="button" role="menuitem" onClick={() => go(b)}>
                  {b.name} <span className="st-muted">· {tabName(b.tab)}</span> <kbd>{MOD}{i + 1}</kbd>
                </button>
              ) : (
                <button type="button" role="menuitem" onClick={() => set(i)}>
                  <span className="st-muted">Keep this view as {i + 1}</span> <kbd>{MOD}Shift+{i + 1}</kbd>
                </button>
              )}
              {b && (
                <button type="button" className="st-bookmark-x" aria-label={`Clear bookmark ${i + 1}`} title="Clear it" onClick={() => clear(i)}>
                  ×
                </button>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
