// The editor's dialogs: paste special (copies in a row, a ring or a spiral), map properties
// (sky, WADs, light, buying) and the entity report.

import { useEffect, useMemo, useState } from "react";
import { BusyButton, Info, Sheet } from "../../../ui";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import { getGameWads, type DocEntity } from "./editorApi";
import { selectionIds } from "./EditorScene";
import { rgbToHex, hexToRgb } from "./EntityForm";
import { useFgd } from "./fgd";
import { keyOf, originOf, selectionBounds, useEditor } from "./store";

export function Dialogs() {
  const dialog = useEditor((s) => s.dialog);
  const close = () => useEditor.getState().set({ dialog: null });
  if (dialog === "paste") return <PasteSpecial onClose={close} />;
  if (dialog === "props") return <MapProperties onClose={close} />;
  if (dialog === "report") return <EntityReport onClose={close} />;
  return null;
}

function Vec({ label, value, onChange, step, help }: { label: string; value: Vec3; onChange: (v: Vec3) => void; step: number; help?: string }) {
  return (
    <div className="editor-vec">
      <span className="editor-vec-label">
        {label}
        {help && <Info text={help} />}
      </span>
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
    </div>
  );
}

function PasteSpecial({ onClose }: { onClose: () => void }) {
  const grid = useEditor((s) => s.grid);
  const b = selectionBounds();
  const size = b ? (b[1].map((x, i) => x - b[0][i]) as Vec3) : ([64, 64, 64] as Vec3);
  const [copies, setCopies] = useState(4);
  const [offset, setOffset] = useState<Vec3>([Math.max(grid, Math.round(size[0])), 0, 0]);
  const [turn, setTurn] = useState<Vec3>([0, 0, 0]);
  const [busy, setBusy] = useState(false);
  const ids = selectionIds();
  const none = !ids.brushes.length && !ids.entities.length;
  const go = async () => {
    setBusy(true);
    const s = useEditor.getState();
    const r = await s.cmd({ op: "array", ...ids, copies, offset, rotate: turn, lock: s.textureLock });
    setBusy(false);
    if (!r.error) {
      onClose();
      if (r.created) {
        const owned = new Set(r.created.entities);
        s.select({ brushes: r.created.brushes.filter((x) => !owned.has(useEditor.getState().brushes.get(x)?.entity ?? -1)), entities: r.created.entities, face: null });
      }
    }
  };
  return (
    <Sheet label="Paste special" onClose={onClose} className="sheet editor-dialog">
      <div className="sheet-body">
        <h2>Paste special</h2>
        <p className="muted small-note">Copies of what's picked, each one moved and turned from the one before: a row of pillars, a staircase, a ring of lights. One undo takes them all back.</p>
        {none ? (
          <p className="banner">Pick something first.</p>
        ) : (
          <>
            <label className="editor-field editor-inline">
              Copies
              <input type="number" min={1} max={256} value={copies} onChange={(e) => setCopies(Math.max(1, Math.min(256, Math.round(Number(e.target.value) || 1))))} />
            </label>
            <Vec label="Move each by" value={offset} onChange={setOffset} step={grid} />
            <Vec label="Turn each by (°)" value={turn} onChange={setTurn} step={15} help="Turns round the middle of what's picked. Z turns it like a compass; turn and move together for a spiral staircase." />
            <div className="editor-presets">
              <span className="muted">Quick:</span>
              <button type="button" className="btn small" onClick={() => { setOffset([Math.round(size[0]), 0, 0]); setTurn([0, 0, 0]); }}>Row</button>
              <button type="button" className="btn small" onClick={() => { setOffset([Math.round(size[0]), 0, Math.max(grid, 8)]); setTurn([0, 0, 0]); }}>Stairs</button>
              <button type="button" className="btn small" onClick={() => { setOffset([0, 0, 0]); setTurn([0, 0, Math.round(360 / (copies + 1))]); }}>Ring</button>
              <button type="button" className="btn small" onClick={() => { setOffset([0, 0, Math.max(grid, 8)]); setTurn([0, 0, 15]); }}>Spiral</button>
            </div>
          </>
        )}
        <div className="row ask-buttons">
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <BusyButton busy={busy} className="btn primary" disabled={none} label={`Make ${copies} cop${copies === 1 ? "y" : "ies"}`} busyLabel="Copying…" onClick={() => void go()} />
        </div>
      </div>
    </Sheet>
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
  useEffect(() => {
    void getGameWads().then((r) => setGameWads(r.wads ?? []));
  }, []);
  const listed = new Set(wads.map((w) => w.split(/[\\/]/).pop()!.toLowerCase()));
  const save = async () => {
    setBusy(true);
    const s = useEditor.getState();
    try {
      const keys = doc.world.keys.filter(([k]) => !["skyname", "message", "light", "MaxRange"].includes(k));
      for (const [k, v] of [["skyname", sky], ["message", title], ["light", light], ["MaxRange", range]] as const) if (v.trim()) keys.push([k, v.trim()]);
      const r = await s.cmd({ op: "set_world", keys, wads });
      if (r.error) return;
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
    <Sheet label="Map properties" onClose={onClose} className="sheet wide editor-dialog">
      <div className="sheet-body">
        <h2>Map properties</h2>
        <div className="editor-props">
          <section>
            <h4>World</h4>
            <label className="editor-field">
              Title (shown while loading)
              <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder="de_mymap" />
            </label>
            <label className="editor-field">
              Sky
              <input value={sky} list="editor-skies" onChange={(e) => setSky(e.target.value)} placeholder="desert" spellCheck={false} />
              <datalist id="editor-skies">
                {SKIES.map((x) => (
                  <option key={x} value={x} />
                ))}
              </datalist>
            </label>
            <label className="editor-field">
              Draw distance
              <input type="number" min={1024} max={32768} step={512} value={range} onChange={(e) => setRange(e.target.value)} placeholder="4096" />
            </label>
            <label className="editor-field">
              <span className="editor-field-head">
                Base light <Info text="A little light everywhere so nothing is pitch black. 0 or empty is off." />
              </span>
              <input type="number" min={0} max={255} value={light} onChange={(e) => setLight(e.target.value)} placeholder="0" />
            </label>
          </section>
          <section>
            <h4>Sun {sun ? "" : <span className="muted">(none yet: one is added)</span>}</h4>
            <div className="editor-row">
              <label className="editor-field">
                Colour
                <input type="color" value={sunColour} onChange={(e) => setSunColour(e.target.value)} />
              </label>
              <label className="editor-field">
                Brightness
                <input type="number" min={0} max={5000} value={sunBright} onChange={(e) => setSunBright(e.target.value)} />
              </label>
            </div>
            <div className="editor-row">
              <label className="editor-field">
                <span className="editor-field-head">
                  Height (pitch) <Info text="-90 shines straight down, -30 is a low evening sun." />
                </span>
                <input type="number" min={-90} max={0} value={sunPitch} onChange={(e) => setSunPitch(e.target.value)} />
              </label>
              <label className="editor-field">
                <span className="editor-field-head">
                  From (angle) <Info text="Which way the sunlight travels, seen from above (0 east, 90 north)." />
                </span>
                <input type="number" min={0} max={359} value={sunAngle} onChange={(e) => setSunAngle(e.target.value)} />
              </label>
            </div>
            <h4>Rules</h4>
            <label className="editor-field">
              Buying
              <select value={buying} onChange={(e) => setBuying(e.target.value)}>
                <option value="0">Both teams can buy</option>
                <option value="1">Only Counter-Terrorists</option>
                <option value="2">Only Terrorists</option>
                <option value="3">Nobody</option>
              </select>
            </label>
            <label className="editor-field">
              Bomb blast radius
              <input type="number" min={0} max={4096} value={bomb} onChange={(e) => setBomb(e.target.value)} placeholder="500" />
            </label>
          </section>
          <section>
            <h4>
              Texture files (WADs) <Info text="The texture files this map uses. Players need them too unless the compiler packs the textures into the map." />
            </h4>
            <ul className="editor-wads">
              {wads.map((w) => (
                <li key={w}>
                  <span className="mono">{w}</span>
                  <button type="button" className="btn small icon" aria-label={`Remove ${w}`} title="Remove" onClick={() => setWads(wads.filter((x) => x !== w))}>
                    ×
                  </button>
                </li>
              ))}
              {!wads.length && <li className="muted">None: the compiler won't find any textures.</li>}
            </ul>
            {gameWads && (
              <div className="editor-row">
                <select value={adding} onChange={(e) => setAdding(e.target.value)} aria-label="Add a WAD">
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
          </section>
        </div>
        <div className="row ask-buttons">
          <button type="button" className="btn" onClick={onClose}>
            Cancel
          </button>
          <BusyButton busy={busy} className="btn primary" label="Save properties" busyLabel="Saving…" onClick={() => void save()} />
        </div>
      </div>
    </Sheet>
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
  const toggle = (id: number) => setPicked((p) => {
    const n = new Set(p);
    n.has(id) ? n.delete(id) : n.add(id);
    return n;
  });
  return (
    <Sheet label="Entity report" onClose={onClose} className="sheet wide editor-dialog">
      <div className="sheet-body">
        <h2>Entity report</h2>
        <div className="editor-row">
          <input type="search" placeholder="Find by class, name or any value" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Find" autoFocus />
          <select value={cls} onChange={(e) => setCls(e.target.value)} aria-label="Class">
            <option value="">Every class ({doc.entities.length})</option>
            {classes.map(([c, n]) => (
              <option key={c} value={c}>
                {c} ({n})
              </option>
            ))}
          </select>
        </div>
        <div className="editor-report">
          <table>
            <thead>
              <tr>
                <th aria-label="Pick" />
                <th>Class</th>
                <th>Name</th>
                <th>Where</th>
              </tr>
            </thead>
            <tbody>
              {rows.slice(0, 500).map((e) => (
                <tr key={e.id} onDoubleClick={() => look(e)}>
                  <td>
                    <input type="checkbox" checked={picked.has(e.id)} onChange={() => toggle(e.id)} aria-label={`Pick ${e.classname}`} />
                  </td>
                  <td title={fgd?.get(e.classname)?.description}>{e.classname}</td>
                  <td className="mono">{keyOf(e.keys, "targetname") ?? ""}</td>
                  <td className="mono">{where(e)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          {rows.length > 500 && <p className="small-note muted">Showing 500 of {rows.length}. Narrow the search to see the rest.</p>}
          {!rows.length && <p className="small-note muted">Nothing matches.</p>}
        </div>
        <p className="small-note muted">Double-click a row to go to it.</p>
        <div className="row ask-buttons">
          <button type="button" className="btn" onClick={() => setPicked(new Set(rows.map((e) => e.id)))} disabled={!rows.length}>
            Tick all shown
          </button>
          <button
            type="button"
            className="btn danger"
            disabled={!picked.size}
            onClick={() => void s.cmd({ op: "delete", entities: [...picked] }).then((r) => !r.error && setPicked(new Set()))}
          >
            Delete {picked.size || ""}
          </button>
          <button
            type="button"
            className="btn primary"
            disabled={!picked.size}
            onClick={() => {
              s.select({ brushes: [], entities: [...picked], face: null });
              onClose();
            }}
          >
            Pick {picked.size || ""}
          </button>
        </div>
      </div>
    </Sheet>
  );
}
