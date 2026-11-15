// Get Bone Position / Get Attachment Position (Y): the bones (by name) or attachments (by number)
// of the node's Model, read from the game by the app (GET /api/tools/studio/model-points).
import { useEffect, useState } from "react";
import { getJson } from "../../../api";

type Points = { bones?: string[]; attachments?: { index: number; name: string; bone: string }[]; fallback?: boolean; error?: string };

export function ModelPointPicker({ kind, model, value, onChange }: { kind: "bone" | "attachment"; model: string; value: string | number; onChange: (v: string | number) => void }) {
  const [pts, setPts] = useState<Points | null>(null);
  useEffect(() => {
    let live = true;
    setPts(null);
    getJson<Points>(`/api/tools/studio/model-points?model=${encodeURIComponent(model || "models/player.mdl")}`)
      .then((p) => live && setPts(p))
      .catch(() => live && setPts({ error: "Couldn't read the model's list." }));
    return () => {
      live = false;
    };
  }, [model]);
  if (!pts)
    return (
      <span className="st-muted">
        <span className="spinner" aria-hidden="true" />
        Reading the model…
      </span>
    );
  if (kind === "bone") {
    const list = pts.bones ?? [];
    const cur = String(value);
    return (
      <>
        <select className="st-field" value={list.includes(cur) ? cur : ""} onChange={(e) => onChange(e.target.value)} aria-label="Bone">
          <option value="">{list.length ? (cur && !list.includes(cur) ? `${cur} (not in this model)` : "Pick a bone…") : pts.error ?? "This model has no bones"}</option>
          {list.map((b) => (
            <option key={b} value={b}>
              {b}
            </option>
          ))}
        </select>
        {pts.fallback && <p className="st-side-help">The standard player skeleton (the game isn't set up here).</p>}
      </>
    );
  }
  const atts = pts.attachments ?? [];
  const cur = Number(value) || 0;
  return (
    <select className="st-field" value={String(cur)} onChange={(e) => onChange(Number(e.target.value))} aria-label="Attachment">
      {atts.length === 0 && <option value={String(cur)}>{pts.error ?? (pts.fallback ? `Attachment ${cur}` : "This model has no attachments")}</option>}
      {atts.map((a) => (
        <option key={a.index} value={String(a.index)}>
          {`Attachment ${a.index}${a.name ? `: ${a.name}` : ""}${a.bone ? ` (on ${a.bone})` : ""}`}
        </option>
      ))}
    </select>
  );
}
