// The editor's side panels: the toolbar and tool options, the inspector (entity keys with
// real controls, brush actions, texture alignment, map settings), the outline of the map,
// the texture browser and the live checks.

import { setVisSets } from "./extras"; // P4
import { MarqueeOptions } from "./Marquee"; // P4
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { BusyButton, Info } from "../../../ui";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import { getGameWads, getTextures, texUrl, type DocEntity, type Issue, type WadTextures } from "./editorApi";
import { selectionIds } from "./EditorScene";
import { EMPTY_SEL, facesOf, keyOf, originOf, selectionBounds, useEditor, type ShapeKind, type ToolId } from "./store";
import { useFgd } from "./fgd";
import { FaceSheet, RecentTextures } from "./FaceSheet";
import { EntityForm } from "./EntityForm";
import { MultiEntityKeys } from "./MultiEntityKeys"; // P5

// ---- toolbar --------------------------------------------------------------------------

export const TOOLS: { id: ToolId; label: string; key: string; tip: string }[] = [
  { id: "select", label: "Select", key: "Q", tip: "Click to pick. Drag a face of something picked to push or pull it." },
  { id: "move", label: "Move", key: "W", tip: "Drag the arrows to move what's picked." },
  { id: "rotate", label: "Rotate", key: "E", tip: "Drag the rings to turn what's picked (15° steps)." },
  { id: "scale", label: "Scale", key: "R", tip: "Drag the handles to stretch what's picked." },
  { id: "vertex", label: "Vertex", key: "V", tip: "Drag a corner (yellow), an edge (blue) or a face (purple) of a picked brush. Shift-click picks several, arrows nudge them. Brushes stay convex." },
  { id: "box", label: "Box", key: "B", tip: "Drag on the floor (or on top of a brush) to draw a box." },
  { id: "shape", label: "Shape", key: "C", tip: "Drag to draw a cylinder, wedge, spike, sphere or arch (pick which above the view)." },
  { id: "clip", label: "Clip", key: "X", tip: "Click two points to place a cut through what's picked. Tab: which side stays. Enter: cut." },
  { id: "entity", label: "Entity", key: "N", tip: "Click a surface to place an entity there." },
  { id: "texture", label: "Texture", key: "T", tip: "Click a face to pick it (Shift adds more), drag it to slide the texture, drag the handles to turn or stretch it. Alt-click picks up a texture." },
  { id: "measure", label: "Measure", key: "M", tip: "Drag between two points to measure the distance." },
];

const ICONS: Record<ToolId, ReactNode> = {
  select: <path d="M5 3l12 8-5 1.5 3 6-2 1-3-6L6 17z" />,
  move: <path d="M12 2l3 3h-2v6h6V9l3 3-3 3v-2h-6v6h2l-3 3-3-3h2v-6H5v2l-3-3 3-3v2h6V5H9z" />,
  rotate: <path d="M12 4a8 8 0 1 1-7.4 5h2.2A6 6 0 1 0 12 6v3L7.5 5 12 1z" />,
  scale: <path d="M3 3h8v2H6.4l5 5-1.4 1.4-5-5V11H3zm18 18h-8v-2h4.6l-5-5 1.4-1.4 5 5V13h2z" />,
  box: <path d="M12 2l9 5v10l-9 5-9-5V7zm0 2.3L5.4 8 12 11.7 18.6 8zM5 9.7v6.1l6 3.4v-6.1zm14 0l-6 3.4v6.1l6-3.4z" />,
  vertex: <path d="M4 4h4v4H4zm12 0h4v4h-4zM4 16h4v4H4zm12 0h4v4h-4zM8 5.5h8v1H8zm0 12h8v1H8zM5.5 8h1v8h-1zm12 0h1v8h-1z" />,
  measure: <path d="M2 16.5 16.5 2 22 7.5 7.5 22zm3 0L7.5 19l1.4-1.4-1.2-1.2 1-1 1.2 1.2 1.4-1.4-2.4-2.4 1-1 2.4 2.4 1.4-1.4-1.2-1.2 1-1 1.2 1.2 1.4-1.4-2.4-2.4 1-1 2.4 2.4 1.4-1.4L16.5 5z" />,
  shape: <path d="M12 2c4.4 0 8 1.3 8 3v14c0 1.7-3.6 3-8 3s-8-1.3-8-3V5c0-1.7 3.6-3 8-3zm0 2C8.1 4 6 5 6 5s2.1 1 6 1 6-1 6-1-2.1-1-6-1zM6 7.3V19c.4.4 2.6 1 6 1s5.6-.6 6-1V7.3C16.5 7.8 14.4 8 12 8s-4.5-.2-6-.7z" />,
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

const SHAPES: { id: ShapeKind; label: string }[] = [
  { id: "cylinder", label: "Cylinder" },
  { id: "wedge", label: "Wedge" },
  { id: "spike", label: "Spike" },
  { id: "sphere", label: "Sphere" },
  { id: "arch", label: "Arch" },
];

const num = (v: string, lo: number, hi: number, fallback: number) => {
  const n = Number(v);
  return Number.isFinite(n) ? Math.max(lo, Math.min(hi, n)) : fallback;
};

/** What the current tool can be set to, above the 3D view. */
export function ToolOptions() {
  const s = useEditor();
  const fgd = useFgd();
  const clip = s.clipInfo;
  const picking = s.tool === "select" || s.tool === "move" || s.tool === "rotate" || s.tool === "scale";
  const points = fgd ? [...fgd.values()].filter((c) => c.kind === "point").map((c) => c.name) : ENTITY_CLASSES.filter(([c]) => !c.startsWith("func_")).map(([c]) => c);
  const m = s.measure;
  const len = m ? Math.hypot(m.b[0] - m.a[0], m.b[1] - m.a[1], m.b[2] - m.a[2]) : 0;
  return (
    <div className="editor-options">
      <label>
        Grid
        <select value={s.grid} onChange={(e) => s.set({ grid: Number(e.target.value) })} title="Grid size ([ and ] change it)">
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
      {picking && (
        <div className="seg" role="group" aria-label="What a click picks">
          {(["groups", "objects"] as const).map((k) => (
            <button key={k} type="button" aria-pressed={s.selectMode === k} title={k === "groups" ? "Clicking something in a group picks the whole group" : "Clicking picks just that brush or entity, even in a group"} onClick={() => s.set({ selectMode: k })}>
              {k === "groups" ? "Pick groups" : "Pick objects"}
            </button>
          ))}
        </div>
      )}
      {(picking || s.tool === "texture") && <MarqueeOptions texture={s.tool === "texture"} />}
      {s.tool === "vertex" && <span className="muted">{s.verts.length ? `${s.verts.length} corner${s.verts.length === 1 ? "" : "s"} picked. Arrows nudge, Page Up/Down move up and down.` : "Pick a brush, then click or drag its corners, edges (blue) or faces (purple)."}</span>}
      {s.tool === "shape" && (
        <div className="seg" role="group" aria-label="Shape">
          {SHAPES.map((x) => (
            <button key={x.id} type="button" aria-pressed={s.shape === x.id} onClick={() => s.set({ shape: x.id })}>
              {x.label}
            </button>
          ))}
        </div>
      )}
      {(s.tool === "box" || s.tool === "shape") && (
        <label>
          Height
          <input type="number" min={1} max={4096} step={s.grid} value={s.boxHeight} onChange={(e) => s.set({ boxHeight: num(e.target.value, 1, 4096, 128) })} />
        </label>
      )}
      {s.tool === "shape" && s.shape !== "wedge" && (
        <label title={s.shape === "sphere" ? "Facets round the middle (4-32)" : s.shape === "arch" ? "How many pieces (3-64)" : "Sides (3-64)"}>
          {s.shape === "arch" ? "Pieces" : "Sides"}
          <input type="number" min={s.shape === "sphere" ? 4 : 3} max={s.shape === "sphere" ? 32 : 64} value={s.sides} onChange={(e) => s.set({ sides: Math.round(num(e.target.value, s.shape === "sphere" ? 4 : 3, s.shape === "sphere" ? 32 : 64, 12)) })} />
        </label>
      )}
      {s.tool === "shape" && s.shape === "arch" && (
        <>
          <label title="How far round the arch goes">
            Arc°
            <input type="number" min={1} max={360} value={s.archArc} onChange={(e) => s.set({ archArc: num(e.target.value, 1, 360, 180) })} />
          </label>
          <label title="How thick the arch's wall is">
            Wall
            <input type="number" min={1} max={1024} step={s.grid} value={s.archWall} onChange={(e) => s.set({ archWall: num(e.target.value, 1, 1024, 16) })} />
          </label>
        </>
      )}
      {s.tool === "texture" && (
        <div className="seg" role="group" aria-label="What a click does">
          <button type="button" aria-pressed={s.texClick === "pick"} title="Click picks faces to line up in the face sheet" onClick={() => s.set({ texClick: "pick" })}>
            Click picks
          </button>
          <button type="button" aria-pressed={s.texClick === "paint"} title="Click paints the current texture (Shift: the whole brush)" onClick={() => s.set({ texClick: "paint" })}>
            Click paints
          </button>
        </div>
      )}
      {(s.tool === "box" || s.tool === "shape" || s.tool === "texture" || s.tool === "clip") && (
        <span className="editor-current-tex" title="The texture new brushes and painted faces get (pick one in Textures)">
          <img src={texUrl(s.texture)} alt="" onError={(e) => (e.currentTarget.style.visibility = "hidden")} onLoad={(e) => (e.currentTarget.style.visibility = "visible")} />
          {s.texture}
        </span>
      )}
      {s.tool === "entity" && (
        <label>
          Place
          <input className="editor-class-input" list="editor-point-classes" value={s.entityClass} onChange={(e) => s.set({ entityClass: e.target.value.trim() })} aria-label="Entity to place" spellCheck={false} />
          <datalist id="editor-point-classes">
            {points.map((c) => (
              <option key={c} value={c}>
                {fgd?.get(c)?.description ?? ENTITY_CLASSES.find(([x]) => x === c)?.[1]}
              </option>
            ))}
          </datalist>
        </label>
      )}
      {s.tool === "entity" && fgd?.get(s.entityClass) && <span className="muted editor-option-note">{fgd.get(s.entityClass)!.description}</span>}
      {s.tool === "measure" && (
        <span className="mono">
          {m ? `${Math.round(len * 100) / 100} units  (x ${Math.round(m.b[0] - m.a[0])}, y ${Math.round(m.b[1] - m.a[1])}, z ${Math.round(m.b[2] - m.a[2])})` : "Drag from one point to another."}
        </span>
      )}
      {s.tool === "clip" && (
        <>
          <span className="muted">{clip?.points ? (clip.points >= 2 ? "Cut placed." : "Click the second point.") : "Click the first point."}</span>
          <div className="seg" role="group" aria-label="Which side stays (Tab)">
            {(["both", "back", "front"] as const).map((k) => (
              <button key={k} type="button" aria-pressed={clip?.keep === k} title="Tab switches" onClick={() => s.setClipKeep?.(k)}>
                {k === "both" ? "Keep both" : k === "back" ? "Keep behind" : "Keep in front"}
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
  if (sel.face && !sel.brushes.length && !sel.entities.length) return <FaceSheet />;
  if (ent) return <EntityForm e={ent} />;
  if (sel.brushes.length || sel.entities.length) return <SelectionPanel />;
  return <MapPanel />;
}

function SelectionPanel() {
  const sel = useEditor((s) => s.sel);
  const thickness = useEditor((s) => s.thickness);
  const grid = useEditor((s) => s.grid);
  const [tie, setTie] = useState("func_wall");
  const fgd = useFgd();
  const bounds = selectionBounds();
  const ids = selectionIds();
  const s = useEditor.getState();
  const size = bounds ? bounds[1].map((x, i) => x - bounds[0][i]) : null;
  const centre = bounds ? (bounds[0].map((x, i) => (x + bounds[1][i]) / 2) as Vec3) : null;
  const n = ids.brushes.length;
  const turn90 = (axis: number, degrees: number) => {
    const ax: Vec3 = [0, 0, 0];
    ax[axis] = 1;
    void s.cmd({ op: "transform", ...ids, rotate: { axis: ax, degrees }, pivot: centre, lock: s.textureLock });
  };
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
      <MultiEntityKeys /> {/* P5: keys of several entities at once */}
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
            <button type="button" className="btn small" title="Several copies, each moved and turned from the last (Ctrl+Shift+V)" onClick={() => s.set({ dialog: "paste" })}>
              Paste special…
            </button>
          </div>
          <div className="editor-row wrap">
            <span className="muted editor-row-label">Flip</span>
            {["X", "Y", "Z"].map((a, i) => (
              <button key={a} type="button" className="btn small" title={`Mirror along ${a}`} onClick={() => mirror(i)}>
                {a}
              </button>
            ))}
          </div>
          <div className="editor-row wrap">
            <span className="muted editor-row-label">Turn 90°</span>
            {["X", "Y", "Z"].map((a, i) => (
              <button key={a} type="button" className="btn small" title={`Turn 90° round ${a} (Shift: the other way)`} onClick={(ev) => turn90(i, ev.shiftKey ? -90 : 90)}>
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
                {fgd && (
                  <optgroup label="Every brush entity">
                    {[...fgd.values()].filter((c) => c.kind === "solid" && c.name !== "worldspawn").map((c) => (
                      <option key={c.name} value={c.name} title={c.description}>
                        {c.name}
                      </option>
                    ))}
                  </optgroup>
                )}
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
        <button type="button" className="btn small" title="Hide what's picked for now (H). U shows everything again." onClick={() => hideSelection()}>
          Hide
        </button>
        <button type="button" className="btn small" title="Hide everything else (Shift+H)" onClick={() => isolateSelection()}>
          Isolate
        </button>
        <button type="button" className="btn small danger" onClick={() => deleteSelection()}>
          Delete
        </button>
      </div>
    </section>
  );
}

/** Hide the selection (this page only, not saved). */
export function hideSelection() {
  const s = useEditor.getState();
  const ids = selectionIds();
  if (!ids.brushes.length && !ids.entities.length) return;
  const brushes = new Set(s.hidden.brushes);
  const entities = new Set(s.hidden.entities);
  ids.brushes.forEach((b) => brushes.add(b));
  ids.entities.forEach((e) => entities.add(e));
  s.set({ hidden: { brushes, entities }, sel: EMPTY_SEL });
}

/** Hide everything but the selection. */
export function isolateSelection() {
  const s = useEditor.getState();
  const ids = selectionIds();
  if (!ids.brushes.length && !ids.entities.length) return;
  const keepB = new Set(ids.brushes);
  const keepE = new Set(ids.entities);
  const brushes = new Set([...s.brushes.keys()].filter((b) => !keepB.has(b)));
  const entities = new Set((s.doc?.entities ?? []).filter((e) => !keepE.has(e.id)).map((e) => e.id));
  s.set({ hidden: { brushes, entities } });
}

export function showAll() {
  useEditor.getState().set({ hidden: { brushes: new Set(), entities: new Set() } });
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
      <div className="editor-row wrap">
        <button type="button" className="btn small" onClick={() => s.set({ dialog: "props" })}>
          Map properties…
        </button>
        <button type="button" className="btn small" onClick={() => s.set({ dialog: "report" })}>
          Entity report…
        </button>
      </div>
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
      <Visibility />
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

/** Hidden things and simple visibility groups (named sets kept with this map on this computer). */
export function Visibility() { // P5: exported for the outliner
  const hidden = useEditor((s) => s.hidden);
  const docName = useEditor((s) => s.doc?.name ?? "");
  const sel = useEditor((s) => s.sel);
  // P4: kept in the project (sets made before, on this computer only, move into it once)
  const key = `ls.editor.vis.${docName}`;
  const groups: VisGroup[] = useEditor((s) => s.doc?.editor?.visSets) ?? [];
  const docId = useEditor((s) => s.doc?.docId);
  useEffect(() => {
    const old = readVis(key);
    if (old.length && !useEditor.getState().doc?.editor?.visSets?.length) {
      void setVisSets(old).then(() => {
        try {
          localStorage.removeItem(key);
        } catch {
          /* private window */
        }
      });
    }
  }, [key, docId]);
  const save = (g: VisGroup[]) => void setVisSets(g);
  const s = useEditor.getState();
  const n = hidden.brushes.size + hidden.entities.size;
  const has = sel.brushes.length + sel.entities.length > 0;
  const isHidden = (g: VisGroup) => g.brushes.every((b) => hidden.brushes.has(b)) && g.entities.every((e) => hidden.entities.has(e));
  const toggle = (g: VisGroup) => {
    const brushes = new Set(hidden.brushes);
    const entities = new Set(hidden.entities);
    const hide = !isHidden(g);
    for (const b of g.brushes) hide ? brushes.add(b) : brushes.delete(b);
    for (const e of g.entities) hide ? entities.add(e) : entities.delete(e);
    s.set({ hidden: { brushes, entities } });
  };
  return (
    <div className="editor-vis">
      <div className="editor-row wrap">
        <span className="muted">{n ? `${n} hidden` : "Nothing hidden"}</span>
        <button type="button" className="btn small" disabled={!n} title="Show everything (U)" onClick={() => showAll()}>
          Show all
        </button>
        <button
          type="button"
          className="btn small"
          disabled={!has}
          title="A named set of what's picked that you can hide and show in one click"
          onClick={() => {
            const ids = selectionIds();
            save([...groups, { name: `Set ${groups.length + 1}`, brushes: ids.brushes, entities: ids.entities }]);
          }}
        >
          New visibility set
        </button>
      </div>
      {groups.map((g, i) => (
        <div key={i} className="editor-tree-row">
          <label className="editor-check" title={isHidden(g) ? "Hidden: tick to show" : "Shown: untick to hide"}>
            <input type="checkbox" checked={!isHidden(g)} onChange={() => toggle(g)} />
          </label>
          <input className="editor-vis-name" value={g.name} aria-label="Set name" onChange={(e) => save(groups.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)))} />
          <button type="button" className="linklike" title="Pick what's in it" onClick={() => s.select({ brushes: g.brushes, entities: g.entities, face: null })}>
            {g.brushes.length + g.entities.length}
          </button>
          <button type="button" className="btn small icon" aria-label={`Remove ${g.name}`} title="Remove the set (not what's in it)" onClick={() => save(groups.filter((_, j) => j !== i))}>
            ×
          </button>
        </div>
      ))}
    </div>
  );
}

type VisGroup = { name: string; brushes: number[]; entities: number[] };
function readVis(key: string): VisGroup[] {
  try {
    const v = JSON.parse(localStorage.getItem(key) ?? "[]");
    return Array.isArray(v) ? v.filter((x) => x && typeof x.name === "string" && Array.isArray(x.brushes) && Array.isArray(x.entities)) : [];
  } catch {
    return [];
  }
}

// ---- texture browser -----------------------------------------------------------------

export function TextureBrowser() {
  const docWads = useEditor((s) => s.doc?.world.wads.join(";"));
  const current = useEditor((s) => s.texture);
  const [list, setList] = useState<WadTextures[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const [more, setMore] = useState(200);
  const [wad, setWad] = useState("");
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
  const all = useMemo(
    () => (list ?? []).filter((w) => !wad || w.name === wad).flatMap((w) => w.textures.map((t) => ({ ...t, wad: w.name }))).filter((t) => !query || t.name.includes(query)),
    [list, query, wad],
  );
  const paint = (name: string) => {
    s.set({ texture: name });
    const ids = selectionIds().brushes;
    const faces = facesOf(s.sel);
    if (faces.length) void s.cmd({ op: "set_texture", faces: faces.map((f) => [f.brush, f.face]), texture: name });
    else if (ids.length) void s.cmd({ op: "set_texture", faces: ids.map((b) => [b, null]), texture: name });
  };
  return (
    <section className="tools-panel editor-textures">
      <div className="editor-row">
        <input type="search" placeholder="Find a texture" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Find a texture" />
        <select value={wad} onChange={(e) => setWad(e.target.value)} aria-label="Which WAD" title="Show one texture file">
          <option value="">All WADs</option>
          {(list ?? []).filter((w) => w.found).map((w) => (
            <option key={w.name} value={w.name}>
              {w.name} ({w.textures.length})
            </option>
          ))}
        </select>
      </div>
      <RecentTextures onPick={(t) => s.set({ texture: t })} onPaint={paint} />
      <p className="small-note muted">Click to choose. Double-click to paint what's picked. {all.length} texture{all.length === 1 ? "" : "s"}.</p>
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
  const texture = useEditor((s) => s.texture);
  const [busy, setBusy] = useState<string | null>(null);
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
  const fix = async (list: Issue[], key: string) => {
    const first = list[0]?.fix;
    if (!first) return;
    if (first.startsWith("place:")) {
      s.set({ tool: "entity", entityClass: first.slice(6) });
      s.notify("Click a floor in the view to place it.");
      return;
    }
    setBusy(key);
    try {
      await s.cmd({ op: "fix", fixes: list.map((i) => ({ fix: i.fix, brush: i.brush, entity: i.entity, texture: i.texture })), with: first === "replace-texture" ? texture : undefined });
    } finally {
      setBusy(null);
    }
  };
  // fixes that can be done all at once, by kind
  const kinds = new Map<string, Issue[]>();
  for (const i of checks.issues) if (i.fix && !i.fix.startsWith("place:")) kinds.set(i.code, [...(kinds.get(i.code) ?? []), i]);
  const many = [...kinds.entries()].filter(([, l]) => l.length > 1);
  return (
    <section className="tools-panel editor-checks">
      {checks.issues.length === 0 ? (
        <p className="small-note">No problems found. (Leaks are found when the map is compiled.)</p>
      ) : (
        <>
          {many.map(([code, l]) => (
            <BusyButton key={code} busy={busy === `all-${code}`} className="btn small" label={`Fix all ${l.length}: ${FIX_LABEL[l[0].fix!] ?? "Fix"}`} busyLabel="Fixing…" onClick={() => void fix(l, `all-${code}`)} />
          ))}
          <ul>
            {checks.issues.map((i, n) => (
              <li key={n} className={`editor-issue ${i.level}`}>
                <button type="button" className="linklike" disabled={i.brush == null && i.entity == null} onClick={() => go(i)}>
                  <strong>{LEVEL_LABEL[i.level]}:</strong> {i.message}
                </button>
                {i.fix && (
                  <BusyButton
                    busy={busy === `one-${n}`}
                    className="btn small editor-fix"
                    label={i.fix === "replace-texture" ? `Replace with ${texture}` : FIX_LABEL[i.fix] ?? "Fix"}
                    busyLabel="Fixing…"
                    onClick={() => void fix([i], `one-${n}`)}
                  />
                )}
              </li>
            ))}
          </ul>
        </>
      )}
      {!checks.texturesChecked && <p className="small-note muted">Textures aren't checked until the game is set up on this computer.</p>}
    </section>
  );
}

const FIX_LABEL: Record<string, string> = {
  "delete-brush": "Delete the brush",
  "to-world": "Make its brushes world brushes",
  "delete-entity": "Delete it",
  "clear-target": "Clear the target",
  "add-wads": "Add halflife.wad and cstrike.wad",
  "replace-texture": "Replace with the current texture",
  "place:info_player_start": "Place a CT spawn",
  "place:info_player_deathmatch": "Place a T spawn",
};

export function BusyLabel({ busy, label }: { busy: boolean; label: string }) {
  return busy ? (
    <span className="editor-busy">
      <span className="spinner" aria-hidden="true" />
      {label}
    </span>
  ) : null;
}

export { BusyButton };
