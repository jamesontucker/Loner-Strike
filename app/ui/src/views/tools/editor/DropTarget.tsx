// P5: while a texture is dragged over the views, the face it would go on (or, with Alt, the
// whole brush) is outlined.

import { Line } from "./StableLine"; // Parity A: drei's Line without the per-edit shader recompiles
import { useP5 } from "./p5store";
import { useEditor } from "./store";

export function DropTarget() {
  const t = useP5((s) => s.dropFace);
  const brushes = useEditor((s) => s.brushes);
  if (!t) return null;
  const b = brushes.get(t.brush);
  if (!b) return null;
  const faces = t.face == null ? [...b.polys.entries()] : [[t.face, b.polys.get(t.face)] as const];
  return (
    <group>
      {faces.map(([f, pts]) =>
        pts && pts.length >= 3 ? <Line key={f} points={[...pts, pts[0]]} color="#ffd84a" lineWidth={3} depthTest={false} renderOrder={8} /> : null,
      )}
    </group>
  );
}
