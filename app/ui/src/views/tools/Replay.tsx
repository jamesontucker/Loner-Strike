// The Replay tool: pick a recorded bot match (gm_simrec), then watch it on its map.

import { useEffect } from "react";
import { useActions } from "../../api";
import { useTools } from "../../stores/tools";
import { BusyButton, LoadError, Skeleton, useToast } from "../../ui";
import { Viewer } from "./MapViewer";
import { openRecording, ToolsError, useRecordings, useReplay, useTrace } from "./toolsApi";

export default function ReplayTool() {
  const id = useTools((s) => s.replay);
  return id ? <ReplayOpen id={id} /> : <RecordingList />;
}

function when(v: number | string) {
  const n = typeof v === "number" ? v : Number(v);
  if (!Number.isFinite(n) || n <= 0) return "";
  return new Date(n * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}
const mins = (s: number) => (s >= 60 ? `${Math.round(s / 60)} min` : `${Math.round(s)} s`);

function RecordingList() {
  const list = useRecordings(true);
  const { run, busy } = useActions();
  const toast = useToast();
  const openReplay = useTools((s) => s.openReplay);
  const open = () =>
    run("tools:replay:open", async () => {
      try {
        const r = await openRecording();
        if (!r) return;
        void list.refetch();
        openReplay(r.id, r.recording?.map ?? null);
      } catch (e) {
        toast(e instanceof ToolsError ? e.message : "Couldn't open that file.", true);
      }
    }, "Opening a recording…");
  const recs = list.data?.recordings ?? [];
  return (
    <div className="stack-tight">
      <div className="row">
        <BusyButton className="btn primary" busy={busy("tools:replay:open")} label="Open a recording…" busyLabel="Opening…" onClick={open} />
        <span className="muted small-note">Recordings are made by the gm_simrec plugin while bots play (a .csv file per match).</span>
      </div>
      {list.isLoading ? (
        <Skeleton rows={3} />
      ) : list.error ? (
        <LoadError text={list.error instanceof Error ? list.error.message : "Couldn't list the recordings."} onRetry={() => void list.refetch()} />
      ) : !recs.length ? (
        <div className="panel">
          <h3>No recordings yet</h3>
          <p className="muted measure">Open one with the button above, or put recordings in {list.data?.folders?.[0] ? <span className="mono">{list.data.folders[0]}</span> : "the Tools recordings folder"} and they'll show here.</p>
        </div>
      ) : (
        <div className="tools-recs">
          {recs.map((r) => (
            <button key={r.id} className="tools-rec" disabled={!!r.error} title={r.error ?? r.file} onClick={() => openReplay(r.id, r.map)}>
              <span className="tools-rec-map">{r.map || "unknown map"}</span>
              <span className="muted small">{when(r.when)}</span>
              {r.error ? (
                <span className="tools-warn small">{r.error}</span>
              ) : (
                <span className="small">
                  {mins(r.seconds)} · {r.bots} bots · {r.kills} kills{r.hasTrace ? " · with bot thoughts" : ""}
                </span>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function ReplayOpen({ id }: { id: string }) {
  const rec = useReplay(id);
  const trace = useTrace(id, !!rec.data?.hasTrace);
  const map = useTools((s) => s.map);
  const openReplay = useTools((s) => s.openReplay);
  // the recording names its map: open that one
  useEffect(() => {
    if (rec.data?.map && rec.data.map !== map) useTools.setState({ map: rec.data.map });
  }, [rec.data, map]);
  const back = () => openReplay(null);
  if (rec.isLoading)
    return (
      <div className="panel" role="status">
        <span className="spinner" aria-hidden="true" />
        Reading the recording…
      </div>
    );
  if (rec.error || !rec.data)
    return <LoadError title="Couldn't open this recording" text={rec.error instanceof Error ? rec.error.message : "Something went wrong."} onRetry={() => void rec.refetch()} />;
  if (!rec.data.map)
    return (
      <div className="panel">
        <h3>Which map?</h3>
        <p className="muted measure">This recording's map isn't in your game, so it can't be shown. Install the map, then open the recording again.</p>
        <button className="btn" onClick={back}>Back</button>
      </div>
    );
  const file = rec.data.id;
  return (
    <Viewer
      map={rec.data.map}
      replay={{
        data: rec.data,
        trace: trace.data ?? null,
        title: "Replay",
        onBack: back,
      }}
      key={`${file}:${rec.data.map}`}
    />
  );
}
