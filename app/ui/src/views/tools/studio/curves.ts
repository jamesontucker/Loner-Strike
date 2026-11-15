// Timeline curves: how a track goes from one key to the next (the key it leaves says): Linear,
// Constant (a step), Ease In, Ease Out, Ease In-Out, or Custom (a cubic Hermite curve shaped by
// the keys' tangents: their own when dragged, else worked out from the neighbors, "Auto"). The
// plugin works out the same curves (graph.rs Key, gen.rs track_eval).

import type { GKey } from "./graphOps";

export type Ease = "linear" | "constant" | "ease" | "in" | "out" | "custom";
export const EASES: [Ease, string][] = [
  ["linear", "Linear"],
  ["constant", "Constant (a step)"],
  ["in", "Ease In"],
  ["out", "Ease Out"],
  ["ease", "Ease In-Out"],
  ["custom", "Custom (drag the tangents)"],
];

export const num = (v: unknown, c?: number): number => (c === undefined ? (typeof v === "number" ? v : 0) : Array.isArray(v) && typeof v[c] === "number" ? v[c] : 0);
export const sortKeys = (keys: GKey[]) => [...keys].sort((a, b) => a.t - b.t);

/** A key's own tangent on a side (value per second), or undefined (Auto). */
export function ownTangent(k: GKey, side: "in" | "out", c?: number): number | undefined {
  const t = (k as { tin?: unknown; tout?: unknown })[side === "in" ? "tin" : "tout"];
  if (t === undefined || t === null) return undefined;
  return c === undefined ? (typeof t === "number" ? t : undefined) : Array.isArray(t) && typeof t[c] === "number" ? t[c] : undefined;
}

/** Auto: the slope through the neighbors (Catmull-Rom); one-sided at the ends. */
export function autoTangent(keys: GKey[], i: number, c?: number): number {
  const a = keys[Math.max(0, i - 1)];
  const b = keys[Math.min(keys.length - 1, i + 1)];
  const dt = b.t - a.t;
  return dt > 1e-6 ? (num(b.v, c) - num(a.v, c)) / dt : 0;
}

export const tangent = (keys: GKey[], i: number, side: "in" | "out", c?: number) => ownTangent(keys[i], side, c) ?? autoTangent(keys, i, c);

/** 0..1 through a segment, shaped by an ease (Custom is handled with the values). */
export function shape(ease: string | undefined, x: number): number {
  switch (ease) {
    case "constant":
      return 0;
    case "in":
      return x * x;
    case "out":
      return x * (2 - x);
    case "ease":
      return x * x * (3 - 2 * x);
    default:
      return x;
  }
}

/** A track's value at time t (one component of a position or color), as the plugin works it out. */
export function valueAt(keys: GKey[], t: number, c?: number): number {
  const k = sortKeys(keys);
  if (!k.length) return 0;
  if (t <= k[0].t) return num(k[0].v, c);
  for (let i = 0; i < k.length - 1; i++) {
    const a = k[i],
      b = k[i + 1];
    if (t <= b.t) {
      const span = Math.max(b.t - a.t, 0.0001);
      const x = (t - a.t) / span;
      const p0 = num(a.v, c),
        p1 = num(b.v, c);
      if (a.ease === "custom") {
        const m0 = tangent(k, i, "out", c) * span,
          m1 = tangent(k, i + 1, "in", c) * span;
        const x2 = x * x,
          x3 = x2 * x;
        return p0 * (2 * x3 - 3 * x2 + 1) + m0 * (x3 - 2 * x2 + x) + p1 * (-2 * x3 + 3 * x2) + m1 * (x3 - x2);
      }
      return p0 + (p1 - p0) * shape(a.ease, x);
    }
  }
  return num(k[k.length - 1].v, c);
}
