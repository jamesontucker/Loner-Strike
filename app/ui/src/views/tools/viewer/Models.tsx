// Studio models (.mdl) placed in the map (hostages, cyclers, props), drawn and lit the way the
// game's studio renderer lights them: the light where the model stands (tools/render.rs
// samples the floor's lightmap, or the sun when the model sees the sky), shared into an
// ambient and a directional part, shaded per vertex with the game's "hemispherical" lambert,
// then the lightgamma table; textures through texgamma. Shown with "Lighting" on.

import { useThree } from "@react-three/fiber";
import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import * as THREE from "three";
import { getModel, modelTextureUrl, type ModelBundle } from "../assets/assetsApi";
import type { MapEntity } from "../toolsApi";
import { modelLight, modelVertexLight, type LookSettings } from "./gameLook";
import type { MapLook } from "./look";

const VERT = /* glsl */ `
attribute float light;
varying vec2 vUv;
varying float vLight;
void main() {
  vUv = uv;
  vLight = light;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`;

const FRAG = /* glsl */ `
uniform sampler2D map;
uniform vec3 colour;
uniform float texGamma;
uniform bool masked;
varying vec2 vUv;
varying float vLight;
void main() {
  vec4 t = texture2D(map, vUv);
  if (masked && t.a < 0.5) discard;
  vec3 c = floor(pow(t.rgb, vec3(texGamma)) * 255.0) / 255.0;
#ifdef FULLBRIGHT
  float lv = 1.0;
#else
  float lv = vLight;
#endif
  gl_FragColor = vec4(c * lv * colour, 1.0);
}`;

type Placed = MapLook["models"][number];

function Model({ m, ent, settings }: { m: Placed; ent: MapEntity | undefined; settings: LookSettings }) {
  const invalidate = useThree((s) => s.invalidate);
  const q = useQuery({ queryKey: ["tools", "assets", "model", m.folder, m.model], queryFn: ({ signal }) => getModel({ folder: m.folder, path: m.model }, signal), staleTime: Infinity, retry: false });
  const b: ModelBundle | undefined = q.data;
  const yaw = ent?.angles?.[1] ?? 0;
  const body = Number(ent?.keys.body ?? 0) || 0;
  const skin = Number(ent?.keys.skin ?? 0) || 0;

  const geometry = useMemo(() => {
    if (!b) return null;
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.BufferAttribute(b.positions, 3));
    g.setAttribute("uv", new THREE.BufferAttribute(b.uv, 2));
    g.setIndex(new THREE.BufferAttribute(b.indices, 1));
    // per-vertex light: normals turned by the entity's yaw, against the light's direction
    const l = modelLight(m.light, m.dir);
    const c = Math.cos((yaw * Math.PI) / 180), s = Math.sin((yaw * Math.PI) / 180);
    const n = b.normals, out = new Float32Array(n.length / 3);
    for (let i = 0; i < out.length; i++) {
      const x = n[i * 3], y = n[i * 3 + 1], z = n[i * 3 + 2];
      out[i] = modelVertexLight([x * c - y * s, x * s + y * c, z], l, settings);
    }
    g.setAttribute("light", new THREE.BufferAttribute(out, 1));
    // one model per body part, as the entity's "body" picks it
    b.draws.forEach((d, i) => {
      const part = b.bodyparts[d.part];
      const count = part?.models.length || 1;
      const want = Math.floor(body / (part?.base || 1)) % count;
      if (d.model === want && d.count > 0) g.addGroup(d.start, d.count, i);
    });
    return g;
  }, [b, m, yaw, body, settings]);
  useEffect(() => () => geometry?.dispose(), [geometry]);

  const materials = useMemo(() => {
    if (!b) return [];
    const loader = new THREE.TextureLoader();
    const tex = new Map<number, THREE.Texture>();
    const l = modelLight(m.light, m.dir);
    const pick = (row: number, ref: number) => b.skins[row]?.[ref] ?? b.skins[0]?.[ref] ?? ref;
    return b.draws.map((d) => {
      const ti = pick(skin, d.skinRef);
      const info = b.textures[ti];
      let t = tex.get(ti);
      if (!t) {
        t = loader.load(modelTextureUrl({ folder: m.folder, path: m.model }, ti), () => invalidate());
        t.flipY = false;
        t.colorSpace = THREE.NoColorSpace;
        t.wrapS = t.wrapT = THREE.RepeatWrapping;
        tex.set(ti, t);
      }
      const mat = new THREE.ShaderMaterial({
        uniforms: {
          map: { value: t },
          colour: { value: info?.fullbright ? new THREE.Color(1, 1, 1) : new THREE.Color(...l.colour) },
          texGamma: { value: settings.texgamma / settings.gamma },
          masked: { value: !!info?.masked },
        },
        vertexShader: VERT,
        fragmentShader: FRAG,
        side: THREE.DoubleSide,
      });
      if (info?.additive) {
        mat.transparent = true;
        mat.depthWrite = false;
        mat.blending = THREE.AdditiveBlending;
      }
      // full-bright textures skip the light: the vertex light is replaced by 1 in the colour
      if (info?.fullbright) mat.defines = { FULLBRIGHT: 1 };
      return mat;
    });
  }, [b, m, skin, settings, invalidate]);
  useEffect(() => () => materials.forEach((x) => {
    x.uniforms.map.value?.dispose();
    x.dispose();
  }), [materials]);
  useEffect(() => invalidate(), [geometry, materials, invalidate]);

  if (!geometry || !ent?.origin) return null;
  return <mesh geometry={geometry} material={materials} position={ent.origin} rotation={[0, 0, (yaw * Math.PI) / 180]} raycast={() => null} />;
}

export function Models({ look, entities, settings }: { look: MapLook | null; entities: MapEntity[]; settings: LookSettings }) {
  const byIndex = useMemo(() => new Map(entities.map((e) => [e.index, e])), [entities]);
  if (!look) return null;
  return (
    <>
      {look.models.map((m) => (
        <Model key={`${m.index}:${m.model}`} m={m} ent={byIndex.get(m.index)} settings={settings} />
      ))}
    </>
  );
}
