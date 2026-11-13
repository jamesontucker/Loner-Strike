// The editor's Gameplay panel (id "library", its old name): game mode components and mode kits,
// side by side (prefabs are in the Content Browser).

import { useEffect, useState } from "react";
import { ComponentPanel } from "./ComponentPanel";
import { KitPanel } from "./KitPanel"; // P5: mode map kits
import { useP4 } from "./extras";
import "./dialog.css";

// P5: prefabs moved to the Content Browser (Content > Prefabs), so they're in one place with
// models, sprites and sounds; Gameplay keeps components and mode kits.
export function LibraryPanel({ start }: { start?: "components" | "kits" }) {
  const [part, setPart] = useState<"components" | "kits">(start ?? "components");
  useEffect(() => {
    if (start) setPart(start);
  }, [start]);
  const placing = useP4((s) => s.placing);
  return (
    <section className="tools-panel p4-prefabs">
      <div className="seg" role="group" aria-label="Gameplay">
        <button type="button" aria-pressed={part === "components"} onClick={() => setPart("components")}>
          Components
        </button>
        <button type="button" aria-pressed={part === "kits"} onClick={() => setPart("kits")}>
          Mode kits
        </button>
      </div>
      {placing && (
        <p className="banner p4-placing" role="status">
          Click on the map to place {placing.name}. {placing.component ? "" : "R turns it, "}Shift keeps placing, Esc stops.
        </p>
      )}
      {part === "kits" ? <KitPanel /> : <ComponentPanel />}
      <p className="small-note muted">Prefabs are in the Content panel (Content › Prefabs), with models, sprites and sounds.</p>
    </section>
  );
}
