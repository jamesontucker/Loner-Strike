// The Replay tool: pick a recorded bot match (gm_simrec), then watch it on its map.

import { useEffect } from "react";
import { useActions } from "../../api";
import { useNav } from "../../store";
import { useTools } from "../../stores/tools";
import { BusyButton, Info, LoadError, Skeleton, useToast } from "../../ui";
import { Empty, Loading } from "./common";
import { Viewer } from "./MapViewer";
import { openRecording, ToolsError, useRecordings, useReplay, useTrace } from "./toolsApi";
import { BundleError } from "./viewer/bundle";

export default function ReplayTool() {
  const id = useTools((s) => s.replay);
  return id ? <ReplayOpen id={id} /> : <RecordingList />;
}

function when(v: number | string) {
  const n = typeof v === "number" ? v : Number(v);
  if (!Number.isFinite(n) || n <= 0) return "";
  return new Date(n * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}
/** A length: "45 s", "3 min 20 s" (under 10 minutes), "26 min". */
const mins = (s: number) => {
  const t = Math.max(0, Math.round(s));
  if (t < 60) return `${t} s`;
  if (t < 600 && t % 60) return `${Math.floor(t / 60)} min ${t % 60} s`;
  return `${Math.round(t / 60)} min`;
};
const count = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
/** Who's in it: bots when it has any, else the players (a demo's path is one person). */
const who = (r: { bots: number; players: number }) => (r.bots > 0 ? count(r.bots, "bot", "bots") : count(r.players, "player", "players"));

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
              Replays come from the Map simulator: bots play a map and you watch where they went.
              <Info
                text={
                  <>
                    A recording is a .csv file per match, written by the gm_simrec plugin while the simulator runs. Open one with Open a recording…
                    {list.data?.folders?.[0] ? <>, or put them in <span className="mono">{list.data.folders[0]}</span> and they show up here</> : ""}.
                  </>
                }
              />
            </>
          }
        >
          <button className="btn primary" onClick={() => useNav.getState().go("tools", "health")}>Open the map simulator</button>
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
              {r.source && <span className="small-note tools-rec-source">{r.source}</span>}
              <span className="muted small-note">{when(r.when)}</span>
              {r.error ? (
                <span className="tools-warn small-note">{r.error}</span>
              ) : (
                <span className="small-note">
                  {mins(r.seconds)} · {who(r)}
                  {r.kills > 0 ? ` · ${count(r.kills, "kill", "kills")}` : ""}
                  {r.hasTrace ? " · with bot thoughts" : ""}
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
  if (rec.error || !rec.data) {
    // only a failed connection or a server hiccup is worth trying again; a file that isn't a
    // recording, is too big or damaged stays that way (QA4 L8)
    const e = rec.error;
    const retry = !(e instanceof BundleError) && !(e instanceof ToolsError && ((e.status >= 400 && e.status < 500) || (e.json && e.status < 500)));
    return (
      <LoadError
        title="Couldn't open this recording"
        text={e instanceof Error ? e.message : "Something went wrong."}
        action={{ label: "Back to recordings", run: back }}
        onRetry={retry ? () => void rec.refetch() : undefined}
      />
    );
  }
  if (!rec.data.map || !rec.data.mapInstalled)
    return (
      <Empty title="That map isn't on this computer" text={`This recording is of ${rec.data.map || "a map your game doesn't have"}. Get the map, then open the recording again.`}>
        <button className="btn primary" onClick={() => useNav.getState().go("maps")}>Get the map</button>
        <button className="btn" onClick={back}>Back to recordings</button>
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
        note: `${count(rec.data.players.length, "player", "players")} · ${mins(rec.data.end - rec.data.start)}${rec.data.hasTrace ? " · with bot thoughts" : ""}`,
        onBack: back,
      }}
      key={`${file}:${rec.data.map}`}
    />
  );
}
