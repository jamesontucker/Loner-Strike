// P5: what triggers what, worked out in the page from the open map's entities (the 3D arrows
// need it on every edit). The same rules as the app's editor/io.rs (the I/O panel and MCP
// editor_io read that one): target, killtarget, master, pathtarget, beams' ends,
// trigger_changetarget's new target and every name in a multi_manager ("name#2" fires "name"
// again, the value is the delay).

import type { DocEntity } from "./editorApi";
import { keyOf } from "./store";

export type IoKind = "fires" | "kills" | "master" | "names";
export type IoLink = { from: number; key: string; name: string; kind: IoKind; delay: number | null; to: number[] };

export const LINK_KEYS: [string, IoKind][] = [
  ["target", "fires"],
  ["killtarget", "kills"],
  ["master", "master"],
  ["pathtarget", "fires"],
  ["m_iszNewTarget", "names"],
  ["LightningStart", "names"],
  ["LightningEnd", "names"],
  ["LaserTarget", "names"],
  ["combattarget", "fires"],
];
// Keys that name another entity on some classes only (editor/io.rs CLASS_LINK_KEYS): a path's
// "message" fires a name when the train passes; on most entities it's text.
export const CLASS_LINK_KEYS: [string, string, IoKind][] = [
  ["path_track", "message", "fires"],
  ["path_track", "altpath", "names"],
  ["path_corner", "message", "fires"],
  ["func_guntarget", "message", "fires"],
  ["trigger_setorigin", "copypointer", "names"],
  ["game_zone_player", "intarget", "fires"],
  ["game_zone_player", "outtarget", "fires"],
  ["game_zone_player", "incount", "fires"],
  ["game_zone_player", "outcount", "fires"],
  ["func_trackchange", "train", "names"],
  ["func_trackchange", "toptrack", "names"],
  ["func_trackchange", "bottomtrack", "names"],
  ["func_trackautochange", "train", "names"],
  ["func_trackautochange", "toptrack", "names"],
  ["func_trackautochange", "bottomtrack", "names"],
  ["func_tanklaser", "laserentity", "names"],
];
/** trigger_random's numbered targets ("target0" to "target15"). */
export const randomTargetKey = (cls: string, key: string) => cls.startsWith("trigger_random") && /^target\d{1,2}$/.test(key) && Number(key.slice(6)) < 16;
const MM_OWN = new Set(["classname", "targetname", "origin", "spawnflags", "wait", "master", "angles", "angle"]);

export const KIND_COLOR: Record<IoKind, string> = { fires: "#ffd84a", kills: "#ff5f56", master: "#c58cff", names: "#5fd3e8" };
export const KIND_WORD: Record<IoKind, string> = { fires: "fires", kills: "kills", master: "waits on", names: "points at" };

const mmName = (k: string) => {
  const i = k.lastIndexOf("#");
  return i > 0 && /^\d+$/.test(k.slice(i + 1)) ? k.slice(0, i) : k;
};

export function ioLinks(ents: DocEntity[]): IoLink[] {
  const named = new Map<string, number[]>();
  for (const e of ents) {
    const n = keyOf(e.keys, "targetname");
    if (n) named.set(n, [...(named.get(n) ?? []), e.id]);
  }
  const out: IoLink[] = [];
  for (const e of ents) {
    const add = (key: string, name: string, kind: IoKind, delay: number | null) => {
      const n = name.trim();
      if (n) out.push({ from: e.id, key, name: n, kind, delay, to: named.get(n) ?? [] });
    };
    for (const [k, kind] of LINK_KEYS) {
      const v = keyOf(e.keys, k);
      if (v) add(k, v, kind, null);
    }
    for (const [c, k, kind] of CLASS_LINK_KEYS) {
      const v = c === e.classname ? keyOf(e.keys, k) : undefined;
      if (v) add(k, v, kind, null);
    }
    if (e.classname.startsWith("trigger_random")) for (const [k, v] of e.keys) if (randomTargetKey(e.classname, k)) add(k, v, "fires", null);
    if (e.classname === "multi_manager") {
      for (const [k, v] of e.keys) {
        if (MM_OWN.has(k) || LINK_KEYS.some(([l]) => l === k)) continue;
        const d = Number(v.trim());
        add(k, mmName(k), "fires", v.trim() !== "" && Number.isFinite(d) ? d : null);
      }
    }
  }
  return out;
}

/** What an entity sets off down the line, and what leads to it (not itself). */
export function chainOf(links: IoLink[], id: number): { down: Set<number>; up: Set<number> } {
  const walk = (down: boolean) => {
    const seen = new Set<number>();
    const todo = [id];
    while (todo.length) {
      const x = todo.pop()!;
      for (const l of links) {
        const next = down ? (l.from === x ? l.to : []) : l.to.includes(x) ? [l.from] : [];
        for (const n of next) if (n !== id && !seen.has(n)) (seen.add(n), todo.push(n));
      }
    }
    return seen;
  };
  return { down: walk(true), up: walk(false) };
}
