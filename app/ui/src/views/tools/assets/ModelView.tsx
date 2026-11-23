// The 3D model viewer: one mesh per model with a material per draw (its skin's texture),
// the body parts chosen, an orbit camera that frames the model, and animations played by
// moving each vertex with its bone (models have a few thousand vertices: cheap on the CPU).

import { OrbitControls } from "@react-three/drei";
import { Canvas, useFrame, useThree } from "@react-three/fiber";
import { useEffect, useMemo, useRef, useState } from "react";
import type { ElementRef } from "react";
import * as THREE from "three";
import { ROOT_ROTATION } from "../viewer/coords";
import { modelTextureUrl, type Animation, type ModelBundle } from "./assetsApi";

type Props = {
  item: { folder: string; path: string };
  bundle: ModelBundle;
  skin: number;
  body: number[];
  anim: Animation | null;
  playing: boolean;
  wire: boolean;
  onLost: () => void;
  /** go round again at the end (default: the sequence's own loop flag); off, it plays once
   *  and holds the last frame */
  loop?: boolean;
  /** playback speed (1 = the sequence's own frame rate) */
  speed?: number;
  /** a played-once animation reached its last frame */
  onEnded?: () => void;
  /** Creature Studio: the entity's scale (pev->scale) and a kRenderFxGlowShell colour (rgb 0-255) with its renderamt */
  scale?: number;
  shell?: { color: [number, number, number]; amount: number } | null;
};

export function ModelView(p: Props) {
  return (
    <Canvas
      onCreated={({ gl }) => gl.domElement.addEventListener("webglcontextlost", (e) => {
        e.preventDefault();
        p.onLost();
      })}
      frameloop={p.playing ? "always" : "demand"}
      dpr={[1, 1.5]}
      linear
      flat
      // (the drawing stays readable, so Share can take a picture of a creature: CP-kind)
      gl={{ antialias: true, preserveDrawingBuffer: true }}
      camera={{ fov: 45, near: 0.5, far: 20000, position: [80, 40, 80] }}
    >
      <color attach="background" args={["#2a2f27"]} />
      {/* brighter: models were lit very dark (QA4-T2 UX-L1) */}
      <ambientLight intensity={0.6} />
      <hemisphereLight args={["#ffffff", "#5c6056", 1.5]} />
      <directionalLight position={[0.6, 1, 0.8]} intensity={1.6} />
      <Floor bundle={p.bundle} />
      <group rotation={ROOT_ROTATION} scale={p.scale ?? 1}>
        <ModelMesh {...p} />
      </group>
      <Fit bundle={p.bundle} scale={p.scale ?? 1} />
    </Canvas>
  );
}

/** A faint grid under the model's feet (three's y is GoldSrc's z). */
function Floor({ bundle }: { bundle: ModelBundle }) {
  const [lo, hi] = bundle.bounds;
  const size = Math.max(hi[0] - lo[0], hi[1] - lo[1], 32) * 3;
  return <gridHelper args={[size, 12, "#5c6353", "#3c4237"]} position={[(lo[0] + hi[0]) / 2, lo[2], -(lo[1] + hi[1]) / 2]} />;
}

/** Frames the model: in front of it (GoldSrc models face +x), a little above. */
function Fit({ bundle, scale = 1 }: { bundle: ModelBundle; scale?: number }) {
  const ref = useRef<ElementRef<typeof OrbitControls>>(null);
  const camera = useThree((s) => s.camera);
  useEffect(() => {
    const [lo, hi] = bundle.bounds.map((v) => v.map((x) => x * scale)) as typeof bundle.bounds;
    const c = new THREE.Vector3((lo[0] + hi[0]) / 2, (lo[2] + hi[2]) / 2, -(lo[1] + hi[1]) / 2);
    const r = Math.max(4, Math.hypot(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]) / 2);
    const d = r / Math.sin(((45 / 2) * Math.PI) / 180) * 1.1;
    camera.position.set(c.x + d * 0.8, c.y + d * 0.25, c.z - d * 0.55);
    camera.near = Math.max(0.1, d / 200);
    camera.far = d * 50;
    camera.updateProjectionMatrix();
    ref.current?.target.copy(c);
    ref.current?.update();
  }, [bundle, camera, scale]);
  return <OrbitControls ref={ref} makeDefault enableDamping dampingFactor={0.15} />;
}

/** The checker the game shows for a texture it can't find. */
function missingTexture() {
  const n = 8, data = new Uint8Array(n * n * 4);
  for (let i = 0; i < n * n; i++) {
    const on = ((i % n) + Math.floor(i / n)) % 2 === 0;
    data.set(on ? [255, 0, 255, 255] : [0, 0, 0, 255], i * 4);
  }
  const t = new THREE.DataTexture(data, n, n);
  t.magFilter = THREE.NearestFilter;
  t.wrapS = t.wrapT = THREE.RepeatWrapping;
  t.needsUpdate = true;
  return t;
}

function ModelMesh({ item, bundle, skin, body, anim, playing, wire, loop, speed, onEnded, shell }: Props) {
  const invalidate = useThree((s) => s.invalidate);
  // the vertex data we move when animating (copies; the bundle stays as it came)
  const geometry = useMemo(() => {
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.BufferAttribute(new Float32Array(bundle.positions), 3));
    g.setAttribute("normal", new THREE.BufferAttribute(new Float32Array(bundle.normals), 3));
    g.setAttribute("uv", new THREE.BufferAttribute(bundle.uv, 2));
    g.setIndex(new THREE.BufferAttribute(bundle.indices, 1));
    return g;
  }, [bundle]);
  useEffect(() => () => geometry.dispose(), [geometry]);

  // one texture per model texture, loaded once; the model shows once they're all in (not as a
  // black shape first)
  const [arrived, setArrived] = useState<{ of: unknown; n: number }>({ of: null, n: 0 });
  const textures = useMemo(() => {
    const loader = new THREE.TextureLoader();
    const list: THREE.Texture[] = [];
    const done = () => {
      setArrived((a) => ({ of: list, n: a.of === list ? a.n + 1 : 1 }));
      invalidate();
    };
    bundle.textures.forEach((_, i) => {
      const t = loader.load(modelTextureUrl(item, i), done, undefined, done);
      t.flipY = false;
      t.wrapS = t.wrapT = THREE.RepeatWrapping;
      t.anisotropy = 4;
      list.push(t);
    });
    return list;
  }, [bundle, item, invalidate]);
  const ready = textures.length === 0 || (arrived.of === textures && arrived.n >= textures.length);
  useEffect(() => () => textures.forEach((t) => t.dispose()), [textures]);

  // a material per draw: its texture in this skin (sized to the texture skin 0 used, which
  // the uvs were divided by)
  const materials = useMemo(() => {
    const missing = missingTexture();
    const pick = (row: number, ref: number) => bundle.skins[row]?.[ref] ?? bundle.skins[0]?.[ref] ?? (bundle.skins.length ? -1 : ref);
    return bundle.draws.map((d) => {
      const ti = pick(skin, d.skinRef);
      const t0 = bundle.textures[pick(0, d.skinRef)];
      const info = bundle.textures[ti];
      let map: THREE.Texture = textures[ti] ?? missing;
      if (info && t0 && (info.width !== t0.width || info.height !== t0.height) && textures[ti]) {
        map = textures[ti].clone();
        map.repeat.set(t0.width / info.width, t0.height / info.height);
        map.needsUpdate = true;
      }
      const opts = {
        map,
        wireframe: wire,
        side: THREE.DoubleSide,
        alphaTest: info?.masked ? 0.5 : 0,
        transparent: !!info?.additive,
        blending: info?.additive ? THREE.AdditiveBlending : THREE.NormalBlending,
        depthWrite: !info?.additive,
      };
      return info?.fullbright ? new THREE.MeshBasicMaterial(opts) : new THREE.MeshLambertMaterial(opts);
    });
  }, [bundle, textures, skin, wire]);
  useEffect(() => () => materials.forEach((m) => m.dispose()), [materials]);

  // only the chosen model of each body part is drawn
  useEffect(() => {
    geometry.clearGroups();
    bundle.draws.forEach((d, i) => {
      if ((body[d.part] ?? 0) === d.model && d.count > 0) geometry.addGroup(d.start, d.count, i);
    });
    invalidate();
  }, [geometry, bundle, body, invalidate]);

  // animation: the bones' matrices for a frame, then every vertex moved with its bone
  const clock = useRef({ t: 0, frame: -1 });
  useEffect(() => {
    clock.current = { t: 0, frame: -1 };
    if (anim) pose(geometry, bundle, anim, 0);
    else {
      // back to the pose the model came in
      (geometry.attributes.position.array as Float32Array).set(bundle.positions);
      (geometry.attributes.normal.array as Float32Array).set(bundle.normals);
      geometry.attributes.position.needsUpdate = true;
      geometry.attributes.normal.needsUpdate = true;
    }
    invalidate();
  }, [anim, geometry, bundle, invalidate]);
  // played once to the end, Play starts it over
  const loops = loop ?? anim?.loop ?? true;
  useEffect(() => {
    if (playing && anim && !loops && clock.current.frame >= anim.frames - 1) clock.current.t = 0;
  }, [playing, anim, loops]);
  useFrame((_, dt) => {
    if (!anim || !playing || anim.frames < 2) return;
    const c = clock.current;
    c.t += Math.min(dt, 0.1) * (speed ?? 1);
    let f = Math.floor(c.t * (anim.fps || 30));
    if (loops) f %= anim.frames;
    else if (f >= anim.frames - 1) {
      f = anim.frames - 1;
      if (c.frame !== f) {
        c.frame = f;
        pose(geometry, bundle, anim, f);
      }
      onEnded?.();
      return;
    }
    if (f !== c.frame) {
      c.frame = f;
      pose(geometry, bundle, anim, f);
    }
  });

  // kRenderFxGlowShell: the model drawn again a little larger along its normals, added on in the colour
  const shellMats = useMemo(() => {
    if (!shell) return null;
    const m = new THREE.MeshBasicMaterial({ color: new THREE.Color(shell.color[0] / 255, shell.color[1] / 255, shell.color[2] / 255), transparent: true, opacity: 0.55, blending: THREE.AdditiveBlending, depthWrite: false, side: THREE.FrontSide });
    const push = Math.max(0.5, shell.amount * 0.1);
    m.onBeforeCompile = (sh) => {
      sh.vertexShader = sh.vertexShader.replace("#include <begin_vertex>", `vec3 transformed = vec3(position) + normalize(normal) * ${push.toFixed(2)};`);
    };
    return bundle.draws.map(() => m);
  }, [shell?.color[0], shell?.color[1], shell?.color[2], shell?.amount, bundle]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => () => shellMats?.[0]?.dispose(), [shellMats]);

  return (
    <>
      <mesh geometry={geometry} material={materials} frustumCulled={false} visible={ready} />
      {shellMats && <mesh geometry={geometry} material={shellMats} frustumCulled={false} renderOrder={1} visible={ready} />}
    </>
  );
}

/** Moves the vertices to frame `f` of an animation. */
function pose(g: THREE.BufferGeometry, b: ModelBundle, a: Animation, f: number) {
  if (!b.bones.length) return;
  const nb = Math.min(a.bones, b.bones.length);
  const m = new Float32Array(b.bones.length * 12);
  // identity for any bone the animation doesn't cover
  for (let i = 0; i < b.bones.length; i++) m.set([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0], i * 12);
  const local = new Float32Array(12);
  for (let i = 0; i < nb; i++) {
    const o = (f * a.bones + i) * 7;
    const [px, py, pz, x, y, z, w] = a.data.subarray(o, o + 7);
    local.set([1 - 2 * (y * y + z * z), 2 * (x * y - w * z), 2 * (x * z + w * y), px, 2 * (x * y + w * z), 1 - 2 * (x * x + z * z), 2 * (y * z - w * x), py, 2 * (x * z - w * y), 2 * (y * z + w * x), 1 - 2 * (x * x + y * y), pz]);
    const parent = b.bones[i].parent;
    if (parent >= 0 && parent < i) {
      const P = parent * 12, O = i * 12;
      for (let r = 0; r < 3; r++) {
        for (let c = 0; c < 4; c++) {
          m[O + r * 4 + c] = m[P + r * 4] * local[c] + m[P + r * 4 + 1] * local[4 + c] + m[P + r * 4 + 2] * local[8 + c] + (c === 3 ? m[P + r * 4 + 3] : 0);
        }
      }
    } else m.set(local, i * 12);
  }
  const pos = g.attributes.position.array as Float32Array, nor = g.attributes.normal.array as Float32Array;
  const L = b.local, N = b.localNormals;
  for (let v = 0; v < b.bone.length; v++) {
    const k = v * 3;
    let o = Math.min(b.bone[v], b.bones.length - 1) * 12;
    const x = L[k], y = L[k + 1], z = L[k + 2];
    pos[k] = m[o] * x + m[o + 1] * y + m[o + 2] * z + m[o + 3];
    pos[k + 1] = m[o + 4] * x + m[o + 5] * y + m[o + 6] * z + m[o + 7];
    pos[k + 2] = m[o + 8] * x + m[o + 9] * y + m[o + 10] * z + m[o + 11];
    o = Math.min(b.normalBone[v], b.bones.length - 1) * 12;
    const nx = N[k], ny = N[k + 1], nz = N[k + 2];
    nor[k] = m[o] * nx + m[o + 1] * ny + m[o + 2] * nz;
    nor[k + 1] = m[o + 4] * nx + m[o + 5] * ny + m[o + 6] * nz;
    nor[k + 2] = m[o + 8] * nx + m[o + 9] * ny + m[o + 10] * nz;
  }
  g.attributes.position.needsUpdate = true;
  g.attributes.normal.needsUpdate = true;
}
