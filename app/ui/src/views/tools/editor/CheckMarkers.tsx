// Checks: the live checks' findings drawn in the 3D views (agent Checks). Problems and warnings
// get a small dot where they are; heavy views and dark rooms get a big soft ball (hotspots);
// the finding the mouse is on in the Checks panel (or last picked there) gets a ring that shows
// through walls. Mounted inside the scene's GoldSrc root group, next to <LeakLine />.

import { create } from "zustand";
import * as THREE from "three";
import { useEditor } from "./store";
import type { Issue } from "./editorApi";

type MarksState = {
  /** draw the findings in 3D (the panel's "Show in 3D") */
  show: boolean;
  /** the finding to point at (its index in the checks list), from hovering or Look */
  focus: number | null;
  set: (p: Partial<Omit<MarksState, "set">>) => void;
};

const SHOW_KEY = "ls.editor.checkMarks";

export const useCheckMarks = create<MarksState>((set) => ({
  show: (() => {
    try {
      return localStorage.getItem(SHOW_KEY) !== "0";
    } catch {
      return true;
    }
  })(),
  focus: null,
  set: (p) => {
    if (p.show !== undefined) {
      try {
        localStorage.setItem(SHOW_KEY, p.show ? "1" : "0");
      } catch {
        /* per computer only */
      }
    }
    set(p);
  },
}));

/** Findings drawn as hotspots (a ball the size of a room) rather than a dot. */
export const HOTSPOT_CODES = new Set(["heavy-view", "dark-room"]);

const COLOR: Record<Issue["level"], string> = { error: "#ff5a3c", warning: "#ffc23a", info: "#7fb6ff" };

function marked(i: Issue) {
  return !!i.at && (i.level !== "info" || HOTSPOT_CODES.has(i.code));
}

export function CheckMarkers() {
  const checks = useEditor((s) => s.checks);
  const show = useCheckMarks((s) => s.show);
  const focus = useCheckMarks((s) => s.focus);
  if (!checks) return null;
  const f = focus != null ? checks.issues[focus] : undefined;
  return (
    <group>
      {show &&
        checks.issues.map((i, n) =>
          !marked(i) ? null : HOTSPOT_CODES.has(i.code) ? (
            <mesh key={n} position={i.at!} renderOrder={4} raycast={() => null}>
              <sphereGeometry args={[i.code === "heavy-view" ? 96 : 64, 20, 14]} />
              <meshBasicMaterial color={i.code === "heavy-view" ? "#ff7a3a" : "#5a7dff"} transparent opacity={0.22} depthWrite={false} />
            </mesh>
          ) : (
            <mesh key={n} position={i.at!} renderOrder={4} raycast={() => null}>
              <sphereGeometry args={[5, 10, 8]} />
              <meshBasicMaterial color={COLOR[i.level]} />
            </mesh>
          ),
        )}
      {f?.at && (
        <mesh position={f.at} renderOrder={9} raycast={() => null}>
          <torusGeometry args={[HOTSPOT_CODES.has(f.code) ? 110 : 14, HOTSPOT_CODES.has(f.code) ? 4 : 2, 8, 32]} />
          <meshBasicMaterial color={COLOR[f.level]} depthTest={false} transparent opacity={0.95} side={THREE.DoubleSide} />
        </mesh>
      )}
    </group>
  );
}
