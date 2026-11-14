// P5: the outliner, Unreal-style. Everything in the map as rows with columns (Name, Type,
// Texture; click a heading to sort): folders (groups, with folders inside), brush entities
// with their brushes, point entities, world brushes. Type icons, an eye (show/hide in the
// views) and a lock per row, hidden rows dimmed. Find by name, type, texture or number, a type
// filter, counts. Picks follow the views both ways (Ctrl/Shift-click adds, Shift-click a
// range); double-click looks at it; ✎ or F2 renames; drag rows onto a folder; right-click
// for everything else.

import { useMemo, useRef, useState, type MouseEvent as RMouseEvent, type ReactNode } from "react";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import type { DocBrush, DocEntity, DocGroup } from "./editorApi";
import { duplicateWith, frame, selectIds } from "./commands";
import { deleteSelection, hideSelection, isolateSelection } from "./Panels";
import { cmd2, useP5 } from "./p5store";
import { keyOf, originOf, useEditor } from "./store";

const PAGE = 300;
type Ids = { brushes: number[]; entities: number[] };
type Row =
  | { kind: "group"; key: string; depth: number; g: DocGroup; ids: Ids; count: number }
  | { kind: "entity"; key: string; depth: number; e: DocEntity; open: boolean }
  | { kind: "brush"; key: string; depth: number; b: DocBrush; inEntity: boolean }
  | { kind: "head"; key: string; label: string; count: number; dropToTop: boolean }
  | { kind: "more"; key: string; left: number; section: string };
type Filter = "all" | "entities" | "brushes" | "lights" | "spawns" | "triggers" | "hidden" | "locked";
type SortCol = "name" | "type" | "texture";

const FILTERS: [Filter, string][] = [
  ["all", "All"],
  ["entities", "Entities"],
  ["brushes", "Brushes"],
  ["lights", "Lights"],
  ["spawns", "Spawns"],
  ["triggers", "Triggers"],
  ["hidden", "Hidden"],
  ["locked", "Locked"],
];

// ---- icons ---------------------------------------------------------------------------------

const P: Record<string, string> = {
  folder: "M2 4h5l1.5 2H14v7H2z",
  folderOpen: "M2 4h5l1.5 2H14v1H4.5L2 13zM4.5 8H15l-2.5 5H2z",
  brush: "M8 1.5l6 3.2v6.6l-6 3.2-6-3.2V4.7zm0 1.7L3.6 5.5 8 7.8l4.4-2.3zM3 6.7v4l4.3 2.3V9zm10 0L8.7 9v4l4.3-2.3z",
  solid: "M2 2h12v12H2zm2 2v8h8V4z",
  light: "M8 1a4.5 4.5 0 0 1 2.6 8.2V11H5.4V9.2A4.5 4.5 0 0 1 8 1zM5.5 12h5v1.2h-5zm1 2h3v1h-3z",
  spawn: "M8 1.5a2.3 2.3 0 1 1 0 4.6 2.3 2.3 0 0 1 0-4.6zM4.5 7.5h7l-1 4H9.6L9 15H7l-.6-3.5H5.5z",
  point: "M8 1a5 5 0 0 1 5 5c0 3.8-5 9-5 9S3 9.8 3 6a5 5 0 0 1 5-5zm0 3a2 2 0 1 0 0 4 2 2 0 0 0 0-4z",
  sound: "M2 6h3l4-3v10L5 10H2zm9 .5a2.5 2.5 0 0 1 0 3l.8.8a3.6 3.6 0 0 0 0-4.6zm1.7-1.7a5 5 0 0 1 0 6.4l.8.8a6.1 6.1 0 0 0 0-8z",
  trigger: "M2 2h12v12H2zm1.5 1.5v9h9v-9z M5 5h6v6H5z",
  eye: "M8 3.5c3.4 0 6 3 7 4.5-1 1.5-3.6 4.5-7 4.5S2 9.5 1 8c1-1.5 3.6-4.5 7-4.5zm0 2a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z",
  eyeOff: "M2.3 1.3l12.4 12.4-1 1-2.3-2.3A7.6 7.6 0 0 1 8 12.5C4.6 12.5 2 9.5 1 8a12 12 0 0 1 3-3.2L1.3 2.3zM8 3.5c3.4 0 6 3 7 4.5a12 12 0 0 1-2.2 2.5L10.4 8.1A2.5 2.5 0 0 0 7.9 5.6L6.4 4.1A7 7 0 0 1 8 3.5z",
  lock: "M5 7V5a3 3 0 0 1 6 0v2h1v7H4V7zm1.5 0h3V5a1.5 1.5 0 0 0-3 0z",
  // NPC-editor: a person with a speech bubble (ls_npc), a pointing hand (ls_interactable)
  npc: "M5.5 4a2 2 0 1 1 0 4 2 2 0 0 1 0-4zM2 13.5c0-2.2 1.6-4 3.5-4s3.5 1.8 3.5 4zM9 1.5h5.5v4H12l-1.5 1.5V5.5H9z",
  use: "M6 1.5a1 1 0 0 1 1 1V7l.5-.2V5.5a1 1 0 0 1 2 0v1.6l.5-.1a1 1 0 0 1 1.5.9v.4l.3-.1a1 1 0 0 1 1.2 1V12c0 1.7-1.3 3-3 3H8.3a3 3 0 0 1-2.4-1.2L3 10a1 1 0 0 1 1.5-1.3L5 9.2V2.5a1 1 0 0 1 1-1z",
  unlock: "M5 7V5a3 3 0 0 1 5.8-1l-1.4.6A1.5 1.5 0 0 0 6.5 5v2H12v7H4V7z",
};
function Icon({ d, title, className }: { d: string; title?: string; className?: string }) {
  return (
    <svg viewBox="0 0 16 16" width="13" height="13" fill="currentColor" aria-hidden={title ? undefined : true} className={className}>
      {title && <title>{title}</title>}
      <path d={P[d]} fillRule="evenodd" />
    </svg>
  );
}
function iconOf(e: DocEntity): string {
  const c = e.classname;
  if (c === "ls_interactable_brush") return "use";
  if (e.brushes.length) return /^trigger_|^func_(buyzone|bomb_target|hostage_rescue|vip_safetyzone|escapezone)/.test(c) ? "trigger" : "solid";
  if (c.startsWith("light")) return "light";
  if (c === "ls_npc") return "npc";
  if (c === "ls_interactable" || c === "ls_interactable_brush") return "use";
  if (/^info_player_|^info_vip_start|^hostage_entity|^monster_/.test(c)) return "spawn";
  if (/^ambient_|^env_sound/.test(c)) return "sound";
  return "point";
}
const isLight = (e: DocEntity) => e.classname.startsWith("light");
const isSpawn = (e: DocEntity) => /^info_player_|^info_vip_start$|^hostage_entity$/.test(e.classname);
const isTrigger = (e: DocEntity) => /^trigger_|^func_(buyzone|bomb_target|hostage_rescue|vip_safetyzone|escapezone|ladder|water)$/.test(e.classname);

/** A stable number per object (Parity A: a row's data is "the same" while its object is). */
const objIds = new WeakMap<object, number>();
let nextObjId = 1;
function objId(o: object): number {
  let i = objIds.get(o);
  if (!i) objIds.set(o, (i = nextObjId++));
  return i;
}

const dims = (b: DocBrush) => b.max.map((x, i) => Math.round(x - b.min[i])).join("×");

export function OutlinePanel({ extra }: { extra?: ReactNode }) {
  const doc = useEditor((s) => s.doc);
  const sel = useEditor((s) => s.sel);
  const hidden = useEditor((s) => s.hidden);
  const meshes = useEditor((s) => s.brushes);
  const locked = useP5((s) => s.locked);
  const [q, setQ] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [sort, setSort] = useState<{ col: SortCol; up: boolean } | null>(null);
  const [open, setOpen] = useState<Record<string, boolean>>({ world: false });
  const [more, setMore] = useState<Record<string, number>>({});
  const [renaming, setRenaming] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState<string | null>(null);
  const [ghost, setGhost] = useState<{ x: number; y: number; label: string; ok: boolean } | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; row: Row } | null>(null);
  const anchor = useRef<string | null>(null);
  const tree = useRef<HTMLDivElement>(null);
  // Parity A (PerfAudit #1): each row's element is kept while nothing it shows has changed, so an
  // edit re-renders only the rows it touched (the document's unchanged entities and brushes keep
  // their objects). Handlers read the latest functions through `live`, never a stale render's.
  const rowCache = useRef(new Map<string, { sig: string; el: JSX.Element }>());
  const live = useRef<{ rowDown: (ev: React.PointerEvent, r: Row) => void; pick: (r: Row, ev: { shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }) => void; look: (r: Row) => void; rename: (r: Row, v: string) => Promise<void>; toggleHide: (r: Row) => void; toggleLock: (r: Row) => void } | null>(null);

  const query = q.trim().toLowerCase();
  const texOf = (bid: number) => meshes.get(bid)?.textures ?? [];
  const firstTex = (b: DocBrush) => texOf(b.id)[0] ?? "";
  const entTex = (e: DocEntity) => (e.brushes.length ? texOf(e.brushes[0])[0] ?? "" : keyOf(e.keys, "model") ?? "");
  const nameOfE = (e: DocEntity) => keyOf(e.keys, "targetname") ?? "";

  const rows: Row[] = useMemo(() => {
    if (!doc) return [];
    const textB = (b: DocBrush) => `brush ${b.id} ${texOf(b.id).join(" ")}`.toLowerCase();
    const textE = (e: DocEntity) => `${e.classname} ${nameOfE(e)} ${keyOf(e.keys, "target") ?? ""} entity ${e.id} ${entTex(e)}`.toLowerCase();
    const filtB = (b: DocBrush) =>
      (filter === "all" || filter === "brushes" || (filter === "hidden" && hidden.brushes.has(b.id)) || (filter === "locked" && locked.brushes.has(b.id))) && (!query || textB(b).includes(query));
    const filtE = (e: DocEntity) => {
      const kind =
        filter === "all" ||
        filter === "entities" ||
        (filter === "lights" && isLight(e)) ||
        (filter === "spawns" && isSpawn(e)) ||
        (filter === "triggers" && isTrigger(e)) ||
        (filter === "hidden" && hidden.entities.has(e.id)) ||
        (filter === "locked" && locked.entities.has(e.id));
      return kind && (!query || textE(e).includes(query) || e.brushes.some((id) => textB(doc.brushes.find((b) => b.id === id) ?? ({ id } as DocBrush)).includes(query)));
    };
    const cmp = <T,>(key: (x: T) => string) => (a: T, b: T) => {
      if (!sort) return 0;
      const r = key(a).localeCompare(key(b), undefined, { numeric: true, sensitivity: "base" });
      return sort.up ? r : -r;
    };
    const sortE = (l: DocEntity[]) => (sort ? [...l].sort(cmp<DocEntity>((e) => (sort.col === "name" ? nameOfE(e) || e.classname : sort.col === "type" ? e.classname : entTex(e)))) : l);
    const sortB = (l: DocBrush[]) => (sort ? [...l].sort(cmp<DocBrush>((b) => (sort.col === "texture" ? firstTex(b) : sort.col === "name" ? String(b.id).padStart(8, "0") : "brush"))) : l);
    const childGroups = (p: number | null) => doc.groups.filter((g) => g.parent === p).sort(cmp<DocGroup>((g) => g.name));
    const entsIn = (g: number | null) => doc.entities.filter((e) => e.group === g && filtE(e));
    const brushesIn = (g: number | null) => doc.brushes.filter((b) => b.entity == null && b.group === g && filtB(b));
    const members = (g: number): Ids => {
      const inside = new Set([g]);
      let grew = true;
      while (grew) {
        grew = false;
        for (const x of doc.groups) if (x.parent != null && inside.has(x.parent) && !inside.has(x.id)) {
          inside.add(x.id);
          grew = true;
        }
      }
      return { brushes: doc.brushes.filter((b) => b.entity == null && b.group != null && inside.has(b.group)).map((b) => b.id), entities: doc.entities.filter((e) => e.group != null && inside.has(e.group)).map((e) => e.id) };
    };
    const has = (g: number): boolean => entsIn(g).length > 0 || brushesIn(g).length > 0 || childGroups(g).some((c) => has(c.id));
    const out: Row[] = [];
    const page = (k: string) => more[k] ?? PAGE;
    const pushEntity = (e: DocEntity, depth: number) => {
      const key = `e${e.id}`;
      const isOpen = !!open[key];
      out.push({ kind: "entity", key, depth, e, open: isOpen });
      if (isOpen) for (const id of e.brushes) {
        const b = doc.brushes.find((x) => x.id === id);
        if (b) out.push({ kind: "brush", key: `b${b.id}`, depth: depth + 1, b, inEntity: true });
      }
    };
    const pushGroup = (g: DocGroup, depth: number) => {
      if (filter !== "all" || query) if (!has(g.id) && !(query && g.name.toLowerCase().includes(query))) return;
      const key = `g${g.id}`;
      const ids = members(g.id);
      out.push({ kind: "group", key, depth, g, ids, count: ids.brushes.length + ids.entities.length });
      // a search opens the folders its matches are in
      if (!open[key] && !query && filter === "all") return;
      for (const c of childGroups(g.id)) pushGroup(c, depth + 1);
      const es = sortE(entsIn(g.id));
      const bs = sortB(brushesIn(g.id));
      const lim = page(key);
      es.slice(0, lim).forEach((e) => pushEntity(e, depth + 1));
      bs.slice(0, Math.max(0, lim - es.length)).forEach((b) => out.push({ kind: "brush", key: `b${b.id}`, depth: depth + 1, b, inEntity: false }));
      if (es.length + bs.length > lim) out.push({ kind: "more", key: `m${key}`, left: es.length + bs.length - lim, section: key });
    };
    const top = childGroups(null);
    if (top.length) {
      out.push({ kind: "head", key: "h-groups", label: "Folders", count: top.length, dropToTop: false });
      if (open["groups"] ?? true) top.forEach((g) => pushGroup(g, 0));
    }
    const loose = sortE(entsIn(null));
    const section = (key: string, label: string, list: (() => void)[], total: number) => {
      out.push({ kind: "head", key: `h-${key}`, label, count: total, dropToTop: true });
      if (!(open[key] ?? true)) return;
      const lim = page(key);
      list.slice(0, lim).forEach((f) => f());
      if (total > lim) out.push({ kind: "more", key: `m-${key}`, left: total - lim, section: key });
    };
    const solid = loose.filter((e) => e.brushes.length);
    const point = loose.filter((e) => !e.brushes.length);
    section("solid", "Brush entities", solid.map((e) => () => pushEntity(e, 0)), solid.length);
    section("point", "Point entities", point.map((e) => () => pushEntity(e, 0)), point.length);
    const world = sortB(brushesIn(null));
    section("world", "World brushes", world.map((b) => () => out.push({ kind: "brush", key: `b${b.id}`, depth: 0, b, inEntity: false })), world.length);
    return out;
  }, [doc, filter, query, hidden, locked, meshes, open, more, sort]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!doc) return null;
  const s = useEditor.getState();
  const p5 = useP5.getState();
  const pickedE = new Set(sel.entities);
  const pickedB = new Set(sel.brushes);
  const idsOf = (r: Row): Ids => (r.kind === "group" ? r.ids : r.kind === "entity" ? { brushes: [], entities: [r.e.id] } : r.kind === "brush" ? { brushes: [r.b.id], entities: [] } : { brushes: [], entities: [] });
  const isOn = (r: Row) => {
    const ids = idsOf(r);
    const n = ids.brushes.length + ids.entities.length;
    return n > 0 && ids.brushes.every((b) => pickedB.has(b)) && ids.entities.every((e) => pickedE.has(e));
  };
  const selectable = rows.filter((r) => r.kind === "group" || r.kind === "entity" || r.kind === "brush");

  const pick = (r: Row, ev: { shiftKey: boolean; ctrlKey: boolean; metaKey: boolean }) => {
    const ids = idsOf(r);
    const cur = s.sel;
    if (ev.shiftKey && anchor.current) {
      // a range from the last clicked row
      const a = selectable.findIndex((x) => x.key === anchor.current);
      const b = selectable.findIndex((x) => x.key === r.key);
      if (a >= 0 && b >= 0) {
        const range = selectable.slice(Math.min(a, b), Math.max(a, b) + 1).map(idsOf);
        s.select({ brushes: [...new Set([...cur.brushes, ...range.flatMap((x) => x.brushes)])], entities: [...new Set([...cur.entities, ...range.flatMap((x) => x.entities)])], face: null });
        return;
      }
    }
    anchor.current = r.key;
    if (!(ev.ctrlKey || ev.metaKey)) return s.select({ ...ids, face: null });
    if (isOn(r)) s.select({ brushes: cur.brushes.filter((b) => !ids.brushes.includes(b)), entities: cur.entities.filter((e) => !ids.entities.includes(e)), face: null });
    else s.select({ brushes: [...new Set([...cur.brushes, ...ids.brushes])], entities: [...new Set([...cur.entities, ...ids.entities])], face: null });
  };
  const look = (r: Row) => {
    const ids = idsOf(r);
    s.select({ ...ids, face: null });
    if (r.kind === "entity" && !r.e.brushes.length) {
      const o = originOf(r.e.keys);
      if (o) return useTools.getState().look(o as Vec3, 64);
    }
    frame(false);
  };

  const hideIds = (ids: Ids, hide: boolean) => {
    const brushes = new Set(hidden.brushes);
    const entities = new Set(hidden.entities);
    ids.brushes.forEach((b) => (hide ? brushes.add(b) : brushes.delete(b)));
    ids.entities.forEach((e) => (hide ? entities.add(e) : entities.delete(e)));
    s.set({ hidden: { brushes, entities } });
  };
  const lockIds = (ids: Ids, lock: boolean) => {
    const brushes = new Set(locked.brushes);
    const entities = new Set(locked.entities);
    ids.brushes.forEach((b) => (lock ? brushes.add(b) : brushes.delete(b)));
    ids.entities.forEach((e) => (lock ? entities.add(e) : entities.delete(e)));
    p5.set({ locked: { brushes, entities } });
    if (lock) s.select({ brushes: s.sel.brushes.filter((b) => !brushes.has(b)), entities: s.sel.entities.filter((e) => !entities.has(e)), face: null });
  };
  const hiddenOf = (r: Row) => (r.kind === "group" ? r.g.hidden : r.kind === "entity" ? hidden.entities.has(r.e.id) : r.kind === "brush" ? hidden.brushes.has(r.b.id) : false);
  const lockedOf = (r: Row) => (r.kind === "group" ? r.g.locked : r.kind === "entity" ? locked.entities.has(r.e.id) : r.kind === "brush" ? locked.brushes.has(r.b.id) : false);
  const toggleHide = (r: Row) => (r.kind === "group" ? void s.cmd({ op: "group_flags", group: r.g.id, hidden: !r.g.hidden }) : hideIds(idsOf(r), !hiddenOf(r)));
  const toggleLock = (r: Row) => (r.kind === "group" ? void s.cmd({ op: "group_flags", group: r.g.id, locked: !r.g.locked }) : lockIds(idsOf(r), !lockedOf(r)));

  // drag rows (the picked set when a picked row is dragged) onto a folder. Pointer events, not
  // HTML5 drag-and-drop (that isn't dependable in the app's web view).
  const suppressClick = useRef(false);
  const rowDown = (ev: React.PointerEvent, r: Row) => {
    if (ev.button !== 0 || (r.kind === "brush" && r.inEntity)) return;
    if ((ev.target as HTMLElement).closest("button, input")) return;
    const start = { x: ev.clientX, y: ev.clientY };
    const payload = r.kind === "group" ? { brushes: [] as number[], entities: [] as number[], groups: [r.g.id] } : isOn(r) ? { brushes: s.sel.brushes, entities: s.sel.entities, groups: [] as number[] } : { ...idsOf(r), groups: [] as number[] };
    const label = r.kind === "group" ? r.g.name : payload.brushes.length + payload.entities.length > 1 ? `${payload.brushes.length + payload.entities.length} things` : r.kind === "entity" ? nameOfE(r.e) || r.e.classname : `Brush ${r.kind === "brush" ? r.b.id : ""}`;
    let on = false;
    const targetAt = (x: number, y: number) => (document.elementFromPoint(x, y) as HTMLElement | null)?.closest<HTMLElement>("[data-ol-drop]") ?? null;
    const move = (m: PointerEvent) => {
      if (!on && Math.hypot(m.clientX - start.x, m.clientY - start.y) < 6) return;
      on = true;
      m.preventDefault();
      const t = targetAt(m.clientX, m.clientY);
      setDragOver(t?.dataset.olDrop ?? null);
      setGhost({ x: m.clientX, y: m.clientY, label, ok: !!t });
    };
    const up = async (u: PointerEvent) => {
      window.removeEventListener("pointermove", move, true);
      window.removeEventListener("pointerup", up, true);
      window.removeEventListener("pointercancel", up, true);
      if (u.type === "pointercancel") {
        setGhost(null);
        setDragOver(null);
        return;
      }
      setGhost(null);
      setDragOver(null);
      if (!on) return;
      suppressClick.current = true;
      setTimeout(() => (suppressClick.current = false), 0);
      const t = targetAt(u.clientX, u.clientY);
      if (!t) return;
      const g = t.dataset.olGroup;
      await cmd2({ op: "move_to_group", group: g === "top" ? null : Number(g), ...payload });
    };
    window.addEventListener("pointermove", move, true);
    window.addEventListener("pointerup", up, true);
    window.addEventListener("pointercancel", up, true);
  };
  const dropOn = (key: string, group: number | null) => ({ "data-ol-drop": key, "data-ol-group": group == null ? "top" : String(group) });

  const rename = async (r: Row, value: string) => {
    setRenaming(null);
    const v = value.trim();
    if (r.kind === "group" && v && v !== r.g.name) await cmd2({ op: "rename_group", group: r.g.id, name: v });
    if (r.kind === "entity" && v !== nameOfE(r.e)) await s.cmd({ op: "set_key", entity: r.e.id, key: "targetname", value: v });
  };

  const sortBy = (col: SortCol) => setSort((cur) => (!cur || cur.col !== col ? { col, up: true } : cur.up ? { col, up: false } : null));
  const arrow = (col: SortCol) => (sort?.col === col ? (sort.up ? " ▲" : " ▼") : "");
  const nHidden = hidden.brushes.size + hidden.entities.size;
  const nLocked = locked.brushes.size + locked.entities.size;
  const total = doc.entities.length + doc.brushes.filter((b) => b.entity == null).length;

  const rowEl = (r: Row) => {
    if (r.kind === "head") {
      const k = r.key.slice(2);
      const isOpen = open[k] ?? true;
      return (
        <div key={r.key} className={`ed-ol-head${dragOver === r.key ? " drop" : ""}`} {...(r.dropToTop ? dropOn(r.key, null) : {})} title={r.dropToTop ? "Drop things here to take them out of their folder" : undefined}>
          <button type="button" className="ed-ol-twist" aria-expanded={isOpen} aria-label={`${isOpen ? "Fold" : "Unfold"} ${r.label}`} onClick={() => setOpen((o) => ({ ...o, [k]: !isOpen }))}>
            {isOpen ? "▾" : "▸"}
          </button>
          <span>{r.label}</span>
          <span className="muted small-note">{r.count}</span>
        </div>
      );
    }
    if (r.kind === "more") {
      return (
        <button key={r.key} type="button" className="btn small ed-ol-more" onClick={() => setMore((x) => ({ ...x, [r.section]: (x[r.section] ?? PAGE) + PAGE }))}>
          Show more ({r.left} left)
        </button>
      );
    }
    const on = isOn(r);
    const hid = hiddenOf(r);
    const lck = lockedOf(r);
    const name = r.kind === "group" ? r.g.name : r.kind === "entity" ? nameOfE(r.e) || r.e.classname : `Brush ${r.b.id}`;
    const type = r.kind === "group" ? `Folder · ${r.count}` : r.kind === "entity" ? (r.e.brushes.length ? `${r.e.classname} · ${r.e.brushes.length}` : r.e.classname) : dims(r.b);
    const tex = r.kind === "entity" ? entTex(r.e) : r.kind === "brush" ? texOf(r.b.id).slice(0, 2).join(", ") + (texOf(r.b.id).length > 2 ? "…" : "") : "";
    const icon = r.kind === "group" ? (open[r.key] ? "folderOpen" : "folder") : r.kind === "entity" ? iconOf(r.e) : "brush";
    const canOpen = r.kind === "group" || (r.kind === "entity" && r.e.brushes.length > 0);
    const isOpen = !!open[r.key];
    const data = r.kind === "group" ? r.g : r.kind === "entity" ? r.e : r.b;
    const sig = [objId(data), r.depth, on, hid, lck, dragOver === r.key, isOpen, renaming === r.key, name, type, tex, icon, r.kind === "brush" ? r.inEntity : "", r.kind === "group" ? `${r.count}:${objId(r.ids)}` : ""].join("|");
    const hit = rowCache.current.get(r.key);
    if (hit && hit.sig === sig) return hit.el;
    const el = (
      <div
        key={r.key}
        className={`ed-ol-row ed-ol-${r.kind}${on ? " on" : ""}${hid ? " dim" : ""}${lck ? " locked" : ""}${dragOver === r.key ? " drop" : ""}`}
        style={{ paddingLeft: 2 + r.depth * 14 }}
        onPointerDown={(ev) => live.current!.rowDown(ev, r)}
        {...(r.kind === "group" ? dropOn(r.key, r.g.id) : {})}
        onClick={(ev) => !suppressClick.current && live.current!.pick(r, ev)}
        onDoubleClick={() => live.current!.look(r)}
        onContextMenu={(ev: RMouseEvent) => {
          ev.preventDefault();
          if (!on) live.current!.pick(r, { shiftKey: false, ctrlKey: false, metaKey: false });
          setMenu({ x: ev.clientX, y: ev.clientY, row: r });
        }}
        role="treeitem"
        aria-selected={on}
        aria-expanded={canOpen ? isOpen : undefined}
        tabIndex={-1}
        data-key={r.key}
      >
        {canOpen ? (
          <button type="button" className="ed-ol-twist" aria-label={isOpen ? "Fold" : "Unfold"} onClick={(ev) => (ev.stopPropagation(), setOpen((o) => ({ ...o, [r.key]: !isOpen })))} onDoubleClick={(ev) => ev.stopPropagation()}>
            {isOpen ? "▾" : "▸"}
          </button>
        ) : (
          <span className="ed-ol-twist" />
        )}
        <Icon d={icon} className={`ed-ol-icon ${icon}`} />
        <span className="ed-ol-name">
          {renaming === r.key ? (
            <input
              className="ed-ol-rename"
              autoFocus
              defaultValue={r.kind === "group" ? r.g.name : r.kind === "entity" ? nameOfE(r.e) : ""}
              placeholder={r.kind === "entity" ? "targetname" : "name"}
              onClick={(e) => e.stopPropagation()}
              onKeyDown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") void live.current!.rename(r, (e.target as HTMLInputElement).value);
                if (e.key === "Escape") setRenaming(null);
              }}
              onBlur={(e) => void live.current!.rename(r, e.target.value)}
            />
          ) : (
            name
          )}
        </span>
        <span className="ed-ol-type muted">{type}</span>
        <span className="ed-ol-tex muted">{tex}</span>
        {r.kind !== "brush" || !r.inEntity ? (
          <span className="ed-ol-toggles">
            <button type="button" className={`ed-ol-eye${hid ? " off" : ""}`} title={hid ? "Hidden: show it" : "Hide it in the views"} aria-label={hid ? "Show" : "Hide"} aria-pressed={hid} onClick={(ev) => (ev.stopPropagation(), live.current!.toggleHide(r))} onDoubleClick={(ev) => ev.stopPropagation()}>
              <Icon d={hid ? "eyeOff" : "eye"} />
            </button>
            <button type="button" className={`ed-ol-lock${lck ? " on" : ""}`} title={lck ? "Locked (can't be picked in the views): unlock" : "Lock (can't be picked in the views)"} aria-label={lck ? "Unlock" : "Lock"} aria-pressed={lck} onClick={(ev) => (ev.stopPropagation(), live.current!.toggleLock(r))} onDoubleClick={(ev) => ev.stopPropagation()}>
              <Icon d={lck ? "lock" : "unlock"} />
            </button>
          </span>
        ) : (
          <span className="ed-ol-toggles" />
        )}
      </div>
    );
    rowCache.current.set(r.key, { sig, el });
    return el;
  };
  live.current = { rowDown, pick, look, rename, toggleHide, toggleLock };

  const menuItems = (r: Row): ([string, () => void] | null | undefined)[] => {
    const ids = idsOf(r);
    const groups = doc.groups;
    return [
      ["Look at", () => look(r)],
      r.kind === "group" || r.kind === "entity" ? ["Rename… (F2)", () => setRenaming(r.key)] : undefined,
      ["Select same type", () => void cmd2({ op: "similar", ...ids, similar: { by: "class" } }).then((x) => x.select && selectIds(x.select))],
      null,
      ["Duplicate", () => void duplicateWith([s.grid, s.grid, 0])],
      ["Delete", () => deleteSelection()],
      null,
      [hiddenOf(r) ? "Show" : "Hide", () => toggleHide(r)],
      ["Hide everything else", () => isolateSelection()],
      ["Hide what's picked", () => hideSelection()],
      [lockedOf(r) ? "Unlock" : "Lock", () => toggleLock(r)],
      null,
      ["New folder from what's picked", () => void s.cmd({ op: "group", name: `Folder ${groups.length + 1}`, brushes: s.sel.brushes, entities: s.sel.entities })],
      ...groups.slice(0, 12).map((g): [string, () => void] => [`Move to “${g.name}”`, () => void cmd2({ op: "move_to_group", group: g.id, brushes: s.sel.brushes, entities: s.sel.entities, groups: r.kind === "group" ? [r.g.id] : [] })]),
      r.kind !== "group" && (r.kind === "entity" ? r.e.group != null : r.kind === "brush" && r.b.group != null) ? ["Take out of its folder", () => void cmd2({ op: "move_to_group", group: null, brushes: s.sel.brushes, entities: s.sel.entities, groups: [] })] : undefined,
      r.kind === "group" ? ["Ungroup (keep what's in it)", () => void s.cmd({ op: "ungroup", group: r.g.id })] : undefined,
    ];
  };

  return (
    <section className="tools-panel editor-outline ed-outline">
      <div className="ed-ol-top">
        <input type="search" placeholder="Find by name, type, texture or number" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Find in the map" />
        <select value={filter} onChange={(e) => setFilter(e.target.value as Filter)} aria-label="Show only" title="Show only one kind">
          {FILTERS.map(([id, l]) => (
            <option key={id} value={id}>
              {id === "hidden" && nHidden ? `${l} (${nHidden})` : id === "locked" && nLocked ? `${l} (${nLocked})` : l}
            </option>
          ))}
        </select>
      </div>
      {extra}
      <div className="ed-ol-cols" role="row">
        <button type="button" onClick={() => sortBy("name")} title="Sort by name (again: reverse, third time: map order)">
          Name{arrow("name")}
        </button>
        <button type="button" onClick={() => sortBy("type")}>Type{arrow("type")}</button>
        <button type="button" onClick={() => sortBy("texture")}>Texture{arrow("texture")}</button>
        <span className="ed-ol-cols-icons" aria-hidden="true">
          <Icon d="eye" />
          <Icon d="lock" />
        </span>
      </div>
      <div
        className="ed-ol-tree"
        ref={tree}
        role="tree"
        aria-label="Everything in the map"
        aria-multiselectable
        onKeyDown={(e) => {
          if (e.key === "F2") {
            const r = rows.find((x) => (x.kind === "entity" || x.kind === "group") && isOn(x));
            if (r) setRenaming(r.key);
          }
        }}
        tabIndex={0}
      >
        {rows.map(rowEl)}
      </div>
      <p className="small-note muted ed-ol-foot">
        {total} things · {sel.brushes.length + sel.entities.length} picked{nHidden ? ` · ${nHidden} hidden` : ""}. Double-click to look; right-click for more; drag onto a folder.
      </p>
      {ghost && (
        <div className="ed-dock-ghost" style={{ left: ghost.x + 12, top: ghost.y + 10, zIndex: 80 }}>
          {ghost.label}
          <span className="muted"> {ghost.ok ? "· into this folder" : "· onto a folder"}</span>
        </div>
      )}
      {menu && (
        <div className="ed-ctx-back" onClick={() => setMenu(null)} onContextMenu={(e) => (e.preventDefault(), setMenu(null))}>
          <ul className="ed-ctx" role="menu" style={{ left: Math.min(menu.x, window.innerWidth - 240), top: Math.min(menu.y, window.innerHeight - 380) }} onClick={(e) => e.stopPropagation()}>
            {menuItems(menu.row).filter((x) => x !== undefined).map((it, i) =>
              it === null ? (
                <li key={`s${i}`} className="sep" role="separator" />
              ) : (
                <li key={it[0]} role="none">
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setMenu(null);
                      it[1]();
                    }}
                  >
                    {it[0]}
                  </button>
                </li>
              ),
            )}
          </ul>
        </div>
      )}
    </section>
  );
}
