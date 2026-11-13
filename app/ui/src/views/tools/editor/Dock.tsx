// P5: the dock. Panels (dockPanels.tsx) sit in areas — left, right (with an optional lower
// part), bottom — as tabs; drag a tab to another area, onto the view to float it, or use the
// ⋯ menu (float, own window, move, close). Areas resize from their inner edge; floats move by
// their title bar and resize from the corner. "Own window" opens a real browser/app window
// rendered from here (same state, so picks and edits stay in step); when a window can't open
// the panel floats instead.

import { useEffect, useRef, useState, type CSSProperties, type DragEvent, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { useEditor } from "./store";
import { AREAS, closePanel, dockTo, floatPanel, panelWindows, popOut, useDock, type AreaId, type Float } from "./dockState";
import { DOCK_PANELS, panelById } from "./dockPanels";

const MIME = "application/x-ls-panel";

/** The editor's grid columns for the open side areas (MapEditor's `editor-main`). */
export function useDockColumns(): CSSProperties {
  const left = useDock((s) => s.areas.left.tabs.length > 0);
  const right = useDock((s) => s.areas.right.tabs.length + s.areas.right2.tabs.length > 0);
  const lw = useDock((s) => s.sizes.left);
  const rw = useDock((s) => s.sizes.right);
  return { gridTemplateColumns: `auto ${left ? `${lw}px ` : ""}minmax(0, 1fr)${right ? ` ${rw}px` : ""}` };
}

function Tabs({ area }: { area: AreaId }) {
  const a = useDock((s) => s.areas[area]);
  useEditor((s) => s.checks); // the Checks tab's count
  const [menu, setMenu] = useState(false);
  const [over, setOver] = useState<string | null>(null);
  const set = useDock.getState().set;
  const drop = (ev: DragEvent, before?: string) => {
    const id = ev.dataTransfer.getData(MIME);
    setOver(null);
    if (!id) return;
    ev.preventDefault();
    ev.stopPropagation();
    if (id !== before) dockTo(id, area, before);
    useDock.getState().set({ dragging: null });
  };
  return (
    <div
      className={`ed-dock-tabs${over === "" ? " drop" : ""}`}
      role="tablist"
      onDragOver={(ev) => ev.dataTransfer.types.includes(MIME) && (ev.preventDefault(), setOver(""))}
      onDragLeave={() => setOver(null)}
      onDrop={(ev) => drop(ev)}
    >
      {a.tabs.map((id) => {
        const p = panelById(id);
        if (!p) return null;
        return (
          <button
            key={id}
            type="button"
            role="tab"
            draggable
            aria-selected={a.active === id}
            aria-pressed={a.active === id}
            className={`${p.bad?.() ? "editor-tab-bad" : ""}${over === id ? " drop-before" : ""}`}
            title="Drag to another side, or onto the view to float it"
            onClick={() => set({ areas: { ...useDock.getState().areas, [area]: { ...a, active: id } } })}
            onDragStart={(ev) => {
              ev.dataTransfer.setData(MIME, id);
              ev.dataTransfer.effectAllowed = "move";
              // (the zones show a moment later: changing the page during dragstart cancels the drag)
              setTimeout(() => useDock.getState().set({ dragging: id }), 0);
            }}
            onDragEnd={(ev) => {
              const was = useDock.getState().dragging;
              useDock.getState().set({ dragging: null });
              // dropped where nothing took it (over the view): float it there
              if (was && ev.dataTransfer.dropEffect === "none" && ev.clientX > 0) floatPanel(was, ev.clientX - 40, ev.clientY - 12);
            }}
            onDragOver={(ev) => ev.dataTransfer.types.includes(MIME) && (ev.preventDefault(), ev.stopPropagation(), setOver(id))}
            onDrop={(ev) => drop(ev, id)}
          >
            {p.label?.() ?? p.title}
          </button>
        );
      })}
      <span className="push" />
      {a.active && (
        <div className="ed-dock-menu">
          <button type="button" className="ed-dock-more" aria-label="Panel options" aria-expanded={menu} title="Float, own window, move, close" onClick={() => setMenu(!menu)}>
            ⋯
          </button>
          {menu && <PanelMenu id={a.active} from={area} onDone={() => setMenu(false)} />}
        </div>
      )}
    </div>
  );
}

function PanelMenu({ id, from, onDone }: { id: string; from: AreaId | "float"; onDone: () => void }) {
  const ref = useRef<HTMLUListElement>(null);
  useEffect(() => {
    const away = (e: PointerEvent) => !ref.current?.contains(e.target as Node) && onDone();
    window.addEventListener("pointerdown", away, true);
    return () => window.removeEventListener("pointerdown", away, true);
  }, [onDone]);
  const item = (label: string, f: () => void) => (
    <li key={label}>
      <button
        type="button"
        onClick={() => {
          onDone();
          f();
        }}
      >
        {label}
      </button>
    </li>
  );
  return (
    <ul className="ed-ctx ed-dock-ctx" ref={ref} role="menu">
      {from !== "float" && item("Float", () => floatPanel(id))}
      {item("Open in its own window", () => popOut(id))}
      {AREAS.filter((a) => a.id !== from).map((a) => item(`Dock: ${a.label}`, () => dockTo(id, a.id)))}
      {item("Close (View > Panels opens it again)", () => closePanel(id))}
    </ul>
  );
}

function Stack({ area, style }: { area: AreaId; style?: CSSProperties }) {
  const a = useDock((s) => s.areas[area]);
  if (!a.tabs.length) return null;
  const p = a.active ? panelById(a.active) : undefined;
  return (
    <div className={`ed-dock-stack ed-dock-${area}`} style={style}>
      <Tabs area={area} />
      <div className="ed-dock-body tools-side">{p?.render()}</div>
    </div>
  );
}

/** Drags an area's inner edge. */
function useResize(key: "left" | "right" | "bottom", dir: 1 | -1) {
  return (e: React.PointerEvent) => {
    e.preventDefault();
    const start = { x: e.clientX, y: e.clientY, v: useDock.getState().sizes[key] };
    const move = (m: PointerEvent) => {
      const d = key === "bottom" ? start.y - m.clientY : (m.clientX - start.x) * dir;
      const lim = key === "bottom" ? [100, Math.max(140, window.innerHeight - 200)] : [200, Math.max(240, window.innerWidth * 0.6)];
      useDock.getState().set({ sizes: { ...useDock.getState().sizes, [key]: Math.round(Math.max(lim[0], Math.min(lim[1], start.v + d))) } });
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };
}

/** A side of the editor: left or right (with its lower part), or the bottom strip under the view. */
export function DockArea({ side }: { side: "left" | "right" | "bottom" }) {
  const has = useDock((s) => (side === "right" ? s.areas.right.tabs.length + s.areas.right2.tabs.length : s.areas[side].tabs.length) > 0);
  const top = useDock((s) => s.areas.right.tabs.length > 0);
  const low = useDock((s) => s.areas.right2.tabs.length > 0);
  const split = useDock((s) => s.sizes.rightSplit);
  const bottomH = useDock((s) => s.sizes.bottom);
  const resize = useResize(side, side === "left" ? 1 : -1);
  const box = useRef<HTMLDivElement>(null);
  if (!has) return null;
  if (side === "bottom") {
    return (
      <div className="ed-dock-area ed-dock-bottom-area" style={{ height: bottomH }}>
        <div className="ed-dock-edge h" onPointerDown={resize} role="separator" aria-orientation="horizontal" aria-label="Resize the bottom panels" />
        <Stack area="bottom" />
      </div>
    );
  }
  const splitDrag = (e: React.PointerEvent) => {
    e.preventDefault();
    const r = box.current?.getBoundingClientRect();
    if (!r) return;
    const move = (m: PointerEvent) => useDock.getState().set({ sizes: { ...useDock.getState().sizes, rightSplit: Math.max(0.15, Math.min(0.85, (m.clientY - r.top) / r.height)) } });
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };
  return (
    <aside ref={box} className={`ed-dock-area ed-dock-side ${side}`}>
      <div className={`ed-dock-edge v ${side}`} onPointerDown={resize} role="separator" aria-orientation="vertical" aria-label={`Resize the ${side} panels`} />
      {side === "left" ? (
        <Stack area="left" />
      ) : (
        <>
          <Stack area="right" style={top && low ? { flex: `0 0 ${split * 100}%` } : undefined} />
          {top && low && <div className="ed-dock-split" onPointerDown={splitDrag} role="separator" aria-orientation="horizontal" aria-label="Resize the two right panels" />}
          <Stack area="right2" />
        </>
      )}
    </aside>
  );
}

/** While a tab is dragged: zones to drop it on (an empty side, or the bottom). */
export function DockDropZones() {
  const dragging = useDock((s) => s.dragging);
  const [over, setOver] = useState<AreaId | null>(null);
  if (!dragging) return null;
  const zone = (id: AreaId, label: string) => (
    <div
      key={id}
      className={`ed-dock-zone ${id}${over === id ? " over" : ""}`}
      onDragOver={(ev) => (ev.preventDefault(), setOver(id))}
      onDragLeave={() => setOver(null)}
      onDrop={(ev) => {
        ev.preventDefault();
        const pid = ev.dataTransfer.getData(MIME);
        setOver(null);
        useDock.getState().set({ dragging: null });
        if (pid) dockTo(pid, id);
      }}
    >
      {label}
    </div>
  );
  return (
    <div className="ed-dock-zones">
      {zone("left", "Dock left")}
      {zone("right", "Dock right")}
      {zone("right2", "Dock right, lower")}
      {zone("bottom", "Dock at the bottom")}
      <div className="ed-dock-zone-hint">Or drop it on the view to float it</div>
    </div>
  );
}

function FloatWin({ f, front }: { f: Float; front: boolean }) {
  const p = panelById(f.id);
  const [menu, setMenu] = useState(false);
  const win = useRef<HTMLDivElement>(null);
  // its size follows the corner handle (CSS resize) and is kept
  useEffect(() => {
    const el = win.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      const w = Math.round(el.offsetWidth);
      const h = Math.round(el.offsetHeight);
      const s = useDock.getState();
      const cur = s.floats.find((x) => x.id === f.id);
      if (cur && (Math.abs(cur.w - w) > 2 || Math.abs(cur.h - h) > 2)) s.set({ floats: s.floats.map((x) => (x.id === f.id ? { ...x, w, h } : x)) });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [f.id]);
  if (!p) return null;
  const drag = (e: React.PointerEvent) => {
    if ((e.target as HTMLElement).closest("button")) return;
    e.preventDefault();
    const start = { x: e.clientX, y: e.clientY, fx: f.x, fy: f.y };
    const s = useDock.getState();
    s.set({ floats: [...s.floats.filter((x) => x.id !== f.id), f] }); // to the front
    const move = (m: PointerEvent) => {
      const st = useDock.getState();
      const x = Math.max(0, Math.min(window.innerWidth - 120, start.fx + m.clientX - start.x));
      const y = Math.max(0, Math.min(window.innerHeight - 40, start.fy + m.clientY - start.y));
      st.set({ floats: st.floats.map((q) => (q.id === f.id ? { ...q, x, y } : q)) });
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };
  return (
    <div ref={win} className={`ed-float${front ? " front" : ""}`} style={{ left: f.x, top: f.y, width: f.w, height: f.h }} role="dialog" aria-label={p.title}>
      <div className="ed-float-title" onPointerDown={drag} onDoubleClick={() => dockTo(f.id, "right")} title="Drag to move. Double-click to dock it on the right.">
        <span>{p.label?.() ?? p.title}</span>
        <span className="push" />
        <div className="ed-dock-menu">
          <button type="button" className="ed-dock-more" aria-label="Panel options" onClick={() => setMenu(!menu)}>
            ⋯
          </button>
          {menu && <PanelMenu id={f.id} from="float" onDone={() => setMenu(false)} />}
        </div>
        <button type="button" className="ed-dock-more" aria-label={`Dock ${p.title}`} title="Dock it on the right" onClick={() => dockTo(f.id, "right")}>
          ⇥
        </button>
      </div>
      <div className="ed-float-body tools-side">{p.render()}</div>
    </div>
  );
}

/** Floating panels and panels in their own windows. */
export function DockFloats() {
  const floats = useDock((s) => s.floats);
  const popped = useDock((s) => s.popped);
  return (
    <>
      {floats.map((f, i) => (
        <FloatWin key={f.id} f={f} front={i === floats.length - 1} />
      ))}
      {popped.map((id) => (
        <PanelWindow key={id} id={id} />
      ))}
    </>
  );
}

/** A panel in a window of its own, drawn from this page (so it shares everything). */
function PanelWindow({ id }: { id: string }) {
  const p = panelById(id);
  const [host, setHost] = useState<HTMLElement | null>(null);
  useEffect(() => {
    let w: Window | null = null;
    try {
      w = window.open("", `ls-editor-${id}`, "popup,width=420,height=720");
    } catch {
      w = null;
    }
    if (!w || !w.document) {
      useEditor.getState().notify("A separate window couldn't open here, so the panel floats inside the editor instead.", true);
      floatPanel(id);
      return;
    }
    const d = w.document;
    d.title = `${p?.title ?? id} · Map editor`;
    d.head.replaceChildren();
    // the page's styles and theme go along
    for (const n of document.querySelectorAll('style, link[rel="stylesheet"]')) d.head.appendChild(n.cloneNode(true));
    d.documentElement.setAttribute("style", document.documentElement.getAttribute("style") ?? "");
    d.documentElement.className = document.documentElement.className;
    d.body.className = `${document.body.className} ed-popped`;
    d.body.replaceChildren();
    const root = d.createElement("div");
    root.className = "ed-popped-root tools-side";
    d.body.appendChild(root);
    panelWindows.set(id, w);
    setHost(root);
    // closing the window docks the panel back on the right
    const timer = window.setInterval(() => {
      if (w?.closed) {
        window.clearInterval(timer);
        panelWindows.delete(id);
        if (useDock.getState().popped.includes(id)) dockTo(id, "right");
      }
    }, 500);
    return () => {
      window.clearInterval(timer);
      panelWindows.delete(id);
      if (w && !w.closed) w.close();
    };
  }, [id]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!host || !p) return null;
  return createPortal(p.render() as ReactNode, host);
}

/** Every panel and where it is, for View > Panels. */
export const allPanels = () => DOCK_PANELS;
