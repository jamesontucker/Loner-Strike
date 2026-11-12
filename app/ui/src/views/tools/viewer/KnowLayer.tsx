// What the bots learned about the map (.lsv, plan M5), drawn on the nav: the picked area's
// cover in 16 directions, what it can see, wallbangs, corners, and the angles bots watch.
// Area tints (cover, how much each area sees) are done by NavLayer.

import { useThree } from "@react-three/fiber";
import { useEffect, useMemo } from "react";
import * as THREE from "three";
import { useTools } from "../../../stores/tools";
import { sees, type Know, type Nav } from "../overlays";

const EYE = 60;

/** Material letters (CHAR_TEX) to colours, for wallbangs. */
const MAT_COLOUR: Record<string, string> = { C: "#b8b8b0", M: "#8fb4d6", D: "#d9b25a", V: "#7fd1c7", G: "#c49be0", T: "#e8c45a", W: "#c9935a", P: "#e0844f", Y: "#9fd18b" };

export function KnowLayer({ nav, know }: { nav: Nav; know: Know }) {
  const mode = useTools((s) => s.know);
  const sel = useTools((s) => s.selection);
  const invalidate = useThree((s) => s.invalidate);
  const area = sel?.kind === "area" ? sel.index : -1;

  const lines = useMemo(() => {
    const pos: number[] = [], col: number[] = [];
    const c = new THREE.Color();
    const add = (a: number[], b: number[], colour: string | THREE.Color, alpha = 1) => {
      pos.push(...a, ...b);
      c.set(colour as THREE.ColorRepresentation).multiplyScalar(alpha);
      col.push(c.r, c.g, c.b, c.r, c.g, c.b);
    };
    const eye = (i: number, h = EYE) => [nav.centres[i][0], nav.centres[i][1], nav.centres[i][2] + h];
    const n = know.count;
    if (mode === "cover" && area >= 0) {
      // open distance per direction (east, then counter-clockwise), crouched and standing
      for (let d = 0; d < 16; d++) {
        const ang = (d * Math.PI * 2) / 16;
        for (const [arr, h, colour] of [[know.coverLow, 34, "#e8c45a"], [know.coverHigh, 60, "#7fd1ff"]] as const) {
          const r = Math.min(1024, arr[area * 16 + d]);
          const o = eye(area, h);
          add(o, [o[0] + Math.cos(ang) * r, o[1] + Math.sin(ang) * r, o[2]], colour);
        }
      }
    } else if (mode === "sight" && area >= 0) {
      // crouched-only sightlines (pair flag 1) in a second colour
      const crouched = new Set<number>();
      for (let p = 0; p < know.pairA.length; p++) {
        if (!(know.pairFlags[p] & 1)) continue;
        if (know.pairA[p] === area) crouched.add(know.pairB[p]);
        if (know.pairB[p] === area) crouched.add(know.pairA[p]);
      }
      for (let j = 0; j < n; j++) if (j !== area && sees(know, area, j)) add(eye(area), eye(j), crouched.has(j) ? "#e8c45a" : "#7fd1ff", 0.8);
    } else if (mode === "bangs") {
      for (let b = 0; b < know.bangA.length; b++) {
        const a = know.bangA[b], z = know.bangB[b];
        if (a >= n || z >= n || (area >= 0 && a !== area && z !== area)) continue;
        add(eye(a, 50), eye(z, 50), MAT_COLOUR[String.fromCharCode(know.bangMat[b])] ?? "#d8ded3", area >= 0 ? 1 : 0.7);
      }
    } else if (mode === "corners") {
      // a short stroke from each corner the way it opens
      for (let i = 0; i < n; i++) {
        if (area >= 0 && i !== area) continue;
        for (let k = 0; k < Math.min(8, know.cornerCount[i]); k++) {
          const o = (i * 8 + k) * 4;
          const [x, y, z, yaw] = [know.corners[o], know.corners[o + 1], know.corners[o + 2], (know.corners[o + 3] * Math.PI) / 180];
          add([x, y, z + 40], [x + Math.cos(yaw) * 40, y + Math.sin(yaw) * 40, z + 40], "#f08a3c");
          add([x, y, z + 4], [x, y, z + 72], "#f08a3c", 0.6);
        }
      }
    } else if (mode === "angles" && area >= 0) {
      let max = 1;
      for (let k = 0; k < Math.min(8, know.angleCount[area]); k++) max = Math.max(max, know.angles[(area * 8 + k) * 5 + 3]);
      for (let k = 0; k < Math.min(8, know.angleCount[area]); k++) {
        const o = (area * 8 + k) * 5;
        const yaw = (know.angles[o] * Math.PI) / 180, pitch = (know.angles[o + 1] * Math.PI) / 180;
        const dist = Math.min(1500, know.angles[o + 2] || 300);
        const from = eye(area);
        const weight = know.angles[o + 3] / max;
        add(from, [from[0] + Math.cos(yaw) * Math.cos(pitch) * dist, from[1] + Math.sin(yaw) * Math.cos(pitch) * dist, from[2] + Math.sin(pitch) * dist], "#ff6f61", 0.35 + 0.65 * weight);
      }
    }
    const g = new THREE.BufferGeometry();
    g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
    g.setAttribute("color", new THREE.Float32BufferAttribute(col, 3));
    return g;
  }, [mode, area, nav, know]);
  useEffect(() => () => lines.dispose(), [lines]);
  useEffect(() => invalidate(), [lines, invalidate]);

  return (
    <lineSegments geometry={lines} renderOrder={6} raycast={() => null}>
      <lineBasicMaterial vertexColors depthTest={false} transparent />
    </lineSegments>
  );
}

/** What the Inspector says about an area's knowledge. */
export function knowFacts(know: Know, i: number): [string, string][] {
  let open = 0;
  for (let d = 0; d < 16; d++) open += Math.min(1024, know.coverHigh[i * 16 + d]);
  return [
    ["Sees", `${know.sees[i]} areas`],
    ["Open around", `${Math.round(open / 16)} units on average`],
    ["Corners", String(know.cornerCount[i])],
    ["Angles watched", String(know.angleCount[i])],
  ];
}
