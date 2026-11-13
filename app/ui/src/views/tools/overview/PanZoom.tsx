// A picture you can zoom (scroll, +/-) and move (drag), fitted to its box at first. Two panes
// can share one view (side by side, current vs new), so they move together.

import { useEffect, useRef, type ReactNode, type PointerEvent as RPointerEvent } from "react";

/** scale 1 = fitted to the box; x, y in box pixels */
export type View = { s: number; x: number; y: number };
export const FIT: View = { s: 1, x: 0, y: 0 };
const MIN = 1, MAX = 12;

/** An offset on whole device pixels: no half-pixel blur (Windows at 125/150 %, Retina). */
const px = (v: number) => {
  const dpr = typeof window !== "undefined" ? window.devicePixelRatio || 1 : 1;
  return Math.round(v * dpr) / dpr;
};

export function zoomBy(v: View, f: number, px: number, py: number): View {
  const s = Math.min(MAX, Math.max(MIN, v.s * f));
  if (s === 1) return FIT;
  const k = s / v.s;
  return { s, x: px - (px - v.x) * k, y: py - (py - v.y) * k };
}

export function PanZoom({ view, onView, children, label }: { view: View; onView: (v: View) => void; children: ReactNode; label: string }) {
  const box = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; y: number; v: View; id: number } | null>(null);

  const local = (e: { clientX: number; clientY: number }) => {
    const r = box.current!.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top] as const;
  };
  const now = useRef({ view, onView });
  now.current = { view, onView };
  // (a real listener: React's wheel events can't stop the page scrolling)
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const wheel = (e: WheelEvent) => {
      e.preventDefault();
      const [px, py] = local(e);
      now.current.onView(zoomBy(now.current.view, Math.exp(-e.deltaY * 0.0018), px, py));
    };
    el.addEventListener("wheel", wheel, { passive: false });
    return () => el.removeEventListener("wheel", wheel);
  }, []);
  const down = (e: RPointerEvent) => {
    if (e.button !== 0) return;
    drag.current = { x: e.clientX, y: e.clientY, v: view, id: e.pointerId };
    (e.target as Element).setPointerCapture?.(e.pointerId);
  };
  const move = (e: RPointerEvent) => {
    const d = drag.current;
    if (!d || d.id !== e.pointerId || view.s === 1) return;
    onView({ ...d.v, x: d.v.x + e.clientX - d.x, y: d.v.y + e.clientY - d.y });
  };
  const up = () => (drag.current = null);

  return (
    <div
      ref={box}
      className={`ov-pz${view.s > 1 ? " zoomed" : ""}`}
      role="img"
      aria-label={label}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={up}
      onDoubleClick={() => onView(FIT)}
    >
      <div className="ov-pz-in" style={{ transform: `translate(${px(view.x)}px, ${px(view.y)}px) scale(${view.s})` }}>
        {children}
      </div>
    </div>
  );
}
