// P5: the Budget panel: how close the map is to GoldSrc's limits while you build
// (tools/editor/budget.rs). The last compile's own numbers (planes, clipnodes, faces, leaves,
// marksurfaces, lighting…), live counts that need no compile (brush entities, entities, an edicts
// estimate, textures), and the precache budget (512 models, 512 sounds): what the map's entities
// load, the game's own baseline, and what the modes the map is made for add (their creature
// roster, from the creature types). Green, yellow over 75%, red over 90%. Click a bar for the
// biggest contributors, the usual fix, and Pick to select them in the map. `budgetWarnings()`
// is asked before Playtest and Publish.

import { useEffect, useState } from "react";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";
import { kitFor, madeForOf, useKits, loadKits } from "./kits";
import { originOf, selectionBounds, useEditor } from "./store";
import "./p5.css";

export type Contributor = { label: string; entities: number[]; brushes: number[]; share?: number; cost?: number; texture?: string; kind?: string };
export type Bar = { id: string; name: string; used: number; max: number; pct: number; what: string; reserve?: number; contributors?: { fix: string; rows: Contributor[]; basis?: string } };
type Report = {
  map: string;
  /** the compile this document ran last (`map`: the name it was compiled as) */
  compile: { rows: Bar[]; when: number; log: string; map?: string } | null;
  live: Bar[];
  precache: { models: number; inlineModels: number; sounds: number; modelRows: Contributor[]; soundRows: Contributor[] };
  error?: string;
};
type Creature = { name: string; display?: string; model?: string; snd?: string[] };

/** The game, a map and Loner-Strike's plugins before creatures, as Y measured them (ls_outpost,
 *  Last Stand, bots: docs/notes/Y.md, "precache budget guard"): about 197 models and 315 sounds.
 *  Stock maps differ; this is a Loner-Strike map with its plugins loaded. */
export const BASELINE = { models: 197, sounds: 315 };
/** The slots the creature guard keeps free (`ls_creature_reserve "40 60"`). */
export const RESERVE = { models: 40, sounds: 60 };

/** Y's measured cost per creature type with the default `ls_creature_sounds 2` (new sounds after
 *  what the game or another type already loaded; the models it adds). */
const Y_TYPES: Record<string, { models: string[]; sounds: number }> = {
  zombie: { models: ["zombie.mdl"], sounds: 12 },
  headcrab: { models: ["headcrab.mdl"], sounds: 11 },
  runner: { models: ["houndeye.mdl"], sounds: 10 },
  babycrab: { models: ["baby_headcrab.mdl"], sounds: 0 },
  spitter: { models: ["bullsquid.mdl"], sounds: 12 },
  vortigaunt: { models: ["islave.mdl", "sprites/lgtning.spr"], sounds: 10 },
  brute: { models: ["agrunt.mdl"], sounds: 10 },
  snark: { models: ["w_squeak.mdl"], sounds: 5 },
  assassin: { models: ["hassassin.mdl"], sounds: 2 },
  soldier: { models: ["hgrunt.mdl"], sounds: 6 },
  babygarg: { models: ["garg.mdl"], sounds: 10 },
  gonarch: { models: ["big_mom.mdl"], sounds: 10 },
  boss: { models: ["garg.mdl"], sounds: 1 },
};

export async function getBudget(): Promise<Report | null> {
  try {
    const r = (await (await fetch("/api/tools/editor/budget", { headers: { "X-Panel": "1" } })).json()) as Report;
    return r.error ? null : r;
  } catch {
    return null;
  }
}

let creatureCache: Promise<Creature[]> | null = null;
function creatures(): Promise<Creature[]> {
  creatureCache ??= fetch("/api/tools/creatures", { headers: { "X-Panel": "1" } })
    .then((r) => r.json())
    .then((r: { types?: Creature[]; templates?: Creature[] }) => {
      const m = new Map<string, Creature>();
      for (const t of [...(r.templates ?? []), ...(r.types ?? [])]) if (t?.name) m.set(t.name, t);
      return [...m.values()];
    })
    .catch(() => ((creatureCache = null), []));
  return creatureCache;
}

/** What the modes the map is made for precache for their creatures: Y's measured table for the
 *  types it lists; another type (yours, a community one) counted from its keys like the guard
 *  does (its model, and 2 sounds per kind). */
export async function modeLoad(): Promise<{ label: string; models: number; sounds: number; roster: string[] } | null> {
  const doc = useEditor.getState().doc;
  const made = madeForOf(doc?.world);
  if (!made.length) return null;
  if (!useKits.getState().loaded) await loadKits();
  const kits = made.map((m) => kitFor(m)).filter((k): k is NonNullable<typeof k> => !!k);
  const names = [...new Set(kits.flatMap((k) => k.creatures))];
  const label = kits.length ? kits.map((k) => k.mode.name).join(" + ") : made.map((m) => m.id).join(" + ");
  if (!names.length) return { label, models: 0, sounds: 0, roster: [] };
  const models = new Set<string>();
  let soundCount = 0;
  const others = new Set<string>();
  const unknown = names.filter((n) => !Y_TYPES[n]);
  for (const n of names) {
    const y = Y_TYPES[n];
    if (!y) continue;
    y.models.forEach((m) => models.add(m));
    soundCount += y.sounds;
  }
  if (unknown.length) {
    const all = await creatures();
    for (const n of unknown) {
      const t = all.find((c) => c.name === n);
      if (!t) continue;
      if (t.model) models.add(t.model.toLowerCase());
      // the guard's default: 2 sounds per kind
      for (const kind of t.snd ?? []) for (const w of kind.split(/[\s|]+/).filter((x) => /\.wav$/i.test(x)).slice(0, 2)) others.add(w.toLowerCase());
    }
  }
  return { label, models: models.size, sounds: soundCount + others.size, roster: names };
}

const tone = (pct: number) => (pct > 90 ? "red" : pct > 75 ? "yellow" : "green");

/** The bars over 90% (compile, live and precache with the baseline and the mode), in words. */
export async function budgetWarnings(): Promise<string[]> {
  const r = await getBudget();
  if (!r) return [];
  const out: string[] = [];
  for (const b of [...(r.compile?.rows ?? []), ...r.live]) if (b.pct > 90) out.push(`${b.name}: ${b.used} of ${b.max} (${Math.round(b.pct)}%)`);
  const mode = await modeLoad();
  const pm = BASELINE.models + r.precache.models + (mode?.models ?? 0);
  const ps = BASELINE.sounds + r.precache.sounds + (mode?.sounds ?? 0);
  if (pm / 512 > 0.9) out.push(`Precached models: about ${pm} of 512`);
  if (ps / 512 > 0.9) out.push(`Precached sounds: about ${ps} of 512`);
  return out;
}

function pick(c: Contributor) {
  const s = useEditor.getState();
  s.select({ brushes: c.brushes, entities: c.entities, face: null });
  const b = selectionBounds();
  if (b) useTools.getState().look(b[0].map((x, i) => (x + b[1][i]) / 2) as Vec3, Math.max(96, ...b[1].map((x, i) => x - b[0][i])));
  else if (c.entities.length) {
    const e = s.doc?.entities.find((x) => x.id === c.entities[0]);
    const o = e && originOf(e.keys);
    if (o) useTools.getState().look(o, 96);
  }
}

function BarRow({ b, open, onToggle }: { b: Bar; open: boolean; onToggle: () => void }) {
  const t = tone(b.pct);
  return (
    <li className={`p5-bud ${t}`}>
      <button type="button" className="p5-bud-head" aria-expanded={open} onClick={onToggle} title={`${b.what}. Click for what uses it.`}>
        <span className="p5-bud-name">{b.name}</span>
        <span className="p5-bud-num mono">
          {b.used.toLocaleString("en-US")} / {b.max.toLocaleString("en-US")}
        </span>
        <span className="p5-bud-track" role="meter" aria-valuemin={0} aria-valuemax={b.max} aria-valuenow={b.used} aria-label={`${b.name}: ${Math.round(b.pct)}%`}>
          <span className="p5-bud-fill" style={{ width: `${Math.min(100, b.pct)}%` }} />
          {b.reserve ? <span className="p5-bud-reserve" style={{ width: `${(b.reserve / b.max) * 100}%` }} title={`${b.reserve} slots the creature guard keeps free (ls_creature_reserve)`} /> : null}
        </span>
        <span className="p5-bud-pct mono">{Math.round(b.pct)}%</span>
      </button>
      {open && b.contributors && (
        <div className="p5-bud-more">
          {b.contributors.fix && <p className="small-note">{b.contributors.fix}</p>}
          {b.contributors.basis && <p className="small-note muted">Shares are {b.contributors.basis}.</p>}
          {b.contributors.rows.length ? (
            <ul>
              {b.contributors.rows.map((c, i) => (
                <li key={i}>
                  <span className="p5-bud-clabel">{c.label}</span>
                  {c.share != null && <span className="muted mono">{c.share}%</span>}
                  {c.entities.length + c.brushes.length > 0 && (
                    <button type="button" className="btn small" onClick={() => pick(c)} title="Select it in the map and look at it">
                      Pick
                    </button>
                  )}
                </li>
              ))}
            </ul>
          ) : (
            <p className="small-note muted">Nothing in particular.</p>
          )}
        </div>
      )}
    </li>
  );
}

export function BudgetPanel() {
  const revision = useEditor((s) => s.doc?.revision);
  const world = useEditor((s) => s.doc?.world);
  const [r, setR] = useState<Report | null>(null);
  const [mode, setMode] = useState<Awaited<ReturnType<typeof modeLoad>>>(null);
  const [busy, setBusy] = useState(false);
  const [open, setOpen] = useState<string | null>(null);
  const [n, setN] = useState(0);
  // after edits settle (and on Refresh: a compile that just finished)
  useEffect(() => {
    let live = true;
    const id = setTimeout(async () => {
      setBusy(true);
      const [b, m] = await Promise.all([getBudget(), modeLoad()]);
      if (!live) return;
      setR(b);
      setMode(m);
      setBusy(false);
    }, r ? 700 : 0);
    return () => {
      live = false;
      clearTimeout(id);
    };
  }, [revision, world, n]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!r) {
    return (
      <section className="tools-panel p5-budget">
        <p className="small-note muted" role="status">
          <span className="spinner" aria-hidden="true" /> Counting…
        </p>
      </section>
    );
  }
  const toggle = (id: string) => setOpen(open === id ? null : id);
  const pm = BASELINE.models + r.precache.models + (mode?.models ?? 0);
  const ps = BASELINE.sounds + r.precache.sounds + (mode?.sounds ?? 0);
  const pre: Bar[] = [
    {
      id: "pre_models",
      name: "Precached models",
      used: pm,
      max: 512,
      pct: (pm / 512) * 100,
      reserve: RESERVE.models,
      what: `the game, map and plugins before creatures (about ${BASELINE.models}, measured by Y), this map's ${r.precache.models} (models, sprites and ${r.precache.inlineModels} brush entities)${mode ? `, and ${mode.label}'s ${mode.models}` : ""}; the striped end is the guard's reserve of ${RESERVE.models}`,
      contributors: { fix: "Use fewer different models and sprites; each brush entity counts too (func_detail doesn't).", rows: r.precache.modelRows.map((c) => ({ ...c, label: `${c.label}${c.entities.length ? ` (${c.entities.length}×)` : ""}` })) },
    },
    {
      id: "pre_sounds",
      name: "Precached sounds",
      used: ps,
      max: 512,
      pct: (ps / 512) * 100,
      reserve: RESERVE.sounds,
      what: `the game, map and plugins before creatures (about ${BASELINE.sounds}, measured by Y), this map's ${r.precache.sounds}${mode ? `, and ${mode.label}'s ${mode.sounds} (2 sounds per kind)` : ""}; the striped end is the guard's reserve of ${RESERVE.sounds}`,
      contributors: { fix: "Reuse the same sounds on doors, buttons and ambient_generics; a creature type brings all its sounds.", rows: r.precache.soundRows.map((c) => ({ ...c, label: `${c.label}${c.entities.length ? ` (${c.entities.length}×)` : ""}` })) },
    },
  ];
  const red = [...(r.compile?.rows ?? []), ...r.live, ...pre].filter((b) => b.pct > 90).length;
  return (
    <section className="tools-panel p5-budget" aria-label="Budget">
      <div className="row">
        {red > 0 ? <strong className="p5-bud-alert">{red} over 90%: Playtest and Publish will ask first.</strong> : <span className="small-note muted">Green: room to spare. Yellow over 75%, red over 90%.</span>}
        <span className="push" />
        {busy && <span className="spinner" aria-label="Counting" />}
        <button type="button" className="btn small" disabled={busy} onClick={() => setN((x) => x + 1)} title="Count again (and read the last compile)">
          Refresh
        </button>
      </div>

      <h4>Precache</h4>
      <p className="small-note">
        {mode ? (
          <>
            {mode.label}
            {mode.roster.length ? ` with this roster (${mode.roster.join(", ")})` : ""}: <b>{Math.max(0, 512 - RESERVE.sounds - ps)} sounds</b> and <b>{Math.max(0, 512 - RESERVE.models - pm)} models</b> of headroom (after the guard's reserve).
          </>
        ) : (
          <>
            <b>{Math.max(0, 512 - RESERVE.sounds - ps)} sounds</b> and <b>{Math.max(0, 512 - RESERVE.models - pm)} models</b> of headroom after the guard's reserve (no mode chosen: Gameplay › Mode kits › Made for adds its creatures).
          </>
        )}{" "}
        <span className="muted">The game's share is Y's measurement on a Loner-Strike map; stock maps differ.</span>
      </p>
      <ul className="p5-bud-list">
        {pre.map((b) => (
          <BarRow key={b.id} b={b} open={open === b.id} onToggle={() => toggle(b.id)} />
        ))}
      </ul>

      <h4>As you build</h4>
      <ul className="p5-bud-list">
        {r.live.map((b) => (
          <BarRow key={b.id} b={b} open={open === b.id} onToggle={() => toggle(b.id)} />
        ))}
      </ul>

      <h4>Last compile</h4>
      {r.compile ? (
        <>
          <p className="small-note muted">
            {r.compile.map ?? r.map}, {new Date(r.compile.when * 1000).toLocaleString("en-US")}. Compile again to update it.
          </p>
          <ul className="p5-bud-list">
            {r.compile.rows.map((b) => (
              <BarRow key={b.id} b={b} open={open === b.id} onToggle={() => toggle(b.id)} />
            ))}
          </ul>
        </>
      ) : (
        <p className="small-note muted">This map hasn't been compiled yet: planes, clipnodes, faces, leaves and lighting show here after a compile (Compile panel, or F5 Playtest).</p>
      )}
    </section>
  );
}
