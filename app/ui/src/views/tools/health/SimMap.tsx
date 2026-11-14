// The simulation report's map: the map from above (the radar maker's picture) with what the
// bots did laid over it: where they spent their time, where they died, where kills came
// from, the nav areas nobody walked, and pins for the problems and the busiest fights.
// One layer at a time, a readout of the spot under the pointer, zoom and drag like the
// radar maker.

import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState, type PointerEvent as RPointerEvent } from "react";
import { LoadError, Segmented, Skeleton } from "../../../ui";
import { DEFAULT_CHOICE, overviewInfo, previewBytes, type OverviewInfo } from "../overview/overviewApi";
import { FIT, PanZoom, type View } from "../overview/PanZoom";
import "../overview/overview.css";
import type { Report, Spot } from "./healthApi";

const W = 1024, H = 768;
type Frame = OverviewInfo["frame"];

/** Picture pixels of a world point (overview.rs Frame::to_pixel). */
export function toPixel(f: Frame, x: number, y: number): [number, number] {
  const z = f.zoom, u = 8192 / (z * W), [ox, oy] = f.origin;
  return f.rotated ? [(x - ox + 4096 / z) / u, (oy + 3072 / z - y) / u] : [(oy + 4096 / z - y) / u, (ox + 3072 / z - x) / u];
}

/** The world point at picture pixels (Frame::to_world). */
export function toWorld(f: Frame, col: number, row: number): [number, number] {
  const z = f.zoom, u = 8192 / (z * W), [ox, oy] = f.origin;
  return f.rotated ? [ox - 4096 / z + col * u, oy + 3072 / z - row * u] : [ox + 3072 / z - row * u, oy + 4096 / z - col * u];
}

/** A world box as a picture rectangle. */
function box(f: Frame, x0: number, y0: number, x1: number, y1: number) {
  const [ax, ay] = toPixel(f, x0, y0);
  const [bx, by] = toPixel(f, x1, y1);
  return { x: Math.min(ax, bx), y: Math.min(ay, by), w: Math.abs(bx - ax), h: Math.abs(by - ay) };
}

export type Layer = "time" | "deaths" | "kills" | "unvisited";
const LAYERS: [Layer, string][] = [
  ["time", "Where bots went"],
  ["deaths", "Where they died"],
  ["kills", "Where kills came from"],
  ["unvisited", "Where nobody went"],
];
const LEGEND: Record<Layer, string> = {
  time: "Brighter: more bot time spent there. Dark: hardly anyone came by.",
  deaths: "Brighter: more bots killed there by enemies. The busiest spots are where fights are decided.",
  kills: "Brighter: more kills made from there. Spots that win many fights are strong angles.",
  unvisited: "Nav areas no bot walked over in the whole run: cut off, hard to reach, or not worth going to.",
};

type Pin = { kind: "fall" | "stuck" | "hurt" | "fight"; at: [number, number]; text: string; size: number };
const PIN_LABEL: Record<Pin["kind"], string> = { fall: "Falls", stuck: "Stuck", hurt: "Map kills", fight: "Fights" };
const PIN_GLYPH: Record<Pin["kind"], string> = { fall: "▼", stuck: "■", hurt: "✕", fight: "●" };

const placeOr = (s: { place: string; at: number[] }) => s.place || `${Math.round(s.at[0])} ${Math.round(s.at[1])}`;

function pinsOf(r: Report): Pin[] {
  const p: Pin[] = [];
  const add = (kind: Pin["kind"], list: Spot[], text: (s: Spot) => string) =>
    list.forEach((s) => p.push({ kind, at: [s.at[0], s.at[1]], text: text(s), size: s.count }));
  add("fall", r.fallSpots, (s) => `${placeOr(s)}: ${s.count} ${s.count === 1 ? "fall" : "falls"}`);
  add("stuck", r.stuckSpots, (s) => `${placeOr(s)}: stuck ${Math.round(s.seconds)} s in all`);
  add("hurt", r.hurtSpots, (s) => `${placeOr(s)}: the map killed ${s.count}`);
  (r.fightSpots ?? []).forEach((s) =>
    p.push({ kind: "fight", at: [s.at[0], s.at[1]], text: `${placeOr(s)}: ${s.deaths} died here (${s.tDeaths} T, ${s.ctDeaths} CT)`, size: s.deaths }),
  );
  return p;
}

export function SimMap({ r }: { r: Report }) {
  const [layer, setLayer] = useState<Layer>("time");
  const [pins, setPins] = useState(true);
  const [view, setView] = useState<View>(FIT);
  const [hover, setHover] = useState<string | null>(null);
  const info = useQuery({ queryKey: ["tools", "overview", "info", r.map], queryFn: ({ signal }) => overviewInfo(r.map, signal), retry: false, staleTime: 60_000 });
  // the picture's bytes are cached (by map); the page makes its own object URL from them each
  // time it's shown (a cached URL would be revoked when the page was left, and come back blank)
  const pic = useQuery({
    queryKey: ["tools", "health", "picture", r.map],
    queryFn: ({ signal }) => previewBytes(r.map, { ...DEFAULT_CHOICE, style: "clean", marks: ["bomb", "rescue", "spawns"], background: "black" }, signal),
    enabled: !!info.data,
    retry: false,
    staleTime: Infinity,
    gcTime: 5 * 60_000,
  });
  const [picUrl, setPicUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!pic.data) return setPicUrl(null);
    const url = URL.createObjectURL(new Blob([pic.data], { type: "image/png" }));
    setPicUrl(url);
    return () => URL.revokeObjectURL(url);
  }, [pic.data]);
  const f = info.data?.frame;
  const heat = r.heat;
  const allPins = useMemo(() => pinsOf(r), [r]);

  const cells = useMemo(() => {
    if (!f || !heat || layer === "unvisited") return [];
    const v = heat[layer] as number[];
    const max = Math.max(0, ...v);
    if (max <= 0) return [];
    const out: { x: number; y: number; w: number; h: number; a: number }[] = [];
    v.forEach((n, i) => {
      if (n <= 0) return;
      const cx = i % heat.w, cy = Math.floor(i / heat.w);
      const x0 = heat.x0 + cx * heat.cell, y0 = heat.y0 + cy * heat.cell;
      // square root: a few busy cells don't wash out the rest
      out.push({ ...box(f, x0, y0, x0 + heat.cell, y0 + heat.cell), a: 0.18 + 0.72 * Math.sqrt(n / max) });
    });
    return out;
  }, [f, heat, layer]);

  const rects = useMemo(() => (f && layer === "unvisited" ? (r.unvisited ?? []).map((u) => box(f, u[0], u[1], u[2], u[3])) : []), [f, layer, r.unvisited]);

  if (info.isPending) return <Skeleton rows={4} />;
  if (info.isError || !f)
    return <LoadError text="The map picture isn't available: the map file isn't on this computer any more, or it couldn't be read." onRetry={() => info.refetch()} />;

  // the readout under the pointer: the cell's numbers and the place there
  const readout = (e: RPointerEvent<SVGSVGElement>) => {
    const svg = e.currentTarget;
    const pt = svg.createSVGPoint();
    pt.x = e.clientX;
    pt.y = e.clientY;
    const m = svg.getScreenCTM();
    if (!m) return;
    const p = pt.matrixTransform(m.inverse());
    const [x, y] = toWorld(f, p.x, p.y);
    const near = allPins.find((q) => pins && Math.hypot(q.at[0] - x, q.at[1] - y) < 160);
    if (near) return setHover(`${PIN_LABEL[near.kind]} · ${near.text}`);
    if (!heat) return setHover(null);
    const cx = Math.floor((x - heat.x0) / heat.cell), cy = Math.floor((y - heat.y0) / heat.cell);
    if (cx < 0 || cy < 0 || cx >= heat.w || cy >= heat.h) return setHover(null);
    const i = cy * heat.w + cx;
    const t = heat.time[i] ?? 0, d = heat.deaths[i] ?? 0, k = heat.kills[i] ?? 0;
    if (!t && !d && !k) return setHover(`${Math.round(x)} ${Math.round(y)}: no bot came here`);
    setHover(`${Math.round(x)} ${Math.round(y)}: ${t >= 60 ? `${(t / 60).toFixed(1)} bot-minutes` : `${Math.round(t)} s of bot time`}, ${d} ${d === 1 ? "death" : "deaths"}, ${k} ${k === 1 ? "kill" : "kills"} made from here`);
  };

  const unit = 8192 / (f.zoom * W);
  const noData = layer === "unvisited" ? rects.length === 0 : cells.length === 0;
  return (
    <div className="sim-map">
      <div className="row sim-map-bar">
        <Segmented label="Show on the map" options={LAYERS} current={layer} onPick={(v) => setLayer(v as Layer)} />
        <label className="sim-map-pins">
          <input type="checkbox" checked={pins} onChange={(e) => setPins(e.target.checked)} /> Problems and fights
        </label>
      </div>
      <div className="sim-map-pic">
        {pic.isPending || (pic.data && !picUrl) ? (
          <Skeleton rows={6} />
        ) : (
          <PanZoom view={view} onView={setView} label={`${r.map} from above: ${LAYERS.find((l) => l[0] === layer)![1].toLowerCase()}`}>
            <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid meet" className="sim-map-svg" onPointerMove={readout} onPointerLeave={() => setHover(null)}>
              {picUrl ? <image href={picUrl} width={W} height={H} /> : <rect width={W} height={H} className="sim-map-blank" />}
              <g className={`sim-heat ${layer}`}>
                {cells.map((c, i) => (
                  <rect key={i} x={c.x} y={c.y} width={c.w} height={c.h} opacity={c.a} />
                ))}
                {rects.map((c, i) => (
                  <rect key={i} x={c.x} y={c.y} width={Math.max(c.w, 1.5)} height={Math.max(c.h, 1.5)} />
                ))}
              </g>
              {pins && (
                <g className="sim-pins">
                  {allPins.map((p, i) => {
                    const [x, y] = toPixel(f, p.at[0], p.at[1]);
                    const rad = Math.min(22, 7 + Math.sqrt(p.size) * 2.5) / Math.max(1, view.s * 0.6);
                    return (
                      <g key={i} className={`sim-pin ${p.kind}`} transform={`translate(${x} ${y})`}>
                        <circle r={rad} />
                        <text dy="0.35em" fontSize={rad * 1.05}>
                          {PIN_GLYPH[p.kind]}
                        </text>
                      </g>
                    );
                  })}
                </g>
              )}
            </svg>
          </PanZoom>
        )}
      </div>
      <p className="sim-map-readout" aria-live="polite">
        {hover ?? (noData ? (layer === "unvisited" ? (r.navAreas ? "Bots walked over every nav area." : "This map had no nav, so there's nothing to show.") : "Nothing recorded for this layer (a report from before maps were added, or no fights).") : "Point at the map for the numbers there. Scroll to zoom, drag to move, double-click to fit.")}
      </p>
      <div className="sim-map-legend">
        <span className={`sim-swatch ${layer}`} aria-hidden="true" />
        <span>{LEGEND[layer]}</span>
        {heat && layer !== "unvisited" && <span className="muted">Squares of {Math.round(heat.cell)} units ({Math.round(heat.cell / unit)} px).</span>}
        {pins && (
          <span className="sim-pin-key">
            {(Object.keys(PIN_LABEL) as Pin["kind"][]).map((k) => (
              <span key={k} className={`sim-pin-key-item ${k}`}>
                <b aria-hidden="true">{PIN_GLYPH[k]}</b> {PIN_LABEL[k]}
              </span>
            ))}
          </span>
        )}
      </div>
    </div>
  );
}
