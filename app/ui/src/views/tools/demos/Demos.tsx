// The demo manager: the demos you recorded in the game and the ones HLTV recorded on your
// servers. A table (sortable, the map's picture, a badge for where it came from, actions on
// hover) and, for the picked demo, a side panel with what's in it: players, server, length.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { useNav } from "../../../store";
import type { MapInfo } from "../../../types";
import { Banner, BusyButton, Info, LoadError, Segmented, useAskText, useConfirm, useToast } from "../../../ui";
import { Empty, Loading } from "../common";
import { errorText, get } from "../common/api";
import { RESERVED_NAME, windowsReserved } from "../common/fileNames";
import { Thumb } from "../common/MapBrowser";
import { matches, searchWords } from "../common/search";
import type { ToolProps } from "../registry";
import { deleteDemo, demoDetail, demoToReplay, listDemos, playDemo, renameDemo, revealDemo, type Demo } from "./demosApi";
import "./demos.css";

const key = (d: Demo) => `${d.at}/${d.name}`;
const stem = (n: string) => n.replace(/\.dem$/i, "");
const FOLDER = typeof navigator !== "undefined" && /Mac/.test(navigator.platform) ? "Finder" : "the folder";

function length(s?: number) {
  if (!s || !Number.isFinite(s) || s <= 0) return "";
  const m = Math.floor(s / 60);
  const sec = Math.round(s % 60);
  return m >= 60 ? `${Math.floor(m / 60)} h ${m % 60} min` : m > 0 ? `${m}:${String(sec).padStart(2, "0")}` : `${sec} s`;
}

const full = (t: number) => (t ? new Date(t * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }) : "");

/** "3 hours ago", "yesterday", "12 Mar" (the full date and time in its tooltip) */
function ago(t: number) {
  if (!t) return "";
  const s = Date.now() / 1000 - t;
  const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
  if (s < 60) return "just now";
  if (s < 3600) return rtf.format(-Math.round(s / 60), "minute");
  if (s < 86400) return rtf.format(-Math.round(s / 3600), "hour");
  if (s < 7 * 86400) return rtf.format(-Math.round(s / 86400), "day");
  const d = new Date(t * 1000);
  return d.toLocaleDateString(undefined, d.getFullYear() === new Date().getFullYear() ? { day: "numeric", month: "short" } : { day: "numeric", month: "short", year: "numeric" });
}

const mb = (n: number) => (n <= 0 ? "Empty" : n >= 1 << 20 ? `${(n / (1 << 20)).toFixed(n >= 100 << 20 ? 0 : 1)} MB` : `${Math.max(1, Math.round(n / 1024))} KB`);
/** Why a new name can't be used, before asking the server (it checks again). */
const nameProblem = (to: string) => {
  const name = /\.dem$/i.test(to) ? to : `${to}.dem`;
  if (!to) return "Type a name.";
  if (windowsReserved(name)) return RESERVED_NAME;
  if (!/^[A-Za-z0-9_\-()[\]][A-Za-z0-9 _\-.()[\]]*$/.test(name) || name.length > 128) return "That name can't be used. Use letters, numbers, spaces and - _ . ( ) [ ].";
  return null;
};

type Sort = "name" | "map" | "seconds" | "modified" | "size";
type Busy = "play" | "rename" | "delete" | "replay" | "reveal";

/** What a demo's buttons do (the row's and the side panel's). */
function useActions(onRenamed: (d: Demo, to: string) => void, onGone: (d: Demo) => void, onCommand: (d: Demo, c: string) => void) {
  const toast = useToast();
  const confirm = useConfirm();
  const askText = useAskText();
  const qc = useQueryClient();
  const [busy, setBusy] = useState<{ key: string; what: Busy } | null>(null);
  const reload = () => qc.invalidateQueries({ queryKey: ["tools", "demos"] });
  const doing = async <T,>(d: Demo, what: Busy, fn: () => Promise<T>) => {
    setBusy({ key: key(d), what });
    try {
      return await fn();
    } finally {
      setBusy(null);
    }
  };

  const play = (d: Demo) =>
    doing(d, "play", async () => {
      const r = await playDemo(d);
      if (r.error) return toast(r.error, true);
      if (r.gameOpen && r.command) return onCommand(d, r.command);
      toast(r.message || "Starting the game…");
      if (r.copied) reload();
    });
  /** The error in words, or null when renamed. */
  const renameNow = async (d: Demo, to: string): Promise<string | null> => {
    const r = await renameDemo(d, to);
    if (r.error) return r.error;
    toast(r.message || "Renamed.");
    onRenamed(d, r.name ?? (/\.dem$/i.test(to) ? to : `${to}.dem`));
    reload();
    return null;
  };
  const renameTo = (d: Demo, to: string) =>
    doing(d, "rename", async () => {
      const why = await renameNow(d, to);
      if (why) toast(why, true);
    });
  // the dialog stays open on an error, with it under the name and what was typed kept
  const rename = (d: Demo) =>
    askText(
      `A new name for ${d.name}`,
      { label: "Name", placeholder: stem(d.name), max: 60, value: stem(d.name) },
      { ok: "Rename", busy: "Renaming…", check: (to) => nameProblem(to) ?? (to === stem(d.name) ? null : doing(d, "rename", () => renameNow(d, to))) },
    );
  const remove = async (d: Demo) => {
    if (!(await confirm(`Delete ${d.name}? It can't be brought back.`, { ok: "Delete", danger: true }))) return;
    await doing(d, "delete", async () => {
      const r = await deleteDemo(d);
      if (r.error) return toast(r.error, true);
      toast(r.message || "Deleted.");
      onGone(d);
      reload();
    });
  };
  const reveal = (d: Demo) =>
    doing(d, "reveal", async () => {
      const r = await revealDemo(d);
      if (r.error) toast(r.error, true);
    });
  const replay = (d: Demo) =>
    doing(d, "replay", async () => {
      const r = await demoToReplay(d);
      if (r.error || !r.id) return toast(r.error || "Couldn't read the demo.", true);
      // the Replay tool knows recordings once it has listed them
      await get("/api/tools/recordings").catch(() => null);
      await qc.invalidateQueries({ queryKey: ["tools", "recordings"] });
      useNav.getState().go("tools", `replay:${r.id}`);
    });
  return { busy, play, rename, renameTo, remove, reveal, replay };
}
type Actions = ReturnType<typeof useActions>;

function Icon({ d }: { d: string }) {
  return (
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={d} />
    </svg>
  );
}
const PLAY = "M7 4.5v15l12-7.5z";
const PEN = "M4 20h4L19 9l-4-4L4 16zM13.5 6.5l4 4";
const FOLDER_ICON = "M3 6h6l2 2h10v11H3z";
const PATH = "M4 19c4 0 3-7 8-7s4-7 8-7M4 19h.01M20 5h.01";
const BIN = "M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13";

export default function Demos({ maps }: ToolProps) {
  const list = useQuery({ queryKey: ["tools", "demos"], queryFn: ({ signal }) => listDemos(signal), retry: false });
  const [q, setQ] = useState("");
  const [kind, setKind] = useState<"all" | "mine" | "hltv">("all");
  const [sort, setSort] = useState<{ by: Sort; up: boolean }>({ by: "modified", up: false });
  const [open, setOpen] = useState<string | null>(null);
  const [command, setCommand] = useState<{ key: string; text: string } | null>(null);
  const tableRef = useRef<HTMLTableSectionElement>(null);
  const acts = useActions(
    (d, to) => setOpen((o) => (o === key(d) ? `${d.at}/${to}` : o)),
    (d) => setOpen((o) => (o === key(d) ? null : o)),
    (d, c) => {
      setOpen(key(d));
      setCommand({ key: key(d), text: c });
    },
  );

  const byName = useMemo(() => new Map(maps.map((m) => [m.name.toLowerCase(), m])), [maps]);
  const demos = list.data?.demos ?? [];
  const shown = useMemo(() => {
    const words = searchWords(q);
    const rows = demos.filter((d) => (kind === "all" || (kind === "hltv") === d.hltv) && matches(words, d.name, d.map ?? "", d.where));
    const v = (d: Demo): string | number => (sort.by === "name" ? d.name.toLowerCase() : sort.by === "map" ? (d.map ?? "").toLowerCase() : (d[sort.by] ?? 0));
    return [...rows].sort((a, b) => (v(a) < v(b) ? -1 : v(a) > v(b) ? 1 : 0) * (sort.up ? 1 : -1));
  }, [demos, q, kind, sort]);
  const picked = demos.find((d) => key(d) === open) ?? null;

  // a demo that went away (deleted elsewhere, renamed) closes its panel
  useEffect(() => {
    if (open && list.data && !picked) setOpen(null);
  }, [open, list.data, picked]);

  if (list.isPending) return <Loading text="Looking for demos…" />;
  if (list.isError) return <LoadError text={errorText(list.error, "Couldn't list your demos.")} onRetry={() => list.refetch()} />;

  const lookedIn = (
    <>
      Looked in: {list.data?.places.map((p) => p.label).join(", ")}. Servers on other computers keep their HLTV demos there: copy one to your game from the server's HLTV page.
    </>
  );

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
          <p className="muted small-note">{lookedIn}</p>
          <BusyButton className="btn" busy={list.isFetching} label="Look again" busyLabel="Looking…" onClick={() => list.refetch()} />
        </Empty>
      </div>
    );

  const head = (by: Sort, label: string, cls: string) => (
    <th className={cls} aria-sort={sort.by === by ? (sort.up ? "ascending" : "descending") : "none"}>
      <button type="button" onClick={() => setSort((s) => (s.by === by ? { by, up: !s.up } : { by, up: by === "name" || by === "map" }))}>
        {label}
        <span className="dm-sort" aria-hidden="true">{sort.by === by ? (sort.up ? "▲" : "▼") : ""}</span>
      </button>
    </th>
  );

  // ↑/↓ move the pick, Enter watches
  const rowKeys = (e: KeyboardEvent<HTMLTableRowElement>, i: number) => {
    if (e.target !== e.currentTarget) return;
    const to = e.key === "ArrowDown" ? i + 1 : e.key === "ArrowUp" ? i - 1 : null;
    if (to !== null && shown[to]) {
      e.preventDefault();
      setOpen(key(shown[to]));
      (tableRef.current?.children[to] as HTMLElement | undefined)?.focus();
    } else if (e.key === "Enter") {
      setOpen(key(shown[i]));
    }
  };

  return (
    <div className="tl-page dm-page">
      {list.data?.gameOpen && <Banner>The game is open. Watch tells you what to type in its console instead of starting it again.</Banner>}
      <div className="toolbar dm-bar">
        <input className="search" type="search" aria-label="Search demos" placeholder="Search by name, map or place" value={q} onChange={(e) => setQ(e.target.value)} />
        <Segmented label="Which demos" values="all mine hltv" labels="All|Recorded in game|HLTV" current={kind} onPick={(v) => setKind(v as typeof kind)} />
        <span className="dm-count">
          <span className="muted small-note tl-count" aria-live="polite">
            {shown.length} demo{shown.length === 1 ? "" : "s"}
          </span>
          <Info text={lookedIn} />
        </span>
        <BusyButton className="btn dm-refresh" busy={list.isFetching} label="Refresh" busyLabel="Looking…" onClick={() => list.refetch()} />
      </div>

      <div className={`dm-layout${picked ? " with-panel" : ""}`}>
        {shown.length === 0 ? (
          kind === "hltv" && !q.trim() ? (
            <Empty plain text="No HLTV demos yet. HLTV on one of your servers records whole matches: Servers, then the server's HLTV.">
              <button type="button" className="btn small" onClick={() => useNav.getState().go("servers")}>
                Go to Servers
              </button>
            </Empty>
          ) : (
            <Empty plain text="No demos match." />
          )
        ) : (
          <div className="dm-table-wrap panel">
            <table className="dm-table">
              <thead>
                <tr>
                  {head("name", "Demo and map", "dm-c-name")}
                  {head("seconds", "Length", "dm-c-len")}
                  {head("modified", "Recorded", "dm-c-when")}
                  {head("size", "Size", "dm-c-size")}
                  <th className="dm-c-act">
                    <span className="sr-only">Actions</span>
                  </th>
                </tr>
              </thead>
              <tbody ref={tableRef}>
                {shown.map((d, i) => (
                  <Row key={key(d)} d={d} map={d.map ? byName.get(d.map.toLowerCase()) : undefined} on={open === key(d)} gameOpen={!!list.data?.gameOpen} acts={acts} onPick={() => setOpen(open === key(d) ? null : key(d))} onKey={(e) => rowKeys(e, i)} />
                ))}
              </tbody>
            </table>
          </div>
        )}
        {picked && (
          <Details
            key={key(picked)}
            demo={picked}
            map={picked.map ? byName.get(picked.map.toLowerCase()) : undefined}
            gameOpen={!!list.data?.gameOpen}
            acts={acts}
            command={command?.key === key(picked) ? command.text : null}
            onClose={() => setOpen(null)}
          />
        )}
      </div>
    </div>
  );
}

function MapPic({ name, m, big }: { name?: string; m?: MapInfo; big?: boolean }) {
  const info = m ?? ({ name: name || "?", thumb: null } as unknown as MapInfo);
  return (
    <span className={`dm-thumb${big ? " big" : ""}`}>
      <Thumb m={info} small={!big} />
    </span>
  );
}

function Badge({ d }: { d: Demo }) {
  return d.hltv ? (
    <span className="dm-badge hltv" title="Recorded by HLTV on one of your servers">
      HLTV
    </span>
  ) : (
    <span className="dm-badge mine" title="Recorded in your game (record / stop in the console)">
      In game
    </span>
  );
}

function Row({ d, map, on, gameOpen, acts, onPick, onKey }: { d: Demo; map?: MapInfo; on: boolean; gameOpen: boolean; acts: Actions; onPick: () => void; onKey: (e: KeyboardEvent<HTMLTableRowElement>) => void }) {
  const mine = acts.busy?.key === key(d) ? acts.busy.what : null;
  const off = !!acts.busy;
  const stop = (fn: () => unknown) => (e: React.MouseEvent) => {
    e.stopPropagation();
    fn();
  };
  return (
    <tr className={`dm-row${on ? " on" : ""}`} tabIndex={0} aria-selected={on} onClick={onPick} onKeyDown={onKey}>
      <td className="dm-c-name">
        <span className="dm-name-cell">
          <MapPic name={d.map} m={map} />
          <span className="dm-name-text">
            <span className="dm-name" title={d.name}>
              {stem(d.name)}
            </span>
            <span className="dm-sub">
              <Badge d={d} />
              {d.error ? <span className="dm-bad">Can't be read</span> : <span className="dm-map">{d.map}</span>}
              {d.problem && !d.error && (
                <span className="dm-bad" title={d.problem}>
                  Cut short
                </span>
              )}
              <span className="muted">· {d.where.replace(/^Your game \((.*)\)$/, "$1")}</span>
            </span>
          </span>
        </span>
      </td>
      <td className="dm-c-len">{length(d.seconds)}</td>
      <td className="dm-c-when" title={full(d.modified)}>
        {ago(d.modified)}
      </td>
      <td className="dm-c-size">{mb(d.size)}</td>
      <td className="dm-c-act">
        <span className="dm-row-acts">
          <button type="button" className="dm-act" disabled={off || !!d.error || !!d.problem || !d.playable} aria-label={`Watch ${d.name}`} title={d.problem ? d.problem : !d.playable ? "Rename it to a plain name first" : gameOpen ? "How to watch it (the game is open)" : "Watch"} onClick={stop(() => acts.play(d))}>
            {mine === "play" ? <span className="spinner" /> : <Icon d={PLAY} />}
          </button>
          {!d.hltv && !d.error && (
            <button type="button" className="dm-act" disabled={off} aria-label={`Show my path in Replay from ${d.name}`} title="Show my path in Replay" onClick={stop(() => acts.replay(d))}>
              {mine === "replay" ? <span className="spinner" /> : <Icon d={PATH} />}
            </button>
          )}
          <button type="button" className="dm-act" disabled={off} aria-label={`Rename ${d.name}`} title="Rename" onClick={stop(() => acts.rename(d))}>
            {mine === "rename" ? <span className="spinner" /> : <Icon d={PEN} />}
          </button>
          <button type="button" className="dm-act" disabled={off} aria-label={`Show ${d.name} in ${FOLDER}`} title={`Show in ${FOLDER}`} onClick={stop(() => acts.reveal(d))}>
            {mine === "reveal" ? <span className="spinner" /> : <Icon d={FOLDER_ICON} />}
          </button>
          <button type="button" className="dm-act danger" disabled={off} aria-label={`Delete ${d.name}`} title="Delete" onClick={stop(() => acts.remove(d))}>
            {mine === "delete" ? <span className="spinner" /> : <Icon d={BIN} />}
          </button>
        </span>
      </td>
    </tr>
  );
}

function Details({ demo, map, gameOpen, acts, command, onClose }: { demo: Demo; map?: MapInfo; gameOpen: boolean; acts: Actions; command: string | null; onClose: () => void }) {
  const toast = useToast();
  const detail = useQuery({ queryKey: ["tools", "demos", key(demo), demo.size, demo.modified], queryFn: ({ signal }) => demoDetail(demo, signal), retry: false, enabled: !demo.error });
  const mine = acts.busy?.key === key(demo) ? acts.busy.what : null;
  const off = !!acts.busy;
  const d = detail.data;

  const copy = async () => {
    if (!command) return;
    try {
      await navigator.clipboard.writeText(command);
      toast("Copied.");
    } catch {
      toast("Couldn't copy it. Select the text and copy it yourself.", true);
    }
  };

  return (
    <aside className="panel dm-panel" aria-label={`About ${demo.name}`}>
      <div className="dm-panel-head">
        <MapPic name={demo.map} m={map} big />
        <button type="button" className="btn small ghost dm-close" aria-label="Close" title="Close" onClick={onClose}>
          ×
        </button>
      </div>
      <h3 className="dm-panel-title" title={demo.name}>
        {stem(demo.name)}
      </h3>
      <div className="dm-sub">
        <Badge d={demo} />
        {demo.map && <span className="muted">{demo.map}</span>}
      </div>

      <div className="dm-panel-actions">
        <BusyButton
          className="btn primary"
          busy={mine === "play"}
          disabled={off || !!demo.error || !!demo.problem || !!d?.cutShort || !demo.playable}
          label={gameOpen ? "How to watch it" : demo.at.startsWith("server:") ? "Copy to my game and watch" : "Watch"}
          busyLabel={demo.at.startsWith("server:") ? "Copying…" : "Starting…"}
          onClick={() => acts.play(demo)}
        />
        {d?.canReplay && <BusyButton className="btn" busy={mine === "replay"} disabled={off} label="Show my path in Replay" busyLabel="Reading the demo…" onClick={() => acts.replay(demo)} />}
      </div>
      {command && (
        <div className="dm-command" role="status">
          <span>The game is open. Open its console (the ~ key) and type:</span>
          <code>{command}</code>
          <button className="btn small" onClick={copy}>
            Copy
          </button>
        </div>
      )}
      {!demo.playable && !demo.error && (
        <div className="dm-note">
          <span className="muted small-note">The game can only be told to play a demo named with plain letters, numbers and - _ . ( ) [ ].</span>
          {demo.safeName && <BusyButton className="btn small" busy={mine === "rename"} disabled={off} label={`Rename to ${demo.safeName}`} busyLabel="Renaming…" onClick={() => acts.renameTo(demo, demo.safeName!)} />}
        </div>
      )}

      <dl className="dm-facts">
        <dt>Length</dt>
        <dd>{length(demo.seconds) || "—"}</dd>
        <dt>Recorded</dt>
        <dd>{full(demo.modified)}</dd>
        <dt>Size</dt>
        <dd>{mb(demo.size)}</dd>
        <dt>Where</dt>
        <dd>{demo.where}</dd>
        {d?.server && (
          <>
            <dt>Server</dt>
            <dd>{d.server}</dd>
          </>
        )}
        {d?.maxPlayers ? (
          <>
            <dt>Slots</dt>
            <dd>{d.maxPlayers}</dd>
          </>
        ) : null}
        {(demo.error || demo.problem) && (
          <>
            <dt>Problem</dt>
            <dd className="dm-bad">{demo.error || demo.problem}</dd>
          </>
        )}
      </dl>
      {detail.isFetching && !d && <Loading text="Looking inside…" />}
      {detail.isError && <p className="dm-bad">{errorText(detail.error, "Couldn't look inside this demo.")}</p>}
      {d && d.names.length > 0 && (
        <div className="dm-players">
          <span className="dm-label">
            Players seen ({d.names.length}
            {d.names.length >= 64 ? ", the first 64" : ""})
          </span>
          <ul>
            {d.names.map((n, i) => (
              <li key={`${n}-${i}`}>{n}</li>
            ))}
          </ul>
        </div>
      )}
      {d?.problem && d.problem !== demo.problem && <p className="muted small-note">{d.problem}</p>}

      <div className="dm-panel-more">
        <BusyButton className="btn small" busy={mine === "rename"} disabled={off} label="Rename" busyLabel="Renaming…" onClick={() => acts.rename(demo)} />
        <BusyButton className="btn small" busy={mine === "reveal"} disabled={off} label={`Show in ${FOLDER}`} busyLabel="Opening…" onClick={() => acts.reveal(demo)} />
        <BusyButton className="btn small danger" busy={mine === "delete"} disabled={off} label="Delete" busyLabel="Deleting…" onClick={() => acts.remove(demo)} />
      </div>
    </aside>
  );
}
