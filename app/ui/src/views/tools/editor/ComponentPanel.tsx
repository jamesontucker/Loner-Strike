// Game mode components (Gameplay > Components): the library from W1's components.json with
// the forms from lonerstrike.fgd. Drag a card onto the map (or Place, then click) to add one;
// zones, capture points, spawners and the like are drawn in 3D. "Save as component file"
// writes the map's components as a pack (configs/components/<map>.ini) to try a mode on any
// map, stock ones too, without compiling; "Load" brings a pack in to edit.

import { useEffect, useMemo, useState } from "react";
import { BusyButton, useToast } from "../../../ui";
import * as api from "./editorApi";
import type { ComponentDef } from "./editorApi";
import { dragging, loadComponents, useP4 } from "./extras";
import { COMPONENT_MIME } from "./P4Scene";
import { useEditor } from "./store";
import "./p4.css";

const norm = (s: string) => s.toLowerCase().replace(/[_-]+/g, " ");

/** The compiler's name for the open map (as tools/compile.rs map_name). */
export function mapNameOf(name: string) {
  const n = name.trim().toLowerCase().replace(/[^a-z0-9_-]/g, "_").replace(/_+/g, "_").replace(/^_+|_+$/g, "").slice(0, 31);
  return !n || n === "untitled" ? "ls_newmap" : n;
}

export function ComponentPanel() {
  const toast = useToast();
  const doc = useEditor((s) => s.doc);
  const [lib, setLib] = useState<{ groups: string[]; components: ComponentDef[] } | null>(null);
  const [q, setQ] = useState("");
  const [map, setMap] = useState(() => mapNameOf(doc?.name ?? ""));
  const [mode, setMode] = useState("");
  const [busy, setBusy] = useState<"save" | "load" | null>(null);
  useEffect(() => {
    void loadComponents().then(setLib);
  }, []);
  const count = doc?.entities.filter((e) => e.classname.startsWith("ls_")).length ?? 0;
  const shown = useMemo(() => {
    const words = norm(q).split(/\s+/).filter(Boolean);
    return (lib?.components ?? []).filter((c) => words.every((w) => norm(`${c.title} ${c.class} ${c.blurb} ${c.group}`).includes(w)));
  }, [lib, q]);
  const groups = (lib?.groups ?? []).filter((g) => shown.some((c) => c.group === g));

  const save = async () => {
    setBusy("save");
    const r = await api.savePack(map.trim(), mode.trim());
    setBusy(null);
    if (r.error) return toast(r.error, true);
    // P5: the file had these already: nothing written
    if ((r as { unchanged?: boolean }).unchanged) return toast(`${r.file} already has these ${r.count} component${r.count === 1 ? "" : "s"}; nothing to save.`);
    toast(`Saved ${r.count} component${r.count === 1 ? "" : "s"} to ${r.file}. Start ${map} with gm_components on to try them.${r.skipped?.length ? ` Left out: ${r.skipped.join("; ")}.` : ""}`);
  };
  const load = async () => {
    setBusy("load");
    const r = await api.loadPack(map.trim(), mode.trim());
    setBusy(null);
    if (r.error) return toast(r.error, true);
    const s = useEditor.getState();
    await s.syncMeshes();
    await s.refreshDoc();
    s.refreshChecks();
    toast(`Loaded ${r.count} component${r.count === 1 ? "" : "s"} from ${r.file}.${r.bad?.length ? ` ${r.bad.length} line${r.bad.length === 1 ? "" : "s"} couldn't be read.` : ""}`);
  };

  return (
    <div className="p4-prefabs">
      <input className="p4-search" type="search" placeholder="Search components" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Search components" />
      {!lib && (
        <p className="small-note muted">
          <span className="spinner" aria-hidden="true" />
          Loading components…
        </p>
      )}
      {groups.map((g) => (
        <div key={g} className="stack-tight">
          <h4 className="p4-head">{g}</h4>
          <div className="p4-comp-list">
            {shown
              .filter((c) => c.group === g)
              .map((c) => (
                <div
                  key={c.class}
                  className="p4-comp"
                  draggable
                  onDragStart={(e) => {
                    e.dataTransfer.setData(COMPONENT_MIME, c.class);
                    e.dataTransfer.setData("text/plain", c.class);
                    e.dataTransfer.effectAllowed = "copy";
                    dragging.current = { id: c.class, size: [32, 32, 32], component: true };
                  }}
                  onDragEnd={() => (dragging.current = null)}
                  title={`${c.class}: ${c.blurb}${c.brush ? " Made of brushes: drops as a box." : ""}${c.needs_nav ? " Bots need a nav area under it." : ""}\nDrag onto the map.`}
                >
                  <span className="p4-comp-dot" style={{ background: `rgb(${c.colour.join(",")})` }} aria-hidden="true" />
                  <span className="p4-comp-text">
                    <strong>{c.title}</strong>
                    <span className="muted">{c.blurb}</span>
                  </span>
                  <button type="button" className="btn small" title="Then click on the map" onClick={() => useP4.getState().set({ placing: { id: c.class, name: c.title, component: true }, turn: 0 })}>
                    Place
                  </button>
                </div>
              ))}
          </div>
        </div>
      ))}
      <fieldset className="p4-cordon p4-pack">
        <legend>Component file (try a mode on any map)</legend>
        <p className="small-note muted">
          {count ? `${count} component${count === 1 ? "" : "s"} in this map.` : "No components in this map yet."} A component file adds them to a map when it loads, no compile needed, stock maps too (needs gm_components on).
        </p>
        <div className="editor-row wrap">
          <label className="editor-field compile-inline">
            Map
            <input value={map} maxLength={31} spellCheck={false} onChange={(e) => setMap(e.target.value.replace(/\s/g, "_"))} />
          </label>
          <label className="editor-field compile-inline" title="Only for this game mode (its id), or leave empty for every mode">
            Mode
            <input value={mode} maxLength={32} placeholder="any" spellCheck={false} onChange={(e) => setMode(e.target.value.replace(/\s/g, ""))} />
          </label>
        </div>
        <div className="p4-actions">
          <BusyButton className="btn small primary" busy={busy === "save"} label="Save as component file" busyLabel="Saving…" disabled={!count || !map.trim() || !!busy} onClick={() => void save()} />
          <BusyButton className="btn small" busy={busy === "load"} label="Load component file" busyLabel="Loading…" disabled={!map.trim() || !!busy} onClick={() => void load()} />
        </div>
        <p className="small-note muted">
          Saved to <span className="p4-mono-path">addons/amxmodx/configs/components/{map || "<map>"}{mode ? `.${mode}` : ""}.ini</span> in your game (the old one kept as .bak). Barricades are brushes, so they only work compiled into the map.
        </p>
      </fieldset>
    </div>
  );
}
