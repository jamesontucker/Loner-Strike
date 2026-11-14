// The Shape tool's live preview: the outline of the brush(es) the core will make for the box
// being dragged, built the same way as editor/shapes.rs and ops::make_cylinder (corners only,
// before grid rounding), so the ghost is the shape picked, not always a cylinder.

import type { ShapeKind } from "./store";

type V = [number, number, number];
export type Seg = [V, V];

export interface ShapeParams {
  shape: ShapeKind;
  sides: number;
  arc: number;
  wall: number;
  start: number;
  /** Parity A: a cone's top as a share of its base (0: a point) */
  top?: number;
  /** Parity A: a torus's tube sides */
  tubeSides?: number;
  /** Parity A: arch and torus pieces climb this much each */
  addHeight?: number;
}

const TAU = Math.PI * 2;

function ringPts(c: V, rx: number, ry: number, sides: number, z: number, from = 0, sweep = 360, open = false): V[] {
  const n = open ? sides + 1 : sides;
  return Array.from({ length: n }, (_, i) => {
    const t = ((from + (sweep * i) / sides) * TAU) / 360;
    return [c[0] + Math.cos(t) * rx, c[1] + Math.sin(t) * ry, z] as V;
  });
}

function loop(pts: V[], closed = true): Seg[] {
  const out: Seg[] = [];
  for (let i = 0; i + 1 < pts.length; i++) out.push([pts[i], pts[i + 1]]);
  if (closed && pts.length > 2) out.push([pts[pts.length - 1], pts[0]]);
  return out;
}

/** Line segments outlining the shape that fills the box min..max (Z up). */
export function shapeOutline(min: V, max: V, p: ShapeParams): Seg[] {
  const c: V = [(min[0] + max[0]) / 2, (min[1] + max[1]) / 2, (min[2] + max[2]) / 2];
  const rx = (max[0] - min[0]) / 2;
  const ry = (max[1] - min[1]) / 2;
  const sides = Math.max(3, Math.min(64, Math.round(p.sides) || 8));
  switch (p.shape) {
    case "wedge": {
      // shapes.rs make_wedge: full floor, the top edge along max Y (the slope rises toward +Y)
      const [a, b, cc, d] = [
        [min[0], min[1], min[2]],
        [max[0], min[1], min[2]],
        [max[0], max[1], min[2]],
        [min[0], max[1], min[2]],
      ] as V[];
      const e: V = [min[0], max[1], max[2]];
      const f: V = [max[0], max[1], max[2]];
      return [...loop([a, b, cc, d]), [d, e], [cc, f], [e, f], [a, e], [b, f]];
    }
    case "spike": {
      const base = ringPts(c, rx, ry, sides, min[2]);
      const apex: V = [c[0], c[1], max[2]];
      return [...loop(base), ...base.map((q) => [q, apex] as Seg)];
    }
    case "sphere": {
      const s = Math.max(4, Math.min(32, sides));
      const bands = Math.max(2, Math.floor(s / 2));
      const rz = (max[2] - min[2]) / 2;
      const at = (band: number, i: number): V => {
        const lat = (Math.PI * band) / bands;
        const lon = (TAU * i) / s;
        return [c[0] + Math.sin(lat) * Math.cos(lon) * rx, c[1] + Math.sin(lat) * Math.sin(lon) * ry, c[2] + Math.cos(lat) * rz];
      };
      const out: Seg[] = [];
      for (let band = 0; band < bands; band++)
        for (let i = 0; i < s; i++) {
          out.push([at(band, i), at(band + 1, i)]);
          if (band > 0) out.push([at(band, i), at(band, (i + 1) % s)]);
        }
      return out;
    }
    case "cone": {
      // Parity A: shapes.rs make_cone: the base on the floor, the top ring (or a point) on top
      const k = Math.max(0, Math.min(0.95, p.top ?? 0));
      const base = ringPts(c, rx, ry, sides, min[2]);
      if (k < 1e-3) return [...loop(base), ...base.map((q) => [q, [c[0], c[1], max[2]]] as Seg)];
      const topRing = ringPts(c, rx * k, ry * k, sides, max[2]);
      return [...loop(base), ...loop(topRing), ...base.map((q, i) => [q, topRing[i]] as Seg)];
    }
    case "torus": {
      // Parity A: shapes.rs make_torus: a tube's cross-sections round the ring, joined
      const sweep = Math.max(1, Math.min(360, p.arc || 360));
      const halfW = Math.max(0.5, (p.wall || 16) / 2);
      const halfH = (max[2] - min[2]) / 2;
      const ts = Math.max(3, Math.min(32, Math.round(p.tubeSides ?? 8)));
      const rise = p.addHeight ?? 0;
      const section = (k: number): V[] => {
        const a = ((p.start + (sweep * k) / sides) * TAU) / 360;
        const z = min[2] + halfH + rise * k;
        return Array.from({ length: ts }, (_, j) => {
          const phi = (TAU * j) / ts;
          const off = Math.cos(phi) * halfW;
          return [c[0] + Math.cos(a) * (rx - halfW + off), c[1] + Math.sin(a) * (ry - halfW + off), z + Math.sin(phi) * halfH] as V;
        });
      };
      const out: Seg[] = [];
      let prev = section(0);
      out.push(...loop(prev));
      for (let k = 1; k <= sides; k++) {
        const next = section(k);
        out.push(...loop(next), ...prev.map((q, j) => [q, next[j]] as Seg));
        prev = next;
      }
      return out;
    }
    case "arch": {
      const sweep = Math.max(1, Math.min(360, p.arc || 180));
      const w = Math.max(1, p.wall || 16);
      const rise = p.addHeight ?? 0;
      if (rise) {
        // Parity A: each piece its own block, raised by add height (shapes.rs make_arch)
        const out: Seg[] = [];
        for (let k = 0; k < sides; k++) {
          const z0 = min[2] + rise * k;
          const z1 = max[2] + rise * k;
          const d0 = p.start + (sweep * k) / sides;
          const d1 = p.start + (sweep * (k + 1)) / sides;
          const at = (d: number, inner: boolean, z: number): V => {
            const t = (d * TAU) / 360;
            const [ex, ey] = inner ? [Math.max(0.5, rx - w), Math.max(0.5, ry - w)] : [rx, ry];
            return [c[0] + Math.cos(t) * ex, c[1] + Math.sin(t) * ey, z];
          };
          const lo = [at(d0, false, z0), at(d1, false, z0), at(d1, true, z0), at(d0, true, z0)];
          const hi = [at(d0, false, z1), at(d1, false, z1), at(d1, true, z1), at(d0, true, z1)];
          out.push(...loop(lo), ...loop(hi), ...lo.map((q, i) => [q, hi[i]] as Seg));
        }
        return out;
      }
      const out: Seg[] = [];
      for (const z of [min[2], max[2]]) {
        const outer = ringPts(c, rx, ry, sides, z, p.start, sweep, true);
        const inner = ringPts(c, Math.max(0.5, rx - w), Math.max(0.5, ry - w), sides, z, p.start, sweep, true);
        out.push(...loop(outer, false), ...loop(inner, false));
        outer.forEach((q, i) => out.push([q, inner[i]]));
        if (z === min[2]) outer.forEach((q, i) => out.push([q, [q[0], q[1], max[2]]], [inner[i], [inner[i][0], inner[i][1], max[2]]]));
      }
      return out;
    }
    default: {
      // the cylinder: radius from the footprint's shorter side, centered on it
      const r = Math.min(rx, ry);
      const lo = ringPts(c, r, r, sides, min[2]);
      const hi = ringPts(c, r, r, sides, max[2]);
      return [...loop(lo), ...loop(hi), ...lo.map((q, i) => [q, hi[i]] as Seg)];
    }
  }
}
