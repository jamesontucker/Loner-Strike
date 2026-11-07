// Copying maps from your own game to servers (mapcopy.rs): the picker of servers on a
// map's sheet, the picker of maps on a server's Maps page, and the progress while it runs.
// Computer only.

import { useDeferredValue, useEffect, useMemo, useState } from "react";
import { getJsonAt, useActions } from "../api";
import { useSetupJob, waitForSetupJob } from "../queries";
import type { MapInfo } from "../types";
import { BusyButton, Sheet, useToast } from "../ui";
import { JobProgress } from "./JobProgress";

const COPY_NOTE = "Players download each map the first time it plays. Big collections are slow to download without a download host (FastDL).";
const KIND = "mapcopy";
export const CATS: [string, string][] = [
  ["all", "All"], ["fav", "Favourites"], ["recent", "Recently played"], ["defuse", "Bomb defusal"],
  ["hostage", "Hostage rescue"], ["objective", "Objectives"], ["surf", "Surf"], ["arena", "Arena & fun"],
  ["other", "Other"],
];

type Reply = { ok?: boolean; error?: string; job?: string };
type Result = { message?: string; copied?: number; stopped?: boolean };
type ServerChoice = { id: string; name: string; game: string; remote: boolean; has: boolean; czOnly: boolean; missing: boolean };
type LocalMap = MapInfo & { files: [string, number][]; czOnly: boolean };

/** About your own game and the servers, whichever page asks: never sent with X-Target. */
async function post(path: string, body: unknown = {}): Promise<Reply> {
  try {
    const r = await fetch(path, { method: "POST", credentials: "same-origin", headers: { "Content-Type": "application/json", "X-Panel": "1" }, body: JSON.stringify(body) });
    return (await r.json()) as Reply;
  } catch {
    return { error: "Couldn't reach the app." };
  }
}

/** "184 MB", "1.2 GB", "640 KB" */
export function size(bytes: number) {
  if (bytes >= 1e9) return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e7) return `${Math.round(bytes / 1e6)} MB`;
  if (bytes >= 1e6) return `${(bytes / 1e6).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(bytes / 1e3))} KB`;
}

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

/** Starts a copy and follows it to the end with a toast. Whether it started. */
function useCopy() {
  const toast = useToast();
  const { run, busy } = useActions();
  const jobQ = useSetupJob();
  const running = jobQ.data?.state === "running" && jobQ.data.kind === KIND;
  const copy = (key: string, maps: string[], servers: string[], label: string, after: () => void) =>
    run(key, async () => {
      const r = await post("/api/mapcopy", { maps, servers });
      if (r.error) return toast(r.error, true);
      const j = await waitForSetupJob();
      const res = await getJsonAt<{ result?: Result | null }>("/api/mapcopy", null).catch(() => null);
      const msg = res?.result?.message;
      if (!j) toast("Lost track of the copy. Check the server's maps to see what arrived.", true);
      else if (j.state === "done") toast(msg || "Copied.");
      else toast(msg || j.error || "Couldn't copy those maps.", true);
      after();
    }, label);
  const stop = () => run("mapcopy:stop", async () => {
    const r = await post("/api/mapcopy/stop");
    if (r.error) toast(r.error, true);
  }, "Stopping…");
  return { copy, stop, running, busy };
}

/** The copy in progress, with Stop. */
function CopyProgress({ stop, stopping }: { stop: () => void; stopping: boolean }) {
  return (
    <div className="stack-tight">
      <JobProgress title="Copying maps…" onEnd={() => {}} />
      <div className="row">
        <BusyButton className="btn" busy={stopping} label="Stop" busyLabel="Stopping…" onClick={stop} />
        <span className="muted small-note">Maps already copied stay on the server.</span>
      </div>
    </div>
  );
}

/** From a map's sheet in your own game: which servers to copy it to. */
export function CopyToServers({ map, onClose }: { map: string; onClose: () => void }) {
  const [list, setList] = useState<ServerChoice[] | null>(null);
  const [error, setError] = useState("");
  const [chosen, setChosen] = useState<Set<string>>(() => new Set());
  const { copy, stop, running, busy } = useCopy();
  const key = `mapcopy:to:${map}`;
  const working = busy(key);
  const load = () =>
    getJsonAt<{ servers?: ServerChoice[]; error?: string }>(`/api/mapcopy/servers?map=${encodeURIComponent(map)}`, null)
      .then((r) => (r.error ? setError(r.error) : setList(r.servers ?? [])))
      .catch(() => setError("Couldn't reach the app."));
  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [map]);
  const tick = (id: string, on: boolean) => setChosen((c) => {
    const n = new Set(c);
    if (on) n.add(id);
    else n.delete(id);
    return n;
  });
  const start = () => copy(key, [map], [...chosen], `Copying ${map}…`, () => {
    setChosen(new Set());
    void load();
  });
  const cz = list?.some((s) => chosen.has(s.id) && s.czOnly);
  return (
    <Sheet label={`Copy ${map} to a server`} onClose={onClose}>
      <div className="sheet-body">
        <h2>Copy {map} to a server</h2>
        <p className="muted measure">The map goes with what it needs (textures, sky, models, sounds, the bots' file). Files a server already has stay as they are.</p>
        {error ? (
          <p className="muted">{error}</p>
        ) : !list ? (
          <p className="muted"><span className="spinner" aria-hidden="true" />Looking at your servers…</p>
        ) : !list.length ? (
          <p className="muted">You have no servers that are set up yet.</p>
        ) : (
          <div className="pick-list" role="group" aria-label="Servers">
            {list.map((s) => (
              <label key={s.id} className="pick-row">
                <input type="checkbox" checked={s.has || chosen.has(s.id)} disabled={s.has || s.missing || working || running} onChange={(e) => tick(s.id, e.target.checked)} />
                <span className="grow">{s.name}</span>
                <span className="muted small-note">
                  {s.has ? "Has it" : s.missing ? "Not in your game" : s.game === "czero" ? "Condition Zero" : "CS 1.6"}
                </span>
              </label>
            ))}
          </div>
        )}
        {cz && <p className="muted small-note">{map} is a Condition Zero map. It usually works on CS 1.6, but check it loads.</p>}
        <p className="muted small-note">{COPY_NOTE}</p>
        {(working || running) && <CopyProgress stop={stop} stopping={busy("mapcopy:stop")} />}
        <div className="row">
          <BusyButton className="btn primary" busy={working} disabled={!chosen.size || running} label={chosen.size > 1 ? `Copy to ${chosen.size} servers` : "Copy"} busyLabel="Copying…" onClick={start} />
          <button className="btn" onClick={onClose}>Close</button>
        </div>
      </div>
    </Sheet>
  );
}

/** On a server's Maps page: your game's maps it doesn't have yet, to pick and copy. */
export function CopyFromGame({ server, name, onClose, onDone }: { server: string; name: string; onClose: () => void; onDone: () => void }) {
  const [maps, setMaps] = useState<LocalMap[] | null>(null);
  const [error, setError] = useState("");
  const [cat, setCat] = useState("all");
  const [q, setQ] = useState("");
  const search = useDeferredValue(q);
  const [chosen, setChosen] = useState<Set<string>>(() => new Set());
  const { copy, stop, running, busy } = useCopy();
  const key = `mapcopy:from:${server}`;
  const working = busy(key);
  const load = () =>
    getJsonAt<{ maps?: LocalMap[]; error?: string }>(`/api/mapcopy/local?server=${encodeURIComponent(server)}`, null)
      .then((r) => (r.error ? setError(r.error) : setMaps(r.maps ?? [])))
      .catch(() => setError("Couldn't reach the app."));
  useEffect(() => {
    void load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [server]);

  const shown = useMemo(() => {
    const query = search.trim().toLowerCase();
    let list = maps ?? [];
    if (cat === "fav") list = list.filter((m) => m.fav);
    else if (cat === "recent") list = list.filter((m) => m.recent >= 0);
    else if (cat !== "all") list = list.filter((m) => m.cat === cat);
    if (query) list = list.filter((m) => `${m.name} ${m.title ?? ""} ${m.by}`.toLowerCase().includes(query));
    return list;
  }, [maps, cat, search]);
  const bytesOf = (m: LocalMap) => m.files.reduce((n, [, b]) => n + b, 0);
  // a texture file two maps share goes once
  const total = useMemo(() => {
    const files = new Map<string, number>();
    for (const m of maps ?? []) if (chosen.has(m.name)) for (const [f, b] of m.files) files.set(f, b);
    let n = 0;
    files.forEach((b) => (n += b));
    return n;
  }, [maps, chosen]);
  const tick = (m: string, on: boolean) => setChosen((c) => {
    const n = new Set(c);
    if (on) n.add(m);
    else n.delete(m);
    return n;
  });
  const allShown = shown.length > 0 && shown.every((m) => chosen.has(m.name));
  const start = () => copy(key, [...chosen], [server], `Copying maps to ${name}…`, () => {
    setChosen(new Set());
    onDone();
    void load();
  });
  const cz = (maps ?? []).some((m) => chosen.has(m.name) && m.czOnly);
  const locked = working || running;

  return (
    <Sheet label="Copy maps from my game" className="sheet wide" onClose={onClose}>
      <div className="sheet-body">
        <h2>Copy maps from my game</h2>
        <p className="muted measure">Maps in your game that {name} doesn't have yet. Each goes with what it needs; files the server already has stay as they are.</p>
        {error ? (
          <p className="muted">{error}</p>
        ) : !maps ? (
          <p className="muted"><span className="spinner" aria-hidden="true" />Looking at your maps…</p>
        ) : !maps.length ? (
          <p className="muted">{name} has every map your game has.</p>
        ) : (
          <>
            <div className="toolbar">
              <input className="search" type="search" aria-label="Search your maps" placeholder="Search maps" value={q} onChange={(e) => setQ(e.target.value)} />
              <div className="chips" role="group" aria-label="Map type">
                {CATS.map(([id, label]) => (
                  <button key={id} className="chip" aria-pressed={cat === id} onClick={() => setCat(id)}>{label}</button>
                ))}
              </div>
            </div>
            <div className="row">
              <button className="btn" disabled={locked || !shown.length || allShown} onClick={() => setChosen((c) => new Set([...c, ...shown.map((m) => m.name)]))}>Select all</button>
              <button className="btn" disabled={locked || !chosen.size} onClick={() => setChosen(new Set())}>Clear</button>
              <span className="muted">{chosen.size ? `${plural(chosen.size, "map")}, ${size(total)}` : "No maps picked yet"}</span>
            </div>
            <div className="pick-list" role="group" aria-label="Your maps">
              {shown.map((m) => (
                <label key={m.name} className="pick-row">
                  <input type="checkbox" checked={chosen.has(m.name)} disabled={locked} onChange={(e) => tick(m.name, e.target.checked)} />
                  <span className="grow">
                    {m.name}
                    {m.title && m.title !== m.name && <span className="muted small-note"> · {m.title}</span>}
                  </span>
                  <span className="muted small-note">{m.czOnly ? "Condition Zero · " : ""}{size(bytesOf(m))}</span>
                </label>
              ))}
              {!shown.length && <p className="muted">No maps match.</p>}
            </div>
          </>
        )}
        {cz && <p className="muted small-note">Condition Zero maps usually work on CS 1.6, but check they load.</p>}
        <p className="muted small-note">{COPY_NOTE}</p>
        {locked && <CopyProgress stop={stop} stopping={busy("mapcopy:stop")} />}
        <div className="row">
          <BusyButton className="btn primary" busy={working} disabled={!chosen.size || running} label={chosen.size ? `Copy ${plural(chosen.size, "map")}` : "Copy"} busyLabel="Copying…" onClick={start} />
          <button className="btn" onClick={onClose}>Close</button>
        </div>
      </div>
    </Sheet>
  );
}
