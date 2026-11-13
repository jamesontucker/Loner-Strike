// P4's parts of the editor's 3D view: each view announces itself (so drag and drop, the
// cordon's "from the view", thumbnails and the playtest spawn can ask it things), the cordon
// boxes, "Box anywhere in 3D" (the camera turns with the right button), and window-level
// gestures that don't belong to one tool: dropping a prefab card on the map, clicking to
// place a prefab.
// Everything drawn here sits inside the root group, in GoldSrc units.

import { Line } from "@react-three/drei";
import { useThree } from "@react-three/fiber";
import { useEffect, useRef } from "react";
import * as THREE from "three";
import type { Vec3 } from "../viewer/coords";
import { cordonOf, dragging, loadComponents, pickAt, placeComponentAt, placePrefabAt, registerScene, sceneAt, updateGhost, useP4 } from "./extras";
import { useP5 } from "./p5store"; // P5: a texture's drop target
import type { ComponentDef, DocEntity } from "./editorApi";
import { useFgd } from "./fgd";
import { Html } from "@react-three/drei";
import { useState } from "react";
import { useMarquee } from "./Marquee";
import { useEditor } from "./store";

/** The MIME type a prefab card carries while dragged. */
export const PREFAB_MIME = "application/x-ls-prefab";
/** ... and a component card (its class name). */
export const COMPONENT_MIME = "application/x-ls-component";
/** P5: a texture dragged from the texture browser (Content › Textures, or the Textures panel). */
export const TEXTURE_MIME = "application/x-ls-texture";

function boxEdges(min: Vec3, max: Vec3): Vec3[][] {
  const c = (k: number): Vec3 => [k & 1 ? max[0] : min[0], k & 2 ? max[1] : min[1], k & 4 ? max[2] : min[2]];
  const pairs = [[0, 1], [2, 3], [4, 5], [6, 7], [0, 2], [1, 3], [4, 6], [5, 7], [0, 4], [1, 5], [2, 6], [3, 7]];
  return pairs.map(([a, b]) => [c(a), c(b)]);
}

function CordonBoxes() {
  const doc = useEditor((s) => s.doc);
  const c = cordonOf(doc);
  if (!c.boxes.length) return null;
  return (
    <>
      {c.boxes.map((b, i) => (
        <group key={i}>
          {boxEdges(b.min, b.max).map((e, j) => (
            <Line key={j} points={e} color={c.on ? "#ff9b3d" : "#c9a37a"} lineWidth={c.on ? 2 : 1} dashed={!c.on} dashSize={16} gapSize={10} transparent opacity={c.on ? 0.95 : 0.5} />
          ))}
          {c.on && (
            <mesh position={b.min.map((x, k) => (x + b.max[k]) / 2) as Vec3} renderOrder={-2}>
              <boxGeometry args={[b.max[0] - b.min[0], b.max[1] - b.min[1], b.max[2] - b.min[2]]} />
              <meshBasicMaterial color="#ff9b3d" transparent opacity={0.04} depthWrite={false} side={THREE.BackSide} />
            </mesh>
          )}
        </group>
      ))}
    </>
  );
}

/** Mounted inside the root group of every view. */
export function P4SceneParts() {
  const get = useThree((s) => s.get);
  useEffect(() => {
    return registerScene({
      state: get,
      el: () => {
        const st = get();
        const pane = st.events.connected as HTMLElement | undefined;
        return pane && pane instanceof HTMLElement ? pane : st.gl.domElement;
      },
    });
  }, [get]);
  useWindowGestures();
  useBoxAnywhere();
  return (
    <>
      <CordonBoxes />
      <ComponentVisuals />
      <Ghost />
    </>
  );
}

/** "Box anywhere in 3D": the left button draws boxes, so the camera turns with the right one
 *  (Shift+right pans). */
function useBoxAnywhere() {
  const on = useMarquee((s) => s.anywhere3d);
  const controls = useThree((s) => s.controls) as unknown as { mouseButtons?: Record<string, number | null> } | null;
  const camera = useThree((s) => s.camera);
  useEffect(() => {
    const mb = controls?.mouseButtons;
    if (!mb || !(camera instanceof THREE.PerspectiveCamera) || !on) return;
    const was = { ...mb };
    mb.LEFT = -1;
    mb.RIGHT = THREE.MOUSE.ROTATE;
    return () => {
      Object.assign(mb, was);
    };
  }, [on, controls, camera]);
}

// ---- window gestures (installed once, however many views) ------------------------------------

let installs = 0;
let uninstall: (() => void) | null = null;

function useWindowGestures() {
  const once = useRef(false);
  useEffect(() => {
    if (once.current) return;
    once.current = true;
    if (installs++ === 0) uninstall = install();
    return () => {
      once.current = false;
      if (--installs === 0) {
        uninstall?.();
        uninstall = null;
      }
    };
  }, []);
}

function install() {
  // drag a prefab card onto the map
  const over = (e: DragEvent) => {
    const types = e.dataTransfer?.types ?? [];
    // P5: a texture onto a face (Alt: the whole brush), the target outlined while hovering
    if (types.includes(TEXTURE_MIME)) {
      const hit = sceneAt(e.clientX, e.clientY) ? pickAt(e.clientX, e.clientY) : null;
      const target = hit?.kind === "brush" && hit.id != null ? { brush: hit.id, face: e.altKey ? null : hit.face ?? null } : null;
      const now = useP5.getState().dropFace;
      if (now?.brush !== target?.brush || now?.face !== target?.face) useP5.getState().set({ dropFace: target });
      if (target) {
        e.preventDefault();
        if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
      }
      return;
    }
    if (!(types.includes(PREFAB_MIME) || types.includes(COMPONENT_MIME)) || !sceneAt(e.clientX, e.clientY)) {
      if (useP4.getState().ghost) useP4.getState().set({ ghost: null });
      return;
    }
    e.preventDefault();
    updateGhost(e.clientX, e.clientY, dragging.current);
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
  };
  const drop = (e: DragEvent) => {
    useP4.getState().set({ ghost: null });
    // P5: a texture onto a face (Alt: the whole brush)
    const tex = e.dataTransfer?.getData(TEXTURE_MIME);
    if (tex) {
      const target = useP5.getState().dropFace;
      useP5.getState().set({ dropFace: null });
      if (!target) return;
      e.preventDefault();
      const s = useEditor.getState();
      void s.cmd({ op: "set_texture", faces: [[target.brush, e.altKey ? null : target.face]], texture: tex }).then((r) => {
        if (!r.error) s.set({ texture: tex });
      });
      return;
    }
    const id = e.dataTransfer?.getData(PREFAB_MIME);
    const comp = e.dataTransfer?.getData(COMPONENT_MIME);
    if ((!id && !comp) || !sceneAt(e.clientX, e.clientY)) return;
    e.preventDefault();
    if (comp) void placeComponentAt(comp, e.clientX, e.clientY);
    else if (id) void placePrefabAt(id, e.clientX, e.clientY);
  };

  // click to place (the library's Place button; Shift keeps placing)
  const down = (e: PointerEvent) => {
    if (e.button !== 0) return;
    const placing = useP4.getState().placing;
    if (!placing || !sceneAt(e.clientX, e.clientY)) return;
    // only presses on the view itself (not a panel over it)
    const target = e.target as HTMLElement | null;
    if (target && target.tagName !== "CANVAS" && target.dataset?.pane == null) return;
    e.preventDefault();
    e.stopPropagation();
    useP4.getState().set({ placing: e.shiftKey ? placing : null, ghost: e.shiftKey ? useP4.getState().ghost : null });
    void (placing.component ? placeComponentAt(placing.id, e.clientX, e.clientY) : placePrefabAt(placing.id, e.clientX, e.clientY));
  };
  // while placing: the ghost follows the mouse, R turns it a quarter (Shift+R back), Esc stops
  const hover = (e: PointerEvent) => {
    const placing = useP4.getState().placing;
    if (!placing) return;
    if (sceneAt(e.clientX, e.clientY)) updateGhost(e.clientX, e.clientY, placing);
    else if (useP4.getState().ghost) useP4.getState().set({ ghost: null });
  };
  let lastXY = { x: 0, y: 0 };
  const track = (e: PointerEvent) => (lastXY = { x: e.clientX, y: e.clientY });
  const esc = (e: KeyboardEvent) => {
    const p4 = useP4.getState();
    if (!p4.placing) return;
    const t = e.target as HTMLElement | null;
    if (t && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName))) return;
    if (e.key === "Escape") {
      p4.set({ placing: null, ghost: null });
    } else if (e.key.toLowerCase() === "r" && !e.ctrlKey && !e.metaKey && !e.altKey) {
      // (before the editor's own R = Scale)
      e.preventDefault();
      e.stopImmediatePropagation();
      p4.set({ turn: (p4.turn + (e.shiftKey ? 270 : 90)) % 360 });
      updateGhost(lastXY.x, lastXY.y, p4.placing);
    }
  };
  const end = () => {
    dragging.current = null;
    if (useP4.getState().ghost && !useP4.getState().placing) useP4.getState().set({ ghost: null });
  };
  window.addEventListener("dragover", over);
  window.addEventListener("drop", drop);
  window.addEventListener("pointerdown", down, true);
  window.addEventListener("keydown", esc, true);
  window.addEventListener("pointermove", hover);
  window.addEventListener("pointermove", track);
  window.addEventListener("dragend", end);
  return () => {
    window.removeEventListener("dragover", over);
    window.removeEventListener("drop", drop);
    window.removeEventListener("pointerdown", down, true);
    window.removeEventListener("keydown", esc, true);
    window.removeEventListener("pointermove", hover);
    window.removeEventListener("pointermove", track);
    window.removeEventListener("dragend", end);
  };
}

/** A dark see-through skin over brushes outside the cordon. */
export const DIM_MATERIAL = new THREE.MeshBasicMaterial({ color: "#000000", transparent: true, opacity: 0.62, depthWrite: false, polygonOffset: true, polygonOffsetFactor: -1, polygonOffsetUnits: -1 });

// ---- game mode components, drawn per components.json -------------------------------------------

const hex = (c: [number, number, number]) => `rgb(${c[0]}, ${c[1]}, ${c[2]})`;
const keyOfE = (e: DocEntity, k?: string) => (k ? e.keys.find(([a]) => a === k)?.[1] : undefined);
const vecOf = (v: string | undefined): Vec3 | null => {
  const n = v?.trim().split(/\s+/).map(Number);
  return n && n.length >= 3 && n.slice(0, 3).every(Number.isFinite) ? [n[0], n[1], n[2]] : null;
};

function circle(r: number, z: number, o: Vec3): Vec3[] {
  return Array.from({ length: 49 }, (_, i) => {
    const a = (i / 48) * Math.PI * 2;
    return [o[0] + Math.cos(a) * r, o[1] + Math.sin(a) * r, o[2] + z] as Vec3;
  });
}

function ComponentVisuals() {
  const [lib, setLib] = useState<Map<string, ComponentDef> | null>(null);
  useEffect(() => {
    let live = true;
    void loadComponents().then((l) => live && setLib(new Map(l.components.map((c) => [c.class, c]))));
    return () => {
      live = false;
    };
  }, []);
  const ents = useEditor((s) => s.doc?.entities);
  const fgd = useFgd();
  if (!lib || !ents) return null;
  const named = new Map<string, DocEntity[]>();
  for (const e of ents) {
    const n = keyOfE(e, "targetname");
    if (n) named.set(n, [...(named.get(n) ?? []), e]);
  }
  const fgdDefault = (cls: string, k?: string) => (k ? fgd?.get(cls)?.keys.find((x) => x.name === k)?.default : undefined);
  const out: JSX.Element[] = [];
  for (const e of ents) {
    const def = lib.get(e.classname);
    if (!def || e.brushes.length) continue;
    const o = vecOf(keyOfE(e, "origin"));
    if (!o) continue;
    const col = hex(def.colour);
    const size = vecOf(keyOfE(e, def.size_key) || fgdDefault(def.class, def.size_key));
    const sizeSet = !!keyOfE(e, def.size_key) && size && size.every((x) => x > 0);
    if (def.visual === "ring" && !sizeSet) {
      const r = Math.max(8, Math.min(16384, Number(keyOfE(e, def.radius_key) || fgdDefault(def.class, def.radius_key) || 128) || 128));
      out.push(
        <group key={`c${e.id}`}>
          <mesh position={[o[0], o[1], o[2] + 32]} rotation={[Math.PI / 2, 0, 0]} raycast={() => null} renderOrder={-1}>
            <cylinderGeometry args={[r, r, 64, 48, 1, true]} />
            <meshBasicMaterial color={col} transparent opacity={0.12} depthWrite={false} side={THREE.DoubleSide} />
          </mesh>
          <Line points={circle(r, 0, o)} color={col} lineWidth={2} transparent opacity={0.9} />
          <Line points={circle(r, 64, o)} color={col} lineWidth={1} transparent opacity={0.5} />
        </group>,
      );
    } else if ((def.visual === "ring" || def.visual === "box") && sizeSet && size) {
      out.push(
        <mesh key={`c${e.id}`} position={[o[0], o[1], o[2] + size[2] / 2]} raycast={() => null} renderOrder={-1}>
          <boxGeometry args={size} />
          <meshBasicMaterial color={col} transparent opacity={0.14} depthWrite={false} />
        </mesh>,
      );
    } else if (def.visual === "beam_up") {
      out.push(<Line key={`c${e.id}`} points={[o, [o[0], o[1], o[2] + 2048]]} color={col} lineWidth={2} dashed dashSize={32} gapSize={16} />);
    } else if (def.visual === "point") {
      out.push(
        <mesh key={`c${e.id}`} position={[o[0], o[1], o[2] + 28]} raycast={() => null}>
          <octahedronGeometry args={[7]} />
          <meshBasicMaterial color={col} />
        </mesh>,
      );
    }
    for (const k of def.link_keys ?? []) {
      const n = keyOfE(e, k);
      for (const t of (n && named.get(n)) || []) {
        const to = vecOf(keyOfE(t, "origin"));
        if (to) out.push(<Line key={`l${e.id}-${k}-${t.id}`} points={[o, to]} color={col} lineWidth={1.5} dashed dashSize={10} gapSize={6} />);
      }
    }
    const badge = keyOfE(e, def.badge_key);
    if (badge) {
      out.push(
        <Html key={`b${e.id}`} position={[o[0], o[1], o[2] + 56]} center zIndexRange={[5, 0]} style={{ pointerEvents: "none" }}>
          <span className="p4-badge" style={{ borderColor: col }}>{badge}</span>
        </Html>,
      );
    }
  }
  return <>{out}</>;
}

/** Where a dragged or placed prefab/component would land: a see-through box with its outline
 *  and an arrow for which way its front faces. */
function Ghost() {
  const g = useP4((s) => s.ghost);
  if (!g) return null;
  const mid = g.min.map((x, k) => (x + g.max[k]) / 2) as Vec3;
  const size = g.max.map((x, k) => Math.max(1, x - g.min[k])) as Vec3;
  const r = (g.yaw * Math.PI) / 180;
  const front: Vec3 = [-Math.sin(r), Math.cos(r), 0];
  const reach = Math.max(size[0], size[1]) / 2 + 16;
  return (
    <group>
      <mesh position={mid} raycast={() => null} renderOrder={8}>
        <boxGeometry args={size} />
        <meshBasicMaterial color="#7ee08a" transparent opacity={0.18} depthWrite={false} />
      </mesh>
      {boxEdges(g.min, g.max).map((e, j) => (
        <Line key={j} points={e} color="#7ee08a" lineWidth={1.5} depthTest={false} renderOrder={9} />
      ))}
      <Line points={[[mid[0], mid[1], g.min[2] + 2], [mid[0] + front[0] * reach, mid[1] + front[1] * reach, g.min[2] + 2]]} color="#7ee08a" lineWidth={3} depthTest={false} renderOrder={9} />
    </group>
  );
}
