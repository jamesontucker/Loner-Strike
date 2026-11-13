// P5: which way is which. A small X/Y/Z triad in the corner of every view, turned with the
// view's camera, in the engine's own axes (GoldSrc and Hammer: Z up, X east, Y north), whatever
// the camera keys are set to. Red X, green Y, blue Z, as in Hammer, J.A.C.K. and Unreal.
//
// The camera lives in the 3D canvas and the triad is plain HTML over the pane, so a tiny feed
// inside each pane's scene hands the camera's turn to the triad on every frame drawn.

import { useFrame } from "@react-three/fiber";
import { useContext, useEffect, useRef } from "react";
import * as THREE from "three";
import { useP5 } from "./p5store";
import { PaneCtx } from "./panes";

export const AXES = [
  { id: "X", word: "east", colour: "#ff5f56", three: new THREE.Vector3(1, 0, 0) },
  { id: "Y", word: "north", colour: "#5fd35f", three: new THREE.Vector3(0, 0, -1) },
  { id: "Z", word: "up", colour: "#5aa0ff", three: new THREE.Vector3(0, 1, 0) },
] as const;

export const COORDS_NOTE = "GoldSrc and Hammer axes: Z is up, X points east, Y points north. Every number in the editor is the engine's own, whatever key set you use.";

const feeds = new Map<number, (q: THREE.Quaternion) => void>();
const seen = new Map<number, THREE.Quaternion>();

/** Inside a pane's scene: send its camera's turn to that pane's triad. */
export function TriadFeed() {
  const slot = useContext(PaneCtx)?.slot ?? 0;
  const last = useRef(new THREE.Quaternion(2, 0, 0, 0));
  useFrame(({ camera }) => {
    if (last.current.equals(camera.quaternion)) return;
    last.current.copy(camera.quaternion);
    seen.set(slot, last.current.clone());
    feeds.get(slot)?.(camera.quaternion);
  });
  return null;
}

const R = 22;
const v = new THREE.Vector3();
const inv = new THREE.Quaternion();

/** Over a pane: the triad. */
export function AxisTriad({ slot }: { slot: number }) {
  const svg = useRef<SVGSVGElement | null>(null);
  useEffect(() => {
    const draw = (q: THREE.Quaternion) => {
      const el = svg.current;
      if (!el) return;
      inv.copy(q).invert();
      const order = AXES.map((a, i) => {
        v.copy(a.three).applyQuaternion(inv);
        return { i, x: v.x, y: -v.y, z: v.z };
      }).sort((a, b) => a.z - b.z);
      order.forEach((o) => {
        const g = el.querySelector<SVGGElement>(`[data-axis="${o.i}"]`);
        if (!g) return;
        const line = g.querySelector("line")!;
        const text = g.querySelector("text")!;
        line.setAttribute("x2", String(o.x * R));
        line.setAttribute("y2", String(o.y * R));
        text.setAttribute("x", String(o.x * (R + 7)));
        text.setAttribute("y", String(o.y * (R + 7) + 3.5));
        // an axis pointing at you or away is a dot: dim it
        const flat = Math.hypot(o.x, o.y) < 0.2;
        g.style.opacity = flat ? "0.45" : "1";
        el.appendChild(g); // the nearest axis draws last
      });
    };
    feeds.set(slot, draw);
    const q = seen.get(slot);
    if (q) draw(q);
    return () => {
      if (feeds.get(slot) === draw) feeds.delete(slot);
    };
  }, [slot]);
  const open = () => useP5.getState().set({ dialog: "coords" });
  return (
    <button type="button" className="ed-triad" title={`${COORDS_NOTE} Click for more.`} aria-label="Axes: Z up, X east, Y north. Click for more about coordinates." onPointerDown={(e) => e.stopPropagation()} onClick={open}>
      <svg ref={svg} width="72" height="72" viewBox="-36 -36 72 72" aria-hidden="true">
        {AXES.map((a, i) => (
          <g key={a.id} data-axis={i}>
            <line x1="0" y1="0" x2="0" y2="0" stroke={a.colour} strokeWidth="2.2" strokeLinecap="round" />
            <text textAnchor="middle" fontSize="10" fontWeight="700" fill={a.colour}>
              {a.id}
            </text>
          </g>
        ))}
      </svg>
    </button>
  );
}
