import { useEffect, useRef, useState } from "react";
import { getJson, postJson } from "../api";
import type { AppInfo, MapInfo, Online, ServerInfo, ServerList } from "../types";
import { SectionHead, SteamProgress, useToast } from "../ui";
import { Admins } from "./Admins";
import { Bots } from "./Bots";
import { Friends } from "./Friends";
import { GetMore } from "./GetMore";
import { Maps } from "./Maps";
import { Mods } from "./Mods";
import { PlayLive } from "./Play";
import { Players } from "./Players";
import { MatchButtons, SettingsPage } from "./Settings";
import type { Job, Scan } from "./Setup";
import { Welcome } from "./Welcome";

const GAME = { cstrike: "Counter-Strike 1.6", czero: "Condition Zero" } as const;
const VISIBILITY = [
  { id: "lan", name: "This network", note: "Only people on your Wi-Fi or LAN" },
  { id: "unlisted", name: "Anyone with the address", note: "Not in Steam's server list" },
  { id: "listed", name: "Listed on Steam", note: "Shows up in Find Servers → Internet" },
] as const;
const STATE = { running: "Running", stopped: "Stopped", crashed: "Stopped (crashed)", "setting-up": "Setting up" };

type Reply = { ok?: boolean; error?: string; job?: string; id?: string; admins?: number; loner?: boolean; dir?: string | null; lines?: string[]; reply?: string };
const post = (path: string, body: unknown = {}) => postJson<Reply>(path, body).catch((): Reply => ({ error: "Couldn't reach the app." }));

function uptime(s: number) {
  if (s < 60) return "just started";
  if (s < 3600) return `up ${Math.floor(s / 60)} min`;
  return `up ${Math.floor(s / 3600)} h ${Math.floor((s % 3600) / 60)} min`;
}

/** The setup job's progress, polled until it ends. */
export function JobProgress({ onEnd }: { onEnd: (ok: boolean) => void }) {
  const [job, setJob] = useState<Job | null>(null);
  const ended = useRef(false);
  useEffect(() => {
    const t = setInterval(async () => {
      const j = await getJson<Job>("/api/setup/job").catch(() => null);
      if (!j) return;
      setJob(j);
      if ((j.state === "done" || j.state === "error") && !ended.current) {
        ended.current = true;
        onEnd(j.state === "done");
      }
    }, 800);
    return () => clearInterval(t);
  }, [onEnd]);
  const now = job?.steps.find((s) => s.state === "running");
  return (
    <SteamProgress
      value={job?.progress ?? 0}
      title={job?.state === "error" ? "Stopped" : job?.kind === "server" ? "Setting up the server…" : "Installing…"}
      text={job?.state === "error" ? job.error : now ? `${now.label}${now.detail ? ` · ${now.detail}` : ""}` : "Getting ready…"}
      error={job?.state === "error"}
    />
  );
}

/** Every server, and making or adding one. Opening a server shows its own page. */
export function Servers({ scan, list, onOpen, onChanged }: { scan: Scan | null; list: ServerList | null; onOpen: (id: string) => void; onChanged: () => void }) {
  const toast = useToast();
  const [creating, setCreating] = useState(false);
  const [importing, setImporting] = useState(false);

  const act = async (s: ServerInfo, action: string) => {
    const r = await post(`/api/servers/${s.id}/${action}`);
    if (r.error) return toast(r.error, true);
    if (action === "join") toast(`Joining ${s.name}…`);
    onChanged();
  };
  const installed = (scan?.installs ?? []).flatMap((i) => i.games.filter((g) => g.installed).map((g) => g.id));

  return (
    <div className="stack">
      <SectionHead title="Servers" note="Optional: run dedicated servers that keep going without your game open, for friends or anyone. Each has its own modes, maps, admins and password, separate from your game.">
        <button className="btn" onClick={() => setImporting(true)}>Add one you already run</button>
        <button className="btn primary" onClick={() => setCreating(true)}>New server</button>
      </SectionHead>

      {list && list.servers.length === 0 && (
        <div className="panel">
          <h3>No servers yet</h3>
          <p className="muted measure">
            Your own game already hosts matches for you and friends (see Play with friends). A dedicated server is for when you want one that keeps
            running while you're not playing. Loner-Strike downloads Valve's free server (about 1 GB), sets it up and runs it for you.
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
                  {GAME[s.game]} · port {s.port} · {VISIBILITY.find((v) => v.id === s.visibility)?.name}
                  {s.imported ? " · added from your own folder" : ""}
                </p>
              </div>
              <span className={`live${s.state === "running" ? " on" : ""}`}>
                {STATE[s.state]}
                {s.state === "running" ? ` · ${uptime(s.uptime)}` : ""}
              </span>
            </div>
            {s.error && <p className="err-text">{s.error}</p>}
            {s.state === "setting-up" ? (
              <JobProgress onEnd={(ok) => { toast(ok ? `${s.name} is ready` : "Setting up the server stopped", !ok); onChanged(); }} />
            ) : (
              <div className="row">
                <button className="btn primary" onClick={() => onOpen(s.id)}>Open</button>
                {s.state === "running" ? (
                  <>
                    <button className="btn" onClick={() => act(s, "join")}>Join</button>
                    <button className="btn danger" onClick={() => act(s, "stop")}>Stop</button>
                  </>
                ) : (
                  <button className="btn" onClick={() => act(s, "start")}>Start</button>
                )}
              </div>
            )}
          </div>
        ))}
      </div>

      <HostingGuide />

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

type Tab = "overview" | "friends" | "modes" | "maps" | "getmaps" | "bots" | "match" | "fun" | "players" | "mods" | "admins" | "welcome";
const TABS: { id: Tab; label: string; live?: boolean }[] = [
  { id: "overview", label: "Overview" },
  { id: "friends", label: "Play with friends" },
  { id: "modes", label: "Modes", live: true },
  { id: "maps", label: "Maps" },
  { id: "getmaps", label: "Get maps" },
  { id: "bots", label: "Bots", live: true },
  { id: "match", label: "Match", live: true },
  { id: "fun", label: "Fun", live: true },
  { id: "players", label: "Players", live: true },
  { id: "mods", label: "Mods" },
  { id: "admins", label: "Admins" },
  { id: "welcome", label: "Welcome page" },
];

/** One server's own control panel. Every request from here names the server. */
export function ServerPage(props: {
  server: ServerInfo;
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
  const { server: s, data, app, scan, maps, refresh, reloadMaps, reloadApp, onChanged, onBack } = props;
  const toast = useToast();
  const [tab, setTab] = useState<Tab>("overview");
  const [busyJob, setBusyJob] = useState(false);

  const act = async (action: string) => {
    if (action === "delete") {
      const msg = s.imported
        ? `Remove "${s.name}" from Loner-Strike? Its folder stays where it is.`
        : `Delete "${s.name}"? Its folder, maps and settings are deleted.`;
      if (!confirm(msg)) return;
    }
    const r = await post(`/api/servers/${s.id}/${action}`);
    if (r.error) return toast(r.error, true);
    if (action === "join") toast(`Joining ${s.name}…`);
    if (action === "start") toast(`Starting ${s.name}…`);
    if (action === "update" && r.job) setBusyJob(true);
    if (action === "delete") return onBack(), onChanged();
    onChanged();
    setTimeout(refresh, 1500);
  };

  // reinstall everything that's on the server, keeping its settings
  const rebuild = async () => {
    if (!confirm(`Rebuild Loner-Strike on ${s.name}? It reinstalls its files; settings, admins and maps stay.`)) return;
    const components = (app?.game?.installed ?? []).filter((c) => c !== "foundation" && c !== "core");
    const r = await post("/api/setup/install", { hl: s.dir, games: [s.game], components, repair: true });
    if (r.error) return toast(r.error, true);
    setBusyJob(true);
  };
  const needsLive = TABS.find((t) => t.id === tab)?.live && !data;
  return (
    <div className="stack">
      <div className="server-top">
        <button className="link back" onClick={onBack}>← All servers</button>
        <div className="row between">
          <div>
            <h2 className="server-title">{s.name}</h2>
            <p className="muted">
              Dedicated server · {GAME[s.game]} · port {s.port} · <span className={`live inline${s.state === "running" ? " on" : ""}`}>{STATE[s.state]}</span>
            </p>
          </div>
          <div className="row">
            {s.state === "running" ? (
              <>
                <button className="btn primary" onClick={() => act("join")}>Join</button>
                <button className="btn" onClick={() => act("restart")}>Restart</button>
                <button className="btn danger" onClick={() => act("stop")}>Stop</button>
              </>
            ) : (
              <button className="btn primary" onClick={() => act("start")}>Start the server</button>
            )}
          </div>
        </div>
        <div className="chips server-tabs" role="tablist" aria-label={`${s.name} sections`}>
          {TABS.map((t) => (
            <button key={t.id} role="tab" className="chip" aria-pressed={tab === t.id} onClick={() => setTab(t.id)}>{t.label}</button>
          ))}
        </div>
      </div>

      {s.error && <p className="err-text">{s.error}</p>}
      {!s.loner && <AddLoner server={s} onDone={() => { onChanged(); reloadApp(); }} />}
      {busyJob && <JobProgress onEnd={(ok) => { setBusyJob(false); toast(ok ? "Done" : "That stopped. The bar shows why.", !ok); reloadApp(); }} />}

      {needsLive ? (
        <div className="waiting">
          <h2>{TABS.find((t) => t.id === tab)!.label}</h2>
          <p className="muted">{s.state === "running" ? "Connecting to the server…" : "This needs the server running."}</p>
          {s.state !== "running" && <button className="btn primary" onClick={() => act("start")}>Start the server</button>}
        </div>
      ) : (
        <>
          {tab === "overview" && (
            <>
              <ServerDetails server={s} onChanged={onChanged} act={act} />
              <Maintenance server={s} installed={app?.game?.installed ?? []} act={act} rebuild={rebuild} />
            </>
          )}
          {tab === "friends" && <Friends data={data} refresh={refresh} server={{ name: s.name, visibility: s.visibility }} />}
          {tab === "modes" && data && <PlayLive data={data} maps={maps} refresh={refresh} openMaps={() => setTab("maps")} />}
          {tab === "maps" && <Maps data={data} maps={maps} reload={reloadMaps} refresh={refresh} />}
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
        </>
      )}
    </div>
  );
}

/** For an imported server without Loner-Strike: put the basics on it. */
function AddLoner({ server, onDone }: { server: ServerInfo; onDone: () => void }) {
  const toast = useToast();
  const [running, setRunning] = useState(false);
  const start = async () => {
    if (!confirm("Add Loner-Strike to this server? It installs Metamod-P, AMX Mod X, ReGameDLL and the Game Mode Menu. Every file it replaces is backed up, and Settings → Remove puts them back.")) return;
    const r = await post("/api/setup/install", { hl: server.dir, games: [server.game], components: [] });
    if (r.error) return toast(r.error, true);
    setRunning(true);
  };
  return (
    <div className="banner">
      {running ? (
        <JobProgress onEnd={(ok) => { setRunning(false); toast(ok ? "Loner-Strike is on the server" : "Setup stopped", !ok); onDone(); }} />
      ) : (
        <>
          <div>This server doesn't have Loner-Strike yet, so the panel can only start, stop and manage admins. Add it to get modes, bots and the rest.</div>
          <button className="btn primary" onClick={start}>Add Loner-Strike</button>
        </>
      )}
    </div>
  );
}

function ServerDetails({ server, onChanged, act }: { server: ServerInfo; onChanged: () => void; act: (a: string) => void }) {
  const toast = useToast();
  const [name, setName] = useState(server.name);
  const [map, setMap] = useState(server.map);
  const [slots, setSlots] = useState(server.maxplayers);
  const [port, setPort] = useState(server.port);
  const [vis, setVis] = useState(server.visibility);
  const save = async () => {
    const r = await post(`/api/servers/${server.id}`, { name, map, maxplayers: slots, port, visibility: vis });
    if (r.error) return toast(r.error, true);
    toast(server.state === "running" ? "Saved. Restart the server to use it." : "Saved");
    onChanged();
  };
  return (
    <div className="server-details">
      <div className="panel">
        <h3>Settings</h3>
        <label className="field">
          <span>Name</span>
          <input className="search" maxLength={48} value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        <div className="field">
          <span>Who can join</span>
          <div className="seg" role="group" aria-label="Who can join">
            {VISIBILITY.map((v) => (
              <button key={v.id} aria-pressed={vis === v.id} onClick={() => setVis(v.id)}>{v.name}</button>
            ))}
          </div>
          <small>{VISIBILITY.find((v) => v.id === vis)?.note}. Friends outside your network also need a way in: see Play with friends.</small>
        </div>
        <div className="row">
          <label className="field">
            <span>Starting map</span>
            <input className="search" value={map} onChange={(e) => setMap(e.target.value)} spellCheck={false} />
          </label>
          <label className="field">
            <span>Player slots</span>
            <input className="search narrow" type="number" min={2} max={32} value={slots} onChange={(e) => setSlots(+e.target.value)} />
          </label>
          <label className="field">
            <span>Port</span>
            <input className="search narrow" type="number" min={1024} max={65000} value={port} onChange={(e) => setPort(+e.target.value)} />
          </label>
        </div>
        <div className="row">
          <button className="btn primary" onClick={save}>Save</button>
        </div>
      </div>
      <Console server={server} />
    </div>
  );
}

/** Updating, rebuilding and deleting a server. */
function Maintenance({ server, installed, act, rebuild }: { server: ServerInfo; installed: string[]; act: (a: string) => void; rebuild: () => void }) {
  const running = server.state === "running";
  return (
    <div className="panel">
      <h3>Maintenance</h3>
      {running && <p className="muted">Stop the server first to update or rebuild it.</p>}
      <ul className="upkeep">
        {!server.imported && (
          <li>
            <div><b>Update Valve's server files</b><p className="muted">Gets the latest version of the server from Valve. Your mods and settings stay.</p></div>
            <button className="btn" disabled={running} onClick={() => act("update")}>Update</button>
          </li>
        )}
        {server.loner && (
          <li>
            <div><b>Rebuild Loner-Strike</b><p className="muted">Reinstalls Metamod, AMX Mod X, ReGameDLL and the {Math.max(0, installed.length - 2)} mode{installed.length - 2 === 1 ? "" : "s"} and add-ons on it. Fixes a server that won't start or lost its mods. Settings, admins and maps stay.</p></div>
            <button className="btn" disabled={running} onClick={rebuild}>Rebuild</button>
          </li>
        )}
        <li>
          <div>
            <b>{server.imported ? "Remove from Loner-Strike" : "Delete the server"}</b>
            <p className="muted">{server.imported ? "Takes it off the list. Its folder stays where it is." : "Deletes its folder, maps, settings and admins. Your own game isn't touched."}</p>
          </div>
          <button className="btn danger" onClick={() => act("delete")}>{server.imported ? "Remove" : "Delete"}</button>
        </li>
      </ul>
    </div>
  );
}

function Console({ server }: { server: ServerInfo }) {
  const toast = useToast();
  const [lines, setLines] = useState<string[]>([]);
  const [cmd, setCmd] = useState("");
  const box = useRef<HTMLPreElement>(null);
  useEffect(() => {
    const load = () => getJson<Reply>(`/api/servers/${server.id}/console`).then((r) => setLines(r.lines ?? [])).catch(() => null);
    load();
    const t = setInterval(load, 2000);
    return () => clearInterval(t);
  }, [server.id]);
  useEffect(() => {
    box.current?.scrollTo({ top: box.current.scrollHeight });
  }, [lines]);
  const send = async () => {
    if (!cmd.trim()) return;
    const r = await post(`/api/servers/${server.id}/console/send`, { command: cmd });
    if (r.error) return toast(r.error.includes("forbidden") || r.error.includes("Couldn't reach") ? "The console only works on this computer." : r.error, true);
    setCmd("");
    if (r.reply?.trim()) setLines((l) => [...l, `] ${cmd}`, ...r.reply!.trim().split("\n")]);
  };
  return (
    <div className="panel">
      <h3>Console</h3>
      <pre className="console" ref={box}>{lines.length ? lines.join("\n") : "Nothing yet. Start the server to see its console."}</pre>
      <form className="row" onSubmit={(e) => { e.preventDefault(); send(); }}>
        <input className="search grow" placeholder="A server command, e.g. status or changelevel de_inferno" value={cmd} onChange={(e) => setCmd(e.target.value)} spellCheck={false} />
        <button className="btn" type="submit" disabled={server.state !== "running"}>Send</button>
      </form>
    </div>
  );
}

function NewServer({ servers, games, onClose, onCreated }: { servers: ServerInfo[]; games: string[]; onClose: () => void; onCreated: () => void }) {
  const toast = useToast();
  const [name, setName] = useState("");
  const [game, setGame] = useState<"cstrike" | "czero">(games.includes("cstrike") || !games.length ? "cstrike" : "czero");
  const [template, setTemplate] = useState(games.includes(game) ? "local" : "fresh");
  const [vis, setVis] = useState<string>("unlisted");
  const [sending, setSending] = useState(false);
  const templates = [
    ...(games.includes(game) ? [{ id: "local", name: "My game", note: "Same modes, settings, admins and maps as your own game" }] : []),
    ...servers.filter((s) => s.game === game && s.ready).map((s) => ({ id: s.id, name: s.name, note: "Copy this server's setup" })),
    { id: "fresh", name: "Start fresh", note: "The basics: classic, deathmatch, knife, surf and the other built-in modes" },
  ];
  const create = async () => {
    setSending(true);
    const r = await post("/api/servers", { name, game, template, visibility: vis });
    setSending(false);
    if (r.error) return toast(r.error, true);
    toast("Setting up the server. It takes a few minutes.");
    onCreated();
  };
  return (
    <div className="sheet-backdrop" onClick={onClose}>
      <div className="sheet" role="dialog" aria-label="New server" onClick={(e) => e.stopPropagation()}>
        <div className="sheet-body">
          <h2>New server</h2>
          <label className="field">
            <span>Name</span>
            <input className="search" autoFocus maxLength={48} placeholder="e.g. Friday night frags" value={name} onChange={(e) => setName(e.target.value)} />
            <small>Shown in the server browser and on the welcome page.</small>
          </label>
          <div className="field">
            <span>Game</span>
            <div className="seg" role="group" aria-label="Game">
              {(["cstrike", "czero"] as const).map((g) => (
                <button key={g} aria-pressed={game === g} onClick={() => { setGame(g); setTemplate(games.includes(g) ? "local" : "fresh"); }}>{GAME[g]}</button>
              ))}
            </div>
          </div>
          <div className="field">
            <span>Set it up like</span>
            <div className="methods">
              {templates.map((t) => (
                <button key={t.id} className="method" role="radio" aria-checked={template === t.id} onClick={() => setTemplate(t.id)}>
                  <span className="method-name">{t.name}</span>
                  <span className="method-blurb">{t.note}</span>
                </button>
              ))}
            </div>
          </div>
          <div className="field">
            <span>Who can join</span>
            <div className="seg" role="group" aria-label="Who can join">
              {VISIBILITY.map((v) => (
                <button key={v.id} aria-pressed={vis === v.id} onClick={() => setVis(v.id)}>{v.name}</button>
              ))}
            </div>
            <small>{VISIBILITY.find((v) => v.id === vis)?.note}. You can change it later.</small>
          </div>
          <p className="muted">Downloads Valve's free server with SteamCMD (about 1 GB the first time), then sets it up. Your game isn't touched.</p>
          <div className="row">
            <button className="btn primary" disabled={!name.trim() || sending} onClick={create}>{sending ? "Starting…" : "Make the server"}</button>
            <button className="btn" onClick={onClose}>Cancel</button>
          </div>
        </div>
      </div>
    </div>
  );
}

function ImportServer({ onClose, onDone }: { onClose: () => void; onDone: () => void }) {
  const toast = useToast();
  const [dir, setDir] = useState("");
  const pick = async () => {
    const r = await post("/api/servers/pick-folder");
    if (r.dir) setDir(r.dir);
    else if (r.error) toast(r.error, true);
  };
  const go = async () => {
    const r = await post("/api/servers/import", { dir });
    if (r.error) return toast(r.error, true);
    toast(r.admins ? `Added. ${r.admins} admin${r.admins === 1 ? "" : "s"} and VIP${r.admins === 1 ? "" : "s"} moved to the Admins page.` : "Added");
    onDone();
  };
  return (
    <div className="sheet-backdrop" onClick={onClose}>
      <div className="sheet" role="dialog" aria-label="Add a server you already run" onClick={(e) => e.stopPropagation()}>
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
          <p className="muted">On a Mac the folder has to be inside a CrossOver bottle, so it can run. Servers on a Linux VPS come in a later version.</p>
          <div className="row">
            <button className="btn primary" disabled={!dir.trim()} onClick={go}>Add server</button>
            <button className="btn" onClick={onClose}>Cancel</button>
          </div>
        </div>
      </div>
    </div>
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
    you: "Leave it on, and open UDP port 27015 on your router (or use Direct).",
  },
  {
    name: "Free cloud servers",
    cost: "Free",
    body: "Oracle Cloud's Always Free tier includes two small AMD servers, enough for about 10 players with a few bots. Pick the AMD one: the free Arm servers can't run the game. AWS and Azure give a small server free for 12 months. Google's free tier allows too little traffic.",
    you: "Sign up (a card is needed), make a server and open UDP 27015 in its firewall.",
  },
  {
    name: "A cheap server (VPS)",
    cost: "About $4–7 a month",
    body: "Hetzner gives the most for the money; DigitalOcean and Vultr are simple; OVH includes DDoS protection. Pick a plan with an IPv4 address and an x86 (Intel/AMD) CPU, not Arm.",
    you: "Buy the plan and open UDP 27015. Setting up a Linux server from this app comes in a later version.",
  },
];

function HostingGuide() {
  return (
    <section className="stack-tight">
      <h2>Where to host</h2>
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
        <table className="sizing">
          <thead>
            <tr><th>Players (with bots)</th><th>CPU</th><th>Memory</th><th>Upload speed</th></tr>
          </thead>
          <tbody>
            <tr><td>Up to 10</td><td>1 core</td><td>512 MB–1 GB</td><td>5 Mbps</td></tr>
            <tr><td>Up to 20</td><td>1 fast core (2 GHz+)</td><td>1 GB</td><td>10 Mbps</td></tr>
            <tr><td>Up to 32</td><td>1 fast core</td><td>2 GB</td><td>15–20 Mbps</td></tr>
          </tbody>
        </table>
        <p className="muted measure">
          The game uses one core, so a faster core beats more of them. Bots cost CPU; a full server of bots needs the fast core. Each player uses about 25 KB/s.
          Disk: about 2 GB plus your maps. It needs a public IPv4 address (IPv6-only servers don't work) and UDP port 27015 open, or 27016 and up for more servers.
          Shared web hosting can't run game servers.
        </p>
      </div>
    </section>
  );
}
