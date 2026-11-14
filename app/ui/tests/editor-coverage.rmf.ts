// A small Hammer .rmf (format 2.2) for the importer test: the same layout as the Rust test's
// sample (app/src-tauri/src/editor/rmf.rs, sample_rmf), with textures the game's WADs have.
// Two visgroups, a solid, a group (a solid and a light), a func_door, the world's keys, a path.

class W {
  parts: number[] = [];
  f(x: number) {
    const b = Buffer.alloc(4);
    b.writeFloatLE(x);
    this.parts.push(...b);
    return this;
  }
  i(x: number) {
    const b = Buffer.alloc(4);
    b.writeInt32LE(x);
    this.parts.push(...b);
    return this;
  }
  b(x: ArrayLike<number> | string) {
    if (typeof x === "string") this.parts.push(...Buffer.from(x, "latin1"));
    else this.parts.push(...Array.from(x));
    return this;
  }
  v(x: number[]) {
    return this.f(x[0]).f(x[1]).f(x[2]);
  }
  fixed(s: string, n: number) {
    const b = Buffer.alloc(n);
    b.write(s, "latin1");
    return this.b(b);
  }
  bs(s: string) {
    this.parts.push(s.length + 1);
    return this.b(s).b([0]);
  }
}

function boxFaces(min: number[], max: number[]): [number[], number[][]][] {
  const [x0, y0, z0] = min;
  const [x1, y1, z1] = max;
  return [
    [[0, 0, 1], [[x0, y0, z1], [x0, y1, z1], [x1, y1, z1], [x1, y0, z1]]],
    [[0, 0, -1], [[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0]]],
    [[1, 0, 0], [[x1, y0, z0], [x1, y0, z1], [x1, y1, z1], [x1, y1, z0]]],
    [[-1, 0, 0], [[x0, y0, z0], [x0, y1, z0], [x0, y1, z1], [x0, y0, z1]]],
    [[0, 1, 0], [[x0, y1, z0], [x1, y1, z0], [x1, y1, z1], [x0, y1, z1]]],
    [[0, -1, 0], [[x0, y0, z0], [x0, y0, z1], [x1, y0, z1], [x1, y0, z0]]],
  ];
}

function solid(w: W, min: number[], max: number[], tex: string, vis: number) {
  w.bs("CMapSolid").i(vis).b([200, 100, 50]).i(0);
  const faces = boxFaces(min, max);
  w.i(faces.length);
  for (const [n, verts] of faces) {
    w.fixed(tex, 260);
    const [u, v] = n[2] !== 0 ? [[1, 0, 0], [0, -1, 0]] : n[0] !== 0 ? [[0, 1, 0], [0, 0, -1]] : [[1, 0, 0], [0, 0, -1]];
    w.v(u).f(8).v(v).f(4).f(0).f(1).f(1).b(new Array(16).fill(0));
    w.i(verts.length);
    for (const p of verts) w.v(p);
    w.v(verts[0]).v(verts[1]).v(verts[2]);
  }
}

function kv(w: W, cls: string, flags: number, keys: [string, string][]) {
  w.bs(cls).b([0, 0, 0, 0]).i(flags).i(keys.length);
  for (const [k, v] of keys) w.bs(k).bs(v);
  w.b(new Array(12).fill(0));
}

export function sampleRmf(): Buffer {
  const w = new W();
  w.f(2.2).b("RMF");
  w.i(2);
  w.fixed("Walls", 128).b([1, 2, 3, 0]).i(1).b([1, 0, 0, 0]);
  w.fixed("Lights", 128).b([1, 2, 3, 0]).i(2).b([0, 0, 0, 0]);
  w.bs("CMapWorld").i(0).b([0, 0, 0]);
  w.i(3);
  solid(w, [0, 0, 0], [64, 64, 16], "CRATE01", 1);
  w.bs("CMapGroup").i(0).b([0, 0, 0]).i(2);
  solid(w, [128, 0, 0], [192, 32, 64], "OUT_WALL5", 1);
  w.bs("CMapEntity").i(2).b([0, 0, 0]).i(0);
  kv(w, "light", 0, [["_light", "255 255 128 200"]]);
  w.b([0, 0]).v([16, 16, 48]).b([0, 0, 0, 0]);
  w.bs("CMapEntity").i(0).b([0, 0, 0]).i(1);
  solid(w, [0, 128, 0], [32, 160, 96], "GENERIC015V", 0);
  kv(w, "func_door", 1, [["targetname", "door1"], ["speed", "100"]]);
  w.b([0, 0]).v([0, 0, 0]).b([0, 0, 0, 0]);
  kv(w, "worldspawn", 0, [["wad", "\\half-life\\valve\\halflife.wad;\\cstrike\\cstrike.wad"], ["skyname", "desert"]]);
  w.i(1);
  w.fixed("train", 128).fixed("path_corner", 128).i(1).i(2);
  w.v([0, 0, 128]).i(1).fixed("", 128).i(1).bs("speed").bs("50");
  w.v([256, 0, 128]).i(2).fixed("", 128).i(0);
  w.b("DOCINFO\0").f(0.2).i(0).i(0);
  return Buffer.from(w.parts);
}
