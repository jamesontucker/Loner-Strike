// P5: picking a brush where the mouse ray runs exactly along one of its edges.
// three.js tests each triangle on its own, and a ray exactly on the line between two faces can
// slip between them (Chromium hands over the exact middle pixel, and a box corner can sit right
// on the 3D camera's line: QA's push/pull test did nothing in Chromium). Brushes are convex, so
// when the triangles miss but the ray meets the brush's bounding sphere, the ray is clipped
// against the brush's face planes with a small tolerance instead; the face it enters through is
// the one that's hit.

import * as THREE from "three";
import type { Vec3 } from "../viewer/coords";
import type { BrushGeo } from "./store";

type Plane = { face: number; n: Vec3; d: number; tri: number };

const planesOf = new WeakMap<BrushGeo, Plane[]>();

function planes(b: BrushGeo): Plane[] {
  let ps = planesOf.get(b);
  if (ps) return ps;
  ps = [];
  const centre: Vec3 = [(b.min[0] + b.max[0]) / 2, (b.min[1] + b.max[1]) / 2, (b.min[2] + b.max[2]) / 2];
  for (const [face, pts] of b.polys) {
    if (pts.length < 3) continue;
    // Newell's normal, then made to point out of the brush
    let n: Vec3 = [0, 0, 0];
    const c: Vec3 = [0, 0, 0];
    for (let k = 0; k < pts.length; k++) {
      const p = pts[k];
      const q = pts[(k + 1) % pts.length];
      n = [n[0] + (p[1] - q[1]) * (p[2] + q[2]), n[1] + (p[2] - q[2]) * (p[0] + q[0]), n[2] + (p[0] - q[0]) * (p[1] + q[1])];
      for (let i = 0; i < 3; i++) c[i] += p[i] / pts.length;
    }
    const len = Math.hypot(n[0], n[1], n[2]);
    if (len < 1e-9) continue;
    n = [n[0] / len, n[1] / len, n[2] / len];
    let d = n[0] * c[0] + n[1] * c[1] + n[2] * c[2];
    if (n[0] * centre[0] + n[1] * centre[1] + n[2] * centre[2] > d) {
      n = [-n[0], -n[1], -n[2]];
      d = -d;
    }
    const tri = b.faceOfTri.indexOf(face);
    ps.push({ face, n, d, tri: tri < 0 ? 0 : tri });
  }
  planesOf.set(b, ps);
  return ps;
}

const inv = new THREE.Matrix4();
const local = new THREE.Ray();
const sphere = new THREE.Sphere();

type Raycast = (this: THREE.Mesh, raycaster: THREE.Raycaster, out: THREE.Intersection[]) => void;
const fns = new WeakMap<BrushGeo, Raycast>();

/** A brush mesh's raycast: the usual triangle test, and the convex fallback when it misses (see
 *  the top of the file). One function per brush, so it's safe to call while rendering. */
export function brushRaycast(b: BrushGeo): Raycast {
  let f = fns.get(b);
  if (!f) fns.set(b, (f = makeRaycast(b)));
  return f;
}

function makeRaycast(b: BrushGeo): Raycast {
  return function (this: THREE.Mesh, raycaster: THREE.Raycaster, out: THREE.Intersection[]) {
    const before = out.length;
    THREE.Mesh.prototype.raycast.call(this, raycaster, out);
    if (out.length > before) return;
    const g = this.geometry;
    if (!g.boundingSphere) g.computeBoundingSphere();
    sphere.copy(g.boundingSphere!).applyMatrix4(this.matrixWorld);
    if (!raycaster.ray.intersectsSphere(sphere)) return;
    inv.copy(this.matrixWorld).invert();
    local.copy(raycaster.ray).applyMatrix4(inv);
    const o = local.origin;
    const dir = local.direction;
    const tol = 0.01;
    let tIn = 0;
    let tOut = Infinity;
    let enter: Plane | null = null;
    for (const p of planes(b)) {
      const denom = p.n[0] * dir.x + p.n[1] * dir.y + p.n[2] * dir.z;
      const num = p.d + tol - (p.n[0] * o.x + p.n[1] * o.y + p.n[2] * o.z);
      if (Math.abs(denom) < 1e-12) {
        if (num < 0) return; // parallel and outside
        continue;
      }
      const t = num / denom;
      if (denom < 0) {
        if (t > tIn || !enter) {
          if (t > tIn) tIn = t;
          enter = p;
        }
      } else if (t < tOut) tOut = t;
      if (tIn > tOut) return;
    }
    if (!enter || tIn <= 0) return; // (from inside a brush, the triangles decide)
    // the point on the face itself (the tolerance only decides whether it's a hit)
    const n = enter.n;
    const t = (enter.d - (n[0] * o.x + n[1] * o.y + n[2] * o.z)) / (n[0] * dir.x + n[1] * dir.y + n[2] * dir.z);
    const point = local.at(Math.max(t, 0), new THREE.Vector3()).applyMatrix4(this.matrixWorld);
    const distance = raycaster.ray.origin.distanceTo(point);
    if (distance < raycaster.near || distance > raycaster.far) return;
    out.push({
      distance,
      point,
      object: this,
      faceIndex: enter.tri,
      face: { a: 0, b: 0, c: 0, normal: new THREE.Vector3(...enter.n), materialIndex: 0 },
    });
  };
}
