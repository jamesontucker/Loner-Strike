// The LSMB mesh bundle (plan 3.3), decoded into typed arrays that look straight into the
// downloaded bytes (no copies).
//
// Layout, little-endian:
//   "LSMB"  u32 version  u32 jsonLength  JSON header (UTF-8)  ...sections at 4-byte alignment
// The header lists the sections as { name, offset, length, type, stride }, with offset and
// length in bytes from the start of the file, and the draw table:
//   { model, texture, start, count, flags, entityIndex? }
// start/count are in index units (what geometry.addGroup takes).

export const LSMB_VERSION = 1;

/** Draw flags, as bits (the header may also send them as a list of names). */
export const FLAG = { sky: 1, water: 2, trigger: 4, transparent: 8 } as const;

export type SectionType = "f32" | "u32" | "u16" | "u8";
export type Section = { name: string; offset: number; length: number; type: SectionType; stride?: number };

export type Draw = {
  /** 0 is the world; N is the brush entity model "*N" */
  model: number;
  texture: string;
  start: number;
  count: number;
  flags: number;
  /** the entity (index in /entities) that uses this model, for models above 0 */
  entityIndex?: number;
};

export type MeshBundle = {
  version: number;
  positions: Float32Array;
  normals: Float32Array | null;
  uv: Float32Array | null;
  /** lightmap atlas coordinates; null when the map has no lighting */
  uv2: Float32Array | null;
  indices: Uint32Array;
  /** for each triangle, the BSP face it came from */
  faceOfTriangle: Uint32Array | null;
  draws: Draw[];
  /** the model numbers in the bundle, 0 first */
  models: number[];
  /** [min, max] of every position, GoldSrc units */
  bounds: [[number, number, number], [number, number, number]];
  /** true when triangles wind clockwise seen from the front (checked against the normals),
   *  so the viewer draws their other side instead */
  clockwise: boolean;
  /** the rest of the JSON header, for things added later */
  header: Record<string, unknown>;
};

/** A bundle that can't be read: shown to the player as is. */
export class BundleError extends Error {}
const DAMAGED = "This map's 3D data couldn't be read. The map file may be damaged.";

const SIZE: Record<SectionType, number> = { f32: 4, u32: 4, u16: 2, u8: 1 };

function view(buf: ArrayBuffer, s: Section) {
  const size = SIZE[s.type];
  if (!size || s.offset < 0 || s.length < 0 || s.offset + s.length > buf.byteLength || s.length % size) throw new BundleError(DAMAGED);
  const n = s.length / size;
  // typed arrays need their offset aligned; the writer aligns to 4, so this copy is a fallback
  const aligned = s.offset % size === 0;
  const src = aligned ? buf : buf.slice(s.offset, s.offset + s.length);
  const off = aligned ? s.offset : 0;
  switch (s.type) {
    case "f32": return new Float32Array(src, off, n);
    case "u32": return new Uint32Array(src, off, n);
    case "u16": return new Uint16Array(src, off, n);
    case "u8": return new Uint8Array(src, off, n);
  }
}

function flagsOf(v: unknown): number {
  if (typeof v === "number") return v | 0;
  const names = Array.isArray(v) ? v : typeof v === "string" ? v.split(/[|, ]+/) : [];
  return names.reduce((f: number, n) => f | (FLAG[String(n) as keyof typeof FLAG] ?? 0), 0);
}

export function decodeBundle(buf: ArrayBuffer): MeshBundle {
  if (buf.byteLength < 12) throw new BundleError(DAMAGED);
  const dv = new DataView(buf);
  const magic = String.fromCharCode(dv.getUint8(0), dv.getUint8(1), dv.getUint8(2), dv.getUint8(3));
  if (magic !== "LSMB") throw new BundleError(DAMAGED);
  const version = dv.getUint32(4, true);
  if (version > LSMB_VERSION) throw new BundleError("This map was prepared by a newer Loner-Strike. Update the app to view it.");
  const jsonLen = dv.getUint32(8, true);
  if (12 + jsonLen > buf.byteLength) throw new BundleError(DAMAGED);
  let header: Record<string, unknown>;
  try {
    header = JSON.parse(new TextDecoder().decode(new Uint8Array(buf, 12, jsonLen)));
  } catch {
    throw new BundleError(DAMAGED);
  }
  const sections = Array.isArray(header.sections) ? (header.sections as Section[]) : [];
  const get = (name: string) => {
    const s = sections.find((x) => x.name === name);
    return s ? view(buf, s) : null;
  };
  const positions = get("positions");
  const indices = get("indices");
  if (!(positions instanceof Float32Array) || positions.length % 3) throw new BundleError(DAMAGED);
  // u16 indices are allowed, but three wants one type per geometry: widen them
  const idx = indices instanceof Uint32Array ? indices : indices instanceof Uint16Array ? Uint32Array.from(indices) : null;
  if (!idx || idx.length % 3) throw new BundleError(DAMAGED);
  const verts = positions.length / 3;
  const f32 = (name: string, per: number) => {
    const a = get(name);
    return a instanceof Float32Array && a.length === verts * per ? a : null;
  };
  const tris = idx.length / 3;
  const fot = get("faceOfTriangle");

  const draws: Draw[] = (Array.isArray(header.draws) ? header.draws : []).map((d: Record<string, unknown>) => ({
    model: Number(d.model) || 0,
    texture: String(d.texture ?? ""),
    start: Number(d.start) || 0,
    count: Number(d.count) || 0,
    flags: flagsOf(d.flags),
    entityIndex: typeof d.entityIndex === "number" ? d.entityIndex : undefined,
  }));
  if (draws.some((d) => d.start < 0 || d.count < 0 || d.start + d.count > idx.length)) throw new BundleError(DAMAGED);
  // an index past the vertices would read garbage on the GPU: checked once here
  let maxIndex = 0;
  for (let i = 0; i < idx.length; i++) if (idx[i] > maxIndex) maxIndex = idx[i];
  if (idx.length && maxIndex >= verts) throw new BundleError(DAMAGED);

  const min: [number, number, number] = [Infinity, Infinity, Infinity];
  const max: [number, number, number] = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < positions.length; i += 3) {
    for (let k = 0; k < 3; k++) {
      const v = positions[i + k];
      if (v < min[k]) min[k] = v;
      if (v > max[k]) max[k] = v;
    }
  }
  if (!verts) {
    min.fill(0);
    max.fill(0);
  }

  const normals = f32("normals", 3);
  return {
    version,
    clockwise: normals ? isClockwise(positions, normals, idx) : false,
    positions,
    normals,
    uv: f32("uv", 2),
    uv2: f32("uv2", 2),
    indices: idx,
    faceOfTriangle: fot instanceof Uint32Array && fot.length === tris ? fot : null,
    draws,
    models: [...new Set([0, ...draws.map((d) => d.model)])].sort((a, b) => a - b),
    bounds: [min, max],
    header,
  };
}

/** Whether most triangles (a sample) wind clockwise around their normal. */
function isClockwise(p: Float32Array, n: Float32Array, idx: Uint32Array) {
  let cw = 0, ccw = 0;
  const tris = idx.length / 3;
  const step = Math.max(1, Math.floor(tris / 500));
  for (let t = 0; t < tris; t += step) {
    const a = idx[t * 3] * 3, b = idx[t * 3 + 1] * 3, c = idx[t * 3 + 2] * 3;
    const ux = p[b] - p[a], uy = p[b + 1] - p[a + 1], uz = p[b + 2] - p[a + 2];
    const vx = p[c] - p[a], vy = p[c + 1] - p[a + 1], vz = p[c + 2] - p[a + 2];
    const d = (uy * vz - uz * vy) * n[a] + (uz * vx - ux * vz) * n[a + 1] + (ux * vy - uy * vx) * n[a + 2];
    if (d > 0) ccw++;
    else if (d < 0) cw++;
  }
  return cw > ccw;
}

/** Builds a bundle (the dev mock uses it; it's also the reference for the Rust writer). */
export function encodeBundle(header: Record<string, unknown>, sections: { name: string; data: Float32Array | Uint32Array | Uint16Array | Uint8Array; stride?: number }[]): ArrayBuffer {
  const typeOf = (a: ArrayBufferView): SectionType => (a instanceof Float32Array ? "f32" : a instanceof Uint32Array ? "u32" : a instanceof Uint16Array ? "u16" : "u8");
  const align = (n: number) => (n + 3) & ~3;
  // the JSON names the offsets, which depend on the JSON's length: settle it in two passes
  let table: Section[] = [];
  let json = new Uint8Array();
  for (let pass = 0; pass < 8; pass++) {
    json = new TextEncoder().encode(JSON.stringify({ ...header, sections: table }));
    let at = align(12 + json.length);
    const next = sections.map((s) => {
      const e: Section = { name: s.name, offset: at, length: s.data.byteLength, type: typeOf(s.data), stride: s.stride };
      at = align(at + s.data.byteLength);
      return e;
    });
    const same = JSON.stringify(next) === JSON.stringify(table);
    table = next;
    if (same) break;
  }
  json = new TextEncoder().encode(JSON.stringify({ ...header, sections: table }));
  const end = table.length ? table[table.length - 1].offset + table[table.length - 1].length : align(12 + json.length);
  const out = new ArrayBuffer(align(end));
  const dv = new DataView(out);
  "LSMB".split("").forEach((c, i) => dv.setUint8(i, c.charCodeAt(0)));
  dv.setUint32(4, LSMB_VERSION, true);
  dv.setUint32(8, json.length, true);
  new Uint8Array(out, 12, json.length).set(json);
  sections.forEach((s, i) => new Uint8Array(out, table[i].offset, s.data.byteLength).set(new Uint8Array(s.data.buffer, s.data.byteOffset, s.data.byteLength)));
  return out;
}
