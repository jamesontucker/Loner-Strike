// P5: the Inspect panel's shared parts, for every kind of selection:
// - Transform, like Unreal's Details: Location, Rotation and Scale (Size for brushes) number
//   fields in the engine's own X Y Z. Type a number, or drag a field's label to scrub it (one
//   undo step per drag; Alt turns the grid snap off). Texture lock and scale lock sit here too.
// - What the brushes are made of: the textures they use (thumbnails; click one to paint with
//   it), which entity they belong to, their group and visgroups.
// - For picked faces: which brush, which way it faces, and how its texture is aligned.

import { useEffect, useRef, useState } from "react";
import type { Vec3 } from "../viewer/coords";
import { alignOf, texUrl, type DocEntity, type MeshFace } from "./editorApi";
import { selectionIds } from "./EditorScene";
import { AXES } from "./AxisTriad";
import { facesOf, keyOf, originOf, selectionBounds, useEditor } from "./store";
import { openInspectMenu } from "./InspectMenu"; // right-click Copy / Paste
import { transformItems } from "./inspectMenus";

const round = (x: number, d = 2) => Math.round(x * 10 ** d) / 10 ** d;

/** A number field whose label scrubs: drag it left/right to change the value. */
function ScrubField({
  axis,
  label,
  value,
  step,
  unit,
  title,
  onSet,
  onScrub,
  onScrubEnd,
}: {
  axis: number;
  label?: string;
  value: number;
  step: number;
  unit?: string;
  title: string;
  onSet: (v: number) => void;
  /** live: the total change so far (in steps of `step`) */
  onScrub?: (total: number) => void;
  onScrubEnd?: () => void;
}) {
  const [text, setText] = useState(String(round(value)));
  const [focused, setFocused] = useState(false);
  const [scrubbing, setScrubbing] = useState(false);
  useEffect(() => {
    if (!focused) setText(String(round(value)));
  }, [value, focused]);
  const a = AXES[axis];
  const down = (e: React.PointerEvent) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const el = e.currentTarget as HTMLElement;
    el.setPointerCapture(e.pointerId);
    const x0 = e.clientX;
    let last = 0;
    setScrubbing(true);
    const move = (m: PointerEvent) => {
      const fine = m.altKey ? 0.1 : 1;
      const total = Math.round(((m.clientX - x0) / 6) * fine) * step;
      if (total !== last) {
        last = total;
        onScrub?.(total);
      }
    };
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
      el.removeEventListener("pointercancel", up);
      setScrubbing(false);
      onScrubEnd?.();
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
    el.addEventListener("pointercancel", up);
  };
  const commit = () => {
    const v = Number(text);
    if (Number.isFinite(v) && Math.abs(v - value) > 1e-9) onSet(v);
    else setText(String(round(value)));
  };
  return (
    <label className={`p5-xf-field${scrubbing ? " scrubbing" : ""}`} title={title}>
      <span className="p5-xf-axis" style={{ background: a.colour }} onPointerDown={onScrub ? down : undefined} aria-hidden="true">
        {label ?? a.id}
      </span>
      <input
        type="text"
        inputMode="decimal"
        value={text}
        aria-label={title}
        onFocus={(e) => {
          setFocused(true);
          e.currentTarget.select();
        }}
        onBlur={() => {
          setFocused(false);
          commit();
        }}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
          else if (e.key === "Escape") {
            setText(String(round(value)));
            (e.currentTarget as HTMLInputElement).blur();
          } else if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            e.preventDefault();
            onSet(round(value + (e.key === "ArrowUp" ? step : -step), 4));
          }
        }}
      />
      {unit && <span className="p5-xf-unit">{unit}</span>}
    </label>
  );
}

/** Sends live steps of one scrub as a single undo step, never more than one on the way. */
function useScrubber() {
  const st = useRef<{ key: string; sent: number; want: number; busy: boolean } | null>(null);
  const begin = () => (st.current ??= { key: `scrub-${Date.now()}`, sent: 0, want: 0, busy: false });
  const pump = async (send: (delta: number, merge: string) => Promise<unknown>) => {
    const s = st.current;
    if (!s || s.busy || s.want === s.sent) return;
    s.busy = true;
    const delta = s.want - s.sent;
    await send(delta, s.key);
    s.sent += delta;
    s.busy = false;
    void pump(send);
  };
  return {
    to(total: number, send: (delta: number, merge: string) => Promise<unknown>) {
      begin().want = total;
      void pump(send);
    },
    end() {
      // the last step still on its way finishes on its own; the next scrub is a new undo step
      st.current = null;
    },
  };
}

/** Location, rotation and scale of what's picked, in the engine's own numbers. */
export function TransformSection({ entity }: { entity?: DocEntity }) {
  useEditor((s) => s.sel);
  useEditor((s) => s.doc); // follow every change
  const grid = useEditor((s) => s.grid);
  const lock = useEditor((s) => s.textureLock);
  const scaleLock = useEditor((s) => s.scaleLock);
  const alt = useEditor((s) => s.alt);
  const scrub = useScrubber();
  const [turnBy, setTurnBy] = useState<Vec3>([0, 0, 0]);
  const [scaleBy, setScaleBy] = useState<Vec3>([1, 1, 1]);
  const s = useEditor.getState();
  const ids = selectionIds();
  const bounds = selectionBounds();
  if (!bounds && !entity) return null;
  const point = entity && !entity.brushes.length ? entity : null;
  const origin = point ? originOf(point.keys) : null;
  const centre: Vec3 = origin ?? (bounds ? (bounds[0].map((x, i) => (x + bounds[1][i]) / 2) as Vec3) : [0, 0, 0]);
  const size = bounds && !point ? (bounds[1].map((x, i) => x - bounds[0][i]) as Vec3) : null;
  const moveStep = alt ? 1 : grid;

  const move = (axis: number, d: number, merge?: string) => {
    const t: Vec3 = [0, 0, 0];
    t[axis] = d;
    return s.cmd({ op: "transform", ...ids, translate: t, lock, ...(merge ? { merge } : {}) }, merge ? { quiet: true } : undefined);
  };
  const turn = (axis: number, deg: number, merge?: string) => {
    const ax: Vec3 = [0, 0, 0];
    ax[axis] = 1;
    return s.cmd({ op: "transform", ...ids, rotate: { axis: ax, degrees: deg }, pivot: centre, lock, ...(merge ? { merge } : {}) }, merge ? { quiet: true } : undefined);
  };
  const stretch = (axis: number, f: number, merge?: string) => {
    const v: Vec3 = [1, 1, 1];
    v[axis] = f;
    return s.cmd({ op: "transform", ...ids, scale: v, pivot: centre, lock, ...(merge ? { merge } : {}) }, merge ? { quiet: true } : undefined);
  };

  // a point entity's angles: "pitch yaw roll" (Y, Z, X), or the older single "angle" (yaw)
  const anglesKey = point ? keyOf(point.keys, "angles") : undefined;
  const angleKey = point ? keyOf(point.keys, "angle") : undefined;
  const pyr = (anglesKey ?? "").split(/\s+/).map(Number);
  const angles: Vec3 = point ? [Number.isFinite(pyr[2]) ? pyr[2] : 0, Number.isFinite(pyr[0]) ? pyr[0] : 0, anglesKey ? (Number.isFinite(pyr[1]) ? pyr[1] : 0) : Number(angleKey ?? 0) || 0] : [0, 0, 0];
  const setAngle = (axis: number, deg: number) => {
    if (!point) return;
    const a: Vec3 = [...angles];
    a[axis] = round(((deg % 360) + 360) % 360, 3);
    // the single "angle" key holds the yaw only: keep using it until pitch or roll is set
    if (!anglesKey && a[0] === 0 && a[1] === 0) return void s.cmd({ op: "set_key", entity: point.id, key: "angle", value: String(a[2]) });
    void s.cmd({ op: "set_key", entity: point.id, key: "angles", value: `${a[1]} ${a[2]} ${a[0]}` });
  };
  const scaleKey = point ? keyOf(point.keys, "scale") : undefined;

  return (
    <div className="p5-xf" role="group" aria-label="Transform">
      <h4 onContextMenu={(ev) => openInspectMenu(ev, transformItems())} title="Right-click: Copy / Paste the Transform">
        Transform <span className="muted small-note">Z up · X east · Y north</span>
      </h4>
      <div className="p5-xf-row" onContextMenu={(ev) => openInspectMenu(ev, transformItems("location"))}>
        <span className="p5-xf-label" title="Where it is: the middle of what's picked (a point entity's origin)">Location</span>
        {[0, 1, 2].map((i) => (
          <ScrubField
            key={i}
            axis={i}
            value={centre[i]}
            step={moveStep}
            unit=""
            title={`${AXES[i].id} (${AXES[i].word}): type a position, or drag the ${AXES[i].id} to slide it ${moveStep} at a time`}
            onSet={(v) => void move(i, v - centre[i])}
            onScrub={(total) => scrub.to(total, (d, m) => move(i, d, m))}
            onScrubEnd={() => scrub.end()}
          />
        ))}
      </div>
      <div className="p5-xf-row" onContextMenu={(ev) => openInspectMenu(ev, transformItems("rotation"))}>
        <span className="p5-xf-label" title={point ? "Its angles: roll round X, pitch round Y, yaw round Z (0 faces east)" : "Turn it round each axis by this many degrees (about its middle); drag a letter to turn it live"}>
          {point ? "Rotation" : "Turn by"}
        </span>
        {[0, 1, 2].map((i) => (
          <ScrubField
            key={i}
            axis={i}
            label={point ? ["R", "P", "Y"][i] : undefined}
            value={point ? angles[i] : turnBy[i]}
            step={alt ? 1 : 15}
            unit="°"
            title={point ? `${["Roll", "Pitch", "Yaw"][i]} (round ${AXES[i].id}), degrees` : `Turn round ${AXES[i].id}, degrees`}
            onSet={(v) => {
              if (point) return setAngle(i, v);
              void turn(i, v);
              setTurnBy([0, 0, 0]);
            }}
            // a point entity scrubs its yaw only (turning it round Z, one undo step); pitch and roll are typed
            onScrub={!point || i === 2 ? (total) => scrub.to(total, (d, m) => turn(i, d, m)) : undefined}
            onScrubEnd={() => {
              scrub.end();
              setTurnBy([0, 0, 0]);
            }}
          />
        ))}
      </div>
      {size ? (
        <div className="p5-xf-row" onContextMenu={(ev) => openInspectMenu(ev, transformItems("size"))}>
          <span className="p5-xf-label" title="How big it is in units; type a size to stretch it to that, about its middle">Size</span>
          {[0, 1, 2].map((i) => (
            <ScrubField
              key={i}
              axis={i}
              value={size[i]}
              step={moveStep}
              title={`Size along ${AXES[i].id}, units: type one to stretch it to that`}
              onSet={(v) => v >= 1 && size[i] > 0 && void stretch(i, v / size[i])}
            />
          ))}
        </div>
      ) : null}
      {size ? (
        <div className="p5-xf-row" onContextMenu={(ev) => openInspectMenu(ev, transformItems("scale"))}>
          <span className="p5-xf-label" title="Stretch it by this much (1 is as it is, 2 twice as big, -1 mirrors)">Scale</span>
          {[0, 1, 2].map((i) => (
            <ScrubField
              key={i}
              axis={i}
              value={scaleBy[i]}
              step={0.25}
              unit="×"
              title={`Stretch along ${AXES[i].id} by this much`}
              onSet={(v) => {
                if (Math.abs(v) < 0.01) return;
                void stretch(i, v);
                setScaleBy([1, 1, 1]);
              }}
            />
          ))}
        </div>
      ) : point && (scaleKey !== undefined || /^env_sprite|^cycler|^env_model/.test(point.classname)) ? (
        <div className="p5-xf-row" onContextMenu={(ev) => openInspectMenu(ev, transformItems("scale"))}>
          <span className="p5-xf-label" title="The sprite's or model's scale key">Scale</span>
          <ScrubField axis={2} label="×" value={Number(scaleKey ?? 1) || 1} step={0.1} title="Scale (the scale key)" onSet={(v) => v > 0 && void s.cmd({ op: "set_key", entity: point.id, key: "scale", value: String(round(v, 3)) })} />
        </div>
      ) : null}
      {!point && (
        <div className="p5-xf-locks">
          <label className="editor-check" title="Textures stay put on the surface when you move, turn or stretch a brush">
            <input type="checkbox" checked={lock} onChange={(e) => s.set({ textureLock: e.target.checked })} /> Texture lock
          </label>
          <label className="editor-check" title={lock ? "On: stretching a brush stretches its textures with it. Off: they keep their size and repeat" : "Works with texture lock on"}>
            <input type="checkbox" disabled={!lock} checked={lock && scaleLock} onChange={(e) => s.set({ scaleLock: e.target.checked })} /> Scale lock
          </label>
        </div>
      )}
    </div>
  );
}

/** What the picked brushes are made of and where they belong. */
export function BrushFacts({ brushIds }: { brushIds: number[] }) {
  const brushes = useEditor((s) => s.brushes);
  const doc = useEditor((s) => s.doc);
  const current = useEditor((s) => s.texture);
  if (!brushIds.length || !doc) return null;
  const tex = new Map<string, number>();
  const owners = new Map<number | null, number>();
  const groups = new Set<number>();
  let faces = 0;
  for (const id of brushIds) {
    const b = brushes.get(id);
    if (!b) continue;
    owners.set(b.entity, (owners.get(b.entity) ?? 0) + 1);
    if (b.group != null) groups.add(b.group);
    for (const f of b.faces) {
      faces++;
      tex.set(f.texture, (tex.get(f.texture) ?? 0) + 1);
    }
  }
  const pick = new Set(brushIds);
  const vis = (doc.editor?.visSets ?? []).filter((v) => v.brushes.some((b) => pick.has(b)));
  const s = useEditor.getState();
  const ownerLabel = (e: number | null) => (e == null ? "World" : doc.entities.find((x) => x.id === e)?.classname ?? "an entity");
  const list = [...tex.entries()].sort((a, b) => b[1] - a[1]);
  return (
    <div className="p5-facts" aria-label="What the brushes are made of">
      <h4>
        Textures <span className="muted small-note">{list.length} on {faces} faces · click one to paint with it</span>
      </h4>
      <div className="p5-facts-tex" role="list">
        {list.slice(0, 24).map(([t, n]) => (
          <button key={t} type="button" role="listitem" className={`p5-facts-tile${t === current ? " on" : ""}`} title={`${t}: ${n} face${n === 1 ? "" : "s"}. Click to make it the current texture.`} onClick={() => s.set({ texture: t })}>
            <img src={texUrl(t)} alt="" loading="lazy" onError={(e) => (e.currentTarget.style.visibility = "hidden")} />
            <span className="mono">{t}</span>
            <span className="muted">{n}</span>
          </button>
        ))}
        {list.length > 24 && <span className="muted small-note">and {list.length - 24} more</span>}
      </div>
      <dl className="p5-facts-dl">
        <dt>Belongs to</dt>
        <dd>
          {[...owners.entries()].map(([e, n], i) => (
            <span key={String(e)}>
              {i > 0 && ", "}
              {e == null ? (
                <span title="Plain world brushes: solid walls, floors and ceilings">World</span>
              ) : (
                <button type="button" className="linklike" title="Pick the entity" onClick={() => s.select({ brushes: [], entities: [e], face: null })}>
                  {ownerLabel(e)}
                </button>
              )}
              {owners.size > 1 || n !== brushIds.length ? ` (${n})` : ""}
            </span>
          ))}
        </dd>
        <dt>Group</dt>
        <dd>{groups.size ? [...groups].map((g) => doc.groups.find((x) => x.id === g)?.name ?? `#${g}`).join(", ") : <span className="muted">none</span>}</dd>
        <dt>Visgroups</dt>
        <dd>{vis.length ? vis.map((v) => v.name).join(", ") : <span className="muted">none</span>}</dd>
      </dl>
    </div>
  );
}

const DIRS: [Vec3, string][] = [
  [[0, 0, 1], "up (+Z)"],
  [[0, 0, -1], "down (−Z)"],
  [[1, 0, 0], "east (+X)"],
  [[-1, 0, 0], "west (−X)"],
  [[0, 1, 0], "north (+Y)"],
  [[0, -1, 0], "south (−Y)"],
];

/** Which way a face points, in words. */
export function facing(n: Vec3): string {
  const best = DIRS.map(([d, w]) => [d[0] * n[0] + d[1] * n[1] + d[2] * n[2], w] as const).sort((a, b) => b[0] - a[0])[0];
  return best[0] > 0.999 ? best[1] : `mostly ${best[1]}, sloped`;
}

/** World (projected from the nearest axis, Hammer's default) or face (lying flat on it). */
export function alignKind(f: MeshFace): "world" | "face" | "both" | "custom" {
  const a = alignOf(f);
  if (!a) return "custom";
  const c = [a.u[1] * a.v[2] - a.u[2] * a.v[1], a.u[2] * a.v[0] - a.u[0] * a.v[2], a.u[0] * a.v[1] - a.u[1] * a.v[0]];
  const len = Math.hypot(...c) || 1;
  const n = f.normal;
  const onFace = Math.abs((c[0] * n[0] + c[1] * n[1] + c[2] * n[2]) / len) > 0.999;
  const onAxis = c.some((x) => Math.abs(x / len) > 0.999);
  return onFace && onAxis ? "both" : onFace ? "face" : onAxis ? "world" : "custom";
}

/** For picked faces: which brush, which way, how the texture lies. */
export function FaceFacts() {
  const sel = useEditor((s) => s.sel);
  const brushes = useEditor((s) => s.brushes);
  const doc = useEditor((s) => s.doc);
  const refs = facesOf(sel);
  if (!refs.length) return null;
  const r = refs[0];
  const b = brushes.get(r.brush);
  const f = b?.faces.find((x) => x.face === r.face);
  if (!b || !f) return null;
  const poly = b.polys.get(r.face) ?? [];
  const lo = [0, 1, 2].map((i) => Math.min(...poly.map((p) => p[i])));
  const hi = [0, 1, 2].map((i) => Math.max(...poly.map((p) => p[i])));
  const axes = [0, 1, 2].filter((i) => hi[i] - lo[i] > 0.01);
  const dims = axes.length ? [`${axes.map((i) => round(hi[i] - lo[i])).join(" × ")} (${axes.map((i) => AXES[i].id).join(" × ")})`] : [];
  const kinds = new Set(refs.map((x) => brushes.get(x.brush)?.faces.find((y) => y.face === x.face)).filter((x): x is MeshFace => !!x).map(alignKind));
  const kind = kinds.size === 1 ? [...kinds][0] : "mixed";
  const owner = b.entity == null ? "World" : doc?.entities.find((e) => e.id === b.entity)?.classname ?? "an entity";
  return (
    <dl className="p5-facts-dl p5-face-facts">
      <dt>Faces</dt>
      <dd>{refs.length === 1 ? facing(f.normal) : `${refs.length} picked; the first faces ${facing(f.normal)}`}</dd>
      {refs.length === 1 && (
        <>
          <dt>Size</dt>
          <dd className="mono">{dims.join(" × ") || "–"}</dd>
        </>
      )}
      <dt>Brush</dt>
      <dd>
        <button type="button" className="linklike" title="Pick the whole brush" onClick={() => useEditor.getState().select({ brushes: [r.brush], entities: [], face: null })}>
          #{r.brush}
        </button>{" "}
        in {owner}
      </dd>
      <dt>Alignment</dt>
      <dd title="World: projected from the nearest axis, like Hammer's default. Face: lying flat along the face (good on slopes). On a face square to an axis the two are the same.">
        {kind === "both" ? "World (and face: it's square to an axis)" : kind === "world" ? "World" : kind === "face" ? "Face" : kind === "mixed" ? "Mixed" : "Custom (turned or skewed)"}
      </dd>
    </dl>
  );
}
