// The nav fixer's 3D view: the map (B's World, reused as it is) with the nav drawn editable on
// top, and the fixer's own cameras (FixCamera). Clicks are picked here, against the canvas, so
// drawing a box on the floor works where there's no nav yet (a beam the learner missed):
//  Select  click an area (Shift adds to the selection)
//  Draw    drag on the floor: a box at the height you started on, snapped to 8 units and to
//          the edges of areas next to it (Alt: no snapping)
//  Link    click A, then B (Shift: one way, a drop)
// Two of these side by side (3D and top-down) share what's under the mouse through useFix.

import { Canvas, useThree } from "@react-three/fiber";
import { forwardRef, useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { useTools, type NavColour } from "../../../stores/tools";
import type { Nav } from "../overlays";
import type { MeshBundle } from "../viewer/bundle";
import { ROOT_ROTATION, type Vec3 } from "../viewer/coords";
import { pointerEvents } from "../viewer/Scene";
import { areaColours } from "../viewer/NavLayer";
import { World, type ModelMesh } from "../viewer/World";
import { REACH_FALLS, REACH_NEVER, REACH_TRAP } from "./navfixApi";
import { FixCamera, visibleHit } from "./FixCamera";
import { useFix, type LinkShow, type Mode, type ViewKind } from "./fixStore";

export type { Mode } from "./fixStore";
export type ColourBy = "reach" | Exclude<NavColour, "island">;
/** the box being drawn; `snap` says which of a.x, a.y, b.x, b.y landed on an area's edge */
export type Draft = { a: Vec3; b: Vec3; snap: [boolean, boolean, boolean, boolean] } | null;

const LIFT = 3;
const C = {
  ok: new THREE.Color("#6fc2b0"),
  never: new THREE.Color("#e2574c"),
  trap: new THREE.Color("#e8a23c"),
  falls: new THREE.Color("#d9c75a"),
  picked: new THREE.Color("#fff6b0"),
  from: new THREE.Color("#7fd1ff"),
  problem: new THREE.Color("#ff5fd2"),
};
/** link colors (also in navfix.css for the legend) */
export const LINK = { both: "#e9f0e4", out: "#ff9a3c", into: "#ff5fd2", band: "#7fd1ff" };

type Props = {
  bundle: MeshBundle;
  meshes: ModelMesh[];
  map: string;
  view: ViewKind;
  nav: Nav;
  reach: number[];
  colourBy: ColourBy;
  links: LinkShow;
  mode: Mode;
  oneWay: boolean;
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
      onPointerLeave={() => {
        useTools.getState().setHover(null);
        useFix.getState().setHover(null, null, null, null);
      }}
      data-view={p.view}
    >
      <color attach="background" args={["#262c21"]} />
      <hemisphereLight args={["#ffffff", "#5a5f50", 0.9]} />
      <directionalLight position={[0.4, 1, 0.25]} intensity={1.6} />
      <Root {...p} />
      <FixCamera view={p.view} map={p.map} bounds={p.bundle.bounds} />
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
      <Focused nav={p.nav} selected={p.selected} linkFrom={p.linkFrom} />
      <Hovered nav={p.nav} view={p.view} />
      {p.mode === "link" && p.linkFrom !== null && <LinkBand nav={p.nav} from={p.linkFrom} oneWay={p.oneWay} />}
      {draft && <DraftBox draft={draft} />}
      <Picker root={root} tiles={tiles} view={p.view} nav={p.nav} onPick={p.onPick} onDraw={p.onDraw} setDraft={setDraft} />
    </group>
  );
}

// ---- lines -------------------------------------------------------------------------------------

/** A line from a to b with an arrowhead part way along (at `at`, 0..1), flat on the floor. */
function arrow(out: number[], a: Vec3, b: Vec3, at = 0.62, head = true) {
  out.push(...a, ...b);
  if (!head) return;
  const dx = b[0] - a[0], dy = b[1] - a[1];
  const len = Math.hypot(dx, dy);
  if (len < 4) return;
  const ux = dx / len, uy = dy / len;
  const s = Math.max(6, Math.min(22, len / 4));
  const tip: Vec3 = [a[0] + dx * at, a[1] + dy * at, a[2] + (b[2] - a[2]) * at];
  for (const side of [1, -1]) out.push(...tip, tip[0] - ux * s - uy * s * 0.55 * side, tip[1] - uy * s + ux * s * 0.55 * side, tip[2]);
}

function lineGeo(v: number[]) {
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.Float32BufferAttribute(v, 3));
  return g;
}

const lifted = (c: Vec3, k = 2): Vec3 => [c[0], c[1], c[2] + LIFT * k];

/** An area's four edges as line segments, just above it. */
function outlineOf(a: Nav["areas"][number]): number[] {
  const z = LIFT * 1.5;
  const c: Vec3[] = [[a.nw[0], a.nw[1], a.nw[2] + z], [a.se[0], a.nw[1], a.neZ + z], [a.se[0], a.se[1], a.se[2] + z], [a.nw[0], a.se[1], a.swZ + z]];
  return [0, 1, 1, 2, 2, 3, 3, 0].flatMap((k) => c[k]);
}

// ---- the nav -----------------------------------------------------------------------------------

const EditNav = forwardRef<THREE.Mesh, Props>(function EditNav({ nav, reach, colourBy, links, selected, linkFrom, shown }, ref) {
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
    // every link: both ways as a plain line, one way with an arrow (drops are always shown)
    const oneWay = new Set(nav.oneWay.map(([a, b]) => `${a}>${b}`));
    const two: number[] = [], one: number[] = [];
    const seen = new Set<string>();
    nav.areas.forEach((a, i) => {
      for (const id of a.conns.flat()) {
        const j = nav.index.get(id);
        if (j === undefined) continue;
        if (oneWay.has(`${a.id}>${id}`)) arrow(one, lifted(nav.centres[i]), lifted(nav.centres[j]));
        else {
          const key = a.id < id ? `${a.id}-${id}` : `${id}-${a.id}`;
          if (seen.has(key)) continue;
          seen.add(key);
          two.push(...lifted(nav.centres[i]), ...lifted(nav.centres[j]));
        }
      }
    });
    const lad: number[] = [];
    for (const l of nav.ladders) lad.push(...l.bottom, ...l.top);
    return { g, e, two: lineGeo(two), one: lineGeo(one), lad: lineGeo(lad) };
  }, [nav]);
  useEffect(() => () => Object.values(geo).forEach((g) => g.dispose()), [geo]);

  useEffect(() => {
    const base = colourBy === "reach" ? null : areaColours(nav, colourBy, null, "off");
    const attr = geo.g.getAttribute("color") as THREE.BufferAttribute;
    nav.areas.forEach((a, i) => {
      let col = base ? base[i] : reach[i] === REACH_NEVER ? C.never : reach[i] === REACH_TRAP ? C.trap : reach[i] === REACH_FALLS ? C.falls : C.ok;
      if (shown.has(a.id)) col = C.problem;
      if (selected.has(a.id)) col = C.picked;
      if (a.id === linkFrom) col = C.from;
      for (let k = 0; k < 4; k++) attr.setXYZ(i * 4 + k, col.r, col.g, col.b);
    });
    attr.needsUpdate = true;
    invalidate();
  }, [nav, reach, colourBy, selected, linkFrom, shown, geo, invalidate]);
  useEffect(() => invalidate(), [links, invalidate]);

  return (
    <group>
      <mesh ref={ref} geometry={geo.g} renderOrder={2} userData={{ navTiles: true }}>
        <meshBasicMaterial vertexColors transparent opacity={0.5} depthWrite={false} side={THREE.DoubleSide} polygonOffset polygonOffsetFactor={-2} polygonOffsetUnits={-2} />
      </mesh>
      <lineSegments geometry={geo.e} renderOrder={3} raycast={() => null}>
        <lineBasicMaterial color="#1c2018" transparent opacity={0.6} />
      </lineSegments>
      <lineSegments geometry={geo.two} renderOrder={3} raycast={() => null} visible={links === "all"}>
        <lineBasicMaterial color={LINK.both} transparent opacity={0.35} />
      </lineSegments>
      <lineSegments geometry={geo.one} renderOrder={4} raycast={() => null}>
        {/* (behind walls only when they're the ones you're working on: Focused) */}
        <lineBasicMaterial color={LINK.out} transparent opacity={links === "all" ? 0.95 : 0.75} />
      </lineSegments>
      <lineSegments geometry={geo.lad} renderOrder={4} raycast={() => null}>
        <lineBasicMaterial color="#7fd1ff" />
      </lineSegments>
    </group>
  );
});

/** The links of the areas you're working on (hovered, picked, linking from), bright and on top:
 *  both ways white, one way out orange with an arrow, one way in (a drop into it) pink. */
function Focused({ nav, selected, linkFrom }: { nav: Nav; selected: Set<number>; linkFrom: number | null }) {
  const hover = useFix((s) => s.hoverArea);
  const invalidate = useThree((s) => s.invalidate);
  const geo = useMemo(() => {
    const ids = new Set(selected);
    if (hover !== null) ids.add(hover);
    if (linkFrom !== null) ids.add(linkFrom);
    const oneWay = new Set(nav.oneWay.map(([a, b]) => `${a}>${b}`));
    const both: number[] = [], out: number[] = [], into: number[] = [], picked: number[] = [];
    // the picked areas' outlines, seen through roofs and walls (top-down under a ceiling)
    for (const id of selected) {
      const i = nav.index.get(id);
      if (i !== undefined && selected.size <= 400) picked.push(...outlineOf(nav.areas[i]));
    }
    if (ids.size > 60) return { both: lineGeo(both), out: lineGeo(out), into: lineGeo(into), picked: lineGeo(picked) };
    for (const id of ids) {
      const i = nav.index.get(id);
      if (i === undefined) continue;
      const a = nav.areas[i];
      const c = lifted(nav.centres[i], 3);
      for (const to of a.conns.flat()) {
        const j = nav.index.get(to);
        if (j === undefined) continue;
        const d = lifted(nav.centres[j], 3);
        if (oneWay.has(`${id}>${to}`)) arrow(out, c, d, 0.7);
        else arrow(both, c, d, 0.5, false);
      }
      for (const [x, y] of nav.oneWay) {
        if (y !== id) continue;
        const j = nav.index.get(x);
        if (j !== undefined) arrow(into, lifted(nav.centres[j], 3), c, 0.7);
      }
    }
    return { both: lineGeo(both), out: lineGeo(out), into: lineGeo(into), picked: lineGeo(picked) };
  }, [nav, selected, hover, linkFrom]);
  useEffect(() => {
    invalidate();
    return () => Object.values(geo).forEach((g) => g.dispose());
  }, [geo, invalidate]);
  return (
    <group>
      <lineSegments geometry={geo.picked} renderOrder={7} raycast={() => null}>
        <lineBasicMaterial color="#fff6b0" depthTest={false} />
      </lineSegments>
      <lineSegments geometry={geo.both} renderOrder={7} raycast={() => null}>
        <lineBasicMaterial color={LINK.both} depthTest={false} transparent opacity={0.95} />
      </lineSegments>
      <lineSegments geometry={geo.out} renderOrder={7} raycast={() => null}>
        <lineBasicMaterial color={LINK.out} depthTest={false} />
      </lineSegments>
      <lineSegments geometry={geo.into} renderOrder={7} raycast={() => null}>
        <lineBasicMaterial color={LINK.into} depthTest={false} />
      </lineSegments>
    </group>
  );
}

/** The area under the mouse, outlined (in both views), and in the other view a marker where the
 *  mouse is. */
function Hovered({ nav, view }: { nav: Nav; view: ViewKind }) {
  const area = useFix((s) => s.hoverArea);
  const point = useFix((s) => s.hoverPoint);
  const from = useFix((s) => s.hoverView);
  const invalidate = useThree((s) => s.invalidate);
  const outline = useMemo(() => {
    const i = area === null ? undefined : nav.index.get(area);
    return i === undefined ? null : lineGeo(outlineOf(nav.areas[i]));
  }, [nav, area]);
  const marker = useMemo(() => {
    if (!point || from === view || from === null) return null;
    const [x, y, z] = point;
    const s = 24;
    return lineGeo([x - s, y, z + 2, x + s, y, z + 2, x, y - s, z + 2, x, y + s, z + 2, x, y, z, x, y, z + 72]);
  }, [point, from, view]);
  useEffect(() => {
    invalidate();
    return () => outline?.dispose();
  }, [outline, invalidate]);
  useEffect(() => {
    invalidate();
    return () => marker?.dispose();
  }, [marker, invalidate]);
  return (
    <>
      {outline && (
        <lineSegments geometry={outline} renderOrder={8} raycast={() => null}>
          <lineBasicMaterial color="#ffffff" depthTest={false} />
        </lineSegments>
      )}
      {marker && (
        <lineSegments geometry={marker} renderOrder={8} raycast={() => null}>
          <lineBasicMaterial color="#ffe14d" depthTest={false} />
        </lineSegments>
      )}
    </>
  );
}

/** Linking: a line from the area you're linking from to the mouse (an arrow when one way). */
function LinkBand({ nav, from, oneWay }: { nav: Nav; from: number; oneWay: boolean }) {
  const area = useFix((s) => s.hoverArea);
  const point = useFix((s) => s.hoverPoint);
  const invalidate = useThree((s) => s.invalidate);
  const geo = useMemo(() => {
    const i = nav.index.get(from);
    if (i === undefined) return null;
    const j = area !== null && area !== from ? nav.index.get(area) : undefined;
    const end = j !== undefined ? nav.centres[j] : point;
    if (!end) return null;
    const v: number[] = [];
    arrow(v, lifted(nav.centres[i], 4), lifted(end, 4), 0.75, true);
    if (!oneWay) arrow(v, lifted(end, 4), lifted(nav.centres[i], 4), 0.75, true);
    return lineGeo(v);
  }, [nav, from, area, point, oneWay]);
  useEffect(() => {
    invalidate();
    return () => geo?.dispose();
  }, [geo, invalidate]);
  if (!geo) return null;
  return (
    <lineSegments geometry={geo} renderOrder={9} raycast={() => null}>
      <lineBasicMaterial color={oneWay ? LINK.out : LINK.band} depthTest={false} />
    </lineSegments>
  );
}

function DraftBox({ draft }: { draft: NonNullable<Draft> }) {
  const geo = useMemo(() => {
    const { a, b, snap } = draft;
    const z = a[2] + LIFT * 2;
    // each edge, colored by whether it snapped to a neighbor (edges at a.x, b.x, a.y, b.y)
    const plain: number[] = [], snapped: number[] = [];
    const edge = (on: boolean, v: number[]) => (on ? snapped : plain).push(...v);
    edge(snap[0], [a[0], a[1], z, a[0], b[1], z]);
    edge(snap[2], [b[0], a[1], z, b[0], b[1], z]);
    edge(snap[1], [a[0], a[1], z, b[0], a[1], z]);
    edge(snap[3], [a[0], b[1], z, b[0], b[1], z]);
    const fill = new THREE.BufferGeometry();
    fill.setAttribute("position", new THREE.Float32BufferAttribute([a[0], a[1], z, b[0], a[1], z, b[0], b[1], z, a[0], a[1], z, b[0], b[1], z, a[0], b[1], z], 3));
    return { plain: lineGeo(plain), snapped: lineGeo(snapped), fill };
  }, [draft]);
  useEffect(() => () => Object.values(geo).forEach((g) => g.dispose()), [geo]);
  return (
    <>
      <mesh geometry={geo.fill} renderOrder={5} raycast={() => null}>
        <meshBasicMaterial color="#fff6b0" transparent opacity={0.22} depthTest={false} side={THREE.DoubleSide} />
      </mesh>
      <lineSegments geometry={geo.plain} renderOrder={6} raycast={() => null}>
        <lineBasicMaterial color="#fff6b0" depthTest={false} />
      </lineSegments>
      <lineSegments geometry={geo.snapped} renderOrder={6} raycast={() => null}>
        <lineBasicMaterial color="#5ff0ff" depthTest={false} />
      </lineSegments>
    </>
  );
}

// ---- picking -----------------------------------------------------------------------------------

const GRID = 8;

function Picker({ root, tiles, view, nav, onPick, onDraw, setDraft }: {
  root: React.RefObject<THREE.Group>;
  tiles: React.RefObject<THREE.Mesh>;
  view: ViewKind;
  nav: Nav;
  onPick: Props["onPick"];
  onDraw: Props["onDraw"];
  setDraft: (d: Draft) => void;
}) {
  const gl = useThree((s) => s.gl);
  const camera = useThree((s) => s.camera);
  const raycaster = useThree((s) => s.raycaster);
  const invalidate = useThree((s) => s.invalidate);
  const latest = useRef({ nav, onPick, onDraw });
  latest.current = { nav, onPick, onDraw };

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
    /** screen pixels per map unit at a point (for snapping by distance on screen) */
    const pxPerUnit = (p: Vec3) => {
      if (!root.current) return 1;
      const r = el.getBoundingClientRect();
      const a = root.current.localToWorld(new THREE.Vector3(...p)).project(camera);
      const b = root.current.localToWorld(new THREE.Vector3(p[0] + 16, p[1], p[2])).project(camera);
      const c = root.current.localToWorld(new THREE.Vector3(p[0], p[1] + 16, p[2])).project(camera);
      const px = (u: THREE.Vector3) => Math.hypot(((u.x - a.x) * r.width) / 2, ((u.y - a.y) * r.height) / 2);
      return Math.max(px(b), px(c)) / 16 || 1;
    };
    /** a corner snapped: to the edge of an area at about this height within ~10 pixels, else
     *  to the 8-unit grid (Alt: neither) */
    const snapPoint = (p: Vec3, z: number, free: boolean, from?: Vec3): { v: Vec3; sx: boolean; sy: boolean } => {
      if (free) return { v: [Math.round(p[0]), Math.round(p[1]), z], sx: false, sy: false };
      const thr = Math.max(2, Math.min(32, 10 / pxPerUnit(p)));
      // (the far corner never snaps onto the near one's edge: no box of no width)
      const ok = (v: number, k: 0 | 1) => !from || Math.abs(v - from[k]) >= GRID;
      let bx = thr, by = thr, x = Math.round(p[0] / GRID) * GRID, y = Math.round(p[1] / GRID) * GRID, sx = false, sy = false;
      for (const a of latest.current.nav.areas) {
        const lo = Math.min(a.nw[2], a.se[2], a.neZ, a.swZ), hi = Math.max(a.nw[2], a.se[2], a.neZ, a.swZ);
        if (z < lo - 48 || z > hi + 48) continue;
        const x0 = Math.min(a.nw[0], a.se[0]), x1 = Math.max(a.nw[0], a.se[0]), y0 = Math.min(a.nw[1], a.se[1]), y1 = Math.max(a.nw[1], a.se[1]);
        if (p[1] > y0 - 64 && p[1] < y1 + 64)
          for (const ex of [x0, x1]) {
            const d = Math.abs(p[0] - ex);
            if (d < bx && ok(ex, 0)) (bx = d), (x = ex), (sx = true);
          }
        if (p[0] > x0 - 64 && p[0] < x1 + 64)
          for (const ey of [y0, y1]) {
            const d = Math.abs(p[1] - ey);
            if (d < by && ok(ey, 1)) (by = d), (y = ey), (sy = true);
          }
      }
      return { v: [x, y, z], sx, sy };
    };
    let down: { x: number; y: number; from: Vec3 | null; snap: [boolean, boolean] } | null = null;
    let draft: Draft = null;
    const show = (d: Draft) => {
      draft = d;
      setDraft(d);
      useFix.getState().setDraft(d ? { w: Math.abs(d.b[0] - d.a[0]), h: Math.abs(d.b[1] - d.a[1]), z: d.a[2], snapped: d.snap.some(Boolean) } : null);
      invalidate();
    };
    const press = (e: PointerEvent) => {
      if (e.button !== 0) return;
      const { mode: m, busy: b } = useFix.getState();
      down = { x: e.clientX, y: e.clientY, from: null, snap: [false, false] };
      if (m === "draw" && !b && !e.shiftKey) {
        const p = surfaceAt(e);
        if (p) {
          const z = Math.round(p[2] * 8) / 8;
          const s = snapPoint(p, z, e.altKey);
          down.from = s.v;
          down.snap = [s.sx, s.sy];
          show({ a: s.v, b: s.v, snap: [s.sx, s.sy, false, false] });
        }
      }
    };
    const move = (e: PointerEvent) => {
      const mouse = { x: e.clientX, y: e.clientY };
      if (down?.from) {
        const b = onPlane(e, down.from[2]);
        if (b) {
          const s = snapPoint(b, down.from[2], e.altKey, down.from);
          show({ a: down.from, b: s.v, snap: [down.snap[0], down.snap[1], s.sx, s.sy] });
          useFix.getState().setHover(null, s.v, view, mouse);
        }
        return;
      }
      if (e.buttons) {
        useFix.getState().setHover(useFix.getState().hoverArea, useFix.getState().hoverPoint, view, mouse);
        return;
      }
      // hover: the area and the point under the mouse, in the status bar and both views
      const id = areaAt(e);
      const p = surfaceAt(e);
      useTools.getState().setHover(id !== null ? `Nav area ${id}` : p ? "Map" : null, p);
      useFix.getState().setHover(id, p, view, mouse);
    };
    const release = (e: PointerEvent) => {
      if (e.button !== 0 || !down) return;
      const moved = Math.hypot(e.clientX - down.x, e.clientY - down.y) > 5;
      const { onPick: pick, onDraw: drew } = latest.current;
      const m = useFix.getState().mode;
      if (down.from) {
        const d = draft;
        show(null);
        if (d && moved) {
          const x: [number, number] = [Math.min(d.a[0], d.b[0]), Math.max(d.a[0], d.b[0])];
          const y: [number, number] = [Math.min(d.a[1], d.b[1]), Math.max(d.a[1], d.b[1])];
          if (x[1] - x[0] >= 2 && y[1] - y[0] >= 2) drew(x, y, d.a[2]);
        }
      } else if (!moved && m !== "draw" && e.target === el) {
        pick(areaAt(e), e.shiftKey);
      }
      down = null;
      draft = null;
    };
    const cancel = (e: KeyboardEvent) => {
      if (e.key === "Escape" && down?.from) {
        down = null;
        show(null);
        e.stopImmediatePropagation();
      }
    };
    el.addEventListener("pointerdown", press);
    el.addEventListener("pointermove", move);
    window.addEventListener("pointerup", release);
    window.addEventListener("keydown", cancel, true);
    return () => {
      el.removeEventListener("pointerdown", press);
      el.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", release);
      window.removeEventListener("keydown", cancel, true);
    };
  }, [gl, camera, raycaster, root, tiles, view, setDraft, invalidate]);
  return null;
}
