// P5: leak tracing like Hammer's pointfile, and then some. When the map leaks (the last compile,
// or a .lin/.pts loaded from another compile) a bar over the views says so with:
// - Go to leak: the 3D camera flies along the red line from the entity that sees the void to
//   the hole, the flat views zoom to the hole, and the world brush nearest the hole is picked
//   (where the gap is to close);
// - Pick the brushes round the hole (all the nearest world brushes);
// - Hide/Show the line, and Forget it.
// The app works out the hole (tools/editor/leak.rs): where the line leaves the world's brushes
// for good. Agents get the same through MCP editor_leak.

import { useFrame, useThree } from "@react-three/fiber";
import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { gsToThree, type Vec3 } from "../viewer/coords";
import { clearLeak, loadPointfile, useLeak } from "./compileApi";
import { frameFlat, usePaneActive } from "./panes";
import { useEditor } from "./store";
import { BusyLabel } from "./Panels";

/** Fly to the leak: the 3D view travels the line, the flat views zoom to the hole, the brush at the hole is picked. */
export function goToLeak() {
  const l = useLeak.getState();
  const hole = l.info?.hole?.at ?? (l.points ? l.points[l.points.length - 1] : undefined);
  if (!l.points || !hole) return;
  if (!l.visible) l.toggle(true);
  useLeak.setState({ fly: l.fly + 1 });
  const r = 160;
  frameFlat([hole[0] - r, hole[1] - r, hole[2] - r], [hole[0] + r, hole[1] + r, hole[2] + r]);
  const near = l.info?.nearest?.[0];
  if (near) useEditor.getState().select({ brushes: [near.brush], entities: [], face: null });
}

/** Pick every world brush round the hole. */
export function pickLeakBrushes() {
  const ids = useLeak.getState().info?.nearest?.map((n) => n.brush) ?? [];
  if (ids.length) useEditor.getState().select({ brushes: ids, entities: [], face: null });
}

/** Map > Load pointfile…: a leak line from another compile. */
export async function loadLeakFile() {
  const r = await loadPointfile();
  const s = useEditor.getState();
  if (r.cancelled) return;
  if (r.error) return s.notify(r.error, true);
  await useLeak.getState().refresh();
  s.notify(`Leak line loaded from ${r.pointfile} (${r.points?.length ?? 0} points).`);
}

/** Inside the 3D pane's scene: the flight along the line when Go to leak is pressed. */
export function LeakFlight() {
  const fly = useLeak((s) => s.fly);
  const active = usePaneActive();
  const camera = useThree((s) => s.camera);
  const controls = useThree((s) => s.controls) as unknown as { target?: THREE.Vector3; update?: () => void } | null;
  const invalidate = useThree((s) => s.invalidate);
  const run = useRef<{ path: THREE.Vector3[]; cum: number[]; t0: number; dur: number; hole: THREE.Vector3 } | null>(null);
  useEffect(() => {
    if (!fly || !active) return;
    const l = useLeak.getState();
    const pts = l.points;
    if (!pts || pts.length < 2) return;
    const end = Math.min(pts.length - 1, l.info?.hole?.index ?? pts.length - 1);
    const path = pts.slice(0, end + 1).map((p) => gsToThree(p as Vec3));
    if (path.length < 2) path.push(path[0].clone().add(new THREE.Vector3(1, 0, 0)));
    const cum = [0];
    for (let i = 1; i < path.length; i++) cum.push(cum[i - 1] + path[i].distanceTo(path[i - 1]));
    const len = cum[cum.length - 1];
    // about 400 units a second, between 1.5 and 6 seconds
    run.current = { path, cum, t0: performance.now(), dur: Math.min(6000, Math.max(1500, (len / 400) * 1000)), hole: gsToThree(pts[end] as Vec3) };
    invalidate();
    const stop = (e: KeyboardEvent) => e.key === "Escape" && (run.current = null);
    window.addEventListener("keydown", stop);
    return () => window.removeEventListener("keydown", stop);
  }, [fly, active, invalidate]);
  useFrame(() => {
    const r = run.current;
    if (!r) return;
    const k = Math.min(1, (performance.now() - r.t0) / r.dur);
    const e = k < 0.5 ? 2 * k * k : 1 - Math.pow(-2 * k + 2, 2) / 2; // ease in and out
    const at = (d: number) => {
      const len = r.cum[r.cum.length - 1];
      const x = Math.max(0, Math.min(len, d));
      let i = r.cum.findIndex((c) => c >= x);
      if (i <= 0) return r.path[0].clone();
      const s = (x - r.cum[i - 1]) / Math.max(1e-6, r.cum[i] - r.cum[i - 1]);
      return r.path[i - 1].clone().lerp(r.path[i], s);
    };
    const len = r.cum[r.cum.length - 1];
    const here = at(e * len);
    const ahead = at(e * len + 96);
    const dir = ahead.clone().sub(here);
    if (dir.lengthSq() < 1e-6) dir.copy(r.hole.clone().sub(here));
    if (dir.lengthSq() < 1e-6) dir.set(1, 0, 0);
    dir.normalize();
    // a little behind and above the line, looking along it; at the end, back off to see the hole
    const back = 48 + 140 * Math.max(0, (k - 0.85) / 0.15);
    camera.position.copy(here).addScaledVector(dir, -back).add(new THREE.Vector3(0, 24, 0));
    const look = k >= 1 ? r.hole : ahead;
    camera.lookAt(look);
    if (controls?.target) {
      controls.target.copy(look);
      controls.update?.();
    }
    if (k >= 1) run.current = null;
    invalidate();
  });
  return null;
}

/** Over the views: the leak, said plainly, with what to do. */
export function LeakBar() {
  const points = useLeak((s) => s.points);
  const info = useLeak((s) => s.info);
  const visible = useLeak((s) => s.visible);
  const [busy, setBusy] = useState(false);
  // (after a reload too: the app keeps the last leak)
  useEffect(() => {
    void useLeak.getState().refresh();
  }, []);
  if (!points || points.length < 2) return null;
  const near = info?.nearest ?? [];
  const ents = info?.brushEntitiesNear ?? [];
  return (
    <div className="p5-leakbar" role="alert" aria-label="The map leaks">
      <strong>The map leaks.</strong>
      <span className="p5-leakbar-text">
        {info?.start ? `The red line runs from ${info.start.classname} to the gap.` : "The red line runs from an entity to the gap."}{" "}
        {near.length ? `The gap is next to brush #${near[0].brush}${near.length > 1 ? ` (and ${near.slice(1).map((n) => `#${n.brush}`).join(", ")})` : ""}.` : ""}
        {ents.length ? ` ${ents[0].classname} is there too: brush entities don't seal a map.` : ""}
        {info?.from === "file" && info.pointfile ? ` (From ${info.pointfile}.)` : ""}
      </span>
      <button type="button" className="btn small primary" data-cmd="map.goToLeak" onClick={() => goToLeak()} title="Fly along the line to the hole and pick the brush beside it (Esc stops the flight)">
        Go to leak
      </button>
      {near.length > 0 && (
        <button type="button" className="btn small" onClick={() => pickLeakBrushes()} title="Pick the world brushes round the hole">
          Pick brushes at the hole
        </button>
      )}
      <button type="button" className="btn small" onClick={() => useLeak.getState().toggle()}>
        {visible ? "Hide line" : "Show line"}
      </button>
      <button
        type="button"
        className="btn small"
        disabled={busy}
        title="Forget this leak line (a compile that leaks shows a new one)"
        onClick={async () => {
          setBusy(true);
          await clearLeak();
          await useLeak.getState().refresh();
          setBusy(false);
        }}
      >
        {busy ? <BusyLabel busy label="Forgetting…" /> : "Forget it"}
      </button>
    </div>
  );
}
