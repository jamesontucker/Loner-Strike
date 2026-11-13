// The demo manager: the demos you recorded in the game and the ones HLTV recorded on your
// servers. What's in each (map, length, who played), rename, delete, watch.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { useNav } from "../../../store";
import { Banner, BusyButton, LoadError, Segmented, useAskText, useConfirm, useToast } from "../../../ui";
import { errorText, get } from "../common/api";
import { Empty, Loading } from "../common";
import { demoDetail, deleteDemo, demoToReplay, listDemos, playDemo, renameDemo, type Demo } from "./demosApi";
import "./demos.css";

const key = (d: Demo) => `${d.at}/${d.name}`;

function length(s?: number) {
  if (!s || !Number.isFinite(s) || s <= 0) return "";
  const m = Math.floor(s / 60);
  const sec = Math.round(s % 60);
  return m >= 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : m > 0 ? `${m}:${String(sec).padStart(2, "0")}` : `${sec} s`;
}

function when(t: number) {
  if (!t) return "";
  return new Date(t * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

const mb = (n: number) => (n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(n >= 100 << 20 ? 0 : 1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);

export default function Demos() {
  const list = useQuery({ queryKey: ["tools", "demos"], queryFn: ({ signal }) => listDemos(signal), retry: false });
  const [q, setQ] = useState("");
  const [kind, setKind] = useState<"all" | "mine" | "hltv">("all");
  const [open, setOpen] = useState<string | null>(null);

  const demos = list.data?.demos ?? [];
  const shown = useMemo(() => {
    const s = q.trim().toLowerCase();
    return demos.filter((d) => (kind === "all" || (kind === "hltv") === d.hltv) && (!s || d.name.toLowerCase().includes(s) || (d.map ?? "").toLowerCase().includes(s) || d.where.toLowerCase().includes(s)));
  }, [demos, q, kind]);

  if (list.isPending) return <Loading text="Looking for demos…" />;
  if (list.isError) return <LoadError text={errorText(list.error, "Couldn't list your demos.")} onRetry={() => list.refetch()} />;

  if (!demos.length)
    return (
      <div className="tl-page">
        <Empty
          title="No demos yet"
          text={
            <>
              Record one in the game: open the console (the ~ key) and type <code>record mydemo</code>, play, then type <code>stop</code>. HLTV on your servers can record whole
              matches too (Servers, then HLTV). They show up here.
            </>
          }
        >
          <BusyButton className="btn" busy={list.isFetching} label="Look again" busyLabel="Looking…" onClick={() => list.refetch()} />
        </Empty>
      </div>
    );

  return (
    <div className="tl-page stack-tight">
      {list.data?.gameOpen && <Banner>The game is open. Watch tells you what to type in its console instead of starting it again.</Banner>}
      <div className="toolbar dm-bar">
        <input className="search" type="search" aria-label="Search demos" placeholder="Search by name or map" value={q} onChange={(e) => setQ(e.target.value)} />
        <Segmented label="Which demos" values="all mine hltv" labels="All|Recorded in game|HLTV" current={kind} onPick={(v) => setKind(v as typeof kind)} />
        <span className="muted small-note tl-count" aria-live="polite">{shown.length} demo{shown.length === 1 ? "" : "s"}</span>
        <BusyButton className="btn dm-refresh" busy={list.isFetching} label="Refresh" busyLabel="Looking…" onClick={() => list.refetch()} />
      </div>
      {shown.length === 0 ? (
        <Empty plain text="No demos match." />
      ) : (
        <ul className="dm-list">
          {shown.map((d) => (
            <li key={key(d)} className={open === key(d) ? "open" : ""}>
              <button className="dm-row" aria-expanded={open === key(d)} onClick={() => setOpen(open === key(d) ? null : key(d))}>
                <span className="dm-name">{d.name.replace(/\.dem$/i, "")}</span>
                {d.hltv && <span className="tag">HLTV</span>}
                <span className="dm-map">{d.error ? <span className="dm-bad">Can't be read</span> : d.map}</span>
                <span className="dm-len">{length(d.seconds)}</span>
                <span className="muted dm-when">{when(d.modified)}</span>
                <span className="muted dm-size">{mb(d.size)}</span>
              </button>
              {open === key(d) && <Details demo={d} gameOpen={!!list.data?.gameOpen} onGone={() => setOpen(null)} />}
            </li>
          ))}
        </ul>
      )}
      <p className="muted small-note">
        Looked in: {list.data?.places.map((p) => p.label).join(", ")}. Servers on other computers keep their HLTV demos there: copy one to your game from the server's HLTV page.
      </p>
    </div>
  );
}

function Details({ demo, gameOpen, onGone }: { demo: Demo; gameOpen: boolean; onGone: () => void }) {
  const toast = useToast();
  const confirm = useConfirm();
  const askText = useAskText();
  const qc = useQueryClient();
  const detail = useQuery({ queryKey: ["tools", "demos", key(demo), demo.size, demo.modified], queryFn: ({ signal }) => demoDetail(demo, signal), retry: false, enabled: !demo.error });
  const [busy, setBusy] = useState<"play" | "rename" | "delete" | "replay" | null>(null);
  const [command, setCommand] = useState<string | null>(null);
  const reload = () => qc.invalidateQueries({ queryKey: ["tools", "demos"] });

  const play = async () => {
    setBusy("play");
    const r = await playDemo(demo);
    setBusy(null);
    if (r.error) return toast(r.error, true);
    if (r.gameOpen && r.command) {
      setCommand(r.command);
      return;
    }
    toast(r.message || "Starting the game…");
    if (r.copied) reload();
  };

  const rename = async () => {
    const to = await askText(`A new name for ${demo.name}`, { label: "Name", placeholder: demo.name.replace(/\.dem$/i, ""), max: 60 }, { ok: "Rename" });
    if (!to) return;
    setBusy("rename");
    const r = await renameDemo(demo, to);
    setBusy(null);
    if (r.error) return toast(r.error, true);
    toast(r.message || "Renamed.");
    onGone();
    reload();
  };

  const safeRename = async () => {
    if (!demo.safeName) return;
    setBusy("rename");
    const r = await renameDemo(demo, demo.safeName);
    setBusy(null);
    if (r.error) return toast(r.error, true);
    toast(r.message || "Renamed.");
    onGone();
    reload();
  };

  const remove = async () => {
    if (!(await confirm(`Delete ${demo.name}? It can't be brought back.`, { ok: "Delete", danger: true }))) return;
    setBusy("delete");
    const r = await deleteDemo(demo);
    setBusy(null);
    if (r.error) return toast(r.error, true);
    toast(r.message || "Deleted.");
    onGone();
    reload();
  };

  const replay = async () => {
    setBusy("replay");
    const r = await demoToReplay(demo);
    if (r.error || !r.id) {
      setBusy(null);
      return toast(r.error || "Couldn't read the demo.", true);
    }
    // the Replay tool knows recordings once it has listed them
    await get("/api/tools/recordings").catch(() => null);
    await qc.invalidateQueries({ queryKey: ["tools", "recordings"] });
    setBusy(null);
    useNav.getState().go("tools", `replay:${r.id}`);
  };

  const copy = async () => {
    if (!command) return;
    try {
      await navigator.clipboard.writeText(command);
      toast("Copied.");
    } catch {
      toast("Couldn't copy it. Select the text and copy it yourself.", true);
    }
  };

  const d = detail.data;
  const off = busy !== null;
  return (
    <div className="dm-detail">
      <dl className="dm-facts">
        <div><dt>Where</dt><dd>{demo.where}</dd></div>
        {demo.map && <div><dt>Map</dt><dd>{demo.map}</dd></div>}
        {demo.seconds ? <div><dt>Length</dt><dd>{length(demo.seconds)}</dd></div> : null}
        <div><dt>Recorded</dt><dd>{when(demo.modified)}</dd></div>
        {d?.server && <div><dt>Server</dt><dd>{d.server}</dd></div>}
        {demo.error && <div><dt>Problem</dt><dd className="dm-bad">{demo.error}</dd></div>}
      </dl>
      {detail.isFetching && !d && <Loading text="Looking inside…" />}
      {detail.isError && <p className="dm-bad">{errorText(detail.error, "Couldn't look inside this demo.")}</p>}
      {d && (
        <>
          {d.names.length > 0 && (
            <div className="dm-names">
              <span className="dm-label">Players seen{d.names.length >= 64 ? " (the first 64)" : ""}</span>
              <p>{d.names.join(", ")}</p>
            </div>
          )}
          {d.problem && <p className="muted small-note">{d.problem}</p>}
        </>
      )}
      {command && (
        <div className="dm-command" role="status">
          <span>The game is open. Open its console (the ~ key) and type:</span>
          <code>{command}</code>
          <button className="btn" onClick={copy}>Copy</button>
        </div>
      )}
      <div className="row dm-actions">
        <BusyButton
          className="btn primary"
          busy={busy === "play"}
          disabled={off || !!demo.error || !demo.playable}
          label={gameOpen ? "How to watch it" : demo.at.startsWith("server:") ? "Copy to my game and watch" : "Watch"}
          busyLabel={demo.at.startsWith("server:") ? "Copying…" : "Starting…"}
          onClick={play}
        />
        {d?.canReplay && (
          <BusyButton className="btn" busy={busy === "replay"} disabled={off} label="Show my path in Replay" busyLabel="Reading the demo…" onClick={replay} />
        )}
        <BusyButton className="btn" busy={busy === "rename"} disabled={off} label="Rename" busyLabel="Renaming…" onClick={rename} />
        <BusyButton className="btn danger" busy={busy === "delete"} disabled={off} label="Delete" busyLabel="Deleting…" onClick={remove} />
      </div>
      {!demo.playable && !demo.error && (
        <div className="row dm-actions">
          <span className="muted small-note">The game can only be told to play a demo named with plain letters, numbers and - _ . ( ) [ ].</span>
          {demo.safeName && <BusyButton className="btn" busy={busy === "rename"} disabled={off} label={`Rename to ${demo.safeName}`} busyLabel="Renaming…" onClick={safeRename} />}
        </div>
      )}
    </div>
  );
}
