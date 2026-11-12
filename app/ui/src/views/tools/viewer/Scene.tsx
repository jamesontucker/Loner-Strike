// The 3D view: a Canvas that draws only when something changes, one root group that turns
// GoldSrc's z-up into three's y-up (every buffer stays in GoldSrc numbers), and the cameras.

import { Canvas, events, type RootState } from "@react-three/fiber";
import { Component, type ReactNode } from "react";
import type { EventManager } from "@react-three/fiber";
import { useTools } from "../../../stores/tools";
import type { MapEntity } from "../toolsApi";
import type { MeshBundle } from "./bundle";
import { Cameras } from "./Cameras";
import { ROOT_ROTATION } from "./coords";
import { Entities } from "./Entities";
import { World, type ModelMesh } from "./World";

/** Whether this system can draw 3D at all (checked before making the Canvas). */
export function canDraw3d(): boolean {
  try {
    const c = document.createElement("canvas");
    const gl = c.getContext("webgl2") ?? c.getContext("webgl");
    const ok = !!gl;
    (gl as WebGL2RenderingContext | null)?.getExtension("WEBGL_lose_context")?.loseContext();
    return ok;
  } catch {
    return false;
  }
}

/** Pointer events: from the mouse, or from the middle of the view while flying with the
 *  mouse grabbed. Measured against the canvas on screen, so the app's text size (a CSS
 *  zoom on the page) doesn't throw picking off. */
function pointerEvents(store: Parameters<typeof events>[0]): EventManager<HTMLElement> {
  const base = events(store);
  return {
    ...base,
    compute(event: Event, state: RootState) {
      const el = state.gl.domElement;
      if (document.pointerLockElement === el) state.pointer.set(0, 0);
      else {
        const r = el.getBoundingClientRect();
        const e = event as PointerEvent;
        state.pointer.set(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
      }
      state.raycaster.setFromCamera(state.pointer, state.camera);
    },
  };
}

/** A Canvas that failed (the 3D context was lost or couldn't start): says so plainly. */
export class SceneBoundary extends Component<{ fallback: ReactNode; children: ReactNode }, { failed: boolean }> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? this.props.fallback : this.props.children;
  }
}

export function Scene({ bundle, meshes, entities, map }: { bundle: MeshBundle; meshes: ModelMesh[]; entities: MapEntity[]; map: string }) {
  return (
    <Canvas
      frameloop="demand"
      dpr={[1, 1.5]}
      linear
      flat
      gl={{ powerPreference: "high-performance", antialias: true }}
      camera={{ fov: 70, near: 2, far: 40000, position: [0, 512, 512] }}
      events={pointerEvents}
      onPointerMissed={(e) => {
        // a click on empty space (not a drag) clears the pick
        if (e.type === "click" && useTools.getState().camera !== "fly") useTools.getState().select(null);
      }}
      onPointerLeave={() => useTools.getState().setHover(null)}
    >
      <color attach="background" args={["#262c21"]} />
      {/* only for the sun-shaded look (lighting off); the map's own light is in its lightmap */}
      <hemisphereLight args={["#ffffff", "#5a5f50", 0.9]} />
      <directionalLight position={[0.4, 1, 0.25]} intensity={1.6} />
      <group rotation={ROOT_ROTATION}>
        <World meshes={meshes} bundle={bundle} map={map} />
        <Entities entities={entities} meshes={meshes} />
      </group>
      <Cameras bounds={bundle.bounds} />
    </Canvas>
  );
}

