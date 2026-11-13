// The nav seen from above on a 2D canvas: one box per area, coloured by its place, with the
// place names written over them. Click an area to pick it, Shift-click to add or take one
// away, drag a box to pick every area whose middle is inside (Shift adds, Alt takes away).
// Drag with the right mouse button (or hold Space) to move, scroll to zoom. Double-click
// picks every area of that place. With a brush (tool "paint" or "erase"), dragging names every
// area the pointer passes over in one step.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { Area } from "./calloutsApi";

type View = { cx: number; cy: number; scale: number };
type Drag = { kind: "box" | "pan" | "paint"; x0: number; y0: number; x1: number; y1: number; view: View; mode: "set" | "add" | "remove"; moved: boolean };
export type Tool = "pick" | "paint" | "erase";

export function TopDown({ areas, colours, selected, onSelect, zRange, fitKey, tool = "pick", brush, onPaint, flash, onHoverArea }: {
  areas: Area[];
  colours: Map<string, string>;
  selected: Set<number>;
  /** the new selection (indexes into areas) */
  onSelect: (s: Set<number>) => void;
  /** only areas with a floor in this range can be picked; the rest are drawn faint */
  zRange: [number, number];
  /** changes when the view should fit the map again */
  fitKey: string;
  /** what a left-drag does: pick areas, or paint the brush's name on them (erase: no name) */
  tool?: Tool;
  /** the brush's colour (tool "paint") */
  brush?: string | null;
  /** a finished stroke: the areas painted (indexes into areas) */
  onPaint?: (painted: number[]) => void;
  /** areas to point out (a problem being shown) */
  flash?: Set<number>;
  onHoverArea?: (a: Area | null) => void;
}) {
  const stroke = useRef<Set<number>>(new Set());
  const wrap = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [size, setSize] = useState({ w: 800, h: 560 });
  const [view, setView] = useState<View | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [hover, setHover] = useState<{ i: number; x: number; y: number } | null>(null);
  const space = useRef(false);

  const bounds = useMemo(() => {
    let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
    for (const a of areas) {
      x0 = Math.min(x0, a.x0);
      y0 = Math.min(y0, a.y0);
      x1 = Math.max(x1, a.x1);
      y1 = Math.max(y1, a.y1);
    }
    return isFinite(x0) ? { x0, y0, x1, y1 } : { x0: -512, y0: -512, x1: 512, y1: 512 };
    // (renaming areas doesn't move them: only a new map or nav changes the bounds)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [fitKey, areas.length]);

  const fit = useCallback(() => {
    const pad = 24;
    const scale = Math.min((size.w - pad * 2) / Math.max(1, bounds.x1 - bounds.x0), (size.h - pad * 2) / Math.max(1, bounds.y1 - bounds.y0));
    setView({ cx: (bounds.x0 + bounds.x1) / 2, cy: (bounds.y0 + bounds.y1) / 2, scale });
  }, [bounds, size.w, size.h]);

  useEffect(() => fit(), [fit]);

  useEffect(() => {
    const el = wrap.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: Math.max(200, el.clientWidth), h: Math.max(200, el.clientHeight) }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.code === "Space" && !(e.target instanceof HTMLInputElement)) space.current = true;
    };
    const up = (e: KeyboardEvent) => {
      if (e.code === "Space") space.current = false;
    };
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
    };
  }, []);

  const v = view ?? { cx: 0, cy: 0, scale: 1 };
  // GoldSrc to screen: +y is up, like the game's overview maps
  const sx = (x: number) => (x - v.cx) * v.scale + size.w / 2;
  const sy = (y: number) => -(y - v.cy) * v.scale + size.h / 2;
  const wx = (px: number) => (px - size.w / 2) / v.scale + v.cx;
  const wy = (py: number) => -(py - size.h / 2) / v.scale + v.cy;
  const pickable = (a: Area) => a.z >= zRange[0] - 0.5 && a.z <= zRange[1] + 0.5;

  // draw order: low floors first, so the higher ones are on top
  const order = useMemo(() => areas.map((_, i) => i).sort((a, b) => areas[a].z - areas[b].z), [areas]);

  /** the top pickable area under a screen point */
  const areaAt = (px: number, py: number) => {
    const x = wx(px);
    const y = wy(py);
    for (let k = order.length - 1; k >= 0; k--) {
      const a = areas[order[k]];
      if (pickable(a) && x >= a.x0 && x <= a.x1 && y >= a.y0 && y <= a.y1) return order[k];
    }
    return -1;
  };

  // ---- drawing
  useEffect(() => {
    const c = canvas.current;
    if (!c || !view) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = Math.round(size.w * dpr);
    c.height = Math.round(size.h * dpr);
    const g = c.getContext("2d");
    if (!g) return;
    g.setTransform(dpr, 0, 0, dpr, 0, 0);
    g.fillStyle = "#23271e";
    g.fillRect(0, 0, size.w, size.h);
    for (const i of order) {
      const a = areas[i];
      const on = pickable(a);
      const x = sx(a.x0);
      const y = sy(a.y1);
      const w = Math.max(1, (a.x1 - a.x0) * v.scale);
      const h = Math.max(1, (a.y1 - a.y0) * v.scale);
      const painting = drag?.kind === "paint" && stroke.current.has(i);
      const col = painting ? (tool === "erase" ? null : brush ?? null) : a.place ? colours.get(a.place) ?? "#999" : null;
      g.globalAlpha = on ? 1 : 0.12;
      g.fillStyle = col ?? "#5b6152";
      g.globalAlpha *= col ? 0.62 : 0.5;
      g.fillRect(x, y, w, h);
      g.globalAlpha = on ? 1 : 0.12;
      g.strokeStyle = "rgba(0,0,0,.55)";
      g.lineWidth = 1;
      if (w > 3 && h > 3) g.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
      if (flash?.has(i)) {
        g.globalAlpha = 1;
        g.strokeStyle = "#ff7a5c";
        g.lineWidth = 2;
        g.strokeRect(x + 1, y + 1, Math.max(1, w - 2), Math.max(1, h - 2));
      }
      if (selected.has(i)) {
        g.globalAlpha = 1;
        g.strokeStyle = "#fff6c0";
        g.lineWidth = 2;
        g.strokeRect(x + 1, y + 1, Math.max(1, w - 2), Math.max(1, h - 2));
        g.fillStyle = "rgba(255, 246, 192, .28)";
        g.fillRect(x, y, w, h);
      }
    }
    g.globalAlpha = 1;
    // labels: each place's name at the visible area nearest the middle of its visible areas
    const groups = new Map<string, number[]>();
    for (let i = 0; i < areas.length; i++) {
      const a = areas[i];
      if (!a.place || !pickable(a)) continue;
      const l = groups.get(a.place) ?? [];
      l.push(i);
      groups.set(a.place, l);
    }
    g.font = "600 13px 'Barlow Semi Condensed', 'Arial Narrow', sans-serif";
    g.textAlign = "center";
    g.textBaseline = "middle";
    for (const [name, list] of groups) {
      let [mx, my, wsum] = [0, 0, 0];
      for (const i of list) {
        const a = areas[i];
        const wgt = (a.x1 - a.x0) * (a.y1 - a.y0) + 1;
        mx += ((a.x0 + a.x1) / 2) * wgt;
        my += ((a.y0 + a.y1) / 2) * wgt;
        wsum += wgt;
      }
      mx /= wsum;
      my /= wsum;
      let best = list[0];
      let bd = Infinity;
      for (const i of list) {
        const a = areas[i];
        const d = ((a.x0 + a.x1) / 2 - mx) ** 2 + ((a.y0 + a.y1) / 2 - my) ** 2;
        if (d < bd) {
          bd = d;
          best = i;
        }
      }
      const a = areas[best];
      const px = sx((a.x0 + a.x1) / 2);
      const py = sy((a.y0 + a.y1) / 2);
      const tw = g.measureText(name).width;
      g.fillStyle = "rgba(20, 22, 16, .78)";
      g.fillRect(px - tw / 2 - 4, py - 9, tw + 8, 18);
      g.fillStyle = colours.get(name) ?? "#fff";
      g.fillText(name, px, py + 1);
    }
    if (drag?.kind === "box" && drag.moved) {
      g.strokeStyle = "#fff6c0";
      g.setLineDash([5, 4]);
      g.lineWidth = 1;
      g.strokeRect(Math.min(drag.x0, drag.x1) + 0.5, Math.min(drag.y0, drag.y1) + 0.5, Math.abs(drag.x1 - drag.x0), Math.abs(drag.y1 - drag.y0));
      g.setLineDash([]);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [areas, colours, selected, view, size, drag, zRange, order, flash, brush, tool]);

  // ---- the mouse
  const point = (e: React.MouseEvent | MouseEvent) => {
    const r = canvas.current!.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  };

  const onDown = (e: React.MouseEvent) => {
    if (!view) return;
    const p = point(e);
    const pan = e.button === 2 || e.button === 1 || space.current;
    const mode = e.altKey ? "remove" : e.shiftKey || e.metaKey || e.ctrlKey ? "add" : "set";
    const paint = !pan && tool !== "pick";
    if (paint) {
      stroke.current = new Set();
      const i = areaAt(p.x, p.y);
      if (i >= 0) stroke.current.add(i);
    }
    setDrag({ kind: pan ? "pan" : paint ? "paint" : "box", x0: p.x, y0: p.y, x1: p.x, y1: p.y, view, mode, moved: false });
  };

  useEffect(() => {
    if (!drag) return;
    const move = (e: MouseEvent) => {
      const p = point(e);
      const moved = drag.moved || Math.abs(p.x - drag.x0) + Math.abs(p.y - drag.y0) > 4;
      if (drag.kind === "pan") {
        setView({ ...drag.view, cx: drag.view.cx - (p.x - drag.x0) / drag.view.scale, cy: drag.view.cy + (p.y - drag.y0) / drag.view.scale });
      }
      if (drag.kind === "paint") {
        // every area along the way since the last move, so a quick stroke leaves no gaps
        const steps = Math.max(1, Math.ceil(Math.hypot(p.x - drag.x1, p.y - drag.y1) / 4));
        for (let k = 1; k <= steps; k++) {
          const i = areaAt(drag.x1 + ((p.x - drag.x1) * k) / steps, drag.y1 + ((p.y - drag.y1) * k) / steps);
          if (i >= 0) stroke.current.add(i);
        }
      }
      setDrag({ ...drag, x1: p.x, y1: p.y, moved });
    };
    const up = (e: MouseEvent) => {
      const p = point(e);
      if (drag.kind === "paint") {
        const list = [...stroke.current];
        stroke.current = new Set();
        if (list.length) onPaint?.(list);
      }
      if (drag.kind === "box") {
        const next = drag.mode === "set" ? new Set<number>() : new Set(selected);
        if (drag.moved) {
          const [ax, bx] = [wx(Math.min(drag.x0, p.x)), wx(Math.max(drag.x0, p.x))];
          const [ay, by] = [wy(Math.max(drag.y0, p.y)), wy(Math.min(drag.y0, p.y))];
          areas.forEach((a, i) => {
            const mx = (a.x0 + a.x1) / 2;
            const my = (a.y0 + a.y1) / 2;
            if (pickable(a) && mx >= ax && mx <= bx && my >= ay && my <= by) {
              if (drag.mode === "remove") next.delete(i);
              else next.add(i);
            }
          });
        } else {
          const i = areaAt(p.x, p.y);
          if (i >= 0) {
            if (drag.mode === "set") next.add(i);
            else if (drag.mode === "remove" || next.has(i)) next.delete(i);
            else next.add(i);
          }
        }
        onSelect(next);
      }
      setDrag(null);
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
    return () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [drag, selected, areas, zRange]);

  // wheel zoom at the cursor (a native listener: React's is passive)
  useEffect(() => {
    const c = canvas.current;
    if (!c) return;
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      setView((old) => {
        if (!old) return old;
        const r = c.getBoundingClientRect();
        const px = e.clientX - r.left;
        const py = e.clientY - r.top;
        const k = Math.exp(-e.deltaY * 0.0015);
        const scale = Math.min(40, Math.max(0.005, old.scale * k));
        const x = (px - size.w / 2) / old.scale + old.cx;
        const y = -(py - size.h / 2) / old.scale + old.cy;
        return { scale, cx: x - (px - size.w / 2) / scale, cy: y + (py - size.h / 2) / scale };
      });
    };
    c.addEventListener("wheel", wheel, { passive: false });
    return () => c.removeEventListener("wheel", wheel);
  }, [size.w, size.h]);

  const onDouble = (e: React.MouseEvent) => {
    if (tool !== "pick") return;
    const p = point(e);
    const i = areaAt(p.x, p.y);
    if (i < 0) return;
    const place = areas[i].place;
    const next = e.shiftKey ? new Set(selected) : new Set<number>();
    areas.forEach((a, j) => {
      if (pickable(a) && (place ? a.place === place : j === i)) next.add(j);
    });
    onSelect(next);
  };

  const onHover = (e: React.MouseEvent) => {
    if (drag) return;
    const p = point(e);
    const i = areaAt(p.x, p.y);
    setHover(i >= 0 ? { i, x: p.x, y: p.y } : null);
    onHoverArea?.(i >= 0 ? areas[i] : null);
  };

  const zoom = (k: number) => setView((o) => (o ? { ...o, scale: Math.min(40, Math.max(0.005, o.scale * k)) } : o));
  const h = hover ? areas[hover.i] : null;

  return (
    <div className="co-canvas" ref={wrap}>
      <canvas
        ref={canvas}
        style={{ width: size.w, height: size.h, cursor: drag?.kind === "pan" ? "grabbing" : tool === "pick" ? "default" : "crosshair" }}
        role="img"
        aria-label="The bot nav seen from above, colored by place"
        onMouseDown={onDown}
        onMouseMove={onHover}
        onMouseLeave={() => (setHover(null), onHoverArea?.(null))}
        onDoubleClick={onDouble}
        onContextMenu={(e) => e.preventDefault()}
      />
      {h && hover && (
        <div className="co-tip" style={{ left: Math.min(hover.x + 14, size.w - 180), top: hover.y + 14 }}>
          {h.place || <span className="muted">No name</span>}
          <span className="muted"> · area {h.id} · height {Math.round(h.z)}</span>
        </div>
      )}
      <div className="co-zoom">
        <button className="btn small" onClick={() => zoom(1.3)} aria-label="Zoom in">+</button>
        <button className="btn small" onClick={() => zoom(1 / 1.3)} aria-label="Zoom out">−</button>
        <button className="btn small" onClick={fit}>Fit</button>
      </div>
    </div>
  );
}
