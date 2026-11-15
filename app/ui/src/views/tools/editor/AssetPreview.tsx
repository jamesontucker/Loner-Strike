// The entity form's previews: play a sound an entity names (ambient_generic's message, a
// button's sound) and look at a model (cycler, env_model, a point entity's model key) in a
// small 3D view, using the asset browser's model viewer.

import { lazy, Suspense, useEffect, useState } from "react";
import { toggleSound, useSound } from "./soundPlayer";
import "./p4.css";

const ModelPreview = lazy(() => import("./ModelPreview"));

/** What kind of file a key's value names, if any. */
export function assetKind(value: string, fgdType?: string): "sound" | "model" | null {
  const v = value.trim().toLowerCase();
  if (fgdType === "sound" || /\.(wav|mp3)$/.test(v)) return v ? "sound" : null;
  if ((fgdType === "studio" || v.endsWith(".mdl")) && v.endsWith(".mdl")) return "model";
  return null;
}

/** Play/pause for one sound; starting it stops any other sound in the editor. */
export function SoundButton({ path, label }: { path: string; label?: string }) {
  const state = useSound(path);
  const name = label ?? path;
  return (
    <span className="p4-asset">
      <button
        type="button"
        className="btn small icon p5-sound-btn"
        title={state === "missing" ? "That sound isn't in the game's folders" : state === "playing" ? `Pause ${name}` : `Play ${name}`}
        aria-label={state === "playing" ? `Pause ${name}` : `Play ${name}`}
        aria-pressed={state === "playing"}
        data-state={state}
        onClick={(e) => {
          e.stopPropagation();
          toggleSound(path);
        }}
      >
        {state === "playing" ? "❚❚" : state === "loading" ? <span className="spinner" aria-hidden="true" /> : "▶"}
      </button>
      {state === "missing" && <span className="small-note muted">not found</span>}
    </span>
  );
}

/** A play button for sounds, a Look button for models. */
export function AssetPreview({ value, fgdType }: { value: string; fgdType?: string }) {
  const kind = assetKind(value, fgdType);
  const [open, setOpen] = useState(false);
  useEffect(() => setOpen(false), [value]);
  if (kind === "sound") return <SoundButton path={value.trim()} />;
  if (kind !== "model") return null;
  return (
    <>
      <button type="button" className="btn small" aria-pressed={open} title={`Look at ${value}`} onClick={() => setOpen((o) => !o)}>
        Look
      </button>
      {open && (
        <div className="p4-model">
          <Suspense fallback={<span className="muted">Opening the model…</span>}>
            <ModelPreview path={value.trim()} />
          </Suspense>
        </div>
      )}
    </>
  );
}
