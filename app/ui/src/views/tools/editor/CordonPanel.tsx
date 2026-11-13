// The cordon (Compile tab): compile only some boxes of the map, for quick looks at one area.
// Boxes come from what's picked or what's in view, are drawn in 3D (everything outside them
// dimmed while the cordon is on), and are kept in the project. Sealed in sky (the sun gets
// in) or black (indoor light stays as in the full map); lights near the boxes can come along.

import type { Vec3 } from "../viewer/coords";
import type { CordonBox } from "./editorApi";
import { cordonOf, setCordon, viewBox } from "./extras";
import { selectionBounds, useEditor } from "./store";
import "./p4.css";

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
  const c = cordonOf(doc);
  const hasSel = sel.brushes.length + sel.entities.length > 0;
  const notify = useEditor((s) => s.notify);
  const add = (b: CordonBox | null, replace: boolean) => {
    if (!b) return notify("Pick some brushes or entities first.", true);
    void setCordon({ boxes: replace ? [b] : [...c.boxes, b].slice(0, 32), on: true });
  };
  const fromView = () => {
    const b = doc ? viewBox(doc.bounds) : null;
    if (!b) return notify("Open a view of the map first.", true);
    add(b, false);
  };
  return (
    <fieldset className="p4-cordon" disabled={disabled}>
      <legend>Cordon: compile only part of the map</legend>
      <label className="editor-check" title="Compile only what's inside the boxes (faster). Everything outside is dimmed in the view.">
        <input type="checkbox" checked={c.on && c.boxes.length > 0} disabled={!c.boxes.length} onChange={(e) => void setCordon({ on: e.target.checked })} />
        {c.boxes.length ? `Only inside ${c.boxes.length === 1 ? "the box" : `the ${c.boxes.length} boxes`}` : "No cordon boxes yet"}
      </label>
      <div className="editor-row wrap">
        <button type="button" className="btn small" disabled={!hasSel} title="One box round what's picked" onClick={() => add(fromSelection(), true)}>
          Box round what's picked
        </button>
        <button type="button" className="btn small" disabled={!hasSel || !c.boxes.length} title="Another box, compiled together with the others" onClick={() => add(fromSelection(), false)}>
          Add one
        </button>
        <button type="button" className="btn small" title="A box round what the view shows (flat views: all of it; 3D: where the camera looks)" onClick={fromView}>
          From the view
        </button>
      </div>
      {c.boxes.length > 0 && (
        <div className="p4-cordon-boxes">
          {c.boxes.map((b, i) => (
            <div key={i} className="p4-cordon-box">
              <span className="mono" title={`${fmt(b.min)} to ${fmt(b.max)}`}>
                {fmt(b.min)} → {fmt(b.max)}
              </span>
              <button type="button" className="btn small icon" aria-label={`Remove box ${i + 1}`} title="Remove this box" onClick={() => void setCordon({ boxes: c.boxes.filter((_, j) => j !== i), on: c.boxes.length > 1 && c.on })}>
                ×
              </button>
            </div>
          ))}
        </div>
      )}
      <div className="editor-row wrap">
        <label className="editor-field compile-inline" title="What closes the area off. Sky lets the sun in; black keeps indoor lighting as it is in the whole map.">
          Seal
          <select value={c.seal} onChange={(e) => void setCordon({ seal: e.target.value as "sky" | "black" })}>
            <option value="sky">Sky (outdoors)</option>
            <option value="black">Black (indoors)</option>
          </select>
        </label>
        <label className="editor-check" title="Lights up to 1024 units outside a box come along (the box grows to take them in, with what's between), so the area is lit as in the whole map.">
          <input type="checkbox" checked={c.keepLights} onChange={(e) => void setCordon({ keepLights: e.target.checked })} /> Keep lights from outside
        </label>
      </div>
      <p className="small-note muted">Touching boxes are joined. Links (target → targetname) crossing a box's edge are listed after compiling. Playtest puts you inside: at the 3D camera if it's in a box, else on a floor.</p>
    </fieldset>
  );
}
