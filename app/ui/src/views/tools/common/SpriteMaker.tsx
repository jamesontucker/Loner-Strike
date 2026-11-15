// The Sprite maker (T1fix): one picture, or several as the frames of an animation, made into a
// GoldSrc sprite (.spr) by the server (tools/spritemaker.rs): crop and fit, a size in multiples
// of 8, how it faces you, how it's drawn, a see-through color, and a live animated preview of
// the sprite as the game draws it (the file read back). "Make sprite" hands the finished file
// to the caller, which saves it the checked way: the pickers' Import (into the game's own
// sprites) or the Content Browser's (into the map project).

import { useEffect, useRef, useState } from "react";
import { BusyButton, Segmented, Sheet, useToast } from "../../../ui";
import { post } from "./api";
import "./spriteMaker.css";

export type SpriteBuild = { ok?: boolean; error?: string; data?: string; size?: number; colors?: number; frames?: number; width?: number; height?: number; preview?: string[] };
export const buildSprite = (body: Record<string, unknown>) => post<SpriteBuild>("/api/tools/sprites/build", body);

const PICTURES = ".png,.jpg,.jpeg,.bmp,.tga,.webp";
const MAX_FRAMES = 64;
const SIDES = [8, 16, 24, 32, 48, 64, 96, 128, 192, 256, 384, 512];
const TYPES: [string, string][] = [
  ["2", "Always faces you (parallel)"],
  ["0", "Faces you, stays upright"],
  ["1", "Turns to you, stays upright"],
  ["3", "Flat, as placed (oriented)"],
  ["4", "Faces you, keeps its roll"],
];
const FORMATS: [string, string, string][] = [
  ["normal", "Normal", "Solid colors; clear parts come out black."],
  ["additive", "Glow (additive)", "Added to what's behind: black disappears. For fire, sparks, light."],
  ["alphatest", "See-through color", "Clear parts are see-through; everything else solid."],
  ["indexalpha", "One color", "One color, faded by each pixel's brightness: smoke, decals."],
];

type Frame = { name: string; url: string };
const readUrl = (f: File) =>
  new Promise<string>((ok, fail) => {
    const r = new FileReader();
    r.onload = () => ok(String(r.result));
    r.onerror = () => fail(r.error);
    r.readAsDataURL(f);
  });
const cleanName = (n: string) =>
  n
    .replace(/\.[^.]*$/, "")
    .toLowerCase()
    .replace(/[^a-z0-9_-]+/g, "_")
    .replace(/^_+|_+$/g, "")
    .slice(0, 40) || "my_sprite";

/** The maker in a sheet. `onMade` gets the finished file; it closes when that resolves true. */
export function SpriteMaker({ onClose, onMade, saveLabel = "Make sprite" }: { onClose: () => void; onMade: (file: File) => Promise<boolean>; saveLabel?: string }) {
  const toast = useToast();
  const [frames, setFrames] = useState<Frame[]>([]);
  const [name, setName] = useState("my_sprite");
  const [width, setWidth] = useState(64);
  const [height, setHeight] = useState(64);
  const [fit, setFit] = useState("contain");
  const [orientation, setOrientation] = useState("2");
  const [format, setFormat] = useState("additive");
  const [useKey, setUseKey] = useState(false);
  const [key, setKey] = useState("#000000");
  const [tint, setTint] = useState("#0000ff");
  const [crop, setCrop] = useState({ x: 0, y: 0, w: 100, h: 100 });
  const [fps, setFps] = useState(10);
  const [built, setBuilt] = useState<SpriteBuild | null>(null);
  const [building, setBuilding] = useState(false);
  const [saving, setSaving] = useState(false);
  const [shown, setShown] = useState(0);
  const [hot, setHot] = useState(false);
  const fileBox = useRef<HTMLInputElement>(null);
  const ask = useRef(0);

  const add = async (files: FileList | File[]) => {
    const list = [...files].filter((f) => /\.(png|jpe?g|bmp|tga|webp)$/i.test(f.name));
    if (!list.length) return void toast("Pick PNG, JPEG, BMP, TGA or WebP pictures.", true);
    const room = MAX_FRAMES - frames.length;
    if (list.length > room) toast(`A sprite holds ${MAX_FRAMES} frames: ${list.length - room} left out.`, true);
    const read = await Promise.all(list.slice(0, Math.max(room, 0)).map(async (f) => ({ name: f.name, url: await readUrl(f) })));
    if (!frames.length && read[0]) setName(cleanName(read[0].name));
    setFrames((fr) => [...fr, ...read]);
  };

  const body = () => ({
    frames: frames.map((f) => f.url),
    width,
    height,
    orientation: Number(orientation),
    format,
    fit,
    crop: crop.x === 0 && crop.y === 0 && crop.w === 100 && crop.h === 100 ? null : { x: crop.x / 100, y: crop.y / 100, w: crop.w / 100, h: crop.h / 100 },
    key: useKey ? key : null,
    tint,
  });
  const sizeBad = width % 8 !== 0 || height % 8 !== 0 || width < 8 || height < 8 || width > 512 || height > 512;

  // the live preview: the sprite made and read back, a moment after the last change
  useEffect(() => {
    if (!frames.length || sizeBad) {
      setBuilt(null);
      return;
    }
    const mine = ++ask.current;
    setBuilding(true);
    const t = setTimeout(async () => {
      const r = await buildSprite(body());
      if (mine !== ask.current) return;
      setBuilding(false);
      setBuilt(r);
    }, 350);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [frames, width, height, fit, orientation, format, useKey, key, tint, crop]);

  // the animation
  const count = built?.preview?.length ?? 0;
  useEffect(() => {
    setShown(0);
    if (count < 2) return;
    const id = setInterval(() => setShown((s) => (s + 1) % count), 1000 / Math.max(1, fps));
    return () => clearInterval(id);
  }, [count, fps]);

  const make = async () => {
    if (!/^[a-z0-9_-]{1,40}$/.test(name)) return void toast("A name is small letters, digits, - and _.", true);
    setSaving(true);
    try {
      const r = await buildSprite(body());
      if (r.error || !r.data) return void toast(r.error || "The sprite couldn't be made.", true);
      const bytes = Uint8Array.from(atob(r.data), (c) => c.charCodeAt(0));
      if (await onMade(new File([bytes], `${name}.spr`, { type: "application/octet-stream" }))) onClose();
    } finally {
      setSaving(false);
    }
  };

  const fmt = FORMATS.find((f) => f[0] === format)!;
  return (
    // (an alertdialog: the picker it was opened from stays open behind it, as for its rename question)
    <Sheet className="sheet wide sm-sheet" label="Make a sprite" onClose={onClose} role="alertdialog">
      <div className="sheet-body sm">
        <h2>Make a sprite</h2>
        <p className="muted">One picture, or several as the frames of an animation. It comes out as a .spr the game reads.</p>
        <div className="sm-grid">
          <div className="sm-left">
            <div
              className={`sm-drop${hot ? " hot" : ""}`}
              onDragOver={(e) => {
                if (!e.dataTransfer.types.includes("Files")) return;
                e.preventDefault();
                setHot(true);
              }}
              onDragLeave={() => setHot(false)}
              onDrop={(e) => {
                e.preventDefault();
                setHot(false);
                void add(e.dataTransfer.files);
              }}
            >
              <button type="button" className="btn" onClick={() => fileBox.current?.click()}>
                {frames.length ? "Add frames…" : "Choose pictures…"}
              </button>
              <span className="muted">or drop them here</span>
              <input ref={fileBox} type="file" hidden multiple accept={PICTURES} aria-label="Pictures for the sprite" onChange={(e) => e.target.files && void add(e.target.files)} />
            </div>
            {frames.length > 0 && (
              <ol className="sm-frames" aria-label="Frames">
                {frames.map((f, i) => (
                  <li key={i}>
                    <img src={f.url} alt={`Frame ${i + 1}`} />
                    <button type="button" className="btn small icon" aria-label={`Remove frame ${i + 1}`} onClick={() => setFrames(frames.filter((_, j) => j !== i))}>
                      ×
                    </button>
                  </li>
                ))}
              </ol>
            )}
            <div className="sm-preview" aria-label="Preview">
              {built?.preview?.length ? (
                <img src={built.preview[Math.min(shown, built.preview.length - 1)]} alt="The sprite as the game draws it" style={{ width: Math.min(256, built.width! * 2), imageRendering: "pixelated" }} />
              ) : (
                <span className="muted">{built?.error ?? (frames.length ? (sizeBad ? "The size must be a multiple of 8, from 8 to 512." : "Making a preview…") : "No pictures yet")}</span>
              )}
              {building && frames.length > 0 && <span className="spinner sm-busy" aria-label="Updating the preview" />}
            </div>
            {built?.ok && (
              <p className="muted sm-facts" role="status">
                {built.frames} frame{built.frames === 1 ? "" : "s"} · {built.width} × {built.height} · {built.colors} colors · {Math.max(1, Math.round((built.size ?? 0) / 1024))} KB
              </p>
            )}
            {count > 1 && (
              <label className="sm-fps">
                Speed <input type="range" min={1} max={30} value={fps} aria-label="Preview speed (frames a second)" onChange={(e) => setFps(Number(e.target.value))} /> {fps} a second
              </label>
            )}
          </div>
          <div className="sm-right">
            <label className="sm-field">
              <span>Name</span>
              <span className="row nowrap">
                <input className="search" aria-label="Sprite name" value={name} maxLength={40} spellCheck={false} onChange={(e) => setName(e.target.value.toLowerCase())} />
                <span className="muted">.spr</span>
              </span>
            </label>
            <div className="sm-field">
              <span>Size</span>
              <span className="row nowrap">
                <select aria-label="Width" value={width} onChange={(e) => setWidth(Number(e.target.value))}>
                  {SIDES.map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
                ×
                <select aria-label="Height" value={height} onChange={(e) => setHeight(Number(e.target.value))}>
                  {SIDES.map((s) => (
                    <option key={s} value={s}>
                      {s}
                    </option>
                  ))}
                </select>
              </span>
            </div>
            <div className="sm-field">
              <span>Fit</span>
              <Segmented values="contain cover stretch" labels="Whole|Fill|Stretch" current={fit} onPick={setFit} label="Fit" />
            </div>
            <details className="sm-crop">
              <summary>Crop (every frame)</summary>
              {(["x", "y", "w", "h"] as const).map((k) => (
                <label key={k} className="sm-crop-row">
                  <span>{{ x: "From left", y: "From top", w: "Width", h: "Height" }[k]}</span>
                  <input type="range" min={k === "w" || k === "h" ? 1 : 0} max={100} value={crop[k]} aria-label={`Crop: ${{ x: "from left", y: "from top", w: "width", h: "height" }[k]}`} onChange={(e) => setCrop({ ...crop, [k]: Number(e.target.value) })} />
                  <span className="muted">{crop[k]}%</span>
                </label>
              ))}
              <button type="button" className="btn small" onClick={() => setCrop({ x: 0, y: 0, w: 100, h: 100 })}>
                Whole picture
              </button>
            </details>
            <label className="sm-field">
              <span>Faces</span>
              <select aria-label="How it faces you" value={orientation} onChange={(e) => setOrientation(e.target.value)}>
                {TYPES.map(([v, l]) => (
                  <option key={v} value={v}>
                    {l}
                  </option>
                ))}
              </select>
            </label>
            <label className="sm-field">
              <span>Drawn as</span>
              <select aria-label="How it's drawn" value={format} onChange={(e) => setFormat(e.target.value)}>
                {FORMATS.map(([v, l]) => (
                  <option key={v} value={v}>
                    {l}
                  </option>
                ))}
              </select>
            </label>
            <p className="muted sm-note">{fmt[2]}</p>
            {(format === "alphatest" || format === "indexalpha") && (
              <label className="sm-field">
                <span>{format === "indexalpha" ? "Its color" : "See-through color"}</span>
                <input type="color" aria-label={format === "indexalpha" ? "Its color" : "See-through color"} value={tint} onChange={(e) => setTint(e.target.value)} />
              </label>
            )}
            <label className="tl-check sm-key">
              <input type="checkbox" checked={useKey} onChange={(e) => setUseKey(e.target.checked)} /> Make a color clear
              {useKey && <input type="color" aria-label="Color to make clear" value={key} onChange={(e) => setKey(e.target.value)} />}
            </label>
          </div>
        </div>
        <div className="row sm-actions">
          <BusyButton className="btn primary" busy={saving} disabled={!frames.length || sizeBad || !!built?.error} label={saveLabel} busyLabel="Making the sprite…" onClick={() => void make()} />
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
        </div>
      </div>
    </Sheet>
  );
}

/** "Make sprite…": opens the maker; the finished file goes to `onMade` (true: saved). */
export function MakeSpriteButton({ onMade, className = "btn small" }: { onMade: (file: File) => Promise<boolean>; className?: string }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button type="button" className={className} onClick={() => setOpen(true)} title="Make a sprite from pictures">
        Make sprite…
      </button>
      {open && <SpriteMaker onClose={() => setOpen(false)} onMade={onMade} />}
    </>
  );
}
