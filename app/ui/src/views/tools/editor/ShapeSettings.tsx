// Parity A: the Shape tool's own settings in the tool bar (a cone's top, a torus's tube sides,
// add height for arches and rings) and a Preview popover: every setting of the current shape
// with a turning wireframe of what you'll get, before you drag it out. The popover's numbers
// are the same ones the tool bar shows, so either place changes both.

import { useEffect, useMemo, useRef, useState } from "react";
import { useP5 } from "./p5store";
import { shapeOutline, type Seg } from "./shapePreview";
import { useCurrentShape, useShapeParams } from "./shapeParams";
import { useEditor } from "./store";
import "./parityA.css";

const num = (v: string, lo: number, hi: number, fallback: number) => {
  const n = Number(v);
  return Number.isFinite(n) ? Math.max(lo, Math.min(hi, n)) : fallback;
};

/** The fields only the newer shapes have, inline in the tool bar. */
export function ShapeExtraFields() {
  const shape = useEditor((s) => s.shape);
  const grid = useEditor((s) => s.grid);
  const x = useShapeParams();
  return (
    <>
      {shape === "cone" && (
        <label title="The top's size as a share of the base: 0 comes to a point, more cuts it off flat">
          Top %
          <input type="number" min={0} max={95} step={5} value={Math.round(x.coneTop * 100)} onChange={(e) => x.set({ coneTop: num(e.target.value, 0, 95, 0) / 100 })} />
        </label>
      )}
      {shape === "torus" && (
        <label title="How many sides the tube has (3-32)">
          Tube sides
          <input type="number" min={3} max={32} value={x.tubeSides} onChange={(e) => x.set({ tubeSides: Math.round(num(e.target.value, 3, 32, 8)) })} />
        </label>
      )}
      {(shape === "arch" || shape === "torus") && (
        <label title="Each piece sits this much higher than the last: spiral stairs, ramps and slides (0: flat)">
          Add height
          <input type="number" min={-4096} max={4096} step={grid} value={x.addHeight} onChange={(e) => x.set({ addHeight: num(e.target.value, -4096, 4096, 0) })} />
        </label>
      )}
      <ShapePreviewButton />
    </>
  );
}

function ShapePreviewButton() {
  const [open, setOpen] = useState(false);
  const wrap = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      if (!wrap.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);
  return (
    <span className="pa-shape-wrap" ref={wrap}>
      <button type="button" className="pa-shape-btn" aria-expanded={open} title="Every setting of this shape, with a preview" onClick={() => setOpen((o) => !o)}>
        Preview…
      </button>
      {open && <ShapePopover />}
    </span>
  );
}

const LABELS: Record<string, string> = { cylinder: "Cylinder", wedge: "Wedge", spike: "Spike", sphere: "Sphere", arch: "Arch", cone: "Cone", torus: "Torus" };

function ShapePopover() {
  const s = useEditor();
  const x = useShapeParams();
  const start = useP5((p) => p.archStart);
  const p = useCurrentShape();
  const shape = s.shape;
  const round = shape === "arch" || shape === "torus";
  const sidesMin = shape === "sphere" ? 4 : 3;
  const sidesMax = shape === "sphere" ? 32 : 64;
  return (
    <div className="pa-shape-pop" role="dialog" aria-label={`${LABELS[shape] ?? shape} settings`}>
      <div className="pa-shape-head">{LABELS[shape] ?? shape}</div>
      <ShapeWire />
      <div className="pa-shape-fields">
        <label>
          Height
          <input type="number" min={1} max={4096} step={s.grid} value={s.boxHeight} onChange={(e) => s.set({ boxHeight: num(e.target.value, 1, 4096, 128) })} />
        </label>
        {shape !== "wedge" && (
          <label>
            {round ? "Pieces" : shape === "sphere" ? "Segments" : "Sides"}
            <input type="number" min={sidesMin} max={sidesMax} value={s.sides} onChange={(e) => s.set({ sides: Math.round(num(e.target.value, sidesMin, sidesMax, 12)) })} />
          </label>
        )}
        {round && (
          <>
            <label>
              Arc°
              <input type="number" min={1} max={360} value={s.archArc} onChange={(e) => s.set({ archArc: num(e.target.value, 1, 360, 180) })} />
            </label>
            <label>
              Start°
              <input type="number" min={-360} max={360} step={15} value={start} onChange={(e) => useP5.getState().set({ archStart: num(e.target.value, -360, 360, 0) })} />
            </label>
            <label>
              {shape === "torus" ? "Tube width" : "Wall"}
              <input type="number" min={1} max={1024} step={s.grid} value={s.archWall} onChange={(e) => s.set({ archWall: num(e.target.value, 1, 1024, 16) })} />
            </label>
            <label>
              Add height
              <input type="number" min={-4096} max={4096} step={s.grid} value={x.addHeight} onChange={(e) => x.set({ addHeight: num(e.target.value, -4096, 4096, 0) })} />
            </label>
          </>
        )}
        {shape === "torus" && (
          <label>
            Tube sides
            <input type="number" min={3} max={32} value={x.tubeSides} onChange={(e) => x.set({ tubeSides: Math.round(num(e.target.value, 3, 32, 8)) })} />
          </label>
        )}
        {shape === "cone" && (
          <label>
            Top %
            <input type="number" min={0} max={95} step={5} value={Math.round(x.coneTop * 100)} onChange={(e) => x.set({ coneTop: num(e.target.value, 0, 95, 0) / 100 })} />
          </label>
        )}
      </div>
      <p className="pa-shape-note muted">
        {p.shape === "arch" || p.shape === "torus" ? `${p.sides} brushes. ` : "One brush. "}Drag it out in the 3D or Top view; the box you drag is its outside size.
      </p>
    </div>
  );
}

/** The shape in a 256-wide box seen from above at an angle, turning slowly. */
function ShapeWire() {
  const p = useCurrentShape();
  const h = useEditor((s) => s.boxHeight);
  const [turn, setTurn] = useState(30);
  useEffect(() => {
    let raf = 0;
    let last = performance.now();
    const step = (t: number) => {
      setTurn((a) => (a + (t - last) * 0.02) % 360);
      last = t;
      raf = requestAnimationFrame(step);
    };
    // no spinning for people who'd rather not
    if (!window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, []);
  const segs: Seg[] = useMemo(() => {
    const height = Math.max(16, Math.min(512, h));
    return shapeOutline([-128, -128, 0], [128, 128, height], p);
  }, [p.shape, p.sides, p.arc, p.wall, p.start, p.top, p.tubeSides, p.addHeight, h]); // eslint-disable-line react-hooks/exhaustive-deps
  const a = (turn * Math.PI) / 180;
  const [ca, sa] = [Math.cos(a), Math.sin(a)];
  const proj = ([x, y, z]: [number, number, number]) => {
    const rx = x * ca - y * sa;
    const ry = x * sa + y * ca;
    return [rx, -z * 0.9 + ry * 0.45] as const;
  };
  let [minX, minY, maxX, maxY] = [Infinity, Infinity, -Infinity, -Infinity];
  const lines = segs.map(([q, r]) => {
    const [x1, y1] = proj(q);
    const [x2, y2] = proj(r);
    minX = Math.min(minX, x1, x2);
    maxX = Math.max(maxX, x1, x2);
    minY = Math.min(minY, y1, y2);
    maxY = Math.max(maxY, y1, y2);
    return `M${x1.toFixed(1)} ${y1.toFixed(1)}L${x2.toFixed(1)} ${y2.toFixed(1)}`;
  });
  const pad = 12;
  const vb = Number.isFinite(minX) ? `${minX - pad} ${minY - pad} ${maxX - minX + pad * 2} ${maxY - minY + pad * 2}` : "0 0 10 10";
  return (
    <svg className="pa-shape-wire" viewBox={vb} preserveAspectRatio="xMidYMid meet" aria-label="Preview of the shape" role="img" data-segments={segs.length}>
      <path d={lines.join("")} vectorEffect="non-scaling-stroke" />
    </svg>
  );
}
