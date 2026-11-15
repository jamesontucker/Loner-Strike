// A recorded match on the map (plan M6): players as hull boxes in team colours with the way
// they look, trails of the last 15 seconds, kill and fall markers, a heat map, and for the
// picked bot the goal it was heading for (from the brain's trace).

import { Html } from "@react-three/drei";
import { useThree, type ThreeEvent } from "@react-three/fiber";
import { useEffect, useMemo, useRef } from "react";
import * as THREE from "three";
import { useTools } from "../../../stores/tools";
import { stateAt, traceRow, type Replay, type Trace } from "../overlays";
import { notAPick } from "./Cameras";
import { gsToThree, type Vec3 } from "./coords";

export const TEAM_COLOUR: Record<number, string> = { 1: "#d9a441", 2: "#8fb4d6" };
const TRAIL = 15;
const unit = new THREE.BoxGeometry(1, 1, 1);

export function ReplayLayer({ replay, trace, top }: { replay: Replay; trace: Trace | null; top: number }) {
  const time = useTools((s) => s.time);
  const sel = useTools((s) => s.selection);
  const follow = useTools((s) => s.follow);
  const heatMode = useTools((s) => s.heat);
  const invalidate = useThree((s) => s.invalidate);
  const camera = useThree((s) => s.camera);
  const controls = useThree((s) => s.controls) as unknown as { target: THREE.Vector3; update: () => void } | null;
  const picked = sel?.kind === "bot" ? sel.id : null;

  const boxes = useMemo(() => {
    const m = new THREE.InstancedMesh(unit, new THREE.MeshBasicMaterial({ transparent: true, opacity: 0.85 }), Math.max(1, replay.players.length));
    m.count = 0;
    return m;
  }, [replay]);
  const looks = useMemo(() => new THREE.BufferGeometry(), []);
  const trails = useMemo(() => new THREE.BufferGeometry(), []);
  useEffect(() => () => {
    (boxes.material as THREE.Material).dispose();
    boxes.dispose();
    looks.dispose();
    trails.dispose();
  }, [boxes, looks, trails]);

  // who is alive now and where
  const now = useMemo(
    () => replay.players.map((p) => ({ p, s: stateAt(replay, p, time) })).filter((x): x is { p: (typeof replay.players)[number]; s: NonNullable<ReturnType<typeof stateAt>> } => !!x.s),
    [replay, time],
  );

  useEffect(() => {
    const o = new THREE.Object3D();
    const c = new THREE.Color();
    const look: number[] = [], lookCol: number[] = [];
    now.forEach(({ p, s }, k) => {
      const h = s.duck ? 36 : 72;
      o.position.set(s.pos[0], s.pos[1], s.pos[2]);
      o.rotation.set(0, 0, (s.yaw * Math.PI) / 180);
      o.scale.set(32, 32, h);
      o.updateMatrix();
      boxes.setMatrixAt(k, o.matrix);
      c.set(p.id === picked ? "#fff6b0" : TEAM_COLOUR[p.team] ?? "#d8ded3");
      boxes.setColorAt(k, c);
      // the way it looks, from the eyes (origin + 17 standing, + 12 ducked)
      const y = (s.yaw * Math.PI) / 180, pt = (s.pitch * Math.PI) / 180;
      const eye = [s.pos[0], s.pos[1], s.pos[2] + (s.duck ? 12 : 17)];
      const len = p.id === picked ? 400 : 120;
      look.push(...eye, eye[0] + Math.cos(y) * Math.cos(pt) * len, eye[1] + Math.sin(y) * Math.cos(pt) * len, eye[2] - Math.sin(pt) * len);
      lookCol.push(c.r, c.g, c.b, c.r, c.g, c.b);
    });
    boxes.count = now.length;
    boxes.instanceMatrix.needsUpdate = true;
    if (boxes.instanceColor) boxes.instanceColor.needsUpdate = true;
    boxes.computeBoundingSphere();
    looks.setAttribute("position", new THREE.Float32BufferAttribute(look, 3));
    looks.setAttribute("color", new THREE.Float32BufferAttribute(lookCol, 3));
    // trails: the last 15 s of each living player, as segments
    const tr: number[] = [], trCol: number[] = [];
    for (const { p, s } of now) {
      c.set(TEAM_COLOUR[p.team] ?? "#d8ded3");
      for (let i = s.i; i > p.start && replay.t[i] > time - TRAIL; i--) {
        if (replay.t[i] - replay.t[i - 1] > 0.6) break;
        const f = 1 - (time - replay.t[i]) / TRAIL;
        tr.push(replay.x[i], replay.y[i], replay.z[i] - 30, replay.x[i - 1], replay.y[i - 1], replay.z[i - 1] - 30);
        trCol.push(c.r * f, c.g * f, c.b * f, c.r * f, c.g * f, c.b * f);
      }
    }
    trails.setAttribute("position", new THREE.Float32BufferAttribute(tr, 3));
    trails.setAttribute("color", new THREE.Float32BufferAttribute(trCol, 3));
    invalidate();
  }, [now, picked, boxes, looks, trails, replay, time, invalidate]);

  // the camera keeps the followed player in the middle
  const followed = useRef<Vec3 | null>(null);
  useEffect(() => {
    const me = follow === null ? null : now.find((x) => x.p.id === follow);
    if (!me) {
      followed.current = null;
      return;
    }
    const at = gsToThree(me.s.pos);
    if (followed.current && controls) {
      const was = gsToThree(followed.current);
      const d = at.clone().sub(was);
      camera.position.add(d);
      controls.target.add(d);
      controls.update();
    } else if (controls) {
      const off = camera.position.clone().sub(controls.target);
      controls.target.copy(at);
      camera.position.copy(at).add(off);
      controls.update();
    }
    followed.current = me.s.pos;
    invalidate();
  }, [now, follow, camera, controls, invalidate]);

  // kills (red crosses) and falls (magenta, with a line from where it stood)
  const marks = useMemo(() => {
    const pos: number[] = [], col: number[] = [];
    const c = new THREE.Color();
    for (const k of replay.kills) {
      const fell = !!k.fall || k.cause === "world" || k.cause === "trigger_hurt";
      c.set(fell ? "#ff4fd8" : "#ff5a4a");
      const [x, y, z] = k.at, s = 14;
      pos.push(x - s, y - s, z, x + s, y + s, z, x - s, y + s, z, x + s, y - s, z, x, y, z - s, x, y, z + s);
      for (let i = 0; i < 6; i++) col.push(c.r, c.g, c.b);
      if (k.fall) {
        pos.push(...k.fall.from, ...k.at);
        col.push(c.r, c.g, c.b, c.r, c.g, c.b);
      }
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
    g.setAttribute("color", new THREE.Float32BufferAttribute(col, 3));
    return g;
  }, [replay]);
  useEffect(() => () => marks.dispose(), [marks]);

  // heat map: one see-through square per 128-unit cell, laid over the map from above
  const heat = useMemo(() => {
    const h = replay.heat;
    if (!h || heatMode === "off") return null;
    const v = heatMode === "time" ? h.time : heatMode === "deaths" ? h.deaths : h.stuck;
    let max = 0;
    for (let i = 0; i < v.length; i++) max = Math.max(max, v[i]);
    if (!max) return null;
    const cells: number[] = [];
    for (let i = 0; i < v.length; i++) if (v[i] > 0) cells.push(i);
    const m = new THREE.InstancedMesh(new THREE.PlaneGeometry(1, 1), new THREE.MeshBasicMaterial({ transparent: true, opacity: 0.55, depthTest: false, depthWrite: false }), Math.max(1, cells.length));
    const o = new THREE.Object3D();
    const c = new THREE.Color();
    cells.forEach((i, k) => {
      const cx = i % h.w, cy = Math.floor(i / h.w);
      o.position.set(h.x0 + (cx + 0.5) * h.cell, h.y0 + (cy + 0.5) * h.cell, top);
      o.scale.set(h.cell * 0.96, h.cell * 0.96, 1);
      o.updateMatrix();
      m.setMatrixAt(k, o.matrix);
      // square root, so a few busy cells don't wash out the rest
      const f = Math.sqrt(v[i] / max);
      m.setColorAt(k, c.setHSL(0.17 - 0.17 * f, 0.9, 0.35 + 0.25 * f));
    });
    m.count = cells.length;
    m.renderOrder = 8;
    m.raycast = () => {};
    return m;
  }, [replay, heatMode, top]);
  useEffect(() => () => {
    if (!heat) return;
    heat.geometry.dispose();
    (heat.material as THREE.Material).dispose();
    heat.dispose();
  }, [heat]);
  useEffect(() => invalidate(), [heat, invalidate]);

  // the picked bot's goal and the noise it heard (from the trace)
  const goal = useMemo(() => {
    if (picked === null || !trace) return null;
    const me = now.find((x) => x.p.id === picked);
    const row = traceRow(trace, picked, time);
    if (!me || !row) return null;
    const pts: number[] = [];
    const gx = Number(row.goal_x), gy = Number(row.goal_y);
    if (gx || gy) pts.push(...me.s.pos, gx, gy, me.s.pos[2]);
    const nx = Number(row.noise_x), ny = Number(row.noise_y);
    if (nx || ny) pts.push(nx, ny, me.s.pos[2] - 30, nx, ny, me.s.pos[2] + 60);
    if (!pts.length) return null;
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
    return g;
  }, [picked, trace, now, time]);
  useEffect(() => () => goal?.dispose(), [goal]);

  const click = (e: ThreeEvent<MouseEvent>) => {
    if (notAPick(e) || e.instanceId === undefined) return;
    e.stopPropagation();
    const who = now[e.instanceId];
    if (who) useTools.getState().select({ kind: "bot", id: who.p.id });
  };
  const hover = (e: ThreeEvent<PointerEvent>) => {
    if (e.instanceId === undefined) return;
    e.stopPropagation();
    const who = now[e.instanceId];
    if (who) useTools.getState().setHover(`${who.p.name} · ${who.s.hp} hp`, who.s.pos);
  };

  return (
    <group>
      <primitive object={boxes} onClick={click} onPointerMove={hover} onPointerOut={() => useTools.getState().setHover(null)} />
      <lineSegments geometry={looks} renderOrder={7} raycast={() => null}>
        <lineBasicMaterial vertexColors />
      </lineSegments>
      <lineSegments geometry={trails} renderOrder={7} raycast={() => null}>
        <lineBasicMaterial vertexColors />
      </lineSegments>
      <lineSegments geometry={marks} renderOrder={7} raycast={() => null}>
        <lineBasicMaterial vertexColors depthTest={false} transparent />
      </lineSegments>
      {goal && (
        <lineSegments geometry={goal} renderOrder={9} raycast={() => null}>
          <lineBasicMaterial color="#fff6b0" depthTest={false} transparent />
        </lineSegments>
      )}
      {heat && <primitive object={heat} />}
      {now.map(({ p, s }) => (
        <Html key={p.id} position={[s.pos[0], s.pos[1], s.pos[2] + 52]} center zIndexRange={[6, 0]} style={{ pointerEvents: "none" }}>
          <span className={`tools-name${p.id === picked ? " picked" : ""}`} style={{ color: TEAM_COLOUR[p.team] }}>{p.name}</span>
        </Html>
      ))}
    </group>
  );
}
