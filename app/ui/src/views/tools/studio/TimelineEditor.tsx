// The Timeline editor (a tab of its own, opened by double-clicking a Timeline node or from My
// mode): its settings, and tracks as lanes of keys over time. Number and position tracks are
// curves (a position's x, y and z in three colors), a color track a gradient, an event track
// diamonds. Click a lane to add a key, drag a key to move it (Shift: one axis only), Delete
// removes the selected one, right-click a key for how it goes to the next (Linear, Constant,
// Ease In, Ease Out, Ease In-Out, Custom), a Custom key's tangent handles shape its curve (Alt
// drags one side only; Auto resets them), the wheel zooms time, dragging empty space pans, F fits
// the whole length.

import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as RPointerEvent } from "react";
import { useStudio } from "./store";
import { newTrack, removeTimeline, updateTimeline, type GKey, type GTimeline, type GTrack } from "./graphOps";
import { useContextMenu } from "./ContextMenu";
import { useFreshFocus } from "./freshFocus";
import { LABEL_CHARS } from "./schema";
import { useConfirm } from "../../../ui";
import { EASES, num, sortKeys as sorted, tangent, valueAt, type Ease } from "./curves";

export { valueAt };

const HEAD = 170;
const LANE: Record<GTrack["kind"], number> = { float: 140, vector: 140, color: 44, event: 36 };
const COMP = ["#e05a4f", "#5fbf6a", "#5a8fe0"];
const KIND_NAME: Record<GTrack["kind"], string> = { float: "Number", vector: "Position", color: "Color", event: "Event" };

const comps = (tr: GTrack) => (tr.kind === "vector" ? [0, 1, 2] : tr.kind === "float" ? [undefined] : []);

/** The values a lane shows, low to high (with some room): the keys and the curve between them, so a
 *  Custom tangent's overshoot isn't cut off at the lane's edge (QA4-B F6.7). */
function range(tr: GTrack): [number, number] {
  const vals = tr.keys.flatMap((k) => comps(tr).map((c) => num(k.v, c)));
  if (!vals.length) return [0, 1];
  if (tr.keys.length > 1) {
    const ts = tr.keys.map((k) => k.t);
    const t0 = Math.min(...ts),
      t1 = Math.max(...ts);
    for (const c of comps(tr)) for (let i = 1; i < 200; i++) vals.push(valueAt(tr.keys, t0 + ((t1 - t0) * i) / 200, c));
  }
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
  // lane heights a drag on a lane's bottom edge set (this editor only, not saved)
  const [heights, setHeights] = useState<Record<string, number>>({});
  const laneH = (tr: GTrack) => heights[tr.id] ?? LANE[tr.kind];
  const laneW = Math.max(200, width - HEAD - 16);
  const length = tl ? (tl.useLastKey ? Math.max(0.1, ...tl.tracks.flatMap((t) => t.keys.map((k) => k.t))) : tl.length) : 1;
  // (null: fitted to the length, following the editor's width until zoomed or panned: UX review M8)
  const fit = useCallback(() => setView(null), []);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setWidth(el.clientWidth));
    ro.observe(el);
    setWidth(el.clientWidth);
    return () => ro.disconnect();
  }, []);
  // fitted: the whole length with a little room at both ends, so keys at 0 and at the end are whole
  const FIT_PAD = 10;
  const fitPps = (laneW - 2 * FIT_PAD) / Math.max(tl?.length ?? 1, 0.1);
  const v = view ?? { pps: fitPps, at: -FIT_PAD / fitPps };
  const xOf = (t: number) => (t - v.at) * v.pps;
  const tOf = (x: number) => x / v.pps + v.at;
  const set = (f: (t: GTimeline) => GTimeline) => edit((g) => updateTimeline(g, id, f));
  const nameRef = useFreshFocus<HTMLInputElement>(id);
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
    const h = laneH(tr);
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

  /** how a key goes to the next (Linear drops the field, as before) */
  const setEase = (tid: string, i: number, e: Ease) => setTrack(tid, (x) => ({ ...x, keys: x.keys.map((k, j) => (j === i ? { ...k, ease: e === "linear" ? undefined : e } : k)) }));
  /** Auto: the key's own tangents go */
  const autoTangents = (tid: string, i: number) =>
    setTrack(tid, (x) => ({
      ...x,
      keys: x.keys.map((k, j) => {
        if (j !== i) return k;
        const { tin: _a, tout: _b, ...rest } = k as GKey & { tin?: unknown; tout?: unknown };
        void _a, _b;
        return rest as GKey;
      }),
    }));

  const keyMenu = (e: React.MouseEvent, tr: GTrack, i: number) =>
    cm.open(e, `${tr.name}: key at ${tr.keys[i].t} s`, [
      ...EASES.map(([e, label]) => ({ label: `${label}${(tr.keys[i].ease ?? "linear") === e ? " ✓" : ""}`, run: () => setEase(tr.id, i, e) })),
      "-",
      { label: "Delete key", danger: true, run: () => (setTrack(tr.id, (x) => ({ ...x, keys: x.keys.filter((_, j) => j !== i) })), setSel(null)) },
    ]);

  const curve = (tr: GTrack, c: number | undefined) => {
    const h = laneH(tr);
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

  /** A Custom key's tangent handles (the selected key): drag to shape the curve; Alt: one side. */
  const tangentHandles = (tr: GTrack) => {
    if (!sel || sel.track !== tr.id || (tr.kind !== "float" && tr.kind !== "vector")) return null;
    const keys = sorted(tr.keys);
    const k = tr.keys[sel.key];
    const si = keys.indexOf(k);
    if (!k || si < 0) return null;
    const custom = k.ease === "custom" || (si > 0 && keys[si - 1].ease === "custom");
    if (!custom) return null;
    const h = laneH(tr);
    const [lo, hi] = range(tr);
    const y = (val: number) => ((hi - val) / (hi - lo)) * h;
    const L = 40;
    const out: JSX.Element[] = [];
    for (const c of comps(tr)) {
      const kx = xOf(k.t),
        kv = num(k.v, c);
      for (const side of ["in", "out"] as const) {
        const dir = side === "in" ? -1 : 1;
        const s = tangent(keys, si, side, c);
        const dt = (L / v.pps) * dir;
        const hx = kx + L * dir,
          hy = y(kv + s * dt);
        out.push(
          <g key={`${side}${c}`}>
            <line x1={kx} y1={y(kv)} x2={hx} y2={hy} stroke={c === undefined ? "var(--signal, #e8c35a)" : COMP[c]} strokeDasharray="3 2" pointerEvents="none" />
            <rect
              x={hx - 4}
              y={hy - 4}
              width={8}
              height={8}
              className="st-tl-tan"
              data-tangent={`${side}${c ?? ""}`}
              aria-label={`${side === "in" ? "Incoming" : "Outgoing"} tangent`}
              onPointerDown={(e) => {
                e.preventDefault();
                e.stopPropagation();
                const svg = ((e.currentTarget as Element).closest("svg.st-tl-lane") as Element).getBoundingClientRect();
                const before = useStudio.getState().graph!;
                const yToV = (py: number) => hi - (py / h) * (hi - lo);
                const move = (ev: PointerEvent) => {
                  const px = ev.clientX - svg.left - kx;
                  if (Math.abs(px) < 4) return;
                  const slope = Math.round(((yToV(ev.clientY - svg.top) - kv) / (px / v.pps)) * 1000) / 1000;
                  const put = (old: unknown) => (c === undefined ? slope : [0, 1, 2].map((q) => (q === c ? slope : Array.isArray(old) && typeof old[q] === "number" ? old[q] : tangent(keys, si, side, q))));
                  live((g) =>
                    updateTimeline(g, id, (x) => ({
                      ...x,
                      tracks: x.tracks.map((tt) =>
                        tt.id !== tr.id
                          ? tt
                          : {
                              ...tt,
                              keys: tt.keys.map((kk, j) => {
                                if (j !== sel.key) return kk;
                                const cur = kk as GKey & { tin?: unknown; tout?: unknown };
                                // (user tangents: both sides together, like Unreal; Alt breaks them)
                                return ev.altKey ? { ...kk, [side === "in" ? "tin" : "tout"]: put(side === "in" ? cur.tin : cur.tout) } : { ...kk, tin: put(cur.tin), tout: put(cur.tout) };
                              }),
                            },
                      ),
                    })),
                  );
                };
                const up = () => {
                  window.removeEventListener("pointermove", move);
                  window.removeEventListener("pointerup", up);
                  commit(before);
                };
                window.addEventListener("pointermove", move);
                window.addEventListener("pointerup", up);
              }}
            />
          </g>,
        );
      }
    }
    return out;
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
          Name{" "}
          <input
            ref={nameRef}
            className="st-field"
            key={tl.name}
            defaultValue={tl.name}
            maxLength={30}
            aria-label="Timeline name"
            onBlur={(e) => {
              const name = [...e.target.value].filter((c) => LABEL_CHARS.test(c)).join("").trim().slice(0, 30);
              if (name && name !== tl.name) set((t) => ({ ...t, name }));
            }}
            onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
          />
        </label>
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
        <label title="How often the server works the values out: smaller is smoother and costs the server more. 0.1 s is the fastest AMX Mod X runs">
          Tick{" "}
          {/* (a graph saved with less than 0.1 s runs at 0.1 s, and shows it: QA4-B) */}
          <select className="st-field" value={Math.max(tl.tick, 0.1)} onChange={(e) => set((t) => ({ ...t, tick: Number(e.target.value) }))} aria-label="Tick">
            {[0.1, 0.2, 0.5].map((x) => (
              <option key={x} value={x}>
                {x} s{x === 0.1 ? " (default, the fastest)" : ""}
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
        {/* (what it removes, said: the whole Timeline; set apart from Fit) */}
        <span className="st-tl-sep" aria-hidden="true" />
        <button
          type="button"
          className="btn small danger"
          title="Delete this Timeline (the nodes that play it show a problem until changed)"
          onClick={async () => {
            const used = graph.nodes.filter((n) => n.type === "timeline.play" && n.params.timeline === id).length;
            if (!(await ask(`Remove the Timeline “${tl.name}”?${used ? ` ${used} node${used === 1 ? "" : "s"} play it and show a problem until changed.` : ""}`, { ok: "Remove", danger: true }))) return;
            closeTab(id);
            openTab("");
            edit((g) => removeTimeline(g, id));
          }}
        >
          Delete timeline
        </button>
      </div>
      {!(selKey && selTrack && sel) && <div className="st-tl-key st-tl-key-hint st-muted">Click a lane to add a key; click a key to change it (right-click: how it goes to the next).</div>}
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
              <select className="st-field" value={selKey.ease ?? "linear"} onChange={(e) => setEase(selTrack.id, sel.key, e.target.value as Ease)} aria-label="Key interpolation">
                {EASES.map(([e, label]) => (
                  <option key={e} value={e}>
                    {label}
                  </option>
                ))}
              </select>
            </label>
          )}
          {selTrack.kind !== "event" && selTrack.kind !== "color" && (
            <button type="button" className="btn small" onClick={() => autoTangents(selTrack.id, sel.key)} title="Its tangents go back to following its neighbors" disabled={!("tin" in selKey) && !("tout" in selKey)}>
              Auto tangents
            </button>
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
              <text y={10} fontSize={10} fill="var(--bone-dim, #9aa)" textAnchor={xOf(t) < 12 ? "start" : xOf(t) > laneW - 12 ? "end" : "middle"}>
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
          const h = laneH(tr);
          return (
            <div key={tr.id} className="st-tl-track" data-track={tr.id}>
              {/* the header is the lane's height, on one line: name, kind, remove (UX: site shots) */}
              <div className="st-tl-head" style={{ width: HEAD, height: h }} data-testid="timeline-head">
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
                <button type="button" className="st-tl-x" aria-label={`Remove track ${tr.name}`} title={`Remove the track “${tr.name}” (Undo brings it back)`} onClick={() => (set((t) => ({ ...t, tracks: t.tracks.filter((x) => x.id !== tr.id) })), setSel(null))}>
                  ×
                </button>
                {/* the lane's value range, beside it (inside the lane the numbers sat under keys) */}
                {comps(tr).length > 0 &&
                  (() => {
                    const [lo, hi] = range(tr);
                    const f = (x: number) => (Math.abs(x) >= 100 ? x.toFixed(0) : x.toFixed(1));
                    return (
                      <>
                        <span className="st-tl-ax hi" title="The lane's top">{f(hi)}</span>
                        <span className="st-tl-ax lo" title="The lane's bottom">{f(lo)}</span>
                      </>
                    );
                  })()}
              </div>
              <svg className="st-tl-lane" width={laneW} height={h} onPointerDown={(e) => press(e, tr, null)} aria-label={`${tr.name} lane`}>
                <rect width={laneW} height={h} fill="transparent" />
                {ticks.map((t) => (
                  <line key={t} x1={xOf(t)} x2={xOf(t)} y1={0} y2={h} stroke="var(--line)" strokeOpacity={0.35} />
                ))}
                <rect x={xOf(length)} y={0} width={laneW} height={h} fill="rgba(0,0,0,.25)" pointerEvents="none" />
                {comps(tr).length > 0 &&
                  (() => {
                    // the zero line (the top and bottom values are in the header, UX review M8)
                    const [lo, hi] = range(tr);
                    return (
                      <g pointerEvents="none">
                        {lo < 0 && hi > 0 && <line x1={0} x2={laneW} y1={(hi / (hi - lo)) * h} y2={(hi / (hi - lo)) * h} stroke="var(--line)" strokeDasharray="3 3" />}
                      </g>
                    );
                  })()}
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
                {tangentHandles(tr)}
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
                      className={`st-tl-k${on ? " on" : ""}${k.ease && k.ease !== "linear" ? ` ease ${k.ease}` : ""}`}
                      style={c === undefined ? undefined : { stroke: COMP[c] }}
                      onPointerDown={(e) => press(e, tr, i, c)}
                      onContextMenu={(e) => keyMenu(e, tr, i)}
                      data-key={i}
                    />
                  ));
                })}
              </svg>
              <div
                className="st-tl-resize"
                title="Drag to make the lane taller or shorter"
                aria-hidden="true"
                onPointerDown={(e) => {
                  e.preventDefault();
                  const y0 = e.clientY;
                  const h0 = h;
                  const move = (m: PointerEvent) => setHeights((x) => ({ ...x, [tr.id]: Math.min(480, Math.max(30, h0 + m.clientY - y0)) }));
                  const up = () => (window.removeEventListener("pointermove", move), window.removeEventListener("pointerup", up));
                  window.addEventListener("pointermove", move);
                  window.addEventListener("pointerup", up);
                }}
              />
            </div>
          );
        })}
      </div>
    </div>
  );
}
