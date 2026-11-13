// The editor's side panels: the toolbar and tool options, the inspector (entity keys with
// real controls, brush actions, texture alignment, map settings), the outline of the map,
// the texture browser and the live checks.

import { useEffect, useMemo, useState, type ReactNode } from "react";
import { BusyButton, Info } from "../../../ui";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import { getGameWads, getTextures, texUrl, type DocEntity, type Issue, type WadTextures } from "./editorApi";
import { selectionIds } from "./EditorScene";
import { EMPTY_SEL, keyOf, originOf, selectionBounds, useEditor, type ToolId } from "./store";

// ---- toolbar --------------------------------------------------------------------------

export const TOOLS: { id: ToolId; label: string; key: string; tip: string }[] = [
  { id: "select", label: "Select", key: "Q", tip: "Click to pick. Drag a face of something picked to push or pull it." },
  { id: "move", label: "Move", key: "W", tip: "Drag the arrows to move what's picked." },
  { id: "rotate", label: "Rotate", key: "E", tip: "Drag the rings to turn what's picked (15° steps)." },
  { id: "scale", label: "Scale", key: "R", tip: "Drag the handles to stretch what's picked." },
  { id: "box", label: "Box", key: "B", tip: "Drag on the floor (or on top of a brush) to draw a box." },
  { id: "cylinder", label: "Cylinder", key: "C", tip: "Drag to draw a cylinder standing up." },
  { id: "clip", label: "Clip", key: "X", tip: "Click two points to place a cut through what's picked. Tab: which side stays. Enter: cut." },
  { id: "entity", label: "Entity", key: "N", tip: "Click a surface to place an entity there." },
  { id: "texture", label: "Texture", key: "T", tip: "Click a face to paint the chosen texture (Shift: the whole brush, Alt: pick up the face's texture)." },
];

const ICONS: Record<ToolId, ReactNode> = {
  select: <path d="M5 3l12 8-5 1.5 3 6-2 1-3-6L6 17z" />,
  move: <path d="M12 2l3 3h-2v6h6V9l3 3-3 3v-2h-6v6h2l-3 3-3-3h2v-6H5v2l-3-3 3-3v2h6V5H9z" />,
  rotate: <path d="M12 4a8 8 0 1 1-7.4 5h2.2A6 6 0 1 0 12 6v3L7.5 5 12 1z" />,
  scale: <path d="M3 3h8v2H6.4l5 5-1.4 1.4-5-5V11H3zm18 18h-8v-2h4.6l-5-5 1.4-1.4 5 5V13h2z" />,
  box: <path d="M12 2l9 5v10l-9 5-9-5V7zm0 2.3L5.4 8 12 11.7 18.6 8zM5 9.7v6.1l6 3.4v-6.1zm14 0l-6 3.4v6.1l6-3.4z" />,
  cylinder: <path d="M12 2c4.4 0 8 1.3 8 3v14c0 1.7-3.6 3-8 3s-8-1.3-8-3V5c0-1.7 3.6-3 8-3zm0 2C8.1 4 6 5 6 5s2.1 1 6 1 6-1 6-1-2.1-1-6-1zM6 7.3V19c.4.4 2.6 1 6 1s5.6-.6 6-1V7.3C16.5 7.8 14.4 8 12 8s-4.5-.2-6-.7z" />,
  clip: <path d="M9.6 7.6A3 3 0 1 0 8 9.4L10.6 12 8 14.6a3 3 0 1 0 1.6 1.8L12 14l7 7h3L9.6 7.6zM6 8a1 1 0 1 1 0-2 1 1 0 0 1 0 2zm0 10a1 1 0 1 1 0-2 1 1 0 0 1 0 2zm8.4-7.6L19 3h3l-6.2 8.8z" />,
  entity: <path d="M12 2a6 6 0 0 1 6 6c0 4.5-6 12-6 12S6 12.5 6 8a6 6 0 0 1 6-6zm0 3.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5z" />,
  texture: <path d="M3 3h8v8H3zm10 0h8v8h-8zM3 13h8v8H3zm10 0h8v8h-8zM5 5v4h4V5zm10 10v4h4v-4z" />,
};

export function Toolbar() {
  const tool = useEditor((s) => s.tool);
  return (
    <nav className="editor-tools" aria-label="Editing tools">
      {TOOLS.map((t) => (
        <button key={t.id} type="button" className="editor-tool" aria-pressed={tool === t.id} title={`${t.label} (${t.key}): ${t.tip}`} onClick={() => useEditor.getState().set({ tool: t.id })}>
          <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true" fill="currentColor">
            {ICONS[t.id]}
          </svg>
          <span>{t.label}</span>
          <kbd>{t.key}</kbd>
        </button>
      ))}
    </nav>
  );
}

const GRIDS = [1, 2, 4, 8, 16, 32, 64, 128, 256];

export const ENTITY_CLASSES = [
  ["info_player_start", "CT spawn"],
  ["info_player_deathmatch", "T spawn"],
  ["light", "Light"],
  ["light_spot", "Spot light"],
  ["light_environment", "Sun"],
  ["hostage_entity", "Hostage"],
  ["info_vip_start", "VIP start"],
  ["armoury_entity", "Weapon on the floor"],
  ["ambient_generic", "Sound"],
  ["env_sprite", "Sprite"],
  ["info_target", "Target point"],
  ["info_map_parameters", "Map rules"],
  ["func_buyzone", "(brush) Buy zone"],
] as const;

export const BRUSH_CLASSES = [
  ["func_wall", "Wall (solid entity)"],
  ["func_illusionary", "Not solid"],
  ["func_breakable", "Breakable"],
  ["func_door", "Door"],
  ["func_door_rotating", "Rotating door"],
  ["func_button", "Button"],
  ["func_water", "Water"],
  ["func_ladder", "Ladder"],
  ["func_buyzone", "Buy zone"],
  ["func_bomb_target", "Bomb site"],
  ["func_hostage_rescue", "Hostage rescue"],
  ["trigger_multiple", "Trigger"],
  ["trigger_hurt", "Hurt zone"],
  ["trigger_push", "Push zone"],
  ["trigger_teleport", "Teleport"],
] as const;

/** What the current tool can be set to, above the 3D view. */
export function ToolOptions() {
  const s = useEditor();
  const clip = s.clipInfo;
  return (
    <div className="editor-options">
      <label>
        Grid
        <select value={s.grid} onChange={(e) => s.set({ grid: Number(e.target.value) })}>
          {GRIDS.map((g) => (
            <option key={g} value={g}>
              {g}
            </option>
          ))}
        </select>
      </label>
      <span className={`editor-snap${s.alt ? " off" : ""}`} title="Hold Alt to place freely">
        {s.alt ? "Snap off (Alt)" : "Snap on"}
      </span>
      <label className="editor-check" title="Textures stay put on the surface when you move, turn or stretch a brush">
        <input type="checkbox" checked={s.textureLock} onChange={(e) => s.set({ textureLock: e.target.checked })} /> Texture lock
      </label>
      {(s.tool === "box" || s.tool === "cylinder") && (
        <label>
          Height
          <input type="number" min={1} max={4096} step={s.grid} value={s.boxHeight} onChange={(e) => s.set({ boxHeight: Math.max(1, Math.min(4096, Number(e.target.value) || 1)) })} />
        </label>
      )}
      {s.tool === "cylinder" && (
        <label>
          Sides
          <input type="number" min={3} max={64} value={s.sides} onChange={(e) => s.set({ sides: Math.max(3, Math.min(64, Math.round(Number(e.target.value) || 3))) })} />
        </label>
      )}
      {(s.tool === "box" || s.tool === "cylinder" || s.tool === "texture" || s.tool === "clip") && (
        <span className="editor-current-tex" title="The texture new brushes and painted faces get (pick one in Textures)">
          <img src={texUrl(s.texture)} alt="" onError={(e) => (e.currentTarget.style.visibility = "hidden")} onLoad={(e) => (e.currentTarget.style.visibility = "visible")} />
          {s.texture}
        </span>
      )}
      {s.tool === "entity" && (
        <label>
          Place
          <select value={s.entityClass} onChange={(e) => s.set({ entityClass: e.target.value })}>
            {ENTITY_CLASSES.filter(([c]) => !c.startsWith("func_")).map(([c, l]) => (
              <option key={c} value={c}>
                {l} ({c})
              </option>
            ))}
          </select>
        </label>
      )}
      {s.tool === "clip" && (
        <>
          <span className="muted">{clip?.points ? (clip.points >= 2 ? "Cut placed." : "Click the second point.") : "Click the first point."}</span>
          <div className="seg" role="group" aria-label="Which side stays">
            {(["both", "back", "front"] as const).map((k) => (
              <button key={k} type="button" aria-pressed={clip?.keep === k} onClick={() => s.setClipKeep?.(k)}>
                {k === "both" ? "Keep both" : k === "back" ? "Keep side A" : "Keep side B"}
              </button>
            ))}
          </div>
          <button type="button" className="btn small primary" disabled={(clip?.points ?? 0) < 2} onClick={() => s.doClip?.()}>
            Cut
          </button>
        </>
      )}
    </div>
  );
}

// ---- inspector ------------------------------------------------------------------------

function useSelectedEntity(): DocEntity | null {
  const sel = useEditor((s) => s.sel);
  const doc = useEditor((s) => s.doc);
  if (sel.entities.length !== 1 || sel.brushes.length) return null;
  return doc?.entities.find((e) => e.id === sel.entities[0]) ?? null;
}

const fmt = (v: Vec3) => v.map((x) => Math.round(x)).join(" ");

export function Inspector() {
  const sel = useEditor((s) => s.sel);
  const ent = useSelectedEntity();
  if (sel.face && !sel.brushes.length && !sel.entities.length) return <FacePanel />;
  if (ent) return <EntityPanel e={ent} />;
  if (sel.brushes.length || sel.entities.length) return <SelectionPanel />;
  return <MapPanel />;
}

function SelectionPanel() {
  const sel = useEditor((s) => s.sel);
  const thickness = useEditor((s) => s.thickness);
  const grid = useEditor((s) => s.grid);
  const [tie, setTie] = useState("func_wall");
  const bounds = selectionBounds();
  const ids = selectionIds();
  const s = useEditor.getState();
  const size = bounds ? bounds[1].map((x, i) => x - bounds[0][i]) : null;
  const centre = bounds ? (bounds[0].map((x, i) => (x + bounds[1][i]) / 2) as Vec3) : null;
  const n = ids.brushes.length;
  const mirror = (axis: number) => {
    const f: Vec3 = [1, 1, 1];
    f[axis] = -1;
    void s.cmd({ op: "transform", ...ids, scale: f, pivot: centre, lock: s.textureLock });
  };
  return (
    <section className="tools-panel">
      <h3>
        {n ? `${n} brush${n === 1 ? "" : "es"}` : ""}
        {n && sel.entities.length ? " and " : ""}
        {sel.entities.length ? `${sel.entities.length} entit${sel.entities.length === 1 ? "y" : "ies"}` : ""}
      </h3>
      {size && <p className="small-note muted">Size {fmt(size as Vec3)} · centre {fmt(centre!)}</p>}
      {n > 0 && (
        <>
          <div className="editor-row">
            <label className="editor-field">
              Wall thickness
              <input type="number" min={1} max={256} value={thickness} onChange={(e) => s.set({ thickness: Math.max(1, Math.min(256, Number(e.target.value) || 8)) })} />
            </label>
            <button type="button" className="btn small" title="Make it a room: walls of this thickness around an empty middle (H)" onClick={() => void s.cmd({ op: "hollow", brushes: ids.brushes, thickness })}>
              Hollow
            </button>
          </div>
          <div className="editor-row wrap">
            <button type="button" className="btn small" disabled={n < 2} title="One brush around all the picked ones" onClick={() => void s.cmd({ op: "merge", brushes: ids.brushes })}>
              Merge
            </button>
            <button
              type="button"
              className="btn small"
              disabled={n !== 1}
              title="Cut this brush's shape out of every brush it overlaps (makes many pieces)"
              onClick={() => void s.cmd({ op: "carve", cutter: ids.brushes[0], targets: [...useEditor.getState().brushes.keys()] })}
            >
              Carve
            </button>
            <button type="button" className="btn small" title={`Put every corner on the ${grid}-unit grid`} onClick={() => void s.cmd({ op: "snap", brushes: ids.brushes, grid })}>
              Snap to grid
            </button>
            <button type="button" className="btn small" title="A copy, one grid step over (Ctrl+D)" onClick={() => duplicate()}>
              Duplicate
            </button>
          </div>
          <div className="editor-row wrap">
            <span className="muted">Mirror</span>
            {["X", "Y", "Z"].map((a, i) => (
              <button key={a} type="button" className="btn small" onClick={() => mirror(i)}>
                {a}
              </button>
            ))}
          </div>
          {!sel.entities.length && (
            <div className="editor-row">
              <select value={tie} onChange={(e) => setTie(e.target.value)} aria-label="Entity type">
                {BRUSH_CLASSES.map(([c, l]) => (
                  <option key={c} value={c}>
                    {l}
                  </option>
                ))}
              </select>
              <button
                type="button"
                className="btn small"
                title="Turn these brushes into a door, trigger, buy zone..."
                onClick={async () => {
                  const r = await s.cmd({ op: "tie", brushes: ids.brushes, classname: tie });
                  if (r.created?.entities.length) s.select({ brushes: [], entities: r.created.entities, face: null });
                }}
              >
                Tie to entity
              </button>
            </div>
          )}
        </>
      )}
      <div className="editor-row wrap">
        <button
          type="button"
          className="btn small"
          onClick={async () => {
            const name = `Group ${(useEditor.getState().doc?.groups.length ?? 0) + 1}`;
            await s.cmd({ op: "group", name, brushes: sel.brushes, entities: sel.entities });
          }}
        >
          Group
        </button>
        {centre && size && (
          <button type="button" className="btn small" onClick={() => useTools.getState().look(centre, Math.max(...size))}>
            Look at
          </button>
        )}
        <button type="button" className="btn small danger" onClick={() => deleteSelection()}>
          Delete
        </button>
      </div>
    </section>
  );
}

export function deleteSelection() {
  const s = useEditor.getState();
  const { sel } = s;
  if (!sel.brushes.length && !sel.entities.length) return;
  void s.cmd({ op: "delete", brushes: sel.brushes, entities: sel.entities }).then((r) => {
    if (!r.error) s.select(EMPTY_SEL);
  });
}

export async function duplicate() {
  const s = useEditor.getState();
  const ids = selectionIds();
  if (!ids.brushes.length && !ids.entities.length) return;
  const r = await s.cmd({ op: "duplicate", brushes: ids.brushes, entities: ids.entities, offset: [s.grid, s.grid, 0] });
  if (r.created) {
    const owned = new Set(r.created.entities);
    const brushes = r.created.brushes.filter((b) => !owned.size || !owned.has(useEditor.getState().brushes.get(b)?.entity ?? -1));
    s.select({ brushes, entities: r.created.entities, face: null });
  }
}

// ---- entity inspector ----------------------------------------------------------------

const KEY_HELP: Record<string, string> = {
  targetname: "This entity's name, so others can set it off.",
  target: "The name of what this sets off.",
  killtarget: "The name of what this removes.",
  angle: "Which way it faces (0 = east, 90 = north).",
  _light: "Colour and brightness (red green blue brightness).",
  rendercolor: "Tint colour (red green blue).",
  renderamt: "How see-through (0-255).",
  rendermode: "0 normal, 1 colour, 2 texture, 4 solid, 5 additive.",
  spawnflags: "Options as a number (each tick adds up).",
  message: "Text or sound file.",
  wait: "Seconds before it can be set off again.",
  speed: "How fast it moves.",
  health: "How much damage it takes before breaking (0 = unbreakable).",
  pitch: "How steep the sun shines (negative = from above).",
};

function names(ents: DocEntity[]): string[] {
  return [...new Set(ents.map((e) => keyOf(e.keys, "targetname")).filter((x): x is string => !!x))].sort();
}

function rgbToHex(v: string): string {
  const n = v.trim().split(/\s+/).map(Number);
  if (n.length < 3 || !n.slice(0, 3).every(Number.isFinite)) return "#ffffff";
  return "#" + n.slice(0, 3).map((x) => Math.max(0, Math.min(255, Math.round(x))).toString(16).padStart(2, "0")).join("");
}

function hexToRgb(h: string): string {
  const n = parseInt(h.slice(1), 16);
  return `${(n >> 16) & 255} ${(n >> 8) & 255} ${n & 255}`;
}

/** A compass dial for "angle": drag or click to point it. */
function AngleDial({ value, onChange }: { value: number; onChange: (v: number) => void }) {
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

function KeyField({ e, k, v, all }: { e: DocEntity; k: string; v: string; all: string[] }) {
  const [text, setText] = useState(v);
  useEffect(() => setText(v), [v]);
  const s = useEditor.getState();
  const commit = (value: string) => {
    if (value === v) return;
    void s.cmd({ op: "set_key", entity: e.id, key: k, value });
  };
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
              const nv = `${hexToRgb(ev.target.value)}${rest ? " " + rest : ""}`;
              setText(nv);
            }}
            onBlur={() => commit(text)}
            aria-label={`${k} colour`}
          />
        )}
        {k === "angle" && <AngleDial value={Number(text) || 0} onChange={(deg) => { setText(String(deg)); commit(String(deg)); }} />}
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
        {k !== "classname" && (
          <button type="button" className="btn small icon" title="Remove this key" aria-label={`Remove ${k}`} onClick={() => void s.cmd({ op: "set_key", entity: e.id, key: k, value: "" })}>
            ×
          </button>
        )}
      </div>
    </div>
  );
}

function EntityPanel({ e }: { e: DocEntity }) {
  const doc = useEditor((s) => s.doc);
  const [nk, setNk] = useState("");
  const [nv, setNv] = useState("");
  const all = useMemo(() => names(doc?.entities ?? []), [doc]);
  const s = useEditor.getState();
  const name = keyOf(e.keys, "targetname");
  const firedBy = name ? (doc?.entities ?? []).filter((x) => keyOf(x.keys, "target") === name || keyOf(x.keys, "killtarget") === name) : [];
  const target = keyOf(e.keys, "target");
  const targets = target ? (doc?.entities ?? []).filter((x) => keyOf(x.keys, "targetname") === target) : [];
  const o = originOf(e.keys);
  const b = selectionBounds();
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
      <p className="small-note muted">{e.brushes.length ? `Brush entity with ${e.brushes.length} brush${e.brushes.length === 1 ? "" : "es"}` : o ? `At ${fmt(o)}` : "No origin"}</p>
      <div className="editor-keys">
        {e.keys.map(([k, v]) => (
          <KeyField key={k} e={e} k={k} v={v} all={all} />
        ))}
      </div>
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

// ---- face (texture) panel ------------------------------------------------------------

function FacePanel() {
  const face = useEditor((s) => s.sel.face)!;
  const brushes = useEditor((s) => s.brushes);
  const texture = useEditor((s) => s.texture);
  const grid = useEditor((s) => s.grid);
  const [scale, setScale] = useState<[number, number]>([1, 1]);
  const [fit, setFit] = useState<[number, number]>([1, 1]);
  const f = brushes.get(face.brush)?.faces.find((x) => x.face === face.face);
  const s = useEditor.getState();
  const faces = [[face.brush, face.face]];
  const align = (action: string, extra: Record<string, unknown> = {}) => void s.cmd({ op: "align", faces, action, ...extra });
  return (
    <section className="tools-panel">
      <h3>Face</h3>
      <div className="editor-face-tex">
        {f && <img src={texUrl(f.texture)} alt="" onError={(e) => (e.currentTarget.style.visibility = "hidden")} />}
        <span className="mono">{f?.texture ?? "?"}</span>
      </div>
      <div className="editor-row wrap">
        <button type="button" className="btn small" disabled={!f || f.texture === texture} onClick={() => void s.cmd({ op: "set_texture", faces: [[face.brush, face.face]], texture })}>
          Paint “{texture}”
        </button>
        <button type="button" className="btn small" onClick={() => f && s.set({ texture: f.texture })}>
          Use this texture
        </button>
      </div>
      <h4>Shift</h4>
      <div className="editor-row wrap">
        <button type="button" className="btn small" onClick={() => align("shift", { u: -grid, v: 0 })}>← {grid}</button>
        <button type="button" className="btn small" onClick={() => align("shift", { u: grid, v: 0 })}>{grid} →</button>
        <button type="button" className="btn small" onClick={() => align("shift", { u: 0, v: -grid })}>↑ {grid}</button>
        <button type="button" className="btn small" onClick={() => align("shift", { u: 0, v: grid })}>↓ {grid}</button>
      </div>
      <h4>Turn</h4>
      <div className="editor-row wrap">
        {[-90, -15, 15, 90].map((d) => (
          <button key={d} type="button" className="btn small" onClick={() => align("rotate", { degrees: d })}>
            {d > 0 ? `+${d}°` : `${d}°`}
          </button>
        ))}
      </div>
      <h4>Scale</h4>
      <div className="editor-row">
        <input type="number" step={0.05} value={scale[0]} onChange={(e) => setScale([Number(e.target.value), scale[1]])} aria-label="Scale across" />
        <input type="number" step={0.05} value={scale[1]} onChange={(e) => setScale([scale[0], Number(e.target.value)])} aria-label="Scale down" />
        <button type="button" className="btn small" onClick={() => align("scale", { u: scale[0], v: scale[1] })}>
          Set
        </button>
      </div>
      <h4>
        Fit <Info text="Stretch the texture so it repeats this many times across the face." />
      </h4>
      <div className="editor-row">
        <input type="number" min={0.1} step={1} value={fit[0]} onChange={(e) => setFit([Number(e.target.value), fit[1]])} aria-label="Repeats across" />
        <input type="number" min={0.1} step={1} value={fit[1]} onChange={(e) => setFit([fit[0], Number(e.target.value)])} aria-label="Repeats down" />
        <button type="button" className="btn small" onClick={() => align("fit", { u: fit[0], v: fit[1] })}>
          Fit
        </button>
      </div>
      <div className="editor-row wrap">
        <button type="button" className="btn small" onClick={() => align("center")}>Centre</button>
        <button type="button" className="btn small" title="Line up with the world grid, like a floor" onClick={() => align("world")}>Align to world</button>
        <button type="button" className="btn small" title="Line up with this face's own edges" onClick={() => align("face")}>Align to face</button>
      </div>
    </section>
  );
}

// ---- map settings --------------------------------------------------------------------

function MapPanel() {
  const doc = useEditor((s) => s.doc);
  const [wads, setWads] = useState<{ name: string; game: string }[] | null>(null);
  const [adding, setAdding] = useState("");
  useEffect(() => {
    void getGameWads().then((r) => setWads(r.wads ?? []));
  }, []);
  if (!doc) return null;
  const s = useEditor.getState();
  const sky = keyOf(doc.world.keys, "skyname") ?? "";
  const setKey = (k: string, v: string) => {
    const keys = doc.world.keys.filter(([a]) => a !== k);
    if (v) keys.push([k, v]);
    void s.cmd({ op: "set_world", keys });
  };
  const listed = new Set(doc.world.wads.map((w) => w.split(/[\\/]/).pop()!.toLowerCase()));
  return (
    <section className="tools-panel">
      <h3>Map settings</h3>
      <p className="small-note muted">
        {doc.stats.brushes} brushes, {doc.stats.entities} entities, {doc.stats.textures} textures. Pick something to change it.
      </p>
      <label className="editor-field">
        Sky
        <input defaultValue={sky} key={sky} placeholder="desert" onBlur={(e) => e.target.value.trim() !== sky && setKey("skyname", e.target.value.trim())} />
      </label>
      <h4>
        Texture files (WADs) <Info text="The texture files this map uses. Players need them too unless the compiler packs the textures into the map." />
      </h4>
      <ul className="editor-wads">
        {doc.world.wads.map((w) => (
          <li key={w}>
            <span className="mono">{w}</span>
            <button type="button" className="btn small icon" aria-label={`Remove ${w}`} title="Remove" onClick={() => void s.cmd({ op: "set_world", wads: doc.world.wads.filter((x) => x !== w) })}>
              ×
            </button>
          </li>
        ))}
      </ul>
      {wads && (
        <div className="editor-row">
          <select value={adding} onChange={(e) => setAdding(e.target.value)} aria-label="Add a WAD">
            <option value="">Add a WAD…</option>
            {wads.filter((w) => !listed.has(w.name.toLowerCase())).map((w) => (
              <option key={w.game + w.name} value={w.name}>
                {w.name} ({w.game})
              </option>
            ))}
          </select>
          <button type="button" className="btn small" disabled={!adding} onClick={() => void s.cmd({ op: "set_world", wads: [...doc.world.wads, adding] }).then(() => setAdding(""))}>
            Add
          </button>
        </div>
      )}
    </section>
  );
}

// ---- outline (hierarchy) -------------------------------------------------------------

const PAGE = 300;

export function Outline() {
  const doc = useEditor((s) => s.doc);
  const sel = useEditor((s) => s.sel);
  const [q, setQ] = useState("");
  const [open, setOpen] = useState<Record<string, boolean>>({ world: false, entities: true, groups: true });
  const [more, setMore] = useState(PAGE);
  if (!doc) return null;
  const s = useEditor.getState();
  const query = q.trim().toLowerCase();
  const world = doc.brushes.filter((b) => b.entity == null);
  const ents = doc.entities.filter((e) => !query || e.classname.includes(query) || (keyOf(e.keys, "targetname") ?? "").toLowerCase().includes(query));
  const pickB = (id: number, add: boolean) => s.select(add ? { ...sel, brushes: sel.brushes.includes(id) ? sel.brushes.filter((x) => x !== id) : [...sel.brushes, id], face: null } : { brushes: [id], entities: [], face: null });
  const pickE = (id: number, add: boolean) => s.select(add ? { ...sel, entities: sel.entities.includes(id) ? sel.entities.filter((x) => x !== id) : [...sel.entities, id], face: null } : { brushes: [], entities: [id], face: null });
  const toggle = (k: string) => setOpen((o) => ({ ...o, [k]: !o[k] }));
  const lookAt = (min: Vec3, max: Vec3) => useTools.getState().look(min.map((x, i) => (x + max[i]) / 2) as Vec3, Math.max(64, ...max.map((x, i) => x - min[i])));
  return (
    <section className="tools-panel editor-outline">
      <input type="search" placeholder="Find an entity or name" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Find" />
      {doc.groups.length > 0 && (
        <>
          <button type="button" className="editor-tree-head" aria-expanded={open.groups} onClick={() => toggle("groups")}>
            Groups ({doc.groups.length})
          </button>
          {open.groups &&
            doc.groups.map((g) => {
              const bs = doc.brushes.filter((b) => b.group === g.id).map((b) => b.id);
              const es = doc.entities.filter((e) => e.group === g.id).map((e) => e.id);
              return (
                <div key={g.id} className="editor-tree-row">
                  <button type="button" className="linklike" onClick={() => s.select({ brushes: bs, entities: es, face: null })}>
                    {g.name} <span className="muted">({bs.length + es.length})</span>
                  </button>
                  <button type="button" className="btn small" aria-pressed={g.hidden} title={g.hidden ? "Show" : "Hide"} onClick={() => void s.cmd({ op: "group_flags", group: g.id, hidden: !g.hidden })}>
                    {g.hidden ? "Show" : "Hide"}
                  </button>
                  <button type="button" className="btn small" title="Ungroup" onClick={() => void s.cmd({ op: "ungroup", group: g.id })}>
                    Ungroup
                  </button>
                </div>
              );
            })}
        </>
      )}
      <button type="button" className="editor-tree-head" aria-expanded={open.entities} onClick={() => toggle("entities")}>
        Entities ({ents.length})
      </button>
      {open.entities &&
        ents.slice(0, more).map((e) => {
          const name = keyOf(e.keys, "targetname");
          return (
            <button
              key={e.id}
              type="button"
              className="editor-tree-item"
              aria-pressed={sel.entities.includes(e.id)}
              onClick={(ev) => pickE(e.id, ev.shiftKey || ev.ctrlKey || ev.metaKey)}
              onDoubleClick={() => {
                const o = originOf(e.keys);
                if (o) useTools.getState().look(o, 64);
                else {
                  const b = selectionBounds();
                  if (b) lookAt(b[0], b[1]);
                }
              }}
            >
              {e.classname}
              {name && <span className="muted"> “{name}”</span>}
              {e.brushes.length > 0 && <span className="muted"> · {e.brushes.length} brush{e.brushes.length === 1 ? "" : "es"}</span>}
            </button>
          );
        })}
      {open.entities && ents.length > more && (
        <button type="button" className="btn small" onClick={() => setMore(more + PAGE)}>
          Show more
        </button>
      )}
      <button type="button" className="editor-tree-head" aria-expanded={open.world} onClick={() => toggle("world")}>
        World brushes ({world.length})
      </button>
      {open.world &&
        world.slice(0, more).map((b) => (
          <button key={b.id} type="button" className="editor-tree-item" aria-pressed={sel.brushes.includes(b.id)} onClick={(ev) => pickB(b.id, ev.shiftKey || ev.ctrlKey || ev.metaKey)} onDoubleClick={() => lookAt(b.min, b.max)}>
            Brush {b.id} <span className="muted">{fmt(b.max.map((x, i) => x - b.min[i]) as Vec3)}</span>
          </button>
        ))}
      {open.world && world.length > more && (
        <button type="button" className="btn small" onClick={() => setMore(more + PAGE)}>
          Show more
        </button>
      )}
      <p className="small-note muted">Shift-click to pick several. Double-click to look at it.</p>
    </section>
  );
}

// ---- texture browser -----------------------------------------------------------------

export function TextureBrowser() {
  const docWads = useEditor((s) => s.doc?.world.wads.join(";"));
  const current = useEditor((s) => s.texture);
  const [list, setList] = useState<WadTextures[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [more, setMore] = useState(200);
  useEffect(() => {
    let live = true;
    setList(null);
    void getTextures().then((r) => {
      if (!live) return;
      if (r.error) setErr(r.error);
      else if (r.noGame) setErr("Set up the game on this computer to see its textures.");
      else {
        setErr(null);
        setList(r.wads);
      }
    });
    return () => {
      live = false;
    };
  }, [docWads]);
  const s = useEditor.getState();
  const query = q.trim().toLowerCase();
  const all = useMemo(() => (list ?? []).flatMap((w) => w.textures.map((t) => ({ ...t, wad: w.name }))).filter((t) => !query || t.name.includes(query)), [list, query]);
  const paint = (name: string) => {
    s.set({ texture: name });
    const ids = selectionIds().brushes;
    const face = s.sel.face;
    if (face) void s.cmd({ op: "set_texture", faces: [[face.brush, face.face]], texture: name });
    else if (ids.length) void s.cmd({ op: "set_texture", faces: ids.map((b) => [b, null]), texture: name });
  };
  return (
    <section className="tools-panel editor-textures">
      <input type="search" placeholder="Find a texture" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Find a texture" />
      <p className="small-note muted">Click to choose. Double-click to paint what's picked.</p>
      {err && <p className="small-note">{err}</p>}
      {!list && !err && (
        <p className="small-note muted">
          <span className="spinner" aria-hidden="true" />
          Reading the texture files…
        </p>
      )}
      {list?.filter((w) => !w.found).map((w) => (
        <p key={w.name} className="small-note muted">
          {w.name}: {w.error ?? "not found"}
        </p>
      ))}
      <div className="editor-tex-grid">
        {all.slice(0, more).map((t) => (
          <button key={t.wad + t.name} type="button" className="editor-tex" aria-pressed={t.name.toLowerCase() === current.toLowerCase()} title={`${t.name} (${t.width}×${t.height}, ${t.wad})`} onClick={() => s.set({ texture: t.name })} onDoubleClick={() => paint(t.name)}>
            <img src={texUrl(t.name)} alt="" loading="lazy" />
            <span>{t.name}</span>
          </button>
        ))}
      </div>
      {all.length > more && (
        <button type="button" className="btn small" onClick={() => setMore(more + 200)}>
          Show more ({all.length - more} left)
        </button>
      )}
    </section>
  );
}

// ---- checks ---------------------------------------------------------------------------

const LEVEL_LABEL: Record<Issue["level"], string> = { error: "Problem", warning: "Warning", info: "Note" };

export function ChecksPanel() {
  const checks = useEditor((s) => s.checks);
  const s = useEditor.getState();
  if (!checks) {
    return (
      <section className="tools-panel">
        <p className="small-note muted">
          <span className="spinner" aria-hidden="true" />
          Checking the map…
        </p>
      </section>
    );
  }
  const go = (i: Issue) => {
    if (i.brush != null) {
      const b = s.brushes.get(i.brush);
      s.select({ brushes: [i.brush], entities: [], face: null });
      if (b) useTools.getState().look(b.min.map((x, k) => (x + b.max[k]) / 2) as Vec3, Math.max(64, ...b.max.map((x, k) => x - b.min[k])));
    } else if (i.entity != null) {
      s.select({ brushes: [], entities: [i.entity], face: null });
      const e = s.doc?.entities.find((x) => x.id === i.entity);
      const o = e && originOf(e.keys);
      if (o) useTools.getState().look(o, 64);
    }
  };
  return (
    <section className="tools-panel editor-checks">
      {checks.issues.length === 0 ? (
        <p className="small-note">No problems found. (Leaks are found when the map is compiled.)</p>
      ) : (
        <ul>
          {checks.issues.map((i, n) => (
            <li key={n} className={`editor-issue ${i.level}`}>
              <button type="button" className="linklike" disabled={i.brush == null && i.entity == null} onClick={() => go(i)}>
                <strong>{LEVEL_LABEL[i.level]}:</strong> {i.message}
              </button>
            </li>
          ))}
        </ul>
      )}
      {!checks.texturesChecked && <p className="small-note muted">Textures aren't checked until the game is set up on this computer.</p>}
    </section>
  );
}

export function BusyLabel({ busy, label }: { busy: boolean; label: string }) {
  return busy ? (
    <span className="editor-busy">
      <span className="spinner" aria-hidden="true" />
      {label}
    </span>
  ) : null;
}

export { BusyButton };
