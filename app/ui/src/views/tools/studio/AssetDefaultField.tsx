// A Model, Sprite or Sound value (Studio types, the asset track): the file picker (Choose…,
// Import…, drop a file, your map projects) with a preview (a model in 3D, a sprite's first frame,
// a sound's play button). Used for file variables' start values, the Model / Sprite / Sound
// nodes and the typed file settings.

import { useEffect, useState } from "react";
import { AssetField } from "../common/AssetField";
import { AssetPreview } from "../editor/AssetPreview";
import { spriteUrl } from "../editor/contentApi";
import { validAsset, type AssetType } from "./schema";
import { nodeDef } from "./registry";

const FOLDERS = ["cstrike", "cstrike_downloads", "czero", "valve"];

/** A sprite's first frame, looked for in the game's folders in the order the game reads them. */
function SpriteThumb({ path }: { path: string }) {
  const [at, setAt] = useState(0);
  useEffect(() => setAt(0), [path]);
  if (at >= FOLDERS.length) return <span className="small-note muted">not found</span>;
  return <img className="st-asset-thumb" src={spriteUrl({ folder: FOLDERS[at], path }, 0)} alt={`First frame of ${path}`} onError={() => setAt((a) => a + 1)} />;
}

export function AssetDefaultField({ kind, value, onChange, disabled, label }: { kind: AssetType | "decal" | "visual"; value: string; onChange: (v: string) => void; disabled?: boolean; label?: string }) {
  // (a Decal value is one of Place Decal's kinds)
  if (kind === "decal") {
    const kinds = (nodeDef("fx.decal")?.params.find((p) => p.id === "decal")?.choices ?? []) as [string, string][];
    return (
      <select className="st-field" aria-label={label ?? "Decal"} value={value} disabled={disabled} onChange={(e) => onChange(e.target.value)}>
        <option value="">None</option>
        {kinds.map(([k, l]) => (
          <option key={k} value={k}>
            {l}
          </option>
        ))}
      </select>
    );
  }
  const pick: AssetType = kind === "visual" ? (value.toLowerCase().endsWith(".spr") ? "sprite" : "model") : kind;
  const ok = validAsset(kind, value);
  return (
    <div className="st-asset-default" aria-disabled={disabled}>
      <AssetField kind={pick} value={value} max={64} allowed={/^[A-Za-z0-9_\-./]*$/} label={label} onChange={(v) => !disabled && onChange(v)} />
      {value && !ok && (
        <span className="st-warn small-note" role="alert">
          {pick === "sound" ? "A sound from inside sound/, ending .wav" : pick === "sprite" ? "A sprite: sprites/….spr" : "A model: models/….mdl"}
        </span>
      )}
      {value && ok && (pick === "sprite" ? <SpriteThumb path={value} /> : <AssetPreview value={value} />)}
    </div>
  );
}
