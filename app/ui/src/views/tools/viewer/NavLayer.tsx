// The bots' nav over the map (plan M4): areas as see-through tiles coloured by place, by
// connected group (islands stand out) or by the moves they ask for; links between areas
// (one-way ones in orange with an arrowhead), hiding spots, ladders and place names.
// With knowledge on, areas can be tinted by what the bots learned (KnowLayer).

import { Html } from "@react-three/drei";
import { useThree, type ThreeEvent } from "@react-three/fiber";
import { useEffect, useMemo } from "react";
import * as THREE from "three";
import { useTools, type KnowMode, type NavColour } from "../../../stores/tools";
import type { Know, Nav } from "../overlays";
import { notAPick } from "./Cameras";

const LIFT = 3;
const MAIN = new THREE.Color("#6fc2b0");
const ISLAND = new THREE.Color("#e2574c");

/** A colour per index that's easy to tell from its neighbours (golden-angle hues). */
const hue = (i: number, s = 0.55, l = 0.55) => new THREE.Color().setHSL(((i * 137.508) % 360) / 360, s, l);

/** Each area's colour for the current view. */
export function areaColours(nav: Nav, mode: NavColour, know: Know | null, knowMode: KnowMode): THREE.Color[] {
  const ramp = (v: number) => new THREE.Color().setHSL(0.33 * (1 - Math.min(1, Math.max(0, v))), 0.7, 0.5);
  const usable = know && know.count === nav.areas.length && know.keyMatches !== false;
  if (usable && knowMode === "cover") {
    // how open an area is: the mean open distance at standing height (short = covered)
    return nav.areas.map((_, i) => {
      let sum = 0;
      for (let d = 0; d < 16; d++) sum += Math.min(1024, know.coverHigh[i * 16 + d]);
      return ramp(1 - sum / 16 / 1024);
    });
  }
  if (usable && knowMode === "sight") {
    let max = 1;
    for (let i = 0; i < know.count; i++) max = Math.max(max, know.sees[i]);
    return nav.areas.map((_, i) => ramp(1 - know.sees[i] / max));
  }
  return nav.areas.map((a, i) => {
    if (mode === "place") return a.place ? hue(a.place) : new THREE.Color("#8a8f80");
    if (mode === "moves") {
      const b = nav.attrBits;
      if (a.attr & b.crouch) return new THREE.Color("#6b9de8");
      if (a.attr & b.jump) return new THREE.Color("#e8c45a");
      if (a.attr & b.precise) return new THREE.Color("#b07de0");
      if (a.attr & b.noJump) return new THREE.Color("#e0844f");
      return new THREE.Color("#7f9a78");
    }
    return nav.island[i] === 0 ? MAIN : ISLAND;
  });
}

export function NavLayer({ nav, know }: { nav: Nav; know: Know | null }) {
  const mode = useTools((s) => s.navColour);
  const knowMode = useTools((s) => s.know);
  const sel = useTools((s) => s.selection);
  const invalidate = useThree((s) => s.invalidate);
  const picked = sel?.kind === "area" ? sel.index : -1;

  // the tiles: two triangles per area on its four corner heights
  const tiles = useMemo(() => {
    const n = nav.areas.length;
    const pos = new Float32Array(n * 4 * 3);
    const idx = new Uint32Array(n * 6);
    nav.areas.forEach((a, i) => {
      const [x0, y0, z0] = a.nw, [x1, y1, z1] = a.se;
      pos.set([x0, y0, z0 + LIFT, x1, y0, a.neZ + LIFT, x1, y1, z1 + LIFT, x0, y1, a.swZ + LIFT], i * 12);
      const v = i * 4;
      // counter-clockwise seen from above (+z)
      idx.set([v, v + 3, v + 2, v, v + 2, v + 1], i * 6);
    });
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.BufferAttribute(pos, 3));
    g.setAttribute("color", new THREE.BufferAttribute(new Float32Array(n * 4 * 3), 3));
    g.setIndex(new THREE.BufferAttribute(idx, 1));
    g.computeBoundingSphere();
    // outlines, so neighbouring tiles of one colour still read as areas
    const edges = new Float32Array(n * 8 * 3);
    nav.areas.forEach((a, i) => {
      const c = [0, 1, 2, 3].map((k) => [pos[i * 12 + k * 3], pos[i * 12 + k * 3 + 1], pos[i * 12 + k * 3 + 2]]);
      [0, 1, 1, 2, 2, 3, 3, 0].forEach((k, j) => edges.set(c[k], (i * 8 + j) * 3));
    });
    const e = new THREE.BufferGeometry();
    e.setAttribute("position", new THREE.BufferAttribute(edges, 3));
    return { g, e };
  }, [nav]);
  useEffect(() => () => {
    tiles.g.dispose();
    tiles.e.dispose();
  }, [tiles]);

  // colours, with the picked area and its neighbours brighter
  useEffect(() => {
    const cols = areaColours(nav, mode, know, knowMode);
    const near = new Set<number>();
    if (picked >= 0) nav.areas[picked]?.conns.flat().forEach((id) => near.add(nav.index.get(id) ?? -1));
    const attr = tiles.g.getAttribute("color") as THREE.BufferAttribute;
    const white = new THREE.Color("#fff6b0");
    cols.forEach((c, i) => {
      const col = i === picked ? white : near.has(i) ? c.clone().lerp(white, 0.45) : c;
      for (let k = 0; k < 4; k++) attr.setXYZ(i * 4 + k, col.r, col.g, col.b);
    });
    attr.needsUpdate = true;
    invalidate();
  }, [nav, mode, know, knowMode, picked, tiles, invalidate]);

  // links between area centres; one-way ones orange with an arrowhead at the far end
  const links = useMemo(() => {
    const oneWay = new Set(nav.oneWay.map(([a, b]) => `${a}>${b}`));
    const two: number[] = [], one: number[] = [];
    const seen = new Set<string>();
    nav.areas.forEach((a, i) => {
      const p = nav.centres[i];
      for (const id of a.conns.flat()) {
        const j = nav.index.get(id);
        if (j === undefined) continue;
        const q = nav.centres[j];
        if (oneWay.has(`${a.id}>${id}`)) {
          one.push(p[0], p[1], p[2] + LIFT * 2, q[0], q[1], q[2] + LIFT * 2);
          // arrowhead: two short strokes back from the end
          const dx = q[0] - p[0], dy = q[1] - p[1];
          const len = Math.hypot(dx, dy) || 1;
          const ux = dx / len, uy = dy / len, s = Math.min(24, len / 3);
          for (const side of [1, -1]) one.push(q[0], q[1], q[2] + LIFT * 2, q[0] - ux * s - uy * s * 0.5 * side, q[1] - uy * s + ux * s * 0.5 * side, q[2] + LIFT * 2);
        } else {
          const key = a.id < id ? `${a.id}-${id}` : `${id}-${a.id}`;
          if (seen.has(key)) continue;
          seen.add(key);
          two.push(p[0], p[1], p[2] + LIFT * 2, q[0], q[1], q[2] + LIFT * 2);
        }
      }
    });
    const make = (v: number[]) => {
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.Float32BufferAttribute(v, 3));
      return g;
    };
    return { two: make(two), one: make(one) };
  }, [nav]);
  useEffect(() => () => {
    links.two.dispose();
    links.one.dispose();
  }, [links]);

  // hiding spots as dots (sniper spots brighter), ladders as posts
  const extras = useMemo(() => {
    const spots: number[] = [], cols: number[] = [];
    const c = new THREE.Color();
    for (const a of nav.areas) for (const h of a.hiding) {
      spots.push(h.pos[0], h.pos[1], h.pos[2] + 8);
      c.set(h.flags & (nav.spotFlags.idealSniper | nav.spotFlags.goodSniper) ? "#f2d04b" : h.flags & nav.spotFlags.inCover ? "#9fd18b" : "#d8ded3");
      cols.push(c.r, c.g, c.b);
    }
    const sg = new THREE.BufferGeometry();
    sg.setAttribute("position", new THREE.Float32BufferAttribute(spots, 3));
    sg.setAttribute("color", new THREE.Float32BufferAttribute(cols, 3));
    const lad: number[] = [];
    for (const l of nav.ladders) lad.push(...l.bottom, ...l.top);
    const lg = new THREE.BufferGeometry();
    lg.setAttribute("position", new THREE.Float32BufferAttribute(lad, 3));
    return { sg, lg };
  }, [nav]);
  useEffect(() => () => {
    extras.sg.dispose();
    extras.lg.dispose();
  }, [extras]);

  // place names at the middle of their areas
  const labels = useMemo(() => {
    const acc = new Map<number, [number, number, number, number]>();
    nav.areas.forEach((a, i) => {
      if (!a.place) return;
      const s = acc.get(a.place) ?? [0, 0, 0, 0];
      const c = nav.centres[i];
      acc.set(a.place, [s[0] + c[0], s[1] + c[1], s[2] + c[2], s[3] + 1]);
    });
    return [...acc.entries()].slice(0, 60).map(([p, s]) => ({ name: nav.places[p - 1] ?? `#${p}`, at: [s[0] / s[3], s[1] / s[3], s[2] / s[3] + 40] as [number, number, number] }));
  }, [nav]);

  const pick = (e: ThreeEvent<MouseEvent>) => {
    if (notAPick(e) || e.faceIndex == null) return;
    e.stopPropagation();
    useTools.getState().select({ kind: "area", index: Math.floor(e.faceIndex / 2) });
  };
  const hover = (e: ThreeEvent<PointerEvent>) => {
    if (e.faceIndex == null) return;
    e.stopPropagation();
    const a = nav.areas[Math.floor(e.faceIndex / 2)];
    if (!a) return;
    const place = a.place ? nav.places[a.place - 1] : "";
    const p = (e.object.parent ?? e.object).worldToLocal(e.point.clone());
    useTools.getState().setHover(`Nav area ${a.id}${place ? ` · ${place}` : ""}`, [p.x, p.y, p.z]);
  };

  return (
    <group>
      <mesh geometry={tiles.g} onClick={pick} onPointerMove={hover} onPointerOut={() => useTools.getState().setHover(null)} renderOrder={2}>
        <meshBasicMaterial vertexColors transparent opacity={0.42} depthWrite={false} side={THREE.DoubleSide} polygonOffset polygonOffsetFactor={-2} polygonOffsetUnits={-2} />
      </mesh>
      <lineSegments geometry={tiles.e} renderOrder={3} raycast={() => null}>
        <lineBasicMaterial color="#1c2018" transparent opacity={0.55} />
      </lineSegments>
      <lineSegments geometry={links.two} renderOrder={3} raycast={() => null}>
        <lineBasicMaterial color="#d8ded3" transparent opacity={0.35} />
      </lineSegments>
      <lineSegments geometry={links.one} renderOrder={3} raycast={() => null}>
        <lineBasicMaterial color="#f08a3c" transparent opacity={0.95} depthTest={false} />
      </lineSegments>
      <points geometry={extras.sg} renderOrder={4} raycast={() => null}>
        <pointsMaterial vertexColors size={5} sizeAttenuation={false} />
      </points>
      <lineSegments geometry={extras.lg} renderOrder={4} raycast={() => null}>
        <lineBasicMaterial color="#7fd1ff" />
      </lineSegments>
      {labels.map((l) => (
        <Html key={l.name + l.at.join()} position={l.at} center zIndexRange={[5, 0]} style={{ pointerEvents: "none" }}>
          <span className="tools-place">{l.name}</span>
        </Html>
      ))}
    </group>
  );
}
