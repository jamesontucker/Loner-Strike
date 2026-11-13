// The entity inspector: a form from the entity definitions (our Counter-Strike FGD) with
// typed fields, flag checkboxes, choice lists, help, colour pickers, an angle dial and
// target name suggestions; keys the definition doesn't know, and unknown classes, fall back
// to plain key/value rows.

import { AssetPreview } from "./AssetPreview";
import { useEffect, useMemo, useState } from "react";
import { Info } from "../../../ui";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import type { DocEntity, FgdClass, FgdKey } from "./editorApi";
import { useFgd } from "./fgd";
import { AssetButtons } from "./AssetPicker"; // P5
import { kindOfKey, type AssetKind } from "./contentApi"; // P5

/** P5: the kind of file a key names: by its FGD type or its value's file type, else by the key's
 *  name (model, sprite, noise…; never a brush model like *12). */
function keyAssetKind(key: string, value: string, fgdType?: string): AssetKind | null {
  if (value.trim().startsWith("*")) return null;
  const k = kindOfKey(value, fgdType);
  if (k) return k;
  if (/^noise\d*$/.test(key)) return "sound";
  if (!value.trim() && (key === "model" || key === "sprite")) return key === "sprite" ? "sprite" : null;
  return null;
}
import { creatureDef, loadCreatures, loadKits, markerDef, useKits } from "./kits"; // P5: kit markers, creature types
import { deleteSelection, duplicate } from "./Panels";
import { keyOf, originOf, selectionBounds, useEditor } from "./store";

const KEY_HELP: Record<string, string> = {
  targetname: "This entity's name, so others can set it off.",
  target: "The name of what this sets off.",
  killtarget: "The name of what this removes.",
  angle: "Which way it faces (0 = east, 90 = north).",
  _light: "Color and brightness (red green blue brightness).",
  rendercolor: "Tint color (red green blue).",
  renderamt: "How see-through (0-255).",
  rendermode: "0 normal, 1 color, 2 texture, 4 solid, 5 additive.",
  spawnflags: "Options as a number (each tick adds up).",
  message: "Text or sound file.",
  wait: "Seconds before it can be set off again.",
  speed: "How fast it moves.",
  health: "How much damage it takes before breaking (0 = unbreakable).",
  pitch: "How steep the sun shines (negative = from above).",
};

const fmt = (v: Vec3) => v.map((x) => Math.round(x)).join(" ");

function names(ents: DocEntity[]): string[] {
  return [...new Set(ents.map((e) => keyOf(e.keys, "targetname")).filter((x): x is string => !!x))].sort();
}

export function rgbToHex(v: string): string {
  const n = v.trim().split(/\s+/).map(Number);
  if (n.length < 3 || !n.slice(0, 3).every(Number.isFinite)) return "#ffffff";
  return "#" + n.slice(0, 3).map((x) => Math.max(0, Math.min(255, Math.round(x))).toString(16).padStart(2, "0")).join("");
}

export function hexToRgb(h: string): string {
  const n = parseInt(h.slice(1), 16);
  return `${(n >> 16) & 255} ${(n >> 8) & 255} ${n & 255}`;
}

/** A compass dial for "angle": drag or click to point it. */
export function AngleDial({ value, onChange }: { value: number; onChange: (v: number) => void }) {
  const r = 18;
  const a = (value * Math.PI) / 180;
  const set = (e: React.PointerEvent<SVGSVGElement>) => {
    const b = e.currentTarget.getBoundingClientRect();
    const x = e.clientX - (b.left + b.width / 2);
    const y = -(e.clientY - (b.top + b.height / 2));
    let deg = Math.round((Math.atan2(y, x) * 180) / Math.PI);
    if (!e.altKey) deg = Math.round(deg / 15) * 15;
    onChange((deg + 360) % 360);
  };
  return (
    <svg className="editor-dial" viewBox="-22 -22 44 44" width="44" height="44" onPointerDown={set} role="slider" aria-valuenow={value} aria-label="Facing">
      <circle r={r} />
      <line x1={0} y1={0} x2={Math.cos(a) * r} y2={-Math.sin(a) * r} />
      <text x={r - 2} y={3} fontSize="6">E</text>
      <text x={-2} y={-r + 7} fontSize="6">N</text>
    </svg>
  );
}

const setKey = (e: DocEntity, k: string, value: string) => void useEditor.getState().cmd({ op: "set_key", entity: e.id, key: k, value });

/** A plain key/value row (keys the definition doesn't know). */
function KeyField({ e, k, v, all }: { e: DocEntity; k: string; v: string; all: string[] }) {
  const [text, setText] = useState(v);
  useEffect(() => setText(v), [v]);
  const commit = (value: string) => value !== v && setKey(e, k, value);
  const list = k === "target" || k === "killtarget" || k === "master" ? `editor-names-${e.id}` : undefined;
  return (
    <div className="editor-key">
      <span className="editor-key-name" title={KEY_HELP[k] ?? k}>
        {k}
      </span>
      <div className="editor-key-value">
        {(k === "_light" || k === "rendercolor") && (
          <input
            type="color"
            value={rgbToHex(text)}
            onChange={(ev) => {
              const rest = text.trim().split(/\s+/).slice(3).join(" ");
              setText(`${hexToRgb(ev.target.value)}${rest ? " " + rest : ""}`);
            }}
            onBlur={() => commit(text)}
            aria-label={`${k} color`}
          />
        )}
        {k === "angle" && (
          <AngleDial
            value={Number(text) || 0}
            onChange={(deg) => {
              setText(String(deg));
              commit(String(deg));
            }}
          />
        )}
        <input
          value={text}
          list={list}
          onChange={(ev) => setText(ev.target.value)}
          onBlur={() => commit(text)}
          onKeyDown={(ev) => {
            if (ev.key === "Enter") commit(text);
            if (ev.key === "Escape") setText(v);
          }}
          aria-label={k}
          disabled={k === "classname"}
        />
        {list && (
          <datalist id={list}>
            {all.map((n) => (
              <option key={n} value={n} />
            ))}
          </datalist>
        )}
        <AssetPreview value={text} />
        {/* P5: Choose… / Show in Content Browser / Use selected */}
        {keyAssetKind(k, text) && <AssetButtons kind={keyAssetKind(k, text)!} value={text} label={k} set={(nv) => (setText(nv), commit(nv))} />}
        {k !== "classname" && (
          <button type="button" className="btn small icon" title="Remove this key" aria-label={`Remove ${k}`} onClick={() => setKey(e, k, "")}>
            ×
          </button>
        )}
      </div>
    </div>
  );
}

/** One field from the definition, with the right control for its type. */
function DefField({ e, def, value, all }: { e: DocEntity; def: FgdKey; value: string | undefined; all: string[] }) {
  const v = value ?? "";
  const [text, setText] = useState(v);
  useEffect(() => setText(v), [v]);
  const commit = (nv: string) => nv !== v && setKey(e, def.name, nv);
  const id = `editor-f-${e.id}-${def.name}`;
  const label = (
    <label className="editor-def-label" htmlFor={id} title={`${def.name}${def.help ? `: ${def.help}` : ""}`}>
      {def.label}
      {def.help && <Info text={def.help} />}
    </label>
  );
  if (def.type === "flags" && def.flags?.length) {
    const n = Number(v || def.default || 0) | 0;
    return (
      <fieldset className="editor-flags">
        <legend title="spawnflags">{def.label === "spawnflags" ? "Options" : def.label}</legend>
        {def.flags.map((f) => (
          <label key={f.bit} className="editor-check">
            <input
              type="checkbox"
              checked={(n & f.bit) !== 0}
              onChange={(ev) => {
                const next = ev.target.checked ? n | f.bit : n & ~f.bit;
                setKey(e, def.name, next ? String(next) : "");
              }}
            />{" "}
            {f.label}
          </label>
        ))}
      </fieldset>
    );
  }
  const reset = value != null && (
    <button type="button" className="btn small icon" title={def.default ? `Back to the default (${def.default})` : "Remove this key"} aria-label={`Reset ${def.label}`} onClick={() => setKey(e, def.name, "")}>
      ×
    </button>
  );
  let control;
  if (def.type === "choices" && def.choices?.length) {
    const known = def.choices.some((c) => c.value === v);
    control = (
      <select id={id} value={v} onChange={(ev) => commit(ev.target.value)}>
        <option value="">{`Default${def.default != null && def.default !== "" ? ` (${def.choices.find((c) => c.value === def.default)?.label ?? def.default})` : ""}`}</option>
        {def.choices.map((c) => (
          <option key={c.value} value={c.value}>
            {c.label}
          </option>
        ))}
        {!known && v && <option value={v}>{v}</option>}
      </select>
    );
  } else {
    const isNum = def.type === "integer" || def.type === "float";
    const isColour = def.type === "color255";
    const list = def.type === "target_destination" ? `editor-names-${e.id}` : undefined;
    const parts = text.trim().split(/\s+/);
    control = (
      <>
        {isColour && (
          <input
            type="color"
            value={rgbToHex(text || def.default || "255 255 255")}
            onChange={(ev) => {
              const base = (text || def.default || "").trim().split(/\s+/);
              const rest = base.slice(3).join(" ");
              setText(`${hexToRgb(ev.target.value)}${rest ? " " + rest : ""}`);
            }}
            onBlur={() => commit(text)}
            aria-label={`${def.label} color`}
          />
        )}
        {isColour && (def.name === "_light" || def.name === "_diffuse_light") && (
          <input
            type="number"
            className="editor-bright"
            min={0}
            max={10000}
            title="Brightness"
            aria-label="Brightness"
            value={parts.length >= 4 ? parts[3] : ""}
            placeholder={(def.default ?? "").split(/\s+/)[3] ?? "200"}
            onChange={(ev) => {
              const base = (text || def.default || "255 255 255").trim().split(/\s+/).slice(0, 3);
              setText(`${base.join(" ")} ${ev.target.value}`.trim());
            }}
            onBlur={() => commit(text)}
          />
        )}
        {def.type === "angle" && (
          <AngleDial
            value={Number(text || def.default) || 0}
            onChange={(deg) => {
              setText(String(deg));
              commit(String(deg));
            }}
          />
        )}
        <input
          id={id}
          type={isNum ? "number" : undefined}
          step={def.type === "float" ? "any" : undefined}
          value={text}
          placeholder={def.default}
          list={list}
          spellCheck={false}
          onChange={(ev) => setText(ev.target.value)}
          onBlur={() => commit(text.trim())}
          onKeyDown={(ev) => {
            if (ev.key === "Enter") commit(text.trim());
            if (ev.key === "Escape") setText(v);
          }}
        />
        <AssetPreview value={text || def.default || ""} fgdType={def.type} />
        {keyAssetKind(def.name, text, def.type) && <AssetButtons kind={keyAssetKind(def.name, text, def.type)!} value={text} label={def.label} set={(nv) => (setText(nv), commit(nv))} />}
      </>
    );
  }
  return (
    <div className="editor-key editor-def">
      {label}
      <div className="editor-key-value">
        {control}
        {reset}
      </div>
    </div>
  );
}

export function EntityForm({ e }: { e: DocEntity }) {
  const doc = useEditor((s) => s.doc);
  const fgd = useFgd();
  const [nk, setNk] = useState("");
  const [nv, setNv] = useState("");
  const [showRaw, setShowRaw] = useState(false);
  const all = useMemo(() => names(doc?.entities ?? []), [doc]);
  const s = useEditor.getState();
  // P5: an ls_marker's form comes from its mode's kit; ls_creature's Type lists every creature type
  const kits = useKits((k) => k.kits);
  const creatures = useKits((k) => k.creatures);
  useEffect(() => {
    if (e.classname === "ls_marker" && !useKits.getState().loaded) void loadKits();
    if (e.classname === "ls_creature" || e.classname === "ls_wave_spawner") void loadCreatures();
  }, [e.classname]);
  const def: FgdClass | undefined = useMemo(
    () => markerDef(e, fgd?.get(e.classname)) ?? creatureDef(fgd?.get(e.classname), creatures),
    [e, fgd, kits, creatures], // eslint-disable-line react-hooks/exhaustive-deps
  );
  const creatureModel = e.classname === "ls_creature" ? creatures.find((c) => c.name === (keyOf(e.keys, "type") ?? "zombie"))?.model : e.classname === "ls_barnacle" ? "models/barnacle.mdl" : undefined;
  const name = keyOf(e.keys, "targetname");
  const firedBy = name ? (doc?.entities ?? []).filter((x) => keyOf(x.keys, "target") === name || keyOf(x.keys, "killtarget") === name) : [];
  const target = keyOf(e.keys, "target");
  const targets = target ? (doc?.entities ?? []).filter((x) => keyOf(x.keys, "targetname") === target) : [];
  const o = originOf(e.keys);
  const b = selectionBounds();
  const defined = new Set(def?.keys.map((k) => k.name) ?? []);
  const other = e.keys.filter(([k]) => k !== "classname" && k !== "origin" && !defined.has(k));
  const add = () => {
    const k = nk.trim();
    if (!k || !nv.trim()) return;
    void s.cmd({ op: "set_key", entity: e.id, key: k, value: nv.trim() }).then((r) => {
      if (!r.error) {
        setNk("");
        setNv("");
      }
    });
  };
  return (
    <section className="tools-panel">
      <h3>{e.classname}</h3>
      <p className="small-note muted">
        {def?.description ? `${def.description} ` : ""}
        {e.brushes.length ? `${e.brushes.length} brush${e.brushes.length === 1 ? "" : "es"}.` : o ? `At ${fmt(o)}.` : "No origin."}
      </p>
      <datalist id={`editor-names-${e.id}`}>
        {all.map((n) => (
          <option key={n} value={n} />
        ))}
      </datalist>
      {def ? (
        <div className="editor-keys">
          {def.keys.map((k) => (
            <DefField key={k.name} e={e} def={k} value={keyOf(e.keys, k.name)} all={all} />
          ))}
        </div>
      ) : (
        fgd && <p className="small-note muted">This class isn't in the entity definitions, so its keys are shown as they are.</p>
      )}
      {e.classname === "ls_wave_spawner" && creatures.length > 0 && (
        <div className="editor-key editor-def">
          <span className="editor-def-label" title="types: the creature types its waves use">Creature types</span>
          <div className="editor-key-value">
            <span className="seg p5-chips" role="group" aria-label="Creature types">
              {creatures.map((c) => {
                const list = (keyOf(e.keys, "types") ?? "").split(/[\s,]+/).filter(Boolean);
                const on = list.includes(c.name);
                return (
                  <button key={c.name} type="button" aria-pressed={on} title={c.model} onClick={() => setKey(e, "types", (on ? list.filter((x) => x !== c.name) : [...list, c.name]).join(" "))}>
                    {c.display}
                  </button>
                );
              })}
            </span>
          </div>
        </div>
      )}
      {creatureModel && (
        <div className="editor-key editor-def">
          <span className="editor-def-label">Its model</span>
          <div className="editor-key-value">
            <span className="mono small-note">{creatureModel}</span>
            <AssetPreview value={creatureModel.trim().split(/\s+/)[0] ?? ""} fgdType="studio" />
          </div>
        </div>
      )}
      {(other.length > 0 || !def) && (
        <div className="editor-keys">
          {def && <h4>Other keys</h4>}
          {(def ? other : e.keys).map(([k, v]) => (
            <KeyField key={k} e={e} k={k} v={v} all={all} />
          ))}
        </div>
      )}
      {def && (
        <button type="button" className="linklike small-note" onClick={() => setShowRaw(!showRaw)} aria-expanded={showRaw}>
          {showRaw ? "Hide the raw keys" : "Show the raw keys"}
        </button>
      )}
      {def && showRaw && (
        <div className="editor-keys">
          {e.keys.map(([k, v]) => (
            <KeyField key={k} e={e} k={k} v={v} all={all} />
          ))}
        </div>
      )}
      <div className="editor-key add">
        <input placeholder="New key" value={nk} onChange={(ev) => setNk(ev.target.value)} aria-label="New key" list="editor-common-keys" />
        <div className="editor-key-value">
          <input placeholder="Value" value={nv} onChange={(ev) => setNv(ev.target.value)} onKeyDown={(ev) => ev.key === "Enter" && add()} aria-label="New value" />
          <button type="button" className="btn small" onClick={add} disabled={!nk.trim() || !nv.trim()}>
            Add
          </button>
        </div>
        <datalist id="editor-common-keys">
          {Object.keys(KEY_HELP).map((k) => (
            <option key={k} value={k} />
          ))}
        </datalist>
      </div>
      {(targets.length > 0 || firedBy.length > 0) && (
        <div className="editor-links">
          {targets.map((t) => (
            <button key={t.id} type="button" className="linklike" onClick={() => s.select({ brushes: [], entities: [t.id], face: null })}>
              Sets off {t.classname} “{target}”
            </button>
          ))}
          {firedBy.map((t) => (
            <button key={t.id} type="button" className="linklike" onClick={() => s.select({ brushes: [], entities: [t.id], face: null })}>
              Set off by {t.classname}
            </button>
          ))}
        </div>
      )}
      <div className="editor-row wrap">
        {b && (
          <button type="button" className="btn small" onClick={() => useTools.getState().look(b[0].map((x, i) => (x + b[1][i]) / 2) as Vec3, Math.max(64, ...b[1].map((x, i) => x - b[0][i])))}>
            Look at
          </button>
        )}
        {e.brushes.length > 0 && (
          <button type="button" className="btn small" title="Make its brushes plain world brushes again" onClick={() => void s.cmd({ op: "to_world", entity: e.id }).then((r) => !r.error && s.select({ brushes: e.brushes, entities: [], face: null }))}>
            Move to world
          </button>
        )}
        <button type="button" className="btn small" onClick={() => void duplicate()}>
          Duplicate
        </button>
        <button type="button" className="btn small danger" onClick={() => deleteSelection()}>
          Delete
        </button>
      </div>
    </section>
  );
}
