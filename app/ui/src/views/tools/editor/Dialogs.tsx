// The editor's dialogs: paste special (copies in a row, a ring or a spiral), map properties
// (world, sun, rules, texture files, entity definitions) and the entity report. All on
// EditorDialog: a header, a body that scrolls, a footer that stays; Enter does the main
// action, Esc closes; numbers are checked as you type.

import { useEffect, useMemo, useState } from "react";
import { BusyButton, Info } from "../../../ui";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import { clearFgd, getFgd, getGameWads, pickFgd, type DocEntity, type EditorSettings } from "./editorApi";
import { DialogSection, EditorDialog, Field } from "./EditorDialog";
import { selectionIds } from "./EditorScene";
import { rgbToHex, hexToRgb } from "./EntityForm";
import { reloadFgd, useFgd } from "./fgd";
import { keyOf, originOf, selectionBounds, useEditor } from "./store";

export function Dialogs() {
  const dialog = useEditor((s) => s.dialog);
  const close = () => useEditor.getState().set({ dialog: null });
  if (dialog === "paste") return <PasteSpecial onClose={close} />;
  if (dialog === "props") return <MapProperties onClose={close} />;
  if (dialog === "report") return <EntityReport onClose={close} />;
  return null;
}

/** A number typed in a field: the value, or an error when it's not one or out of range. */
function checkNum(text: string, lo: number, hi: number, what: string, optional = false): string | null {
  if (!text.trim()) return optional ? null : `Type ${what}.`;
  const n = Number(text);
  if (!Number.isFinite(n)) return "That isn't a number.";
  if (n < lo || n > hi) return `Between ${lo} and ${hi}.`;
  return null;
}

function Vec({ label, value, onChange, step, help }: { label: string; value: string[]; onChange: (v: string[]) => void; step: number; help?: string }) {
  const bad = value.some((x) => x.trim() !== "" && !Number.isFinite(Number(x)));
  return (
    <Field label={label} hint={help} error={bad ? "Numbers only." : null}>
      {(id) => (
        <div className="ed-vec" id={id}>
          {(["X", "Y", "Z"] as const).map((a, i) => (
            <label key={a}>
              <span>{a}</span>
              <input
                type="number"
                step={step}
                value={value[i]}
                aria-label={`${label} ${a}`}
                onChange={(e) => {
                  const v = [...value];
                  v[i] = e.target.value;
                  onChange(v);
                }}
              />
            </label>
          ))}
        </div>
      )}
    </Field>
  );
}

const vecNum = (v: string[]) => v.map((x) => Number(x) || 0) as Vec3;

function PasteSpecial({ onClose }: { onClose: () => void }) {
  const grid = useEditor((s) => s.grid);
  const b = selectionBounds();
  const size = b ? (b[1].map((x, i) => x - b[0][i]) as Vec3) : ([64, 64, 64] as Vec3);
  const [copies, setCopies] = useState("4");
  const [offset, setOffset] = useState<string[]>([String(Math.max(grid, Math.round(size[0]))), "0", "0"]);
  const [turn, setTurn] = useState<string[]>(["0", "0", "0"]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ids = selectionIds();
  const none = !ids.brushes.length && !ids.entities.length;
  const copiesError = checkNum(copies, 1, 256, "how many copies");
  const n = Math.round(Number(copies) || 1);
  const bad = !!copiesError || [...offset, ...turn].some((x) => x.trim() !== "" && !Number.isFinite(Number(x)));
  const go = async () => {
    if (bad || none) return;
    setBusy(true);
    const s = useEditor.getState();
    const r = await s.cmd({ op: "array", ...ids, copies: n, offset: vecNum(offset), rotate: vecNum(turn), lock: s.textureLock }, { quiet: true });
    setBusy(false);
    if (r.error) return setError(r.error);
    onClose();
    if (r.created) {
      const owned = new Set(r.created.entities);
      s.select({ brushes: r.created.brushes.filter((x) => !owned.has(useEditor.getState().brushes.get(x)?.entity ?? -1)), entities: r.created.entities, face: null });
    }
  };
  const preset = (o: Vec3, t: Vec3) => {
    setOffset(o.map(String));
    setTurn(t.map(String));
  };
  return (
    <EditorDialog
      title="Paste special"
      subtitle="Copies of what's picked, each one moved and turned from the one before. One undo takes them all back."
      onClose={onClose}
      error={none ? "Pick something first." : error}
      primary={{ label: `Make ${n} cop${n === 1 ? "y" : "ies"}`, onClick: () => void go(), busy, busyLabel: "Copying…", disabled: none || bad }}
    >
      <DialogSection title="Quick start">
        <div className="ed-inline">
          <button type="button" className="btn small" onClick={() => preset([Math.round(size[0]), 0, 0], [0, 0, 0])}>Row</button>
          <button type="button" className="btn small" onClick={() => preset([Math.round(size[0]), 0, Math.max(grid, 8)], [0, 0, 0])}>Stairs</button>
          <button type="button" className="btn small" onClick={() => preset([0, 0, 0], [0, 0, Math.round(360 / (n + 1))])}>Ring</button>
          <button type="button" className="btn small" onClick={() => preset([0, 0, Math.max(grid, 8)], [0, 0, 15])}>Spiral</button>
        </div>
      </DialogSection>
      <DialogSection title="Each copy">
        <Field label="Copies" error={copiesError}>{(id) => <input id={id} type="number" min={1} max={256} value={copies} onChange={(e) => setCopies(e.target.value)} />}</Field>
        <Vec label="Moved by" value={offset} onChange={setOffset} step={grid} />
        <Vec label="Turned by (°)" value={turn} onChange={setTurn} step={15} help="Turns round the middle of what's picked. Z turns it like a compass; turn and move together for a spiral staircase." />
      </DialogSection>
    </EditorDialog>
  );
}

const SKIES = ["desert", "night", "city", "cliff", "office", "snow", "space", "morning", "dusk", "black", "neb1", "trainyard", "2desert", "backalley", "de_storm", "green", "hav", "cx", "tornsky"];

function MapProperties({ onClose }: { onClose: () => void }) {
  const doc = useEditor((s) => s.doc)!;
  const [gameWads, setGameWads] = useState<{ name: string; game: string }[] | null>(null);
  const kv = (k: string) => keyOf(doc.world.keys, k) ?? "";
  const [sky, setSky] = useState(kv("skyname"));
  const [title, setTitle] = useState(kv("message"));
  const [light, setLight] = useState(kv("light"));
  const [range, setRange] = useState(kv("MaxRange"));
  const [wads, setWads] = useState(doc.world.wads);
  const [adding, setAdding] = useState("");
  const sun = doc.entities.find((e) => e.classname === "light_environment");
  const sunLight = (sun && keyOf(sun.keys, "_light")) || "255 255 128 200";
  const [sunColour, setSunColour] = useState(rgbToHex(sunLight));
  const [sunBright, setSunBright] = useState(sunLight.split(/\s+/)[3] ?? "200");
  const [sunPitch, setSunPitch] = useState((sun && keyOf(sun.keys, "pitch")) || "-60");
  const [sunAngle, setSunAngle] = useState((sun && keyOf(sun.keys, "angle")) || "45");
  const rules = doc.entities.find((e) => e.classname === "info_map_parameters");
  const [buying, setBuying] = useState((rules && keyOf(rules.keys, "buying")) || "0");
  const [bomb, setBomb] = useState((rules && keyOf(rules.keys, "bombradius")) || "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    void getGameWads().then((r) => setGameWads(r.wads ?? []));
  }, []);
  const listed = new Set(wads.map((w) => w.split(/[\\/]/).pop()!.toLowerCase()));
  const errors = {
    range: checkNum(range, 1024, 32768, "a distance", true),
    light: checkNum(light, 0, 255, "a light level", true),
    bright: checkNum(sunBright, 0, 5000, "a brightness"),
    pitch: checkNum(sunPitch, -90, 90, "a height"),
    angle: checkNum(sunAngle, 0, 360, "an angle"),
    bomb: checkNum(bomb, 0, 4096, "a radius", true),
    sky: sky.trim() && !/^[A-Za-z0-9_-]{1,32}$/.test(sky.trim()) ? "Letters, numbers, _ and - only." : null,
  };
  const bad = Object.values(errors).some(Boolean);
  const save = async () => {
    if (bad) return;
    setBusy(true);
    setError(null);
    const s = useEditor.getState();
    try {
      const keys = doc.world.keys.filter(([k]) => !["skyname", "message", "light", "MaxRange"].includes(k));
      for (const [k, v] of [["skyname", sky], ["message", title], ["light", light], ["MaxRange", range]] as const) if (v.trim()) keys.push([k, v.trim()]);
      const r = await s.cmd({ op: "set_world", keys, wads }, { quiet: true });
      if (r.error) return setError(r.error);
      const sunValue = `${hexToRgb(sunColour)} ${sunBright || "200"}`;
      if (sun) {
        for (const [k, v] of [["_light", sunValue], ["pitch", sunPitch], ["angle", sunAngle]] as const) if ((keyOf(sun.keys, k) ?? "") !== v) await s.cmd({ op: "set_key", entity: sun.id, key: k, value: v });
      } else if (sunValue !== "255 255 128 200" || sunPitch !== "-60" || sunAngle !== "45") {
        const c = selectionBounds({ ...s, sel: { brushes: [...s.brushes.keys()], entities: [], face: null } });
        const o = c ? [(c[0][0] + c[1][0]) / 2, (c[0][1] + c[1][1]) / 2, c[1][2] - 32].map(Math.round) : [0, 0, 128];
        await s.cmd({ op: "add_entity", classname: "light_environment", origin: o, keys: [["_light", sunValue], ["pitch", sunPitch], ["angle", sunAngle]] });
      }
      const wantRules = buying !== "0" || bomb.trim() !== "";
      if (rules) {
        if ((keyOf(rules.keys, "buying") ?? "0") !== buying) await s.cmd({ op: "set_key", entity: rules.id, key: "buying", value: buying === "0" ? "" : buying });
        if ((keyOf(rules.keys, "bombradius") ?? "") !== bomb.trim()) await s.cmd({ op: "set_key", entity: rules.id, key: "bombradius", value: bomb.trim() });
      } else if (wantRules) {
        const keys: [string, string][] = [];
        if (buying !== "0") keys.push(["buying", buying]);
        if (bomb.trim()) keys.push(["bombradius", bomb.trim()]);
        await s.cmd({ op: "add_entity", classname: "info_map_parameters", origin: [0, 0, 0], keys });
      }
      onClose();
    } finally {
      setBusy(false);
    }
  };
  return (
    <EditorDialog
      title="Map properties"
      subtitle={doc.name}
      size="lg"
      onClose={onClose}
      error={error}
      primary={{ label: "Save properties", onClick: () => void save(), busy, busyLabel: "Saving…", disabled: bad }}
      footerNote={bad ? "Fix the fields in red first." : "One undo step per change."}
    >
      <DialogSection title="World" cols={2}>
        <Field label="Title" hint="Shown while the map loads.">{(id) => <input id={id} value={title} onChange={(e) => setTitle(e.target.value)} placeholder="de_mymap" />}</Field>
        <Field label="Sky" error={errors.sky}>
          {(id) => (
            <>
              <input id={id} value={sky} list="editor-skies" onChange={(e) => setSky(e.target.value)} placeholder="desert" spellCheck={false} />
              <datalist id="editor-skies">
                {SKIES.map((x) => (
                  <option key={x} value={x} />
                ))}
              </datalist>
            </>
          )}
        </Field>
        <Field label="Draw distance" hint="How far players see (units). Empty: the game's 4096." error={errors.range}>
          {(id) => <input id={id} type="number" min={1024} max={32768} step={512} value={range} onChange={(e) => setRange(e.target.value)} placeholder="4096" />}
        </Field>
        <Field label="Base light" hint="A little light everywhere so nothing is pitch black. 0 or empty is off." error={errors.light}>
          {(id) => <input id={id} type="number" min={0} max={255} value={light} onChange={(e) => setLight(e.target.value)} placeholder="0" />}
        </Field>
      </DialogSection>
      <DialogSection title={sun ? "Sun" : "Sun (none yet: one is added)"} cols={2}>
        <Field label="Color">{(id) => <input id={id} type="color" value={sunColour} onChange={(e) => setSunColour(e.target.value)} />}</Field>
        <Field label="Brightness" error={errors.bright}>{(id) => <input id={id} type="number" min={0} max={5000} value={sunBright} onChange={(e) => setSunBright(e.target.value)} />}</Field>
        <Field label="Height (pitch)" hint="-90 shines straight down, -30 is a low evening sun." error={errors.pitch}>
          {(id) => <input id={id} type="number" min={-90} max={0} value={sunPitch} onChange={(e) => setSunPitch(e.target.value)} />}
        </Field>
        <Field label="From (angle)" hint="Which way the sunlight travels, seen from above (0 east, 90 north)." error={errors.angle}>
          {(id) => <input id={id} type="number" min={0} max={359} value={sunAngle} onChange={(e) => setSunAngle(e.target.value)} />}
        </Field>
      </DialogSection>
      <DialogSection title="Rules" cols={2}>
        <Field label="Buying">
          {(id) => (
            <select id={id} value={buying} onChange={(e) => setBuying(e.target.value)}>
              <option value="0">Both teams can buy</option>
              <option value="1">Only Counter-Terrorists</option>
              <option value="2">Only Terrorists</option>
              <option value="3">Nobody</option>
            </select>
          )}
        </Field>
        <Field label="Bomb blast radius" hint="Empty: the game's 500." error={errors.bomb}>
          {(id) => <input id={id} type="number" min={0} max={4096} value={bomb} onChange={(e) => setBomb(e.target.value)} placeholder="500" />}
        </Field>
      </DialogSection>
      <DialogSection title={<>Texture files (WADs) <Info text="The texture files this map uses. Players need them too unless the compiler packs the textures into the map." /></>}>
        <ul className="ed-list">
          {wads.map((w) => (
            <li key={w}>
              <span className="mono" title={w}>{w}</span>
              <button type="button" className="btn small icon" aria-label={`Remove ${w}`} title="Remove" onClick={() => setWads(wads.filter((x) => x !== w))}>
                ×
              </button>
            </li>
          ))}
          {!wads.length && <li className="muted">None: the compiler won't find any textures.</li>}
        </ul>
        {gameWads && (
          <div className="ed-inline">
            <select className="ed-grow" value={adding} onChange={(e) => setAdding(e.target.value)} aria-label="Add a WAD">
              <option value="">Add a WAD…</option>
              {gameWads.filter((w) => !listed.has(w.name.toLowerCase())).map((w) => (
                <option key={w.game + w.name} value={w.name}>
                  {w.name} ({w.game})
                </option>
              ))}
            </select>
            <button type="button" className="btn small" disabled={!adding} onClick={() => { setWads([...wads, adding]); setAdding(""); }}>
              Add
            </button>
          </div>
        )}
      </DialogSection>
      <FgdChoice />
    </EditorDialog>
  );
}

function EntityReport({ onClose }: { onClose: () => void }) {
  const doc = useEditor((s) => s.doc)!;
  const fgd = useFgd();
  const [q, setQ] = useState("");
  const [cls, setCls] = useState("");
  const [picked, setPicked] = useState<Set<number>>(new Set());
  const classes = useMemo(() => {
    const m = new Map<string, number>();
    for (const e of doc.entities) m.set(e.classname, (m.get(e.classname) ?? 0) + 1);
    return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  }, [doc.entities]);
  const query = q.trim().toLowerCase();
  const rows = doc.entities.filter((e) => (!cls || e.classname === cls) && (!query || e.classname.includes(query) || e.keys.some(([k, v]) => k !== "classname" && v.toLowerCase().includes(query))));
  const s = useEditor.getState();
  const where = (e: DocEntity) => {
    const o = originOf(e.keys);
    return o ? o.map(Math.round).join(" ") : e.brushes.length ? `${e.brushes.length} brush${e.brushes.length === 1 ? "" : "es"}` : "–";
  };
  const look = (e: DocEntity) => {
    s.select({ brushes: [], entities: [e.id], face: null });
    const o = originOf(e.keys);
    if (o) useTools.getState().look(o, 96);
    else {
      const b = selectionBounds();
      if (b) useTools.getState().look(b[0].map((x, i) => (x + b[1][i]) / 2) as Vec3, Math.max(64, ...b[1].map((x, i) => x - b[0][i])));
    }
    onClose();
  };
  const toggle = (id: number) =>
    setPicked((p) => {
      const n = new Set(p);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const allShown = rows.length > 0 && rows.every((e) => picked.has(e.id));
  return (
    <EditorDialog
      title="Entity report"
      subtitle={`${doc.entities.length} entities. Double-click a row to go to it.`}
      size="lg"
      onClose={onClose}
      closeLabel="Close"
      primary={{
        label: picked.size ? `Pick ${picked.size}` : "Pick",
        disabled: !picked.size,
        onClick: () => {
          s.select({ brushes: [], entities: [...picked], face: null });
          onClose();
        },
      }}
      secondary={
        <button type="button" className="btn danger" disabled={!picked.size} onClick={() => void s.cmd({ op: "delete", entities: [...picked] }).then((r) => !r.error && setPicked(new Set()))}>
          Delete {picked.size || ""}
        </button>
      }
    >
      <div className="ed-inline">
        <input className="ed-grow" type="search" placeholder="Find by class, name or any value" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Find" autoFocus />
        <select value={cls} onChange={(e) => setCls(e.target.value)} aria-label="Class">
          <option value="">Every class ({doc.entities.length})</option>
          {classes.map(([c, n]) => (
            <option key={c} value={c}>
              {c} ({n})
            </option>
          ))}
        </select>
      </div>
      <div className="ed-table-wrap">
        <table>
          <thead>
            <tr>
              <th>
                <input type="checkbox" checked={allShown} disabled={!rows.length} aria-label="Tick all shown" title="Tick all shown" onChange={() => setPicked(allShown ? new Set() : new Set(rows.map((e) => e.id)))} />
              </th>
              <th>Class</th>
              <th>Name</th>
              <th>Where</th>
            </tr>
          </thead>
          <tbody>
            {rows.slice(0, 500).map((e) => (
              <tr key={e.id} aria-selected={picked.has(e.id)} onDoubleClick={() => look(e)}>
                <td>
                  <input type="checkbox" checked={picked.has(e.id)} onChange={() => toggle(e.id)} aria-label={`Pick ${e.classname}`} />
                </td>
                <td title={fgd?.get(e.classname)?.description ?? e.classname}>{e.classname}</td>
                <td className="mono" title={keyOf(e.keys, "targetname") ?? ""}>{keyOf(e.keys, "targetname") ?? ""}</td>
                <td className="mono">{where(e)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {rows.length > 500 && <p className="small-note muted">Showing 500 of {rows.length}. Narrow the search to see the rest.</p>}
      {!rows.length && <p className="small-note muted">Nothing matches.</p>}
    </EditorDialog>
  );
}

/** P4: the map's own entity definitions (an FGD file read on top of ours; kept in the project). */
function FgdChoice() {
  const file = useEditor((s) => s.doc?.editor?.fgd ?? null);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ text: string; bad?: boolean } | null>(null);
  const [info, setInfo] = useState<{ source: string; error?: string } | null>(null);
  useEffect(() => {
    void getFgd().then((r) => !r.error && setInfo({ source: r.source, error: r.custom?.error }));
  }, [file]);
  const done = async (r: { error?: string; editor?: EditorSettings; cancelled?: boolean; classes?: number; dirty?: boolean; revision?: number }) => {
    setBusy(false);
    if (r.error) return setMsg({ text: r.error, bad: true });
    if (r.cancelled) return;
    const d = useEditor.getState().doc;
    if (d && r.editor) useEditor.setState({ doc: { ...d, editor: r.editor, dirty: r.dirty ?? d.dirty, revision: r.revision ?? d.revision } });
    await reloadFgd();
    setMsg(r.classes != null ? { text: `Read ${r.classes} entity classes from it.` } : { text: "Back to ours only." });
  };
  return (
    <DialogSection title={<>Entity definitions <Info text="The entity classes, keys and help the inspector shows. Ours cover Counter-Strike and the game mode components; add your own FGD file for custom entities (its classes win over ours). Kept with the project." /></>}>
      <p className="small-note">{info?.source ?? "Loner-Strike entity definitions"}</p>
      {info?.error && (
        <p className="small-note ed-field-error" role="alert">
          {file} couldn't be read: {info.error}
        </p>
      )}
      {msg && <p className={`small-note${msg.bad ? " ed-field-error" : " muted"}`} role="status">{msg.text}</p>}
      <div className="ed-inline">
        <BusyButton className="btn small" busy={busy} label={file ? "Choose another FGD…" : "Add your own FGD…"} busyLabel="Reading…" onClick={() => { setBusy(true); setMsg(null); void pickFgd().then(done); }} />
        {file && (
          <button type="button" className="btn small" disabled={busy} onClick={() => { setBusy(true); setMsg(null); void clearFgd().then(done); }}>
            Use ours only
          </button>
        )}
      </div>
    </DialogSection>
  );
}
