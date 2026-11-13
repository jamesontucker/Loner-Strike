// The composer's side panels: what to add (pictures, words, stickers), the list of layers,
// and the picked layer's settings (or the canvas's when nothing is picked).

import { useEffect, useRef, type ReactNode } from "react";
import { Info, Segmented, Switch } from "../../../ui";
import { FONTS, SHAPES, type Doc, type ImageLayer, type Layer, type Shape, type ShapeLayer, type TextLayer } from "./doc";
import { STICKERS, type Sticker } from "./stickers";

/** A sticker drawn small, for its button. */
export function StickerIcon({ s, size = 34 }: { s: Sticker; size?: number }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = ref.current;
    const g = c?.getContext("2d");
    if (!c || !g) return;
    const px = size * 2;
    g.clearRect(0, 0, px, px);
    const a = s.aspect ?? 1;
    const k = (px * 0.82) / 100 / Math.max(1, a);
    g.setTransform(k, 0, 0, k * a, px / 2, px / 2);
    if (s.stroke) {
      g.strokeStyle = s.colour ?? "#fff";
      g.lineWidth = s.stroke;
      g.lineCap = "round";
      g.stroke(s.path());
    } else {
      g.fillStyle = s.colour ?? "#fff";
      g.fill(s.path(), "evenodd");
    }
  }, [s, size]);
  return <canvas ref={ref} width={size * 2} height={size * 2} style={{ width: size, height: size }} aria-hidden="true" />;
}

export function AddPanel({ onPicture, onWords, onSticker }: { onPicture: () => void; onWords: () => void; onSticker: (id: string) => void }) {
  return (
    <section className="panel sc-add" aria-label="Add">
      <h3>Add</h3>
      <div className="sc-add-row">
        <button type="button" className="btn" onClick={onPicture}>
          Picture…
        </button>
        <button type="button" className="btn" onClick={onWords}>
          Words
        </button>
      </div>
      <div className="sc-stickers" role="group" aria-label="Stickers">
        {STICKERS.map((s) => (
          <button key={s.id} type="button" className="sc-sticker" title={s.label} aria-label={`Add a ${s.label.toLowerCase()} sticker`} onClick={() => onSticker(s.id)}>
            <StickerIcon s={s} size={30} />
          </button>
        ))}
      </div>
    </section>
  );
}

const KIND_WORD: Record<Layer["kind"], string> = { image: "Picture", text: "Words", shape: "Sticker" };

function Icon({ d }: { d: string }) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={d} />
    </svg>
  );
}
const EYE = "M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12zm10 3a3 3 0 1 0 0-6 3 3 0 0 0 0 6z";
const EYE_OFF = "M3 3l18 18M10.6 5.1A10.7 10.7 0 0 1 12 5c6.5 0 10 7 10 7a17 17 0 0 1-3.2 4.1M6.3 6.3A17 17 0 0 0 2 12s3.5 7 10 7a10 10 0 0 0 4.7-1.2";
const LOCK = "M6 11h12v10H6zM8 11V7a4 4 0 0 1 8 0v4";
const UNLOCK = "M6 11h12v10H6zM8 11V7a4 4 0 0 1 7.5-2";

export function LayerList({ doc, selected, onSelect, onChange, onMove, onDuplicate, onDelete }: {
  doc: Doc;
  selected: string | null;
  onSelect: (id: string) => void;
  onChange: (id: string, v: Partial<Layer>) => void;
  onMove: (id: string, by: number) => void;
  onDuplicate: (id: string) => void;
  onDelete: (id: string) => void;
}) {
  const top = [...doc.layers].reverse();
  const sel = doc.layers.find((l) => l.id === selected);
  const at = sel ? doc.layers.indexOf(sel) : -1;
  return (
    <section className="panel sc-layers" aria-label="Layers">
      <div className="sc-layers-head">
        <h3>Layers</h3>
        <Info text="The top of the list is drawn on top. Click a layer to pick it; the eye hides it, the lock stops it being picked on the canvas. Keys: Delete, Mod+D duplicate, [ and ] move down and up, arrows nudge (Shift: 10)." />
      </div>
      {top.length === 0 ? (
        <p className="muted sc-none">Nothing yet. Add a picture, words or a sticker, or start from a template.</p>
      ) : (
        <ul className="sc-layer-list" role="listbox" aria-label="Layers, top first">
          {top.map((l) => (
            <li key={l.id} role="option" aria-selected={l.id === selected} className={`sc-layer${l.id === selected ? " on" : ""}${l.hidden ? " hidden" : ""}`} onClick={() => onSelect(l.id)}>
              <span className={`sc-kind ${l.kind}`} aria-hidden="true">
                {l.kind === "image" ? <img src={l.src} alt="" /> : l.kind === "text" ? "Aa" : <StickerIcon s={STICKERS.find((s) => s.id === (l as ShapeLayer).shape) ?? STICKERS[0]} size={18} />}
              </span>
              <span className="sc-layer-name" title={l.name}>
                {l.kind === "text" ? (l as TextLayer).text.split("\n")[0] || l.name : l.name}
                <small className="muted">{KIND_WORD[l.kind]}</small>
              </span>
              <button type="button" className="sc-icon-btn" aria-pressed={l.hidden} aria-label={l.hidden ? `Show ${l.name}` : `Hide ${l.name}`} title={l.hidden ? "Show" : "Hide"} onClick={(e) => (e.stopPropagation(), onChange(l.id, { hidden: !l.hidden }))}>
                <Icon d={l.hidden ? EYE_OFF : EYE} />
              </button>
              <button type="button" className="sc-icon-btn" aria-pressed={l.locked} aria-label={l.locked ? `Unlock ${l.name}` : `Lock ${l.name}`} title={l.locked ? "Unlock" : "Lock"} onClick={(e) => (e.stopPropagation(), onChange(l.id, { locked: !l.locked }))}>
                <Icon d={l.locked ? LOCK : UNLOCK} />
              </button>
            </li>
          ))}
        </ul>
      )}
      <div className="sc-layer-actions">
        <button type="button" className="btn small" disabled={!sel || at === doc.layers.length - 1} onClick={() => sel && onMove(sel.id, 1)} title="Move up (])">
          Up
        </button>
        <button type="button" className="btn small" disabled={!sel || at === 0} onClick={() => sel && onMove(sel.id, -1)} title="Move down ([)">
          Down
        </button>
        <button type="button" className="btn small" disabled={!sel} onClick={() => sel && onDuplicate(sel.id)} title="Duplicate (Mod+D)">
          Duplicate
        </button>
        <button type="button" className="btn small danger" disabled={!sel} onClick={() => sel && onDelete(sel.id)} title="Delete (Delete)">
          Delete
        </button>
      </div>
    </section>
  );
}

/** One setting: its name on the left, the control on the right. */
function Field({ name, children, info }: { name: ReactNode; children: ReactNode; info?: ReactNode }) {
  return (
    <div className="sc-field">
      <span className="sc-field-name">
        {name}
        {info && <Info text={info} />}
      </span>
      <div className="sc-field-control">{children}</div>
    </div>
  );
}

function Slider({ value, min, max, step, onChange, label, unit = "", digits = 0, scale = 1 }: { value: number; min: number; max: number; step: number; onChange: (v: number) => void; label: string; unit?: string; digits?: number; scale?: number }) {
  return (
    <span className="sc-slider">
      <input type="range" min={min} max={max} step={step} value={value} aria-label={label} onChange={(e) => onChange(Number(e.target.value))} />
      <input
        type="number"
        className="search"
        aria-label={`${label} (number)`}
        value={Number((value * scale).toFixed(digits))}
        step={step * scale}
        onChange={(e) => {
          const v = Number(e.target.value) / scale;
          if (Number.isFinite(v)) onChange(Math.min(max, Math.max(min, v)));
        }}
      />
      {unit && <span className="muted">{unit}</span>}
    </span>
  );
}

function Colour({ value, onChange, label }: { value: string; onChange: (v: string) => void; label: string }) {
  return <input type="color" className="sc-colour" value={value} aria-label={label} onChange={(e) => onChange(e.target.value)} />;
}

export function Inspector({ doc, layer, onChange, onDoc, onReplacePicture, onCentre, onFit }: {
  doc: Doc;
  layer: Layer | null;
  /** `key`: changes with the same key close together are one undo step */
  onChange: (v: Partial<Layer>, key?: string) => void;
  onDoc: (v: Partial<Doc>, key?: string) => void;
  onReplacePicture: () => void;
  onCentre: () => void;
  onFit: () => void;
}) {
  if (!layer)
    return (
      <section className="panel sc-inspector" aria-label="Canvas">
        <h3>Canvas</h3>
        <Field name="Shape" info="The game takes a spray up to 12288 pixels: 96×96 square, 128×96 wide or 96×128 tall at the largest size.">
          <Segmented label="Shape" options={(Object.keys(SHAPES) as Shape[]).map((s) => [s, SHAPES[s].label] as const)} current={doc.shape} onPick={(v) => onDoc({ shape: v as Shape })} />
        </Field>
        <Field name="Background" info="See-through shows the wall around your design. A colour fills the whole spray.">
          <span className="sc-inline">
            <Segmented label="Background" values=",colour" labels="See-through|Colour" current={doc.background ? "colour" : ""} onPick={(v) => onDoc({ background: v ? "#202020" : "" })} />
            {doc.background && <Colour label="Background colour" value={doc.background} onChange={(c) => onDoc({ background: c }, "bg")} />}
          </span>
        </Field>
        <p className="muted sc-hint">Click a layer on the canvas or in the list to change it.</p>
      </section>
    );

  const set = (v: Partial<Layer>, key?: string) => onChange(v, key);
  return (
    <section className="panel sc-inspector" aria-label={`${KIND_WORD[layer.kind]} settings`}>
      <div className="sc-insp-head">
        <h3>{KIND_WORD[layer.kind]}</h3>
        {layer.locked && <span className="tag">Locked</span>}
      </div>
      {layer.kind === "text" ? (
        <TextFields l={layer} set={set} />
      ) : (
        <Field name="Name">
          <input className="search" value={layer.name} maxLength={40} aria-label="Layer name" onChange={(e) => set({ name: e.target.value }, "name")} />
        </Field>
      )}
      {layer.kind === "shape" && <ShapeFields l={layer} set={set} />}
      {layer.kind === "image" && <ImageFields l={layer} set={set} onReplace={onReplacePicture} />}

      <h4>Place</h4>
      <Field name="Size">
        <Slider label="Size" value={layer.scale} min={0.05} max={4} step={0.01} scale={100} unit="%" onChange={(v) => set({ scale: v }, "scale")} />
      </Field>
      <Field name="Turn">
        <Slider label="Turn" value={layer.rot} min={-180} max={180} step={1} unit="°" onChange={(v) => set({ rot: v }, "rot")} />
      </Field>
      <Field name="See-through">
        <Slider label="Opacity" value={layer.opacity} min={0} max={1} step={0.01} scale={100} unit="%" onChange={(v) => set({ opacity: v }, "opacity")} />
      </Field>
      <Field name="Flip">
        <span className="sc-inline">
          <button type="button" className="btn small" aria-pressed={layer.flipX} onClick={() => set({ flipX: !layer.flipX })}>
            Left–right
          </button>
          <button type="button" className="btn small" aria-pressed={layer.flipY} onClick={() => set({ flipY: !layer.flipY })}>
            Upside down
          </button>
        </span>
      </Field>
      <Field name="Line up">
        <span className="sc-inline">
          <button type="button" className="btn small" onClick={onCentre}>
            Centre
          </button>
          <button type="button" className="btn small" onClick={onFit}>
            Fill the canvas
          </button>
        </span>
      </Field>
    </section>
  );
}

function TextFields({ l, set }: { l: TextLayer; set: (v: Partial<TextLayer>, key?: string) => void }) {
  return (
    <>
      <Field name="Words">
        <textarea className="search sc-text" rows={2} maxLength={80} value={l.text} aria-label="Words" onChange={(e) => set({ text: e.target.value, name: e.target.value.split("\n")[0].slice(0, 40) || "Words" }, "text")} />
      </Field>
      <Field name="Font">
        <select className="search" value={l.font} aria-label="Font" onChange={(e) => set({ font: e.target.value })}>
          {FONTS.map(([v, label]) => (
            <option key={v} value={v}>
              {label}
            </option>
          ))}
        </select>
      </Field>
      <Field name="Style">
        <span className="sc-inline">
          <button type="button" className="btn small" aria-pressed={l.bold} onClick={() => set({ bold: !l.bold })} style={{ fontWeight: 800 }}>
            Bold
          </button>
          <button type="button" className="btn small" aria-pressed={l.italic} onClick={() => set({ italic: !l.italic })} style={{ fontStyle: "italic" }}>
            Italic
          </button>
          <Colour label="Words colour" value={l.colour} onChange={(c) => set({ colour: c }, "colour")} />
        </span>
      </Field>
      <Field name="Outline">
        <span className="sc-inline">
          <Slider label="Outline" value={l.outline} min={0} max={24} step={1} onChange={(v) => set({ outline: v }, "outline")} />
          <Colour label="Outline colour" value={l.outlineColour} onChange={(c) => set({ outlineColour: c }, "ocolour")} />
        </span>
      </Field>
      <Field name="Shadow">
        <span className="sc-inline">
          <Switch on={l.shadow} onChange={(v) => set({ shadow: v })} label="Shadow" />
          {l.shadow && <Colour label="Shadow colour" value={l.shadowColour} onChange={(c) => set({ shadowColour: c }, "scolour")} />}
        </span>
      </Field>
    </>
  );
}

function ShapeFields({ l, set }: { l: ShapeLayer; set: (v: Partial<ShapeLayer>, key?: string) => void }) {
  return (
    <>
      <Field name="Sticker">
        <div className="sc-stickers small" role="group" aria-label="Sticker">
          {STICKERS.map((s) => (
            <button key={s.id} type="button" className="sc-sticker" aria-pressed={l.shape === s.id} title={s.label} aria-label={s.label} onClick={() => set({ shape: s.id, name: s.label, h: l.w * (s.aspect ?? 1) })}>
              <StickerIcon s={s} size={20} />
            </button>
          ))}
        </div>
      </Field>
      <Field name="Colour">
        <Colour label="Sticker colour" value={l.colour} onChange={(c) => set({ colour: c }, "colour")} />
      </Field>
      <Field name="Outline">
        <span className="sc-inline">
          <Slider label="Outline" value={l.outline} min={0} max={24} step={1} onChange={(v) => set({ outline: v }, "outline")} />
          <Colour label="Outline colour" value={l.outlineColour} onChange={(c) => set({ outlineColour: c }, "ocolour")} />
        </span>
      </Field>
      <Field name="Stretch" info="Width and height of the sticker before its size: make a circle an oval, a banner longer.">
        <span className="sc-stack">
          <Slider label="Width" value={l.w} min={10} max={1024} step={1} onChange={(v) => set({ w: v }, "w")} />
          <Slider label="Height" value={l.h} min={10} max={1024} step={1} onChange={(v) => set({ h: v }, "h")} />
        </span>
      </Field>
    </>
  );
}

function ImageFields({ l, set, onReplace }: { l: ImageLayer; set: (v: Partial<ImageLayer>, key?: string) => void; onReplace: () => void }) {
  return (
    <>
      <Field name="Picture">
        <button type="button" className="btn small" onClick={onReplace}>
          Replace…
        </button>
      </Field>
      <Field name="Clear the background" info="Makes the colour in the picture's top-left corner, and colours close to it, see-through. For a logo on a plain background.">
        <Switch on={l.keyOut} onChange={(v) => set({ keyOut: v })} label="Clear the background" />
      </Field>
      <Field name="Colour it">
        <span className="sc-inline">
          <Slider label="Colour it" value={l.tintAmount} min={0} max={1} step={0.01} scale={100} unit="%" onChange={(v) => set({ tintAmount: v }, "tint")} />
          <Colour label="Colour" value={l.tint} onChange={(c) => set({ tint: c }, "tintc")} />
        </span>
      </Field>
    </>
  );
}
