// Waves & spawn groups: which creatures each wave of a mode brings (waves.ini), with a bar per
// line showing the mix, and "Place in map" to put wave spawners down in the map editor.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { useNav, useDirty } from "../../../store";
import { Banner, BusyButton, Info, LoadError, useToast } from "../../../ui";
import { Loading, Row, Rows } from "../common";
import { errorText, useBusy } from "../common/api";
import { saveWaves, wavesStatus, type WaveRow, type WaveSet } from "./creaturesApi";

const COLOURS = ["#c9b458", "#8c9a6a", "#d9774b", "#7aa6c2", "#b07cc6", "#6fbf8e", "#e0a85a", "#a0a0a0"];
const clone = <T,>(v: T): T => JSON.parse(JSON.stringify(v));

function rowLabel(r: WaveRow) {
  return r.to == null ? `Wave ${r.from} and after` : r.to === r.from ? `Wave ${r.from}` : `Waves ${r.from} to ${r.to}`;
}

/** Problems a person can fix here (the server checks again on save). */
function localProblems(w: WaveSet, types: string[]): string[] {
  const out: string[] = [];
  const rows = [...w.rows].sort((a, b) => a.from - b.from);
  rows.forEach((r, i) => {
    if (r.from < 1 || (r.to != null && r.to < r.from)) out.push(`${rowLabel(r)}: waves start at 1, and the last can't come before the first.`);
    const next = rows[i + 1];
    if (next && (r.to == null || r.to >= next.from)) out.push(`${rowLabel(r)} overlaps the line for wave ${next.from}.`);
    if (!r.mix.some(([, n]) => n > 0)) out.push(`${rowLabel(r)}: give at least one type a weight.`);
    for (const [t, n] of r.mix) if (n > 0 && !types.includes(t)) out.push(`${rowLabel(r)}: there's no type called ${t}.`);
  });
  if (!w.rows.length) out.push("Add at least one wave.");
  if (w.boss && !types.includes(w.boss)) out.push(`There's no type called ${w.boss} for the boss.`);
  return out;
}

export function WavesTab() {
  const toast = useToast();
  const qc = useQueryClient();
  const go = useNav((s) => s.go);
  const { working, run } = useBusy();
  const st = useQuery({ queryKey: ["tools", "creatures", "waves"], queryFn: ({ signal }) => wavesStatus(signal) });
  const [mode, setMode] = useState("laststand");
  const [other, setOther] = useState("");
  const [w, setW] = useState<WaveSet | null>(null);
  const [saved, setSaved] = useState<WaveSet | null>(null);

  const modeId = mode === "\u0000other" ? other.trim() : mode;
  useEffect(() => {
    if (!st.data || !modeId) return;
    const have = st.data.sets.find((s) => s.mode === modeId);
    const start = have ?? (st.data.defaults[modeId] ? { ...st.data.defaults[modeId] } : { mode: modeId, boss: "", boss_every: 0, count_scale: 1, hp_scale: 1, type_hp: false, rows: [{ from: 1, to: null, mix: [["zombie", 10]] }] });
    setW(clone(start));
    setSaved(have ? clone(have) : null);
  }, [st.data, modeId]);

  const dirty = !!w && JSON.stringify(w) !== JSON.stringify(saved ?? st.data?.defaults[w.mode] ?? null);
  useDirty("creatures-waves", dirty);
  const types = st.data?.types ?? [];
  const used = useMemo(() => {
    const s = new Set<string>();
    w?.rows.forEach((r) => r.mix.forEach(([t]) => s.add(t)));
    return [...s];
  }, [w]);
  const cols = used.length ? used : types.slice(0, 3);

  if (st.isPending) return <Loading text="Reading the waves…" />;
  if (st.isError) return <LoadError text={errorText(st.error, "Couldn't read the waves.")} onRetry={() => st.refetch()} />;
  if (!w) return null;
  const info = st.data.modes.find((m) => m.id === w.mode);
  const problems = localProblems(w, types);

  const setRow = (i: number, r: WaveRow) => setW({ ...w, rows: w.rows.map((x, j) => (j === i ? r : x)) });
  const weight = (r: WaveRow, t: string) => r.mix.find(([x]) => x === t)?.[1] ?? 0;
  const setWeight = (i: number, t: string, n: number) => {
    const r = w.rows[i];
    const mix: [string, number][] = r.mix.some(([x]) => x === t) ? r.mix.map(([x, v]) => [x, x === t ? n : v]) : [...r.mix, [t, n]];
    setRow(i, { ...r, mix });
  };
  const addType = (t: string) => t && setW({ ...w, rows: w.rows.map((r, i) => (i === 0 && !r.mix.some(([x]) => x === t) ? { ...r, mix: [...r.mix, [t, 0]] } : r)) });
  const addRow = () => {
    const sorted = [...w.rows].sort((a, b) => a.from - b.from);
    const last = sorted[sorted.length - 1];
    const from = last ? (last.to ?? last.from) + 1 : 1;
    const rows = w.rows.map((r) => (r === last && r.to == null ? { ...r, to: r.from } : r));
    setW({ ...w, rows: [...rows, { from, to: null, mix: clone(last?.mix ?? [["zombie", 10]]) }] });
  };
  const save = () =>
    run("Saving…", async () => {
      const r = await saveWaves(w);
      if (r.error) return toast(r.error, true);
      toast(r.message || "Saved.");
      if (r.status) qc.setQueryData(["tools", "creatures", "waves"], r.status);
    });

  return (
    <div className="cs-waves">
      {info && !info.reads && <Banner>{info.note} Saving now means it's ready the moment it does.</Banner>}
      <div className="panel">
        <Rows bare>
          <Row name="Mode" note={info ? info.name : "A mode that reads waves.ini by its id"}>
            <span className="row">
              <select value={mode} aria-label="Mode" onChange={(e) => setMode(e.target.value)}>
                {st.data.modes.map((m) => (
                  <option key={m.id} value={m.id}>
                    {m.name}
                  </option>
                ))}
                {st.data.sets.filter((s) => !st.data.modes.some((m) => m.id === s.mode)).map((s) => (
                  <option key={s.mode} value={s.mode}>
                    {s.mode}
                  </option>
                ))}
                <option value={"\u0000other"}>Another mode…</option>
              </select>
              {mode === "\u0000other" && <input className="search" placeholder="mode id, e.g. my_mode" value={other} aria-label="Mode id" onChange={(e) => setOther(e.target.value.toLowerCase().replace(/[^a-z0-9_]/g, ""))} />}
            </span>
          </Row>
          <Row name="Boss" note={w.boss ? `A ${w.boss} leads every ${w.boss_every || "—"}th wave` : "No boss waves"}>
            <span className="row">
              <select value={w.boss} aria-label="Boss type" onChange={(e) => setW({ ...w, boss: e.target.value })}>
                <option value="">None</option>
                {types.map((t) => (
                  <option key={t} value={t}>
                    {t}
                  </option>
                ))}
              </select>
              {w.boss && (
                <label className="cs-inline">
                  every <input className="search cs-numbox" type="number" min={0} max={100} value={w.boss_every} aria-label="Boss every so many waves" onChange={(e) => setW({ ...w, boss_every: Math.max(0, Math.round(Number(e.target.value) || 0)) })} /> waves
                </label>
              )}
            </span>
          </Row>
          <Row name="How many" note="Times the mode's own number of enemies a wave">
            <input className="search cs-numbox" type="number" min={0.1} max={10} step={0.1} value={w.count_scale} aria-label="Enemy count scale" onChange={(e) => setW({ ...w, count_scale: Number(e.target.value) })} />
          </Row>
          <Row name="Health" note={w.type_hp ? "Each enemy has its type's health (from the Types tab), times this" : "Times the mode's own health for each enemy"} info="Last Stand gives its enemies its own health table (zombie 100, runner 60, brute 450, spitter 120, boss 3000), growing each wave. Tick the box to use the types' health from this Studio instead.">
            <span className="row">
              <input className="search cs-numbox" type="number" min={0.1} max={10} step={0.1} value={w.hp_scale} aria-label="Health scale" onChange={(e) => setW({ ...w, hp_scale: Number(e.target.value) })} />
              <label className="tl-check">
                <input type="checkbox" checked={w.type_hp} onChange={(e) => setW({ ...w, type_hp: e.target.checked })} /> Use each type's own health
              </label>
            </span>
          </Row>
        </Rows>
      </div>

      <div className="panel cs-wave-table-wrap">
        <h4 className="cs-sub">
          The mix <Info text="Each enemy a wave brings is one of these types, picked by weight: with zombie 10 and runner 5, two in three are zombies. A wave with no line of its own uses the line before it." />
        </h4>
        <div className="cs-wave-table" role="table" aria-label="Wave mixes">
          <div className="cs-wave-head" role="row" style={{ gridTemplateColumns: `minmax(150px, 1.2fr) repeat(${cols.length}, minmax(64px, 90px)) minmax(120px, 1fr) 36px` }}>
            <span role="columnheader">Waves</span>
            {cols.map((t, i) => (
              <span key={t} role="columnheader" className="cs-col">
                <span className="cs-swatch" style={{ background: COLOURS[i % COLOURS.length] }} />
                {t}
              </span>
            ))}
            <span role="columnheader">Mix</span>
            <span />
          </div>
          {w.rows.map((r, i) => {
            const total = cols.reduce((n, t) => n + weight(r, t), 0) || 1;
            return (
              <div key={i} className="cs-wave-row" role="row" style={{ gridTemplateColumns: `minmax(150px, 1.2fr) repeat(${cols.length}, minmax(64px, 90px)) minmax(120px, 1fr) 36px` }}>
                <span className="cs-wave-range" role="cell">
                  <input className="search cs-numbox" type="number" min={1} value={r.from} aria-label="From wave" onChange={(e) => setRow(i, { ...r, from: Math.max(1, Math.round(Number(e.target.value) || 1)) })} />
                  {r.to == null ? (
                    <button type="button" className="btn small" title="Make this line end at a wave" onClick={() => setRow(i, { ...r, to: r.from })}>
                      and after
                    </button>
                  ) : (
                    <>
                      to <input className="search cs-numbox" type="number" min={r.from} value={r.to} aria-label="To wave" onChange={(e) => setRow(i, { ...r, to: Math.max(1, Math.round(Number(e.target.value) || 1)) })} />
                      <button type="button" className="btn small" title="This line goes on for every wave after" onClick={() => setRow(i, { ...r, to: null })}>
                        +
                      </button>
                    </>
                  )}
                </span>
                {cols.map((t) => (
                  <input key={t} className="search cs-numbox" role="cell" type="number" min={0} max={1000} value={weight(r, t)} aria-label={`${rowLabel(r)}: ${t}`} onChange={(e) => setWeight(i, t, Math.max(0, Math.min(1000, Math.round(Number(e.target.value) || 0))))} />
                ))}
                <span className="cs-bar" role="cell" title={cols.map((t) => `${t} ${Math.round((weight(r, t) / total) * 100)}%`).join(", ")}>
                  {cols.map((t, k) => (weight(r, t) > 0 ? <span key={t} style={{ flexGrow: weight(r, t), background: COLOURS[k % COLOURS.length] }} /> : null))}
                </span>
                <button type="button" className="btn small" aria-label={`Remove ${rowLabel(r)}`} disabled={w.rows.length <= 1} onClick={() => setW({ ...w, rows: w.rows.filter((_, j) => j !== i) })}>
                  ×
                </button>
              </div>
            );
          })}
        </div>
        <div className="row cs-wave-actions">
          <button type="button" className="btn small" onClick={addRow}>
            Add a line
          </button>
          <select value="" aria-label="Add a type to the mix" onChange={(e) => addType(e.target.value)}>
            <option value="">Add a type…</option>
            {types.filter((t) => !cols.includes(t)).map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </select>
          {st.data.defaults[w.mode] && (
            <button type="button" className="btn small" onClick={() => setW(clone(st.data.defaults[w.mode]))}>
              Start from the mode's own mix
            </button>
          )}
        </div>
        {problems.length > 0 && (
          <ul className="cs-problems">
            {problems.map((p) => (
              <li key={p} className="cs-error">
                {p}
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="panel">
        <h4 className="cs-sub">Where they come from</h4>
        <p className="muted cs-hint">
          Waves come out of wave spawners placed on the map. Last Stand puts its own on the stock maps it knows (and one at the Terrorist spawn on any other); place your own in the map editor to choose
          where, how far apart and how fast they come.
        </p>
        <button type="button" className="btn" onClick={() => go("tools", "editor:components")}>
          Place in map…
        </button>
      </div>

      <div className="panel cs-save">
        <div className="row between cs-save-row">
          <div>
            <strong>{dirty ? "You have changes to save" : saved ? "Saved" : "Not saved yet: the mode uses its own mix"}</strong>
            <p className="muted">In {st.data.file}</p>
          </div>
          <div className="row">
            {dirty && (
              <button type="button" className="btn" disabled={!!working} onClick={() => setW(clone(saved ?? st.data.defaults[w.mode] ?? w))}>
                Undo changes
              </button>
            )}
            <BusyButton className="btn primary" busy={working === "Saving…"} disabled={!!working || problems.length > 0 || (!dirty && !!saved) || !w.mode} label="Save waves" busyLabel="Saving…" onClick={save} />
          </div>
        </div>
      </div>
    </div>
  );
}
