// P5: the panes' DOM side: one box per pane (the 3D canvas draws into each box's place),
// the splitters between them and each pane's small header (what it shows, maximise).

import { useRef, type MutableRefObject } from "react";
import "./p5.css";
import { paneCount, toggleMaximise, usePanes, type LayoutId, type Split } from "./panes";
import { modeFor, RENDER_MODES, setPaneMode, type RenderMode } from "./renderModes";
import { useEffect, useState } from "react";
import { SPEEDS, useNav } from "./navigation";

/** How to move, in the corner of the 3D view until you've flown once (? brings it back). */
function NavHint() {
  const preset = useNav((s) => s.preset);
  const seen = useNav((s) => s.hintDone);
  if (seen) return null;
  const text = preset === "unreal" ? "Hold right mouse + WASD to fly · Alt+drag to orbit · F to frame" : preset === "hammer" ? "Z for mouse-look + WASD · drag to look · right-drag to move" : "Drag to orbit · right-drag to pan · scroll to zoom · F to fly";
  return (
    <div className="ed-nav-hint" role="note">
      {text}
      <button type="button" aria-label="Hide this hint" title="Hide (? shows the keys again)" onClick={() => useNav.getState().set({ hintDone: true })}>
        ×
      </button>
    </div>
  );
}

/** The camera speed note ("Speed 4 · 512 u/s") shown a moment after it changes, and while looking round. */
function NavHud() {
  const until = useNav((s) => s.hudUntil);
  const speed = useNav((s) => s.speed);
  const looking = useNav((s) => s.looking);
  const preset = useNav((s) => s.preset);
  const scale = useNav((s) => s.mapScale);
  const [, tick] = useState(0);
  useEffect(() => {
    const left = until - performance.now();
    if (left <= 0) return;
    const id = setTimeout(() => tick((n) => n + 1), left + 20);
    return () => clearTimeout(id);
  }, [until]);
  if (preset === "blender" || (!looking && performance.now() > until)) return null;
  return (
    <div className="ed-nav-hud" aria-live="polite">
      Speed {speed + 1} <span className="muted">· {Math.round(SPEEDS[speed] * scale)} u/s</span>
      {looking && <span className="muted"> · {preset === "hammer" ? "Z or Esc to stop" : "scroll: speed"}</span>}
    </div>
  );
}
import type { ViewId } from "./store";

export const VIEW_LABELS: Record<ViewId, string> = { "3d": "3D", top: "Top", front: "Front", side: "Side" };

type Rect = [number, number, number, number]; // left, top, width, height (0-1)

/** Where each pane sits, for a layout and its splitters. */
export function paneRects(layout: LayoutId, s: Split): Rect[] {
  const { x, y, y2 } = s;
  switch (layout) {
    case "single":
      return [[0, 0, 1, 1]];
    case "two":
      return [
        [0, 0, x, 1],
        [x, 0, 1 - x, 1],
      ];
    case "quad":
      return [
        [0, 0, x, y],
        [x, 0, 1 - x, y],
        [0, y, x, 1 - y],
        [x, y, 1 - x, 1 - y],
      ];
    case "one3":
      return [
        [0, 0, x, 1],
        [x, 0, 1 - x, y],
        [x, y, 1 - x, y2 - y],
        [x, y2, 1 - x, 1 - y2],
      ];
  }
}

const pct = (n: number) => `${n * 100}%`;

/** The pane boxes (tracked by the canvas), splitters and headers. */
export function PaneBoxes({ tracks }: { tracks: MutableRefObject<HTMLDivElement | null>[] }) {
  const layout = usePanes((s) => s.layout);
  const views = usePanes((s) => s.views);
  const split = usePanes((s) => s.splits[s.layout]);
  const active = usePanes((s) => s.active);
  const max = usePanes((s) => s.max);
  const modes = usePanes((s) => s.modes);
  const wrap = useRef<HTMLDivElement>(null);
  const rects = paneRects(layout, split);
  const firstThreeD = views.slice(0, paneCount(layout)).indexOf("3d");
  const n = paneCount(layout);
  const set = usePanes.getState().set;

  /** Drag a splitter: which split value it changes, along x or y. */
  const drag = (key: keyof Split) => (e: React.PointerEvent) => {
    e.preventDefault();
    e.stopPropagation();
    const box = wrap.current?.getBoundingClientRect();
    if (!box) return;
    const move = (m: PointerEvent) => {
      const st = usePanes.getState();
      const cur = st.splits[st.layout];
      let v = key === "x" ? (m.clientX - box.left) / box.width : (m.clientY - box.top) / box.height;
      v = Math.max(0.1, Math.min(0.9, v));
      // 1+3: the two splits of the small column keep their order
      if (st.layout === "one3" && key === "y") v = Math.min(v, cur.y2 - 0.08);
      if (st.layout === "one3" && key === "y2") v = Math.max(v, cur.y + 0.08);
      st.set({ splits: { ...st.splits, [st.layout]: { ...cur, [key]: v } } });
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  return (
    <div ref={wrap} className="ed-panes-boxes">
      {Array.from({ length: 4 }, (_, i) => {
        const shown = i < n && (max == null || max === i);
        const r: Rect = max === i ? [0, 0, 1, 1] : rects[i] ?? [0, 0, 0, 0];
        return (
          <div
            key={i}
            ref={tracks[i]}
            data-pane={i}
            className={`ed-pane${n > 1 && active === i ? " active" : ""}`}
            style={shown ? { left: pct(r[0]), top: pct(r[1]), width: pct(r[2]), height: pct(r[3]) } : { display: "none" }}
            tabIndex={-1}
            onPointerDownCapture={(e) => {
              if (usePanes.getState().active !== i) set({ active: i });
              // a press in a view takes the keys from any text box (WebKit doesn't move focus on
              // a right-click, so a search box kept the keys and Right mouse + WASD did nothing)
              const el = e.currentTarget as HTMLElement;
              if (document.activeElement !== el && !(e.target as HTMLElement).closest(".ed-pane-head")) el.focus({ preventScroll: true });
            }}
            onPointerEnter={() => usePanes.setState({ hover: i })}
            onPointerLeave={() => usePanes.getState().hover === i && usePanes.setState({ hover: null })}
          >
            {shown && views[i] === "3d" && <NavHud />}
            {shown && views[i] === "3d" && i === firstThreeD && <NavHint />}
            {shown && (
              // one pane: the view is picked in the top bar, so only the drawing style shows here
              <div className="ed-pane-head" onPointerDown={(e) => e.stopPropagation()} onDoubleClick={() => n > 1 && toggleMaximise(i)}>
                {n > 1 && (
                <select
                  value={views[i]}
                  aria-label={`What pane ${i + 1} shows`}
                  onChange={(e) => {
                    const v = [...usePanes.getState().views];
                    v[i] = e.target.value as ViewId;
                    set({ views: v, active: i });
                  }}
                >
                  {(Object.keys(VIEW_LABELS) as ViewId[]).map((v) => (
                    <option key={v} value={v}>
                      {VIEW_LABELS[v]}
                    </option>
                  ))}
                </select>
                )}
                <select
                  className="ed-pane-mode"
                  value={modeFor(modes?.[i], views[i])}
                  aria-label={`How pane ${i + 1} draws`}
                  title="Textured, flat colors, wireframe, or textured with the edges"
                  onChange={(e) => setPaneMode(i, e.target.value as RenderMode)}
                >
                  {RENDER_MODES.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.label}
                    </option>
                  ))}
                </select>
                {n > 1 && (
                <button type="button" className="ed-pane-max" title={max === i ? "Put the panes back (Shift+Space)" : "Fill the space with this pane (Shift+Space)"} aria-label={max === i ? "Restore" : "Maximize"} onClick={() => toggleMaximise(i)}>
                  {max === i ? "▣" : "□"}
                </button>
                )}
              </div>
            )}
          </div>
        );
      })}
      {max == null && layout !== "single" && (
        <>
          <div className="ed-split v" style={{ left: pct(split.x) }} onPointerDown={drag("x")} role="separator" aria-orientation="vertical" aria-label="Resize the panes" />
          {layout === "quad" && <div className="ed-split h" style={{ top: pct(split.y) }} onPointerDown={drag("y")} role="separator" aria-orientation="horizontal" aria-label="Resize the panes" />}
          {layout === "one3" && (
            <>
              <div className="ed-split h" style={{ top: pct(split.y), left: pct(split.x), right: 0 }} onPointerDown={drag("y")} role="separator" aria-orientation="horizontal" aria-label="Resize the panes" />
              <div className="ed-split h" style={{ top: pct(split.y2), left: pct(split.x), right: 0 }} onPointerDown={drag("y2")} role="separator" aria-orientation="horizontal" aria-label="Resize the panes" />
            </>
          )}
        </>
      )}
    </div>
  );
}
