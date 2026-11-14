// A server's "Community content" tab: which maps and Game Mode Studio modes installed from
// Community this server carries. Maps go over with the app's map copy (Linux servers: copied,
// then uploaded; FastDL serves them to players from there); modes are built on this computer
// first, then their plugin and mode files go to the server. Unticking takes them off again.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState } from "react";
import { useNav } from "../../../store";
import { BusyButton, LoadError, useToast } from "../../../ui";
import { JobProgress } from "../../JobProgress";
import { errorText, useBusy } from "../common/api";
import { Empty, Loading } from "../common";
import { serverContent, setServerContent, type ServerItem } from "./communityApi";
import "./community.css";

export default function ServerContent({ server }: { server: { id: string; name: string } }) {
  const toast = useToast();
  const qc = useQueryClient();
  const q = useQuery({ queryKey: ["tools", "community", "server", server.id], queryFn: ({ signal }) => serverContent(server.id, signal), retry: false });
  const [maps, setMaps] = useState<Set<string>>(new Set());
  const [modes, setModes] = useState<Set<string>>(new Set());
  const [copying, setCopying] = useState(false);
  const [problems, setProblems] = useState<string[]>([]);
  // (a server whose menu couldn't read the mode list again on the spot)
  const [modesNote, setModesNote] = useState<string | null>(null);
  const { working, run } = useBusy();
  useEffect(() => {
    if (!q.data) return;
    setMaps(new Set(q.data.items.filter((i) => i.kind === "map" && i.on).map((i) => i.key)));
    setModes(new Set(q.data.items.filter((i) => i.kind === "mode" && i.on).map((i) => i.key)));
  }, [q.data]);
  const items = q.data?.items ?? [];
  const changed = useMemo(
    () => items.some((i) => (i.kind === "map" ? maps.has(i.key) : modes.has(i.key)) !== i.on),
    [items, maps, modes],
  );
  if (q.isPending) return <Loading text="Loading community content…" />;
  if (q.isError) return <LoadError text={errorText(q.error, "Couldn't load this server's community content.")} onRetry={() => q.refetch()} />;
  if (!items.length)
    return (
      <Empty title="Nothing from Community to carry yet" text="Maps and Game Mode Studio modes you install from Tools > Community can be put on your servers from here.">
        <button className="btn primary" onClick={() => useNav.getState().go("tools", "community")}>Browse Community</button>
      </Empty>
    );
  const toggle = (set: Set<string>, put: (s: Set<string>) => void, key: string) => {
    const n = new Set(set);
    if (n.has(key)) n.delete(key);
    else n.add(key);
    put(n);
  };
  const save = () =>
    run("Applying…", async () => {
      const r = await setServerContent(server.id, { maps: [...maps], modes: [...modes] });
      if (r.error) return toast(r.error, true);
      setProblems(r.problems ?? []);
      setModesNote(r.modes === "next_map" ? (r.modesNote ?? "Available after the next map change") : null);
      if (r.uploaded === false) toast(`Saved, but ${server.name} couldn't be reached to upload the changes. They go with the next upload.`, true);
      if (r.job) setCopying(true);
      else toast("Done.");
      qc.invalidateQueries({ queryKey: ["tools", "community", "server", server.id] });
    });
  const row = (i: ServerItem) => {
    const on = i.kind === "map" ? maps.has(i.key) : modes.has(i.key);
    return (
      <li key={i.key} className="cm-srv-row">
        <label className="check-row">
          <input type="checkbox" checked={on} disabled={working !== null || copying} onChange={() => (i.kind === "map" ? toggle(maps, setMaps, i.key) : toggle(modes, setModes, i.key))} />
          <span>
            <strong>{i.name}</strong>
            <span className="muted small-note">
              {" "}
              {i.kind === "map" ? (i.maps.length ? i.maps.join(", ") : "map") : `mode "${i.mode}"`}
              {i.author ? ` · by ${i.author}` : ""}
            </span>
          </span>
        </label>
        <span className={`small-note ${i.there ? "cm-ok" : "muted"}`}>{i.there ? "On the server" : on ? "Not there yet" : ""}</span>
      </li>
    );
  };
  const mapRows = items.filter((i) => i.kind === "map");
  const modeRows = items.filter((i) => i.kind === "mode");
  return (
    <div className="stack-tight">
      <div className="section-head">
        <div className="section-title"><h2>Community content</h2></div>
        <p>
          Maps and Game Mode Studio modes you installed from Community that {server.name} carries. Maps are copied over{q.data.server.remote ? " and uploaded" : ""}, and players joining download them (FastDL, when it's on). Modes are built on this computer first. Untick to take one off again.
        </p>
      </div>
      {copying && (
        <JobProgress
          title={`Copying maps to ${server.name}…`}
          onEnd={() => {
            setCopying(false);
            qc.invalidateQueries({ queryKey: ["tools", "community", "server", server.id] });
          }}
        />
      )}
      {mapRows.length > 0 && (
        <section className="panel">
          <h3>Maps</h3>
          <ul className="cm-srv">{mapRows.map(row)}</ul>
        </section>
      )}
      {modeRows.length > 0 && (
        <section className="panel">
          <h3>Game modes</h3>
          <ul className="cm-srv">{modeRows.map(row)}</ul>
          {modesNote && <p className="muted small-note">{modesNote}: {server.name} lists mode changes once its map changes.</p>}
        </section>
      )}
      {problems.length > 0 && (
        <div className="panel" role="alert">
          {problems.map((p) => <p key={p} className="cm-line warn">{p}</p>)}
        </div>
      )}
      <div className="row">
        <BusyButton className="btn primary" busy={working !== null} busyLabel={working ?? "Applying…"} label="Apply" disabled={!changed || copying} onClick={save} />
        {changed && <button className="btn" onClick={() => q.data && (setMaps(new Set(items.filter((i) => i.kind === "map" && i.on).map((i) => i.key))), setModes(new Set(items.filter((i) => i.kind === "mode" && i.on).map((i) => i.key))))}>Undo changes</button>}
      </div>
    </div>
  );
}
