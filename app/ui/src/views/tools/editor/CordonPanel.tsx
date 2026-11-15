// The cordon (Compile tab): compile only some boxes of the map, for quick looks at one area.
// In the panel: one line (on/off, how many boxes, the seal) and the quick "box round what's
// picked"; everything else in the Cordon dialog. Boxes come from what's picked or what's in
// view, are drawn in 3D (everything outside dimmed while the cordon is on), and are kept in the
// project. Sealed in sky (the sun gets in) or black (indoor light stays as in the full map);
// lights near the boxes can come along.

import { useState } from "react";
import type { Vec3 } from "../viewer/coords";
import type { CordonBox } from "./editorApi";
import { DialogSection, EditorDialog, Field } from "./EditorDialog";
import { cordonOf, setCordon, viewBox } from "./extras";
import { selectionBounds, useEditor } from "./store";
import "./p4.css";
import "./dialog.css";

const fmt = (v: Vec3) => v.map((x) => Math.round(x)).join(" ");

/** The picked things' box with a little room round it (walls aren't flush with the seal). */
function fromSelection(): CordonBox | null {
  const b = selectionBounds();
  if (!b) return null;
  return { min: b[0].map((x) => Math.floor((x - 64) / 16) * 16) as Vec3, max: b[1].map((x) => Math.ceil((x + 64) / 16) * 16) as Vec3 };
}

export function CordonPanel({ disabled }: { disabled?: boolean }) {
  const doc = useEditor((s) => s.doc);
  const sel = useEditor((s) => s.sel);
  const [open, setOpen] = useState(false);
  const c = cordonOf(doc);
  const hasSel = sel.brushes.length + sel.entities.length > 0;
  const on = c.on && c.boxes.length > 0;
  const summary = !c.boxes.length ? "Whole map" : `${on ? "Only inside" : "Off:"} ${c.boxes.length === 1 ? "1 box" : `${c.boxes.length} boxes`}, ${c.seal === "black" ? "black" : "sky"} seal${c.keepLights ? ", lights kept" : ""}`;
  return (
    <div className="p4-cordon-line">
      <label className="ed-check" title={c.boxes.length ? "Compile only what's inside the boxes (faster). Everything outside is dimmed in the view." : "Add a box first"}>
        <input type="checkbox" checked={on} disabled={disabled || !c.boxes.length} onChange={(e) => void setCordon({ on: e.target.checked })} />
        <span className="ed-ellipsis" title={summary}>
          Cordon: {summary}
        </span>
      </label>
      <span className="p4-actions">
        <button type="button" className="btn small" disabled={disabled || !hasSel} title={hasSel ? "Compile only a box round what's picked" : "Pick something first"} onClick={() => { const b = fromSelection(); if (b) void setCordon({ boxes: [b], on: true }); }}>
          Box round what's picked
        </button>
        <button type="button" className="btn small" disabled={disabled} onClick={() => setOpen(true)}>
          Cordon…
        </button>
      </span>
      {open && <CordonDialog onClose={() => setOpen(false)} />}
    </div>
  );
}

function CordonDialog({ onClose }: { onClose: () => void }) {
  const doc = useEditor((s) => s.doc);
  const sel = useEditor((s) => s.sel);
  const c = cordonOf(doc);
  const hasSel = sel.brushes.length + sel.entities.length > 0;
  const [msg, setMsg] = useState<string | null>(null);
  const add = (b: CordonBox | null, replace: boolean) => {
    if (!b) return setMsg("Pick some brushes or entities first.");
    setMsg(null);
    void setCordon({ boxes: replace ? [b] : [...c.boxes, b].slice(0, 32), on: true });
  };
  const fromView = () => {
    const b = doc ? viewBox(doc.bounds) : null;
    if (!b) return setMsg("Open a view of the map first.");
    add(b, false);
  };
  return (
    <EditorDialog
      title="Cordon"
      subtitle="Compile only part of the map: what's inside the boxes, sealed off. Kept with the project."
      onClose={onClose}
      closeLabel="Done"
      error={msg}
      secondary={
        <label className="ed-check">
          <input type="checkbox" checked={c.on && c.boxes.length > 0} disabled={!c.boxes.length} onChange={(e) => void setCordon({ on: e.target.checked })} /> Compile only inside the boxes
        </label>
      }
    >
      <DialogSection title="Boxes" aside={<span className="small-note muted">{c.boxes.length} of 32</span>}>
        <div className="ed-inline">
          <button type="button" className="btn small" disabled={!hasSel} title="One box round what's picked (replaces the others)" onClick={() => add(fromSelection(), true)}>
            Round what's picked
          </button>
          <button type="button" className="btn small" disabled={!hasSel} title="Another box round what's picked, compiled with the others" onClick={() => add(fromSelection(), false)}>
            Add one round what's picked
          </button>
          <button type="button" className="btn small" title="A box round what the view shows (flat views: all of it; 3D: where the camera looks)" onClick={fromView}>
            Add one from the view
          </button>
        </div>
        {c.boxes.length ? (
          <ul className="ed-list">
            {c.boxes.map((b, i) => (
              <li key={i}>
                <span className="mono" title={`${fmt(b.min)} to ${fmt(b.max)}`}>
                  {i + 1}. {fmt(b.min)} → {fmt(b.max)}
                </span>
                <button type="button" className="btn small icon" aria-label={`Remove box ${i + 1}`} title="Remove this box" onClick={() => void setCordon({ boxes: c.boxes.filter((_, j) => j !== i), on: c.boxes.length > 1 && c.on })}>
                  ×
                </button>
              </li>
            ))}
          </ul>
        ) : (
          <p className="small-note muted">No boxes yet: the whole map compiles.</p>
        )}
        {c.boxes.length > 0 && (
          <button type="button" className="btn small" onClick={() => void setCordon({ boxes: [], on: false })}>
            Remove all boxes
          </button>
        )}
      </DialogSection>
      <DialogSection title="How it's closed off" cols={2}>
        <Field label="Seal" hint="Sky lets the sun in (outdoor areas). Black keeps the light as it is in the whole map (indoors).">
          {(id) => (
            <select id={id} value={c.seal} onChange={(e) => void setCordon({ seal: e.target.value as "sky" | "black" })}>
              <option value="sky">Sky (outdoors)</option>
              <option value="black">Black (indoors)</option>
            </select>
          )}
        </Field>
        <Field label="Lights" hint="Lights up to 1024 units outside a box come along: the box grows to take them in, with what's between, so the area is lit as in the whole map.">
          {(id) => (
            <label className="ed-check" htmlFor={id}>
              <input id={id} type="checkbox" checked={c.keepLights} onChange={(e) => void setCordon({ keepLights: e.target.checked })} /> Keep lights from outside
            </label>
          )}
        </Field>
      </DialogSection>
      <p className="small-note muted">Touching boxes are joined. Links (target → targetname) crossing a box's edge are listed after compiling. Playtest puts you inside: at the 3D camera if it's in a box, else on a floor.</p>
    </EditorDialog>
  );
}
