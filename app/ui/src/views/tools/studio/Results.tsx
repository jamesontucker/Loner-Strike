// The results under the canvas, like a compiler's output: the problems (click one to go to its
// node) and what the compiler said last. Two checks fill Problems: the quick check (the app's
// own, updated as you edit) and the full check (Compile: the real compiler), each item says
// which found it.

import { useEffect, useMemo, useState } from "react";
import { checkKey, useStudio, type ResultsTab } from "./store";
import { nodeTitle } from "./pins";
import type { Problem, TestRunState } from "./studioApi";
import { cancelTestRun } from "./studioApi";

/** "Full check: ..." for the status line and the toolbar icon's hover text. */
/** A sentence's end: none after an ellipsis ("compiling…", not "compiling….") */
export const withStop = (t: string) => (t.endsWith("…") ? t : `${t}.`);

export function fullText(state: string, stale: boolean): string {
  if (state === "running") return "Full check: compiling…";
  if (state === "none") return "Full check: needs compiling";
  if (stale) return "Full check: needs compiling (edited since the last compile)";
  if (state === "ok") return "Full check: compiled, no problems";
  if (state === "warn") return "Full check: compiled with warnings";
  if (state === "error") return "Full check: the compiler found errors";
  return "Full check: the compiler couldn't run";
}

/** "Tested ✓ 2 min ago" and the like, for the toolbar. */
export function testText(run: TestRunState | null, stale: boolean, now: number): string {
  if (!run) return "Not test-run yet";
  if (run.phase === "running") return "Test run going…";
  if (run.phase === "stopped") return "Test run canceled";
  if (run.phase === "failed") return `Test run couldn't finish: ${run.error ?? "it stopped"}`;
  const v = run.result?.verdict;
  const end = run.steps.length ? run.steps[run.steps.length - 1].at : run.started;
  const mins = Math.max(0, Math.round((now - end) / 60000));
  const ago = mins < 1 ? "just now" : `${mins} min ago`;
  const what = v === "pass" ? "Tested \u2713" : v === "warn" ? "Tested with warnings" : "Test run failed";
  return `${what} ${ago}${stale ? " (edited since)" : ""}`;
}

function TestRunTab({ focus, titleOf }: { focus: (id: string) => void; titleOf: (id: string) => string }) {
  const run = useStudio((s) => s.testRun);
  const [now, setNow] = useState(Date.now());
  const [showNever, setShowNever] = useState(false);
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (run?.phase !== "running") return;
    const t = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(t);
  }, [run?.phase]);
  if (!run) return <p className="st-muted st-test-empty">Test run plays the mode on a hidden server with bots for about a minute and a half (a stretch without the mode first, then the mode, a round restart and a map change) and says what happened. Your game isn't touched.</p>;
  const r = run.result;
  const steps = (
        <ol className="st-test-steps">
        {run.steps.map((s, i) => {
          const last = i === run.steps.length - 1 && run.phase === "running";
          return (
            <li key={i} className={last ? "on" : ""}>
              {last ? <span className="spinner" /> : <span className="st-test-done">{"\u2713"}</span>} {s.label}
              {last ? `… ${Math.floor((now - s.at) / 1000)} s` : s.secs != null ? ` (${s.secs} s)` : ""}
            </li>
          );
        })}
      </ol>
  );
  const took = run.steps.length ? Math.round((run.steps[run.steps.length - 1].at - run.started) / 1000) : 0;
  return (
    <div className="st-test" data-testid="studio-testrun">
      {run.phase === "running" && steps}
      {run.phase === "running" && (
        <button type="button" className="btn small" onClick={() => void cancelTestRun()}>
          Cancel
        </button>
      )}
      {run.phase === "stopped" && <p className="st-muted">Canceled. The hidden server was stopped.</p>}
      {run.phase === "failed" && <p className="st-test-verdict fail">The test run couldn't finish: {run.error}</p>}
      {run.phase === "done" && r && (
        <>
          <p className={`st-test-verdict ${r.verdict}`} data-testid="studio-test-verdict" data-verdict={r.verdict}>
            {r.verdict === "pass" ? "Passed" : r.verdict === "warn" ? "Passed with warnings" : "Failed"}
          </p>
          <ul className="st-test-summary">
            {r.summary.map((l, i) => (
              <li key={i}>{l}</li>
            ))}
          </ul>
          {r.problems.length > 0 && (
            <ul className="st-results-list">
              {r.problems.map((p, i) => (
                <li key={i} className={p.level}>
                  <span className="st-results-level">{p.level === "error" ? "Error" : "Warning"}</span>
                  {p.node ? (
                    <button type="button" className="st-link" onClick={() => focus(p.node!)}>
                      {titleOf(p.node)}
                    </button>
                  ) : (
                    <span className="st-muted">The mode</span>
                  )}{" "}
                  {p.message}
                </li>
              ))}
            </ul>
          )}
          {r.cost && (
            <p className="st-muted">
              Server cost: {r.cost.baseline && r.cost.mode ? `${Math.round(r.cost.mode.fps)} frames a second and ${Math.round(r.cost.mode.cpu)}% CPU with the mode, ${Math.round(r.cost.baseline.fps)} and ${Math.round(r.cost.baseline.cpu)}% without; ` : ""}
              {r.cost.nodeRunsPerSec ?? 0} node runs a second.
            </p>
          )}
          {!!r.neverRan?.length && (
            <p className="st-muted">
              {r.neverRan.length} node{r.neverRan.length > 1 ? "s" : ""} never ran during the test (information, not a problem).{" "}
              <button type="button" className="st-link" onClick={() => setShowNever(!showNever)}>
                {showNever ? "Hide" : "Show them"}
              </button>
              {showNever && (
                <span className="st-test-never">
                  {r.neverRan.map((id) => (
                    <button key={id} type="button" className="st-link" onClick={() => focus(id)}>
                      {titleOf(id)}
                    </button>
                  ))}
                </span>
              )}
            </p>
          )}
          <button
            type="button"
            className="btn small"
            onClick={() => {
              void navigator.clipboard?.writeText(r.log).then(() => (setCopied(true), setTimeout(() => setCopied(false), 1500)));
            }}
          >
            {copied ? "Copied" : "Copy the log"}
          </button>
        </>
      )}
      {run.phase !== "running" && (
        <details>
          <summary className="st-muted">
            The steps ({run.steps.length}, about {took} s)
          </summary>
          {steps}
        </details>
      )}
    </div>
  );
}

export function Results({ focus }: { focus: (id: string) => void }) {
  const graph = useStudio((s) => s.graph);
  const problems = useStudio((s) => s.problems);
  const output = useStudio((s) => s.output);
  const full = useStudio((s) => s.full);
  const ask = useStudio((s) => s.resultsAsk);
  const [tab, setTab] = useState<ResultsTab>("problems");
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
        <button type="button" role="tab" aria-selected={tab === "testrun"} className={tab === "testrun" ? "on" : ""} onClick={() => (setTab("testrun"), setOpen(true))}>
          Test run
        </button>
        <span className="st-grow" />
        <button type="button" className="st-link" onClick={() => setOpen(!open)}>
          {open ? "Hide" : "Show"}
        </button>
      </div>
      {open && tab === "problems" && (
        <ul className="st-results-list">
          <li className="st-muted st-results-status" data-testid="studio-check-status">
            {quick} (updated as you edit). {withStop(fullText(full.state, stale))}
          </li>
          {items.map(({ p, from }, i) => (
            <li key={i} className={`${p.level}${from === "full" && stale ? " stale" : ""}`}>
              <span className="st-results-level">{p.level === "error" ? "Error" : "Warning"}</span>
              <span className="st-results-from" title={from === "quick" ? "Found by the quick check (updated as you edit)" : stale ? "Found by the last full check (Compile), before your latest edits" : "Found by the full check (Compile: the real compiler)"}>
                {from === "quick" ? "Quick check" : "Full check"}
              </span>
              {p.node ? (
                <button type="button" className="st-link" onClick={() => focus(p.node!)}>
                  {(() => {
                    const n = graph.nodes.find((x) => x.id === p.node);
                    return n ? nodeTitle(graph, n) : p.node;
                  })()}
                </button>
              ) : (
                <span className="st-muted">The mode</span>
              )}{" "}
              {p.message}
            </li>
          ))}
        </ul>
      )}
      {open && tab === "testrun" && <TestRunTab focus={focus} titleOf={(id) => {
            const n = graph.nodes.find((x) => x.id === id);
            return n ? nodeTitle(graph, n) : id;
          }} />}
      {open && tab === "output" && <pre className="st-results-out">{output ?? "Nothing compiled yet in this session."}</pre>}
    </section>
  );
}
