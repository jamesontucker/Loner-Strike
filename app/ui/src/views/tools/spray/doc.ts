// The spray composer's document: a few layers (pictures, words, stickers) on a canvas the
// spray's shape, drawn bottom to top. Everything here is plain data, so a design saves as
// JSON (tools/spray.rs keeps it in sprays/designs) and undo is a list of earlier documents.
//
// The canvas is in design units: 512 on its long side. The spray the game gets is much
// smaller (at most 12288 pixels: 96×96 square, 128×96 wide), so the page hands the server
// the drawing at 512 and the server scales it and reduces its colours exactly as for a
// picture from the file dialog (goldsrc/wadwrite.rs).

import { STICKERS } from "./stickers";

export type Shape = "square" | "wide" | "tall";
export const SHAPES: Record<Shape, { w: number; h: number; label: string }> = {
  square: { w: 512, h: 512, label: "Square" },
  wide: { w: 512, h: 384, label: "Wide" },
  tall: { w: 384, h: 512, label: "Tall" },
};

type Base = {
  id: string;
  name: string;
  /** the middle of the layer, in design units */
  x: number;
  y: number;
  /** degrees, clockwise */
  rot: number;
  scale: number;
  flipX: boolean;
  flipY: boolean;
  /** 0-1: 1 = fully shown, 0 = hidden */
  opacity: number;
  hidden: boolean;
  locked: boolean;
};

export type ImageLayer = Base & {
  kind: "image";
  /** a PNG data URL (scaled to at most 1024 on its long side when added) */
  src: string;
  /** its size at scale 1, in design units */
  w: number;
  h: number;
  /** make the colour in its top-left corner (and close ones) see-through */
  keyOut: boolean;
  /** paint the picture in one colour: 0 = its own colours, 1 = all `tint` */
  tint: string;
  tintAmount: number;
};

export type TextLayer = Base & {
  kind: "text";
  text: string;
  font: string;
  /** font size in design units at scale 1 */
  size: number;
  bold: boolean;
  italic: boolean;
  colour: string;
  outline: number;
  outlineColour: string;
  shadow: boolean;
  shadowColour: string;
};

export type ShapeLayer = Base & {
  kind: "shape";
  /** a STICKERS id */
  shape: string;
  w: number;
  h: number;
  colour: string;
  outline: number;
  outlineColour: string;
};

export type Layer = ImageLayer | TextLayer | ShapeLayer;

export type Doc = {
  /** 2: layers carry `opacity` (1 = fully shown). A layer with a `seeThrough` value (0-1, the
   *  opposite) is turned into opacity when read. */
  v: 2;
  shape: Shape;
  /** "" = see-through, else "#rrggbb" */
  background: string;
  /** bottom first */
  layers: Layer[];
};

export const FONTS: [string, string][] = [
  ["Counter-Strike", "Counter-Strike"],
  ["Impact, Haettenschweiler, 'Arial Narrow Bold', sans-serif", "Impact"],
  ["'Arial Black', 'Arial Bold', Gadget, sans-serif", "Arial Black"],
  ["Verdana, Geneva, sans-serif", "Verdana"],
  ["'Trebuchet MS', Helvetica, sans-serif", "Trebuchet"],
  ["Georgia, 'Times New Roman', serif", "Georgia"],
  ["'Courier New', Courier, monospace", "Courier"],
  ["'Comic Sans MS', 'Comic Sans', 'Chalkboard SE', cursive", "Comic"],
];

let seq = 0;
export const newId = () => `l${Date.now().toString(36)}${(seq++).toString(36)}`;

const base = (doc: Doc, name: string): Base => {
  const s = SHAPES[doc.shape];
  return { id: newId(), name, x: s.w / 2, y: s.h / 2, rot: 0, scale: 1, flipX: false, flipY: false, opacity: 1, hidden: false, locked: false };
};

export function textLayer(doc: Doc, text = "Your name", over: Partial<TextLayer> = {}): TextLayer {
  return { ...base(doc, "Words"), kind: "text", text, font: FONTS[1][0], size: 96, bold: false, italic: false, colour: "#ffffff", outline: 8, outlineColour: "#111111", shadow: true, shadowColour: "#000000", ...over };
}

export function shapeLayer(doc: Doc, shape: string, over: Partial<ShapeLayer> = {}): ShapeLayer {
  const st = STICKERS.find((s) => s.id === shape) ?? STICKERS[0];
  const side = Math.min(SHAPES[doc.shape].w, SHAPES[doc.shape].h) * 0.55;
  return { ...base(doc, st.label), kind: "shape", shape: st.id, w: side, h: side * (st.aspect ?? 1), colour: st.colour ?? "#ff7818", outline: 0, outlineColour: "#111111", ...over };
}

/** A picture layer fitted inside the canvas (90%). */
export function imageLayer(doc: Doc, src: string, natW: number, natH: number, name: string): ImageLayer {
  const s = SHAPES[doc.shape];
  const fit = Math.min((s.w * 0.9) / natW, (s.h * 0.9) / natH);
  return { ...base(doc, name.slice(0, 40) || "Picture"), kind: "image", src, w: natW * fit, h: natH * fit, keyOut: false, tint: "#ff7818", tintAmount: 0 };
}

export const blankDoc = (shape: Shape = "square"): Doc => ({ v: 2, shape, background: "", layers: [] });

/** The doc as read from a saved file, with anything missing or odd put right. */
export function cleanDoc(raw: unknown): Doc {
  const d = (raw && typeof raw === "object" ? raw : {}) as Partial<Doc>;
  const shape: Shape = d.shape && d.shape in SHAPES ? d.shape : "square";
  const doc = blankDoc(shape);
  doc.background = typeof d.background === "string" && /^#[0-9a-f]{6}$/i.test(d.background) ? d.background : "";
  const num = (v: unknown, def: number) => (typeof v === "number" && Number.isFinite(v) ? v : def);
  for (const l of Array.isArray(d.layers) ? d.layers.slice(0, 64) : []) {
    if (!l || typeof l !== "object") continue;
    const b = { ...base(doc, "Layer"), ...l, id: newId() } as Layer;
    b.x = num(b.x, SHAPES[shape].w / 2);
    b.y = num(b.y, SHAPES[shape].h / 2);
    b.rot = num(b.rot, 0);
    b.scale = Math.max(0.02, num(b.scale, 1));
    const seeThrough = (l as { seeThrough?: unknown }).seeThrough;
    b.opacity = Math.min(1, Math.max(0, typeof seeThrough === "number" ? 1 - seeThrough : num(b.opacity, 1)));
    delete (b as { seeThrough?: unknown }).seeThrough;
    if (b.kind === "image" && typeof b.src === "string" && b.src.startsWith("data:image/")) doc.layers.push({ ...imageLayer(doc, b.src, 1, 1, b.name), ...b });
    else if (b.kind === "text") doc.layers.push({ ...textLayer(doc), ...b, text: String(b.text ?? ""), font: typeof b.font === "string" && b.font.trim() && b.font.length <= 120 ? b.font : FONTS[1][0] });
    else if (b.kind === "shape") doc.layers.push({ ...shapeLayer(doc, String(b.shape)), ...b });
  }
  return doc;
}

// ---- size ------------------------------------------------------------------------------

/** wadwrite::fit_size: the spray size the game gets for a picture of w×h. */
export function fitSize(w: number, h: number, maxPixels: number): [number, number] {
  const STEP = 16;
  const budget = Math.min(12288, Math.max(STEP * STEP, maxPixels));
  const aspect = Math.log(Math.max(1, w) / Math.max(1, h));
  let best: [number, number, number] = [STEP, STEP, Infinity];
  for (let sw = STEP; sw <= 256; sw += STEP)
    for (let sh = STEP; sh <= 256; sh += STEP) {
      if (sw * sh > budget) continue;
      const cost = Math.abs(Math.log(sw / sh) - aspect) + 0.15 * (1 - (sw * sh) / budget);
      if (cost < best[2] - 1e-9) best = [sw, sh, cost];
    }
  return [best[0], best[1]];
}

export const SIZE_PIXELS = { small: 4096, medium: 8192, large: 12288 } as const;

// ---- drawing ---------------------------------------------------------------------------

const images = new Map<string, HTMLImageElement>();
const keyed = new Map<string, HTMLCanvasElement>();

/** The picture for a data URL (cached); `onLoad` asks for another draw once it's in. */
export function imageFor(src: string, onLoad?: () => void): HTMLImageElement | null {
  let img = images.get(src);
  if (!img) {
    img = new Image();
    img.onload = () => onLoad?.();
    img.src = src;
    images.set(src, img);
    if (images.size > 64) images.delete(images.keys().next().value!);
  }
  return img.complete && img.naturalWidth > 0 ? img : null;
}

function keyedImage(img: HTMLImageElement, src: string): HTMLCanvasElement {
  let c = keyed.get(src);
  if (c) return c;
  c = document.createElement("canvas");
  c.width = img.naturalWidth;
  c.height = img.naturalHeight;
  const g = c.getContext("2d", { willReadFrequently: true })!;
  g.drawImage(img, 0, 0);
  const data = g.getImageData(0, 0, c.width, c.height);
  const p = data.data;
  const [kr, kg, kb] = [p[0], p[1], p[2]];
  for (let i = 0; i < p.length; i += 4) {
    const d = Math.max(Math.abs(p[i] - kr), Math.abs(p[i + 1] - kg), Math.abs(p[i + 2] - kb));
    if (d <= 40) p[i + 3] = 0;
    else if (d <= 60) p[i + 3] = Math.min(p[i + 3], Math.round(((d - 40) / 20) * 255));
  }
  g.putImageData(data, 0, 0);
  keyed.set(src, c);
  if (keyed.size > 16) keyed.delete(keyed.keys().next().value!);
  return c;
}

let scratch: HTMLCanvasElement | null = null;
const measure = () => (scratch ??= document.createElement("canvas")).getContext("2d")!;

/** A font name as CSS: a built-in stack as it is, an installed family quoted with Impact after it
 *  (what it draws in on a computer without that family). */
export const fontCss = (font: string) => (font.includes(",") || font.includes("'") ? font : `"${font.replace(/["\\]/g, "")}", Impact, Haettenschweiler, sans-serif`);
export const fontOf = (l: TextLayer) => `${l.italic ? "italic " : ""}${l.bold ? "bold " : ""}${l.size}px ${fontCss(l.font)}`;

/** A layer's size at scale 1 (design units), before turning. */
export function layerSize(l: Layer): { w: number; h: number } {
  if (l.kind === "text") {
    const g = measure();
    g.font = fontOf(l);
    const lines = (l.text || " ").split("\n");
    const w = Math.max(...lines.map((t) => g.measureText(t || " ").width)) + l.outline * 2 + 8;
    return { w: Math.max(8, w), h: lines.length * l.size * 1.12 + l.outline * 2 };
  }
  return { w: l.w, h: l.h };
}

/** A point in design units, in the layer's own unturned frame (its middle at 0,0). */
export function toLocal(l: Layer, px: number, py: number) {
  const a = (-l.rot * Math.PI) / 180;
  const dx = px - l.x;
  const dy = py - l.y;
  const rx = dx * Math.cos(a) - dy * Math.sin(a);
  const ry = dx * Math.sin(a) + dy * Math.cos(a);
  return { x: rx / l.scale, y: ry / l.scale };
}

/** The topmost layer under a point that can be picked (shown, not locked). */
export function hit(doc: Doc, px: number, py: number): Layer | null {
  for (let i = doc.layers.length - 1; i >= 0; i--) {
    const l = doc.layers[i];
    if (l.hidden || l.locked) continue;
    const p = toLocal(l, px, py);
    const { w, h } = layerSize(l);
    if (Math.abs(p.x) <= w / 2 && Math.abs(p.y) <= h / 2) return l;
  }
  return null;
}

/** the scale `render` draws at (shadows aren't turned or scaled by the canvas transform) */
let px = 1;

function drawLayer(g: CanvasRenderingContext2D, l: Layer, redraw?: () => void) {
  g.save();
  g.translate(l.x, l.y);
  g.rotate((l.rot * Math.PI) / 180);
  g.scale(l.scale * (l.flipX ? -1 : 1), l.scale * (l.flipY ? -1 : 1));
  g.globalAlpha = l.opacity;
  if (l.kind === "image") {
    const img = imageFor(l.src, redraw);
    if (img) {
      const pic: CanvasImageSource = l.keyOut ? keyedImage(img, l.src) : img;
      if (l.tintAmount > 0) {
        const c = document.createElement("canvas");
        c.width = Math.max(1, Math.round(l.w * l.scale * 2));
        c.height = Math.max(1, Math.round(l.h * l.scale * 2));
        const t = c.getContext("2d")!;
        t.drawImage(pic, 0, 0, c.width, c.height);
        t.globalCompositeOperation = "source-atop";
        t.globalAlpha = l.tintAmount;
        t.fillStyle = l.tint;
        t.fillRect(0, 0, c.width, c.height);
        g.drawImage(c, -l.w / 2, -l.h / 2, l.w, l.h);
      } else g.drawImage(pic, -l.w / 2, -l.h / 2, l.w, l.h);
    }
  } else if (l.kind === "text") {
    g.font = fontOf(l);
    g.textAlign = "center";
    g.textBaseline = "middle";
    g.lineJoin = "round";
    const lines = (l.text || "").split("\n");
    const lh = l.size * 1.12;
    const top = -((lines.length - 1) * lh) / 2;
    lines.forEach((t, i) => {
      const y = top + i * lh;
      if (l.shadow) {
        g.save();
        g.shadowColor = l.shadowColour;
        g.shadowBlur = l.size * 0.1 * l.scale * px;
        g.shadowOffsetX = l.size * 0.05 * l.scale * px;
        g.shadowOffsetY = l.size * 0.07 * l.scale * px;
        g.fillStyle = l.outline > 0 ? l.outlineColour : l.colour;
        if (l.outline > 0) {
          g.strokeStyle = l.outlineColour;
          g.lineWidth = l.outline * 2;
          g.strokeText(t, 0, y);
        } else g.fillText(t, 0, y);
        g.restore();
      }
      if (l.outline > 0) {
        g.strokeStyle = l.outlineColour;
        g.lineWidth = l.outline * 2;
        g.strokeText(t, 0, y);
      }
      g.fillStyle = l.colour;
      g.fillText(t, 0, y);
    });
  } else {
    const st = STICKERS.find((s) => s.id === l.shape) ?? STICKERS[0];
    g.scale(l.w / 100, l.h / 100);
    const path = st.path();
    if (l.outline > 0) {
      g.strokeStyle = l.outlineColour;
      g.lineJoin = "round";
      g.lineWidth = (l.outline * 2 * 100) / Math.max(1, Math.min(l.w, l.h));
      g.stroke(path);
    }
    if (st.stroke) {
      g.strokeStyle = l.colour;
      g.lineWidth = st.stroke;
      g.lineCap = "round";
      g.stroke(path);
    } else {
      g.fillStyle = l.colour;
      g.fill(path, st.fill ?? "evenodd");
    }
  }
  g.restore();
}

/** Draws the design on a canvas `scale` times its design size. */
export function render(g: CanvasRenderingContext2D, doc: Doc, scale = 1, redraw?: () => void) {
  const s = SHAPES[doc.shape];
  px = scale;
  g.save();
  g.setTransform(scale, 0, 0, scale, 0, 0);
  g.clearRect(0, 0, s.w, s.h);
  if (doc.background) {
    g.fillStyle = doc.background;
    g.fillRect(0, 0, s.w, s.h);
  }
  for (const l of doc.layers) if (!l.hidden) drawLayer(g, l, redraw);
  g.restore();
}

/** The design as a PNG data URL, `scale` times its design size. */
export function toPng(doc: Doc, scale = 1): string {
  const s = SHAPES[doc.shape];
  const c = document.createElement("canvas");
  c.width = Math.round(s.w * scale);
  c.height = Math.round(s.h * scale);
  render(c.getContext("2d")!, doc, scale);
  return c.toDataURL("image/png");
}

/** Every picture in the design loaded (so a drawing for the game isn't missing one). */
export function imagesReady(doc: Doc): Promise<void> {
  const waits = doc.layers
    .filter((l): l is ImageLayer => l.kind === "image")
    .map(
      (l) =>
        new Promise<void>((ok) => {
          if (imageFor(l.src)) return ok();
          const img = images.get(l.src)!;
          img.addEventListener("load", () => ok(), { once: true });
          img.addEventListener("error", () => ok(), { once: true });
        }),
    );
  return Promise.all(waits).then(() => undefined);
}

/** Reads a picture file into a data URL no bigger than 1024 on its long side. */
export function readPicture(file: File): Promise<{ src: string; w: number; h: number; name: string }> {
  return new Promise((ok, fail) => {
    if (file.size > 40 << 20) return fail(new Error(`${file.name} is too big. Pick one under 40 MB.`));
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      const k = Math.min(1, 1024 / Math.max(img.naturalWidth, img.naturalHeight));
      const c = document.createElement("canvas");
      c.width = Math.max(1, Math.round(img.naturalWidth * k));
      c.height = Math.max(1, Math.round(img.naturalHeight * k));
      const g = c.getContext("2d")!;
      g.imageSmoothingQuality = "high";
      g.drawImage(img, 0, 0, c.width, c.height);
      URL.revokeObjectURL(url);
      ok({ src: c.toDataURL("image/png"), w: c.width, h: c.height, name: file.name.replace(/\.[a-z0-9]+$/i, "") });
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      fail(new Error(`${file.name} isn't a picture Loner-Strike can read. Use a PNG, JPEG, GIF, BMP or WebP file.`));
    };
    img.src = url;
  });
}
