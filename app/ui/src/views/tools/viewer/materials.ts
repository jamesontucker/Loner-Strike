// Materials for the map's draws (plan 3.3, M2; the game's look: docs/TOOLS-RENDER.md).
//
// Each texture is its own PNG (repeat wrap, mipmaps); a texture that won't load draws as a
// magenta checker, like the game's missing texture. With "Lighting" on, surfaces are drawn
// the way the game draws them (gameLook.ts): texgamma on the textures, every light style of
// the lightmap added up with its current brightness and sent through the lightgamma table,
// water full-bright and wobbling, sky faces showing the sky box, brush entities by their
// render mode. "Bright" is the same with the brightness lifted. With "Lighting" off a simple
// sun shades the faces (the editor draws that way too). The canvas renders without colour
// management ("linear flat"), so these numbers mean what they do in the game. Triggers only
// show with the Triggers layer.

import * as THREE from "three";
import { FLAG, type Draw } from "./bundle";
import { applySettings, effectiveSettings, GAME_DEFAULTS, lookUniforms, NORMAL_RENDER, SkyMaterial, SurfaceMaterial, type LookMode, type LookSettings, type LookUniforms, type RenderMode, type SurfaceKind } from "./gameLook";

/** Kept for anything that still multiplies a lightmap in by hand (the game's gl_overbright 1). */
export const OVERBRIGHT = 2;
export const SKY_COLOUR = "#8aa4ba";

/** sky: draw the sky faces. Sky volumes have faces on both sides (the map keeps them as
 *  their own contents), so from outside they're a lid over the map: shown only when flying.
 *  look: "game" (default) draws as the game does, "bright" lifts the dark parts. */
export type MaterialOptions = { textures: boolean; lighting: boolean; triggers: boolean; sky: boolean; look?: LookMode };

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

/** The game's gl_texturemode as three's filters (min, mag, mipmaps). */
function filters(mode: string): [THREE.MinificationTextureFilter, THREE.MagnificationTextureFilter, boolean] {
  switch (mode) {
    case "GL_NEAREST": return [THREE.NearestFilter, THREE.NearestFilter, false];
    case "GL_LINEAR": return [THREE.LinearFilter, THREE.LinearFilter, false];
    case "GL_NEAREST_MIPMAP_NEAREST": return [THREE.NearestMipmapNearestFilter, THREE.NearestFilter, true];
    case "GL_NEAREST_MIPMAP_LINEAR": return [THREE.NearestMipmapLinearFilter, THREE.NearestFilter, true];
    case "GL_LINEAR_MIPMAP_NEAREST": return [THREE.LinearMipmapNearestFilter, THREE.LinearFilter, true];
    default: return [THREE.LinearMipmapLinearFilter, THREE.LinearFilter, true];
  }
}

type Urls = { tex: (name: string) => Promise<string | null>; lightmap: () => Promise<string | null> };
type AnyMaterial = THREE.MeshBasicMaterial | THREE.MeshLambertMaterial | SurfaceMaterial | SkyMaterial;
type Kind = "sky" | "trigger" | SurfaceKind;
/** which material class a draw needs */
type Type = "basic" | "lambert" | "surface" | "sky";

const renderKey = (r: RenderMode) => (r.mode === 0 ? "" : `|${r.mode},${r.amt},${r.colour.join(",")}`);

/**
 * The textures and materials of one open map. Materials are shared by every draw with the
 * same texture, kind and render mode; changing options updates them in place.
 */
export class MapMaterials {
  private textures = new Map<string, THREE.Texture | "missing" | "loading">();
  private materials = new Map<string, AnyMaterial>();
  private renders = new Map<string, RenderMode>();
  private lightmap: THREE.Texture | null = null;
  private sky: THREE.Texture | null = null;
  private loader = new THREE.TextureLoader();
  private disposed = false;
  private settings: LookSettings = GAME_DEFAULTS;
  /** the textures that couldn't be loaded */
  readonly missing = new Set<string>();
  /** shared by every game-look material: the clock, the light styles, the gamma settings */
  readonly uniforms: LookUniforms = lookUniforms();
  /** the lightmap's planes (light styles; docs/TOOLS-RENDER.md), from the bundle header */
  private planes = { maps: 1, planes: 1, height: 0 };

  /** the side faces are seen from: their front, so you can look into the map from outside */
  private front: THREE.Side;

  constructor(private urls: Urls, private opts: MaterialOptions, private redraw: () => void, private anisotropy = 4, clockwise = false, lightmapInfo?: { maps?: number; planes?: number; height?: number }) {
    this.front = clockwise ? THREE.BackSide : THREE.FrontSide;
    if (lightmapInfo) this.planes = { maps: Math.max(1, Math.min(4, lightmapInfo.maps ?? 1)), planes: Math.max(1, lightmapInfo.planes ?? 1), height: lightmapInfo.height ?? 0 };
    applySettings(this.uniforms, effectiveSettings(this.settings, opts.look ?? "game"));
    urls.lightmap().then((url) => {
      if (!url || this.disposed) return;
      this.loader.load(url, (t) => {
        if (this.disposed) return t.dispose();
        t.flipY = false;
        t.colorSpace = THREE.NoColorSpace;
        t.minFilter = THREE.LinearFilter;
        t.magFilter = THREE.LinearFilter;
        t.generateMipmaps = false;
        // an older picture (or the dev mock) has one plane: style 0 only
        const img = t.image as { width: number; height: number };
        if (!this.planes.height || img.height !== this.planes.height * this.planes.planes) this.planes = { maps: 1, planes: 1, height: img.height };
        this.lightmap = t;
        this.refresh();
      }, undefined, () => {
        /* no lighting: drawn with the sun */
      });
    });
  }

  private kind(d: Draw): Kind {
    return isSky(d) ? "sky" : isTrigger(d) ? "trigger" : isWater(d) ? "water" : isMasked(d) ? "masked" : "solid";
  }

  private gameLook() {
    return this.opts.lighting && !!this.lightmap;
  }

  private typeOf(kind: Kind): Type {
    if (kind === "trigger") return "basic";
    if (kind === "sky") return this.gameLook() ? "sky" : "basic";
    return this.gameLook() ? "surface" : "lambert";
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
        this.filter(tex);
        this.textures.set(k, tex);
        this.refresh();
      }, undefined, fail);
    }, fail);
    return null;
  }

  private filter(tex: THREE.Texture) {
    const [min, mag, mips] = filters(this.settings.texturemode);
    tex.minFilter = min;
    tex.magFilter = mag;
    tex.generateMipmaps = mips;
    tex.anisotropy = Math.min(this.anisotropy, mips ? this.settings.anisotropy : 1);
  }

  /** The material for a draw (made once per texture, kind and render mode). `render`: the
   *  brush entity's render mode, amount and colour (normal when left out). */
  get(d: Draw, render: RenderMode = NORMAL_RENDER) {
    const kind = this.kind(d);
    const key = `${kind}:${d.texture.toLowerCase()}${renderKey(render)}`;
    let m = this.materials.get(key);
    if (!m) {
      this.renders.set(key, render);
      m = this.make(d.texture, kind, render);
      this.materials.set(key, m);
    }
    return m;
  }

  private make(name: string, kind: Kind, render: RenderMode): AnyMaterial {
    const type = this.typeOf(kind);
    const m: AnyMaterial =
      type === "surface" ? new SurfaceMaterial(this.uniforms)
      : type === "sky" ? new SkyMaterial(SKY_COLOUR)
      : type === "basic" ? new THREE.MeshBasicMaterial()
      : new THREE.MeshLambertMaterial();
    // walls only from inside the map; water already has a face for each side (like sky), so
    // it's front-only too; fences and triggers from both sides
    m.side = kind === "masked" || kind === "trigger" ? THREE.DoubleSide : this.front;
    m.name = name;
    m.userData.kind = kind;
    m.userData.type = type;
    this.fill(m, name, kind, render);
    return m;
  }

  private fill(m: AnyMaterial, name: string, kind: Kind, render: RenderMode) {
    if (m instanceof SkyMaterial) {
      m.uniforms.sky.value = this.sky;
      m.uniforms.hasSky.value = !!this.sky;
      if (this.sky) m.uniforms.faceInset.value = 0.5 / (this.sky.userData.side || 256);
      m.visible = this.opts.sky;
    } else if (m instanceof SurfaceMaterial) {
      const u = m.uniforms;
      const tex = this.opts.textures ? this.texture(name) : null;
      u.map.value = tex;
      u.hasMap.value = !!tex;
      u.diffuse.value.copy(tex ? new THREE.Color(1, 1, 1) : hashColour(name));
      const img = tex?.image as { width?: number; height?: number } | undefined;
      u.texSize.value.set(img?.width || 64, img?.height || 64);
      m.setRender(render, kind as SurfaceKind);
      // the game draws no lightmap on water, nor on see-through render modes
      const lit = kind !== "water" && (render.mode === 0 || render.mode === 4);
      u.lit.value = lit && !!this.lightmap;
      u.lightmap.value = this.lightmap;
      const img2 = this.lightmap?.image as { width: number; height: number } | undefined;
      u.lmSize.value.set(img2?.width ?? 1, this.planes.height || img2?.height || 1);
      u.lmPlanes.value = this.planes.planes;
      u.lmMaps.value = this.planes.maps;
      // a see-through render mode with no amount isn't drawn at all
      m.visible = !(render.mode !== 0 && render.amt <= 0);
    } else if (kind === "sky") {
      m.color.set(SKY_COLOUR);
      m.map = null;
      m.visible = this.opts.sky;
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
    }
    m.needsUpdate = true;
  }

  /** New options: materials of the wrong type are remade, the rest updated in place. */
  setOptions(opts: MaterialOptions) {
    const lookChanged = (opts.look ?? "game") !== (this.opts.look ?? "game");
    this.opts = opts;
    if (lookChanged) applySettings(this.uniforms, effectiveSettings(this.settings, opts.look ?? "game"));
    this.refresh();
  }

  /** The player's picture settings (from the game's config): gamma, filtering and so on. */
  setSettings(s: LookSettings) {
    const filterChanged = s.texturemode !== this.settings.texturemode || s.anisotropy !== this.settings.anisotropy;
    this.settings = s;
    applySettings(this.uniforms, effectiveSettings(s, this.opts.look ?? "game"));
    if (filterChanged) for (const t of this.textures.values()) if (t instanceof THREE.Texture) {
      this.filter(t);
      t.needsUpdate = true;
    }
    this.refresh();
  }

  /** The sky box picture (six sides in one texture; gameLook.loadSky). */
  setSky(t: THREE.Texture | null) {
    if (this.disposed) return void t?.dispose();
    this.sky?.dispose();
    this.sky = t;
    this.refresh();
  }

  /** Materials that changed type are swapped on the next get(). */
  private swapped = false;
  private refresh() {
    if (this.disposed) return;
    for (const [key, m] of this.materials) {
      const kind = m.userData.kind as Kind;
      if (this.typeOf(kind) !== m.userData.type) {
        m.dispose();
        this.materials.delete(key);
        this.swapped = true;
        continue;
      }
      this.fill(m, m.name, kind, this.renders.get(key) ?? NORMAL_RENDER);
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
    this.sky?.dispose();
    this.materials.clear();
    this.textures.clear();
  }
}
