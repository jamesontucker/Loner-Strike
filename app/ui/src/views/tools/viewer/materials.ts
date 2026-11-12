// Materials for the map's draws (plan 3.3, M2).
//
// Each texture is its own PNG (repeat wrap, mipmaps); a texture that won't load draws as a
// magenta checker, like the game's missing texture. The map's lighting is one picture (the
// lightmap atlas, read through uv2) multiplied in with an overbright of about 2, as GoldSrc
// does. The canvas renders without colour management ("linear"), so these numbers mean
// what they do in the game. Sky is a flat colour, water is see-through, triggers only show
// with the Triggers layer.

import * as THREE from "three";
import { FLAG, type Draw } from "./bundle";

export const OVERBRIGHT = 2;
export const SKY_COLOUR = "#8aa4ba";

export type MaterialOptions = { textures: boolean; lighting: boolean; triggers: boolean };

/** Names that are tool textures: never drawn in the game, shown with Triggers. */
const TOOL_TEXTURES = /^(aaatrigger|clip|null|origin|hint|skip|bevel)$/i;

export const isSky = (d: Draw) => !!(d.flags & FLAG.sky) || /^sky$/i.test(d.texture);
export const isWater = (d: Draw) => !!(d.flags & FLAG.water) || /^[!*]/.test(d.texture);
export const isTrigger = (d: Draw) => !!(d.flags & FLAG.trigger) || TOOL_TEXTURES.test(d.texture);
export const isMasked = (d: Draw) => !!(d.flags & FLAG.transparent) || d.texture.startsWith("{");

/** A colour that's always the same for a name (textures off, or still loading). */
export function hashColour(name: string): THREE.Color {
  let h = 2166136261;
  for (let i = 0; i < name.length; i++) h = Math.imul(h ^ name.toLowerCase().charCodeAt(i), 16777619);
  const hue = ((h >>> 0) % 360) / 360;
  return new THREE.Color().setHSL(hue, 0.28, 0.55);
}

let checker: THREE.Texture | null = null;
/** The missing-texture pattern: magenta and black squares. */
function checkerTexture() {
  if (checker) return checker;
  const n = 8, data = new Uint8Array(n * n * 4);
  for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) {
    const on = (x >> 2) ^ (y >> 2);
    data.set(on ? [255, 0, 255, 255] : [0, 0, 0, 255], (y * n + x) * 4);
  }
  checker = new THREE.DataTexture(data, n, n);
  checker.wrapS = checker.wrapT = THREE.RepeatWrapping;
  checker.magFilter = THREE.NearestFilter;
  checker.minFilter = THREE.NearestFilter;
  // 8 texels over one texture repeat would be tiny: one repeat = 2x2 squares
  checker.repeat.set(0.5, 0.5);
  checker.needsUpdate = true;
  return checker;
}

type Urls = { tex: (name: string) => Promise<string | null>; lightmap: () => Promise<string | null> };

/**
 * The textures and materials of one open map. Materials are shared by every draw with the
 * same texture and kind; changing options updates them in place.
 */
export class MapMaterials {
  private textures = new Map<string, THREE.Texture | "missing" | "loading">();
  private materials = new Map<string, THREE.MeshBasicMaterial | THREE.MeshLambertMaterial>();
  private lightmap: THREE.Texture | null = null;
  private loader = new THREE.TextureLoader();
  private disposed = false;
  /** the textures that couldn't be loaded */
  readonly missing = new Set<string>();

  /** the side faces are seen from: their front, so you can look into the map from outside */
  private front: THREE.Side;

  constructor(private urls: Urls, private opts: MaterialOptions, private redraw: () => void, private anisotropy = 4, clockwise = false) {
    this.front = clockwise ? THREE.BackSide : THREE.FrontSide;
    urls.lightmap().then((url) => {
      if (!url || this.disposed) return;
      this.loader.load(url, (t) => {
        if (this.disposed) return t.dispose();
        t.flipY = false;
        t.colorSpace = THREE.NoColorSpace;
        t.minFilter = THREE.LinearFilter;
        t.generateMipmaps = false;
        this.lightmap = t;
        this.refresh();
      }, undefined, () => {
        /* no lighting: drawn fullbright */
      });
    });
  }

  private key(d: Draw) {
    const kind = isSky(d) ? "sky" : isTrigger(d) ? "trigger" : isWater(d) ? "water" : isMasked(d) ? "masked" : "solid";
    return `${kind}:${d.texture.toLowerCase()}`;
  }

  private texture(name: string): THREE.Texture | null {
    const k = name.toLowerCase();
    const t = this.textures.get(k);
    if (t === "missing") return checkerTexture();
    if (t) return t === "loading" ? null : t;
    this.textures.set(k, "loading");
    const fail = () => {
      if (this.disposed) return;
      this.textures.set(k, "missing");
      this.missing.add(name);
      this.refresh();
    };
    this.urls.tex(name).then((url) => {
      if (this.disposed) return;
      if (!url) return fail();
      this.loader.load(url, (tex) => {
        if (this.disposed) return tex.dispose();
        tex.flipY = false;
        tex.colorSpace = THREE.NoColorSpace;
        tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
        tex.magFilter = THREE.LinearFilter;
        tex.minFilter = THREE.LinearMipmapLinearFilter;
        tex.generateMipmaps = true;
        tex.anisotropy = this.anisotropy;
        this.textures.set(k, tex);
        this.refresh();
      }, undefined, fail);
    }, fail);
    return null;
  }

  /** The material for a draw (made once per texture and kind). */
  get(d: Draw) {
    const key = this.key(d);
    let m = this.materials.get(key);
    if (!m) {
      m = this.make(d);
      this.materials.set(key, m);
    }
    return m;
  }

  private make(d: Draw) {
    const kind = this.key(d).split(":")[0];
    const lit = this.opts.lighting && !!this.lightmap && kind !== "sky" && kind !== "trigger";
    // with the map's own lighting it's all in the lightmap; without, a simple sun shades the faces
    const m = lit || kind === "sky" || kind === "trigger" ? new THREE.MeshBasicMaterial() : new THREE.MeshLambertMaterial();
    // see-through things are seen from both sides; walls only from inside the map
    m.side = kind === "water" || kind === "masked" || kind === "trigger" ? THREE.DoubleSide : this.front;
    m.name = d.texture;
    m.userData.kind = kind;
    this.fill(m, d.texture, kind);
    return m;
  }

  private fill(m: THREE.MeshBasicMaterial | THREE.MeshLambertMaterial, name: string, kind: string) {
    if (kind === "sky") {
      m.color.set(SKY_COLOUR);
      m.map = null;
    } else if (kind === "trigger") {
      m.color.set("#e08a3c");
      m.map = null;
      m.transparent = true;
      m.opacity = 0.28;
      m.depthWrite = false;
      m.visible = this.opts.triggers;
    } else {
      const tex = this.opts.textures ? this.texture(name) : null;
      m.map = tex;
      m.color.copy(tex ? new THREE.Color(1, 1, 1) : hashColour(name));
      if (kind === "water") {
        m.transparent = true;
        m.opacity = 0.6;
        m.depthWrite = false;
      }
      if (kind === "masked") m.alphaTest = 0.5;
      const lit = this.opts.lighting && !!this.lightmap;
      m.lightMap = lit ? this.lightmap : null;
      if (m.lightMap) m.lightMap.channel = 1;
      m.lightMapIntensity = OVERBRIGHT;
    }
    m.needsUpdate = true;
  }

  /** New options: materials of the wrong type are remade, the rest updated in place. */
  setOptions(opts: MaterialOptions) {
    this.opts = opts;
    this.refresh();
  }

  /** Materials that changed type (lit <-> sun-shaded) are swapped on the next get(). */
  private swapped = false;
  private refresh() {
    if (this.disposed) return;
    for (const [key, m] of this.materials) {
      const [kind, name] = [key.slice(0, key.indexOf(":")), m.name];
      const wantBasic = (this.opts.lighting && !!this.lightmap) || kind === "sky" || kind === "trigger";
      if (wantBasic !== m instanceof THREE.MeshBasicMaterial) {
        m.dispose();
        this.materials.delete(key);
        this.swapped = true;
        continue;
      }
      this.fill(m, name, kind);
    }
    this.onChange?.(this.swapped);
    this.swapped = false;
    this.redraw();
  }

  /** Called after textures arrive or options change; true when materials were remade. */
  onChange: ((remade: boolean) => void) | null = null;

  dispose() {
    this.disposed = true;
    for (const m of this.materials.values()) m.dispose();
    for (const t of this.textures.values()) if (t instanceof THREE.Texture) t.dispose();
    this.lightmap?.dispose();
    this.materials.clear();
    this.textures.clear();
  }
}
