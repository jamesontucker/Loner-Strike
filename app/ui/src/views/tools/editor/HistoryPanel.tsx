// P5: the undo history as a list: click a step to go back (or forward) to just after it.

import { useEffect, useRef, useState } from "react";
import { afterJump, getHistory, jumpHistory } from "./p5store";
import { useEditor } from "./store";

export function HistoryPanel() {
  const rev = useEditor((s) => s.doc?.revision);
  const docId = useEditor((s) => s.doc?.docId);
  const [h, setH] = useState<{ undo: string[]; redo: string[] } | null>(null);
  const [busy, setBusy] = useState(false);
  const now = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    let live = true;
    void getHistory().then((r) => live && !r.error && setH({ undo: r.undo, redo: r.redo }));
    return () => {
      live = false;
    };
  }, [rev, docId]);
  useEffect(() => {
    now.current?.scrollIntoView({ block: "nearest" });
  }, [h]);
  const jump = async (steps: number) => {
    if (!steps || busy) return;
    setBusy(true);
    try {
      const r = await jumpHistory(steps);
      if (r.error) return useEditor.getState().notify(r.error, true);
      if (r.changed) await afterJump(r.state, r.changes);
    } finally {
      setBusy(false);
    }
  };
  if (!h) {
    return (
      <section className="tools-panel">
        <p className="small-note muted">
          <span className="spinner" aria-hidden="true" />
          Reading the history…
        </p>
      </section>
    );
  }
  const n = h.undo.length;
  return (
    <section className="tools-panel ed-history" aria-busy={busy}>
      <p className="small-note muted">Click a step to go back to just after it. Steps after it can still be redone until you change something.</p>
      <ol className="ed-history-list">
        <li>
          <button type="button" className={n === 0 ? "now" : undefined} ref={n === 0 ? now : undefined} disabled={busy} onClick={() => void jump(n)}>
            Opened the map
          </button>
        </li>
        {h.undo.map((l, i) => (
          <li key={`u${i}`}>
            <button type="button" className={i === n - 1 ? "now" : undefined} ref={i === n - 1 ? now : undefined} disabled={busy} onClick={() => void jump(n - 1 - i)}>
              {l}
            </button>
          </li>
        ))}
        {h.redo.map((l, i) => (
          <li key={`r${i}`} className="undone">
            <button type="button" disabled={busy} title="Undone: click to redo up to here" onClick={() => void jump(-(i + 1))}>
              {l}
            </button>
          </li>
        ))}
      </ol>
      {n >= 500 && <p className="small-note muted">The oldest steps drop off after 500.</p>}
    </section>
  );
}
