import { Suspense, useEffect, useRef, useState } from "react";
import { create } from "zustand";
import { getJson, ON_THIS_COMPUTER, postJson, useActions, useOnce, usePoll } from "../api";
import { reloadServers } from "../queries";
import { useDirty, useDirtyStore } from "../store";
import type { AppInfo, MapInfo, Online, ServerInfo, ServerList } from "../types";
import { HostingGuide, SizingTable, troubleFor } from "./HostingGuide";
import { JobProgress } from "./JobProgress";
import { BusyButton, Info, lazyPage, LoadError, PageBoundary, SectionHead, Sheet, Skeleton, useConfirm, useToast } from "../ui";
import { Admins } from "./Admins";
import { Bots } from "./Bots";
import { Friends } from "./Friends";
import { GetMore } from "./GetMore";
import { Maps } from "./Maps";
import { Mods } from "./Mods";
import { PlayLive } from "./Play";
import { Players } from "./Players";
import { MatchButtons, SettingsPage } from "./Settings";
import type { Scan } from "./Setup";
import { Community } from "./Community";
import { ServerExtras } from "./ServerExtras";
import { Sounds } from "./Sounds";
import { Welcome } from "./Welcome";

// only servers that use it open this tab
const Hltv = lazyPage(() => import("./Hltv").then((m) => ({ default: m.Hltv })));

const GAME = { cstrike: "Counter-Strike 1.6", czero: "Condition Zero" } as const;
const VISIBILITY = [
  { id: "lan", name: "This network", note: "Only people on your Wi-Fi or LAN" },
  { id: "unlisted", name: "Anyone with the address", note: "Not in Steam's server list" },
  { id: "listed", name: "Listed on Steam", note: "Shows up in the Internet tab of Find Servers" },
] as const;
const STATE = { running: "Running", stopped: "Stopped", crashed: "Stopped (crashed)", "setting-up": "Setting up", failed: "Not finished", checking: "Checking…" };

/** What a phone is told when it tries something only the computer can do. */
export const ON_COMPUTER_ONLY = "Change this from the computer running Loner-Strike.";

type Reply = { ok?: boolean; error?: string; job?: string; id?: string; path?: string; note?: string | null; admins?: number; loner?: boolean; dir?: string | null; lines?: string[]; reply?: string; need_restart_game?: boolean; restart?: boolean; password?: string };
const post = (path: string, body: unknown = {}) =>
  postJson<Reply>(path, body)
    .then((r) => (r.error === "forbidden" ? { ...r, error: ON_COMPUTER_ONLY } : r))
    .catch((): Reply => ({ error: "Couldn't reach the app." }));

function uptime(s: number) {
  if (s < 60) return "just started";
  if (s < 3600) return `up ${Math.floor(s / 60)} min`;
  return `up ${Math.floor(s / 3600)} h ${Math.floor((s % 3600) / 60)} min`;
}

/** The one question before deleting (or removing) a server: [question, button]. */
function deleteQuestion(s: ServerInfo): [string, string] {
  if (s.imported) return [`Remove "${s.name}" from Loner-Strike? Its folder stays where it is.`, "Remove from Loner-Strike"];
  if (s.host) return [`Delete "${s.name}"? Stops it and deletes its folder on ${s.host}. The machine itself keeps running (and costing money) at your provider until you delete it there.`, "Delete the server"];
  return [`Delete "${s.name}"? Its folder, maps, settings and admins are deleted. Your own game isn't touched.`, "Delete the server"];
}
const wipeQuestion = (s: ServerInfo) =>
  `Rebuild "${s.name}" from scratch? Everything in its folder on ${s.host} is deleted (its settings, admins, bans, stats and any maps added to it) and it's set up again like when you made it. Takes 5 to 15 minutes.`;

// ---- start options saved while running: used after a restart -----------------------------

/** Server id: its uptime when the new start options were saved. Cleared once it restarts. */
const useRestartNeeded = create<{ at: Record<string, number>; mark: (id: string, up: number) => void; clear: (id: string) => void }>((set) => ({
  at: {},
  mark: (id, up) => set((s) => ({ at: { ...s.at, [id]: up } })),
  clear: (id) => set((s) => {
    if (!(id in s.at)) return s;
    const at = { ...s.at };
    delete at[id];
    return { at };
  }),
}));

const ACTION_LABEL: Record<string, string> = { start: "Starting", stop: "Stopping", restart: "Restarting", join: "Joining", retry: "Setting up", rebuild: "Rebuilding", delete: "Deleting", update: "Updating" };

/** An unlisted server can't be joined from a game that's already open (it doesn't answer
 *  the game's lookups), so the game is closed and started again with the address. */
function useJoinRestart() {
  const ask = useConfirm();
  const toast = useToast();
  return async (s: ServerInfo) => {
    // closing the game is for the person at the computer
    if (!ON_THIS_COMPUTER) {
      toast(`${s.name} is unlisted, so the game has to start with its address. Close Counter-Strike on the computer, then press Join.`, true);
      return null;
    }
    if (!(await ask(`Counter-Strike is open. Restart it and join ${s.name}? Unlisted servers can only be joined as the game starts.`, { ok: "Restart and join" }))) return null;
    return post(`/api/servers/${s.id}/join`, { restart: true });
  };
}

/** Running, Stopped... or that the machine it's on didn't answer. */
function StateTag({ s, inline }: { s: ServerInfo; inline?: boolean }) {
  if (s.unreachable) return <span className={`live off-line${inline ? " inline" : ""}`}>Can't reach {s.host}</span>;
  return (
    <span className={`live${inline ? " inline" : ""}${s.state === "running" ? " on" : ""}`}>
      {STATE[s.state]}
      {s.state === "running" ? ` · ${uptime(s.uptime)}` : ""}
    </span>
  );
}

/** A server on a machine that didn't answer: why, the fix if one fits, and asking again. */
function Unreachable({ s, onGuide }: { s: ServerInfo; onGuide?: () => void }) {
  const { run, busy } = useActions();
  const tip = troubleFor(s.unreachable);
  return (
    <div className="unreachable" role="status">
      <p className="err-text">{s.unreachable}</p>
      {tip && <p className="small-note"><b>{tip.q}.</b> <span className="muted">{tip.a}</span></p>}
      <div className="row">
        <BusyButton className="btn small" busy={busy(`${s.id}:recheck`)} label="Try again" busyLabel="Checking…" onClick={() => run(`${s.id}:recheck`, () => reloadServers(), `Checking ${s.host}…`)} />
        {onGuide ? (
          <button className="btn small" onClick={onGuide}>Troubleshooting</button>
        ) : (
          ON_THIS_COMPUTER && <span className="muted small-note">More fixes: All servers, Setting up a Linux server, Troubleshooting.</span>
        )}
      </div>
    </div>
  );
}

/** Every server, and making or adding one. Opening a server shows its own page. */
export function Servers({ scan, list, loadError, onOpen, onChanged }: { scan: Scan | null; list: ServerList | null; loadError?: boolean; onOpen: (id: string) => void; onChanged: () => void }) {
  const ask = useConfirm();
  const toast = useToast();
  const [creating, setCreating] = useState(false);
  const [importing, setImporting] = useState(false);
  const guide = useRef<HTMLDetailsElement>(null);
  const openGuide = () => {
    if (guide.current) guide.current.open = true;
    requestAnimationFrame(() => document.getElementById("hosting-trouble")?.scrollIntoView({ behavior: "smooth", block: "start" }));
  };

  const { run: once, busy } = useActions();
  const joinRestart = useJoinRestart();
  const act = (s: ServerInfo, action: string) => once(`${s.id}:${action}`, async () => {
    if (action === "delete") {
      const [q, ok] = deleteQuestion(s);
      if (!(await ask(q, { ok, danger: true }))) return;
    }
    if (action === "rebuild" && !(await ask(wipeQuestion(s), { ok: "Wipe and rebuild", danger: true }))) return;
    // the list doesn't know who's on, so stopping always asks
    if (action === "stop" && !(await ask(`Stop ${s.name}? Anyone playing is disconnected.`, { ok: "Stop the server", danger: true }))) return;
    let r: Reply | null = await post(`/api/servers/${s.id}/${action}`);
    if (r.need_restart_game) r = await joinRestart(s);
    if (!r) return;
    if (r.error) return toast(r.error, true);
    if (action === "retry") toast("Trying again. It carries on from where it got to.");
    if (action === "rebuild") toast(`Rebuilding on ${s.host}. It takes 5 to 15 minutes.`);
    if (action === "join") toast(`Joining ${s.name}…`);
    if (action === "delete") toast(r.note || `${s.name} ${s.imported ? "removed" : "deleted"}`, !!r.note);
    onChanged();
  }, `${ACTION_LABEL[action] ?? "Working on"} ${s.name}…`);
  const installed = (scan?.installs ?? []).flatMap((i) => i.games.filter((g) => g.installed).map((g) => g.id));

  return (
    <div className="stack">
      <SectionHead title="Servers" note="Optional: run dedicated servers that keep going without your game open, for friends or anyone. Each has its own modes, maps, admins and password, separate from your game.">
        {ON_THIS_COMPUTER && <button className="btn" onClick={() => setImporting(true)}>Add one you already run</button>}
        {ON_THIS_COMPUTER && <button className="btn primary" onClick={() => setCreating(true)}>New server</button>}
      </SectionHead>

      {!list && (loadError ? <LoadError text="Couldn't load your servers." onRetry={() => void reloadServers()} /> : <Skeleton rows={4} />)}

      {list && list.servers.length === 0 && (
        <div className="panel">
          <h3>No servers yet</h3>
          <p className="muted measure">
            Your own game already hosts matches for you and friends (see Play with friends). A dedicated server is for when you want one that keeps
            running while you're not playing. Loner-Strike downloads Valve's free server (about 1 GB), sets it up and runs it for you.
            {!ON_THIS_COMPUTER && " Make one from the computer running Loner-Strike."}
          </p>
        </div>
      )}

      <div className="server-list">
        {list?.servers.map((s) => (
          <div key={s.id} className="server">
            <div className="server-head">
              <div>
                <h3>{s.name}</h3>
                <p className="muted">
                  {GAME[s.game]} · {s.host ? `on ${s.host}` : `port ${s.port}`} · {VISIBILITY.find((v) => v.id === s.visibility)?.name}
                  {s.imported ? " · added from your own folder" : ""}
                </p>
              </div>
              <StateTag s={s} />
            </div>
            {s.error && <p className="err-text">{s.error}</p>}
            {s.unreachable && s.state !== "setting-up" && <Unreachable s={s} onGuide={ON_THIS_COMPUTER ? openGuide : undefined} />}
            {s.state === "setting-up" ? (
              <JobProgress remote={!!s.host} onEnd={(ok) => { toast(ok ? `${s.name} is ready` : `Setting up ${s.name} stopped. It says why below.`, !ok); onChanged(); }} />
            ) : s.state === "failed" ? (
              ON_THIS_COMPUTER ? (
                <div className="row">
                  {s.host && <BusyButton className="btn primary" busy={busy(`${s.id}:retry`)} label="Try again" busyLabel="Starting…" onClick={() => act(s, "retry")} />}
                  {s.host && <BusyButton busy={busy(`${s.id}:rebuild`)} label="Rebuild from scratch" busyLabel="Starting…" onClick={() => act(s, "rebuild")} />}
                  <button className="btn danger" onClick={() => act(s, "delete")}>{s.imported ? "Remove" : "Delete"}</button>
                </div>
              ) : (
                <p className="muted small-note">{ON_COMPUTER_ONLY}</p>
              )
            ) : (
              <div className="row">
                <button className="btn primary" onClick={() => onOpen(s.id)}>Open</button>
                {s.state === "running" ? (
                  <>
                    <BusyButton busy={busy(`${s.id}:join`)} label="Join" busyLabel="Joining…" onClick={() => act(s, "join")} />
                    <BusyButton className="btn danger" busy={busy(`${s.id}:stop`)} label="Stop" busyLabel="Stopping…" onClick={() => act(s, "stop")} />
                  </>
                ) : (
                  <BusyButton busy={busy(`${s.id}:start`)} disabled={!!s.unreachable} label="Start" busyLabel="Starting…" onClick={() => act(s, "start")} />
                )}
                {ON_THIS_COMPUTER && <button className="btn danger push" onClick={() => act(s, "delete")}>{s.imported ? "Remove" : "Delete"}</button>}
              </div>
            )}
          </div>
        ))}
      </div>

      <WhereToHost />
      {ON_THIS_COMPUTER && (
        <details className="where-to-host" ref={guide}>
          <summary><h2>Setting up a Linux server</h2><span className="muted">Step by step, from renting a machine to the first game</span></summary>
          <HostingGuide />
        </details>
      )}

      {creating && (
        <NewServer
          servers={list?.servers ?? []}
          games={installed}
          onClose={() => setCreating(false)}
          onCreated={() => { setCreating(false); onChanged(); }}
        />
      )}
      {importing && <ImportServer onClose={() => setImporting(false)} onDone={() => { setImporting(false); onChanged(); }} />}
    </div>
  );
}

type Tab = "community" | "hltv" | "extras" | "sounds" | "overview" | "friends" | "modes" | "maps" | "getmaps" | "bots" | "match" | "fun" | "players" | "mods" | "admins" | "welcome";
const TABS: { id: Tab; label: string; group: string; live?: boolean }[] = [
  { id: "overview", label: "Overview", group: "Server" },
  { id: "friends", label: "Play with friends", group: "Server" },
  { id: "extras", label: "Rotation & messages", group: "Server" },
  { id: "hltv", label: "Spectators & demos", group: "Server" },
  { id: "modes", label: "Modes", group: "Game", live: true },
  { id: "maps", label: "Maps", group: "Game" },
  { id: "getmaps", label: "Get maps", group: "Game" },
  { id: "bots", label: "Bots", group: "Game", live: true },
  { id: "match", label: "Match", group: "Game", live: true },
  { id: "fun", label: "Fun", group: "Game", live: true },
  { id: "players", label: "Players", group: "People", live: true },
  { id: "admins", label: "Admins", group: "People" },
  { id: "community", label: "Stats & moderation", group: "People" },
  { id: "welcome", label: "Welcome page", group: "People" },
  { id: "mods", label: "Mods", group: "Content" },
  { id: "sounds", label: "Sounds", group: "Content" },
];
const TAB_GROUPS = [...new Set(TABS.map((t) => t.group))];

/** The tab strip; on a phone it scrolls sideways, with faded edges where more tabs are. */
function TabStrip({ label, tab, onPick }: { label: string; tab: Tab; onPick: (t: Tab) => void }) {
  const box = useRef<HTMLDivElement>(null);
  const [edges, setEdges] = useState({ left: false, right: false });
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const fit = () => setEdges({ left: el.scrollLeft > 2, right: el.scrollLeft + el.clientWidth < el.scrollWidth - 2 });
    fit();
    el.addEventListener("scroll", fit, { passive: true });
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => {
      el.removeEventListener("scroll", fit);
      ro.disconnect();
    };
  }, []);
  return (
    <div ref={box} className={`server-tabs${edges.left ? " more-left" : ""}${edges.right ? " more-right" : ""}`} role="tablist" aria-label={label}>
      {TAB_GROUPS.map((g) => (
        <div key={g} className="tab-group">
          <span className="tab-group-name">{g}</span>
          <div className="chips">
            {TABS.filter((t) => t.group === g).map((t) => (
              <button key={t.id} role="tab" className="chip" aria-selected={tab === t.id} onClick={() => onPick(t.id)}>{t.label}</button>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

type RunningJob = { title: string; done: string; retry?: () => void };

/** One server's own control panel. Every request from here names the server. */
export function ServerPage(props: {
  server: ServerInfo;
  others: ServerInfo[];
  data: Online | null;
  reason: string;
  app: AppInfo | null;
  scan: Scan | null;
  maps: MapInfo[];
  refresh: () => void;
  reloadMaps: () => void;
  reloadApp: () => void;
  onChanged: () => void;
  onBack: () => void;
}) {
  const ask = useConfirm();
  const { server: s, others, data, reason, app, scan, maps, refresh, reloadMaps, reloadApp, onChanged, onBack } = props;
  const toast = useToast();
  const [tab, setTab] = useState<Tab>("overview");
  const [job, setJob] = useState<RunningJob | null>(null);
  const running = s.state === "running";
  const off = !!s.unreachable;

  // unsaved edits on a tab: asked about before they're thrown away
  const dirty = useDirtyStore((st) => Object.values(st.dirty).some(Boolean));
  const leaveOk = async () => !dirty || (await ask("Your changes here aren't saved. Leave without saving them?", { ok: "Leave without saving", danger: true }));
  const pick = async (t: Tab) => {
    if (t !== tab && (await leaveOk())) setTab(t);
  };
  const back = async () => {
    if (await leaveOk()) onBack();
  };

  // new start options wait for a restart: until the uptime starts again, or it stops
  const restartAt = useRestartNeeded((st) => st.at[s.id]);
  const clearRestart = useRestartNeeded((st) => st.clear);
  useEffect(() => {
    if (restartAt !== undefined && (!running || s.uptime < restartAt)) clearRestart(s.id);
  }, [restartAt, running, s.uptime, s.id, clearRestart]);

  const { run: once, busy: busyKey } = useActions();
  const joinRestart = useJoinRestart();
  // same keys as the server list, so a spinner follows the server, not the page
  const busy = (action: string) => busyKey(`${s.id}:${action}`);
  const humans = data?.state.humans ?? 0;
  const act = (action: string) => once(`${s.id}:${action}`, async () => {
    if (action === "delete") {
      const [q, ok] = deleteQuestion(s);
      if (!(await ask(q, { ok, danger: true }))) return;
    }
    if (action === "stop" || action === "restart") {
      const verb = action === "stop" ? "Stop" : "Restart";
      // who's on comes from the live connection; without one, stopping asks anyway
      const q = humans > 0
        ? `${humans} ${humans === 1 ? "person is" : "people are"} playing on ${s.name}. ${verb} it? They're disconnected.`
        : action === "stop" && !data ? `Stop ${s.name}? Anyone playing is disconnected.` : null;
      if (q && !(await ask(q, { ok: `${verb} the server`, danger: true }))) return;
    }
    let r: Reply | null = await post(`/api/servers/${s.id}/${action}`);
    if (r.need_restart_game) r = await joinRestart(s);
    if (!r) return;
    if (r.error) return toast(r.error, true);
    if (action === "join") toast(`Joining ${s.name}…`);
    if (action === "start") toast(`Starting ${s.name}…`);
    if (action === "restart") clearRestart(s.id);
    if (action === "update" && r.job) setJob({ title: "Updating Valve's server files…", done: "Valve's server files are up to date", retry: () => act("update") });
    if (action === "delete") return onBack(), onChanged();
    onChanged();
    setTimeout(refresh, 1500);
  }, `${ACTION_LABEL[action] ?? "Working on"} ${s.name}…`);

  // reinstall everything that's on the server, keeping its settings
  const rebuild = async () => {
    if (!(await ask(`Rebuild Loner-Strike on ${s.name}? It reinstalls its files; settings, admins and maps stay.`, { ok: "Rebuild Loner-Strike" }))) return;
    once(`${s.id}:reinstall`, reinstall, `Rebuilding Loner-Strike on ${s.name}…`);
  };
  const reinstall = async () => {
    const components = (app?.game?.installed ?? []).filter((c) => c !== "foundation" && c !== "core");
    const r = await post("/api/setup/install", { hl: s.dir, games: [s.game], components, repair: true });
    if (r.error) return toast(r.error, true);
    setJob({ title: "Rebuilding Loner-Strike…", done: `Loner-Strike is rebuilt on ${s.name}`, retry: reinstall });
  };
  // a server on another machine: wipe it and set it up again with the same details
  // (same key as the server list's Rebuild, so either shows it's under way)
  const wipeRebuild = async () => {
    if (!(await ask(wipeQuestion(s), { ok: "Wipe and rebuild", danger: true }))) return;
    await once(`${s.id}:rebuild`, async () => {
    const r = await post(`/api/servers/${s.id}/rebuild`);
    if (r.error) return toast(r.error, true);
    toast(`Rebuilding on ${s.host}. It takes 5 to 15 minutes.`);
    onChanged();
    onBack();
    }, `Wiping ${s.name}…`);
  };

  // a live tab waits for the server's answer; after a few seconds it says why it's waiting
  const needsLive = TABS.find((t) => t.id === tab)?.live && !data;
  const [slow, setSlow] = useState(false);
  const [tries, setTries] = useState(0);
  useEffect(() => {
    setSlow(false);
    if (!needsLive || !running) return;
    const t = setTimeout(() => setSlow(true), 4000);
    return () => clearTimeout(t);
  }, [needsLive, running, tab, tries]);

  const restartBar = running && !off && (s.pending_restart || restartAt !== undefined);
  return (
    <div className="stack">
      <div className="server-top">
        <button className="link back" onClick={back}>All servers</button>
        <div className="row between">
          <div>
            <h2 className="server-title">{s.name}</h2>
            <p className="muted">
              {s.host ? `Server on ${s.host}` : "Dedicated server"} · {GAME[s.game]} · {s.host ? s.address : `port ${s.port}`} · <StateTag s={s} inline />
            </p>
          </div>
          <div className="row">
            {running ? (
              <>
                <BusyButton className="btn primary" busy={busy("join")} label="Join" busyLabel="Joining…" onClick={() => act("join")} />
                <BusyButton busy={busy("restart")} label="Restart" busyLabel="Restarting…" onClick={() => act("restart")} />
                <BusyButton className="btn danger" busy={busy("stop")} label="Stop" busyLabel="Stopping…" onClick={() => act("stop")} />
              </>
            ) : (
              <BusyButton className="btn primary" busy={busy("start")} disabled={off} label="Start the server" busyLabel="Starting…" onClick={() => act("start")} />
            )}
          </div>
        </div>
        <TabStrip label={`${s.name} sections`} tab={tab} onPick={pick} />
      </div>

      {restartBar && (
        <div className="files-bar server-restart-bar" role="status">
          <span>
            {s.pending_restart && restartAt !== undefined
              ? "New files and start options are saved. They're used after a restart."
              : s.pending_restart
                ? "New files are on the server. They're used after a restart (or the next map)."
                : "New start options are saved. They're used after a restart."}
          </span>
          <span className="files-bar-actions">
            <BusyButton className="btn small primary" busy={busy("restart")} label="Restart now" busyLabel="Restarting…" onClick={() => act("restart")} />
          </span>
        </div>
      )}
      {off && <Unreachable s={s} />}
      {s.error && <p className="err-text">{s.error}</p>}
      {!s.loner && <AddLoner server={s} onDone={() => { onChanged(); reloadApp(); }} />}
      {job && (
        <JobProgress
          title={job.title}
          remote={!!s.host}
          onEnd={(ok) => { if (ok) setJob(null); toast(ok ? job.done : "That stopped. The bar shows why.", !ok); reloadApp(); onChanged(); }}
          onClose={() => setJob(null)}
          onRetry={job.retry && (() => { const again = job.retry!; setJob(null); again(); })}
        />
      )}

      {needsLive ? (
        <div className="waiting">
          <h2>{TABS.find((t) => t.id === tab)!.label}</h2>
          {!running ? (
            <>
              <p className="muted">{off ? `This needs the server running, and ${s.host} isn't answering.` : "This needs the server running."}</p>
              <BusyButton className="btn primary" busy={busy("start")} disabled={off} label="Start the server" busyLabel="Starting…" onClick={() => act("start")} />
            </>
          ) : !slow ? (
            <p className="muted"><span className="spinner" aria-hidden="true" />Connecting to the server…</p>
          ) : (
            <>
              <p className="muted">{reason || "The server isn't answering yet."}</p>
              <div className="row">
                <button className="btn primary" onClick={() => { refresh(); setTries((n) => n + 1); }}>Try again</button>
                <button className="btn" onClick={() => setTab("overview")}>Open the console</button>
              </div>
            </>
          )}
        </div>
      ) : (
        <>
          {tab === "overview" && (
            <>
              <ServerDetails server={s} maps={maps} onChanged={onChanged} />
              {ON_THIS_COMPUTER && <CopyMods server={s} others={others} onStarted={(title) => setJob({ title, done: `Mods copied to ${s.name}` })} />}
              {ON_THIS_COMPUTER && <Maintenance server={s} installed={app?.game?.installed ?? []} act={act} rebuild={rebuild} wipeRebuild={wipeRebuild} busy={busy} />}
            </>
          )}
          {tab === "friends" && <Friends data={data} refresh={refresh} server={{ name: s.name, visibility: s.visibility }} />}
          {tab === "modes" && data && <PlayLive data={data} maps={maps} refresh={refresh} openMaps={() => setTab("maps")} />}
          {tab === "maps" && <Maps current={data?.state.map} live={!!data} maps={maps} reload={reloadMaps} refresh={refresh} />}
          {tab === "getmaps" && <GetMore only="maps" onInstalled={reloadMaps} />}
          {tab === "bots" && data && <Bots data={data} refresh={refresh} />}
          {tab === "match" && data && (
            <SettingsPage title="Match" note="Saved for this mode on this server." data={data} cat={2} refresh={refresh}>
              <MatchButtons refresh={refresh} />
            </SettingsPage>
          )}
          {tab === "fun" && data && <SettingsPage title="Fun & effects" note="These apply immediately." data={data} cat={3} refresh={refresh} />}
          {tab === "players" && data && <Players data={data} refresh={refresh} />}
          {tab === "mods" && <Mods app={app} scan={scan} running={!!data} onChanged={() => { reloadApp(); onChanged(); }} openSetup={() => {}} />}
          {tab === "admins" && <Admins data={data} />}
          {tab === "welcome" && <Welcome target={s.id} />}
          {tab === "sounds" && <Sounds target={s.id} />}
          {tab === "extras" && <ServerExtras target={s.id} />}
          {tab === "hltv" && <PageBoundary><Suspense fallback={<Skeleton rows={4} />}><Hltv server={s} /></Suspense></PageBoundary>}
          {tab === "community" && <Community target={s.id} />}
        </>
      )}
    </div>
  );
}

/** For an imported server without Loner-Strike: put the basics on it. */
function AddLoner({ server, onDone }: { server: ServerInfo; onDone: () => void }) {
  const ask = useConfirm();
  const toast = useToast();
  const [running, setRunning] = useState(false);
  const install = async () => {
    const r = await post("/api/setup/install", { hl: server.dir, games: [server.game], components: [] });
    if (r.error) return toast(r.error, true);
    setRunning(true);
  };
  const start = async () => {
    if (!(await ask("Add Loner-Strike to this server? It installs Metamod-P, AMX Mod X, ReGameDLL and the Game Mode Menu. Every file it replaces is backed up, and removing it in Mods puts them back.", { ok: "Add Loner-Strike" }))) return;
    install();
  };
  if (!ON_THIS_COMPUTER) return null;
  return running ? (
    <JobProgress
      title="Adding Loner-Strike…"
      onEnd={(ok) => { if (ok) setRunning(false); toast(ok ? "Loner-Strike is on the server" : "Adding Loner-Strike stopped. The bar shows why.", !ok); onDone(); }}
      onClose={() => setRunning(false)}
      onRetry={() => { setRunning(false); install(); }}
    />
  ) : (
    <div className="banner">
      <div>This server doesn't have Loner-Strike yet, so the panel can only start, stop and manage admins. Add it to get modes, bots and the rest.</div>
      <button className="btn primary" onClick={start}>Add Loner-Strike</button>
    </div>
  );
}

/** A number field that keeps what's typed (even empty) until Save checks it. */
function useNumber(initial: number) {
  const [text, setText] = useState(String(initial));
  return { text, setText, value: text.trim() === "" ? NaN : Number(text) };
}
/** Why a number is out of its range, or "" when it's fine. */
const numberProblem = (v: number, min: number, max: number, what: string) =>
  !Number.isInteger(v) || v < min || v > max ? `${what}: a whole number from ${min} to ${max}.` : "";
const clampText = (text: string, min: number, max: number) => {
  const v = Math.round(Number(text));
  return text.trim() === "" || isNaN(v) ? text : String(Math.min(max, Math.max(min, v)));
};

function ServerDetails({ server, maps, onChanged }: { server: ServerInfo; maps: MapInfo[]; onChanged: () => void }) {
  const toast = useToast();
  const mark = useRestartNeeded((st) => st.mark);
  // a phone sees the settings but changes them from the computer
  const ro = !ON_THIS_COMPUTER;
  const [name, setName] = useState(server.name);
  const [map, setMap] = useState(server.map);
  const slots = useNumber(server.maxplayers);
  const port = useNumber(server.port);
  const [vis, setVis] = useState(server.visibility);
  const [password, setPassword] = useState(server.password ?? "");
  const [listenIp, setListenIp] = useState(server.listen_ip ?? "");
  const [extra, setExtra] = useState(server.extra_args ?? "");
  const [problem, setProblem] = useState<{ slots?: string; port?: string }>({});
  const portMin = server.host ? 27015 : 1024;
  const portMax = server.host ? 27019 : 65000;

  // what was last saved, to tell whether anything changed since
  const now = JSON.stringify([name, map, slots.text, port.text, vis, password, listenIp, extra]);
  const [base, setBase] = useState(now);
  const dirty = !ro && now !== base;
  useDirty("server-settings", dirty);

  const { run, busy } = useActions();
  const saving = busy(`${server.id}:save`);
  const save = () => {
    const bad = { slots: numberProblem(slots.value, 2, 32, "Player slots"), port: numberProblem(port.value, portMin, portMax, "Port") };
    setProblem(bad);
    if (bad.slots || bad.port) return;
    const sent = now;
    run(`${server.id}:save`, async () => {
      const r = await post(`/api/servers/${server.id}`, { name, map, maxplayers: slots.value, port: port.value, visibility: vis, password, listen_ip: listenIp, extra_args: extra });
      if (r.error) return toast(r.error, true);
      setBase(sent);
      // (an uptime of 0 means it's unknown: a restart couldn't be told apart, so no bar)
      if (r.restart && server.uptime > 0) mark(server.id, server.uptime);
      toast(r.restart ? `${name} saved. Restart it to use the new start options.` : `${name} saved`);
      onChanged();
    }, server.host ? `Saving and uploading to ${server.name}…` : `Saving ${server.name}…`);
  };

  // the maps on the server, and the current one even if it isn't among them
  const mapNames = [...new Set([...maps.map((m) => m.name), map].filter(Boolean))].sort((a, b) => a.localeCompare(b));
  return (
    <div className="server-details">
      <div className="panel">
        <h3>Settings</h3>
        {ro && <p className="muted small-note">{ON_COMPUTER_ONLY}</p>}
        <label className="field">
          <span>Name</span>
          <input className="search" disabled={ro} maxLength={48} value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <div className="field">
          <span>Who can join<Info text="Anyone with the address suits friends: they join with the address you send them, but strangers can't find it. Listed on Steam puts it in everyone's Internet server list." /></span>
          <div className="seg" role="group" aria-label="Who can join">
            {VISIBILITY.map((v) => (
              <button key={v.id} type="button" disabled={ro} aria-pressed={vis === v.id} onClick={() => setVis(v.id)}>{v.name}</button>
            ))}
          </div>
          <small>{VISIBILITY.find((v) => v.id === vis)?.note}. {server.host ? "Friends join with the address shown above: see Play with friends." : "Friends outside your network also need a way in: see Play with friends."}</small>
        </div>
        <label className="field">
          <span>Join password<Info text="Players type this to get in. Leave it empty for no password. It takes effect straight away, and Join in this app uses it for you." /></span>
          <input className="search" disabled={ro} maxLength={24} placeholder="None" value={password} onChange={(e) => setPassword(e.target.value.replace(/[^A-Za-z0-9_-]/g, ""))} spellCheck={false} autoComplete="off" />
          <small>Letters, numbers, - and _.</small>
        </label>
        <div className="row">
          <label className="field">
            <span>Starting map<Info text="The map the server loads when it starts. After that, the map rotation decides what comes next." /></span>
            {mapNames.length > 1 ? (
              <select disabled={ro} value={map} onChange={(e) => setMap(e.target.value)}>
                {mapNames.map((m) => <option key={m} value={m}>{m}</option>)}
              </select>
            ) : (
              <input className="search" disabled={ro} value={map} onChange={(e) => setMap(e.target.value.trim())} spellCheck={false} />
            )}
          </label>
          <label className="field">
            <span>Player slots<Info text="The most players the server holds at once, bots included. Each bot takes a slot, so leave room for people." /></span>
            <input
              className="search narrow" type="number" disabled={ro} min={2} max={32} value={slots.text}
              onChange={(e) => { slots.setText(e.target.value); setProblem((p) => ({ ...p, slots: "" })); }}
              onBlur={() => slots.setText(clampText(slots.text, 2, 32))}
              aria-invalid={!!problem.slots}
            />
          </label>
        </div>
        {problem.slots && <p className="err-text small-note">{problem.slots}</p>}
        <details className="advanced">
          <summary>Advanced</summary>
          <div className="stack-tight">
            <div className="row">
              <label className="field">
                <span>Port<Info text={server.host ? "The port players connect to. On a Linux server it's 27015 to 27019 (setup opened 27015-27020 on the machine; 27020 is for HLTV). Open it in your provider's firewall too." : "The network port the server listens on; each server needs its own, from 27016 up (your own game uses 27015). Change it only if something else uses it, and open the new port on your router if friends connect to you directly."} /></span>
                <input
                  className="search narrow" type="number" disabled={ro} min={portMin} max={portMax} value={port.text}
                  onChange={(e) => { port.setText(e.target.value); setProblem((p) => ({ ...p, port: "" })); }}
                  onBlur={() => port.setText(clampText(port.text, portMin, portMax))}
                  aria-invalid={!!problem.port}
                />
              </label>
              {!server.host && (
                <label className="field">
                  <span>Listen address<Info text="Leave empty to answer on every network this computer is on. Set it to one of this computer's addresses to use only that network." /></span>
                  <input className="search" disabled={ro} placeholder="All addresses" value={listenIp} onChange={(e) => setListenIp(e.target.value.trim())} spellCheck={false} />
                </label>
              )}
            </div>
            {problem.port && <p className="err-text small-note">{problem.port}</p>}
            <label className="field">
              <span>Extra start options<Info text="Added to the server's command line, like +sv_gravity 600 or -pingboost 2. Options the app sets itself (port, map, players, passwords) are changed above instead. Used from the next start." /></span>
              <input className="search" disabled={ro} placeholder="e.g. -pingboost 2 +sys_ticrate 1000" value={extra} onChange={(e) => setExtra(e.target.value)} spellCheck={false} />
            </label>
            {ON_THIS_COMPUTER && <ConsolePassword server={server} />}
          </div>
        </details>
        {!ro && (
          <div className="row">
            <BusyButton className="btn primary" busy={saving} label="Save" busyLabel={server.host ? "Saving and uploading…" : "Saving…"} onClick={save} />
            {dirty && !saving && <span className="muted small-note">Unsaved changes</span>}
          </div>
        )}
      </div>
      <Console server={server} />
    </div>
  );
}

/** The console (RCON) password, for HLSW and other server tools. */
function ConsolePassword({ server }: { server: ServerInfo }) {
  const toast = useToast();
  const [shown, setShown] = useState<string | null>(null);
  const [typed, setTyped] = useState("");
  const { run, busy } = useActions();
  const show = async () => {
    if (shown !== null) return setShown(null);
    const r = await getJson<Reply>(`/api/servers/${server.id}/rcon-password`).catch((): Reply => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    setShown(r.password ?? "");
  };
  const change = (password?: string) => run(`${server.id}:rcon`, async () => {
    const r = await postJson<Reply>(`/api/servers/${server.id}/rcon-password`, password ? { password } : {}).catch((): Reply => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    setShown(r.password ?? "");
    setTyped("");
    toast("Console password changed");
  }, `Changing ${server.name}'s console password…`);
  const changing = busy(`${server.id}:rcon`);
  return (
    <div className="field">
      <span>Console password<Info text="The RCON password, for running the server from other tools like HLSW. The app sets a random one and uses it itself. Changing it applies straight away." /></span>
      <div className="row">
        <code className="secret">{shown ?? "••••••••••••"}</code>
        <button className="btn small" type="button" onClick={show}>{shown === null ? "Show" : "Hide"}</button>
        <BusyButton className="btn small" busy={changing} label="New random one" busyLabel="Changing…" onClick={() => change()} />
      </div>
      <form className="row" onSubmit={(e) => { e.preventDefault(); if (typed.length >= 8) change(typed); }}>
        <input className="search" placeholder="Or type your own (8 to 32 characters)" maxLength={32} value={typed} onChange={(e) => setTyped(e.target.value.replace(/[^A-Za-z0-9_-]/g, ""))} spellCheck={false} autoComplete="off" />
        <BusyButton className="btn small" busy={changing} label="Set" busyLabel="Setting…" disabled={typed.length < 8} onClick={() => change(typed)} />
      </form>
    </div>
  );
}

/** Another server's modes and add-ons, installed on this one too (its settings stay). */
function CopyMods({ server, others, onStarted }: { server: ServerInfo; others: ServerInfo[]; onStarted: (title: string) => void }) {
  const toast = useToast();
  const sources = others.filter((o) => o.game === server.game && o.ready && o.loner);
  const [from, setFrom] = useState(sources[0]?.id ?? "");
  const { run, busy } = useActions();
  if (!server.loner || !sources.length) return null;
  const source = sources.find((o) => o.id === from) ?? sources[0];
  const have = new Set(server.installed ?? []);
  const missing = (source.installed ?? []).filter((c) => c !== "foundation" && c !== "core" && !have.has(c));
  const copy = () => run(`${server.id}:copy-mods`, async () => {
    if (!missing.length) return toast(`${server.name} already has everything ${source.name} has.`);
    const r = await post("/api/setup/install", { hl: server.dir, games: [server.game], components: missing });
    if (r.error) return toast(r.error, true);
    toast(`Installing ${missing.length} from ${source.name}…`);
    onStarted(`Copying ${missing.length} mod${missing.length === 1 ? "" : "s"} from ${source.name}…`);
  }, `Copying mods to ${server.name}…`);
  return (
    <div className="panel">
      <h3>Copy mods from another server</h3>
      <p className="muted measure">Installs the modes and add-ons another server has and this one doesn't. Nothing on this server is removed, and its settings stay.{server.host ? " New mods load after a restart." : ""}</p>
      <div className="row">
        <select value={source.id} onChange={(e) => setFrom(e.target.value)} aria-label="Copy from">
          {sources.map((o) => <option key={o.id} value={o.id}>{o.name}</option>)}
        </select>
        <span className="muted small-note">{missing.length ? `${missing.length} to add` : "Nothing to add"}</span>
        <BusyButton className="btn" busy={busy(`${server.id}:copy-mods`)} label="Copy mods" busyLabel="Starting…" disabled={!missing.length} onClick={copy} />
      </div>
    </div>
  );
}

/** Updating, rebuilding and deleting a server (on the computer running Loner-Strike). */
function Maintenance({ server, installed, act, rebuild, wipeRebuild, busy }: { server: ServerInfo; installed: string[]; act: (a: string) => void; rebuild: () => void; wipeRebuild: () => void; busy: (action: string) => boolean }) {
  const running = server.state === "running";
  return (
    <div className="panel">
      <h3>Maintenance</h3>
      {running && <p className="muted">Stop the server first to update or rebuild it.</p>}
      <ul className="upkeep">
        {!server.imported && (
          <li>
            <div><b>Update Valve's server files</b><p className="muted">Gets the latest version of the server from Valve. Your mods and settings stay.</p></div>
            <BusyButton className="btn" busy={busy("update")} disabled={running} label="Update" busyLabel="Starting…" onClick={() => act("update")} />
          </li>
        )}
        {server.loner && !server.host && (
          <li>
            <div><b>Rebuild Loner-Strike</b><p className="muted">Reinstalls Metamod, AMX Mod X, ReGameDLL{installed.length > 2 ? ` and the ${installed.length - 2} extra mode${installed.length === 3 ? "" : "s"} and add-ons on it` : " and the built-in modes"}. Fixes a server that won't start or lost its mods. Settings, admins and maps stay.</p></div>
            <BusyButton className="btn" busy={busy("reinstall")} disabled={running} label="Rebuild" busyLabel="Starting…" onClick={rebuild} />
          </li>
        )}
        {server.host && (
          <li>
            <div><b>Wipe and rebuild</b><p className="muted">Deletes everything in its folder on {server.host} (settings, admins, bans, stats and added maps) and sets it up again like when you made it. For a server that won't start or is badly broken. Takes 5 to 15 minutes.</p></div>
            <BusyButton className="btn danger" busy={busy("rebuild")} disabled={running} label="Wipe and rebuild" busyLabel="Wiping…" onClick={wipeRebuild} />
          </li>
        )}
        <li>
          <div>
            <b>{server.imported ? "Remove from Loner-Strike" : "Delete the server"}</b>
            <p className="muted">
              {server.imported
                ? "Takes it off the list. Its folder stays where it is."
                : server.host
                  ? `Stops it and deletes its folder on ${server.host}. The machine itself keeps running (and costing money) until you delete it at your provider.`
                  : "Deletes its folder, maps, settings and admins. Your own game isn't touched."}
            </p>
          </div>
          <BusyButton className="btn danger" busy={busy("delete")} label={server.imported ? "Remove" : "Delete"} busyLabel={server.imported ? "Removing…" : "Deleting…"} onClick={() => act("delete")} />
        </li>
      </ul>
    </div>
  );
}

function Console({ server }: { server: ServerInfo }) {
  const toast = useToast();
  const [lines, setLines] = useState<string[]>([]);
  // commands sent from here and their replies: kept after the log, which each poll replaces
  const [sent, setSent] = useState<string[]>([]);
  const [failed, setFailed] = useState(false);
  const [cmd, setCmd] = useState("");
  const box = useRef<HTMLPreElement>(null);
  // off: the console stays where you scrolled it while new lines come in
  const [follow, setFollow] = useState(true);
  usePoll(() =>
    getJson<Reply>(`/api/servers/${server.id}/console`)
      .then((r) => {
        if (r.error && !r.lines) return setFailed(true);
        setFailed(false);
        setLines(r.lines ?? []);
      })
      .catch(() => setFailed(true)), 2000, [server.id]);
  const shown = [...lines, ...sent];
  useEffect(() => {
    if (follow) box.current?.scrollTo({ top: box.current.scrollHeight });
  }, [lines, sent, follow]);
  const once = useOnce();
  const send = () => once("send", async () => {
    const c = cmd.trim();
    if (!c || server.state !== "running") return;
    const r = await post(`/api/servers/${server.id}/console/send`, { command: c });
    if (r.error) return toast(r.error === ON_COMPUTER_ONLY || r.error.includes("Couldn't reach") ? "The console only works on this computer." : r.error, true);
    setCmd("");
    setSent((l) => [...l, `] ${c}`, ...(r.reply?.trim() ? r.reply.trim().split("\n") : [])].slice(-200));
  });
  return (
    <div className="panel">
      <div className="row between">
        <h3>Console</h3>
        <label className="follow-toggle small-note">
          <input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} /> Follow new lines
        </label>
      </div>
      <pre className="console" ref={box}>{shown.length ? shown.join("\n") : failed ? "Can't read the console right now." : "Nothing yet. Start the server to see its console."}</pre>
      {failed && shown.length > 0 && <p className="muted small-note">Can't read the console right now. These are the last lines it showed.</p>}
      {ON_THIS_COMPUTER ? (
        <form className="row" onSubmit={(e) => { e.preventDefault(); send(); }}>
          <input className="search grow" placeholder="A server command, e.g. status or changelevel de_inferno" value={cmd} onChange={(e) => setCmd(e.target.value)} spellCheck={false} />
          <button className="btn" type="submit" disabled={server.state !== "running"}>Send</button>
        </form>
      ) : (
        <p className="muted small-note">Server commands can only be sent from the computer running Loner-Strike.</p>
      )}
    </div>
  );
}

/** A tip from the hosting guide's troubleshooting under an SSH error, when one fits. */
function ErrorWithFix({ error }: { error: string }) {
  const tip = troubleFor(error);
  return (
    <div className="inline-error" role="alert">
      <p className="err-text">{error}</p>
      {tip && <p className="small-note"><b>{tip.q}.</b> <span className="muted">{tip.a}</span></p>}
    </div>
  );
}

function NewServer({ servers, games, onClose, onCreated }: { servers: ServerInfo[]; games: string[]; onClose: () => void; onCreated: () => void }) {
  const toast = useToast();
  const ask = useConfirm();
  const [name, setName] = useState("");
  const [game, setGame] = useState<"cstrike" | "czero">(games.includes("cstrike") || !games.length ? "cstrike" : "czero");
  const [template, setTemplate] = useState(games.includes(game) ? "local" : "fresh");
  const [vis, setVis] = useState<string>("unlisted");
  const [password, setPassword] = useState("");
  // where it runs: this computer, or a Linux machine (a VPS) reached over SSH
  const [where, setWhere] = useState<"here" | "vps">("here");
  const [host, setHost] = useState("");
  // no guess: it's ubuntu on some hosts and root on others, and a wrong one just fails
  const [user, setUser] = useState("");
  const [key, setKey] = useState("");
  // the machine as first seen: its fingerprint to confirm before anything is set up
  const [probe, setProbe] = useState<{ fingerprint: string; known: boolean; arch: string; x86: boolean } | null>(null);
  const [checkErr, setCheckErr] = useState("");
  const [makeErr, setMakeErr] = useState("");
  const { run, busy } = useActions();
  const checking = busy("new-server:check");
  const sending = busy("new-server:make");

  // the first field of what's shown: the address for a VPS, else the name
  const nameBox = useRef<HTMLInputElement>(null);
  const hostBox = useRef<HTMLInputElement>(null);
  const first = useRef(true);
  useEffect(() => {
    if (first.current) return void (first.current = false);
    (where === "vps" ? hostBox : nameBox).current?.focus();
  }, [where]);

  // typed-in details aren't thrown away by a stray click or Escape
  const filled = !!(name.trim() || host || key || password);
  const close = async () => {
    if (sending) return;
    if (!filled || (await ask("Close without making the server? What you typed here is lost.", { ok: "Close without making it", danger: true }))) onClose();
  };
  const machineChanged = () => {
    setProbe(null);
    setCheckErr("");
  };

  const check = () => run("new-server:check", async () => {
    setProbe(null);
    setCheckErr("");
    type Probe = { error?: string; fingerprint?: string; known?: boolean; arch?: string; x86?: boolean };
    const r = await postJson<Probe>("/api/servers/remote/probe", { name, game, host, user, key }).catch((): Probe => ({ error: "Couldn't reach the app." }));
    if (r.error || !r.fingerprint) return setCheckErr(r.error || "Couldn't check the server.");
    setProbe({ fingerprint: r.fingerprint, known: !!r.known, arch: r.arch ?? "", x86: !!r.x86 });
  }, `Connecting to ${host}…`);
  const pickKey = async () => {
    const r = await post("/api/servers/pick-key");
    if (r.error) return toast(r.error, true);
    if (r.path) {
      setKey(String(r.path));
      machineChanged();
    }
  };
  const templates = [
    ...(games.includes(game) ? [{ id: "local", name: "My game", note: "Same modes, settings, admins and maps as your own game" }] : []),
    ...servers.filter((s) => s.game === game && s.ready).map((s) => ({ id: s.id, name: s.name, note: "Copy this server's setup" })),
    { id: "fresh", name: "Start fresh", note: "The basics: classic, deathmatch, knife, surf and the other built-in modes" },
  ];
  const create = () => run("new-server:make", async () => {
    setMakeErr("");
    const r = where === "vps"
      ? await post("/api/servers/remote", { name, game, template, visibility: vis, password, host, user, key, host_key: probe?.fingerprint ?? "" })
      : await post("/api/servers", { name, game, template, visibility: vis, password });
    if (r.error) return setMakeErr(r.error);
    // the setup runs inside the app: closing this window is fine, quitting stops it
    toast(where === "vps"
      ? `Setting up on ${host}. It takes 5 to 15 minutes; it keeps going if you close this window but not if you quit Loner-Strike.`
      : "Setting up the server. It takes a few minutes; it keeps going if you close this window but not if you quit Loner-Strike.");
    onCreated();
  }, where === "vps" ? `Starting the setup on ${host}…` : "Starting the server setup…");

  // why Make the server can't be pressed yet
  const why = where === "vps" && !probe ? "Check the server first." : where === "vps" && probe && !probe.x86 ? "This machine can't run the server." : !name.trim() ? "Give it a name." : "";
  return (
    <Sheet label="New server" onClose={close}>
      <div className="sheet-body">
        <h2>New server</h2>
        {ON_THIS_COMPUTER && (
          <div className="field">
            <span>Where it runs<Info text="This computer runs it while it's on. A Linux server is a machine you rent (a VPS, like Oracle's free tier) that runs it all the time; the app sets it up over SSH with your key." /></span>
            <div className="seg" role="group" aria-label="Where it runs">
              <button type="button" aria-pressed={where === "here"} onClick={() => setWhere("here")}>This computer</button>
              <button type="button" aria-pressed={where === "vps"} onClick={() => setWhere("vps")}>A Linux server (VPS)</button>
            </div>
          </div>
        )}
        {where === "vps" && (
          <>
            <label className="field">
              <span>Address<Info text="The machine's public IP address or host name, from your hosting provider's page." /></span>
              <input ref={hostBox} className="search" placeholder="e.g. 146.235.20.10" value={host} onChange={(e) => { setHost(e.target.value.trim()); machineChanged(); }} spellCheck={false} />
            </label>
            <label className="field">
              <span>User<Info text="The account you log in with over SSH: ubuntu on Oracle and AWS, root on Hetzner, DigitalOcean and Vultr. It needs to be root or able to use sudo." /></span>
              <input className="search" placeholder="ubuntu or root" value={user} onChange={(e) => { setUser(e.target.value.trim()); machineChanged(); }} spellCheck={false} />
              {user === "root" && <small>Works. The game server then runs as root too; the guide shows how to add a separate user if you'd rather it didn't.</small>}
            </label>
            <div className="field">
              <span>SSH key<Info text="The private key you log in with (the file without .pub), usually in the .ssh folder in your home folder. It stays on this computer; the app uses it to connect." /></span>
              <div className="row">
                <input className="search grow" placeholder="Pick your key file" value={key} onChange={(e) => { setKey(e.target.value); machineChanged(); }} spellCheck={false} />
                <button className="btn" type="button" onClick={pickKey}>Browse…</button>
              </div>
            </div>
            <div className="field">
              <span>Server's identity<Info text="Every SSH server has a fingerprint. Checking it once makes sure you're connected to your machine and not something pretending to be it; the app then refuses to connect if it ever changes." /></span>
              {!probe ? (
                <>
                  <div className="row">
                    <BusyButton className="btn" busy={checking} disabled={!host || !key || !user} label="Check the server" busyLabel="Connecting…" onClick={check} />
                    {(!host || !key || !user) && <span className="muted small-note">Fill in the address, user and key first.</span>}
                  </div>
                  {checkErr && <ErrorWithFix error={checkErr} />}
                </>
              ) : (
                <div className="probe">
                  <code>{probe.fingerprint}</code>
                  {!probe.x86 ? (
                    <ErrorWithFix error={`This machine is ${probe.arch}. The Counter-Strike server only runs on x86 (AMD or Intel) machines.`} />
                  ) : probe.known ? (
                    <p className="muted small-note">Matches what this computer's SSH has on record for {host}.</p>
                  ) : (
                    <p className="muted small-note">First time this computer sees {host}. To be sure it's yours, compare with what the machine shows for <code>{"for f in /etc/ssh/ssh_host_*_key.pub; do ssh-keygen -lf $f; done"}</code> (your provider's web console can run it): one of those lines should match.</p>
                  )}
                </div>
              )}
            </div>
          </>
        )}
        <label className="field">
          <span>Name</span>
          <input ref={nameBox} className="search" autoFocus maxLength={48} placeholder="e.g. Friday night frags" value={name} onChange={(e) => setName(e.target.value)} />
          <small>Shown in the server browser and on the welcome page.</small>
        </label>
        <div className="field">
          <span>Game</span>
          <div className="seg" role="group" aria-label="Game">
            {(["cstrike", "czero"] as const).map((g) => (
              <button key={g} type="button" aria-pressed={game === g} onClick={() => { setGame(g); setTemplate(games.includes(g) ? "local" : "fresh"); }}>{GAME[g]}</button>
            ))}
          </div>
        </div>
        <div className="field">
          <span>Set it up like</span>
          <div className="methods">
            {templates.map((t) => (
              <button key={t.id} type="button" className="method" role="radio" aria-checked={template === t.id} onClick={() => setTemplate(t.id)}>
                <span className="method-name">{t.name}</span>
                <span className="method-blurb">{t.note}</span>
              </button>
            ))}
          </div>
        </div>
        <div className="field">
          <span>Who can join<Info text="Anyone with the address suits friends: they join with the address you send them, but strangers can't find it. Listed on Steam puts it in everyone's Internet server list." /></span>
          <div className="seg" role="group" aria-label="Who can join">
            {VISIBILITY.map((v) => (
              <button key={v.id} type="button" aria-pressed={vis === v.id} onClick={() => setVis(v.id)}>{v.name}</button>
            ))}
          </div>
          <small>{VISIBILITY.find((v) => v.id === vis)?.note}. You can change it later.</small>
        </div>
        <label className="field">
          <span>Join password<Info text="Players type this to get in. Leave it empty for no password. You can change it later on the server's page." /></span>
          <input className="search" maxLength={24} placeholder="None" value={password} onChange={(e) => setPassword(e.target.value.replace(/[^A-Za-z0-9_-]/g, ""))} spellCheck={false} autoComplete="off" />
        </label>
        <p className="muted">
          {where === "vps"
            ? "Connects to the machine, installs Valve's free server and Loner-Strike there, opens the game ports in its firewall and runs it as a service that keeps going on its own. Ubuntu or Debian on an x86 (AMD or Intel) machine. Open UDP ports 27015-27020 in your provider's firewall too."
            : "Downloads Valve's free server with SteamCMD (about 1 GB the first time), then sets it up. Your game isn't touched."}
        </p>
        {makeErr && <ErrorWithFix error={makeErr} />}
        <div className="row">
          <BusyButton className="btn primary" busy={sending} disabled={!!why} label="Make the server" busyLabel="Starting…" onClick={create} />
          <button className="btn" type="button" onClick={close}>Cancel</button>
          {why && <span className="muted small-note">{why}</span>}
        </div>
      </div>
    </Sheet>
  );
}

function ImportServer({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const toast = useToast();
  const [dir, setDir] = useState("");
  const { run, busy } = useActions();
  const pick = async () => {
    const r = await post("/api/servers/pick-folder");
    if (r.dir) setDir(r.dir);
    else if (r.error) toast(r.error, true);
  };
  const go = () => run("server:import", async () => {
    const r = await post("/api/servers/import", { dir });
    if (r.error) return toast(r.error, true);
    toast(r.admins ? `Added. ${r.admins} admin${r.admins === 1 ? "" : "s"} and VIP${r.admins === 1 ? "" : "s"} moved to the Admins page.` : "Added");
    onDone();
  }, "Adding the server…");
  return (
    <Sheet label="Add a server you already run" onClose={dir.trim() ? undefined : onClose}>
      <div className="sheet-body">
        <h2>Add a server you already run</h2>
        <p className="muted measure">Pick the folder with hlds.exe in it. Nothing is moved or deleted. Loner-Strike brings over:</p>
        <ul className="bring">
          <li>The server's name and password from server.cfg</li>
          <li>Admins and VIPs listed by SteamID in users.ini. They move to the Admins page; the old lines stay in the file as comments.</li>
          <li>Bans, maps and your plugins stay as they are</li>
        </ul>
        <div className="row">
          <input className="search grow" placeholder="Server folder" value={dir} onChange={(e) => setDir(e.target.value)} spellCheck={false} />
          <button className="btn" onClick={pick}>Browse…</button>
        </div>
        <p className="muted">On a Mac the folder has to be inside a CrossOver bottle, so it can run. For a server on a Linux VPS, use New server instead.</p>
        <div className="row">
          <BusyButton className="btn primary" busy={busy("server:import")} disabled={!dir.trim()} label="Add server" busyLabel="Adding…" onClick={go} />
          <button className="btn" onClick={onClose}>Cancel</button>
        </div>
      </div>
    </Sheet>
  );
}

const HOSTS = [
  {
    name: "This computer",
    cost: "Free",
    body: "Make a server here. It runs while Loner-Strike is open. Friends outside your network get in with Direct, playit.gg or a virtual LAN (see Play with friends).",
    you: "Nothing extra.",
  },
  {
    name: "A spare PC at home",
    cost: "Free",
    body: "An old Windows PC or laptop that stays on. Install Loner-Strike and Steam with Counter-Strike on it, then make the server there.",
    you: "Leave it on, and open the server's UDP port on your router (27016 for the first server), or use Direct.",
  },
  {
    name: "Free cloud servers",
    cost: "Free",
    body: "Oracle Cloud's Always Free tier includes two small AMD servers, enough for about 10 players with a few bots. Pick the AMD one: the free Arm servers can't run the game. AWS and Azure give a small server free for 12 months. Google's free tier allows too little traffic.",
    you: "Sign up (a card is needed), make an AMD Ubuntu server with a public IP, open UDP ports 27015-27020 in its firewall, then add it here: New server, A Linux server (VPS).",
  },
  {
    name: "A cheap server (VPS)",
    cost: "About $4 to $7 a month",
    body: "Hetzner gives the most for the money; DigitalOcean and Vultr are simple; OVH includes DDoS protection. Pick a plan with an IPv4 address and an x86 (Intel/AMD) CPU, not Arm.",
    you: "Buy an Ubuntu plan, open UDP ports 27015-27020 in its firewall, then add it here: New server, A Linux server (VPS).",
  },
];

/** Where to run a server, and how big; folded away until opened. */
function WhereToHost() {
  return (
    <details className="where-to-host">
      <summary><h2>Where to host</h2><span className="muted">Free and cheap places, and how big a server you need</span></summary>
      <section className="stack-tight">
      <div className="host-grid">
        {HOSTS.map((h) => (
          <div key={h.name} className="panel host">
            <div className="row between">
              <h3>{h.name}</h3>
              <span className="tag">{h.cost}</span>
            </div>
            <p>{h.body}</p>
            <p className="muted"><b>What you do:</b> {h.you}</p>
          </div>
        ))}
      </div>
      <div className="panel">
        <h3>How big a server you need</h3>
        <SizingTable />
        <p className="muted measure">
          The game uses one core, so a faster core beats more of them. Bots cost CPU; a full server of bots needs the bigger plan. Each player uses about 25 KB/s.
          Disk: about 2 GB plus your maps. It needs a public IPv4 address (IPv6-only servers don't work). Shared web hosting can't run game servers.
        </p>
        <p className="muted measure">
          Ports (UDP): servers on this computer or a PC at home use 27016 and up, since your own game has 27015. A Linux server (VPS) uses 27015 to 27019, one per
          server, and 27020 for HLTV.
        </p>
      </div>
      </section>
    </details>
  );
}
