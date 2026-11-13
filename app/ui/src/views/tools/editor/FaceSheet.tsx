// The face sheet (Hammer's texture application tool, modernised): the picked faces' texture
// and its numbers, justify and align buttons, treat-as-one, recent textures. In the 3D view
// the picked face can be dragged to slide its texture, with handles to turn and stretch it.

import { useEffect, useState } from "react";
import { Info } from "../../../ui";
import { alignOf, texUrl } from "./editorApi";
import { facesOf, useEditor, type FaceRef } from "./store";

const round = (x: number, k = 1000) => Math.round(x * k) / k;

/** A number box that sends on Enter or when you leave it (only if it changed). */
function NumField({ label, value, step, onSet, title }: { label: string; value: number | null; step: number; onSet: (v: number) => void; title?: string }) {
  const shown = value == null ? "" : String(round(value));
  const [text, setText] = useState(shown);
  useEffect(() => setText(shown), [shown]);
  const commit = () => {
    const n = Number(text);
    if (text.trim() === "" || !Number.isFinite(n) || text === shown) return setText(shown);
    onSet(n);
  };
  return (
    <label className="editor-num" title={title}>
      <span>{label}</span>
      <input
        type="number"
        step={step}
        value={text}
        placeholder={value == null ? "mixed" : undefined}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "Escape") setText(shown);
        }}
      />
    </label>
  );
}

export function RecentTextures({ onPick, onPaint }: { onPick: (t: string) => void; onPaint?: (t: string) => void }) {
  const recent = useEditor((s) => s.recentTex);
  const current = useEditor((s) => s.texture);
  if (!recent.length) return null;
  return (
    <div className="editor-recent" role="list" aria-label="Recent textures">
      {recent.map((t) => (
        <button
          key={t}
          type="button"
          role="listitem"
          className="editor-recent-tex"
          aria-pressed={t.toLowerCase() === current.toLowerCase()}
          title={`${t}${onPaint ? " (double-click to paint the picked faces)" : ""}`}
          onClick={() => onPick(t)}
          onDoubleClick={() => onPaint?.(t)}
        >
          <img src={texUrl(t)} alt={t} onError={(e) => (e.currentTarget.style.visibility = "hidden")} />
        </button>
      ))}
    </div>
  );
}

export function FaceSheet() {
  const sel = useEditor((s) => s.sel);
  const brushes = useEditor((s) => s.brushes);
  const texture = useEditor((s) => s.texture);
  const grid = useEditor((s) => s.grid);
  const one = useEditor((s) => s.treatAsOne);
  const lock = useEditor((s) => s.textureLock);
  const [fit, setFit] = useState<[number, number]>([1, 1]);
  const s = useEditor.getState();
  const refs = facesOf(sel);
  const faces = refs.map((r) => brushes.get(r.brush)?.faces.find((x) => x.face === r.face)).filter((f) => !!f);
  const first = faces[0];
  const a = alignOf(first);
  const aligns = faces.map(alignOf);
  // a number is shown only when every picked face has the same one
  const same = (k: "ou" | "ov" | "su" | "sv" | "rot") => (a && aligns.every((x) => x && Math.abs(x[k] - a[k]) < 1e-6) ? a[k] : null);
  const texNames = [...new Set(faces.map((f) => f.texture))];
  const list = refs.map((r: FaceRef) => [r.brush, r.face]);
  const align = (action: string, extra: Record<string, unknown> = {}) => void s.cmd({ op: "align", faces: list, action, one, ...extra });
  const set = (k: string, v: number) => align("set", { [k]: v });
  const paint = (t: string) => void s.cmd({ op: "set_texture", faces: list, texture: t });
  return (
    <section className="tools-panel editor-facesheet">
      <h3>
        {refs.length === 1 ? "Face" : `${refs.length} faces`}{" "}
        <Info text="Shift-click faces in the view to pick more. Drag a picked face to slide its texture; drag the round handles to turn it (ring) or stretch it (the ends of the arrows)." />
      </h3>
      <div className="editor-face-tex">
        {first && <img src={texUrl(first.texture)} alt="" onError={(e) => (e.currentTarget.style.visibility = "hidden")} />}
        <div>
          <div className="mono">{texNames.length === 1 ? texNames[0] : `${texNames.length} textures`}</div>
          <div className="editor-row wrap">
            <button type="button" className="btn small" disabled={!faces.length || texNames.every((t) => t === texture)} onClick={() => paint(texture)} title="Put the current texture on the picked faces">
              Paint “{texture}”
            </button>
            {texNames.length === 1 && texNames[0] !== texture && (
              <button type="button" className="btn small" onClick={() => s.set({ texture: texNames[0] })}>
                Use this one
              </button>
            )}
          </div>
        </div>
      </div>
      <RecentTextures onPick={(t) => s.set({ texture: t })} onPaint={paint} />

      <div className="editor-sheet-grid">
        <span className="editor-sheet-head" />
        <span className="editor-sheet-head">Across</span>
        <span className="editor-sheet-head">Down</span>
        <span className="editor-sheet-label" title="Texture offset in pixels">Shift</span>
        <NumField label="Shift across" value={same("ou")} step={grid} onSet={(v) => set("ou", v)} />
        <NumField label="Shift down" value={same("ov")} step={grid} onSet={(v) => set("ov", v)} />
        <span className="editor-sheet-label" title="Units per texture pixel (1 is normal, 0.5 is twice as dense)">Scale</span>
        <NumField label="Scale across" value={same("su")} step={0.05} onSet={(v) => set("su", v)} />
        <NumField label="Scale down" value={same("sv")} step={0.05} onSet={(v) => set("sv", v)} />
        <span className="editor-sheet-label">Turn</span>
        <NumField label="Rotation (degrees)" value={same("rot")} step={15} onSet={(v) => set("rot", v)} />
        <span />
      </div>
      <div className="editor-row wrap editor-nudges">
        <button type="button" className="btn small" title={`Slide left ${grid} pixels`} onClick={() => align("shift", { u: -grid, v: 0 })}>←</button>
        <button type="button" className="btn small" title={`Slide right ${grid} pixels`} onClick={() => align("shift", { u: grid, v: 0 })}>→</button>
        <button type="button" className="btn small" title={`Slide up ${grid} pixels`} onClick={() => align("shift", { u: 0, v: -grid })}>↑</button>
        <button type="button" className="btn small" title={`Slide down ${grid} pixels`} onClick={() => align("shift", { u: 0, v: grid })}>↓</button>
        <span className="editor-gap" />
        {[-90, -15, 15, 90].map((d) => (
          <button key={d} type="button" className="btn small" title="Turn the texture" onClick={() => align("rotate", { degrees: d })}>
            {d > 0 ? `+${d}°` : `${d}°`}
          </button>
        ))}
      </div>

      <h4>
        Justify <Info text="Line the texture up with an edge of the face, or centre it, or fit it once." />
      </h4>
      <div className="editor-justify" role="group" aria-label="Justify">
        {(
          [
            ["left", "Left"],
            ["right", "Right"],
            ["top", "Top"],
            ["bottom", "Bottom"],
            ["center", "Centre"],
            ["fit", "Fit"],
          ] as const
        ).map(([m, l]) => (
          <button key={m} type="button" className="btn small" onClick={() => align("justify", { mode: m })}>
            {l}
          </button>
        ))}
      </div>
      <label className="editor-check" title="Justify, fit and centre work across all the picked faces as one surface, like a wall made of several brushes">
        <input type="checkbox" checked={one} onChange={(e) => s.set({ treatAsOne: e.target.checked })} /> Treat as one
      </label>
      <div className="editor-row">
        <span className="muted">Fit</span>
        <input type="number" min={0.1} step={1} value={fit[0]} onChange={(e) => setFit([Number(e.target.value), fit[1]])} aria-label="Repeats across" title="Repeats across" />
        <span className="muted">×</span>
        <input type="number" min={0.1} step={1} value={fit[1]} onChange={(e) => setFit([fit[0], Number(e.target.value)])} aria-label="Repeats down" title="Repeats down" />
        <button type="button" className="btn small" onClick={() => align("fit", { u: fit[0], v: fit[1] })}>
          Fit
        </button>
      </div>
      <h4>Align</h4>
      <div className="editor-row wrap">
        <button type="button" className="btn small" title="Project from the world's axes, like a floor (Hammer's default)" onClick={() => align("world")}>
          To the world
        </button>
        <button type="button" className="btn small" title="Lie flat along this face (good for slopes)" onClick={() => align("face")}>
          To the face
        </button>
      </div>
      <label className="editor-check" title="Textures stay put on the surface when you move, turn or stretch a brush">
        <input type="checkbox" checked={lock} onChange={(e) => s.set({ textureLock: e.target.checked })} /> Texture lock when moving brushes
      </label>
    </section>
  );
}
