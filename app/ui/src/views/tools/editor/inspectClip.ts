// The Inspector's Copy and Paste, like Unreal's right-click Copy / Paste on a property: a
// Transform row (Location, Rotation / Turn by, Size, Scale) or the whole Transform, one key's
// value, or an entity's properties (every key but what makes it itself). A small clipboard of
// our own (the system clipboard gets the same as text). Each paste is one undo step, onto
// everything picked.

import type { Vec3 } from "../viewer/coords";
import { selectionIds } from "./EditorScene";
import { fgdNow } from "./fgd";
import { keyOf, originOf, selectionBounds, useEditor } from "./store";

export type XfRow = "location" | "rotation" | "size" | "scale";
export type Clip =
  | { kind: "transform"; location?: Vec3; rotation?: Vec3; size?: Vec3; scale?: number }
  | { kind: "value"; key: string; value: string }
  | { kind: "props"; from: string; keys: [string, string][] };

let clip: Clip | null = null;
const listeners = new Set<() => void>();
export const clipNow = () => clip;
export const onClip = (f: () => void) => (listeners.add(f), () => void listeners.delete(f));

/** Keys never copied with an entity's properties: what it is, where it is, its brush model,
 *  and its name (two entities with one name answer each other's triggers). */
export const OWN_KEYS = ["classname", "origin", "model", "targetname"];

function put(c: Clip, text: string) {
  clip = c;
  for (const f of listeners) f();
  try {
    void navigator.clipboard?.writeText(text).catch(() => {});
  } catch {
    /* no system clipboard here */
  }
}

const S = () => useEditor.getState();
const num = (x: number) => String(Math.round(x * 1000) / 1000);
const v3 = (v: Vec3) => v.map(num).join(" ");

function picked() {
  const s = S();
  return (s.doc?.entities ?? []).filter((e) => s.sel.entities.includes(e.id));
}
const pointEnts = () => picked().filter((e) => !e.brushes.length);

/** A point entity's angles as [roll, pitch, yaw] (Rotation's X Y Z), from angles or angle. */
export function anglesOf(keys: [string, string][]): Vec3 {
  const a = keyOf(keys, "angles");
  if (a) {
    const p = a.split(/\s+/).map(Number);
    return [Number.isFinite(p[2]) ? p[2] : 0, Number.isFinite(p[0]) ? p[0] : 0, Number.isFinite(p[1]) ? p[1] : 0];
  }
  return [0, 0, Number(keyOf(keys, "angle") ?? 0) || 0];
}

/** What the Transform shows for what's picked now. */
export function transformNow(): Extract<Clip, { kind: "transform" }> | null {
  const b = selectionBounds();
  const pts = pointEnts();
  const only = pts.length === 1 && !selectionIds().brushes.length ? pts[0] : null;
  const loc = only ? originOf(only.keys) : b ? (b[0].map((x, i) => (x + b[1][i]) / 2) as Vec3) : null;
  if (!loc) return null;
  const out: Extract<Clip, { kind: "transform" }> = { kind: "transform", location: loc };
  if (only) {
    out.rotation = anglesOf(only.keys);
    const sc = keyOf(only.keys, "scale");
    if (sc !== undefined && Number(sc) > 0) out.scale = Number(sc);
  } else if (b) out.size = b[1].map((x, i) => x - b[0][i]) as Vec3;
  return out;
}

export function copyTransform(row?: XfRow) {
  const t = transformNow();
  if (!t) return;
  const c: Extract<Clip, { kind: "transform" }> = { kind: "transform" };
  if (!row || row === "location") c.location = t.location;
  if (!row || row === "rotation") c.rotation = t.rotation;
  if (!row || row === "size") c.size = t.size;
  if (!row || row === "scale") c.scale = t.scale;
  const text = [c.location && `Location ${v3(c.location)}`, c.rotation && `Rotation ${v3(c.rotation)}`, c.size && `Size ${v3(c.size)}`, c.scale !== undefined && `Scale ${num(c.scale)}`].filter(Boolean).join("\n");
  put(c, text);
  S().notify(`Copied ${row ? rowWord(row) : "the Transform"}.`);
}

export const rowWord = (r: XfRow) => ({ location: "Location", rotation: "Rotation", size: "Size", scale: "Scale" })[r];

/** Can the clipboard's Transform (or one row of it) go onto what's picked? */
export function canPasteTransform(row?: XfRow) {
  const c = clip;
  if (c?.kind !== "transform" || !(S().sel.brushes.length || S().sel.entities.length)) return false;
  if (!row) return !!(c.location || c.rotation || c.size || c.scale !== undefined);
  return row === "location" ? !!c.location : row === "rotation" ? !!c.rotation && pointEnts().length > 0 : row === "size" ? !!c.size && !!selectionIds().brushes.length : c.scale !== undefined && pointEnts().length > 0;
}

/** Puts the copied Transform (or one row) onto what's picked: one undo step. Location moves
 *  what's picked so its middle (a point entity's origin) is there; Size stretches the brushes
 *  to it about their middle; Rotation and Scale set point entities' angles and scale keys. */
export async function pasteTransform(row?: XfRow) {
  const c = clip;
  if (c?.kind !== "transform") return;
  const s = S();
  const ids = selectionIds();
  const now = transformNow();
  if (!now) return;
  const merge = `paste-${Date.now()}`;
  const lock = s.textureLock;
  let did = 0;
  const want = (r: XfRow) => !row || row === r;
  // (the keys first, then the move: the move's pivot is where things are after it)
  const sets: [number, string, string][] = [];
  if (want("rotation") && c.rotation) {
    for (const e of pointEnts()) sets.push([e.id, "angles", `${num(c.rotation[1])} ${num(c.rotation[2])} ${num(c.rotation[0])}`], [e.id, "angle", ""]);
  }
  if (want("scale") && c.scale !== undefined) for (const e of pointEnts()) sets.push([e.id, "scale", num(c.scale)]);
  if (sets.length) {
    const r = await s.cmd({ op: "set_keys", sets, label: row ? `Paste ${rowWord(row)}` : "Paste Transform", merge });
    if (!r.error) did++;
  }
  if (want("size") && c.size && now.size && ids.brushes.length) {
    const f = c.size.map((x, i) => (now.size![i] > 0 && x > 0 ? x / now.size![i] : 1)) as Vec3;
    if (f.some((x) => Math.abs(x - 1) > 1e-6)) {
      const r = await s.cmd({ op: "transform", ...ids, scale: f, pivot: now.location, lock, merge });
      if (!r.error) did++;
    }
  }
  if (want("location") && c.location && now.location) {
    const d = c.location.map((x, i) => x - now.location![i]) as Vec3;
    if (d.some((x) => Math.abs(x) > 1e-6)) {
      const r = await s.cmd({ op: "transform", ...ids, translate: d, lock, merge });
      if (!r.error) did++;
    }
  }
  s.notify(did ? `Pasted ${row ? rowWord(row) : "the Transform"}.` : row === "rotation" || row === "scale" ? `Only point entities take a pasted ${rowWord(row)}.` : "Nothing to change.");
}

export function copyValue(key: string, value: string) {
  put({ kind: "value", key, value }, value);
  S().notify(`Copied ${key}.`);
}

/** The copied value into `key` of every picked entity (one undo step). */
export async function pasteValue(key: string) {
  const c = clip;
  if (c?.kind !== "value") return;
  const ents = picked();
  if (!ents.length) return;
  const r = await S().cmd({ op: "set_keys", sets: ents.map((e) => [e.id, key, c.value]), label: `Paste ${key}` });
  if (!r.error) S().notify(`Pasted into ${key}${ents.length > 1 ? ` on ${ents.length} entities` : ""}.`);
}

/** Copies an entity's properties: every key it has but classname, origin, its brush model and
 *  its name. */
export function copyProps(entityId?: number) {
  const s = S();
  const e = (s.doc?.entities ?? []).find((x) => x.id === (entityId ?? s.sel.entities[0]));
  if (!e) return s.notify("Pick an entity to copy its properties.", true);
  const keys = e.keys.filter(([k]) => !OWN_KEYS.includes(k.toLowerCase()));
  put({ kind: "props", from: e.classname, keys }, keys.map(([k, v]) => `"${k}" "${v}"`).join("\n"));
  s.notify(`Copied ${keys.length} propert${keys.length === 1 ? "y" : "ies"} of ${e.classname}.`);
}

/** Which of the copied keys a class takes: those its definition has (any key when the class
 *  isn't defined, or for the same class). */
export function keysFor(classname: string, keys: [string, string][], from: string): { take: [string, string][]; skip: string[] } {
  const def = fgdNow()?.get(classname);
  if (!def || classname === from) return { take: keys, skip: [] };
  const has = new Set(def.keys.map((k) => k.name.toLowerCase()));
  const take = keys.filter(([k]) => has.has(k.toLowerCase()));
  return { take, skip: keys.filter(([k]) => !has.has(k.toLowerCase())).map(([k]) => k) };
}

/** Pastes copied properties onto every picked entity (one undo step), skipping keys a class
 *  doesn't have, and says which. */
export async function pasteProps() {
  const c = clip;
  const s = S();
  if (c?.kind !== "props") return s.notify("Copy an entity's properties first (right-click in the Inspector, or Edit > Copy properties).", true);
  const ents = picked();
  if (!ents.length) return s.notify("Pick the entities to paste onto.", true);
  const sets: [number, string, string][] = [];
  const skipped = new Set<string>();
  for (const e of ents) {
    const { take, skip } = keysFor(e.classname, c.keys, c.from);
    for (const [k, v] of take) sets.push([e.id, k, v]);
    for (const k of skip) skipped.add(k);
  }
  if (!sets.length) return s.notify(`None of those keys go on ${ents.map((e) => e.classname).filter((x, i, a) => a.indexOf(x) === i).join(", ")}.`, true);
  const r = await s.cmd({ op: "set_keys", sets, label: "Paste properties" });
  if (r.error) return;
  const sk = [...skipped];
  s.notify(`Pasted properties onto ${ents.length} entit${ents.length === 1 ? "y" : "ies"}.${sk.length ? ` Skipped (not keys of that class): ${sk.slice(0, 8).join(", ")}${sk.length > 8 ? "…" : ""}.` : ""}`);
}
