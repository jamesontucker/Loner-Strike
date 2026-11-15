// Parity A: the Top, Front and Side views' grid. One quad and a shader (one draw call however
// big the map): lines every grid step, doubling while they'd be closer than a few pixels (so
// zoomed out it never turns into a grey sheet, and zoomed in you see the real grid size),
// the 64-unit lines a little brighter (Hammer's highlight), 1024-unit lines brighter still,
// and the axes through 0.

import { useEffect, useMemo } from "react";
import * as THREE from "three";
import { ROOT_ROTATION, type Vec3 } from "../viewer/coords";
import type { ViewId } from "./store";
import { flatAxes } from "./FlatHandles";

const VERT = /* glsl */ `
  attribute vec2 coord;
  varying vec2 vC;
  void main() {
    vC = coord;
    gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
  }
`;

const FRAG = /* glsl */ `
  varying vec2 vC;
  uniform float uGrid;
  uniform float uMajor;
  uniform vec3 uMinorCol;
  uniform vec3 uMajorCol;
  uniform vec3 uBigCol;
  uniform vec3 uAxisCol;
  // how strongly a pixel sits on a line every s units (1 on it, fading over one pixel)
  float lineAt(vec2 c, float s, vec2 fw) {
    vec2 g = abs(fract(c / s - 0.5) - 0.5) * s / max(fw, vec2(1e-6));
    return 1.0 - min(min(g.x, g.y), 1.0);
  }
  void main() {
    vec2 fw = fwidth(vC);
    float px = max(fw.x, fw.y);
    float s = uGrid;
    for (int i = 0; i < 24; i++) {
      if (s / px >= 6.0) break;
      s *= 2.0;
    }
    float minor = lineAt(vC, s, fw);
    float major = (uMajor > 0.5 && 64.0 > s && 64.0 / px >= 6.0) ? lineAt(vC, 64.0, fw) : 0.0;
    float big = (1024.0 > s && 1024.0 / px >= 6.0) ? lineAt(vC, 1024.0, fw) : 0.0;
    vec2 ax = abs(vC) / max(fw, vec2(1e-6));
    float axis = 1.0 - min(min(ax.x, ax.y), 1.0);
    vec3 col = uMinorCol;
    float a = minor * 0.7;
    if (major > 0.0) { col = mix(col, uMajorCol, major); a = max(a, major * 0.8); }
    if (big > 0.0) { col = mix(col, uBigCol, big); a = max(a, big * 0.85); }
    if (axis > 0.0) { col = mix(col, uAxisCol, axis); a = max(a, axis * 0.9); }
    if (a < 0.01) discard;
    gl_FragColor = vec4(col, a);
  }
`;

/** Half the size of the quad (GoldSrc units): well past the editor's world limit. */
const HALF = 262144;

export function FlatGrid({ view, grid, major, bounds }: { view: Exclude<ViewId, "3d">; grid: number; major: boolean; bounds: [Vec3, Vec3] }) {
  const { axis, u, v } = flatAxes(view);
  // behind everything in the view (the camera looks down from +z, from -y and from +x), so
  // the map's brushes hide it where they are
  const depth = view === "front" ? bounds[1][axis] + 64 : bounds[0][axis] - 64;
  const geo = useMemo(() => {
    const g = new THREE.BufferGeometry();
    const pos: number[] = [];
    const coord: number[] = [];
    for (const [a, b] of [
      [-HALF, -HALF],
      [HALF, -HALF],
      [HALF, HALF],
      [-HALF, HALF],
    ]) {
      const p: Vec3 = [0, 0, 0];
      p[u] = a;
      p[v] = b;
      p[axis] = depth;
      pos.push(...p);
      coord.push(a, b);
    }
    g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
    g.setAttribute("coord", new THREE.Float32BufferAttribute(coord, 2));
    g.setIndex([0, 1, 2, 0, 2, 3]);
    return g;
  }, [axis, u, v, depth]);
  const mat = useMemo(
    () =>
      new THREE.ShaderMaterial({
        vertexShader: VERT,
        fragmentShader: FRAG,
        transparent: true,
        depthWrite: false,
        side: THREE.DoubleSide,
        uniforms: {
          uGrid: { value: 16 },
          uMajor: { value: 1 },
          uMinorCol: { value: new THREE.Color("#3a4233") },
          uMajorCol: { value: new THREE.Color("#56634a") },
          uBigCol: { value: new THREE.Color("#6f7d5c") },
          uAxisCol: { value: new THREE.Color("#8a9a74") },
        },
      }),
    [],
  );
  mat.uniforms.uGrid.value = Math.max(1, grid);
  mat.uniforms.uMajor.value = major ? 1 : 0;
  useEffect(() => () => geo.dispose(), [geo]);
  useEffect(() => () => mat.dispose(), [mat]);
  return (
    <group rotation={ROOT_ROTATION}>
      <mesh geometry={geo} material={mat} renderOrder={-2} frustumCulled={false} raycast={() => null} />
    </group>
  );
}
