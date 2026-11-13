// The prefab library (the editor's Prefabs tab): your own prefabs and the starter ones, with
// search; drag a card onto the map (or press Place and click) to put one down, standing on the
// floor, hanging from a ceiling or against a wall, its corner on the grid. Save what's picked
// as a prefab (a picture of it from the 3D view becomes its thumbnail), share one as a
// package for Tools > Community, or add one from a file.

import { useCallback, useEffect, useMemo, useState } from "react";
import { BusyButton, useConfirm, useToast } from "../../../ui";
import * as api from "./editorApi";
import type { Prefab } from "./editorApi";
import { selectionIds } from "./EditorScene";
import { snapshot, useP4 } from "./extras";
import { PREFAB_MIME } from "./P4Scene";
import { selectionBounds, useEditor } from "./store";
import "./p4.css";

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

export function PrefabPanel() {
  const toast = useToast();
  const ask = useConfirm();
  const sel = useEditor((s) => s.sel);
  const rev = useP4((s) => s.prefabRev);
  const [list, setList] = useState<Prefab[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [saving, setSaving] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);

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

  const doImport = async () => {
    setBusy("Adding…");
    const r = await api.importPrefab();
    setBusy(null);
    if (r.error) return toast(r.error, true);
    if (r.cancelled) return;
    toast(`Added ${r.prefabs?.join(", ")}${r.wads?.length ? ` (and ${r.wads.join(", ")} to the game folder)` : ""}.`);
    useP4.getState().set({ prefabRev: useP4.getState().prefabRev + 1 });
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
      <div className="editor-row wrap">
        <input className="p4-search" type="search" placeholder="Search prefabs" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search prefabs" />
      </div>
      <div className="editor-row wrap">
        <button type="button" className="btn small primary" disabled={!hasSel} title={hasSel ? "Save the picked brushes and entities as a prefab" : "Pick some brushes or entities first"} onClick={() => setSaving(true)}>
          Save what's picked…
        </button>
        <BusyButton className="btn small" busy={busy === "Adding…"} label="Add from a file…" busyLabel="Adding…" onClick={() => void doImport()} />
      </div>
      {busy && busy !== "Adding…" && (
        <p className="small-note muted">
          <span className="spinner" aria-hidden="true" />
          {busy}
        </p>
      )}
      {saving && <SaveForm onClose={() => setSaving(false)} onSaved={() => void load()} />}
      {error && <p className="banner">{error}</p>}
      {!list && !error && (
        <p className="small-note muted">
          <span className="spinner" aria-hidden="true" />
          Loading prefabs…
        </p>
      )}
      {list && (
        <>
          <h4 className="p4-head">Yours {mine.length ? `(${mine.length})` : ""}</h4>
          {mine.length === 0 && <p className="small-note muted">{q ? "None of yours match." : "Pick some brushes and entities, then Save what's picked. Prefabs from Tools > Community land here too."}</p>}
          <div className="p4-grid">
            {mine.map((p) => (
              <Card key={p.id} p={p} rev={rev} editing={editing === p.id} onEdit={() => setEditing(editing === p.id ? null : p.id)} onShare={() => void share(p)} onDelete={() => void remove(p)} onSaved={() => void load()} />
            ))}
          </div>
          <h4 className="p4-head">Starter prefabs</h4>
          {starters.length === 0 && <p className="small-note muted">None match.</p>}
          <div className="p4-grid">
            {starters.map((p) => (
              <Card key={p.id} p={p} rev={rev} onShare={() => void share(p)} />
            ))}
          </div>
          <p className="small-note muted">Drag a card onto the map: it stands on floors, hangs from ceilings and sits against walls, with its corner on the grid (Alt: no snapping). It comes in as one group; Ungroup it to change the pieces.</p>
        </>
      )}
    </div>
  );
}

function Card({ p, rev, editing, onEdit, onShare, onDelete, onSaved }: { p: Prefab; rev: number; editing?: boolean; onEdit?: () => void; onShare: () => void; onDelete?: () => void; onSaved?: () => void }) {
  const size = p.size.map((x) => Math.round(x)).join(" × ");
  return (
    <div
      className="p4-card"
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData(PREFAB_MIME, p.id);
        e.dataTransfer.setData("text/plain", p.name);
        e.dataTransfer.effectAllowed = "copy";
      }}
      title={`${p.description || p.name}\n${size} units · ${p.brushes} brush${p.brushes === 1 ? "" : "es"}${p.entities ? `, ${p.entities} entit${p.entities === 1 ? "y" : "ies"}` : ""}${p.wads.length ? `\nTextures from ${p.wads.join(", ")}` : ""}\nDrag onto the map to place it.`}
    >
      <img className="p4-thumb" src={`${p.thumb}?r=${rev}`} alt="" draggable={false} loading="lazy" />
      <div className="p4-card-name">{p.name}</div>
      <div className="p4-card-meta muted">
        {size}
        {p.author && !p.builtin ? ` · ${p.author}` : ""}
      </div>
      {p.tags.length > 0 && (
        <div className="p4-tags">
          {p.tags.slice(0, 4).map((t) => (
            <span key={t} className="tag">
              {t}
            </span>
          ))}
        </div>
      )}
      <div className="p4-card-actions">
        <button type="button" className="btn small" title="Then click on the map" onClick={() => useP4.getState().set({ placing: { id: p.id, name: p.name } })}>
          Place
        </button>
        <button type="button" className="btn small" title="Save it as a package to share (Tools > Community)" onClick={onShare}>
          Share
        </button>
        {onEdit && (
          <button type="button" className="btn small" aria-pressed={editing} onClick={onEdit}>
            Edit
          </button>
        )}
        {onDelete && (
          <button type="button" className="btn small icon" aria-label={`Delete ${p.name}`} title="Delete" onClick={onDelete}>
            ×
          </button>
        )}
      </div>
      {editing && onSaved && <EditForm p={p} onSaved={onSaved} onClose={onEdit!} />}
    </div>
  );
}

function EditForm({ p, onSaved, onClose }: { p: Prefab; onSaved: () => void; onClose: () => void }) {
  const toast = useToast();
  const [name, setName] = useState(p.name);
  const [tags, setTags] = useState(p.tags.join(", "));
  const [desc, setDesc] = useState(p.description);
  const [busy, setBusy] = useState(false);
  return (
    <form
      className="p4-form"
      onSubmit={async (e) => {
        e.preventDefault();
        setBusy(true);
        const r = await api.editPrefab({ id: p.id, name, tags: tags.split(","), description: desc });
        setBusy(false);
        if (r.error) return toast(r.error, true);
        onSaved();
        onClose();
      }}
    >
      <label className="editor-field">
        Name
        <input value={name} maxLength={64} onChange={(e) => setName(e.target.value)} />
      </label>
      <label className="editor-field">
        Tags
        <input value={tags} placeholder="door, wall" onChange={(e) => setTags(e.target.value)} />
      </label>
      <label className="editor-field">
        About it
        <input value={desc} maxLength={400} onChange={(e) => setDesc(e.target.value)} />
      </label>
      <div className="editor-row">
        <BusyButton type="submit" className="btn small primary" busy={busy} label="Save" busyLabel="Saving…" />
        <button type="button" className="btn small" onClick={onClose}>
          Cancel
        </button>
      </div>
    </form>
  );
}

function SaveForm({ onClose, onSaved }: { onClose: () => void; onSaved: () => void }) {
  const toast = useToast();
  const [name, setName] = useState("");
  const [tags, setTags] = useState("");
  const [desc, setDesc] = useState("");
  const [author, setAuthor] = useState(savedAuthor);
  const [busy, setBusy] = useState(false);
  // the thumbnail: a picture of what's picked, from the 3D view's direction
  // (drawn without the picked highlight: the selection steps aside for two frames)
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
  const save = async () => {
    const ids = selectionIds();
    if (!name.trim()) return toast("Give the prefab a name.", true);
    setBusy(true);
    try {
      localStorage.setItem(AUTHOR_KEY, author.trim());
    } catch {
      /* private window */
    }
    const r = await api.savePrefab({ name, tags: tags.split(","), author, description: desc, brushes: ids.brushes, entities: ids.entities, thumb });
    setBusy(false);
    if (r.error) return toast(r.error, true);
    toast(`Saved the prefab ${r.name}.`);
    useP4.getState().set({ prefabRev: useP4.getState().prefabRev + 1 });
    onSaved();
    onClose();
  };
  return (
    <form
      className="p4-form panel"
      onSubmit={(e) => {
        e.preventDefault();
        void save();
      }}
    >
      <div className="p4-save-top">
        {thumb ? <img className="p4-thumb" src={thumb} alt="Thumbnail" /> : <div className="p4-thumb p4-nothumb">A picture is drawn from its shapes</div>}
        <div className="stack-tight">
          <label className="editor-field">
            Name
            <input autoFocus value={name} maxLength={64} placeholder="Market stall" onChange={(e) => setName(e.target.value)} />
          </label>
          <label className="editor-field">
            Tags
            <input value={tags} placeholder="cover, wood" onChange={(e) => setTags(e.target.value)} />
          </label>
        </div>
      </div>
      <label className="editor-field">
        About it
        <input value={desc} maxLength={400} placeholder="One line on what it is" onChange={(e) => setDesc(e.target.value)} />
      </label>
      <label className="editor-field">
        Your name
        <input value={author} maxLength={48} placeholder="Shown when you share it" onChange={(e) => setAuthor(e.target.value)} />
      </label>
      <div className="editor-row">
        <BusyButton type="submit" className="btn small primary" busy={busy} label="Save prefab" busyLabel="Saving…" />
        <button type="button" className="btn small" onClick={onClose}>
          Cancel
        </button>
      </div>
    </form>
  );
}
