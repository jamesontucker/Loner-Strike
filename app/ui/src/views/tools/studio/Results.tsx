// The results under the canvas, like a compiler's output: the problems (click one to go to its
// node) and what the compiler said last.

import { useState } from "react";
import { useStudio } from "./store";
import { nodeDef } from "./registry";

export function Results({ focus }: { focus: (id: string) => void }) {
  const graph = useStudio((s) => s.graph);
  const problems = useStudio((s) => s.problems);
  const output = useStudio((s) => s.output);
  const [tab, setTab] = useState<"problems" | "output">("problems");
  const [open, setOpen] = useState(true);
  if (!graph) return null;
  const errors = problems.filter((p) => p.level === "error").length;
  return (
    <section className={`st-results${open ? "" : " closed"}`} aria-label="Results">
      <div className="st-results-tabs" role="tablist">
        <button type="button" role="tab" aria-selected={tab === "problems"} className={tab === "problems" ? "on" : ""} onClick={() => (setTab("problems"), setOpen(true))}>
          Problems{" "}
          <span className={`st-count${errors ? " bad" : problems.length ? " warn" : ""}`}>{problems.length}</span>
        </button>
        <button type="button" role="tab" aria-selected={tab === "output"} className={tab === "output" ? "on" : ""} onClick={() => (setTab("output"), setOpen(true))}>
          Compiler output
        </button>
        <span className="st-grow" />
        <button type="button" className="st-link" onClick={() => setOpen(!open)}>
          {open ? "Hide" : "Show"}
        </button>
      </div>
      {open && tab === "problems" && (
        <ul className="st-results-list">
          {problems.length === 0 && <li className="st-muted">No problems: ready to compile.</li>}
          {problems.map((p, i) => (
            <li key={i} className={p.level}>
              <span className="st-results-level">{p.level === "error" ? "Error" : "Warning"}</span>
              {p.node ? (
                <button type="button" className="st-link" onClick={() => focus(p.node!)}>
                  {nodeDef(graph.nodes.find((n) => n.id === p.node)?.type ?? "")?.title ?? p.node}
                </button>
              ) : (
                <span className="st-muted">The mode</span>
              )}{" "}
              {p.message}
            </li>
          ))}
        </ul>
      )}
      {open && tab === "output" && <pre className="st-results-out">{output ?? "Nothing compiled yet in this session."}</pre>}
    </section>
  );
}
