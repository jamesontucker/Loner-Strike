// Development only (not in the release build): put the camera exactly where the game's would
// be and take a picture, to compare the viewer with in-game screenshots (docs/TOOLS-RENDER.md).
//
//   window.__lsViewer.camera(x, y, z, pitch, yaw, fov = 90)   // a player's setpos/setang
//   await window.__lsViewer.shot()                             // PNG data URL of the view
//
// x y z is the player's origin as `setpos` takes it; the eye is 17 units above (standing,
// as in Counter-Strike). fov is the game's (horizontal at 4:3; the game keeps the 4:3 height
// on wider screens), so the view matches the game's at any width.

import { useThree } from "@react-three/fiber";
import { useEffect } from "react";
import * as THREE from "three";
import { useTools } from "../../../stores/tools";
import { dirFromAngles, gsToThree, type Vec3 } from "./coords";

export const EYE_HEIGHT = 17;

/** The game's horizontal fov (at 4:3) as a vertical one. */
export const verticalFov = (fov: number) => (2 * Math.atan(Math.tan((fov * Math.PI) / 360) * 0.75) * 180) / Math.PI;

declare global {
  interface Window {
    __lsViewer?: { camera: (x: number, y: number, z: number, pitch: number, yaw: number, fov?: number) => void; shot: () => Promise<string> };
  }
}

export function DevCamera() {
  const get = useThree((s) => s.get);
  useEffect(() => {
    window.__lsViewer = {
      camera(x, y, z, pitch, yaw, fov = 90) {
        useTools.getState().setCamera?.("fly");
        // the fly camera mounts on the next render; place it after that
        setTimeout(() => {
          const { camera, invalidate } = get();
          if (!(camera instanceof THREE.PerspectiveCamera)) return;
          const eye: Vec3 = [x, y, z + EYE_HEIGHT];
          const d = dirFromAngles(yaw, pitch);
          camera.position.copy(gsToThree(eye));
          camera.up.set(0, 1, 0);
          camera.lookAt(gsToThree([eye[0] + d[0], eye[1] + d[1], eye[2] + d[2]]));
          camera.fov = verticalFov(fov);
          camera.near = 4;
          camera.updateProjectionMatrix();
          invalidate();
        }, 50);
      },
      async shot() {
        const { gl, scene, camera } = get();
        gl.render(scene, camera);
        return gl.domElement.toDataURL("image/png");
      },
    };
    return () => void delete window.__lsViewer;
  }, [get]);
  return null;
}
