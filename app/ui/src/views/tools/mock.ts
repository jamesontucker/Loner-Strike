// Development only: a small test room that stands in for every map until the Rust routes
// answer. It goes through the real LSMB encoder and decoder, so it also checks them.
// toolsApi.ts loads this file only when import.meta.env.DEV is true.

import { decodeBundle, encodeBundle, FLAG } from "./viewer/bundle";

type V = [number, number, number];
type Face = { model: number; texture: string; flags: number; c: [V, V, V, V] };

const faces: Face[] = [];
const ATLAS = 256;
const CELL = 16;

/** A quad, corners counter-clockwise seen from the side it faces. */
const quad = (model: number, texture: string, c: [V, V, V, V], flags = 0) => faces.push({ model, texture, flags, c });

/** An axis-aligned box, faces pointing out (or in, for the room). */
function box(model: number, texture: string, lo: V, hi: V, flags = 0, inward = false) {
  const [x0, y0, z0] = lo;
  const [x1, y1, z1] = hi;
  const sides: [V, V, V, V][] = [
    [[x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]], // top
    [[x0, y0, z0], [x0, y1, z0], [x1, y1, z0], [x1, y0, z0]], // bottom
    [[x0, y0, z0], [x1, y0, z0], [x1, y0, z1], [x0, y0, z1]], // -y
    [[x1, y1, z0], [x0, y1, z0], [x0, y1, z1], [x1, y1, z1]], // +y
    [[x0, y1, z0], [x0, y0, z0], [x0, y0, z1], [x0, y1, z1]], // -x
    [[x1, y0, z0], [x1, y1, z0], [x1, y1, z1], [x1, y0, z1]], // +x
  ];
  for (const s of sides) quad(model, texture, inward ? ([s[0], s[3], s[2], s[1]] as [V, V, V, V]) : s, flags);
}

// the room: floor, walls, a sky ceiling, a pool, a fence, a crate and a trigger
quad(0, "DUSTFLOOR", [[-512, -512, 0], [512, -512, 0], [512, 512, 0], [-512, 512, 0]]);
quad(0, "SKY", [[-512, -512, 384], [-512, 512, 384], [512, 512, 384], [512, -512, 384]], FLAG.sky);
quad(0, "DUSTWALL", [[-512, -512, 0], [-512, -512, 384], [512, -512, 384], [512, -512, 0]]);
quad(0, "DUSTWALL", [[512, 512, 0], [512, 512, 384], [-512, 512, 384], [-512, 512, 0]]);
quad(0, "DUSTWALL", [[-512, 512, 0], [-512, 512, 384], [-512, -512, 384], [-512, -512, 0]]);
quad(0, "MISSING_TEX", [[512, -512, 0], [512, -512, 384], [512, 512, 384], [512, 512, 0]]);
quad(0, "!WATERBLUE", [[128, -384, 8], [384, -384, 8], [384, -128, 8], [128, -128, 8]], FLAG.water);
quad(0, "{FENCE", [[-100, 300, 0], [100, 300, 0], [100, 300, 96], [-100, 300, 96]], FLAG.transparent);
box(1, "CRATE01", [-232, 118, 0], [-168, 182, 64]);
box(2, "AAATRIGGER", [-64, -364, 0], [64, -236, 128], FLAG.trigger);

function build() {
  const order = faces.map((f, i) => ({ f, i })).sort((a, b) => a.f.model - b.f.model || a.f.texture.localeCompare(b.f.texture));
  const pos: number[] = [], nrm: number[] = [], uv: number[] = [], uv2: number[] = [], idx: number[] = [], fot: number[] = [];
  const draws: Record<string, unknown>[] = [];
  const perRow = ATLAS / CELL;
  order.forEach(({ f, i }, n) => {
    const [a, b, c] = f.c;
    const e1 = [b[0] - a[0], b[1] - a[1], b[2] - a[2]], e2 = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    const nx = e1[1] * e2[2] - e1[2] * e2[1], ny = e1[2] * e2[0] - e1[0] * e2[2], nz = e1[0] * e2[1] - e1[1] * e2[0];
    const len = Math.hypot(nx, ny, nz) || 1;
    const ax = Math.abs(nx) >= Math.abs(ny) && Math.abs(nx) >= Math.abs(nz) ? 0 : Math.abs(ny) >= Math.abs(nz) ? 1 : 2;
    const cx = (n % perRow) * CELL, cy = Math.floor(n / perRow) * CELL;
    const v0 = pos.length / 3;
    f.c.forEach((p, k) => {
      pos.push(...p);
      nrm.push(nx / len, ny / len, nz / len);
      // texture: 64 units per tile, t running down the picture
      const [s, t] = ax === 2 ? [p[0], -p[1]] : ax === 1 ? [p[0], -p[2]] : [p[1], -p[2]];
      uv.push(s / 64, t / 64);
      const lu = k === 1 || k === 2 ? CELL - 1 : 1, lv = k === 2 || k === 3 ? CELL - 1 : 1;
      uv2.push((cx + lu) / ATLAS, (cy + lv) / ATLAS);
    });
    const start = idx.length;
    idx.push(v0, v0 + 1, v0 + 2, v0, v0 + 2, v0 + 3);
    fot.push(i, i);
    const last = draws[draws.length - 1];
    if (last && last.model === f.model && last.texture === f.texture) (last.count as number) += 6;
    else draws.push({ model: f.model, texture: f.texture, start, count: 6, flags: f.flags, ...(f.model ? { entityIndex: f.model === 1 ? 6 : 7 } : {}) });
  });
  return { order, buf: encodeBundle({ draws, map: "test room", mock: true }, [
    { name: "positions", data: new Float32Array(pos), stride: 3 },
    { name: "normals", data: new Float32Array(nrm), stride: 3 },
    { name: "uv", data: new Float32Array(uv), stride: 2 },
    { name: "uv2", data: new Float32Array(uv2), stride: 2 },
    { name: "indices", data: new Uint32Array(idx), stride: 1 },
    { name: "faceOfTriangle", data: new Uint32Array(fot), stride: 1 },
  ]) };
}

let built: ReturnType<typeof build> | null = null;
const room = () => (built ??= build());

export async function mockBundle() {
  // a moment, like the real thing reading a file
  await new Promise((r) => setTimeout(r, 400));
  return decodeBundle(room().buf);
}

export const mockEntities = (): Record<string, unknown>[] => [
  { index: 0, classname: "worldspawn", keys: { classname: "worldspawn", wad: "\\half-life\\cstrike\\cstrike.wad", skyname: "desert", mapversion: "220" } },
  { index: 1, classname: "info_player_start", origin: [-300, -300, 36], keys: { classname: "info_player_start", origin: "-300 -300 36", angles: "0 45 0" } },
  { index: 2, classname: "info_player_start", origin: [-230, -300, 36], keys: { classname: "info_player_start", origin: "-230 -300 36", angles: "0 45 0" } },
  { index: 3, classname: "info_player_deathmatch", origin: [300, 300, 36], keys: { classname: "info_player_deathmatch", origin: "300 300 36", angles: "0 225 0" } },
  { index: 4, classname: "info_player_deathmatch", origin: [230, 300, 36], keys: { classname: "info_player_deathmatch", origin: "230 300 36", angles: "0 225 0" } },
  { index: 5, classname: "light", origin: [0, 0, 300], keys: { classname: "light", origin: "0 0 300", _light: "255 230 200 300" } },
  { index: 6, classname: "func_breakable", model: "*1", keys: { classname: "func_breakable", model: "*1", targetname: "crate1", health: "50", material: "1" } },
  { index: 7, classname: "trigger_multiple", model: "*2", keys: { classname: "trigger_multiple", model: "*2", target: "crate1", wait: "1" } },
  { index: 8, classname: "armoury_entity", origin: [200, -40, 16], keys: { classname: "armoury_entity", origin: "200 -40 16", item: "15", count: "1" } },
];

export const mockInfo = () => ({
  bounds: { min: [-512, -512, 0] as [number, number, number], max: [512, 512, 384] as [number, number, number] },
  textures: [
    { name: "DUSTFLOOR", from: "cstrike.wad" },
    { name: "DUSTWALL", from: "cstrike.wad" },
    { name: "MISSING_TEX", from: "missing" },
  ],
  skyname: "desert",
});

// ---- pictures ------------------------------------------------------------------------------

function canvas(w: number, h: number, draw: (g: CanvasRenderingContext2D) => void) {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  draw(c.getContext("2d")!);
  return c.toDataURL("image/png");
}

const texCache = new Map<string, string | null>();
export async function mockTexture(name: string): Promise<string | null> {
  const key = name.toUpperCase();
  if (texCache.has(key)) return texCache.get(key)!;
  let url: string | null = null;
  if (key === "DUSTFLOOR") url = canvas(64, 64, (g) => {
    g.fillStyle = "#b39a6c"; g.fillRect(0, 0, 64, 64);
    for (let i = 0; i < 220; i++) { g.fillStyle = `rgba(${90 + (i % 5) * 20},${70 + (i % 7) * 10},40,.25)`; g.fillRect((i * 37) % 64, (i * 53) % 64, 2, 2); }
    g.strokeStyle = "rgba(60,45,25,.5)"; g.strokeRect(0.5, 0.5, 63, 63);
  });
  else if (key === "DUSTWALL") url = canvas(64, 64, (g) => {
    g.fillStyle = "#c8b07c"; g.fillRect(0, 0, 64, 64);
    g.strokeStyle = "rgba(80,60,30,.55)";
    for (let y = 0; y < 64; y += 16) { g.beginPath(); g.moveTo(0, y + 0.5); g.lineTo(64, y + 0.5); g.stroke(); for (let x = (y / 16) % 2 ? 16 : 0; x < 64; x += 32) { g.beginPath(); g.moveTo(x + 0.5, y); g.lineTo(x + 0.5, y + 16); g.stroke(); } }
  });
  else if (key === "CRATE01") url = canvas(64, 64, (g) => {
    g.fillStyle = "#8a6438"; g.fillRect(0, 0, 64, 64);
    g.strokeStyle = "#4e3518"; g.lineWidth = 5; g.strokeRect(2.5, 2.5, 59, 59);
    g.beginPath(); g.moveTo(4, 4); g.lineTo(60, 60); g.moveTo(60, 4); g.lineTo(4, 60); g.stroke();
  });
  else if (key === "!WATERBLUE") url = canvas(64, 64, (g) => {
    g.fillStyle = "#2f5f7a"; g.fillRect(0, 0, 64, 64);
    g.strokeStyle = "rgba(200,230,255,.35)";
    for (let y = 4; y < 64; y += 12) { g.beginPath(); for (let x = 0; x <= 64; x += 4) g.lineTo(x, y + Math.sin(x / 6) * 2); g.stroke(); }
  });
  else if (key === "{FENCE") url = canvas(64, 64, (g) => {
    g.clearRect(0, 0, 64, 64);
    g.strokeStyle = "#9aa0a0"; g.lineWidth = 2;
    for (let i = -64; i < 128; i += 12) { g.beginPath(); g.moveTo(i, 0); g.lineTo(i + 64, 64); g.moveTo(i + 64, 0); g.lineTo(i, 64); g.stroke(); }
  });
  else if (key === "AAATRIGGER") url = canvas(64, 64, (g) => {
    g.fillStyle = "#c86a1e"; g.fillRect(0, 0, 64, 64);
    g.fillStyle = "#fff"; g.font = "bold 11px sans-serif"; g.fillText("TRIGGER", 6, 36);
  });
  else if (key === "SKY") url = canvas(8, 8, (g) => { g.fillStyle = "#8aa9c4"; g.fillRect(0, 0, 8, 8); });
  texCache.set(key, url);
  return url;
}

let lightmap: string | null = null;
/** Light from a lamp in the middle of the room, baked into each face's cell. */
export async function mockLightmap(): Promise<string> {
  if (lightmap) return lightmap;
  const { order } = room();
  const perRow = ATLAS / CELL;
  lightmap = canvas(ATLAS, ATLAS, (g) => {
    const img = g.createImageData(ATLAS, ATLAS);
    order.forEach(({ f }, n) => {
      const cx = (n % perRow) * CELL, cy = Math.floor(n / perRow) * CELL;
      const [a, b, , d] = f.c;
      for (let y = 0; y < CELL; y++) for (let x = 0; x < CELL; x++) {
        const u = (x - 1) / (CELL - 2), v = (y - 1) / (CELL - 2);
        const p = [0, 1, 2].map((k) => a[k] + (b[k] - a[k]) * u + (d[k] - a[k]) * v);
        const dist2 = (p[0] - 0) ** 2 + (p[1] - 0) ** 2 + (p[2] - 300) ** 2;
        // half brightness is "normal": the viewer doubles it, like the game's overbright
        const l = Math.min(1, 0.3 + 150000 / dist2) * 0.55;
        const o = ((cy + y) * ATLAS + cx + x) * 4;
        img.data[o] = 255 * l; img.data[o + 1] = 235 * l; img.data[o + 2] = 205 * l; img.data[o + 3] = 255;
      }
    });
    g.putImageData(img, 0, 0);
  });
  return lightmap;
}
