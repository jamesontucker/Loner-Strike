// P5: the outliner. Everything in the map as a tree: groups (with groups inside them), brush
// entities with their brushes, point entities, world brushes. Find by name, type, texture or
// number; picks follow the views both ways (Ctrl/Shift-click adds); rename (entities: their
// targetname; groups: their name); drag rows onto a group to put them in it; hide and lock
// per row.

import { useMemo, useState, type DragEvent, type ReactNode } from "react";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import type { DocBrush, DocEntity, DocGroup } from "./editorApi";
import { frame } from "./commands";
import { cmd2, useP5 } from "./p5store";
import { keyOf, originOf, useEditor } from "./store";

const PAGE = 200;
type Row = { kind: "group"; g: DocGroup } | { kind: "entity"; e: DocEntity } | { kind: "brush"; b: DocBrush };
type Filter = "all" | "entities" | "brushes" | "hidden" | "locked";

const dims = (b: DocBrush) => b.max.map((x, i) => Math.round(x - b.min[i])).join("×");

export function OutlinePanel({ extra }: { extra?: ReactNode }) {
  const doc = useEditor((s) => s.doc);
  const sel = useEditor((s) => s.sel);
  const hidden = useEditor((s) => s.hidden);
  const meshes = useEditor((s) => s.brushes);
  const locked = useP5((s) => s.locked);
  const [q, setQ] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const [open, setOpen] = useState<Record<string, boolean>>({ world: false });
  const [more, setMore] = useState<Record<string, number>>({});
  const [renaming, setRenaming] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState<string | null>(null);

  const query = q.trim().toLowerCase();
  const m = useMemo(() => {
    if (!doc) return null;
    const textOfB = (b: DocBrush) => `brush ${b.id} ${(meshes.get(b.id)?.textures ?? []).join(" ")}`.toLowerCase();
    const textOfE = (e: DocEntity) => `${e.classname} ${keyOf(e.keys, "targetname") ?? ""} ${keyOf(e.keys, "target") ?? ""} entity ${e.id}`.toLowerCase();
    const okB = (b: DocBrush) =>
      (filter === "all" || filter === "brushes" || (filter === "hidden" && hidden.brushes.has(b.id)) || (filter === "locked" && locked.brushes.has(b.id))) && (!query || textOfB(b).includes(query));
    const okE = (e: DocEntity) =>
      (filter === "all" || filter === "entities" || (filter === "hidden" && hidden.entities.has(e.id)) || (filter === "locked" && locked.entities.has(e.id))) &&
      (!query || textOfE(e).includes(query) || e.brushes.some((id) => textOfB(doc.brushes.find((b) => b.id === id) ?? ({ id } as DocBrush)).includes(query)));
    return { okB, okE };
  }, [doc, filter, query, hidden, locked, meshes]);
  if (!doc || !m) return null;
  const s = useEditor.getState();
  const p5 = useP5.getState();

  // the tree
  const childGroups = (parent: number | null) => doc.groups.filter((g) => g.parent === parent);
  const entsIn = (g: number | null) => doc.entities.filter((e) => e.group === g && m.okE(e));
  const brushesIn = (g: number | null) => doc.brushes.filter((b) => b.entity == null && b.group === g && m.okB(b));
  const groupHas = (g: number): boolean => entsIn(g).length > 0 || brushesIn(g).length > 0 || childGroups(g).some((c) => groupHas(c.id));
  const matchesGroup = (g: DocGroup) => !query || g.name.toLowerCase().includes(query);

  const pickedE = new Set(sel.entities);
  const pickedB = new Set(sel.brushes);
  const groupMembers = (g: number) => {
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

  const pick = (ids: { brushes: number[]; entities: number[] }, add: boolean) => {
    const cur = s.sel;
    if (!add) return s.select({ ...ids, face: null });
    const all = ids.brushes.every((b) => cur.brushes.includes(b)) && ids.entities.every((e) => cur.entities.includes(e));
    if (all) s.select({ brushes: cur.brushes.filter((b) => !ids.brushes.includes(b)), entities: cur.entities.filter((e) => !ids.entities.includes(e)), face: null });
    else s.select({ brushes: [...new Set([...cur.brushes, ...ids.brushes])], entities: [...new Set([...cur.entities, ...ids.entities])], face: null });
  };
  const look = (ids: { brushes: number[]; entities: number[] }) => {
    if (ids.entities.length === 1 && !ids.brushes.length) {
      const e = doc.entities.find((x) => x.id === ids.entities[0]);
      const o = e && !e.brushes.length ? originOf(e.keys) : null;
      if (o) return useTools.getState().look(o as Vec3, 64);
    }
    frame(false);
  };

  const toggleHide = (ids: { brushes: number[]; entities: number[] }, hide: boolean) => {
    const brushes = new Set(hidden.brushes);
    const entities = new Set(hidden.entities);
    ids.brushes.forEach((b) => (hide ? brushes.add(b) : brushes.delete(b)));
    ids.entities.forEach((e) => (hide ? entities.add(e) : entities.delete(e)));
    s.set({ hidden: { brushes, entities } });
  };
  const toggleLock = (ids: { brushes: number[]; entities: number[] }, lock: boolean) => {
    const brushes = new Set(locked.brushes);
    const entities = new Set(locked.entities);
    ids.brushes.forEach((b) => (lock ? brushes.add(b) : brushes.delete(b)));
    ids.entities.forEach((e) => (lock ? entities.add(e) : entities.delete(e)));
    p5.set({ locked: { brushes, entities } });
    if (lock) s.select({ brushes: s.sel.brushes.filter((b) => !brushes.has(b)), entities: s.sel.entities.filter((e) => !entities.has(e)), face: null });
  };

  // drag rows (or the whole selection when a picked row is dragged) onto a group
  const dragStart = (ev: DragEvent, ids: { brushes: number[]; entities: number[]; groups?: number[] }) => {
    const inSel = ids.brushes.some((b) => pickedB.has(b)) || ids.entities.some((e) => pickedE.has(e));
    const payload = inSel ? { brushes: s.sel.brushes, entities: s.sel.entities, groups: [] } : { groups: [], ...ids };
    ev.dataTransfer.setData("application/x-ls-outline", JSON.stringify(payload));
    ev.dataTransfer.effectAllowed = "move";
  };
  const drop = async (ev: DragEvent, group: number | null) => {
    ev.preventDefault();
    setDragOver(null);
    const raw = ev.dataTransfer.getData("application/x-ls-outline");
    if (!raw) return;
    try {
      const p = JSON.parse(raw) as { brushes: number[]; entities: number[]; groups: number[] };
      await cmd2({ op: "move_to_group", group, ...p });
    } catch {
      /* not ours */
    }
  };
  const dropProps = (key: string, group: number | null) => ({
    onDragOver: (ev: DragEvent) => {
      if (!ev.dataTransfer.types.includes("application/x-ls-outline")) return;
      ev.preventDefault();
      setDragOver(key);
    },
    onDragLeave: () => setDragOver((k) => (k === key ? null : k)),
    onDrop: (ev: DragEvent) => void drop(ev, group),
  });

  const rename = async (key: string, value: string, row: Row) => {
    setRenaming(null);
    const v = value.trim();
    if (row.kind === "group") {
      if (v && v !== row.g.name) await cmd2({ op: "rename_group", group: row.g.id, name: v });
    } else if (row.kind === "entity") {
      if (v !== (keyOf(row.e.keys, "targetname") ?? "")) await s.cmd({ op: "set_key", entity: row.e.id, key: "targetname", value: v });
    }
    void key;
  };

  const toggles = ({ ids, isHidden, isLocked, group }: { ids: { brushes: number[]; entities: number[] }; isHidden: boolean; isLocked: boolean; group?: DocGroup }) => (
    <span className="ed-ol-toggles">
      <button
        type="button"
        className={`ed-ol-eye${isHidden ? " off" : ""}`}
        title={isHidden ? "Hidden: click to show" : "Click to hide"}
        aria-pressed={isHidden}
        aria-label={isHidden ? "Show" : "Hide"}
        onClick={(ev) => {
          ev.stopPropagation();
          if (group) void s.cmd({ op: "group_flags", group: group.id, hidden: !group.hidden });
          else toggleHide(ids, !isHidden);
        }}
      >
        {isHidden ? "◌" : "●"}
      </button>
      <button
        type="button"
        className={`ed-ol-lock${isLocked ? " on" : ""}`}
        title={isLocked ? "Locked (can't be picked in the views): click to unlock" : "Click to lock (can't be picked in the views)"}
        aria-pressed={isLocked}
        aria-label={isLocked ? "Unlock" : "Lock"}
        onClick={(ev) => {
          ev.stopPropagation();
          if (group) void s.cmd({ op: "group_flags", group: group.id, locked: !group.locked });
          else toggleLock(ids, !isLocked);
        }}
      >
        {isLocked ? "🔒︎" : "🔓︎"}
      </button>
    </span>
  );

  const label = (key: string, text: ReactNode, row: Row, current: string) =>
    renaming === key ? (
      <input
        className="ed-ol-rename"
        autoFocus
        defaultValue={current}
        placeholder={row.kind === "entity" ? "targetname" : "name"}
        onClick={(e) => e.stopPropagation()}
        onKeyDown={(e) => {
          if (e.key === "Enter") void rename(key, (e.target as HTMLInputElement).value, row);
          if (e.key === "Escape") setRenaming(null);
          e.stopPropagation();
        }}
        onBlur={(e) => void rename(key, e.target.value, row)}
      />
    ) : (
      text
    );

  const renameBtn = (key: string, title: string) => (
    <button type="button" className="ed-ol-rename-btn" title={title} aria-label={title} onClick={(ev) => (ev.stopPropagation(), setRenaming(key))} onDoubleClick={(ev) => ev.stopPropagation()}>
      ✎
    </button>
  );
  const pageOf = (key: string) => more[key] ?? PAGE;
  const showMore = (key: string, total: number) =>
    total > pageOf(key) ? (
      <button type="button" className="btn small ed-ol-more" onClick={() => setMore((x) => ({ ...x, [key]: pageOf(key) + PAGE }))}>
        Show more ({total - pageOf(key)} left)
      </button>
    ) : null;

  const entityRow = (e: DocEntity, depth: number) => {
    const key = `e${e.id}`;
    const name = keyOf(e.keys, "targetname");
    const ids = { brushes: [], entities: [e.id] };
    const isHidden = hidden.entities.has(e.id);
    const isLocked = locked.entities.has(e.id);
    const kids = e.brushes.length > 0;
    const isOpen = open[key];
    return (
      <div key={key}>
        <div
          className={`ed-ol-row${pickedE.has(e.id) ? " on" : ""}${isHidden ? " dim" : ""}`}
          style={{ paddingLeft: 4 + depth * 14 }}
          draggable
          onDragStart={(ev) => dragStart(ev, ids)}
          onClick={(ev) => pick(ids, ev.shiftKey || ev.ctrlKey || ev.metaKey)}
          onDoubleClick={() => look(ids)}
          role="treeitem"
          aria-selected={pickedE.has(e.id)}
          aria-expanded={kids ? !!isOpen : undefined}
        >
          {kids ? (
            <button type="button" className="ed-ol-twist" aria-label={isOpen ? "Fold" : "Unfold"} onClick={(ev) => (ev.stopPropagation(), setOpen((o) => ({ ...o, [key]: !o[key] })))}>
              {isOpen ? "▾" : "▸"}
            </button>
          ) : (
            <span className="ed-ol-twist" />
          )}
          <span className={`ed-ol-kind ${kids ? "solid" : "point"}`} aria-hidden="true" />
          <span className="ed-ol-text">{label(key, <>{e.classname}{name && <span className="muted"> “{name}”</span>}</>, { kind: "entity", e }, name ?? "")}</span>
          {renameBtn(key, "Rename (sets its targetname)")}
          {toggles({ ids, isHidden, isLocked })}
        </div>
        {kids && isOpen && e.brushes.map((id) => {
          const b = doc.brushes.find((x) => x.id === id);
          return b ? brushRow(b, depth + 1, true) : null;
        })}
      </div>
    );
  };

  const brushRow = (b: DocBrush, depth: number, inEntity = false) => {
    const key = `b${b.id}`;
    const ids = { brushes: [b.id], entities: [] };
    const isHidden = hidden.brushes.has(b.id);
    const isLocked = locked.brushes.has(b.id);
    const tex = meshes.get(b.id)?.textures ?? [];
    return (
      <div
        key={key}
        className={`ed-ol-row${pickedB.has(b.id) ? " on" : ""}${isHidden ? " dim" : ""}`}
        style={{ paddingLeft: 4 + depth * 14 }}
        draggable={!inEntity}
        onDragStart={(ev) => dragStart(ev, ids)}
        onClick={(ev) => pick(ids, ev.shiftKey || ev.ctrlKey || ev.metaKey)}
        onDoubleClick={() => {
          s.select({ ...ids, face: null });
          frame(false);
        }}
        role="treeitem"
        aria-selected={pickedB.has(b.id)}
      >
        <span className="ed-ol-twist" />
        <span className="ed-ol-kind brush" aria-hidden="true" />
        <span className="ed-ol-text">
          Brush {b.id} <span className="muted small-note">{dims(b)}{tex.length ? ` · ${tex.slice(0, 2).join(", ")}${tex.length > 2 ? "…" : ""}` : ""}</span>
        </span>
        {!inEntity && toggles({ ids, isHidden, isLocked })}
      </div>
    );
  };

  const groupRow = (g: DocGroup, depth: number): ReactNode => {
    if (!groupHas(g.id) && !matchesGroup(g) && query) return null;
    const key = `g${g.id}`;
    const ids = groupMembers(g.id);
    const isOpen = open[key] ?? false;
    const all = ids.brushes.length + ids.entities.length;
    const on = all > 0 && ids.brushes.every((b) => pickedB.has(b)) && ids.entities.every((e) => pickedE.has(e));
    const ents = entsIn(g.id);
    const bs = brushesIn(g.id);
    return (
      <div key={key}>
        <div
          className={`ed-ol-row group${on ? " on" : ""}${g.hidden ? " dim" : ""}${dragOver === key ? " drop" : ""}`}
          style={{ paddingLeft: 4 + depth * 14 }}
          draggable
          onDragStart={(ev) => {
            ev.dataTransfer.setData("application/x-ls-outline", JSON.stringify({ brushes: [], entities: [], groups: [g.id] }));
            ev.dataTransfer.effectAllowed = "move";
          }}
          {...dropProps(key, g.id)}
          onClick={(ev) => pick(ids, ev.shiftKey || ev.ctrlKey || ev.metaKey)}
          onDoubleClick={() => look(ids)}
          role="treeitem"
          aria-expanded={isOpen}
          aria-selected={on}
        >
          <button type="button" className="ed-ol-twist" aria-label={isOpen ? "Fold" : "Unfold"} onClick={(ev) => (ev.stopPropagation(), setOpen((o) => ({ ...o, [key]: !isOpen })))}>
            {isOpen ? "▾" : "▸"}
          </button>
          <span className="ed-ol-kind group" aria-hidden="true" />
          <span className="ed-ol-text" title="Drop things here to put them in the group">
            {label(key, <>{g.name} <span className="muted small-note">{all}</span></>, { kind: "group", g }, g.name)}
          </span>
          {renameBtn(key, "Rename the group")}
          <button type="button" className="btn small ed-ol-ungroup" title="Ungroup (the things stay)" onClick={(ev) => (ev.stopPropagation(), void s.cmd({ op: "ungroup", group: g.id }))}>
            Ungroup
          </button>
          {toggles({ ids, isHidden: g.hidden, isLocked: g.locked, group: g })}
        </div>
        {isOpen && (
          <>
            {childGroups(g.id).map((c) => groupRow(c, depth + 1))}
            {ents.slice(0, pageOf(key)).map((e) => entityRow(e, depth + 1))}
            {bs.slice(0, Math.max(0, pageOf(key) - ents.length)).map((b) => brushRow(b, depth + 1))}
            {showMore(key, ents.length + bs.length)}
          </>
        )}
      </div>
    );
  };

  const looseEnts = entsIn(null);
  const looseB = brushesIn(null);
  const solidEnts = looseEnts.filter((e) => e.brushes.length);
  const pointEnts = looseEnts.filter((e) => !e.brushes.length);
  const section = (key: string, title: string, count: number, body: ReactNode, dropGroup?: null) => (
    <div key={key}>
      <button
        type="button"
        className={`editor-tree-head${dragOver === key ? " drop" : ""}`}
        aria-expanded={open[key] ?? true}
        onClick={() => setOpen((o) => ({ ...o, [key]: !(o[key] ?? true) }))}
        {...(dropGroup === null ? dropProps(key, null) : {})}
        title={dropGroup === null ? "Drop things here to take them out of their group" : undefined}
      >
        {title} ({count})
      </button>
      {(open[key] ?? true) && body}
    </div>
  );
  const topGroups = childGroups(null);
  const nHidden = hidden.brushes.size + hidden.entities.size;
  const nLocked = locked.brushes.size + locked.entities.size;

  return (
    <section className="tools-panel editor-outline ed-outline">
      <div className="row">
        <input type="search" placeholder="Find by name, type, texture or number" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Find in the map" />
      </div>
      <div className="seg ed-ol-filter" role="group" aria-label="Show">
        {(
          [
            ["all", "All"],
            ["entities", "Entities"],
            ["brushes", "Brushes"],
            ["hidden", `Hidden${nHidden ? ` ${nHidden}` : ""}`],
            ["locked", `Locked${nLocked ? ` ${nLocked}` : ""}`],
          ] as [Filter, string][]
        ).map(([id, l]) => (
          <button key={id} type="button" aria-pressed={filter === id} onClick={() => setFilter(id)}>
            {l}
          </button>
        ))}
      </div>
      {extra}
      <div className="ed-ol-tree" role="tree" aria-label="Everything in the map" aria-multiselectable>
        {topGroups.length > 0 && section("groups", "Groups", topGroups.length, topGroups.map((g) => groupRow(g, 0)))}
        {section(
          "solid",
          "Brush entities",
          solidEnts.length,
          <>
            {solidEnts.slice(0, pageOf("solid")).map((e) => entityRow(e, 0))}
            {showMore("solid", solidEnts.length)}
          </>,
          null,
        )}
        {section(
          "point",
          "Point entities",
          pointEnts.length,
          <>
            {pointEnts.slice(0, pageOf("point")).map((e) => entityRow(e, 0))}
            {showMore("point", pointEnts.length)}
          </>,
          null,
        )}
        {section(
          "world",
          "World brushes",
          looseB.length,
          <>
            {looseB.slice(0, pageOf("world")).map((b) => brushRow(b, 0))}
            {showMore("world", looseB.length)}
          </>,
          null,
        )}
      </div>
      <p className="small-note muted">Click to pick (Ctrl/Shift adds). Double-click to look at it; ✎ renames. Drag onto a group to put it in.</p>
    </section>
  );
}
