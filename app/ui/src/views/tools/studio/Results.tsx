// The results under the canvas, like a compiler's output: the problems (click one to go to its
// node) and what the compiler said last. Two checks fill Problems: the quick check (the app's
// own, updated as you edit) and the full check (Compile: the real compiler), each item says
// which found it.

import { useEffect, useMemo, useState } from "react";
import { checkKey, useStudio } from "./store";
import { nodeDef } from "./registry";
import type { Problem } from "./studioApi";

/** "Full check: ..." for the status line and the toolbar icon's hover text. */
export function fullText(state: string, stale: boolean): string {
  if (state === "running") return "Full check: compiling…";
  if (state === "none") return "Full check: needs compiling";
  if (stale) return "Full check: needs compiling (edited since the last compile)";
  if (state === "ok") return "Full check: compiled, no problems";
  if (state === "warn") return "Full check: compiled with warnings";
  if (state === "error") return "Full check: the compiler found errors";
  return "Full check: the compiler couldn't run";
}

export function Results({ focus }: { focus: (id: string) => void }) {
  const graph = useStudio((s) => s.graph);
  const problems = useStudio((s) => s.problems);
  const output = useStudio((s) => s.output);
  const full = useStudio((s) => s.full);
  const ask = useStudio((s) => s.resultsAsk);
  const [tab, setTab] = useState<"problems" | "output">("problems");
  const [open, setOpen] = useState(true);
  useEffect(() => {
    if (ask.n) (setTab(ask.tab), setOpen(true));
  }, [ask]);
  const stale = useMemo(() => !!graph && full.state !== "running" && full.state !== "none" && full.key !== checkKey(graph), [graph, full]);
  if (!graph) return null;
  const items: { p: Problem; from: "quick" | "full" }[] = [...problems.map((p) => ({ p, from: "quick" as const })), ...full.problems.map((p) => ({ p, from: "full" as const }))];
  const errors = items.filter((x) => x.p.level === "error").length;
  const quickErrors = problems.filter((p) => p.level === "error").length;
  const quick = problems.length === 0 ? "Quick check: no problems" : `Quick check: ${quickErrors ? `${quickErrors} error${quickErrors > 1 ? "s" : ""}` : ""}${quickErrors && problems.length > quickErrors ? ", " : ""}${problems.length > quickErrors ? `${problems.length - quickErrors} warning${problems.length - quickErrors > 1 ? "s" : ""}` : ""}`;
  return (
    <section className={`st-results${open ? "" : " closed"}`} aria-label="Results">
      <div className="st-results-tabs" role="tablist">
        <button type="button" role="tab" aria-selected={tab === "problems"} className={tab === "problems" ? "on" : ""} onClick={() => (setTab("problems"), setOpen(true))}>
          Problems{" "}
          <span className={`st-count${errors ? " bad" : items.length ? " warn" : ""}`}>{items.length}</span>
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
          <li className="st-muted st-results-status" data-testid="studio-check-status">
            {quick} (updated as you edit). {fullText(full.state, stale)}.
          </li>
          {items.map(({ p, from }, i) => (
            <li key={i} className={`${p.level}${from === "full" && stale ? " stale" : ""}`}>
              <span className="st-results-level">{p.level === "error" ? "Error" : "Warning"}</span>
              <span className="st-results-from" title={from === "quick" ? "Found by the quick check (updated as you edit)" : stale ? "Found by the last full check (Compile), before your latest edits" : "Found by the full check (Compile: the real compiler)"}>
                {from === "quick" ? "Quick check" : "Full check"}
              </span>
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
