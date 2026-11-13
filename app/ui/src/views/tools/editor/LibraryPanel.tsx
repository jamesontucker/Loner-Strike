// The editor's Library tab: prefabs and game mode components, side by side.

import { useEffect, useState } from "react";
import { ComponentPanel } from "./ComponentPanel";
import { PrefabPanel } from "./PrefabPanel";
import { useP4 } from "./extras";
import "./dialog.css";

export function LibraryPanel({ start }: { start?: "prefabs" | "components" }) {
  const [part, setPart] = useState<"prefabs" | "components">(start ?? "prefabs");
  useEffect(() => {
    if (start) setPart(start);
  }, [start]);
  const placing = useP4((s) => s.placing);
  return (
    <section className="tools-panel p4-prefabs">
      <div className="seg" role="group" aria-label="Library">
        <button type="button" aria-pressed={part === "prefabs"} onClick={() => setPart("prefabs")}>
          Prefabs
        </button>
        <button type="button" aria-pressed={part === "components"} onClick={() => setPart("components")}>
          Components
        </button>
      </div>
      {placing && (
        <p className="banner p4-placing" role="status">
          Click on the map to place {placing.name}. {placing.component ? "" : "R turns it, "}Shift keeps placing, Esc stops.
        </p>
      )}
      {part === "prefabs" ? <PrefabPanel /> : <ComponentPanel />}
    </section>
  );
}
