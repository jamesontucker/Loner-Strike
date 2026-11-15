// P5: find and replace (textures, classnames, keys, values), transform by exact numbers (move,
// turn, stretch, line up and spread out), go to a brush or entity by number (compiler
// messages), grid and snapping settings, and replace with an entity or a prefab.

import { useEffect, useState } from "react";
import { BusyButton, Sheet } from "../../../ui";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import { selectionIds } from "./EditorScene";
import { frame, selectIds } from "./commands";
import { useFgd } from "./fgd";
import { cmd2, useP5, type FindSpec, type Found } from "./p5store";
import { originOf, selectionBounds, useEditor } from "./store";
import { AXES } from "./AxisTriad";

export function P5Dialogs() {
  const dialog = useP5((s) => s.dialog);
  const close = () => useP5.getState().set({ dialog: null });
  if (dialog === "find") return <FindReplace onClose={close} />;
  if (dialog === "transform") return <Transform onClose={close} />;
  if (dialog === "goto") return <GoTo onClose={close} />;
  if (dialog === "grid") return <GridSettings onClose={close} />;
  if (dialog === "replaceWith") return <ReplaceWith onClose={close} />;
  if (dialog === "info") return <MapInfo onClose={close} />;
  if (dialog === "coords") return <Coordinates onClose={close} />;
  return null;
}

const FIELDS: { id: FindSpec["field"]; label: string; find: string; replace: string }[] = [
  { id: "texture", label: "Textures", find: "e.g. CRATE01", replace: "e.g. CRATE02" },
  { id: "classname", label: "Entity types", find: "e.g. func_wall", replace: "e.g. func_illusionary" },
  { id: "value", label: "Values", find: "e.g. door1", replace: "e.g. gate1" },
  { id: "key", label: "Key names", find: "e.g. target", replace: "e.g. killtarget" },
];

function FindReplace({ onClose }: { onClose: () => void }) {
  const texture = useEditor((s) => s.texture);
  const [field, setField] = useState<FindSpec["field"]>("texture");
  const [find, setFind] = useState("");
  const [rep, setRep] = useState("");
  const [key, setKey] = useState("");
  const [whole, setWhole] = useState(true);
  const [mcase, setCase] = useState(false);
  const [inSel, setInSel] = useState(false);
  const [found, setFound] = useState<Found | null>(null);
  const [busy, setBusy] = useState<"find" | "replace" | null>(null);
  const hasSel = useEditor((s) => s.sel.brushes.length + s.sel.entities.length > 0);
  const f = FIELDS.find((x) => x.id === field)!;
  // a picked face or brush: start from its texture
  useEffect(() => {
    const s = useEditor.getState();
    const face = s.sel.face;
    const t = face ? s.brushes.get(face.brush)?.faces.find((x) => x.face === face.face)?.texture : s.sel.brushes[0] != null ? s.brushes.get(s.sel.brushes[0])?.faces[0]?.texture : null;
    if (t) setFind(t);
  }, []);
  useEffect(() => setFound(null), [field, find, key, whole, mcase, inSel]);
  const spec = (): FindSpec => {
    const ids = selectionIds();
    return { field, find: find.trim(), replace: rep.trim(), key: field === "value" && key.trim() ? key.trim() : null, whole, case: mcase, brushes: inSel ? ids.brushes : null, entities: inSel ? ids.entities : null };
  };
  const doFind = async () => {
    setBusy("find");
    try {
      const r = await cmd2({ op: "find", spec: spec() });
      if (r.found) setFound(r.found);
    } finally {
      setBusy(null);
    }
  };
  const doReplace = async () => {
    setBusy("replace");
    try {
      const r = await cmd2({ op: "replace", spec: spec() });
      if (!r.error) {
        useEditor.getState().notify(r.note ?? "Done.");
        setFound(null);
      }
    } finally {
      setBusy(null);
    }
  };
  return (
    <Sheet label="Find and replace" onClose={onClose} className="sheet ed-dialog">
      <h2>Find and replace</h2>
      <div className="seg" role="group" aria-label="What to look in">
        {FIELDS.map((x) => (
          <button key={x.id} type="button" aria-pressed={field === x.id} onClick={() => setField(x.id)}>
            {x.label}
          </button>
        ))}
      </div>
      <div className="ed-form">
        {field === "value" && (
          <label>
            Only the key
            <input value={key} onChange={(e) => setKey(e.target.value)} placeholder="any key (or e.g. target)" spellCheck={false} />
          </label>
        )}
        <label>
          Find
          <input autoFocus value={find} onChange={(e) => setFind(e.target.value)} placeholder={f.find} spellCheck={false} onKeyDown={(e) => e.key === "Enter" && void doFind()} />
        </label>
        <label>
          Replace with
          <span className="row">
            <input value={rep} onChange={(e) => setRep(e.target.value)} placeholder={f.replace} spellCheck={false} />
            {field === "texture" && (
              <button type="button" className="btn small" title="The texture picked in Textures" onClick={() => setRep(texture)}>
                Current texture
              </button>
            )}
          </span>
        </label>
        <div className="row wrap">
          <label className="editor-check">
            <input type="checkbox" checked={whole} onChange={(e) => setWhole(e.target.checked)} /> Whole {field === "value" ? "value" : "name"} only
          </label>
          <label className="editor-check">
            <input type="checkbox" checked={mcase} onChange={(e) => setCase(e.target.checked)} /> Match upper/lower case
          </label>
          <label className="editor-check" title={hasSel ? "" : "Pick something first"}>
            <input type="checkbox" checked={inSel && hasSel} disabled={!hasSel} onChange={(e) => setInSel(e.target.checked)} /> Only in what's picked
          </label>
        </div>
      </div>
      {found && (
        <p className="small-note">
          {found.count ? `Found in ${found.count} place${found.count === 1 ? "" : "s"}: ${found.brushes.length ? `${found.brushes.length} brush${found.brushes.length === 1 ? "" : "es"}` : ""}${found.brushes.length && found.entities.length ? ", " : ""}${found.entities.length ? `${found.entities.length} entit${found.entities.length === 1 ? "y" : "ies"}` : ""}.` : "Nothing matches."}{" "}
          {found.count > 0 && (
            <button
              type="button"
              className="linklike"
              onClick={() => {
                selectIds(found);
                frame(false);
              }}
            >
              Pick them
            </button>
          )}
        </p>
      )}
      <div className="row">
        <span className="push" />
        <button type="button" className="btn small" onClick={onClose}>
          Close
        </button>
        <BusyButton busy={busy === "find"} className="btn small" label="Find" busyLabel="Looking…" disabled={!find.trim() || !!busy} onClick={() => void doFind()} />
        <BusyButton
          busy={busy === "replace"}
          className="btn small primary"
          label={found?.count ? `Replace ${found.count}` : "Replace all"}
          busyLabel="Replacing…"
          disabled={!find.trim() || !!busy || (field !== "value" && !rep.trim())}
          onClick={() => void doReplace()}
        />
      </div>
      <p className="small-note muted">Replacing is one step: Undo takes it all back.</p>
    </Sheet>
  );
}

type TTab = "move" | "turn" | "stretch" | "arrange";

function Vec({ value, onChange, step }: { value: Vec3; onChange: (v: Vec3) => void; step: number }) {
  return (
    <span className="ed-vec3">
      {(["X", "Y", "Z"] as const).map((a, i) => (
        <label key={a}>
          <span>{a}</span>
          <input
            type="number"
            step={step}
            value={value[i]}
            onChange={(e) => {
              const v: Vec3 = [...value];
              v[i] = Number(e.target.value) || 0;
              onChange(v);
            }}
          />
        </label>
      ))}
    </span>
  );
}

function Transform({ onClose }: { onClose: () => void }) {
  const grid = useEditor((s) => s.grid);
  const lock = useEditor((s) => s.textureLock);
  const sel = useEditor((s) => s.sel);
  const many = sel.brushes.length + sel.entities.length > 1;
  const [tab, setTab] = useState<TTab>(many && useP5.getState().dialog === "transform" ? "move" : "move");
  const [move, setMove] = useState<Vec3>([0, 0, 0]);
  const [axis, setAxis] = useState(2);
  const [deg, setDeg] = useState(90);
  const [scale, setScale] = useState<Vec3>([100, 100, 100]);
  const [busy, setBusy] = useState(false);
  const b = selectionBounds();
  const size = b ? (b[1].map((x, i) => Math.round((x - b[0][i]) * 100) / 100) as Vec3) : null;
  const centre = b ? (b[0].map((x, i) => (x + b[1][i]) / 2) as Vec3) : null;
  const run = async (body: Record<string, unknown>) => {
    setBusy(true);
    try {
      await useEditor.getState().cmd({ op: "transform", ...selectionIds(), pivot: centre, lock, ...body });
    } finally {
      setBusy(false);
    }
  };
  const apply = () => {
    if (tab === "move") return run({ translate: move });
    if (tab === "turn") return run({ rotate: { axis: [0, 1, 2].map((i) => (i === axis ? 1 : 0)), degrees: deg } });
    return run({ scale: scale.map((x) => x / 100) });
  };
  const arrange = async (body: Record<string, unknown>) => {
    setBusy(true);
    try {
      await cmd2({ ...selectionIds(), lock, ...body });
    } finally {
      setBusy(false);
    }
  };
  return (
    <Sheet label="Transform" onClose={onClose} className="sheet ed-dialog">
      <h2>Transform</h2>
      {size && centre ? (
        <p className="small-note muted mono">
          Size {size.join(" × ")} · middle {centre.map((x) => Math.round(x)).join(" ")}
        </p>
      ) : (
        <p className="small-note">Pick something first.</p>
      )}
      <div className="seg" role="group" aria-label="Kind">
        {(
          [
            ["move", "Move"],
            ["turn", "Turn"],
            ["stretch", "Stretch"],
            ["arrange", "Line up"],
          ] as [TTab, string][]
        ).map(([id, l]) => (
          <button key={id} type="button" aria-pressed={tab === id} onClick={() => setTab(id)}>
            {l}
          </button>
        ))}
      </div>
      <div className="ed-form">
        {tab === "move" && (
          <label>
            Move by (units)
            <Vec value={move} onChange={setMove} step={grid} />
          </label>
        )}
        {tab === "turn" && (
          <>
            <div className="row">
              <span className="muted">Round</span>
              <div className="seg" role="group" aria-label="Axis">
                {["X", "Y", "Z"].map((a, i) => (
                  <button key={a} type="button" aria-pressed={axis === i} onClick={() => setAxis(i)}>
                    {a}
                  </button>
                ))}
              </div>
            </div>
            <label>
              Degrees (anticlockwise seen from the end of the axis)
              <input type="number" step={15} value={deg} onChange={(e) => setDeg(Number(e.target.value) || 0)} />
            </label>
          </>
        )}
        {tab === "stretch" && (
          <label>
            Stretch to (% of now, round the middle; −100 flips)
            <Vec value={scale} onChange={setScale} step={25} />
          </label>
        )}
        {tab === "arrange" && (
          <>
            {!many && <p className="small-note">Pick two or more things to line them up.</p>}
            {["X", "Y", "Z"].map((a, i) => (
              <div key={a} className="row wrap">
                <span className="ed-arr-axis">{a}</span>
                {(["min", "centre", "max"] as const).map((edge) => (
                  <button key={edge} type="button" className="btn small" disabled={!many || busy} onClick={() => void arrange({ op: "align", axis: i, edge })}>
                    {edge === "min" ? (i === 2 ? "Bottoms" : "Low sides") : edge === "max" ? (i === 2 ? "Tops" : "High sides") : "Middles"}
                  </button>
                ))}
                <button type="button" className="btn small" disabled={sel.brushes.length + sel.entities.length < 3 || busy} title="Equal gaps between them (first and last stay)" onClick={() => void arrange({ op: "distribute", axis: i })}>
                  Spread out
                </button>
              </div>
            ))}
          </>
        )}
      </div>
      <div className="row">
        <span className="muted small-note">Texture lock {lock ? "on" : "off"}</span>
        <span className="push" />
        <button type="button" className="btn small" onClick={onClose}>
          Close
        </button>
        {tab !== "arrange" && <BusyButton busy={busy} className="btn small primary" label="Apply" busyLabel="Working…" disabled={!size} onClick={() => void apply()} />}
      </div>
    </Sheet>
  );
}

/** Compiler numbers: entity 0 is the world (its brushes in id order), then entities in id order. */
export function byMapNumber(entity: number, brush: number | null): { brushes: number[]; entities: number[] } | string {
  const doc = useEditor.getState().doc;
  if (!doc) return "No map is open.";
  const ents = [...doc.entities].sort((a, b) => a.id - b.id);
  const brushesOf = (eid: number | null) => doc.brushes.filter((b) => b.entity === eid).sort((a, b) => a.id - b.id);
  if (entity === 0) {
    const list = brushesOf(null);
    if (brush == null) return "Entity 0 is the world: give a brush number too.";
    const b = list[brush];
    return b ? { brushes: [b.id], entities: [] } : `The world has ${list.length} brushes (0 to ${list.length - 1}).`;
  }
  const e = ents[entity - 1];
  if (!e) return `There are ${ents.length} entities besides the world (1 to ${ents.length}).`;
  if (brush == null) return { brushes: [], entities: [e.id] };
  const b = brushesOf(e.id)[brush];
  return b ? { brushes: [b.id], entities: [] } : `${e.classname} has ${brushesOf(e.id).length} brush(es).`;
}

function GoTo({ onClose }: { onClose: () => void }) {
  const [mode, setMode] = useState<"map" | "id">("map");
  const [text, setText] = useState("");
  const [err, setErr] = useState<string | null>(null);
  const go = () => {
    const nums = (text.match(/\d+/g) ?? []).map(Number);
    if (!nums.length) return setErr("Type a number.");
    const s = useEditor.getState();
    let r: ReturnType<typeof byMapNumber>;
    if (mode === "map") r = byMapNumber(nums[0], nums.length > 1 ? nums[1] : null);
    else {
      const isEnt = /ent/i.test(text);
      const id = nums[0];
      if (isEnt) r = s.doc?.entities.some((e) => e.id === id) ? { brushes: [], entities: [id] } : `There's no entity ${id}.`;
      else r = s.brushes.has(id) ? { brushes: [id], entities: [] } : s.doc?.entities.some((e) => e.id === id) ? { brushes: [], entities: [id] } : `There's no brush or entity ${id}.`;
    }
    if (typeof r === "string") return setErr(r);
    // brushes in a brush entity: pick the brush itself (the compiler means that one)
    s.select({ brushes: r.brushes, entities: r.entities, face: null });
    const ent = r.entities[0] != null ? s.doc?.entities.find((e) => e.id === r.entities[0]) : null;
    const o = ent && !ent.brushes.length ? originOf(ent.keys) : null;
    if (o) useTools.getState().look(o, 64);
    else frame(false);
    onClose();
  };
  return (
    <Sheet label="Go to a number" onClose={onClose} className="sheet ed-dialog">
      <h2>Go to a brush or entity</h2>
      <div className="seg" role="group" aria-label="Which numbers">
        <button type="button" aria-pressed={mode === "map"} onClick={() => setMode("map")} title="The numbers in compiler messages and other editors">
          Compiler numbers
        </button>
        <button type="button" aria-pressed={mode === "id"} onClick={() => setMode("id")} title="The numbers this editor shows (Brush 12)">
          Editor numbers
        </button>
      </div>
      <div className="ed-form">
        <label>
          {mode === "map" ? "Entity, brush (e.g. “Entity 0, Brush 12” or “3 1”)" : "Brush or entity number (e.g. “12” or “entity 4”)"}
          <input
            autoFocus
            value={text}
            onChange={(e) => {
              setText(e.target.value);
              setErr(null);
            }}
            onKeyDown={(e) => e.key === "Enter" && go()}
            placeholder={mode === "map" ? "Entity 0, Brush 12" : "12"}
          />
        </label>
        {err && <p className="small-note ed-err">{err}</p>}
        {mode === "map" && <p className="small-note muted">You can paste a whole line from the compiler's output. Entity 0 is the world.</p>}
      </div>
      <div className="row">
        <span className="push" />
        <button type="button" className="btn small" onClick={onClose}>
          Close
        </button>
        <button type="button" className="btn small primary" onClick={go}>
          Go
        </button>
      </div>
    </Sheet>
  );
}

const GRIDS = [1, 2, 4, 8, 16, 32, 64, 128, 256];
const TURNS = [0, 1, 5, 7.5, 11.25, 15, 22.5, 30, 45, 90];

function GridSettings({ onClose }: { onClose: () => void }) {
  const grid = useEditor((s) => s.grid);
  const lock = useEditor((s) => s.textureLock);
  const snapOff = useEditor((s) => s.snapOff);
  const p = useP5();
  const s = useEditor.getState();
  return (
    <Sheet label="Grid and snapping" onClose={onClose} className="sheet ed-dialog">
      <h2>Grid and snapping</h2>
      <div className="ed-form">
        <label>
          Grid size (units; [ and ] change it)
          <select value={grid} onChange={(e) => s.set({ grid: Number(e.target.value) })}>
            {GRIDS.map((g) => (
              <option key={g} value={g}>
                {g}
              </option>
            ))}
          </select>
        </label>
        <label className="editor-check">
          <input type="checkbox" checked={p.showGrid} onChange={(e) => p.set({ showGrid: e.target.checked })} /> Show the grid
        </label>
        <label>
          Turning snaps to (degrees; hold Alt for free)
          <select value={p.rotSnap} onChange={(e) => p.set({ rotSnap: Number(e.target.value) })}>
            {TURNS.map((t) => (
              <option key={t} value={t}>
                {t === 0 ? "Off (free)" : `${t}°`}
              </option>
            ))}
          </select>
        </label>
        <label className="editor-check" title="Brighter lines every 64 units in the flat views (Hammer's highlight lines)">
          <input type="checkbox" checked={p.gridMajor} onChange={(e) => p.set({ gridMajor: e.target.checked })} /> Brighter lines every 64 units (flat views)
        </label>
        <label className="editor-check" title="Off: things move freely (Alt flips it while held)">
          <input type="checkbox" checked={!snapOff} onChange={(e) => s.set({ snapOff: !e.target.checked })} /> Snap to the grid
        </label>
        <label className="editor-check" title="New points (boxes, clip points, entities, measuring) land on a corner or edge of a brush when you're close to one">
          <input type="checkbox" checked={p.geoSnap} onChange={(e) => p.set({ geoSnap: e.target.checked })} /> Snap to brush corners and edges
        </label>
        <label className="editor-check" title="Textures stay put on the surface when you move, turn or stretch a brush">
          <input type="checkbox" checked={lock} onChange={(e) => s.set({ textureLock: e.target.checked })} /> Texture lock
        </label>
      </div>
      <div className="row">
        <span className="push" />
        <button type="button" className="btn small primary" onClick={onClose}>
          Done
        </button>
      </div>
    </Sheet>
  );
}

type PrefabCard = { id: string; name: string; builtin: boolean; thumb: string; brushes: number; entities: number };

function ReplaceWith({ onClose }: { onClose: () => void }) {
  const fgd = useFgd();
  const sel = useEditor((s) => s.sel);
  const doc = useEditor((s) => s.doc);
  const [mode, setMode] = useState<"entity" | "prefab">("entity");
  const [cls, setCls] = useState(useEditor.getState().entityClass);
  const [prefabs, setPrefabs] = useState<PrefabCard[] | null>(null);
  const [pick, setPick] = useState<string>("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (mode !== "prefab" || prefabs) return;
    void fetch("/api/tools/editor/prefabs", { credentials: "same-origin", headers: { "X-Panel": "1" } })
      .then((r) => r.json())
      .then((j) => setPrefabs(Array.isArray(j.prefabs) ? j.prefabs : []))
      .catch(() => setPrefabs([]));
  }, [mode, prefabs]);
  const points = (doc?.entities ?? []).filter((e) => sel.entities.includes(e.id) && !e.brushes.length);
  const brushThings = sel.brushes.length + (doc?.entities ?? []).filter((e) => sel.entities.includes(e.id) && e.brushes.length).length;
  const fgdClass = fgd?.get(cls);
  const toEntity = async () => {
    const s = useEditor.getState();
    setBusy(true);
    try {
      if (fgdClass?.kind === "solid") {
        if (!selectionIds().brushes.length) return s.notify("That's a brush entity: pick brushes to turn into it.", true);
        const r = await s.cmd({ op: "tie", brushes: selectionIds().brushes, classname: cls });
        if (r.created?.entities.length) s.select({ brushes: [], entities: r.created.entities, face: null });
        return;
      }
      if (!points.length) return s.notify("Pick point entities (spawns, lights…) to change their type.", true);
      for (const e of points) await s.cmd({ op: "set_key", entity: e.id, key: "classname", value: cls }, { doc: false });
      await s.refreshDoc();
      s.notify(`Changed ${points.length} entit${points.length === 1 ? "y" : "ies"} to ${cls}.`);
      onClose();
    } finally {
      setBusy(false);
    }
  };
  const toPrefab = async () => {
    const s = useEditor.getState();
    if (!pick) return;
    setBusy(true);
    try {
      // one prefab per picked thing, standing where it stood; then the old ones go
      const spots: Vec3[] = [];
      for (const id of s.sel.brushes) {
        const b = s.brushes.get(id);
        if (b) spots.push([(b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, b.min[2]]);
      }
      for (const id of s.sel.entities) {
        const e = s.doc?.entities.find((x) => x.id === id);
        if (!e) continue;
        if (e.brushes.length) {
          const g = e.brushes.map((x) => s.brushes.get(x)).filter(Boolean) as { min: Vec3; max: Vec3 }[];
          if (g.length) spots.push([(Math.min(...g.map((x) => x.min[0])) + Math.max(...g.map((x) => x.max[0]))) / 2, (Math.min(...g.map((x) => x.min[1])) + Math.max(...g.map((x) => x.max[1]))) / 2, Math.min(...g.map((x) => x.min[2]))]);
        } else {
          const o = originOf(e.keys);
          if (o) spots.push(o);
        }
      }
      if (!spots.length) return;
      const old = { brushes: s.sel.brushes, entities: s.sel.entities };
      const made: { brushes: number[]; entities: number[] } = { brushes: [], entities: [] };
      for (const p of spots) {
        const r = await s.cmd({ op: "place_prefab", prefab: pick, point: p, normal: [0, 0, 1], grid: 0 }, { doc: false });
        if (r.error) return;
        if (r.created) {
          made.brushes.push(...r.created.brushes);
          made.entities.push(...r.created.entities);
        }
      }
      await s.cmd({ op: "delete", ...old });
      selectIds(made);
      onClose();
    } finally {
      setBusy(false);
    }
  };
  const n = sel.brushes.length + sel.entities.length;
  return (
    <Sheet label="Replace with" onClose={onClose} className="sheet ed-dialog">
      <h2>Replace {n} picked thing{n === 1 ? "" : "s"} with…</h2>
      <div className="seg" role="group" aria-label="Replace with">
        <button type="button" aria-pressed={mode === "entity"} onClick={() => setMode("entity")}>
          Another entity
        </button>
        <button type="button" aria-pressed={mode === "prefab"} onClick={() => setMode("prefab")}>
          A prefab
        </button>
      </div>
      {mode === "entity" ? (
        <div className="ed-form">
          <label>
            Entity type
            <input list="p5-classes" value={cls} onChange={(e) => setCls(e.target.value.trim())} spellCheck={false} autoFocus />
            <datalist id="p5-classes">
              {[...(fgd?.values() ?? [])].map((c) => (
                <option key={c.name} value={c.name}>
                  {c.description}
                </option>
              ))}
            </datalist>
          </label>
          <p className="small-note muted">
            {fgdClass ? `${fgdClass.description} (${fgdClass.kind === "solid" ? "brush entity: the picked brushes become it" : "point entity: picked point entities change type, keeping their keys"}).` : "Point entities keep their place and keys; brushes are tied to a brush entity."}
            {fgdClass?.kind !== "solid" && brushThings > 0 && " Picked brushes are left as they are."}
          </p>
        </div>
      ) : (
        <div className="ed-prefab-pick">
          {!prefabs && (
            <p className="small-note muted">
              <span className="spinner" aria-hidden="true" />
              Reading the prefab library…
            </p>
          )}
          {prefabs?.length === 0 && <p className="small-note">No prefabs yet. Save some from the prefab library.</p>}
          {prefabs?.map((p) => (
            <button key={p.id} type="button" className="ed-prefab-card" aria-pressed={pick === p.id} onClick={() => setPick(p.id)} title={`${p.brushes} brushes, ${p.entities} entities`}>
              <img src={p.thumb} alt="" loading="lazy" onError={(e) => (e.currentTarget.style.visibility = "hidden")} />
              <span>{p.name}</span>
            </button>
          ))}
          <p className="small-note muted">Each picked thing is swapped for the prefab, standing where it stood. One undo step per piece.</p>
        </div>
      )}
      <div className="row">
        <span className="push" />
        <button type="button" className="btn small" onClick={onClose}>
          Close
        </button>
        <BusyButton busy={busy} className="btn small primary" label="Replace" busyLabel="Replacing…" disabled={!n || (mode === "prefab" ? !pick : !cls)} onClick={() => void (mode === "entity" ? toEntity() : toPrefab())} />
      </div>
    </Sheet>
  );
}

/** The arch tool's start angle, next to its other options. */
export function ArchStart() {
  const v = useP5((s) => s.archStart);
  return (
    <label title="Where the arch starts, in degrees round from east (x)">
      Start°
      <input type="number" min={-360} max={360} step={15} value={v} onChange={(e) => useP5.getState().set({ archStart: Math.max(-360, Math.min(360, Number(e.target.value) || 0)) })} />
    </label>
  );
}

/** Hammer's Map information: counts, textures and texture files in use. */
function MapInfo({ onClose }: { onClose: () => void }) {
  const doc = useEditor((s) => s.doc);
  const meshes = useEditor((s) => s.brushes);
  if (!doc) return null;
  const tex = new Map<string, number>();
  let faces = 0;
  for (const b of meshes.values()) for (const f of b.faces) {
    faces++;
    tex.set(f.texture, (tex.get(f.texture) ?? 0) + 1);
  }
  const ents = doc.entities;
  const solid = ents.filter((e) => e.brushes.length).length;
  const classes = new Map<string, number>();
  for (const e of ents) classes.set(e.classname, (classes.get(e.classname) ?? 0) + 1);
  const world = doc.brushes.filter((b) => b.entity == null).length;
  const [lo, hi] = doc.bounds;
  const rows: [string, string | number][] = [
    ["Brushes", `${doc.brushes.length} (${world} world, ${doc.brushes.length - world} in brush entities)`],
    ["Faces", faces],
    ["Entities", `${ents.length} (${ents.length - solid} point, ${solid} brush)`],
    ["Kinds of entity", classes.size],
    ["Textures used", tex.size],
    ["Texture files", doc.world.wads.join(", ") || "none"],
    ["Groups", doc.groups.length],
    ["Size", hi.map((x, i) => Math.round(x - lo[i])).join(" × ")],
  ];
  return (
    <Sheet label="Map information" onClose={onClose} className="sheet ed-dialog">
      <h2>Map information</h2>
      <dl className="ed-info">
        {rows.map(([k, v]) => (
          <div key={k}>
            <dt>{k}</dt>
            <dd>{v}</dd>
          </div>
        ))}
      </dl>
      <h4>Most used textures</h4>
      <p className="small-note mono">{[...tex.entries()].sort((a, b) => b[1] - a[1]).slice(0, 12).map(([t, n]) => `${t} ${n}`).join(" · ")}</p>
      <h4>Entities by kind</h4>
      <p className="small-note mono">{[...classes.entries()].sort((a, b) => b[1] - a[1]).slice(0, 16).map(([c, n]) => `${c} ${n}`).join(" · ")}</p>
      <div className="row">
        <span className="push" />
        <button type="button" className="btn small primary" onClick={onClose}>
          Done
        </button>
      </div>
    </Sheet>
  );
}

/** Help > Coordinates and axes: the convention, said once, plainly. */
function Coordinates({ onClose }: { onClose: () => void }) {
  return (
    <Sheet label="Coordinates and axes" onClose={onClose} className="sheet ed-dialog">
      <h2>Coordinates and axes</h2>
      <p>The editor uses the game's own coordinates, the same as Hammer, J.A.C.K. and the .map file:</p>
      <ul className="ed-axes">
        {AXES.map((a) => (
          <li key={a.id}>
            <b style={{ color: a.colour }}>{a.id}</b> points {a.word}
            {a.id === "Z" ? " (Z is height; a player stands 72 units tall)" : a.id === "X" ? " (to the right in the Top view)" : " (up the screen in the Top view)"}
          </li>
        ))}
      </ul>
      <p className="small-note">
        The numbers never change with the key set (Hammer, TrenchBroom, Unreal): only the keys and mouse do. Unreal and Blender also put Z up, but Unreal's Y points the other way and its units differ, so a position copied from there will be mirrored. The status bar shows the cursor's position as X Y Z; the triad in each view's corner shows where the axes point from that view.
      </p>
      <p className="small-note muted">Angles: yaw turns round Z, 0 faces east (+X), 90 faces north (+Y). Pitch tips up and down, roll leans sideways.</p>
      <div className="row">
        <span className="push" />
        <button type="button" className="btn small primary" onClick={onClose}>
          Done
        </button>
      </div>
    </Sheet>
  );
}
