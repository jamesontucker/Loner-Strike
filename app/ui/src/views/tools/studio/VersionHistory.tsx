// Version history: the mode's last saves (the app keeps 50, as differences: history.rs). Picking
// one marks the canvas against it (nodes added since in green, changed in amber; ones it had that
// are gone now are listed), and Restore brings it back as one undo step after saving what's
// there now (so nothing is lost).

import { useEffect, useState } from "react";
import { get } from "../common/api";
import { useStudio } from "./store";
import { parseGraph, type Graph } from "./schema";
import * as api from "./studioApi";
import { compareGraphs, useMarks } from "./versionOps";

export type Version = { n: number; at: number; nodes: number; added: number; removed: number; changed: number };
const when = (ms: number) => {
  const d = new Date(ms);
  const today = new Date().toDateString() === d.toDateString();
  return today ? d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : d.toLocaleString([], { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
};

export function VersionHistory({ onClose, toast }: { onClose: () => void; toast: (m: string, bad?: boolean) => void }) {
  const graph = useStudio((s) => s.graph);
  const saved = useStudio((s) => s.saved);
  const [list, setList] = useState<Version[] | null>(null);
  const [error, setError] = useState("");
  const [pick, setPick] = useState<{ n: number; graph: Graph; gone: { id: string; title: string }[] } | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const id = graph?.mode.id ?? "";
  const load = () =>
    get<{ versions?: Version[]; error?: string }>(`/api/tools/studio/history/${encodeURIComponent(id)}`).then(
      (r) => (r.error ? setError(r.error) : setList(r.versions ?? [])),
      (e: Error) => setError(e.message),
    );
  useEffect(() => {
    void load();
    return () => useMarks.getState().set({});
  }, [id]); // eslint-disable-line react-hooks/exhaustive-deps
  // the marks follow edits made while a version is picked
  useEffect(() => {
    if (pick && graph) useMarks.getState().set(compareGraphs(graph, pick.graph).marks);
  }, [graph, pick]);
  if (!graph) return null;
  const show = async (v: Version) => {
    setBusy(`Reading the version from ${when(v.at)}…`);
    const r = await get<{ graph?: unknown; error?: string }>(`/api/tools/studio/history/${encodeURIComponent(id)}/${v.n}`).catch((e: Error) => ({ error: e.message }) as { graph?: unknown; error?: string });
    setBusy(null);
    if (!r.graph) return toast(r.error ?? "Couldn't read that version.", true);
    const p = parseGraph(r.graph);
    if ("error" in p) return toast(p.error, true);
    const c = compareGraphs(graph, p.graph);
    useMarks.getState().set(c.marks);
    setPick({ n: v.n, graph: p.graph, gone: c.gone });
  };
  const restore = async () => {
    if (!pick) return;
    // what's there now is kept first (it becomes the newest version)
    if (graph !== saved) {
      setBusy("Saving what's there now…");
      const r = await api.saveGraph(graph).catch((e: Error) => ({ error: e.message }));
      if (r && "error" in r && r.error) {
        setBusy(null);
        return toast(r.error, true);
      }
      useStudio.getState().markSaved();
    }
    setBusy(null);
    const old = pick.graph;
    // (the mode keeps its id and uid)
    useStudio.getState().edit(() => ({ ...old, mode: { ...old.mode, id: graph.mode.id, uid: graph.mode.uid } }));
    useMarks.getState().set({});
    setPick(null);
    toast("Restored. Undo brings back what was there; Save keeps it.");
    void load();
  };
  const marks = pick ? Object.values(compareGraphs(graph, pick.graph).marks) : [];
  return (
    <aside className="st-help st-versions" role="complementary" aria-label="Version history" data-testid="studio-versions">
      <div className="st-help-head">
        <strong>Version history</strong>
        <span className="st-grow" />
        <button type="button" className="btn small" onClick={onClose} aria-label="Close version history" title="Close">
          ✕
        </button>
      </div>
      <p className="st-help-pad st-muted">The last 50 saves of this mode. Pick one to see how the graph has changed since; Restore brings it back.</p>
      {busy && (
        <p className="st-help-pad" role="status">
          <span className="spinner" /> {busy}
        </p>
      )}
      {error ? (
        <p className="st-help-pad st-bad-text">{error}</p>
      ) : !list ? (
        <p className="st-help-pad st-muted" role="status">
          <span className="spinner" /> Reading the versions…
        </p>
      ) : list.length === 0 ? (
        <p className="st-help-pad st-muted">No saves yet: each Save adds one.</p>
      ) : (
        <div className="st-help-list" role="listbox" aria-label="Versions">
          {list.map((v, i) => (
            <button key={v.n} type="button" role="option" aria-selected={pick?.n === v.n} className={`st-palette-item${pick?.n === v.n ? " cur" : ""}`} onClick={() => void show(v)}>
              <span className="st-palette-title">
                {when(v.at)}
                {i === 0 ? " (the last save)" : ""}
              </span>
              <span className="st-palette-help">
                {v.nodes} nodes{v.added || v.removed || v.changed ? ` · ${[v.added && `${v.added} added`, v.removed && `${v.removed} removed`, v.changed && `${v.changed} changed`].filter(Boolean).join(", ")}` : ""}
              </span>
            </button>
          ))}
        </div>
      )}
      {pick && (
        <div className="st-help-pad st-versions-pick">
          <p>
            Since then: <span className="st-mark added">{marks.filter((m) => m === "added").length} added</span> · <span className="st-mark changed">{marks.filter((m) => m === "changed").length} changed</span> · {pick.gone.length} removed
            {pick.gone.length > 0 && <span className="st-muted"> ({pick.gone.slice(0, 6).map((g) => g.title).join(", ")}{pick.gone.length > 6 ? "…" : ""})</span>}
          </p>
          <div className="row">
            <button type="button" className="btn small" onClick={() => (useMarks.getState().set({}), setPick(null))}>
              Stop comparing
            </button>
            <button type="button" className="btn small primary" disabled={!!busy} onClick={() => void restore()} data-testid="studio-version-restore">
              Restore this version
            </button>
          </div>
        </div>
      )}
    </aside>
  );
}
