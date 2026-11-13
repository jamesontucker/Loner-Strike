// P5: drawing big maps fast. Every brush that isn't picked is drawn as part of one mesh per
// texture and one set of outlines (a 3000-brush map: ~60 draws instead of ~9000), rebuilt only
// when the brushes, what's picked or hidden, or the drawing style change. Each brush keeps an
// invisible mesh of its own so clicking still finds the brush and its face.

import { useEffect, useMemo } from "react";
import * as THREE from "three";
import type { Draw } from "../viewer/bundle";
import type { MapMaterials } from "../viewer/materials";
import { flatMaterial, edgeLook, type RenderMode } from "./renderModes";
import type { BrushGeo } from "./store";

const drawOf = (texture: string): Draw => ({ model: 0, texture, start: 0, count: 0, flags: 0 });
const isSky = (t: string) => /^sky$/i.test(t);

const edgeCache = new WeakMap<BrushGeo, Float32Array>();
/** A brush's outline segments (cached with the brush's mesh). */
function edgesOf(b: BrushGeo): Float32Array {
  let e = edgeCache.get(b);
  if (!e) {
    const g = new THREE.EdgesGeometry(b.geometry, 20);
    e = (g.getAttribute("position").array as Float32Array).slice();
    g.dispose();
    edgeCache.set(b, e);
  }
  return e;
}

type Built = { meshes: { texture: string; geometry: THREE.BufferGeometry }[]; world: THREE.BufferGeometry; tied: THREE.BufferGeometry };

/** Merges the brushes' triangles by texture, and their outlines (world and entity brushes apart). */
export function mergeBrushes(list: BrushGeo[], hideSky: boolean): Built {
  const parts = new Map<string, { n: number; chunks: { b: BrushGeo; start: number; count: number }[] }>();
  let worldLen = 0;
  let tiedLen = 0;
  for (const b of list) {
    if (hideSky && b.textures.every(isSky)) continue;
    for (const g of b.geometry.groups) {
      const t = b.textures[g.materialIndex ?? 0] ?? "";
      let p = parts.get(t);
      if (!p) parts.set(t, (p = { n: 0, chunks: [] }));
      p.chunks.push({ b, start: g.start, count: g.count });
      p.n += g.count;
    }
    const e = edgesOf(b);
    if (b.entity != null) tiedLen += e.length;
    else worldLen += e.length;
  }
  const meshes: Built["meshes"] = [];
  for (const [texture, p] of parts) {
    const P = new Float32Array(p.n * 3);
    const N = new Float32Array(p.n * 3);
    const U = new Float32Array(p.n * 2);
    let at = 0;
    for (const c of p.chunks) {
      const pos = c.b.geometry.getAttribute("position").array as Float32Array;
      const nor = c.b.geometry.getAttribute("normal").array as Float32Array;
      const uv = c.b.geometry.getAttribute("uv").array as Float32Array;
      P.set(pos.subarray(c.start * 3, (c.start + c.count) * 3), at * 3);
      N.set(nor.subarray(c.start * 3, (c.start + c.count) * 3), at * 3);
      U.set(uv.subarray(c.start * 2, (c.start + c.count) * 2), at * 2);
      at += c.count;
    }
    const geometry = new THREE.BufferGeometry();
    geometry.setAttribute("position", new THREE.BufferAttribute(P, 3));
    geometry.setAttribute("normal", new THREE.BufferAttribute(N, 3));
    geometry.setAttribute("uv", new THREE.BufferAttribute(U, 2));
    geometry.computeBoundingSphere();
    meshes.push({ texture, geometry });
  }
  const W = new Float32Array(worldLen);
  const T = new Float32Array(tiedLen);
  let wi = 0;
  let ti = 0;
  for (const b of list) {
    if (hideSky && b.textures.every(isSky)) continue;
    const e = edgesOf(b);
    if (b.entity != null) {
      T.set(e, ti);
      ti += e.length;
    } else {
      W.set(e, wi);
      wi += e.length;
    }
  }
  const world = new THREE.BufferGeometry();
  world.setAttribute("position", new THREE.BufferAttribute(W, 3));
  world.computeBoundingSphere();
  const tied = new THREE.BufferGeometry();
  tied.setAttribute("position", new THREE.BufferAttribute(T, 3));
  tied.computeBoundingSphere();
  return { meshes, world, tied };
}

/** The unpicked brushes, drawn merged; plus an invisible pick mesh for each. */
export function MergedBrushes({ list, mats, version, mode, hideSky }: { list: BrushGeo[]; mats: MapMaterials; version: number; mode: RenderMode; hideSky: boolean }) {
  const built = useMemo(() => mergeBrushes(list, hideSky), [list, hideSky]);
  useEffect(
    () => () => {
      built.meshes.forEach((m) => m.geometry.dispose());
      built.world.dispose();
      built.tied.dispose();
    },
    [built],
  );
  const materials = useMemo(
    () => built.meshes.map((m) => (mode === "flat" ? flatMaterial(m.texture) : mats.get(drawOf(m.texture)))),
    [built, mats, version, mode], // eslint-disable-line react-hooks/exhaustive-deps
  );
  const w = edgeLook(mode, false, false);
  const t = edgeLook(mode, false, true);
  return (
    <>
      {mode !== "wire" &&
        built.meshes.map((m, i) => <mesh key={m.texture} geometry={m.geometry} material={materials[i]} raycast={() => null} />)}
      <lineSegments geometry={built.world} renderOrder={1} raycast={() => null}>
        <lineBasicMaterial color={w.color} transparent opacity={w.opacity} depthTest={mode !== "wire"} />
      </lineSegments>
      <lineSegments geometry={built.tied} renderOrder={1} raycast={() => null}>
        <lineBasicMaterial color={t.color} transparent opacity={t.opacity} depthTest={mode !== "wire"} />
      </lineSegments>
      {/* clicking finds each brush and its face here (never drawn) */}
      {list.map((b) => (
        <mesh key={b.id} geometry={b.geometry} visible={false} userData={hideSky && b.textures.every(isSky) ? { kind: "brush", id: b.id, sky: true } : { kind: "brush", id: b.id }} />
      ))}
    </>
  );
}
