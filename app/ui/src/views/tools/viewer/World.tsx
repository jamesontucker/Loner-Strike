// The map's brushes: one geometry per model (0 = the world, N = brush entity "*N") sharing
// the bundle's buffers, with a group per draw so each texture is its own material.

import { useThree, type ThreeEvent } from "@react-three/fiber";
import { useEffect, useMemo, useRef, useState } from "react";
import * as THREE from "three";
import { useTools } from "../../../stores/tools";
import { notAPick } from "./Cameras";
import type { MapEntity } from "../toolsApi";
import { lightmapUrl, texUrl } from "../toolsApi";
import type { Draw, MeshBundle } from "./bundle";
import { parseVec, type Vec3 } from "./coords";
import { MapMaterials, isSky, isTrigger, isWater } from "./materials";
import { animates, loadSky, renderOf, styleStrings, styleValues } from "./gameLook";
import { skyUrls, useLook, useLookMode } from "./look";
import { DevCamera } from "./DevCamera";

/** origin: where the model sits (GoldSrc); models built around an origin brush (rotating
 *  doors) are at 0,0,0 in the buffers and moved there. */
export type ModelMesh = { model: number; geometry: THREE.BufferGeometry; draws: Draw[]; entity: MapEntity | null; kind: "world" | "moving" | "trigger"; origin: Vec3 };

/** Which layer a brush entity belongs to. */
function kindOf(model: number, draws: Draw[], ent: MapEntity | null): ModelMesh["kind"] {
  if (model === 0) return "world";
  const cls = ent?.classname ?? "";
  if (/^(trigger_|func_(buyzone|bomb_target|hostage_rescue|escapezone|vip_safetyzone|ladder)$)/.test(cls) || draws.every(isTrigger)) {
    // ladders are "moving parts" people want to see; the rest are invisible zones
    return cls === "func_ladder" ? "moving" : "trigger";
  }
  return "moving";
}

export function useModelMeshes(bundle: MeshBundle | null, entities: MapEntity[]) {
  const meshes = useMemo(() => {
    if (!bundle) return [];
    const shared = {
      position: new THREE.BufferAttribute(bundle.positions, 3),
      normal: bundle.normals ? new THREE.BufferAttribute(bundle.normals, 3) : null,
      uv: bundle.uv ? new THREE.BufferAttribute(bundle.uv, 2) : null,
      // three reads the lightmap through the second uv set, called "uv1"
      uv1: bundle.uv2 ? new THREE.BufferAttribute(bundle.uv2, 2) : null,
      index: new THREE.BufferAttribute(bundle.indices, 1),
    };
    // the header's per-model facts (origin), by model number
    const headerModels = new Map<number, Record<string, unknown>>();
    if (Array.isArray(bundle.header.models)) for (const m of bundle.header.models as Record<string, unknown>[]) headerModels.set(Number(m.index), m);
    const byModel = new Map<string, MapEntity>();
    for (const e of entities) if (e.model?.startsWith("*")) byModel.set(e.model.slice(1), e);
    return bundle.models.map((model): ModelMesh => {
      const draws = bundle.draws.filter((d) => d.model === model && d.count > 0);
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", shared.position);
      if (shared.normal) g.setAttribute("normal", shared.normal);
      if (shared.uv) g.setAttribute("uv", shared.uv);
      if (shared.uv1) g.setAttribute("uv1", shared.uv1);
      g.setIndex(shared.index);
      draws.forEach((d, i) => g.addGroup(d.start, d.count, i));
      // bounds of this model's own triangles, for culling and picking
      const box = new THREE.Box3();
      const v = new THREE.Vector3();
      for (const d of draws) for (let i = d.start; i < d.start + d.count; i++) box.expandByPoint(v.fromBufferAttribute(shared.position, bundle.indices[i]));
      g.boundingBox = box;
      g.boundingSphere = box.isEmpty() ? new THREE.Sphere() : box.getBoundingSphere(new THREE.Sphere());
      const ei = draws.find((d) => d.entityIndex !== undefined)?.entityIndex;
      const ent = (ei !== undefined ? entities.find((e) => e.index === ei) : undefined) ?? byModel.get(String(model)) ?? null;
      const origin: Vec3 = model === 0 ? [0, 0, 0] : parseVec(headerModels.get(model)?.origin) ?? ent?.origin ?? [0, 0, 0];
      return { model, geometry: g, draws, entity: model === 0 ? null : ent, kind: kindOf(model, draws, ent), origin };
    });
  }, [bundle, entities]);
  useEffect(() => () => meshes.forEach((m) => m.geometry.dispose()), [meshes]);
  return meshes;
}

/** The draw a picked triangle belongs to, or null if it's one you can't see (or sky). */
export function drawAt(mesh: ModelMesh, faceIndex: number | null | undefined, materials: THREE.Material[]): Draw | null {
  if (faceIndex == null) return null;
  const at = faceIndex * 3;
  const i = mesh.draws.findIndex((d) => at >= d.start && at < d.start + d.count);
  if (i < 0) return null;
  const d = mesh.draws[i];
  if (!materials[i]?.visible || isSky(d)) return null;
  return d;
}

export function World({ meshes, bundle, map, entities = [] }: { meshes: ModelMesh[]; bundle: MeshBundle; map: string; entities?: MapEntity[] }) {
  const invalidate = useThree((s) => s.invalidate);
  const gl = useThree((s) => s.gl);
  const layers = useTools((s) => s.layers);
  const camera = useTools((s) => s.camera);
  const { select, setHover } = useTools.getState();
  const [version, setVersion] = useState(0);
  // (the dev test room draws its own pictures; a real map's come from the app)
  const mock = import.meta.env.DEV && bundle.header.mock === true;
  const lookMode = useLookMode((s) => s.mode);
  const mats = useMemo(
    () => new MapMaterials({ tex: (n) => texUrl(map, n, mock), lightmap: () => (bundle.uv2 ? lightmapUrl(map, mock) : Promise.resolve(null)) }, { textures: layers.textures, lighting: layers.lighting, triggers: layers.triggers, sky: camera === "fly", look: lookMode }, invalidate, Math.min(16, gl.capabilities.getMaxAnisotropy()), bundle.clockwise, bundle.header.lightmap as { maps?: number; planes?: number; height?: number } | undefined),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [map, bundle],
  );
  useEffect(() => {
    mats.onChange = (remade) => remade && setVersion((v) => v + 1);
    return () => mats.dispose();
  }, [mats]);
  useEffect(() => mats.setOptions({ textures: layers.textures, lighting: layers.lighting, triggers: layers.triggers, sky: camera === "fly", look: lookMode }), [mats, layers.textures, layers.lighting, layers.triggers, camera, lookMode]);
  // the player's own picture settings and the sky box
  const look = useLook(map, mock).data ?? null;
  useEffect(() => {
    if (look) mats.setSettings(look.settings);
  }, [mats, look]);
  useEffect(() => {
    const urls = skyUrls(map, look);
    if (!urls) return;
    let live = true;
    loadSky(urls).then((t) => (live ? mats.setSky(t) : t?.dispose()));
    return () => void (live = false);
  }, [mats, map, look]);
  // brush entities by their render mode (rendermode, renderamt, rendercolor)
  // eslint-disable-next-line react-hooks/exhaustive-deps
  const materialSets = useMemo(() => meshes.map((m) => m.draws.map((d) => mats.get(d, m.entity ? renderOf(m.entity.keys) : undefined))), [meshes, mats, version]);
  useEffect(() => invalidate(), [materialSets, layers, invalidate]);

  const pick = (m: ModelMesh, i: number) => (e: ThreeEvent<MouseEvent>) => {
    if (notAPick(e) || !e.object.visible) return;
    const d = drawAt(m, e.faceIndex, materialSets[i]);
    if (!d) return;
    e.stopPropagation();
    if (m.entity) return select({ kind: "entity", index: m.entity.index });
    // in GoldSrc numbers: the root group's space, not the (maybe moved) brush model's
    const p = (e.object.parent ?? e.object).worldToLocal(e.point.clone());
    select({ kind: "face", texture: d.texture, model: m.model, face: bundle.faceOfTriangle?.[e.faceIndex!] ?? null, point: [p.x, p.y, p.z] });
  };
  const hover = (m: ModelMesh, i: number) => (e: ThreeEvent<PointerEvent>) => {
    if (!e.object.visible) return;
    const d = drawAt(m, e.faceIndex, materialSets[i]);
    if (!d) return;
    e.stopPropagation();
    const p = (e.object.parent ?? e.object).worldToLocal(e.point.clone());
    setHover(m.entity ? `${d.texture} (${m.entity.classname})` : d.texture, [p.x, p.y, p.z]);
  };

  return (
    <>
      <GameClock mats={mats} meshes={meshes} entities={entities} on={layers.lighting} />
      {import.meta.env.DEV && <DevCamera />}
      {meshes.map((m, i) => {
        const visible = m.kind === "world" || (m.kind === "trigger" ? layers.triggers : layers.moving);
        return (
          <mesh
            key={m.model}
            geometry={m.geometry}
            material={materialSets[i]}
            visible={visible}
            // brush entities with an origin (rotating doors) are built around it
            position={m.origin}
            userData={{ model: m.model }}
            onClick={pick(m, i)}
            onPointerMove={hover(m, i)}
            onPointerOut={() => setHover(null)}
          />
        );
      })}
    </>
  );
}

/** The game's clock for the open map: light styles flicker and pulse (10 steps a second) and
 *  water wobbles. Only runs while something on the map moves, the page is visible and the
 *  map's own lighting is on; the view otherwise draws only when something changes. */
function GameClock({ mats, meshes, entities, on }: { mats: MapMaterials; meshes: ModelMesh[]; entities: MapEntity[]; on: boolean }) {
  const invalidate = useThree((s) => s.invalidate);
  const styles = useMemo(() => styleStrings(entities), [entities]);
  // the styles lights on this map use (faces lit by them carry their pictures)
  const used = useMemo(() => {
    const u = new Set<number>([0]);
    for (const e of entities) if (e.classname.startsWith("light")) u.add(Number(e.keys.style) || 0);
    return u;
  }, [entities]);
  const water = useMemo(() => meshes.some((m) => m.draws.some(isWater)), [meshes]);
  const start = useRef(performance.now());
  useEffect(() => {
    const u = mats.uniforms;
    styleValues(styles, 0, u.uStyles.value);
    invalidate();
    if (!on || !(water || animates(styles, used))) return;
    let raf = 0;
    const tick = () => {
      raf = requestAnimationFrame(tick);
      if (document.hidden) return;
      const t = (performance.now() - start.current) / 1000;
      u.uTime.value = t;
      styleValues(styles, t, u.uStyles.value);
      invalidate();
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [mats, styles, used, water, on, invalidate]);
  return null;
}
