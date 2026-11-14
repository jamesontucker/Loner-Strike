// Parity A: the Vertex tool's extra actions in the tool bar: Merge corners (the picked corners
// of a brush become one) and how to split an edge or a face.
import { useState } from "react";
import { mergePickedCorners } from "./extrude";
import { useEditor } from "./store";

const MAC = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

export function VertexActions() {
  const verts = useEditor((s) => s.verts);
  const [busy, setBusy] = useState(false);
  const counts = new Map<number, number>();
  for (const v of verts) counts.set(v.brush, (counts.get(v.brush) ?? 0) + 1);
  const can = [...counts.values()].some((n) => n >= 2);
  return (
    <>
      <button
        type="button"
        disabled={!can || busy}
        title="The picked corners of a brush become one corner, at their middle (dropping a corner on another merges them too)"
        onClick={async () => {
          setBusy(true);
          try {
            await mergePickedCorners();
          } finally {
            setBusy(false);
          }
        }}
      >
        {busy ? (
          <>
            <span className="spinner" aria-hidden="true" /> Merging…
          </>
        ) : (
          "Merge corners"
        )}
      </button>
      <span className="muted">{MAC ? "⌘" : "Ctrl"}-drag an edge or face middle to split it.</span>
    </>
  );
}
