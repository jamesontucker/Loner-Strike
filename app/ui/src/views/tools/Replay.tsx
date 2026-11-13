// The Replay tool: pick a recorded bot match (gm_simrec), then watch it on its map.

import { useEffect } from "react";
import { useActions } from "../../api";
import { useNav } from "../../store";
import { useTools } from "../../stores/tools";
import { BusyButton, LoadError, Skeleton, useToast } from "../../ui";
import { Empty, Loading } from "./common";
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
  const openButton = <BusyButton className="btn" busy={busy("tools:replay:open")} label="Open a recording…" busyLabel="Opening…" onClick={open} />;
  return (
    <div className="stack-tight">
      {list.isLoading ? (
        <Skeleton rows={3} />
      ) : list.error ? (
        <LoadError text={list.error instanceof Error ? list.error.message : "Couldn't list the recordings."} onRetry={() => void list.refetch()} />
      ) : !recs.length ? (
        <Empty
          title="No recordings yet"
          text={
            <>
              Run a map health check to make one: bots play the map and the match is recorded for you to watch here. Recordings from the gm_simrec plugin (a .csv per match) work too
              {list.data?.folders?.[0] ? <>: open one, or put them in <span className="mono">{list.data.folders[0]}</span></> : ""}.
            </>
          }
        >
          <button className="btn primary" onClick={() => useNav.getState().go("tools", "health")}>Run a map health check</button>
          {openButton}
        </Empty>
      ) : (
        <>
          <div className="tl-head">
            <span className="muted small-note">{recs.length} recording{recs.length === 1 ? "" : "s"}, newest first. Click one to watch it.</span>
            <div className="tl-head-actions">{openButton}</div>
          </div>
        <div className="tools-recs">
          {recs.map((r) => (
            <button key={r.id} className="tools-rec" disabled={!!r.error} title={r.error ?? `${r.file}: click to watch it`} onClick={() => openReplay(r.id, r.map)}>
              <span className="tools-rec-map">{r.map || "unknown map"}</span>
              <span className="muted small-note">{when(r.when)}</span>
              {r.error ? (
                <span className="tools-warn small-note">{r.error}</span>
              ) : (
                <span className="small-note">
                  {mins(r.seconds)} · {r.bots} bots · {r.kills} kills{r.hasTrace ? " · with bot thoughts" : ""}
                </span>
              )}
            </button>
          ))}
        </div>
        </>
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
  if (rec.isLoading) return <Loading text="Reading the recording…" />;
  if (rec.error || !rec.data)
    return <LoadError title="Couldn't open this recording" text={rec.error instanceof Error ? rec.error.message : "Something went wrong."} onRetry={() => void rec.refetch()} />;
  if (!rec.data.map)
    return (
      <Empty title="Which map?" text="This recording's map isn't in your game, so it can't be shown. Install the map, then open the recording again.">
        <button className="btn" onClick={back}>Back</button>
      </Empty>
    );
  const file = rec.data.id;
  return (
    <Viewer
      map={rec.data.map}
      replay={{
        data: rec.data,
        trace: trace.data ?? null,
        title: rec.data.map,
        note: `${rec.data.players.length} players · ${mins(rec.data.end - rec.data.start)}${rec.data.hasTrace ? " · with bot thoughts" : ""}`,
        onBack: back,
      }}
      key={`${file}:${rec.data.map}`}
    />
  );
}
