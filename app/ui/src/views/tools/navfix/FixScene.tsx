// The nav fixer's 3D view: the map (B's World and Cameras, reused as they are) with the nav
// drawn editable on top. Clicks are picked here, against the canvas, so drawing a box on the
// floor works where there's no nav yet (a beam the learner missed):
//  Select  click an area (Shift adds to the selection)
//  Draw    drag on the floor: a box at the height you started on
//  Link    click A, then B (Shift: one way, a drop)

import { Canvas, useThree } from "@react-three/fiber";
import { forwardRef, useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { useTools, type NavColour } from "../../../stores/tools";
import type { Nav } from "../overlays";
import type { MeshBundle } from "../viewer/bundle";
import { Cameras } from "../viewer/Cameras";
import { ROOT_ROTATION, type Vec3 } from "../viewer/coords";
import { pointerEvents } from "../viewer/Scene";
import { areaColours } from "../viewer/NavLayer";
import { World, type ModelMesh } from "../viewer/World";
import { REACH_NEVER, REACH_TRAP } from "./navfixApi";

export type Mode = "select" | "draw" | "link";
export type ColourBy = "reach" | Exclude<NavColour, "island">;
export type Draft = { a: Vec3; b: Vec3 } | null;

const LIFT = 3;
const C = {
  ok: new THREE.Color("#6fc2b0"),
  never: new THREE.Color("#e2574c"),
  trap: new THREE.Color("#e8a23c"),
  picked: new THREE.Color("#fff6b0"),
  from: new THREE.Color("#7fd1ff"),
  problem: new THREE.Color("#ff5fd2"),
};

type Props = {
  bundle: MeshBundle;
  meshes: ModelMesh[];
  map: string;
  nav: Nav;
  reach: number[];
  colourBy: ColourBy;
  mode: Mode;
  selected: Set<number>;
  linkFrom: number | null;
  shown: Set<number>;
  busy: boolean;
  onPick: (id: number | null, shift: boolean) => void;
  onDraw: (x: [number, number], y: [number, number], z: number) => void;
  onLost: () => void;
};

export function FixScene(p: Props) {
  return (
    <Canvas
      onCreated={({ gl }) => gl.domElement.addEventListener("webglcontextlost", (e) => {
        e.preventDefault();
        // a canvas being put away (leaving the page, another map) drops its context on purpose
        if (gl.domElement.isConnected) p.onLost();
      })}
      frameloop="demand"
      dpr={[1, 1.5]}
      linear
      flat
      gl={{ powerPreference: "high-performance", antialias: true }}
      camera={{ fov: 70, near: 2, far: 40000, position: [0, 512, 512] }}
      events={pointerEvents}
      onPointerLeave={() => useTools.getState().setHover(null)}
    >
      <color attach="background" args={["#262c21"]} />
      <hemisphereLight args={["#ffffff", "#5a5f50", 0.9]} />
      <directionalLight position={[0.4, 1, 0.25]} intensity={1.6} />
      <Root {...p} />
      <Cameras bounds={p.bundle.bounds} />
    </Canvas>
  );
}

function Root(p: Props) {
  const root = useRef<THREE.Group>(null);
  const tiles = useRef<THREE.Mesh>(null);
  const [draft, setDraft] = useState<Draft>(null);
  return (
    <group ref={root} rotation={ROOT_ROTATION}>
      <World meshes={p.meshes} bundle={p.bundle} map={p.map} />
      <EditNav ref={tiles} {...p} />
      {draft && <DraftBox draft={draft} />}
      <Picker root={root} tiles={tiles} mode={p.mode} busy={p.busy} nav={p.nav} onPick={p.onPick} onDraw={p.onDraw} setDraft={setDraft} />
    </group>
  );
}

// ---- the nav ---------------------------------------------------------------------------------

const EditNav = forwardRef<THREE.Mesh, Props>(function EditNav({ nav, reach, colourBy, selected, linkFrom, shown }, ref) {
  const invalidate = useThree((s) => s.invalidate);
  const geo = useMemo(() => {
    const n = nav.areas.length;
    const pos = new Float32Array(n * 12);
    const idx = new Uint32Array(n * 6);
    nav.areas.forEach((a, i) => {
      const [x0, y0, z0] = a.nw, [x1, y1, z1] = a.se;
      pos.set([x0, y0, z0 + LIFT, x1, y0, a.neZ + LIFT, x1, y1, z1 + LIFT, x0, y1, a.swZ + LIFT], i * 12);
      const v = i * 4;
      idx.set([v, v + 3, v + 2, v, v + 2, v + 1], i * 6);
    });
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
    g.setAttribute("color", new THREE.BufferAttribute(new Float32Array(n * 12), 3));
    g.setIndex(new THREE.BufferAttribute(idx, 1));
    g.computeBoundingSphere();
    const edges = new Float32Array(n * 24);
    nav.areas.forEach((_, i) => {
      const c = [0, 1, 2, 3].map((k) => [pos[i * 12 + k * 3], pos[i * 12 + k * 3 + 1], pos[i * 12 + k * 3 + 2]]);
      [0, 1, 1, 2, 2, 3, 3, 0].forEach((k, j) => edges.set(c[k], (i * 8 + j) * 3));
    });
    const e = new THREE.BufferGeometry();
    e.setAttribute("position", new THREE.BufferAttribute(edges, 3));
    // links between centres: both ways grey, one way orange with an arrowhead
    const oneWay = new Set(nav.oneWay.map(([a, b]) => `${a}>${b}`));
    const two: number[] = [], one: number[] = [];
    const seen = new Set<string>();
    nav.areas.forEach((a, i) => {
      const q0 = nav.centres[i];
      for (const id of a.conns.flat()) {
        const j = nav.index.get(id);
        if (j === undefined) continue;
        const q1 = nav.centres[j];
        const z0 = q0[2] + LIFT * 2, z1 = q1[2] + LIFT * 2;
        if (oneWay.has(`${a.id}>${id}`)) {
          one.push(q0[0], q0[1], z0, q1[0], q1[1], z1);
          const dx = q1[0] - q0[0], dy = q1[1] - q0[1];
          const len = Math.hypot(dx, dy) || 1;
          const ux = dx / len, uy = dy / len, s = Math.min(24, len / 3);
          for (const side of [1, -1]) one.push(q1[0], q1[1], z1, q1[0] - ux * s - uy * s * 0.5 * side, q1[1] - uy * s + ux * s * 0.5 * side, z1);
        } else {
          const key = a.id < id ? `${a.id}-${id}` : `${id}-${a.id}`;
          if (seen.has(key)) continue;
          seen.add(key);
          two.push(q0[0], q0[1], z0, q1[0], q1[1], z1);
        }
      }
    });
    const lad: number[] = [];
    for (const l of nav.ladders) lad.push(...l.bottom, ...l.top);
    const mk = (v: number[]) => {
      const g2 = new THREE.BufferGeometry();
      g2.setAttribute("position", new THREE.Float32BufferAttribute(v, 3));
      return g2;
    };
    return { g, e, two: mk(two), one: mk(one), lad: mk(lad) };
  }, [nav]);
  useEffect(() => () => Object.values(geo).forEach((g) => g.dispose()), [geo]);

  useEffect(() => {
    const base = colourBy === "reach" ? null : areaColours(nav, colourBy, null, "off");
    const attr = geo.g.getAttribute("color") as THREE.BufferAttribute;
    nav.areas.forEach((a, i) => {
      let col = base ? base[i] : reach[i] === REACH_NEVER ? C.never : reach[i] === REACH_TRAP ? C.trap : C.ok;
      if (shown.has(a.id)) col = C.problem;
      if (selected.has(a.id)) col = C.picked;
      if (a.id === linkFrom) col = C.from;
      for (let k = 0; k < 4; k++) attr.setXYZ(i * 4 + k, col.r, col.g, col.b);
    });
    attr.needsUpdate = true;
    invalidate();
  }, [nav, reach, colourBy, selected, linkFrom, shown, geo, invalidate]);

  return (
    <group>
      <mesh ref={ref} geometry={geo.g} renderOrder={2} userData={{ navTiles: true }}>
        <meshBasicMaterial vertexColors transparent opacity={0.5} depthWrite={false} side={THREE.DoubleSide} polygonOffset polygonOffsetFactor={-2} polygonOffsetUnits={-2} />
      </mesh>
      <lineSegments geometry={geo.e} renderOrder={3} raycast={() => null}>
        <lineBasicMaterial color="#1c2018" transparent opacity={0.6} />
      </lineSegments>
      <lineSegments geometry={geo.two} renderOrder={3} raycast={() => null}>
        <lineBasicMaterial color="#d8ded3" transparent opacity={0.45} />
      </lineSegments>
      <lineSegments geometry={geo.one} renderOrder={4} raycast={() => null}>
        <lineBasicMaterial color="#f08a3c" transparent opacity={0.95} depthTest={false} />
      </lineSegments>
      <lineSegments geometry={geo.lad} renderOrder={4} raycast={() => null}>
        <lineBasicMaterial color="#7fd1ff" />
      </lineSegments>
    </group>
  );
});

function DraftBox({ draft }: { draft: NonNullable<Draft> }) {
  const geo = useMemo(() => {
    const [x0, x1] = [Math.min(draft.a[0], draft.b[0]), Math.max(draft.a[0], draft.b[0])];
    const [y0, y1] = [Math.min(draft.a[1], draft.b[1]), Math.max(draft.a[1], draft.b[1])];
    const z = draft.a[2] + LIFT * 2;
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute([x0, y0, z, x1, y0, z, x1, y0, z, x1, y1, z, x1, y1, z, x0, y1, z, x0, y1, z, x0, y0, z], 3));
    return g;
  }, [draft]);
  useEffect(() => () => geo.dispose(), [geo]);
  return (
    <lineSegments geometry={geo} renderOrder={6} raycast={() => null}>
      <lineBasicMaterial color="#fff6b0" depthTest={false} />
    </lineSegments>
  );
}

// ---- picking ---------------------------------------------------------------------------------

/** Whether a hit is something you can see (a shown mesh, a shown texture, not a line). */
function visibleHit(h: THREE.Intersection): boolean {
  const o = h.object as THREE.Mesh;
  if (!o.isMesh) return false;
  for (let p: THREE.Object3D | null = o; p; p = p.parent) if (!p.visible) return false;
  const m = o.material;
  if (Array.isArray(m)) return h.face ? !!m[h.face.materialIndex]?.visible : false;
  return m.visible;
}

type Controls = { mouseButtons: Record<string, THREE.MOUSE | null | undefined>; enableRotate?: boolean } | null;

function Picker({ root, tiles, mode, busy, nav, onPick, onDraw, setDraft }: {
  root: React.RefObject<THREE.Group>;
  tiles: React.RefObject<THREE.Mesh>;
  mode: Mode;
  busy: boolean;
  nav: Nav;
  onPick: Props["onPick"];
  onDraw: Props["onDraw"];
  setDraft: (d: Draft) => void;
}) {
  const gl = useThree((s) => s.gl);
  const camera = useThree((s) => s.camera);
  const raycaster = useThree((s) => s.raycaster);
  const invalidate = useThree((s) => s.invalidate);
  const controls = useThree((s) => s.controls) as unknown as Controls;
  const latest = useRef({ mode, busy, nav, onPick, onDraw });
  latest.current = { mode, busy, nav, onPick, onDraw };

  // drawing: the left button draws (the view moves with the right button meanwhile)
  useEffect(() => {
    const c = controls;
    if (!c?.mouseButtons || mode !== "draw") return;
    const prev = { ...c.mouseButtons };
    c.mouseButtons = { ...prev, LEFT: null, RIGHT: THREE.MOUSE.PAN };
    return () => {
      c.mouseButtons = prev;
    };
  }, [controls, mode]);

  useEffect(() => {
    const el = gl.domElement;
    const ndc = (e: PointerEvent) => {
      const r = el.getBoundingClientRect();
      return new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
    };
    const cast = (e: PointerEvent, what: THREE.Object3D[]) => {
      raycaster.setFromCamera(ndc(e), camera);
      return raycaster.intersectObjects(what, true).filter(visibleHit);
    };
    const toGs = (v: THREE.Vector3): Vec3 => {
      const l = root.current!.worldToLocal(v.clone());
      return [l.x, l.y, l.z];
    };
    /** the area under the mouse (its id), if any */
    const areaAt = (e: PointerEvent): number | null => {
      if (!tiles.current) return null;
      const h = cast(e, [tiles.current])[0];
      if (h?.faceIndex == null) return null;
      return latest.current.nav.areas[Math.floor(h.faceIndex / 2)]?.id ?? null;
    };
    /** where the mouse meets the map (or the nav) */
    const surfaceAt = (e: PointerEvent): Vec3 | null => {
      if (!root.current) return null;
      const h = cast(e, root.current.children)[0];
      return h ? toGs(h.point) : null;
    };
    /** where the mouse's ray crosses the flat height z */
    const onPlane = (e: PointerEvent, z: number): Vec3 | null => {
      if (!root.current) return null;
      raycaster.setFromCamera(ndc(e), camera);
      const o = root.current.worldToLocal(raycaster.ray.origin.clone());
      const d = root.current.worldToLocal(raycaster.ray.origin.clone().add(raycaster.ray.direction)).sub(o);
      if (Math.abs(d.z) < 1e-6) return null;
      const t = (z - o.z) / d.z;
      return t > 0 ? [o.x + d.x * t, o.y + d.y * t, z] : null;
    };
    let down: { x: number; y: number; from: Vec3 | null } | null = null;
    let draft: Draft = null;
    const snap = (v: number) => Math.round(v);
    const press = (e: PointerEvent) => {
      if (e.button !== 0) return;
      const { mode: m, busy: b } = latest.current;
      down = { x: e.clientX, y: e.clientY, from: null };
      if (m === "draw" && !b) {
        const p = surfaceAt(e);
        if (p) {
          down.from = [snap(p[0]), snap(p[1]), Math.round(p[2] * 8) / 8];
          draft = { a: down.from, b: down.from };
          setDraft(draft);
          invalidate();
        }
      }
    };
    const move = (e: PointerEvent) => {
      if (down?.from) {
        const b = onPlane(e, down.from[2]);
        if (b) {
          draft = { a: down.from, b: [snap(b[0]), snap(b[1]), down.from[2]] };
          setDraft(draft);
          invalidate();
        }
        return;
      }
      if (e.buttons) return;
      // hover: the area and the point under the mouse, in the status bar
      const id = areaAt(e);
      const p = surfaceAt(e);
      useTools.getState().setHover(id !== null ? `Nav area ${id}` : p ? "Map" : null, p);
    };
    const release = (e: PointerEvent) => {
      if (e.button !== 0 || !down) return;
      const moved = Math.hypot(e.clientX - down.x, e.clientY - down.y) > 5;
      const { mode: m, onPick: pick, onDraw: drew } = latest.current;
      if (down.from) {
        const d = draft;
        setDraft(null);
        invalidate();
        if (d && moved) {
          const x: [number, number] = [Math.min(d.a[0], d.b[0]), Math.max(d.a[0], d.b[0])];
          const y: [number, number] = [Math.min(d.a[1], d.b[1]), Math.max(d.a[1], d.b[1])];
          if (x[1] - x[0] >= 2 && y[1] - y[0] >= 2) drew(x, y, d.a[2]);
        }
      } else if (!moved && m !== "draw") {
        pick(areaAt(e), e.shiftKey);
      }
      down = null;
      draft = null;
    };
    el.addEventListener("pointerdown", press);
    el.addEventListener("pointermove", move);
    window.addEventListener("pointerup", release);
    return () => {
      el.removeEventListener("pointerdown", press);
      el.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", release);
    };
  }, [gl, camera, raycaster, root, tiles, setDraft, invalidate]);
  return null;
}
