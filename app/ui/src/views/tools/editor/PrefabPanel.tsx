// The prefab library (Library > Prefabs): your own prefabs and the starter ones, with search.
// Drag a row onto the map (a see-through box shows where it lands) or press Place and click
// (R turns it a quarter, Shift keeps placing, Esc stops). Prefabs stand on floors, hang from
// ceilings and face out of walls, corner on the grid. Save what's picked as a prefab (a
// picture of it from the 3D view becomes its thumbnail), share one as a package for Tools >
// Community, or add one from a file. Rows never clip: names and details shorten with "…" and
// show in full on hover; the less used actions are in the row's ⋯ menu.

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { BusyButton, useConfirm, useToast } from "../../../ui";
import * as api from "./editorApi";
import type { Prefab } from "./editorApi";
import { DialogSection, EditorDialog, Field } from "./EditorDialog";
import { selectionIds } from "./EditorScene";
import { dragging, snapshot, useP4 } from "./extras";
import { PREFAB_MIME } from "./P4Scene";
import { selectionBounds, useEditor } from "./store";
import "./p4.css";
import { CommunityChip, CommunityLink, communityPrefab, useCommunity } from "../community/chip";

const AUTHOR_KEY = "ls.editor.prefabAuthor";

function savedAuthor() {
  try {
    return localStorage.getItem(AUTHOR_KEY) ?? "";
  } catch {
    return "";
  }
}

/** Every word of the search somewhere in the name, tags, textures or author (any case; _ and - don't matter). */
function matches(p: Prefab, q: string) {
  const norm = (s: string) => s.toLowerCase().replace(/[_-]+/g, " ");
  const hay = norm([p.name, p.author, p.description, ...p.tags, ...p.textures].join(" "));
  return norm(q).split(/\s+/).filter(Boolean).every((w) => hay.includes(w));
}

const sizeText = (p: Prefab) => p.size.map((x) => Math.round(x)).join(" × ");

export function PrefabPanel() {
  const toast = useToast();
  const ask = useConfirm();
  const sel = useEditor((s) => s.sel);
  const rev = useP4((s) => s.prefabRev);
  const [list, setList] = useState<Prefab[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [saving, setSaving] = useState(false);
  const [editing, setEditing] = useState<Prefab | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    const r = await api.getPrefabs();
    if (r.error) return setError(r.error);
    setError(null);
    setList(r.prefabs);
  }, []);
  useEffect(() => {
    void load();
  }, [load, rev]);

  const shown = useMemo(() => (list ?? []).filter((p) => matches(p, q)), [list, q]);
  const mine = shown.filter((p) => !p.builtin);
  const starters = shown.filter((p) => p.builtin);
  const hasSel = sel.brushes.length + sel.entities.length > 0;
  const bump = () => useP4.getState().set({ prefabRev: useP4.getState().prefabRev + 1 });

  const doImport = async () => {
    setBusy("Adding…");
    const r = await api.importPrefab();
    setBusy(null);
    if (r.error) return toast(r.error, true);
    if (r.cancelled) return;
    toast(`Added ${r.prefabs?.join(", ")}${r.wads?.length ? ` (and ${r.wads.join(", ")} to the game folder)` : ""}.`);
    bump();
  };

  const share = async (p: Prefab) => {
    setBusy(`Packing ${p.name}…`);
    const r = await api.exportPrefab(p.id);
    setBusy(null);
    if (r.error) return toast(r.error, true);
    if (!r.cancelled) toast(`Saved ${r.file}. Upload it to GameBanana's Prefabs with the attribute Software Used: LonerStrike, then register it in Tools > Community.`);
  };

  const remove = async (p: Prefab) => {
    if (!(await ask(`Delete the prefab “${p.name}”? Maps it's in keep their copy.`, { ok: "Delete", danger: true }))) return;
    const r = await api.deletePrefab(p.id);
    if (r.error) return toast(r.error, true);
    void load();
  };

  return (
    <div className="p4-prefabs">
      <input className="p4-search" type="search" placeholder="Search prefabs" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search prefabs" />
      <div className="p4-actions">
        <button type="button" className="btn small primary" disabled={!hasSel} title={hasSel ? "Save the picked brushes and entities as a prefab" : "Pick some brushes or entities first"} onClick={() => setSaving(true)}>
          Save what's picked…
        </button>
        <BusyButton className="btn small" busy={busy === "Adding…"} label="Add from a file…" busyLabel="Adding…" onClick={() => void doImport()} />
        <CommunityLink kind="prefab" label="Community prefabs…" />{/* (community) */}
      </div>
      {busy && busy !== "Adding…" && (
        <p className="small-note muted" role="status">
          <span className="spinner" aria-hidden="true" />
          {busy}
        </p>
      )}
      {error && <p className="banner">{error}</p>}
      {!list && !error && (
        <p className="small-note muted">
          <span className="spinner" aria-hidden="true" />
          Loading prefabs…
        </p>
      )}
      {list && (
        <>
          <h4 className="p4-head">Yours{mine.length ? ` (${mine.length})` : ""}</h4>
          {mine.length === 0 && <p className="small-note muted">{q ? "None of yours match." : "Pick some brushes and entities, then Save what's picked. Prefabs from Tools > Community land here too."}</p>}
          <ul className="p4-rows">
            {mine.map((p) => (
              <Row key={p.id} p={p} rev={rev} onShare={() => void share(p)} onEdit={() => setEditing(p)} onDelete={() => void remove(p)} />
            ))}
          </ul>
          <h4 className="p4-head">Starter prefabs</h4>
          {starters.length === 0 && <p className="small-note muted">None match.</p>}
          <ul className="p4-rows">
            {starters.map((p) => (
              <Row key={p.id} p={p} rev={rev} onShare={() => void share(p)} />
            ))}
          </ul>
          <p className="small-note muted">Drag one onto the map, or Place and click: it stands on floors, hangs from ceilings and faces out of walls, corner on the grid (Alt: off the grid). R turns it while placing. It comes in as one group; Ungroup it to change the pieces.</p>
        </>
      )}
      {saving && <SaveDialog onClose={() => setSaving(false)} onSaved={() => void load()} />}
      {editing && <EditDialog p={editing} onClose={() => setEditing(null)} onSaved={() => void load()} />}
    </div>
  );
}

/** One prefab: thumbnail, name and size, Place, and a ⋯ menu for the rest. Drag it onto the map. */
function Row({ p, rev, onShare, onEdit, onDelete }: { p: Prefab; rev: number; onShare: () => void; onEdit?: () => void; onDelete?: () => void }) {
  const img = useRef<HTMLImageElement>(null);
  const meta = `${sizeText(p)}${p.tags.length ? ` · ${p.tags.join(", ")}` : ""}${p.author && !p.builtin ? ` · ${p.author}` : ""}`;
  const cm = communityPrefab(useCommunity(), p.id); // (community chip)
  return (
    <li
      className="p4-row"
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData(PREFAB_MIME, p.id);
        e.dataTransfer.setData("text/plain", p.name);
        e.dataTransfer.effectAllowed = "copy";
        if (img.current) e.dataTransfer.setDragImage(img.current, 36, 27);
        dragging.current = { id: p.id, size: p.size };
      }}
      onDragEnd={() => (dragging.current = null)}
      title={`${p.name}\n${p.description ? `${p.description}\n` : ""}${sizeText(p)} units · ${p.brushes} brush${p.brushes === 1 ? "" : "es"}${p.entities ? `, ${p.entities} entit${p.entities === 1 ? "y" : "ies"}` : ""}${p.wads.length ? `\nTextures from ${p.wads.join(", ")}` : ""}\nDrag onto the map to place it.`}
    >
      <img ref={img} className="p4-row-thumb" src={`${p.thumb}?r=${rev}`} alt="" draggable={false} loading="lazy" />
      <span className="p4-row-text">
        <span className="p4-row-name ed-ellipsis">{p.name}</span>
        <span className="p4-row-meta ed-ellipsis muted">{meta}</span>
        {cm && <CommunityChip chip={cm} />}
      </span>
      <span className="p4-row-actions">
        <button type="button" className="btn small" title="Then click on the map (R turns it)" onClick={() => useP4.getState().set({ placing: { id: p.id, name: p.name, size: p.size }, turn: 0 })}>
          Place
        </button>
        <RowMenu name={p.name} items={[["Share as a package…", onShare], ...(onEdit ? [["Rename and tags…", onEdit] as [string, () => void]] : []), ...(onDelete ? [["Delete…", onDelete] as [string, () => void]] : [])]} />
      </span>
    </li>
  );
}

/** A small ⋯ menu (opens below the button, closes on a pick, Esc or a click elsewhere). */
export function RowMenu({ name, items }: { name: string; items: [string, () => void][] }) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => !box.current?.contains(e.target as Node) && setOpen(false);
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        setOpen(false);
      }
    };
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", key, true);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", key, true);
    };
  }, [open]);
  return (
    <span className="p4-menu" ref={box}>
      <button type="button" className="btn small icon" aria-haspopup="menu" aria-expanded={open} aria-label={`More for ${name}`} title="More" onClick={() => setOpen((o) => !o)}>
        ⋯
      </button>
      {open && (
        <span className="p4-menu-list" role="menu">
          {items.map(([label, run]) => (
            <button
              key={label}
              type="button"
              role="menuitem"
              onClick={() => {
                setOpen(false);
                run();
              }}
            >
              {label}
            </button>
          ))}
        </span>
      )}
    </span>
  );
}

function EditDialog({ p, onSaved, onClose }: { p: Prefab; onSaved: () => void; onClose: () => void }) {
  const [name, setName] = useState(p.name);
  const [tags, setTags] = useState(p.tags.join(", "));
  const [desc, setDesc] = useState(p.description);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const nameError = name.trim() ? null : "Give it a name.";
  const save = async () => {
    if (nameError) return;
    setBusy(true);
    const r = await api.editPrefab({ id: p.id, name, tags: tags.split(","), description: desc });
    setBusy(false);
    if (r.error) return setError(r.error);
    onSaved();
    onClose();
  };
  return (
    <EditorDialog title="Rename prefab" subtitle={sizeText(p)} size="sm" onClose={onClose} error={error} primary={{ label: "Save", onClick: () => void save(), busy, busyLabel: "Saving…", disabled: !!nameError }}>
      <DialogSection>
        <Field label="Name" error={nameError}>{(id) => <input id={id} autoFocus value={name} maxLength={64} onChange={(e) => setName(e.target.value)} />}</Field>
        <Field label="Tags" note="Separate them with commas.">{(id) => <input id={id} value={tags} placeholder="door, wall" onChange={(e) => setTags(e.target.value)} />}</Field>
        <Field label="About it">{(id) => <input id={id} value={desc} maxLength={400} onChange={(e) => setDesc(e.target.value)} />}</Field>
      </DialogSection>
    </EditorDialog>
  );
}

function SaveDialog({ onClose, onSaved }: { onClose: () => void; onSaved: () => void }) {
  const toast = useToast();
  const [name, setName] = useState("");
  const [touched, setTouched] = useState(false);
  const [tags, setTags] = useState("");
  const [desc, setDesc] = useState("");
  const [author, setAuthor] = useState(savedAuthor);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ids = useMemo(() => selectionIds(), []);
  // the thumbnail: what's picked, from the 3D view's direction, drawn without the pick highlight
  const [thumb, setThumb] = useState<string | null>(null);
  useEffect(() => {
    const b = selectionBounds();
    if (!b) return;
    const s = useEditor.getState();
    const keep = s.sel;
    s.select({ brushes: [], entities: [], face: null, faces: [] });
    let id = requestAnimationFrame(() => {
      id = requestAnimationFrame(() => {
        setThumb(snapshot(b));
        useEditor.getState().select(keep);
      });
    });
    return () => {
      cancelAnimationFrame(id);
      useEditor.getState().select(keep);
    };
  }, []);
  const nameError = touched && !name.trim() ? "Give it a name." : null;
  const save = async () => {
    setTouched(true);
    if (!name.trim()) return;
    setBusy(true);
    try {
      localStorage.setItem(AUTHOR_KEY, author.trim());
    } catch {
      /* private window */
    }
    const r = await api.savePrefab({ name, tags: tags.split(","), author, description: desc, brushes: ids.brushes, entities: ids.entities, thumb });
    setBusy(false);
    if (r.error) return setError(r.error);
    toast(`Saved the prefab ${r.name}.`);
    useP4.getState().set({ prefabRev: useP4.getState().prefabRev + 1 });
    onSaved();
    onClose();
  };
  const what = `${ids.brushes.length} brush${ids.brushes.length === 1 ? "" : "es"}${ids.entities.length ? `, ${ids.entities.length} entit${ids.entities.length === 1 ? "y" : "ies"}` : ""}`;
  return (
    <EditorDialog title="Save as a prefab" subtitle={`What's picked: ${what}`} onClose={onClose} error={error} primary={{ label: "Save prefab", onClick: () => void save(), busy, busyLabel: "Saving…" }} footerNote="Kept in your prefab library, for any map.">
      <div className="ed-thumb-pick">
        {thumb ? <img src={thumb} alt="The prefab's picture" /> : <div className="p4-nothumb">A picture is drawn from its shapes</div>}
        <DialogSection>
          <Field label="Name" error={nameError}>
            {(id) => <input id={id} autoFocus value={name} maxLength={64} placeholder="Market stall" onChange={(e) => setName(e.target.value)} onBlur={() => setTouched(true)} />}
          </Field>
          <Field label="Tags" note="Separate them with commas; they help search.">
            {(id) => <input id={id} value={tags} placeholder="cover, wood" onChange={(e) => setTags(e.target.value)} />}
          </Field>
        </DialogSection>
      </div>
      <DialogSection cols={2}>
        <Field label="About it" wide>{(id) => <input id={id} value={desc} maxLength={400} placeholder="One line on what it is" onChange={(e) => setDesc(e.target.value)} />}</Field>
        <Field label="Your name" hint="Shown when you share it.">{(id) => <input id={id} value={author} maxLength={48} onChange={(e) => setAuthor(e.target.value)} />}</Field>
      </DialogSection>
    </EditorDialog>
  );
}
