// P5: mode kit markers drawn in the views: an area's box (size "x y z") or circle (radius) round
// its marker, a path's spots joined in order, each in its kit's colour. (The markers themselves
// are entity boxes in the kit's colour: EditorScene's entityLook.)

import { Line } from "./StableLine"; // Parity A: drei's Line without the per-edit shader recompiles
import { useEffect, useMemo } from "react";
import type { Vec3 } from "../viewer/coords";
import { loadKits, markerOf, useKits } from "./kits";
import { keyOf, originOf, useEditor } from "./store";

const nums = (v: string | undefined) => (v ?? "").trim().split(/\s+/).map(Number);

export function KitOverlay() {
  const doc = useEditor((s) => s.doc);
  const kits = useKits((k) => k.kits);
  const loaded = useKits((k) => k.loaded);
  const hasMarkers = !!doc?.entities.some((e) => e.classname === "ls_marker");
  useEffect(() => {
    if (hasMarkers && !loaded) void loadKits();
  }, [hasMarkers, loaded]);
  const parts = useMemo(() => {
    const boxes: { at: Vec3; size: Vec3; colour: string; key: number }[] = [];
    const rings: { at: Vec3; r: number; colour: string; key: number }[] = [];
    const paths = new Map<string, { pts: [number, Vec3][]; colour: string }>();
    for (const e of doc?.entities ?? []) {
      if (e.classname !== "ls_marker") continue;
      const o = originOf(e.keys);
      if (!o) continue;
      const found = markerOf(e);
      const colour = found?.marker.draw.colour || "#e8c547";
      const size = nums(keyOf(e.keys, "size"));
      const r = Number(keyOf(e.keys, "radius"));
      if (size.length === 3 && size.every((x) => x > 0 && x <= 16384)) boxes.push({ at: o, size: size as Vec3, colour, key: e.id });
      else if (r > 0 && r <= 16384) rings.push({ at: o, r, colour, key: e.id });
      if (found?.marker.type === "path") {
        const k = `${keyOf(e.keys, "mode") ?? ""}/${found.marker.id}/${keyOf(e.keys, "path") ?? "1"}`;
        const p = paths.get(k) ?? { pts: [], colour };
        p.pts.push([Number(keyOf(e.keys, "order")) || 0, o]);
        paths.set(k, p);
      }
    }
    return { boxes, rings, paths: [...paths.entries()].map(([k, p]) => ({ key: k, colour: p.colour, pts: p.pts.sort((a, b) => a[0] - b[0]).map((x) => x[1]) })).filter((p) => p.pts.length > 1) };
  }, [doc, kits]); // eslint-disable-line react-hooks/exhaustive-deps
  if (!parts.boxes.length && !parts.rings.length && !parts.paths.length) return null;
  return (
    <group>
      {parts.boxes.map((b) => (
        <mesh key={`b${b.key}`} position={b.at} raycast={() => null}>
          <boxGeometry args={b.size} />
          <meshBasicMaterial color={b.colour} wireframe transparent opacity={0.55} depthWrite={false} />
        </mesh>
      ))}
      {parts.rings.map((c) => (
        <mesh key={`r${c.key}`} position={c.at} raycast={() => null}>
          <torusGeometry args={[c.r, Math.max(1, c.r / 96), 6, 64]} />
          <meshBasicMaterial color={c.colour} transparent opacity={0.6} depthWrite={false} />
        </mesh>
      ))}
      {parts.paths.map((p) => (
        <Line key={p.key} points={p.pts} color={p.colour} lineWidth={2.5} dashed dashSize={12} gapSize={6} raycast={() => null} />
      ))}
    </group>
  );
}
