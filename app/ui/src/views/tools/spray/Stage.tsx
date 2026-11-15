// The composer's canvas: the design drawn at twice its design size, and over it the picked
// layer's box with handles: drag inside to move, a corner to scale, the dot above to turn.
// Snapping (on unless Alt is held): the middle and edges of the canvas and the middles of the
// other layers, drawn as guide lines; turning snaps to 15 degrees.

import { useEffect, useRef, useState } from "react";
import { hit, layerSize, render, SHAPES, type Doc, type Layer } from "./doc";

type Props = {
  doc: Doc;
  selected: string | null;
  onSelect: (id: string | null) => void;
  /** the first move of a drag (one undo step) */
  onBegin: () => void;
  onLive: (fn: (d: Doc) => Doc) => void;
  snap: boolean;
  /** the drawn canvas (for the thumbnail) */
  canvasRef?: React.RefObject<HTMLCanvasElement>;
};

type Drag = {
  mode: "move" | "scale" | "rotate";
  id: string;
  start: { x: number; y: number };
  layer: Layer;
  moved: boolean;
};

const SNAP = 8;
const RES = 2;

export function Stage({ doc, selected, onSelect, onBegin, onLive, snap, canvasRef }: Props) {
  const own = useRef<HTMLCanvasElement>(null);
  const canvas = canvasRef ?? own;
  const svg = useRef<SVGSVGElement>(null);
  const [k, setK] = useState(1);
  const [guides, setGuides] = useState<{ x: number[]; y: number[] }>({ x: [], y: [] });
  const [tick, setTick] = useState(0);
  const drag = useRef<Drag | null>(null);
  const { w: W, h: H } = SHAPES[doc.shape];

  useEffect(() => {
    const g = canvas.current?.getContext("2d");
    if (g) render(g, doc, RES, () => setTick((t) => t + 1));
  }, [doc, tick, canvas]);

  // fonts load late: draw again once they're in
  useEffect(() => {
    const again = () => setTick((t) => t + 1);
    void document.fonts?.load('48px "Counter-Strike"').then(again, () => {});
    document.fonts?.addEventListener?.("loadingdone", again);
    // an installed font loaded for the lettering (sysfonts.ts)
    window.addEventListener("ls-font-loaded", again);
    return () => {
      document.fonts?.removeEventListener?.("loadingdone", again);
      window.removeEventListener("ls-font-loaded", again);
    };
  }, []);

  // design units per screen pixel (handles keep their size on screen)
  useEffect(() => {
    const el = svg.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setK(W / Math.max(1, el.getBoundingClientRect().width)));
    ro.observe(el);
    return () => ro.disconnect();
  }, [W]);

  const point = (e: { clientX: number; clientY: number }) => {
    const r = svg.current!.getBoundingClientRect();
    return { x: ((e.clientX - r.left) / r.width) * W, y: ((e.clientY - r.top) / r.height) * H };
  };

  const sel = doc.layers.find((l) => l.id === selected && !l.hidden) ?? null;

  const down = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    const p = point(e);
    const handle = (e.target as Element).getAttribute("data-handle") as Drag["mode"] | null;
    let layer: Layer | null = null;
    let mode: Drag["mode"] = "move";
    if (handle && sel && !sel.locked) {
      layer = sel;
      mode = handle;
    } else {
      layer = hit(doc, p.x, p.y);
      onSelect(layer?.id ?? null);
    }
    if (!layer) return;
    e.preventDefault();
    svg.current!.setPointerCapture(e.pointerId);
    drag.current = { mode, id: layer.id, start: p, layer, moved: false };
  };

  const move = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d) return;
    const p = point(e);
    if (!d.moved) {
      if (Math.hypot(p.x - d.start.x, p.y - d.start.y) < 2 * k) return;
      d.moved = true;
      onBegin();
    }
    const L = d.layer;
    const snapping = snap && !e.altKey;
    let next: Partial<Layer> = {};
    const gx: number[] = [];
    const gy: number[] = [];
    if (d.mode === "move") {
      let x = L.x + p.x - d.start.x;
      let y = L.y + p.y - d.start.y;
      if (e.shiftKey) {
        // along one axis only
        if (Math.abs(p.x - d.start.x) > Math.abs(p.y - d.start.y)) y = L.y;
        else x = L.x;
      }
      if (snapping) {
        const { w, h } = layerSize(L);
        const hw = (w * L.scale) / 2;
        const hh = (h * L.scale) / 2;
        const xs = [W / 2, ...doc.layers.filter((o) => o.id !== L.id && !o.hidden).map((o) => o.x)];
        const ys = [H / 2, ...doc.layers.filter((o) => o.id !== L.id && !o.hidden).map((o) => o.y)];
        const near = (v: number, list: number[]) => list.find((t) => Math.abs(t - v) <= SNAP * k);
        const sx = near(x, xs);
        if (sx !== undefined) (x = sx), gx.push(sx);
        else if (L.rot % 180 === 0) {
          // edges to the canvas edges (unturned layers)
          if (Math.abs(x - hw) <= SNAP * k) (x = hw), gx.push(0);
          else if (Math.abs(x + hw - W) <= SNAP * k) (x = W - hw), gx.push(W);
        }
        const sy = near(y, ys);
        if (sy !== undefined) (y = sy), gy.push(sy);
        else if (L.rot % 180 === 0) {
          if (Math.abs(y - hh) <= SNAP * k) (y = hh), gy.push(0);
          else if (Math.abs(y + hh - H) <= SNAP * k) (y = H - hh), gy.push(H);
        }
      }
      next = { x: Math.round(x * 10) / 10, y: Math.round(y * 10) / 10 };
    } else if (d.mode === "scale") {
      const d0 = Math.hypot(d.start.x - L.x, d.start.y - L.y);
      const d1 = Math.hypot(p.x - L.x, p.y - L.y);
      let s = Math.max(0.03, Math.min(12, (L.scale * d1) / Math.max(1, d0)));
      if (snapping && Math.abs(s - 1) < 0.04) s = 1;
      next = { scale: Math.round(s * 1000) / 1000 };
    } else {
      const a0 = Math.atan2(d.start.y - L.y, d.start.x - L.x);
      const a1 = Math.atan2(p.y - L.y, p.x - L.x);
      let r = L.rot + ((a1 - a0) * 180) / Math.PI;
      if (snapping) r = Math.round(r / 15) * 15;
      r = ((((r + 180) % 360) + 360) % 360) - 180;
      next = { rot: Math.round(r * 10) / 10 };
    }
    setGuides({ x: gx, y: gy });
    onLive((doc) => ({ ...doc, layers: doc.layers.map((l) => (l.id === d.id ? ({ ...l, ...next } as Layer) : l)) }));
  };

  const up = () => {
    drag.current = null;
    setGuides({ x: [], y: [] });
  };

  // the picked layer's box (turned) and handles
  let box = null;
  if (sel) {
    const { w, h } = layerSize(sel);
    const a = (sel.rot * Math.PI) / 180;
    const c = Math.cos(a);
    const s = Math.sin(a);
    const at = (lx: number, ly: number) => [sel.x + (lx * c - ly * s) * sel.scale, sel.y + (lx * s + ly * c) * sel.scale] as const;
    const corners = [at(-w / 2, -h / 2), at(w / 2, -h / 2), at(w / 2, h / 2), at(-w / 2, h / 2)];
    const top = at(0, -h / 2);
    const knob = [top[0] + s * 28 * k, top[1] - c * 28 * k] as const;
    const hs = 9 * k;
    box = (
      <g className="sc-box">
        <polygon points={corners.map((p) => p.join(",")).join(" ")} strokeWidth={1.5 * k} strokeDasharray={sel.locked ? `${4 * k} ${3 * k}` : undefined} />
        {!sel.locked && (
          <>
            <line x1={top[0]} y1={top[1]} x2={knob[0]} y2={knob[1]} strokeWidth={1.5 * k} />
            <circle data-handle="rotate" className="sc-knob" cx={knob[0]} cy={knob[1]} r={6.5 * k} strokeWidth={1.5 * k}>
              <title>Drag to turn (snaps to 15°; Alt: free)</title>
            </circle>
            {corners.map((p, i) => (
              <rect key={i} data-handle="scale" className="sc-handle" x={p[0] - hs / 2} y={p[1] - hs / 2} width={hs} height={hs} strokeWidth={1.5 * k}>
                <title>Drag to resize</title>
              </rect>
            ))}
          </>
        )}
      </g>
    );
  }

  return (
    <div className={`sc-stage${doc.background ? "" : " clear"}`} style={{ aspectRatio: `${W} / ${H}` }}>
      <canvas ref={canvas} width={W * RES} height={H * RES} aria-label="Your spray design" role="img" />
      <svg
        ref={svg}
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        onPointerDown={down}
        onPointerMove={move}
        onPointerUp={up}
        onPointerCancel={up}
        aria-hidden="true"
      >
        {guides.x.map((x, i) => (
          <line key={`x${i}`} className="sc-guide" x1={x} y1={0} x2={x} y2={H} strokeWidth={1 * k} />
        ))}
        {guides.y.map((y, i) => (
          <line key={`y${i}`} className="sc-guide" x1={0} y1={y} x2={W} y2={y} strokeWidth={1 * k} />
        ))}
        {box}
      </svg>
    </div>
  );
}
