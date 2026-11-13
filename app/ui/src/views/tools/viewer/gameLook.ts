// How Counter-Strike 1.6 / Condition Zero draw a map (the GoldSrc OpenGL renderer), as
// numbers and shaders for the viewer. Every formula here is written out with its source
// in docs/TOOLS-RENDER.md; the short version:
//
// - Textures are uploaded through the "texgamma" table: c' = 255 * (c/255)^(texgamma/gamma).
// - Lightmaps: each face has up to four pictures, one per light style. Every frame the game
//   adds them up, each times its style's brightness ('a' = 0 ... 'm' = 264 ... 'z' = 550),
//   scales the sum (by 2^(1/lightgamma) without gl_overbright, so a luxel of 128 already
//   means "full texture"), caps it, and sends it through the "lightgamma" table (linearise
//   with lightgamma, the brightness lift, back with 1/gamma). The texture is multiplied by
//   that (and by 2 more with gl_overbright 1).
// - Styles animate at 10 letters a second; light entities with a style of 32 or more are
//   switchable lights (on: their pattern or "m"; "start off": "a").
// - Water (`!`, `*`) has no lightmap: full-bright, its texture coordinates wobble
//   (8 * sin), one texture repeat per 64 units. Sky faces show the sky box behind them.
// - Brush entities: render mode 0 normal, 1 colour (rendercolor, renderamt see-through, no
//   texture), 2 texture and 3 glow (see-through by renderamt, no lightmap), 4 solid (holes
//   where the `{` texture is clear: alpha above 0.25), 5 additive (texture * renderamt added
//   on top, no lightmap). renderamt 0 with any mode but 0: not drawn. `{` textures on the
//   world or a mode-0 entity aren't see-through: their clear pixels are black.

import * as THREE from "three";
import type { MapEntity } from "../toolsApi";

/** The player's picture settings (the game's own defaults until /look says otherwise). */
export type LookSettings = { gamma: number; brightness: number; texgamma: number; lightgamma: number; overbright: boolean; anisotropy: number; texturemode: string };
export const GAME_DEFAULTS: LookSettings = { gamma: 2.5, brightness: 1, texgamma: 2, lightgamma: 2.5, overbright: false, anisotropy: 16, texturemode: "GL_LINEAR_MIPMAP_LINEAR" };

/** "game": as the game draws it; "bright": the same but lifted, for dark corners. */
export type LookMode = "game" | "bright";

/** What "Bright" changes: the game's brightness slider at its top and overbright on. */
export function effectiveSettings(s: LookSettings, mode: LookMode): LookSettings {
  return mode === "bright" ? { ...s, brightness: 3, overbright: true } : s;
}

// ---- the gamma tables -----------------------------------------------------------------------

/** The brightness lift's knee (the engine's g3). */
export function brightnessKnee(brightness: number) {
  if (brightness <= 0) return 0.125;
  if (brightness <= 1) return 0.125 - brightness * brightness * 0.075;
  return 0.05;
}

/** The lightgamma table: a light level 0..1023 → 0..1023. */
export function lightGamma(i: number, s: LookSettings): number {
  let f = Math.pow(i / 1023, s.lightgamma);
  if (s.brightness > 1) f *= s.brightness;
  const g3 = brightnessKnee(s.brightness);
  f = f <= g3 ? (f / g3) * 0.125 : ((f - g3) / (1 - g3)) * 0.875 + 0.125;
  return Math.max(0, Math.min(1023, Math.floor(Math.pow(f, 1 / s.gamma) * 1023)));
}

/** The texgamma table: a texture's colour 0..255 → 0..255. */
export const texGamma = (c: number, s: LookSettings) => Math.max(0, Math.min(255, Math.floor(Math.pow(c / 255, s.texgamma / s.gamma) * 255)));

/** The lightmap scale before the table: 256 with gl_overbright, else 2^(1/lightgamma) * 256. */
export const lightScale = (s: LookSettings) => (s.overbright ? 256 : Math.floor(Math.pow(2, 1 / s.lightgamma) * 256 + 0.5));

/** One luxel as the game ends up drawing it (0..255 per channel, before the texture): the
 *  styles' pictures `samples[k]` (0..255) times their brightness `values[k]` (264 = normal). */
export function luxel(samples: number[], values: number[], s: LookSettings): number {
  let acc = 0;
  samples.forEach((v, k) => (acc += v * (values[k] ?? 0)));
  const t = Math.min(1023, Math.floor((acc * lightScale(s)) / 16384));
  return lightGamma(t, s) >> 2;
}

// ---- light styles ---------------------------------------------------------------------------

/** The game's built-in styles (the game DLL's world setup; the same letters as Quake's but 7). */
export const STYLE_PATTERNS: Record<number, string> = {
  0: "m",
  1: "mmnmmommommnonmmonqnmmo", // flicker
  2: "abcdefghijklmnopqrstuvwxyzyxwvutsrqponmlkjihgfedcba", // slow strong pulse
  3: "mmmmmaaaaammmmmaaaaaabcdefgabcdefg", // candle
  4: "mamamamamama", // fast strobe
  5: "jklmnopqrstuvwxyzyxwvutsrqponmlkj", // gentle pulse
  6: "nmonqnmomnmomomno", // flicker 2
  7: "mmmaaaabcdefgmmmmaaaammmaamm", // candle 2
  8: "mmmaaammmaaammmabcdefaaaammmmabcdefmmmaaaa", // candle 3
  9: "aaaaaaaazzzzzzzz", // slow strobe
  10: "mmamammmmammamamaaamammma", // fluorescent flicker
  11: "abcdefghijklmnopqrrqponmlkjihgfedcba", // slow pulse, not to black
  12: "mmnnmmnnnmmnn", // underwater
  63: "a",
};
export const MAX_STYLES = 64;

/** Every style's letters for this map: the built-in ones, then switchable lights (style 32+)
 *  as they start: off ("a") with the "Initially dark" flag, else their pattern or "m". */
export function styleStrings(entities: MapEntity[]): Map<number, string> {
  const out = new Map<number, string>(Object.entries(STYLE_PATTERNS).map(([k, v]) => [Number(k), v]));
  for (const e of entities) {
    if (!/^light(_spot|_environment)?$/.test(e.classname)) continue;
    const style = Number(e.keys.style);
    if (!Number.isInteger(style) || style < 32 || style >= MAX_STYLES) continue;
    const off = (Number(e.keys.spawnflags) & 1) !== 0;
    const pattern = (e.keys.pattern ?? "").trim();
    out.set(style, off ? "a" : /^[a-z]+$/.test(pattern) ? pattern : "m");
  }
  return out;
}

/** Whether any style the map uses changes over time. */
export const animates = (styles: Map<number, string>, used: Set<number>) => [...used].some((s) => (styles.get(s)?.length ?? 0) > 1);

/** Each style's brightness at `time` seconds (10 letters a second, no blending, as the game);
 *  a style nobody set is 256. */
export function styleValues(styles: Map<number, string>, time: number, out = new Float32Array(MAX_STYLES)): Float32Array {
  const frame = Math.floor(time * 10);
  for (let i = 0; i < MAX_STYLES; i++) {
    const p = styles.get(i);
    out[i] = !p ? 256 : (p.charCodeAt(p.length === 1 ? 0 : frame % p.length) - 97) * 22;
  }
  return out;
}

// ---- model shading (the studio renderer) ---------------------------------------------------

export type ModelLight = { ambient: number; shade: number; colour: [number, number, number]; dir: [number, number, number] };

/** The light a model gets from its spot (`light` 0..255+, `dir` the way it travels), as the
 *  engine shares it into ambient and directional parts (v_direct 0.9). */
export function modelLight(light: [number, number, number], dir: [number, number, number]): ModelLight {
  let total = Math.max(light[0], light[1], light[2]);
  if (total === 0) total = 1;
  let shade = total * 0.9;
  let ambient = total - shade;
  const top = Math.max(light[0], light[1], light[2]);
  const colour: [number, number, number] = top > 0 ? [light[0] / top, light[1] / top, light[2] / top] : [1, 1, 1];
  if (ambient > 128) ambient = 128;
  if (ambient + shade > 255) shade = 255 - ambient;
  const len = Math.hypot(...dir) || 1;
  return { ambient, shade, colour, dir: [dir[0] / len, dir[1] / len, dir[2] / len] };
}

/** One vertex's light (0..1) for a normal (unit, GoldSrc axes), as the studio renderer. */
export function modelVertexLight(n: [number, number, number], l: ModelLight, s: LookSettings): number {
  const SHADE_LAMBERT = 1.4953241;
  let cos = Math.min(1, n[0] * l.dir[0] + n[1] * l.dir[1] + n[2] * l.dir[2]);
  let illum = l.ambient + l.shade;
  cos = (cos + (SHADE_LAMBERT - 1)) / SHADE_LAMBERT;
  if (cos > 0) illum -= l.shade * cos;
  illum = Math.min(255, Math.max(0, illum));
  return lightGamma(Math.floor(illum * 4), s) / 1023;
}

// ---- shared uniforms ------------------------------------------------------------------------

/** The uniforms every map material shares (one object per open map): the clock, the styles'
 *  brightness, and the gamma settings. */
export function lookUniforms() {
  return {
    uTime: { value: 0 },
    uStyles: { value: styleValues(new Map(Object.entries(STYLE_PATTERNS).map(([k, v]) => [Number(k), v])), 0) },
    uGamma: { value: GAME_DEFAULTS.gamma },
    uLightGamma: { value: GAME_DEFAULTS.lightgamma },
    uBrightness: { value: GAME_DEFAULTS.brightness },
    uKnee: { value: brightnessKnee(GAME_DEFAULTS.brightness) },
    uLightScale: { value: lightScale(GAME_DEFAULTS) },
    uOverbright: { value: 1 },
    uTexGamma: { value: GAME_DEFAULTS.texgamma / GAME_DEFAULTS.gamma },
  };
}
export type LookUniforms = ReturnType<typeof lookUniforms>;

export function applySettings(u: LookUniforms, s: LookSettings) {
  u.uGamma.value = s.gamma;
  u.uLightGamma.value = s.lightgamma;
  u.uBrightness.value = s.brightness;
  u.uKnee.value = brightnessKnee(s.brightness);
  u.uLightScale.value = lightScale(s);
  u.uOverbright.value = s.overbright ? 2 : 1;
  u.uTexGamma.value = s.texgamma / s.gamma;
}

// ---- the surface shader ---------------------------------------------------------------------

/** How a draw is drawn (kinds from materials.ts). */
export type SurfaceKind = "solid" | "masked" | "water";

export type RenderMode = { mode: number; amt: number; colour: [number, number, number] };
export const NORMAL_RENDER: RenderMode = { mode: 0, amt: 255, colour: [255, 255, 255] };

/** An entity's render mode, amount and colour from its keys. */
export function renderOf(keys: Record<string, string> | undefined | null): RenderMode {
  if (!keys) return NORMAL_RENDER;
  const mode = Math.trunc(Number(keys.rendermode ?? 0));
  const amt = Number(keys.renderamt ?? (mode ? 0 : 255));
  const c = (keys.rendercolor ?? "").trim().split(/\s+/).map(Number);
  return {
    mode: Number.isFinite(mode) && mode >= 0 && mode <= 5 ? mode : 0,
    amt: Number.isFinite(amt) ? Math.max(0, Math.min(255, amt)) : 255,
    colour: c.length >= 3 && c.slice(0, 3).every(Number.isFinite) ? [c[0], c[1], c[2]] : [255, 255, 255],
  };
}

const VERT = /* glsl */ `
#ifndef USE_UV1
attribute vec2 uv1;
#endif
varying vec2 vUv;
varying vec2 vLm;
void main() {
  vUv = uv;
  vLm = uv1;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}`;

const FRAG = /* glsl */ `
precision highp float;
uniform sampler2D map;
uniform bool hasMap;
uniform vec3 diffuse;
uniform vec2 texSize;
uniform sampler2D lightmap;
uniform bool lit;
uniform vec2 lmSize;   // one plane's width, height (texels)
uniform float lmPlanes;
uniform int lmMaps;
uniform float uStyles[${MAX_STYLES}];
uniform float uTime, uGamma, uLightGamma, uBrightness, uKnee, uLightScale, uOverbright, uTexGamma;
uniform int kind;      // 0 solid, 1 masked, 2 water
uniform int mode;      // render mode 0-5
uniform float amt;     // renderamt 0..1
uniform vec3 rcolour;  // rendercolor 0..1
varying vec2 vUv;
varying vec2 vLm;

float styleValue(float id) {
  if (id > 254.5) return 0.0;
  int i = int(id + 0.5);
  return i < ${MAX_STYLES} ? uStyles[i] : 256.0;
}

// the lightgamma table, 0..1023 in and out
float lightTable(float t) {
  float f = pow(t / 1023.0, uLightGamma);
  if (uBrightness > 1.0) f *= uBrightness;
  f = f <= uKnee ? (f / uKnee) * 0.125 : ((f - uKnee) / (1.0 - uKnee)) * 0.875 + 0.125;
  return clamp(floor(pow(f, 1.0 / uGamma) * 1023.0), 0.0, 1023.0);
}

vec3 lightAt(vec2 uv) {
  float maps = float(lmMaps);
  vec3 ids = vec3(0.0, 255.0, 255.0);
  float id3 = 255.0;
  if (lmPlanes > maps + 0.5) {
    ivec2 px = ivec2(floor(uv * lmSize));
    px = clamp(px, ivec2(0), ivec2(lmSize) - 1);
    ids = texelFetch(lightmap, px + ivec2(0, int(maps * lmSize.y)), 0).rgb * 255.0;
    if (lmMaps >= 4) id3 = texelFetch(lightmap, px + ivec2(0, int((maps + 1.0) * lmSize.y)), 0).r * 255.0;
  }
  vec3 acc = vec3(0.0);
  for (int k = 0; k < 4; k++) {
    if (k >= lmMaps) break;
    float id = k == 0 ? ids.r : k == 1 ? ids.g : k == 2 ? ids.b : id3;
    float v = styleValue(id);
    if (v <= 0.0) continue;
    vec3 s = texture2D(lightmap, vec2(uv.x, (uv.y + float(k)) / lmPlanes)).rgb * 255.0;
    acc += floor(s + 0.5) * v;
  }
  vec3 t = min(floor(acc * uLightScale / 16384.0), vec3(1023.0));
  vec3 g = vec3(lightTable(t.r), lightTable(t.g), lightTable(t.b));
  return floor(g / 4.0) / 255.0;
}

void main() {
  vec2 uv = vUv;
  if (kind == 2) {
    // water: coordinates wobble, one repeat per 64 texture units
    vec2 st = vUv * texSize;
    uv = vec2(st.x + 8.0 * sin(st.y * 0.125 + uTime), st.y + 8.0 * sin(st.x * 0.125 + uTime)) / 64.0;
  }
  vec4 tex = hasMap ? texture2D(map, uv) : vec4(diffuse, 1.0);
  // the texgamma table (the game applies it as the texture is uploaded)
  tex.rgb = floor(pow(tex.rgb, vec3(uTexGamma)) * 255.0) / 255.0;
  vec4 c = tex;
  if (mode == 1) c = vec4(rcolour, amt);
  else if (mode == 2 || mode == 3) c.a *= amt;
  else if (mode == 5) c = vec4(tex.rgb * amt * tex.a, 1.0);
  else if (mode == 4) {
    if (tex.a <= 0.25) discard;
    c.a = 1.0;
  } else c.a = 1.0;
  if (lit) c.rgb = min(c.rgb * lightAt(vLm) * uOverbright, vec3(1.0));
  gl_FragColor = c;
}`;

/** The uniforms of one surface material (on top of the shared ones). */
function surfaceUniforms(shared: LookUniforms) {
  return {
    ...shared,
    map: { value: null as THREE.Texture | null },
    hasMap: { value: false },
    diffuse: { value: new THREE.Color(1, 1, 1) },
    texSize: { value: new THREE.Vector2(64, 64) },
    lightmap: { value: null as THREE.Texture | null },
    lit: { value: false },
    lmSize: { value: new THREE.Vector2(1, 1) },
    lmPlanes: { value: 1 },
    lmMaps: { value: 1 },
    kind: { value: 0 },
    mode: { value: 0 },
    amt: { value: 1 },
    rcolour: { value: new THREE.Color(1, 1, 1) },
  };
}

/** A map surface drawn the game's way. Its uniforms are filled by MapMaterials. */
export class SurfaceMaterial extends THREE.ShaderMaterial {
  declare uniforms: ReturnType<typeof surfaceUniforms>;
  constructor(shared: LookUniforms) {
    super({ uniforms: surfaceUniforms(shared), vertexShader: VERT, fragmentShader: FRAG });
  }
  /** The blending, depth and visibility for a render mode (as the game sets them up). */
  setRender(r: RenderMode, kind: SurfaceKind) {
    const u = this.uniforms;
    u.mode.value = r.mode;
    u.amt.value = r.amt / 255;
    u.rcolour.value.setRGB(r.colour[0] / 255, r.colour[1] / 255, r.colour[2] / 255);
    u.kind.value = kind === "water" ? 2 : kind === "masked" ? 1 : 0;
    const see = r.mode === 1 || r.mode === 2 || r.mode === 3 || r.mode === 5;
    this.transparent = see;
    this.depthWrite = !see || r.mode === 1;
    this.blending = r.mode === 5 ? THREE.AdditiveBlending : THREE.NormalBlending;
    // additive: the texture's colour (already times renderamt) added as it is
    if (r.mode === 5) {
      this.blending = THREE.CustomBlending;
      this.blendSrc = THREE.OneFactor;
      this.blendDst = THREE.OneFactor;
      this.blendEquation = THREE.AddEquation;
    }
  }
}

// ---- the sky box ----------------------------------------------------------------------------

/** The six sides as the game names them, in its order. */
export const SKY_SIDES = ["rt", "bk", "lf", "ft", "up", "dn"] as const;

const SKY_VERT = /* glsl */ `
varying vec3 vWorld;
void main() {
  vec4 w = modelMatrix * vec4(position, 1.0);
  vWorld = w.xyz;
  gl_Position = projectionMatrix * viewMatrix * w;
}`;

// The box: the direction from the eye picks a side and a spot on it, the way the game lays
// its six pictures around the view (rt looks along +x with +y on its left, up has -x at its
// top, and so on). The six pictures sit side by side in one texture, in the game's order.
const SKY_FRAG = /* glsl */ `
precision highp float;
uniform sampler2D sky;
uniform bool hasSky;
uniform vec3 skyColour;
uniform float faceInset;
varying vec3 vWorld;
void main() {
  vec3 w = vWorld - cameraPosition;
  vec3 d = vec3(w.x, -w.z, w.y); // three's y-up back to GoldSrc's z-up
  vec3 a = abs(d);
  float face; float s; float t;
  if (a.x >= a.y && a.x >= a.z) {
    if (d.x > 0.0) { face = 0.0; s = -d.y / a.x; t = d.z / a.x; }
    else { face = 1.0; s = d.y / a.x; t = d.z / a.x; }
  } else if (a.y >= a.z) {
    if (d.y > 0.0) { face = 2.0; s = d.x / a.y; t = d.z / a.y; }
    else { face = 3.0; s = -d.x / a.y; t = d.z / a.y; }
  } else {
    if (d.z > 0.0) { face = 4.0; s = -d.y / a.z; t = -d.x / a.z; }
    else { face = 5.0; s = -d.y / a.z; t = d.x / a.z; }
  }
  if (!hasSky) { gl_FragColor = vec4(skyColour, 1.0); return; }
  float u = clamp((s + 1.0) * 0.5, faceInset, 1.0 - faceInset);
  float v = clamp((1.0 - t) * 0.5, faceInset, 1.0 - faceInset);
  gl_FragColor = vec4(texture2D(sky, vec2((face + u) / 6.0, v)).rgb, 1.0);
}`;

export class SkyMaterial extends THREE.ShaderMaterial {
  declare uniforms: { sky: { value: THREE.Texture | null }; hasSky: { value: boolean }; skyColour: { value: THREE.Color }; faceInset: { value: number } };
  constructor(colour: string) {
    super({
      uniforms: { sky: { value: null }, hasSky: { value: false }, skyColour: { value: new THREE.Color(colour) }, faceInset: { value: 0.5 / 256 } },
      vertexShader: SKY_VERT,
      fragmentShader: SKY_FRAG,
    });
  }
}

/** Loads the six sky pictures into one texture (side by side, each scaled to the biggest).
 *  Null if any side is missing (the game then draws no sky box either). */
export async function loadSky(urls: string[]): Promise<THREE.Texture | null> {
  const imgs = await Promise.all(urls.map((u) => new Promise<HTMLImageElement | null>((res) => {
    const i = new Image();
    i.onload = () => res(i);
    i.onerror = () => res(null);
    i.src = u;
  })));
  if (imgs.some((i) => !i)) return null;
  const n = Math.min(1024, Math.max(...imgs.map((i) => Math.max(i!.naturalWidth, i!.naturalHeight))));
  const canvas = document.createElement("canvas");
  canvas.width = n * 6;
  canvas.height = n;
  const g = canvas.getContext("2d");
  if (!g) return null;
  imgs.forEach((img, k) => g.drawImage(img!, k * n, 0, n, n));
  const t = new THREE.CanvasTexture(canvas);
  t.flipY = false;
  t.colorSpace = THREE.NoColorSpace;
  t.minFilter = THREE.LinearFilter;
  t.magFilter = THREE.LinearFilter;
  t.generateMipmaps = false;
  t.userData.side = n;
  return t;
}
