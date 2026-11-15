// Comfort limits (limits.rs, the same ids): defaults, not hard caps. Adding a node that has one
// adds its mode setting (a number variable lim_<id> in the "Limits" group) when the mode doesn't
// have it yet: the creator sets its start value, hosts change it in mode settings.

import type { GVariable, Graph } from "./schema";

export type Limit = { id: string; label: string; default: number; lagAbove: number; max: number; kinds: string[] };

export const LIMITS: Limit[] = [
  { id: "markers", label: "Objective markers at once", default: 32, lagAbove: 64, max: 256, kinds: ["fx.marker"] },
  { id: "trails", label: "Trails at once", default: 32, lagAbove: 64, max: 256, kinds: ["fun.trail"] },
];

export const limitVar = (l: Limit): GVariable =>
  ({
    id: `lim_${l.id}`,
    name: l.label.slice(0, 30),
    type: "number",
    scope: "mode",
    default: l.default,
    setting: { label: l.label, help: `A comfort limit (default ${l.default}). Higher may lag servers with many players.`, min: 1, max: l.max, step: 1, group: "Limits" },
  }) as GVariable;

/** The graph with the limit settings a node of this type needs (unchanged when it has them). */
export function withLimitVars(g: Graph, type: string): Graph {
  const add = LIMITS.filter((l) => l.kinds.includes(type) && !g.variables.some((v) => v.id === `lim_${l.id}`));
  return add.length ? { ...g, variables: [...g.variables, ...add.map(limitVar)] } : g;
}
