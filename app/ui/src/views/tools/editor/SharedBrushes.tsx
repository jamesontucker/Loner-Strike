// Parity A: a pane's view of the shared brush buffers (brushBuffers.ts): the same GPU geometry in
// every pane, drawn in the pane's own style (textured, flat, wire), the selection overlay, and
// the pane's picking object.

import { useMemo, useRef } from "react";
import type { Draw } from "../viewer/bundle";
import type { MapMaterials } from "../viewer/materials";
import { allTrianglePools, makePicker, outlinePools, useBrushBuffers, type Overlay } from "./brushBuffers";
import { byClass, categoryMaterial, edgeLook, flatMaterial, type RenderMode } from "./renderModes";

const drawOf = (texture: string): Draw => ({ model: 0, texture, start: 0, count: 0, flags: 0 });
const isSky = (t: string) => /^sky$/i.test(t);
const noRay = () => null;

function materialFor(mats: MapMaterials, mode: RenderMode, texture: string) {
  if (mode === "missing") {
    // (asked for so a texture that can't be found shows up in the missing list)
    mats.get(drawOf(texture));
    const gone = [...mats.missing].some((x) => x.toLowerCase() === texture.toLowerCase());
    return categoryMaterial(mode, gone ? "missing" : "ok");
  }
  // (a class style draws each brush on its own; the shared pools and the picked overlay are flat)
  return mode === "flat" || byClass(mode) ? flatMaterial(texture) : mats.get(drawOf(texture));
}

/** Every unpicked, shown brush: one draw per texture and up to three outline draws. */
/** hideSky: no sky faces (and sky brushes are tagged for hitOf); skyGhost: no sky outlines either. */
export function SharedBrushes({ mats, version, mode, hideSky, skyGhost }: { mats: MapMaterials; version: number; mode: RenderMode; hideSky: boolean; skyGhost: boolean }) {
  useBrushBuffers((s) => s.version); // pools added or grown: draw the new geometry objects
  const hide = useRef(hideSky);
  hide.current = hideSky;
  const picker = useMemo(() => makePicker(() => hide.current), []);
  const pools = allTrianglePools().filter((p) => !(hideSky && isSky(p.key)));
  const e = outlinePools();
  const w = edgeLook(mode, false, false);
  const t = edgeLook(mode, false, true);
  // sky brushes with no faces (2D views) are only their outlines: drawn light, as in Wireframe,
  // so they show on the dark background in every mode
  const sk = hideSky ? edgeLook("wire", false, false) : w;
  void version;
  return (
    <>
      {mode !== "wire" && !byClass(mode) && pools.map((p) => <mesh key={`${p.key}:${p.gen}`} geometry={p.geometry} material={materialFor(mats, mode, p.key)} raycast={noRay} frustumCulled={false} />)}
      {e.world && (
        <lineSegments key={`w${e.world.gen}`} geometry={e.world.geometry} renderOrder={1} raycast={noRay} frustumCulled={false}>
          <lineBasicMaterial color={w.color} transparent opacity={w.opacity} depthTest={mode !== "wire"} />
        </lineSegments>
      )}
      {e.tied && (
        <lineSegments key={`t${e.tied.gen}`} geometry={e.tied.geometry} renderOrder={1} raycast={noRay} frustumCulled={false}>
          <lineBasicMaterial color={t.color} transparent opacity={t.opacity} depthTest={mode !== "wire"} />
        </lineSegments>
      )}
      {e.sky && !skyGhost && (
        <lineSegments key={`s${e.sky.gen}`} geometry={e.sky.geometry} renderOrder={1} raycast={noRay} frustumCulled={false}>
          <lineBasicMaterial color={sk.color} transparent opacity={sk.opacity} depthTest={mode !== "wire" && !hideSky} />
        </lineSegments>
      )}
      <primitive object={picker} />
    </>
  );
}

/** The picked brushes, drawn on top in one mesh per texture and one outline draw. */
export function PickedOverlay({ overlay, mats, version, mode }: { overlay: Overlay; mats: MapMaterials; version: number; mode: RenderMode }) {
  const look = edgeLook(mode, true, false);
  void version;
  return (
    <>
      {mode !== "wire" && overlay.meshes.map((m) => <mesh key={m.texture} geometry={m.geometry} material={materialFor(mats, mode, m.texture)} raycast={noRay} frustumCulled={false} />)}
      <lineSegments geometry={overlay.edges} renderOrder={3} raycast={noRay} frustumCulled={false}>
        <lineBasicMaterial color={look.color} transparent opacity={look.opacity} depthTest={false} />
      </lineSegments>
    </>
  );
}
