// The Timeline editor (a tab of its own, opened by double-clicking a Timeline node or from My
// mode): its settings, and tracks as lanes of keys over time. Number and position tracks are
// curves (a position's x, y and z in three colors), a color track a gradient, an event track
// diamonds. Click a lane to add a key, drag a key to move it (Shift: one axis only), Delete
// removes the selected one, right-click a key for Linear or Ease, the wheel zooms time, dragging
// empty space pans, F fits the whole length.

import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as RPointerEvent } from "react";
import { useStudio } from "./store";
import { newTrack, removeTimeline, updateTimeline, type GKey, type GTimeline, type GTrack } from "./graphOps";
import { useContextMenu } from "./ContextMenu";
import { useConfirm } from "../../../ui";

const HEAD = 170;
const LANE: Record<GTrack["kind"], number> = { float: 96, vector: 96, color: 44, event: 36 };
const COMP = ["#e05a4f", "#5fbf6a", "#5a8fe0"];
const KIND_NAME: Record<GTrack["kind"], string> = { float: "Number", vector: "Position", color: "Color", event: "Event" };

const ease = (x: number) => x * x * (3 - 2 * x);
const num = (v: unknown, c?: number): number => (c === undefined ? (typeof v === "number" ? v : 0) : Array.isArray(v) && typeof v[c] === "number" ? v[c] : 0);
const sorted = (keys: GKey[]) => [...keys].sort((a, b) => a.t - b.t);

/** A track's value at time t (one component of a position or color), as the plugin works it out. */
export function valueAt(keys: GKey[], t: number, c?: number): number {
  const k = sorted(keys);
  if (!k.length) return 0;
  if (t <= k[0].t) return num(k[0].v, c);
  for (let i = 0; i < k.length - 1; i++) {
    const a = k[i],
      b = k[i + 1];
    if (t <= b.t) {
      const x = (t - a.t) / Math.max(b.t - a.t, 0.0001);
      const f = a.ease === "ease" ? ease(x) : x;
      return num(a.v, c) + (num(b.v, c) - num(a.v, c)) * f;
    }
  }
  return num(k[k.length - 1].v, c);
}

const comps = (tr: GTrack) => (tr.kind === "vector" ? [0, 1, 2] : tr.kind === "float" ? [undefined] : []);

/** The values a lane shows, low to high (with some room). */
function range(tr: GTrack): [number, number] {
  const vals = tr.keys.flatMap((k) => comps(tr).map((c) => num(k.v, c)));
  if (!vals.length) return [0, 1];
  let lo = Math.min(...vals),
    hi = Math.max(...vals);
  if (hi - lo < 1e-6) (lo -= 1), (hi += 1);
  const pad = (hi - lo) * 0.12;
  return [lo - pad, hi + pad];
}

type Sel = { track: string; key: number } | null;

export function TimelineEditor({ id }: { id: string }) {
  const graph = useStudio((s) => s.graph);
  const tl = graph?.timelines?.find((t) => t.id === id);
  const { edit, live, commit, closeTab, openTab } = useStudio.getState();
  const ask = useConfirm();
  const cm = useContextMenu();
  const box = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(800);
  const [view, setView] = useState<{ pps: number; at: number } | null>(null);
  const [sel, setSel] = useState<Sel>(null);
  const laneW = Math.max(200, width - HEAD - 16);
  const length = tl ? (tl.useLastKey ? Math.max(0.1, ...tl.tracks.flatMap((t) => t.keys.map((k) => k.t))) : tl.length) : 1;
  const fit = useCallback(() => setView({ pps: laneW / Math.max(tl?.length ?? 1, 0.1), at: 0 }), [laneW, tl?.length]);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setWidth(el.clientWidth));
    ro.observe(el);
    setWidth(el.clientWidth);
    return () => ro.disconnect();
  }, []);
  useEffect(() => {
    if (!view && width > 0) fit();
  }, [view, width, fit]);
  const v = view ?? { pps: laneW / Math.max(tl?.length ?? 1, 0.1), at: 0 };
  const xOf = (t: number) => (t - v.at) * v.pps;
  const tOf = (x: number) => x / v.pps + v.at;
  const set = (f: (t: GTimeline) => GTimeline) => edit((g) => updateTimeline(g, id, f));
  const setTrack = (tid: string, f: (t: GTrack) => GTrack) => set((t) => ({ ...t, tracks: t.tracks.map((x) => (x.id === tid ? f(x) : x)) }));
  const ticks = useMemo(() => {
    const step = [0.05, 0.1, 0.25, 0.5, 1, 2, 5, 10, 30, 60].find((s) => s * v.pps >= 60) ?? 60;
    const out: number[] = [];
    for (let t = Math.ceil(v.at / step) * step; t <= tOf(laneW) + 1e-9; t += step) out.push(Math.round(t * 1000) / 1000);
    return out;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [v.pps, v.at, laneW]);

  if (!graph || !tl) return <div className="st-tl-empty">That Timeline isn't there any more.</div>;
  const selKey = sel && tl.tracks.find((t) => t.id === sel.track)?.keys[sel.key];
  const selTrack = sel && tl.tracks.find((t) => t.id === sel.track);

  const removeSel = () => {
    if (!sel) return;
    setTrack(sel.track, (tr) => ({ ...tr, keys: tr.keys.filter((_, i) => i !== sel.key) }));
    setSel(null);
  };

  // a press on a lane: on a key, drag it; on empty space, pan (or add a key when it didn't move)
  const press = (e: RPointerEvent<Element>, tr: GTrack, keyIndex: number | null, comp?: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    box.current?.focus({ preventScroll: true });
    // (the lane's box, whatever was pressed in it)
    const lane = (e.currentTarget as Element).closest("svg.st-tl-lane") ?? (e.currentTarget as Element);
    const svg = lane.getBoundingClientRect();
    const h = LANE[tr.kind];
    const [lo, hi] = range(tr);
    const yToV = (y: number) => hi - (y / h) * (hi - lo);
    const start = { x: e.clientX, y: e.clientY };
    const before = useStudio.getState().graph!;
    let moved = false;
    if (keyIndex !== null) setSel({ track: tr.id, key: keyIndex });
    const orig = keyIndex !== null ? tr.keys[keyIndex] : null;
    const startAt = v.at;
    const move = (ev: PointerEvent) => {
      const dx = ev.clientX - start.x,
        dy = ev.clientY - start.y;
      if (!moved && Math.hypot(dx, dy) < 4) return;
      moved = true;
      if (!orig) {
        setView({ pps: v.pps, at: Math.max(-1, startAt - dx / v.pps) });
        return;
      }
      const lockT = ev.shiftKey && Math.abs(dy) > Math.abs(dx);
      const lockV = ev.shiftKey && !lockT;
      const t = lockT ? orig.t : Math.min(tl.length, Math.max(0, Math.round((orig.t + dx / v.pps) * 100) / 100));
      let val = orig.v;
      if (!lockV && (tr.kind === "float" || tr.kind === "vector")) {
        const nv = Math.round(yToV(ev.clientY - svg.top) * 100) / 100;
        val = tr.kind === "float" ? nv : (orig.v as number[]).map((x, c) => (c === comp ? nv : x));
      }
      live((g) => updateTimeline(g, id, (x) => ({ ...x, tracks: x.tracks.map((y) => (y.id === tr.id ? { ...y, keys: y.keys.map((k, i) => (i === keyIndex ? { ...k, t, v: val } : k)) } : y)) })));
    };
    const up = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      if (orig) return commit(before);
      if (moved) return;
      // a click on empty lane: a key there (a number at the height clicked; others as they are there)
      const t = Math.min(tl.length, Math.max(0, Math.round(tOf(ev.clientX - svg.left) * 100) / 100));
      const val =
        tr.kind === "float"
          ? Math.round(yToV(ev.clientY - svg.top) * 100) / 100
          : tr.kind === "vector"
            ? [0, 1, 2].map((c) => Math.round(valueAt(tr.keys, t, c) * 100) / 100)
            : tr.kind === "color"
              ? tr.keys.length
                ? [0, 1, 2].map((c) => Math.round(valueAt(tr.keys, t, c)))
                : [255, 255, 255]
              : undefined;
      const key: GKey = val === undefined ? { t } : { t, v: val };
      edit((g) => updateTimeline(g, id, (x) => ({ ...x, tracks: x.tracks.map((y) => (y.id === tr.id ? { ...y, keys: [...y.keys, key] } : y)) })));
      setSel({ track: tr.id, key: tr.keys.length });
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const keyMenu = (e: React.MouseEvent, tr: GTrack, i: number) =>
    cm.open(e, `${tr.name}: key at ${tr.keys[i].t} s`, [
      { label: "Linear", run: () => setTrack(tr.id, (x) => ({ ...x, keys: x.keys.map((k, j) => (j === i ? { ...k, ease: undefined } : k)) })) },
      { label: "Ease", run: () => setTrack(tr.id, (x) => ({ ...x, keys: x.keys.map((k, j) => (j === i ? { ...k, ease: "ease" as const } : k)) })) },
      "-",
      { label: "Delete key", danger: true, run: () => (setTrack(tr.id, (x) => ({ ...x, keys: x.keys.filter((_, j) => j !== i) })), setSel(null)) },
    ]);

  const curve = (tr: GTrack, c: number | undefined) => {
    const h = LANE[tr.kind];
    const [lo, hi] = range(tr);
    const y = (val: number) => ((hi - val) / (hi - lo)) * h;
    const pts: string[] = [];
    const n = Math.max(32, Math.round(laneW / 6));
    for (let i = 0; i <= n; i++) {
      const t = tOf((i / n) * laneW);
      pts.push(`${((i / n) * laneW).toFixed(1)},${y(valueAt(tr.keys, t, c)).toFixed(1)}`);
    }
    return <polyline points={pts.join(" ")} fill="none" stroke={c === undefined ? "var(--signal, #e8c35a)" : COMP[c]} strokeWidth={1.6} />;
  };

  return (
    <div
      className="st-tl"
      ref={box}
      tabIndex={0}
      data-testid="timeline-editor"
      onKeyDown={(e) => {
        if ((e.target as HTMLElement).closest("input, select")) return;
        if (e.key === "Delete" || e.key === "Backspace") (e.preventDefault(), removeSel());
        else if (e.key === "f" || e.key === "F") (e.preventDefault(), fit());
      }}
      onWheel={(e) => {
        const r = box.current!.getBoundingClientRect();
        const x = e.clientX - r.left - HEAD;
        if (x < 0) return;
        const t = tOf(x);
        const pps = Math.min(4000, Math.max(5, v.pps * (e.deltaY < 0 ? 1.2 : 1 / 1.2)));
        setView({ pps, at: Math.max(-1, t - x / pps) });
      }}
    >
      {cm.menu}
      <div className="st-tl-bar">
        <label>
          Length (s){" "}
          <input className="st-field st-field-num" type="number" min={0.1} max={600} step={0.1} value={tl.length} onChange={(e) => set((t) => ({ ...t, length: Math.min(600, Math.max(0.1, Number(e.target.value) || 0.1)) }))} aria-label="Length" />
        </label>
        <label>
          <input type="checkbox" checked={!!tl.loop} onChange={(e) => set((t) => ({ ...t, loop: e.target.checked }))} /> Loop
        </label>
        <label>
          <input type="checkbox" checked={!!tl.autoplay} onChange={(e) => set((t) => ({ ...t, autoplay: e.target.checked }))} /> AutoPlay
        </label>
        <label title="It ends at its last key, not at the length">
          <input type="checkbox" checked={!!tl.useLastKey} onChange={(e) => set((t) => ({ ...t, useLastKey: e.target.checked }))} /> Use Last Keyframe
        </label>
        <label title="How often the server works the values out: smaller is smoother and costs the server more">
          Tick{" "}
          <select className="st-field" value={tl.tick} onChange={(e) => set((t) => ({ ...t, tick: Number(e.target.value) }))} aria-label="Tick">
            {[0.02, 0.03, 0.05, 0.1, 0.2, 0.5].map((x) => (
              <option key={x} value={x}>
                {x} s{x === 0.05 ? " (default)" : x < 0.05 ? " (costly)" : ""}
              </option>
            ))}
          </select>
        </label>
        <span className="st-grow" />
        {(["float", "vector", "color", "event"] as const).map((k) => (
          <button key={k} type="button" className="btn small" onClick={() => set((t) => (t.tracks.length >= 8 ? t : { ...t, tracks: [...t.tracks, newTrack(t, k)] }))} disabled={tl.tracks.length >= 8}>
            + {KIND_NAME[k]} track
          </button>
        ))}
        <button type="button" className="btn small" onClick={fit} title="Fit the whole length (F)">
          Fit
        </button>
        <button
          type="button"
          className="btn small danger"
          onClick={async () => {
            const used = graph.nodes.filter((n) => n.type === "timeline.play" && n.params.timeline === id).length;
            if (!(await ask(`Remove the Timeline “${tl.name}”?${used ? ` ${used} node${used === 1 ? "" : "s"} play it and show a problem until changed.` : ""}`, { ok: "Remove", danger: true }))) return;
            closeTab(id);
            openTab("");
            edit((g) => removeTimeline(g, id));
          }}
        >
          Remove
        </button>
      </div>
      {selKey && selTrack && sel && (
        <div className="st-tl-key" data-testid="timeline-key">
          <span>
            {selTrack.name}: key
          </span>
          <label>
            Time{" "}
            <input className="st-field st-field-num" type="number" min={0} max={tl.length} step={0.01} value={selKey.t} onChange={(e) => setTrack(selTrack.id, (x) => ({ ...x, keys: x.keys.map((k, j) => (j === sel.key ? { ...k, t: Math.min(tl.length, Math.max(0, Number(e.target.value) || 0)) } : k)) }))} aria-label="Key time" />
          </label>
          {selTrack.kind === "float" && (
            <label>
              Value <input className="st-field st-field-num" type="number" step={0.1} value={num(selKey.v)} onChange={(e) => setTrack(selTrack.id, (x) => ({ ...x, keys: x.keys.map((k, j) => (j === sel.key ? { ...k, v: Number(e.target.value) || 0 } : k)) }))} aria-label="Key value" />
            </label>
          )}
          {selTrack.kind === "vector" &&
            ["X", "Y", "Z"].map((axis, c) => (
              <label key={axis}>
                {axis}{" "}
                <input
                  className="st-field st-field-num"
                  type="number"
                  step={1}
                  value={num(selKey.v, c)}
                  onChange={(e) => setTrack(selTrack.id, (x) => ({ ...x, keys: x.keys.map((k, j) => (j === sel.key ? { ...k, v: [0, 1, 2].map((q) => (q === c ? Number(e.target.value) || 0 : num(k.v, q))) } : k)) }))}
                  aria-label={`Key ${axis}`}
                />
              </label>
            ))}
          {selTrack.kind === "color" && (
            <label>
              Color{" "}
              <input
                type="color"
                value={`#${[0, 1, 2].map((c) => Math.round(num(selKey.v, c)).toString(16).padStart(2, "0")).join("")}`}
                onChange={(e) => {
                  const h = e.target.value;
                  const rgb = [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
                  setTrack(selTrack.id, (x) => ({ ...x, keys: x.keys.map((k, j) => (j === sel.key ? { ...k, v: rgb } : k)) }));
                }}
                aria-label="Key color"
              />
            </label>
          )}
          {selTrack.kind !== "event" && (
            <label>
              To the next{" "}
              <select className="st-field" value={selKey.ease ?? "linear"} onChange={(e) => setTrack(selTrack.id, (x) => ({ ...x, keys: x.keys.map((k, j) => (j === sel.key ? { ...k, ease: e.target.value === "ease" ? ("ease" as const) : undefined } : k)) }))} aria-label="Key interpolation">
                <option value="linear">Linear</option>
                <option value="ease">Ease</option>
              </select>
            </label>
          )}
          <button type="button" className="btn small" onClick={removeSel}>
            Delete key
          </button>
        </div>
      )}
      <div className="st-tl-ruler" style={{ marginLeft: HEAD }}>
        <svg width={laneW} height={20}>
          {ticks.map((t) => (
            <g key={t} transform={`translate(${xOf(t)},0)`}>
              <line y1={12} y2={20} stroke="var(--line)" />
              <text y={10} fontSize={10} fill="var(--bone-dim, #9aa)" textAnchor="middle">
                {t}s
              </text>
            </g>
          ))}
          <rect x={xOf(length)} y={0} width={2} height={20} fill="var(--st-bad, #c55)" />
        </svg>
      </div>
      <div className="st-tl-tracks">
        {tl.tracks.length === 0 && <p className="st-muted st-tl-empty">Add a track: a number (a speed, a height), a position, a color, or events at set moments.</p>}
        {tl.tracks.map((tr) => {
          const h = LANE[tr.kind];
          return (
            <div key={tr.id} className="st-tl-track" data-track={tr.id}>
              <div className="st-tl-head" style={{ width: HEAD, height: h }}>
                <input className="st-field" value={tr.name} maxLength={30} onChange={(e) => setTrack(tr.id, (x) => ({ ...x, name: e.target.value.slice(0, 30) }))} aria-label={`Track name ${tr.name}`} />
                <span className="st-muted">
                  {KIND_NAME[tr.kind]}
                  {tr.kind === "vector" && (
                    <>
                      {" "}
                      {["x", "y", "z"].map((a, c) => (
                        <span key={a} style={{ color: COMP[c] }}>
                          {a}
                        </span>
                      ))}
                    </>
                  )}
                </span>
                <button type="button" className="st-tl-x" aria-label={`Remove track ${tr.name}`} onClick={() => (set((t) => ({ ...t, tracks: t.tracks.filter((x) => x.id !== tr.id) })), setSel(null))}>
                  ×
                </button>
              </div>
              <svg className="st-tl-lane" width={laneW} height={h} onPointerDown={(e) => press(e, tr, null)} aria-label={`${tr.name} lane`}>
                <rect width={laneW} height={h} fill="transparent" />
                {ticks.map((t) => (
                  <line key={t} x1={xOf(t)} x2={xOf(t)} y1={0} y2={h} stroke="var(--line)" strokeOpacity={0.35} />
                ))}
                <rect x={xOf(length)} y={0} width={laneW} height={h} fill="rgba(0,0,0,.25)" pointerEvents="none" />
                {tr.kind === "color" && (
                  <>
                    <defs>
                      <linearGradient id={`g_${id}_${tr.id}`} x1={0} x2={laneW} gradientUnits="userSpaceOnUse">
                        {(tr.keys.length ? sorted(tr.keys) : [{ t: 0, v: [255, 255, 255] }]).map((k, i) => (
                          <stop key={i} offset={Math.min(1, Math.max(0, xOf(k.t) / laneW))} stopColor={`rgb(${[0, 1, 2].map((c) => num(k.v, c)).join(",")})`} />
                        ))}
                      </linearGradient>
                    </defs>
                    <rect y={4} width={laneW} height={h - 18} fill={`url(#g_${id}_${tr.id})`} rx={3} pointerEvents="none" />
                  </>
                )}
                {comps(tr).map((c) => (
                  <g key={String(c)} pointerEvents="none">
                    {curve(tr, c)}
                  </g>
                ))}
                {tr.keys.map((k, i) => {
                  const on = sel?.track === tr.id && sel.key === i;
                  const x = xOf(k.t);
                  if (tr.kind === "event")
                    return <rect key={i} x={x - 6} y={h / 2 - 6} width={12} height={12} transform={`rotate(45 ${x} ${h / 2})`} className={`st-tl-k${on ? " on" : ""}`} onPointerDown={(e) => press(e, tr, i)} onContextMenu={(e) => keyMenu(e, tr, i)} data-key={i} />;
                  if (tr.kind === "color") return <circle key={i} cx={x} cy={h - 7} r={6} className={`st-tl-k${on ? " on" : ""}`} onPointerDown={(e) => press(e, tr, i)} onContextMenu={(e) => keyMenu(e, tr, i)} data-key={i} />;
                  const [lo, hi] = range(tr);
                  return comps(tr).map((c) => (
                    <circle
                      key={`${i}_${c}`}
                      cx={x}
                      cy={((hi - num(k.v, c)) / (hi - lo)) * h}
                      r={5}
                      className={`st-tl-k${on ? " on" : ""}${k.ease === "ease" ? " ease" : ""}`}
                      style={c === undefined ? undefined : { stroke: COMP[c] }}
                      onPointerDown={(e) => press(e, tr, i, c)}
                      onContextMenu={(e) => keyMenu(e, tr, i)}
                      data-key={i}
                    />
                  ));
                })}
              </svg>
            </div>
          );
        })}
      </div>
    </div>
  );
}
