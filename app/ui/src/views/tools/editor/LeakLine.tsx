// The leak line from the last compile (the compiler's pointfile), drawn through walls in the
// editor's 3D view. Mount it inside the scene's GoldSrc root group (next to <Links />):
// the points are GoldSrc units, like everything else there.

import { Line } from "./StableLine"; // Parity A: drei's Line without the per-edit shader recompiles
import { useLeak } from "./compileApi";

export function LeakLine() {
  const points = useLeak((s) => s.points);
  const visible = useLeak((s) => s.visible);
  const hole = useLeak((s) => s.info?.hole?.at ?? null);
  if (!points || points.length < 2 || !visible) return null;
  return (
    <>
      {/* a faint copy that respects walls under a bright one that doesn't, so it reads in depth */}
      <Line points={points} color="#ff3b30" lineWidth={4} depthTest={false} transparent opacity={0.55} renderOrder={6} />
      <Line points={points} color="#ff6a4d" lineWidth={2} renderOrder={7} />
      <mesh position={points[0]} renderOrder={8}>
        <sphereGeometry args={[6, 12, 8]} />
        <meshBasicMaterial color="#ffd84a" depthTest={false} />
      </mesh>
      {/* P5: the hole, where the line leaves the world (tools/editor/leak.rs) */}
      {hole && (
        <group position={hole}>
          <mesh renderOrder={8}>
            <sphereGeometry args={[10, 16, 10]} />
            <meshBasicMaterial color="#ff3b30" depthTest={false} transparent opacity={0.85} />
          </mesh>
          <mesh renderOrder={8}>
            <sphereGeometry args={[28, 16, 10]} />
            <meshBasicMaterial color="#ff3b30" depthTest={false} transparent opacity={0.18} wireframe />
          </mesh>
        </group>
      )}
    </>
  );
}
