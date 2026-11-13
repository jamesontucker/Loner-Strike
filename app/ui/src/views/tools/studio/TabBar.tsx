// The graph tabs above the canvas, as in Blueprints: the Event Graph, then one tab per open
// Function or Macro. Click to switch, × (or middle-click) to close, drag to reorder.

import { useState } from "react";
import { useStudio } from "./store";

export function TabBar() {
  const graph = useStudio((s) => s.graph);
  const tab = useStudio((s) => s.tab);
  const tabs = useStudio((s) => s.tabs);
  const { openTab, closeTab, moveTab } = useStudio.getState();
  const [dragging, setDragging] = useState<string | null>(null);
  if (!graph) return null;
  const nameOf = (id: string) => graph.functions?.find((f) => f.id === id)?.name ?? graph.macros?.find((m) => m.id === id)?.name ?? id;
  const kindOf = (id: string) => (graph.macros?.some((m) => m.id === id) ? "mac" : "fn");
  return (
    <div className="st-tabs" role="tablist" aria-label="Graphs">
      <button type="button" role="tab" aria-selected={tab === ""} className={`st-tab${tab === "" ? " on" : ""}`} onClick={() => openTab("")}>
        <span className="st-mb-dot ev" /> Event Graph
      </button>
      {tabs.map((id) => (
        <div
          key={id}
          role="tab"
          aria-selected={tab === id}
          tabIndex={0}
          className={`st-tab${tab === id ? " on" : ""}${dragging === id ? " dragging" : ""}`}
          draggable
          onDragStart={(e) => {
            setDragging(id);
            e.dataTransfer.effectAllowed = "move";
            e.dataTransfer.setData("text/plain", id);
          }}
          onDragEnd={() => setDragging(null)}
          onDragOver={(e) => {
            if (dragging && dragging !== id) e.preventDefault();
          }}
          onDrop={(e) => {
            e.preventDefault();
            if (dragging && dragging !== id) moveTab(dragging, id);
            setDragging(null);
          }}
          onClick={() => openTab(id)}
          onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && openTab(id)}
          onAuxClick={(e) => e.button === 1 && closeTab(id)}
          title={kindOf(id) === "mac" ? "Macro" : "Function"}
        >
          <span className={`st-mb-dot ${kindOf(id)}`} /> {nameOf(id)}
          <button
            type="button"
            className="st-tab-x"
            aria-label={`Close ${nameOf(id)}`}
            onClick={(e) => {
              e.stopPropagation();
              closeTab(id);
            }}
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
