// The callout editor's server side (tools/callouts.rs).

import { get, post, tryGet } from "../common/api";

/** [id, x0, y0, x1, y1, z, place (1-based into places, 0 none)] */
export type RawArea = [number, number, number, number, number, number, number];

export type CalloutNav = {
  map: string;
  game: string;
  file: string | null;
  version: number;
  matchesMap: boolean | null;
  places: string[];
  areas: RawArea[];
  /** places bots have a voice line for (the game's BotChatter.db) */
  voice: string[];
  /** callouts bot chat knows beyond those */
  chat: string[];
  /** the bots' map knowledge fits this nav (null: there's none) */
  knowledge: boolean | null;
  stamp: string;
  /** the usual callouts are known for this map */
  usual: boolean;
  /** the other game folder has a named nav of this map */
  other: string | null;
  backup: boolean;
  inUse: boolean;
};

export type Area = { id: number; x0: number; y0: number; x1: number; y1: number; z: number; place: string };

export type Change = { id: number; place: string };

const base = (map: string) => `/api/tools/callouts/${encodeURIComponent(map)}`;

export const loadCallouts = (map: string): Promise<CalloutNav> => get<CalloutNav>(base(map));

export const suggest = (map: string, from: "usual" | "other") => tryGet<{ changes?: Change[]; message?: string }>(`${base(map)}/suggest?from=${from}`);

export const saveCallouts = (map: string, stamp: string, changes: Change[]) =>
  post<{ ok?: boolean; changed?: number; message?: string; stamp?: string; knowledge?: boolean | null }>(base(map), { stamp, changes });

/** The same rule as the server: a letter, then letters, digits or _, up to 31. */
export const validPlace = (n: string) => /^[A-Za-z][A-Za-z0-9_]{0,30}$/.test(n);

/** "UpperTunnel" -> "upper tunnel", as bot chat says it. */
export const spoken = (n: string) =>
  n
    .replace(/_/g, " ")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .toLowerCase();

/** A colour per place, spread round the wheel by its place in the sorted list. */
export function placeColours(names: string[]): Map<string, string> {
  const sorted = [...new Set(names)].filter(Boolean).sort();
  const m = new Map<string, string>();
  sorted.forEach((n, i) => {
    const hue = Math.round((i * 137.508) % 360);
    m.set(n, `hsl(${hue} 62% 58%)`);
  });
  return m;
}
