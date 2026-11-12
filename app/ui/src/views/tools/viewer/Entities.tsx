// Point entities as small boxes coloured by kind (spawns are player-sized), one instanced
// mesh for all of them; the picked entity gets an outline, and lines to what it triggers.

import { useThree, type ThreeEvent } from "@react-three/fiber";
import { useEffect, useMemo } from "react";
import * as THREE from "three";
import { useTools } from "../../../stores/tools";
import { notAPick } from "./Cameras";
import type { MapEntity } from "../toolsApi";
import { HULL_STAND, type Vec3 } from "./coords";
import type { ModelMesh } from "./World";

type Look = { colour: string; size: Vec3; group: string };

/** How a point entity is drawn, and the group it's listed under. */
export function lookOf(e: MapEntity): Look {
  const c = e.classname;
  if (c === "info_player_start") return { colour: "#8fb4d6", size: HULL_STAND, group: "CT spawns" };
  if (c === "info_player_deathmatch") return { colour: "#d9a441", size: HULL_STAND, group: "T spawns" };
  if (c === "info_vip_start") return { colour: "#e6e6e6", size: HULL_STAND, group: "VIP spawn" };
  if (c === "hostage_entity" || c === "monster_scientist") return { colour: "#9fd18b", size: HULL_STAND, group: "Hostages" };
  if (c.startsWith("light")) return { colour: lightColour(e) ?? "#f2e38b", size: [10, 10, 10], group: "Lights" };
  if (c === "ambient_generic" || c === "env_sound") return { colour: "#c49be0", size: [12, 12, 12], group: "Sounds" };
  if (c === "armoury_entity" || c.startsWith("weapon_") || c.startsWith("item_") || c.startsWith("ammo_")) return { colour: "#cfd6c4", size: [16, 16, 16], group: "Items" };
  if (c.startsWith("info_")) return { colour: "#7fd1c7", size: [14, 14, 14], group: "Markers" };
  if (c.startsWith("env_")) return { colour: "#7fb0d1", size: [14, 14, 14], group: "Effects" };
  return { colour: "#e8e8e8", size: [14, 14, 14], group: "Other" };
}

function lightColour(e: MapEntity) {
  const v = (e.keys._light ?? "").trim().split(/\s+/).map(Number);
  if (v.length < 3 || !v.slice(0, 3).every(Number.isFinite)) return null;
  return `rgb(${v[0]},${v[1]},${v[2]})`;
}

/** Point entities: the ones with an origin and no brush model. */
export const pointEntities = (ents: MapEntity[]) => ents.filter((e) => e.origin && !e.model?.startsWith("*") && e.classname !== "worldspawn");

/** Where an entity is: its origin, or the middle of its brush model. */
export function entityCentre(e: MapEntity, meshes: ModelMesh[]): { point: Vec3; size: number } | null {
  if (e.model?.startsWith("*")) {
    const m = meshes.find((x) => x.entity?.index === e.index);
    const b = m?.geometry.boundingBox;
    if (b && !b.isEmpty()) {
      const c = b.getCenter(new THREE.Vector3());
      const s = b.getSize(new THREE.Vector3());
      const o = e.origin ?? [0, 0, 0];
      return { point: [c.x + o[0], c.y + o[1], c.z + o[2]], size: Math.max(s.x, s.y, s.z) };
    }
  }
  return e.origin ? { point: e.origin, size: Math.max(...lookOf(e).size) } : null;
}

const unitBox = new THREE.BoxGeometry(1, 1, 1);

export function Entities({ entities, meshes }: { entities: MapEntity[]; meshes: ModelMesh[] }) {
  const show = useTools((s) => s.layers.entities);
  const invalidate = useThree((s) => s.invalidate);
  const points = useMemo(() => pointEntities(entities), [entities]);

  const mesh = useMemo(() => {
    const mat = new THREE.MeshBasicMaterial({ transparent: true, opacity: 0.8, depthWrite: true });
    const m = new THREE.InstancedMesh(unitBox, mat, Math.max(1, points.length));
    m.count = points.length;
    const o = new THREE.Object3D();
    const col = new THREE.Color();
    points.forEach((e, i) => {
      const look = lookOf(e);
      o.position.set(...e.origin!);
      o.rotation.set(0, 0, ((e.angles?.[1] ?? 0) * Math.PI) / 180);
      o.scale.set(...look.size);
      o.updateMatrix();
      m.setMatrixAt(i, o.matrix);
      m.setColorAt(i, col.set(look.colour));
    });
    m.instanceMatrix.needsUpdate = true;
    if (m.instanceColor) m.instanceColor.needsUpdate = true;
    m.computeBoundingSphere();
    return m;
  }, [points]);
  useEffect(() => () => {
    (mesh.material as THREE.Material).dispose();
    mesh.dispose();
  }, [mesh]);
  useEffect(() => invalidate(), [show, mesh, invalidate]);

  const click = (e: ThreeEvent<MouseEvent>) => {
    if (notAPick(e) || e.instanceId === undefined) return;
    e.stopPropagation();
    useTools.getState().select({ kind: "entity", index: points[e.instanceId].index });
  };
  const hover = (e: ThreeEvent<PointerEvent>) => {
    if (e.instanceId === undefined) return;
    e.stopPropagation();
    const ent = points[e.instanceId];
    useTools.getState().setHover(ent.keys.targetname ? `${ent.classname} "${ent.keys.targetname}"` : ent.classname, ent.origin);
  };

  return (
    <>
      <primitive object={mesh} visible={show && points.length > 0} onClick={click} onPointerMove={hover} onPointerOut={() => useTools.getState().setHover(null)} />
      <Picked entities={entities} meshes={meshes} />
    </>
  );
}

const outlineMat = new THREE.LineBasicMaterial({ color: "#f4e46a", depthTest: false, transparent: true });
const linkMat = new THREE.LineBasicMaterial({ color: "#f4e46a", depthTest: false, transparent: true, opacity: 0.7 });
const overlayMat = new THREE.MeshBasicMaterial({ color: "#f4e46a", transparent: true, opacity: 0.25, depthTest: false, depthWrite: false, side: THREE.DoubleSide });

/** The picked entity: an outline (point) or a tint (brush), and lines to its targets. */
function Picked({ entities, meshes }: { entities: MapEntity[]; meshes: ModelMesh[] }) {
  const sel = useTools((s) => s.selection);
  const invalidate = useThree((s) => s.invalidate);
  const ent = sel?.kind === "entity" ? entities.find((e) => e.index === sel.index) ?? null : null;
  const brush = ent?.model?.startsWith("*") ? meshes.find((m) => m.entity?.index === ent.index) ?? null : null;

  const outline = useMemo(() => {
    if (!ent || brush || !ent.origin) return null;
    const s = lookOf(ent).size.map((n) => n + 4) as Vec3;
    const g = new THREE.EdgesGeometry(new THREE.BoxGeometry(...s));
    return { g, pos: ent.origin };
  }, [ent, brush]);
  useEffect(() => () => outline?.g.dispose(), [outline]);

  // lines from it to what it targets, and from what targets it
  const links = useMemo(() => {
    if (!ent) return null;
    const from = entityCentre(ent, meshes);
    if (!from) return null;
    const name = ent.keys.targetname;
    const targets = [ent.keys.target, ent.keys.killtarget].filter(Boolean);
    const others = entities.filter((o) => o !== ent && ((o.keys.targetname && targets.includes(o.keys.targetname)) || (name && (o.keys.target === name || o.keys.killtarget === name))));
    const pts: number[] = [];
    for (const o of others) {
      const to = entityCentre(o, meshes);
      if (to) pts.push(...from.point, ...to.point);
    }
    if (!pts.length) return null;
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
    return g;
  }, [ent, entities, meshes]);
  useEffect(() => () => links?.dispose(), [links]);
  useEffect(() => invalidate(), [sel, invalidate]);

  const overlay = useMemo(() => (brush ? brush.draws.map(() => overlayMat) : null), [brush]);
  return (
    <>
      {outline && <lineSegments geometry={outline.g} material={outlineMat} position={outline.pos} renderOrder={10} />}
      {brush && overlay && <mesh geometry={brush.geometry} material={overlay} position={ent?.origin ?? [0, 0, 0]} renderOrder={10} raycast={() => null} />}
      {links && <lineSegments geometry={links} material={linkMat} renderOrder={10} />}
    </>
  );
}
