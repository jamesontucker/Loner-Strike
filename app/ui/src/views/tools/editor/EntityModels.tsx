// P5: studio models drawn where point entities stand, instead of their coloured boxes: a
// model key (cycler, env_model, monster_*), hostages, spawns as the team's player model,
// weapons on the floor as their w_ model, env_sprite as a glow facing the camera. Loaded
// lazily through the asset browser's model route (agent H) and cached; the box stays for
// clicking (invisible) and as the fallback when a model isn't found. View > Models toggles.

import { Billboard } from "@react-three/drei";
import { useThree } from "@react-three/fiber";
import { useEffect, useMemo, useState } from "react";
import * as THREE from "three";
import { getModel, modelTextureUrl, type ModelBundle } from "../assets/assetsApi";
import type { DocEntity } from "./editorApi";
import { loadCreatures, useKits, type Creature } from "./kits";
import { useP5 } from "./p5store";
import { keyOf } from "./store";
import { redrawAll } from "./paneRedraw"; // Parity A

const FOLDERS = ["cstrike", "cstrike_downloads", "czero", "valve"];
const ARMOURY = ["mp5", "tmp", "p90", "mac10", "ak47", "sg552", "m4a1", "aug", "scout", "g3sg1", "awp", "m3", "xm1014", "m249", "flashbang", "hegrenade", "kevlar", "assault", "smokegrenade"];

/** ls_creature's model when the creature types aren't loaded yet (the brain's built-in types). */
const CREATURE_MODELS: Record<string, string> = {
  zombie: "models/zombie.mdl", runner: "models/houndeye.mdl", hound: "models/houndeye.mdl", brute: "models/agrunt.mdl",
  spitter: "models/bullsquid.mdl", turret: "models/sentry.mdl", boss: "models/garg.mdl", headcrab: "models/headcrab.mdl",
  vortigaunt: "models/islave.mdl", babygarg: "models/garg.mdl", snark: "models/w_squeak.mdl", babycrab: "models/baby_headcrab.mdl",
  miniturret: "models/miniturret.mdl models/sentry.mdl", assassin: "models/hassassin.mdl", soldier: "models/hgrunt.mdl", gonarch: "models/big_mom.mdl",
};

/** The model file(s) a point entity shows in the game (null: none we can draw). Several paths,
 *  space-separated, are tried in order (a creature type's stand-ins). */
export function modelOf(e: DocEntity, creatures: Creature[] = []): string | null {
  const m = keyOf(e.keys, "model")?.trim().replace(/\\/g, "/");
  if (m && /\.mdl$/i.test(m) && !m.includes("..")) return m.replace(/^\/+/, "");
  switch (e.classname) {
    case "ls_barnacle":
      return "models/barnacle.mdl";
    case "ls_creature": {
      const type = keyOf(e.keys, "type")?.trim() || "zombie";
      return creatures.find((c) => c.name === type)?.model?.trim() || CREATURE_MODELS[type] || null;
    }
    case "hostage_entity":
    case "monster_scientist":
      return "models/scientist.mdl";
    case "info_player_start":
      return "models/player/gign/gign.mdl";
    case "info_player_deathmatch":
      return "models/player/terror/terror.mdl";
    case "info_vip_start":
      return "models/player/vip/vip.mdl";
    case "armoury_entity": {
      const i = Number(keyOf(e.keys, "item") ?? 0) || 0;
      return ARMOURY[i] ? `models/w_${ARMOURY[i]}.mdl` : null;
    }
  }
  return null;
}

export type Got = { item: { folder: string; path: string }; bundle: ModelBundle } | null;
const cache = new Map<string, Promise<Got>>();

/** The first of the paths found in a game folder (null when none are); one request per list. */
export function loadModel(paths: string): Promise<Got> {
  const key = paths.toLowerCase();
  let p = cache.get(key);
  if (!p) {
    p = (async () => {
      for (const path of paths.split(/\s+/).filter((x) => /\.mdl$/i.test(x) && !x.includes(".."))) {
        for (const folder of FOLDERS) {
          try {
            const item = { folder, path };
            return { item, bundle: await getModel(item) };
          } catch {
            /* the next folder */
          }
        }
      }
      return null;
    })();
    cache.set(key, p);
  }
  return p;
}

/** The model for an entity once it's loaded (null meanwhile, when off, or not found). */
export function useEntityModel(e: DocEntity): Got {
  const on = useP5((s) => s.showModels);
  const creatures = useKits((k) => k.creatures);
  const isCreature = e.classname === "ls_creature";
  useEffect(() => {
    if (on && isCreature && !useKits.getState().creatures.length) void loadCreatures();
  }, [on, isCreature]);
  const path = on ? modelOf(e, creatures) : null;
  const [got, setGot] = useState<Got>(null);
  useEffect(() => {
    if (!path) return setGot(null);
    let live = true;
    void loadModel(path).then((g) => live && setGot(g));
    return () => {
      live = false;
    };
  }, [path]);
  return path ? got : null;
}

const texLoader = new THREE.TextureLoader();

// Parity A (PerfAudit #1): one geometry per model+body and one set of materials (textures
// uploaded once) per model+skin, for the whole session: edits never reload model textures.
const geoCache = new WeakMap<ModelBundle, Map<number, THREE.BufferGeometry>>();
const matCache = new Map<string, THREE.MeshLambertMaterial[]>();
/** The shared geometry of a model's body (never disposed by its users). */
export function sharedModelGeometry(b: ModelBundle, body: number): THREE.BufferGeometry {
  let m = geoCache.get(b);
  if (!m) geoCache.set(b, (m = new Map()));
  let g = m.get(body);
  if (!g) m.set(body, (g = modelGeometry(b, body)));
  return g;
}
/** The shared materials of a model's skin (never disposed by their users). */
export function sharedModelMaterials(b: ModelBundle, item: { folder: string; path: string }, skin: number): THREE.MeshLambertMaterial[] {
  const key = `${item.folder}/${item.path}|${skin}`.toLowerCase();
  let m = matCache.get(key);
  if (!m) matCache.set(key, (m = modelMaterials(b, item, skin, () => redrawAll())));
  return m;
}

/** Parity A: a model's geometry for one body (shared by the single mesh and the instanced batch). */
export function modelGeometry(b: ModelBundle, body: number): THREE.BufferGeometry {
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.BufferAttribute(b.positions, 3));
  g.setAttribute("normal", new THREE.BufferAttribute(b.normals, 3));
  g.setAttribute("uv", new THREE.BufferAttribute(b.uv, 2));
  g.setIndex(new THREE.BufferAttribute(b.indices, 1));
  b.draws.forEach((d, i) => {
    const part = b.bodyparts[d.part];
    const count = part?.models.length || 1;
    const want = Math.floor(body / (part?.base || 1)) % count;
    if (d.model === want && d.count > 0) g.addGroup(d.start, d.count, i);
  });
  return g;
}

/** Parity A: a model's materials for one skin; `loaded` runs as each texture arrives. */
export function modelMaterials(b: ModelBundle, item: { folder: string; path: string }, skin: number, loaded: () => void): THREE.MeshLambertMaterial[] {
  const tex = new Map<number, THREE.Texture>();
  const pick = (ref: number) => b.skins[skin]?.[ref] ?? b.skins[0]?.[ref] ?? ref;
  return b.draws.map((d) => {
    const ti = pick(d.skinRef);
    const info = b.textures[ti];
    let t = tex.get(ti);
    if (!t) {
      t = texLoader.load(modelTextureUrl(item, ti), loaded);
      t.flipY = false;
      t.colorSpace = THREE.NoColorSpace;
      t.wrapS = t.wrapT = THREE.RepeatWrapping;
      tex.set(ti, t);
    }
    return new THREE.MeshLambertMaterial({ map: t, side: THREE.DoubleSide, alphaTest: info?.masked ? 0.5 : 0, transparent: !!info?.additive, depthWrite: !info?.additive, blending: info?.additive ? THREE.AdditiveBlending : THREE.NormalBlending });
  });
}

/** The model's first body and skin, lit by the scene. Not pickable (the entity's box is). */
export function EntityModelMesh({ got, yaw, body = 0, skin = 0 }: { got: NonNullable<Got>; yaw: number; body?: number; skin?: number }) {
  const invalidate = useThree((s) => s.invalidate);
  const { bundle: b, item } = got;
  // Parity A: shared, cached for the session (not disposed here)
  const geometry = useMemo(() => sharedModelGeometry(b, body), [b, body]);
  const materials = useMemo(() => sharedModelMaterials(b, item, skin), [b, item, skin]);
  useEffect(() => invalidate(), [geometry, materials, invalidate]);
  return <mesh geometry={geometry} material={materials} rotation={[0, 0, (yaw * Math.PI) / 180]} raycast={() => null} />;
}

/** env_sprite and friends: a soft glow in the sprite's colour that always faces the camera. */
export function SpriteGlow({ e }: { e: DocEntity }) {
  const on = useP5((s) => s.showModels);
  const rgb = (keyOf(e.keys, "rendercolor") ?? "255 255 255").split(/\s+/).map(Number);
  const scale = Math.max(0.1, Math.min(8, Number(keyOf(e.keys, "scale") ?? 1) || 1));
  const tex = useMemo(() => {
    const c = document.createElement("canvas");
    c.width = c.height = 64;
    const g = c.getContext("2d")!;
    const grad = g.createRadialGradient(32, 32, 0, 32, 32, 32);
    grad.addColorStop(0, "rgba(255,255,255,1)");
    grad.addColorStop(0.4, "rgba(255,255,255,0.5)");
    grad.addColorStop(1, "rgba(255,255,255,0)");
    g.fillStyle = grad;
    g.fillRect(0, 0, 64, 64);
    const t = new THREE.CanvasTexture(c);
    t.colorSpace = THREE.NoColorSpace;
    return t;
  }, []);
  useEffect(() => () => tex.dispose(), [tex]);
  if (!on || !/^env_(sprite|glow)$/.test(e.classname)) return null;
  const col = rgb.length >= 3 && rgb.slice(0, 3).every(Number.isFinite) && rgb.some((x) => x > 0) ? new THREE.Color(rgb[0] / 255, rgb[1] / 255, rgb[2] / 255) : new THREE.Color(1, 1, 1);
  const size = 32 * scale;
  return (
    <Billboard>
      <mesh raycast={() => null} renderOrder={4}>
        <planeGeometry args={[size, size]} />
        <meshBasicMaterial map={tex} color={col} transparent depthWrite={false} blending={THREE.AdditiveBlending} />
      </mesh>
    </Billboard>
  );
}
