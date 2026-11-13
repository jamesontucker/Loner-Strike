// P5: Library > Mode kits: every installed game mode's map kit as its own group (its markers,
// the components it uses, the creatures it spawns), "Made for" (which modes this map is for,
// saved as the worldspawn key ls_modes) and what each of those modes still needs, with Place.
// Drag a row onto the map, or Place and click (Shift keeps placing).

import { useEffect, useState } from "react";
import type { Issue } from "./editorApi";
import { dragging, useP4 } from "./extras";
import { kitFor, kitPlacement, loadKits, madeForKit, madeForOf, setMadeFor, type Kit, type KitsReply, useKits } from "./kits";
import { COMPONENT_MIME } from "./P4Scene";
import { useEditor } from "./store";
import "./p4.css";
import "./p5.css";

function needText(min: number, max: number | null) {
  if (max === min) return `${min}`;
  if (max == null) return min ? `${min} or more` : "any number";
  return min ? `${min}–${max}` : `up to ${max}`;
}

/** Starts placing a kit marker or component (click on the map). */
export function placeKit(id: string) {
  const k = kitPlacement(id.startsWith("kit:") ? id : `kit:${id}`);
  if (!k) return useEditor.getState().notify("That mode's kit isn't installed.", true);
  useP4.getState().set({ placing: { id: id.startsWith("kit:") ? id : `kit:${id}`, name: k.name, component: true }, turn: 0 });
}

function Row({ id, colour, title, sub, note }: { id: string; colour: string; title: string; sub: string; note?: string }) {
  return (
    <div
      className="p4-comp"
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData(COMPONENT_MIME, id);
        e.dataTransfer.setData("text/plain", id);
        e.dataTransfer.effectAllowed = "copy";
        dragging.current = { id, size: [32, 32, 32], component: true };
      }}
      onDragEnd={() => (dragging.current = null)}
      title={`${note ? `${note}\n` : ""}Drag onto the map.`}
    >
      <span className="p4-comp-dot" style={{ background: colour }} aria-hidden="true" />
      <span className="p4-comp-text">
        <strong>{title}</strong>
        <span className="muted">{sub}</span>
      </span>
      <button type="button" className="btn small" title="Then click on the map" aria-label={`Place ${title}`} onClick={() => placeKit(id)}>
        Place
      </button>
    </div>
  );
}

function KitGroup({ kit, chosen, needs }: { kit: Kit; chosen: boolean; needs: Issue[] }) {
  const [open, setOpen] = useState(chosen);
  useEffect(() => setOpen(chosen), [chosen]);
  return (
    <div className={`p5-kit${chosen ? " chosen" : ""}`} role="group" aria-label={`${kit.mode.name} kit`}>
      <button type="button" className="p5-kit-head" aria-expanded={open} onClick={() => setOpen(!open)}>
        <span aria-hidden="true">{open ? "▾" : "▸"}</span> <strong>{kit.mode.name}</strong>
        <span className="muted small-note">{kit.mode.description}</span>
        {chosen && <span className="p5-kit-tag">{needs.length ? `needs ${needs.length}` : "ready"}</span>}
      </button>
      {open && (
        <div className="stack-tight">
          {chosen && needs.length > 0 && (
            <ul className="p5-kit-needs" aria-label={`What ${kit.mode.name} still needs`}>
              {needs.map((n, i) => (
                <li key={i}>
                  <span>{n.message}</span>
                  {n.place && (
                    <button type="button" className="btn small" onClick={() => placeKit(n.place!)}>
                      Place
                    </button>
                  )}
                </li>
              ))}
            </ul>
          )}
          {kit.markers.length > 0 && <h4 className="p4-head">Markers</h4>}
          <div className="p4-comp-list">
            {kit.markers.map((m) => (
              <Row
                key={m.id}
                id={`kit:${kit.mode.id}:${m.id}`}
                colour={m.draw.colour || "#e8c547"}
                title={m.label}
                sub={`${m.type === "path" ? "Path" : m.type === "area" ? "Area" : "Spot"} · ${needText(m.required.min, m.required.max)}${m.description ? ` · ${m.description}` : ""}`}
                note={m.description}
              />
            ))}
          </div>
          {kit.components.length > 0 && <h4 className="p4-head">Components it uses</h4>}
          <div className="p4-comp-list">
            {kit.components.map((c) => (
              <Row key={c.classname} id={`kit:${kit.mode.id}:${c.classname}`} colour="#ff7a45" title={c.classname} sub={`${needText(c.min, c.max)}${c.note ? ` · ${c.note}` : ""}`} note={c.note} />
            ))}
          </div>
          {kit.creatures.length > 0 && <p className="small-note muted">Creatures it spawns: {kit.creatures.join(", ")}. Give them room and bot paths.</p>}
          {kit.notes && <p className="small-note muted">{kit.notes}</p>}
        </div>
      )}
    </div>
  );
}

export function KitPanel() {
  const doc = useEditor((s) => s.doc);
  const kits = useKits((s) => s.kits);
  const [reply, setReply] = useState<KitsReply | null>(null);
  const [busy, setBusy] = useState(false);
  const rev = doc?.revision;
  useEffect(() => {
    let live = true;
    void loadKits().then((r) => live && setReply(r));
    return () => {
      live = false;
    };
  }, [rev]);
  const made = madeForOf(doc?.world);
  // which made-for entry is this kit (by uid; old data by short id)
  const isMade = (k: Kit) => made.some((m) => kitFor(m, kits) === k);
  const toggle = async (k: Kit) => {
    setBusy(true);
    try {
      // (an old plain id becomes uid:id when it's set again)
      await setMadeFor(isMade(k) ? made.filter((m) => kitFor(m, kits) !== k) : [...made, madeForKit(k)]);
    } finally {
      setBusy(false);
    }
  };
  if (!reply) {
    return (
      <p className="small-note muted">
        <span className="spinner" aria-hidden="true" />
        Reading the installed modes' kits…
      </p>
    );
  }
  const sorted = [...kits].sort((a, b) => Number(isMade(b)) - Number(isMade(a)) || a.mode.name.localeCompare(b.mode.name));
  const unknown = made.filter((m) => !kitFor(m, kits));
  // (every kit issue starts with its mode's name)
  const needsOf = (k: Kit) => reply.needs.filter((n) => n.code !== "kit-unknown" && (n.message.startsWith(`${k.mode.name}:`) || n.message.startsWith(`${k.mode.name} `)));
  return (
    <div className="p4-prefabs">
      <fieldset className="p5-madefor">
        <legend>Made for</legend>
        {kits.length ? (
          <div className="seg" role="group" aria-label="Made for">
            {kits.map((k) => (
              <button key={k.mode.uid ?? k.mode.id} type="button" aria-pressed={isMade(k)} disabled={busy} onClick={() => void toggle(k)} title={k.mode.description}>
                {k.mode.name}
              </button>
            ))}
          </div>
        ) : (
          <p className="small-note muted">No mode kits are installed. Installing a game mode (Game Mode Studio, Community) adds its kit to {reply.folder}.</p>
        )}
        {unknown.map((m) => (
          <p key={m.uid ?? m.id} className="small-note p5-kit-missing" role="note">
            This map is also made for the mode “{m.id}”, which isn't installed here, so what it needs can't be checked. Install it (Game Mode Studio or Community) to see it.{" "}
            <button type="button" className="linklike" onClick={() => void setMadeFor(made.filter((x) => x !== m))}>
              Remove
            </button>
          </p>
        ))}
        <p className="small-note muted">Saved with the map. The checks list what each of these modes still needs, Playtest offers them first, and Publish says the map supports them.</p>
      </fieldset>
      {reply.bad.length > 0 && (
        <p className="small-note muted">
          {reply.bad.length} kit file{reply.bad.length === 1 ? " couldn't" : "s couldn't"} be read: {reply.bad.map((b) => `${b.file} (${b.error})`).join("; ")}
        </p>
      )}
      {sorted.map((k) => (
        <KitGroup key={k.mode.uid ?? k.mode.id} kit={k} chosen={isMade(k)} needs={needsOf(k)} />
      ))}
    </div>
  );
}
