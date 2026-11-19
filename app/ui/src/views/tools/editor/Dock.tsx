// P5: the dock. Panels (dockPanels.tsx) sit in areas — left, right (with an optional lower
// part), bottom — as tabs; drag a tab to another area, onto the view to float it, or use the
// ⋯ menu (float, own window, move, close). Areas resize from their inner edge; floats move by
// their title bar (over a tab strip or a dock target they dock) and resize from the corner. "Own window" opens a real browser/app window
// rendered from here (same state, so picks and edits stay in step); when a window can't open
// the panel floats instead.

import { Component, useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { create } from "zustand";
import { dropChanges, resolveDrop, targetBoxes, type AreaKey, type Box, type Drop, type Geometry } from "./dockDrag";
import { createPortal } from "react-dom";
import { useEditor } from "./store";
import { AREAS, closePanel, dockTo, floatPanel, panelWindows, popOut, useDock, type AreaId, type Float } from "./dockState";
import { DOCK_PANELS, panelById } from "./dockPanels";
import { useCompileRun } from "./compileApi";


/** One panel failing doesn't take the editor down with it: it says so, with a way back. */
class PanelBoundary extends Component<{ title: string; children: ReactNode }, { failed: string | null }> {
  state = { failed: null as string | null };
  static getDerivedStateFromError(e: unknown) {
    return { failed: e instanceof Error ? e.message : String(e) };
  }
  render() {
    if (!this.state.failed) return this.props.children;
    return (
      <section className="tools-panel">
        <p className="small-note">The {this.props.title} panel stopped working.</p>
        <button type="button" className="btn small" onClick={() => this.setState({ failed: null })}>
          Try again
        </button>
      </section>
    );
  }
}

/** The editor's grid columns for the open side areas (MapEditor's `editor-main`). */
export function useDockColumns(): CSSProperties {
  const left = useDock((s) => s.areas.left.tabs.length > 0);
  const right = useDock((s) => s.areas.right.tabs.length + s.areas.right2.tabs.length > 0);
  let lw = useDock((s) => s.sizes.left);
  let rw = useDock((s) => s.sizes.right);
  // the view keeps at least ~380 px: on a narrow window the sides give way (in proportion)
  const [width, setWidth] = useState(() => window.innerWidth);
  useEffect(() => {
    const on = () => setWidth(window.innerWidth);
    window.addEventListener("resize", on);
    return () => window.removeEventListener("resize", on);
  }, []);
  const room = Math.max(0, width - 110 - 380);
  const want = (left ? lw : 0) + (right ? rw : 0);
  if (want > room && want > 0) {
    const k = Math.max(room, 0) / want;
    lw = Math.max(170, Math.round(lw * k));
    rw = Math.max(200, Math.round(rw * k));
  }
  return { gridTemplateColumns: `auto ${left ? `${lw}px ` : ""}minmax(0, 1fr)${right ? ` ${rw}px` : ""}` };
}

/** Folds the bottom panels to their tab strip, or opens them (remembered). */
function FoldButton() {
  const folded = useDock((s) => !!s.bottomFolded);
  return (
    <button
      type="button"
      className="ed-dock-more ed-dock-fold"
      aria-expanded={!folded}
      aria-label={folded ? "Open the bottom panels" : "Fold the bottom panels"}
      title={folded ? "Open the bottom panels" : "Fold the bottom panels to their tabs (the views get the room)"}
      onClick={() => useDock.getState().set({ bottomFolded: !folded })}
    >
      {folded ? "▴" : "▾"}
    </button>
  );
}

function Tabs({ area }: { area: AreaId }) {
  const a = useDock((s) => s.areas[area]);
  useEditor((s) => s.checks); // the Checks tab's count
  useCompileRun((s) => s.job || s.starting || s.playing); // the Compile tab's running dot
  const [menu, setMenu] = useState(false);
  const more = useRef<HTMLButtonElement>(null);
  const set = useDock.getState().set;
  return (
    <div className="ed-dock-tabs" role="tablist" data-dock-strip={area}>
      {a.tabs.map((id) => {
        const p = panelById(id);
        if (!p) return null;
        const busy = p.busy?.() ?? false;
        return (
          <button
            key={id}
            type="button"
            role="tab"
            data-dock-tab={id}
            aria-selected={a.active === id}
            aria-pressed={a.active === id}
            className={p.bad?.() ? "editor-tab-bad" : undefined}
            aria-label={busy ? `${p.label?.() ?? p.title} (running)` : undefined}
            title={`${p.subtitle ? `${p.subtitle}. ` : ""}Drag to move it: along the tabs, to another side, or onto the view to float it`}
            onPointerDown={(ev) => startPanelDrag(ev, id, () => set({ areas: { ...useDock.getState().areas, [area]: { ...useDock.getState().areas[area], active: id } }, ...(area === "bottom" ? { bottomFolded: false } : {}) }))}
            onKeyDown={(ev) => ev.key === "Enter" && set({ areas: { ...useDock.getState().areas, [area]: { ...a, active: id } }, ...(area === "bottom" ? { bottomFolded: false } : {}) })}
          >
            {p.label?.() ?? p.title}
            {busy && <span className="ed-tab-run" aria-hidden="true" />}
          </button>
        );
      })}
      <span className="push" />
      {area === "bottom" && <FoldButton />}
      {a.active && (
        <div className="ed-dock-menu">
          <button ref={more} type="button" className="ed-dock-more" aria-label="Panel options" aria-expanded={menu} title="Float, own window, move, close" onClick={() => setMenu(!menu)}>
            ⋯
          </button>
          {menu && <PanelMenu id={a.active} from={area} anchor={more.current} onDone={() => setMenu(false)} />}
        </div>
      )}
    </div>
  );
}

// ---- dragging panels (pointer events: HTML5 drag-and-drop isn't dependable in app web views) ----

type DragView = { id: string; x: number; y: number; drop: Drop };
const useDrag = create<{ d: DragView | null }>(() => ({ d: null }));

/** The boxes a drop can land on, read from the page right now. */
function geometry(): Geometry {
  const box = (el: Element): Box => {
    const r = el.getBoundingClientRect();
    return { left: r.left, top: r.top, right: r.right, bottom: r.bottom };
  };
  const strips = [...document.querySelectorAll<HTMLElement>("[data-dock-strip]")].map((el) => ({
    area: el.dataset.dockStrip as AreaId,
    box: box(el),
    tabs: [...el.querySelectorAll<HTMLElement>("[data-dock-tab]")].map((t) => ({ id: t.dataset.dockTab!, box: box(t) })),
  }));
  const main = document.querySelector(".editor-main");
  const targets = main ? targetBoxes(box(main)) : [];
  const bodies = [...document.querySelectorAll<HTMLElement>("[data-dock-body]")].map((el) => ({ area: el.dataset.dockBody as AreaId, box: box(el) }));
  const v = document.querySelector(".tools-canvas");
  return { strips, targets, bodies, view: v ? box(v) : null };
}

function whereNow(id: string) {
  const s = useDock.getState();
  for (const a of AREAS) if (s.areas[a.id].tabs.includes(id)) return { area: a.id as AreaKey, tabs: s.areas[a.id].tabs };
  return { area: null, tabs: [] as string[] };
}

/** Press on a tab or a float's title: a click (no movement) runs `onClick`; a drag shows where
 *  the panel would land and puts it there on release (Esc cancels). `float` moves a floating
 *  panel's window while dragging. */
export function startPanelDrag(ev: React.PointerEvent, id: string, onClick?: () => void, float?: { x: number; y: number }) {
  if (ev.button !== 0) return;
  if ((ev.target as HTMLElement).closest(".ed-dock-more")) return;
  const start = { x: ev.clientX, y: ev.clientY };
  let dragging = false;
  const el = ev.currentTarget as HTMLElement;
  try {
    el.setPointerCapture(ev.pointerId);
  } catch {
    /* fine without */
  }
  const finish = (apply: boolean) => {
    window.removeEventListener("pointermove", move, true);
    window.removeEventListener("pointerup", up, true);
    window.removeEventListener("pointercancel", cancel, true);
    window.removeEventListener("keydown", key, true);
    try {
      el.releasePointerCapture(ev.pointerId);
    } catch {
      /* already let go */
    }
    const d = useDrag.getState().d;
    useDrag.setState({ d: null });
    useDock.getState().set({ dragging: null });
    if (!apply) return;
    if (!dragging) return onClick?.();
    const drop = d?.drop ?? null;
    if (!drop || !dropChanges(drop, id, whereNow(id))) return;
    if (drop.kind === "tab") dockTo(id, drop.area, drop.before ?? undefined);
    else if (drop.kind === "area") dockTo(id, drop.area);
    else if (!float) floatPanel(id, drop.x - 60, drop.y - 14);
  };
  const move = (m: PointerEvent) => {
    if (!dragging) {
      if (Math.hypot(m.clientX - start.x, m.clientY - start.y) < 5) return;
      dragging = true;
      useDock.getState().set({ dragging: id });
    }
    m.preventDefault();
    // the boxes as they are now (the targets appear once dragging starts)
    const geo = geometry();
    if (float) {
      const s = useDock.getState();
      const fx = Math.max(0, Math.min(window.innerWidth - 120, float.x + m.clientX - start.x));
      const fy = Math.max(0, Math.min(window.innerHeight - 40, float.y + m.clientY - start.y));
      s.set({ floats: s.floats.map((q) => (q.id === id ? { ...q, x: fx, y: fy } : q)) });
    }
    const drop = resolveDrop(geo, m.clientX, m.clientY, id, !float);
    useDrag.setState({ d: { id, x: m.clientX, y: m.clientY, drop } });
  };
  const up = () => finish(true);
  // the system took the pointer away (a gesture, a window switch): stop, change nothing
  const cancel = () => finish(false);
  const key = (k: KeyboardEvent) => {
    if (k.key !== "Escape") return;
    k.preventDefault();
    k.stopPropagation();
    finish(false);
  };
  window.addEventListener("pointermove", move, true);
  window.addEventListener("pointerup", up, true);
  window.addEventListener("pointercancel", cancel, true);
  window.addEventListener("keydown", key, true);
}

/** A panel's ⋯ menu. In a layer of its own on the page (a portal), placed under its button: the
 *  tab strip scrolls sideways and clipped it to a sliver (it looked like a scrollbar). */
function PanelMenu({ id, from, onDone, anchor }: { id: string; from: AreaId | "float"; onDone: () => void; anchor: HTMLElement | null }) {
  const ref = useRef<HTMLUListElement>(null);
  const r = anchor?.getBoundingClientRect();
  const pos: CSSProperties = r ? { top: Math.min(r.bottom + 2, window.innerHeight - 220), right: Math.max(4, window.innerWidth - r.right), zIndex: 120 } : { top: 80, right: 20, zIndex: 120 };
  useEffect(() => {
    // a press outside closes it (its own ⋯ button toggles it, so not that); Esc closes it too
    const away = (e: PointerEvent) => !ref.current?.contains(e.target as Node) && !anchor?.contains(e.target as Node) && onDone();
    const esc = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      onDone();
      anchor?.focus();
    };
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc, true);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc, true);
    };
  }, [onDone, anchor]);
  const item = (label: string, f: () => void) => (
    <li key={label} role="none">
      <button
        type="button"
        role="menuitem"
        onClick={() => {
          onDone();
          f();
        }}
      >
        {label}
      </button>
    </li>
  );
  return createPortal(
    <ul className="ed-ctx ed-dock-ctx" ref={ref} role="menu" aria-label="Panel options" style={pos}>
      {from !== "float" && item("Float", () => floatPanel(id))}
      {item("Open in its own window", () => popOut(id))}
      {AREAS.filter((a) => a.id !== from).map((a) => item(`Dock: ${a.label}`, () => dockTo(id, a.id)))}
      {item("Close (View > Panels opens it again)", () => closePanel(id))}
    </ul>,
    (ref.current?.ownerDocument ?? anchor?.ownerDocument ?? document).body,
  );
}

function Stack({ area, style, folded }: { area: AreaId; style?: CSSProperties; folded?: boolean }) {
  const a = useDock((s) => s.areas[area]);
  if (!a.tabs.length) return null;
  const p = a.active ? panelById(a.active) : undefined;
  return (
    <div className={`ed-dock-stack ed-dock-${area}`} style={style}>
      <Tabs area={area} />
      {!folded && (
        <div className="ed-dock-body tools-side" data-dock-body={area}>
          {p?.subtitle && <p className="ed-dock-sub">{p.subtitle}</p>}
          {p && <PanelBoundary key={p.id} title={p.title}>{p.render()}</PanelBoundary>}
        </div>
      )}
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
  const folded = useDock((s) => !!s.bottomFolded);
  const resize = useResize(side, side === "left" ? 1 : -1);
  const box = useRef<HTMLDivElement>(null);
  if (!has) return null;
  if (side === "bottom") {
    return (
      <div className={`ed-dock-area ed-dock-bottom-area${folded ? " folded" : ""}`} style={folded ? undefined : { height: bottomH }}>
        {!folded && <div className="ed-dock-edge h" onPointerDown={resize} role="separator" aria-orientation="horizontal" aria-label="Resize the bottom panels" />}
        <Stack area="bottom" folded={folded} />
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

/** While a panel is dragged: small dock targets near the edges, the insertion marker in a
 *  tab strip, and the panel's name under the pointer. */
export function DockDropZones() {
  const dragging = useDock((s) => s.dragging);
  const d = useDrag((s) => s.d);
  const [main, setMain] = useState<Box | null>(null);
  useEffect(() => {
    if (!dragging) return;
    const el = document.querySelector(".editor-main");
    if (!el) return;
    const r = el.getBoundingClientRect();
    setMain({ left: r.left, top: r.top, right: r.right, bottom: r.bottom });
  }, [dragging]);
  if (!dragging || !main) return null;
  const drop = d?.drop ?? null;
  const label: Record<AreaKey, string> = { left: "Dock left", right: "Dock right", right2: "Dock right, lower", bottom: "Dock at the bottom" };
  const p = panelById(dragging);
  return (
    <div className="ed-dock-zones" aria-hidden="true">
      {targetBoxes(main).map((t) => (
        <div key={t.area} className={`ed-dock-target ${t.area}${drop?.kind === "area" && drop.area === t.area ? " over" : ""}`} style={{ left: t.box.left, top: t.box.top, width: t.box.right - t.box.left, height: t.box.bottom - t.box.top }} title={label[t.area]}>
          <TargetIcon area={t.area} />
        </div>
      ))}
      {drop?.kind === "tab" && <div className="ed-dock-marker" style={{ left: drop.markerX - 1, top: drop.markerTop, height: drop.markerBottom - drop.markerTop }} />}
      {d && (
        <div className="ed-dock-ghost" style={{ left: d.x + 12, top: d.y + 10 }}>
          {p?.title}
          <span className="muted">
            {" "}
            {drop == null ? "· let go to cancel" : drop.kind === "float" ? "· float here" : drop.kind === "area" ? `· ${label[drop.area].toLowerCase()}` : "· here"}
          </span>
        </div>
      )}
    </div>
  );
}

function TargetIcon({ area }: { area: AreaKey }) {
  // a little window with the side that would fill
  const fill = area === "left" ? <rect x="3" y="4" width="6" height="16" /> : area === "right" ? <rect x="15" y="4" width="6" height="8" /> : area === "right2" ? <rect x="15" y="12" width="6" height="8" /> : <rect x="3" y="14" width="18" height="6" />;
  return (
    <svg viewBox="0 0 24 24" width="26" height="26" aria-hidden="true">
      <rect x="2.5" y="3.5" width="19" height="17" rx="1.5" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <g fill="currentColor" opacity="0.85">{fill}</g>
    </svg>
  );
}

function FloatWin({ f, front }: { f: Float; front: boolean }) {
  const p = panelById(f.id);
  const [menu, setMenu] = useState(false);
  const more = useRef<HTMLButtonElement>(null);
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
    const s = useDock.getState();
    s.set({ floats: [...s.floats.filter((x) => x.id !== f.id), f] }); // to the front
    // moves the window; let go over a tab strip or a dock target to dock it
    startPanelDrag(e, f.id, undefined, { x: f.x, y: f.y });
  };
  return (
    <div ref={win} className={`ed-float${front ? " front" : ""}`} style={{ left: f.x, top: f.y, width: f.w, height: f.h }} role="dialog" aria-label={p.title}>
      <div className="ed-float-title" onPointerDown={drag} onDoubleClick={() => dockTo(f.id, "right")} title="Drag to move. Double-click to dock it on the right.">
        <span>{p.label?.() ?? p.title}</span>
        <span className="push" />
        <div className="ed-dock-menu">
          <button ref={more} type="button" className="ed-dock-more" aria-label="Panel options" onClick={() => setMenu(!menu)}>
            ⋯
          </button>
          {menu && <PanelMenu id={f.id} from="float" anchor={more.current} onDone={() => setMenu(false)} />}
        </div>
        <button type="button" className="ed-dock-more" aria-label={`Dock ${p.title}`} title="Dock it on the right" onClick={() => dockTo(f.id, "right")}>
          ⇥
        </button>
      </div>
      <div className="ed-float-body tools-side">
        <PanelBoundary title={p.title}>{p.render()}</PanelBoundary>
      </div>
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
  return createPortal(<PanelBoundary title={p.title}>{p.render()}</PanelBoundary>, host);
}

/** Every panel and where it is, for View > Panels. */
export const allPanels = () => DOCK_PANELS;
