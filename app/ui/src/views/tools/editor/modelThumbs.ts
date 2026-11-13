// P5: model thumbnails for the Content Browser: each model drawn once, in its first skin, from
// the front-right, by one small WebGL renderer shared by every tile (one model at a time, so a
// folder of hundreds never stalls the editor), and kept for this session.

import * as THREE from "three";
import { getModel, modelTextureUrl } from "../assets/assetsApi";

const SIZE = 192;
const cache = new Map<string, string | null>();
const waiting = new Map<string, ((url: string | null) => void)[]>();
const queue: { folder: string; path: string; key: string }[] = [];
let renderer: THREE.WebGLRenderer | null = null;
let running = false;

const keyOf = (m: { folder: string; path: string }) => `${m.folder}/${m.path}`;

/** A cached thumbnail now (undefined: not drawn yet; null: it couldn't be drawn). */
export const cachedThumb = (m: { folder: string; path: string }) => cache.get(keyOf(m));

/** The model's thumbnail (a PNG data URL), or null when it can't be drawn. */
export function modelThumb(m: { folder: string; path: string }): Promise<string | null> {
  const key = keyOf(m);
  if (cache.has(key)) return Promise.resolve(cache.get(key) ?? null);
  return new Promise((done) => {
    const list = waiting.get(key);
    if (list) list.push(done);
    else {
      waiting.set(key, [done]);
      queue.push({ ...m, key });
      void pump();
    }
  });
}

/** Forget queued ones nobody waits for any more (a section closed, a search changed). */
export function dropQueued(keep: (key: string) => boolean) {
  for (let i = queue.length - 1; i >= 0; i--) {
    if (!keep(queue[i].key)) {
      const k = queue[i].key;
      queue.splice(i, 1);
      waiting.delete(k);
    }
  }
}

function loadTex(url: string): Promise<THREE.Texture | null> {
  return new Promise((ok) => new THREE.TextureLoader().load(url, (t) => ok(t), undefined, () => ok(null)));
}

async function draw(m: { folder: string; path: string }): Promise<string | null> {
  const b = await getModel(m);
  renderer ??= new THREE.WebGLRenderer({ antialias: true, alpha: true, preserveDrawingBuffer: true });
  renderer.setSize(SIZE, SIZE, false);
  const scene = new THREE.Scene();
  scene.add(new THREE.AmbientLight(0xffffff, 1.1));
  const sun = new THREE.DirectionalLight(0xffffff, 1.6);
  sun.position.set(1, 1.4, 1.2);
  scene.add(sun);
  const g = new THREE.BufferGeometry();
  g.setAttribute("position", new THREE.BufferAttribute(b.positions, 3));
  g.setAttribute("normal", new THREE.BufferAttribute(b.normals, 3));
  g.setAttribute("uv", new THREE.BufferAttribute(b.uv, 2));
  g.setIndex(new THREE.BufferAttribute(b.indices, 1));
  const pick = (ref: number) => b.skins[0]?.[ref] ?? (b.skins.length ? -1 : ref);
  const used = [...new Set(b.draws.map((d) => pick(d.skinRef)))].filter((i) => i >= 0 && i < b.textures.length);
  const tex = new Map<number, THREE.Texture | null>();
  await Promise.all(used.map(async (i) => tex.set(i, await loadTex(modelTextureUrl(m, i)))));
  const mats = b.draws.map((d) => {
    const ti = pick(d.skinRef);
    const t = tex.get(ti) ?? null;
    if (t) {
      t.flipY = false;
      t.wrapS = t.wrapT = THREE.RepeatWrapping;
    }
    const info = b.textures[ti];
    return new THREE.MeshLambertMaterial({ map: t, color: t ? 0xffffff : 0x9a9a8a, side: THREE.DoubleSide, alphaTest: info?.masked ? 0.5 : 0, transparent: !!info?.additive });
  });
  // the first model of each body part
  b.draws.forEach((d, i) => d.model === 0 && d.count > 0 && g.addGroup(d.start, d.count, i));
  const mesh = new THREE.Mesh(g, mats);
  // GoldSrc is z-up
  mesh.rotation.x = -Math.PI / 2;
  scene.add(mesh);
  g.computeBoundingBox();
  const box = new THREE.Box3().setFromObject(mesh);
  const c = box.getCenter(new THREE.Vector3());
  const r = Math.max(1, box.getSize(new THREE.Vector3()).length() / 2);
  const cam = new THREE.PerspectiveCamera(32, 1, r / 50, r * 20);
  const dir = new THREE.Vector3(0.75, 0.38, 1).normalize();
  cam.position.copy(c).addScaledVector(dir, r / Math.sin((16 * Math.PI) / 180));
  cam.lookAt(c);
  renderer.setClearColor(0x000000, 0);
  renderer.render(scene, cam);
  const url = renderer.domElement.toDataURL("image/png");
  g.dispose();
  mats.forEach((x) => x.dispose());
  tex.forEach((t) => t?.dispose());
  return url;
}

async function pump() {
  if (running) return;
  running = true;
  try {
    while (queue.length) {
      const job = queue.shift()!;
      let url: string | null = null;
      try {
        url = await draw(job);
      } catch {
        url = null;
      }
      cache.set(job.key, url);
      for (const done of waiting.get(job.key) ?? []) done(url);
      waiting.delete(job.key);
      // (a breath between models, so the editor's own frames go first)
      await new Promise((r) => setTimeout(r, 0));
    }
  } finally {
    running = false;
  }
}
