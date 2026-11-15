// Parity A (PerfAudit #1): drei's <Line> (9.117.3, MIT, @react-three/drei core/Line) with one fix.
// drei rebuilds the line's geometry whenever `points` is a new array, and its cleanup for that
// also disposes the line's material, which lives on. When every line drawn with a shader is
// disposed at once (the editor's component markers and links get new point arrays on every
// edit), three drops that shader program and compiles it again on the next frame: ~150 lines
// and a recompile per edit on br_valley. Here the geometry is disposed when it's replaced and
// the material only when the line goes away. Same props as drei's Line.

import type { LineProps } from "@react-three/drei";
import { useThree } from "@react-three/fiber";
import { forwardRef, useEffect, useLayoutEffect, useMemo, useState } from "react";
import { Color, Vector2, Vector3, Vector4 } from "three";
import { Line2, LineGeometry, LineMaterial, LineSegments2, LineSegmentsGeometry } from "three-stdlib";

export const Line = forwardRef<Line2 | LineSegments2, LineProps>(function Line({ points, color = 0xffffff, vertexColors, linewidth, lineWidth, segments, dashed, ...rest }, ref) {
  const size = useThree((s) => s.size);
  const line2 = useMemo(() => (segments ? new LineSegments2() : new Line2()), [segments]);
  const [lineMaterial] = useState(() => new LineMaterial());
  const first = vertexColors?.[0] as unknown as { length?: number } | undefined;
  const itemSize = first?.length === 4 ? 4 : 3;
  const lineGeom = useMemo(() => {
    const geom = segments ? new LineSegmentsGeometry() : new LineGeometry();
    const values = points.map((p) => {
      const isArray = Array.isArray(p);
      return p instanceof Vector3 || p instanceof Vector4 ? [p.x, p.y, p.z] : p instanceof Vector2 ? [p.x, p.y, 0] : isArray && p.length === 3 ? [p[0], p[1], p[2]] : isArray && p.length === 2 ? [p[0], p[1], 0] : p;
    });
    geom.setPositions((values as number[][]).flat());
    if (vertexColors) {
      const c = vertexColors.map((x) => (x instanceof Color ? x.toArray() : x));
      geom.setColors((c as number[][]).flat(), itemSize);
    }
    return geom;
  }, [points, segments, vertexColors, itemSize]);
  useLayoutEffect(() => {
    line2.computeLineDistances();
  }, [points, line2]);
  useLayoutEffect(() => {
    if (dashed) lineMaterial.defines.USE_DASH = "";
    else delete lineMaterial.defines.USE_DASH;
    lineMaterial.needsUpdate = true;
  }, [dashed, lineMaterial]);
  // the fix: a replaced geometry goes; the material stays until the line does
  useEffect(() => () => lineGeom.dispose(), [lineGeom]);
  useEffect(() => () => lineMaterial.dispose(), [lineMaterial]);
  return (
    <primitive object={line2} ref={ref} {...rest}>
      <primitive object={lineGeom} attach="geometry" />
      <primitive
        object={lineMaterial}
        attach="material"
        color={vertexColors ? 0xffffff : color}
        vertexColors={Boolean(vertexColors)}
        resolution={[size.width, size.height]}
        linewidth={linewidth ?? lineWidth ?? 1}
        dashed={dashed}
        transparent={itemSize === 4}
        {...rest}
      />
    </primitive>
  );
});
