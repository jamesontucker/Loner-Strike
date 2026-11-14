// The bots' data laid over a map (plan M4-M6): the nav, what the bots learned (.lsv), and
// recorded matches with the brain's trace. Shapes as agent C's routes send them
// (docs/TOOLS-NOTES.md); decoders never trust a length they didn't check.

import { BundleError, readLsmb } from "./viewer/bundle";
import type { Vec3 } from "./viewer/coords";

// ---- nav ------------------------------------------------------------------------------------

export type NavArea = {
  id: number;
  attr: number;
  nw: Vec3;
  se: Vec3;
  neZ: number;
  swZ: number;
  /** area ids per direction: north (-y), east (+x), south (+y), west (-x) */
  conns: number[][];
  hiding: { id: number; pos: Vec3; flags: number }[];
  /** 1-based into places, 0 = none */
  place: number;
  approach: number;
  encounters: number;
};
export type NavLadder = { entity: number; top: Vec3; bottom: Vec3; length: number; dir: number; bottomArea: number | null; topForward: number | null; topLeft: number | null; topRight: number | null; topBehind: number | null; dangling: boolean };
export type Nav = {
  version: number;
  matchesMap: boolean | null;
  places: string[];
  areas: NavArea[];
  ladders: NavLadder[];
  oneWay: [number, number][];
  danglingLinks: unknown[];
  attrBits: { crouch: number; jump: number; precise: number; noJump: number };
  spotFlags: { inCover: number; goodSniper: number; idealSniper: number };
  warning: string | null;
  /** worked out here: area id -> index (file order) */
  index: Map<number, number>;
  /** centre of each area (file order), GoldSrc */
  centres: Vec3[];
  /** connected group of each area (undirected), 0 = the biggest */
  island: Int32Array;
  islands: number;
};

const num = (v: unknown, d = 0) => (typeof v === "number" && Number.isFinite(v) ? v : d);
const vec = (v: unknown): Vec3 => (Array.isArray(v) && v.length >= 3 ? [num(v[0]), num(v[1]), num(v[2])] : [0, 0, 0]);

export function toNav(raw: Record<string, unknown>): Nav {
  const areas: NavArea[] = (Array.isArray(raw.areas) ? raw.areas : []).map((a: Record<string, unknown>) => ({
    id: num(a.id),
    attr: num(a.attr),
    nw: vec(a.nw),
    se: vec(a.se),
    neZ: num(a.neZ),
    swZ: num(a.swZ),
    conns: Array.isArray(a.conns) ? (a.conns as unknown[]).slice(0, 4).map((c) => (Array.isArray(c) ? c.map((x) => num(x)) : [])) : [[], [], [], []],
    hiding: (Array.isArray(a.hiding) ? a.hiding : []).map((h: Record<string, unknown>) => ({ id: num(h.id), pos: vec(h.pos), flags: num(h.flags) })),
    place: num(a.place),
    approach: num(a.approach),
    encounters: num(a.encounters),
  }));
  const index = new Map<number, number>();
  areas.forEach((a, i) => index.set(a.id, i));
  const centres = areas.map((a): Vec3 => [(a.nw[0] + a.se[0]) / 2, (a.nw[1] + a.se[1]) / 2, (a.nw[2] + a.se[2] + a.neZ + a.swZ) / 4]);
  // connected groups, ignoring direction; renumbered so the biggest is 0
  const group = new Int32Array(areas.length).fill(-1);
  const adj: number[][] = areas.map(() => []);
  areas.forEach((a, i) => a.conns.flat().forEach((id) => {
    const j = index.get(id);
    if (j !== undefined) {
      adj[i].push(j);
      adj[j].push(i);
    }
  }));
  // ladders join their bottom area to the areas at the top (the game links them itself)
  for (const l of Array.isArray(raw.ladders) ? (raw.ladders as Record<string, unknown>[]) : []) {
    const b = index.get(num(l.bottomArea, -1));
    if (b === undefined) continue;
    for (const k of ["topForward", "topLeft", "topRight", "topBehind"]) {
      const t = index.get(num(l[k], -1));
      if (t !== undefined) {
        adj[b].push(t);
        adj[t].push(b);
      }
    }
  }
  let groups = 0;
  const sizes: number[] = [];
  for (let s = 0; s < areas.length; s++) {
    if (group[s] >= 0) continue;
    const stack = [s];
    group[s] = groups;
    let size = 0;
    while (stack.length) {
      const i = stack.pop()!;
      size++;
      for (const j of adj[i]) if (group[j] < 0) {
        group[j] = groups;
        stack.push(j);
      }
    }
    sizes.push(size);
    groups++;
  }
  const order = sizes.map((n, g) => [n, g]).sort((a, b) => b[0] - a[0]).map(([, g]) => g);
  const rank = new Int32Array(groups);
  order.forEach((g, r) => (rank[g] = r));
  const island = group.map((g) => rank[g]);
  const bits = (raw.attrBits ?? {}) as Record<string, unknown>;
  const spots = (raw.spotFlags ?? {}) as Record<string, unknown>;
  return {
    version: num(raw.version),
    matchesMap: typeof raw.matchesMap === "boolean" ? raw.matchesMap : null,
    places: Array.isArray(raw.places) ? raw.places.map(String) : [],
    areas,
    ladders: (Array.isArray(raw.ladders) ? raw.ladders : []).map((l: Record<string, unknown>) => ({
      entity: num(l.entity), top: vec(l.top), bottom: vec(l.bottom), length: num(l.length), dir: num(l.dir),
      bottomArea: typeof l.bottomArea === "number" ? l.bottomArea : null,
      topForward: typeof l.topForward === "number" ? l.topForward : null,
      topLeft: typeof l.topLeft === "number" ? l.topLeft : null,
      topRight: typeof l.topRight === "number" ? l.topRight : null,
      topBehind: typeof l.topBehind === "number" ? l.topBehind : null,
      dangling: !!l.dangling,
    })),
    oneWay: (Array.isArray(raw.oneWay) ? raw.oneWay : []).filter((p): p is [number, number] => Array.isArray(p) && p.length >= 2).map((p) => [num(p[0]), num(p[1])]),
    danglingLinks: Array.isArray(raw.danglingLinks) ? raw.danglingLinks : [],
    attrBits: { crouch: num(bits.crouch, 1), jump: num(bits.jump, 2), precise: num(bits.precise, 4), noJump: num(bits.noJump, 8) },
    spotFlags: { inCover: num(spots.inCover, 1), goodSniper: num(spots.goodSniper, 2), idealSniper: num(spots.idealSniper, 4) },
    warning: typeof raw.warning === "string" ? raw.warning : null,
    index,
    centres,
    island,
    islands: groups,
  };
}

/** The moves an area asks for, in plain words. */
export function attrWords(nav: Nav, attr: number): string[] {
  const b = nav.attrBits;
  return [attr & b.crouch && "crouch", attr & b.jump && "jump", attr & b.precise && "walk carefully", attr & b.noJump && "no jumping"].filter(Boolean) as string[];
}

// ---- knowledge (.lsv) -----------------------------------------------------------------------

export type Know = {
  count: number;
  stageName: string;
  done: boolean;
  keyMatches: boolean | null;
  warning: string | null;
  traces: number;
  materials: Record<string, string>;
  objectTypes: string[];
  sight: Uint8Array;
  bangA: Uint16Array; bangB: Uint16Array; bangThick: Uint8Array; bangMat: Uint8Array;
  pairA: Uint16Array; pairB: Uint16Array; pairQuality: Uint8Array; pairFlags: Uint8Array;
  coverLow: Uint16Array; coverHigh: Uint16Array;
  sees: Uint16Array; high: Uint8Array;
  cornerCount: Uint8Array; corners: Float32Array;
  angleCount: Uint8Array; angles: Float32Array;
};

const KNOW_DAMAGED = "This map's knowledge file couldn't be read. It may be damaged; playing the map with bots rebuilds it.";

export function decodeKnow(buf: ArrayBuffer): Know {
  const { header: h, get } = readLsmb(buf, KNOW_DAMAGED);
  const n = num(h.count);
  const need = <T extends ArrayBufferView>(name: string, type: new (...a: never[]) => T, per: number, exact = true): T => {
    const a = get(name);
    if (!(a instanceof type) || (exact && (a as unknown as { length: number }).length !== n * per)) throw new BundleError(KNOW_DAMAGED);
    return a as T;
  };
  const sight = need("sight", Uint8Array, 0, false);
  if (sight.length < Math.ceil((n * n) / 8)) throw new BundleError(KNOW_DAMAGED);
  const pairA = need("pairA", Uint16Array, 0, false), pairB = need("pairB", Uint16Array, 0, false);
  const bangA = need("bangA", Uint16Array, 0, false), bangB = need("bangB", Uint16Array, 0, false);
  const pairQuality = need("pairQuality", Uint8Array, 0, false), pairFlags = need("pairFlags", Uint8Array, 0, false);
  const bangThick = need("bangThick", Uint8Array, 0, false), bangMat = need("bangMat", Uint8Array, 0, false);
  if (pairB.length !== pairA.length || pairQuality.length !== pairA.length || pairFlags.length !== pairA.length) throw new BundleError(KNOW_DAMAGED);
  if (bangB.length !== bangA.length || bangThick.length !== bangA.length || bangMat.length !== bangA.length) throw new BundleError(KNOW_DAMAGED);
  return {
    count: n,
    stageName: String(h.stageName ?? ""),
    done: !!h.done,
    keyMatches: typeof h.keyMatches === "boolean" ? h.keyMatches : null,
    warning: typeof h.warning === "string" ? h.warning : null,
    traces: num(h.traces),
    materials: (h.materials && typeof h.materials === "object" ? h.materials : {}) as Record<string, string>,
    objectTypes: Array.isArray(h.objectTypes) ? h.objectTypes.map(String) : [],
    sight, pairA, pairB, pairQuality, pairFlags, bangA, bangB, bangThick, bangMat,
    coverLow: need("coverLow", Uint16Array, 16),
    coverHigh: need("coverHigh", Uint16Array, 16),
    sees: need("sees", Uint16Array, 1),
    high: need("high", Uint8Array, 1),
    cornerCount: need("cornerCount", Uint8Array, 1),
    corners: need("corners", Float32Array, 32),
    angleCount: need("angleCount", Uint8Array, 1),
    angles: need("angles", Float32Array, 40),
  };
}

/** Whether area i sees area j (indices in nav file order). */
export const sees = (k: Know, i: number, j: number) => {
  const bit = i * k.count + j;
  return (k.sight[bit >> 3] >> (bit & 7)) & 1;
};

// ---- recordings -----------------------------------------------------------------------------

export type RecordingInfo = { id: string; file: string; map: string; when: number | string; size: number; hasTrace: boolean; seconds: number; bots: number; players: number; kills: number; rounds: number; source?: string; error?: string };

export type Player = { id: number; name: string; bot: boolean; team: number; start: number; count: number };
export type Kill = { t: number; killer: number; victim: number; headshot: boolean; weapon: string; cause: string; teamKill: boolean; at: Vec3; fall: { from: Vec3; fromT: number; drop: number; speed: number } | null };
export type Replay = {
  id: string;
  map: string;
  /** false: the recording's map isn't in the game */
  mapInstalled: boolean;
  hasTrace: boolean;
  start: number;
  end: number;
  players: Player[];
  rounds: { start: number; end: number }[];
  kills: Kill[];
  stuck: { id: number; from: number; to: number; at: Vec3 }[];
  chat: { t: number; id: number; text: string }[];
  heat: { cell: number; x0: number; y0: number; w: number; h: number; time: Float32Array; deaths: Uint32Array; stuck: Float32Array } | null;
  t: Float32Array; x: Float32Array; y: Float32Array; z: Float32Array; yaw: Float32Array; pitch: Float32Array;
  hp: Uint8Array; duck: Uint8Array;
};

const REC_DAMAGED = "This recording couldn't be read. It may be cut off or damaged.";

export function decodeReplay(buf: ArrayBuffer): Replay {
  const { header: h, get } = readLsmb(buf, REC_DAMAGED);
  const f32 = (n: string) => {
    const a = get(n);
    if (!(a instanceof Float32Array)) throw new BundleError(REC_DAMAGED);
    return a;
  };
  const u8 = (n: string) => {
    const a = get(n);
    if (!(a instanceof Uint8Array)) throw new BundleError(REC_DAMAGED);
    return a;
  };
  const t = f32("t");
  const cols = { x: f32("x"), y: f32("y"), z: f32("z"), yaw: f32("yaw"), pitch: f32("pitch"), hp: u8("hp"), duck: u8("duck") };
  if (Object.values(cols).some((c) => c.length !== t.length)) throw new BundleError(REC_DAMAGED);
  const players: Player[] = (Array.isArray(h.players) ? h.players : []).map((p: Record<string, unknown>) => ({
    id: num(p.id), name: String(p.name ?? `#${p.id}`), bot: !!p.bot, team: num(p.team), start: num(p.start), count: num(p.count),
  })).filter((p) => p.start >= 0 && p.count >= 0 && p.start + p.count <= t.length);
  const hh = (h.heat ?? null) as Record<string, unknown> | null;
  let heat: Replay["heat"] = null;
  if (hh) {
    const w = num(hh.w), hgt = num(hh.h);
    const time = get("heatTime"), deaths = get("heatDeaths"), stuck = get("heatStuck");
    if (time instanceof Float32Array && deaths instanceof Uint32Array && stuck instanceof Float32Array && time.length === w * hgt && deaths.length === w * hgt && stuck.length === w * hgt)
      heat = { cell: num(hh.cell, 128), x0: num(hh.x0), y0: num(hh.y0), w, h: hgt, time, deaths, stuck };
  }
  return {
    id: String(h.id ?? ""),
    map: String(h.map ?? ""),
    mapInstalled: h.mapInstalled !== false,
    hasTrace: !!h.hasTrace,
    start: num(h.start, t[0] ?? 0),
    end: num(h.end, t[t.length - 1] ?? 0),
    players,
    rounds: (Array.isArray(h.rounds) ? h.rounds : []).map((r: Record<string, unknown>) => ({ start: num(r.start), end: num(r.end) })),
    kills: (Array.isArray(h.kills) ? h.kills : []).map((k: Record<string, unknown>) => {
      const f = k.fall as Record<string, unknown> | null;
      return {
        t: num(k.t), killer: num(k.killer), victim: num(k.victim), headshot: !!k.headshot, weapon: String(k.weapon ?? ""), cause: String(k.cause ?? ""),
        teamKill: !!(k.team_kill ?? k.teamKill), at: vec(k.at),
        fall: f && typeof f === "object" ? { from: vec(f.from), fromT: num(f.from_t ?? f.fromT), drop: num(f.drop), speed: num(f.speed) } : null,
      };
    }),
    stuck: (Array.isArray(h.stuck) ? h.stuck : []).map((s: Record<string, unknown>) => ({ id: num(s.id), from: num(s.from), to: num(s.to), at: vec(s.at) })),
    chat: (Array.isArray(h.chat) ? h.chat : []).map((c: Record<string, unknown>) => ({ t: num(c.t), id: num(c.id), text: String(c.text ?? c.message ?? "") })),
    heat,
    t,
    ...cols,
  };
}

/** The sample index at or just before time `at` for a player (binary search), or -1. */
export function sampleAt(r: Replay, p: Player, at: number): number {
  let lo = p.start, hi = p.start + p.count - 1;
  if (p.count === 0 || r.t[lo] > at) return -1;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (r.t[mid] <= at) lo = mid;
    else hi = mid - 1;
  }
  return lo;
}

/** A player's state at time `at`: position (interpolated), yaw, pitch, ducking, alive. */
export function stateAt(r: Replay, p: Player, at: number) {
  const i = sampleAt(r, p, at);
  if (i < 0) return null;
  // samples come every 0.1 s while alive: a gap means dead (or not in the game)
  const last = p.start + p.count - 1;
  if (at - r.t[i] > 0.6) return null;
  const j = i < last && r.t[i + 1] - r.t[i] < 0.6 ? i + 1 : i;
  const f = j === i ? 0 : Math.min(1, (at - r.t[i]) / (r.t[j] - r.t[i]));
  const lerp = (a: Float32Array) => a[i] + (a[j] - a[i]) * f;
  let dy = r.yaw[j] - r.yaw[i];
  if (dy > 180) dy -= 360;
  if (dy < -180) dy += 360;
  return { pos: [lerp(r.x), lerp(r.y), lerp(r.z)] as Vec3, yaw: r.yaw[i] + dy * f, pitch: lerp(r.pitch), duck: r.duck[i] > 0, hp: r.hp[i], i };
}

// ---- brain trace ----------------------------------------------------------------------------

export type Trace = {
  offset: number;
  columns: string[];
  tasks: string[];
  bots: Map<number, (string | number)[][]>;
  why: { t: number; id: number; why: string; at: Vec3 | null }[];
  matches: boolean;
};

export function toTrace(raw: Record<string, unknown>): Trace {
  const bots = new Map<number, (string | number)[][]>();
  for (const b of Array.isArray(raw.bots) ? raw.bots : []) {
    const rows = Array.isArray(b?.rows) ? (b.rows as (string | number)[][]).filter((r) => Array.isArray(r)) : [];
    rows.sort((a, c) => num(a[0]) - num(c[0]));
    bots.set(num(b?.id), rows);
  }
  return {
    offset: num(raw.offset),
    columns: Array.isArray(raw.columns) ? raw.columns.map(String) : [],
    tasks: Array.isArray(raw.tasks) ? raw.tasks.map(String) : [],
    bots,
    why: (Array.isArray(raw.why) ? raw.why : []).map((w: Record<string, unknown>) => ({ t: num(w.t), id: num(w.id), why: String(w.why ?? ""), at: Array.isArray(w.at) ? vec(w.at) : null })),
    matches: raw.matches !== false,
  };
}

/** The trace row for a bot at time `at` (the last one at or before it, within 2 s). */
export function traceRow(tr: Trace, id: number, at: number): Record<string, string | number> | null {
  const rows = tr.bots.get(id);
  if (!rows?.length) return null;
  const t = at + tr.offset;
  let lo = 0, hi = rows.length - 1;
  if (num(rows[0][0]) > t) return null;
  while (lo < hi) {
    const mid = (lo + hi + 1) >> 1;
    if (num(rows[mid][0]) <= t) lo = mid;
    else hi = mid - 1;
  }
  if (t - num(rows[lo][0]) > 2) return null;
  const o: Record<string, string | number> = {};
  tr.columns.forEach((c, k) => (o[c] = rows[lo][k]));
  return o;
}
