import { useCallback, useEffect, useRef, useState } from "react";
import { getJson, postJson } from "../api";
import type { ServerInfo, ServerList } from "../types";
import { SectionHead, useToast } from "../ui";
import type { Job, Scan } from "./Setup";

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
  if (!job) return <p className="muted">Starting…</p>;
  const now = job.steps.find((s) => s.state === "running");
  return (
    <div className="stack-tight">
      <progress className="setup-progress" max={1} value={job.progress} />
      <p className="muted">{job.state === "error" ? <span className="err-text">{job.error}</span> : now ? `${now.label}${now.detail ? `: ${now.detail}` : ""}` : "Finishing…"}</p>
    </div>
  );
}

export function Servers({ scan, selected, onSelect, onChanged }: { scan: Scan | null; selected: string; onSelect: (id: string) => void; onChanged: () => void }) {
  const toast = useToast();
  const [list, setList] = useState<ServerList | null>(null);
  const [creating, setCreating] = useState(false);
  const [importing, setImporting] = useState(false);
  const [busyJob, setBusyJob] = useState<string | null>(null);
  const [open, setOpen] = useState<string | null>(null);

  const load = useCallback(() => {
    getJson<ServerList>("/api/servers").then(setList).catch(() => null);
  }, []);
  useEffect(() => {
    load();
    const t = setInterval(() => !document.hidden && load(), 3000);
    return () => clearInterval(t);
  }, [load]);

  const act = async (s: ServerInfo, action: string) => {
    if (action === "delete") {
      const msg = s.imported
        ? `Remove "${s.name}" from Loner-Strike? Its folder stays where it is.`
        : `Delete "${s.name}"? Its folder, maps and settings are deleted.`;
      if (!confirm(msg)) return;
    }
    const r = await post(`/api/servers/${s.id}/${action}`);
    if (r.error) return toast(r.error, true);
    if (action === "join") toast(`Joining ${s.name}…`);
    if (action === "update" && r.job) setBusyJob(s.id);
    if (action === "delete") onChanged();
    load();
  };

  const installed = (scan?.installs ?? []).flatMap((i) => i.games.filter((g) => g.installed).map((g) => g.id));

  return (
    <div className="stack">
      <SectionHead title="Servers" note="Dedicated servers run on their own, separate from your game. Each has its own modes, maps, admins and password.">
        <button className="btn" onClick={() => setImporting(true)}>Add one you already run</button>
        <button className="btn primary" onClick={() => setCreating(true)}>New server</button>
      </SectionHead>

      {list && list.servers.length === 0 && (
        <div className="panel">
          <h3>No servers yet</h3>
          <p className="muted measure">
            A dedicated server keeps going when you close the game, and you join it like anyone else. Loner-Strike downloads Valve's free
            server (about 1 GB), sets it up like your game and runs it for you.
          </p>
        </div>
      )}

      <div className="server-list">
        {list?.servers.map((s) => (
          <div key={s.id} className={`server${selected === s.id ? " selected" : ""}`}>
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
              <JobProgress onEnd={(ok) => { toast(ok ? `${s.name} is ready` : "Setting up the server stopped", !ok); load(); onChanged(); }} />
            ) : busyJob === s.id ? (
              <JobProgress onEnd={(ok) => { setBusyJob(null); toast(ok ? "Server files updated" : "The update stopped", !ok); }} />
            ) : (
              <div className="row">
                {s.state === "running" ? (
                  <>
                    <button className="btn primary" onClick={() => act(s, "join")}>Join</button>
                    <button className="btn" onClick={() => act(s, "restart")}>Restart</button>
                    <button className="btn danger" onClick={() => act(s, "stop")}>Stop</button>
                  </>
                ) : (
                  <button className="btn primary" onClick={() => act(s, "start")}>Start</button>
                )}
                {selected !== s.id && <button className="btn" onClick={() => onSelect(s.id)}>Manage</button>}
                <button className="btn" aria-expanded={open === s.id} onClick={() => setOpen(open === s.id ? null : s.id)}>
                  {open === s.id ? "Hide details" : "Settings & console"}
                </button>
              </div>
            )}
            {!s.loner && s.ready && <AddLoner server={s} onDone={load} />}
            {open === s.id && s.ready && <ServerDetails server={s} onChanged={load} act={(a) => act(s, a)} />}
          </div>
        ))}
      </div>

      <HostingGuide />

      {creating && (
        <NewServer
          servers={list?.servers ?? []}
          games={installed}
          onClose={() => setCreating(false)}
          onCreated={() => { setCreating(false); load(); }}
        />
      )}
      {importing && <ImportServer onClose={() => setImporting(false)} onDone={() => { setImporting(false); load(); }} />}
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
          {!server.imported && <button className="btn" onClick={() => act("update")} disabled={server.state === "running"}>Update Valve's server files</button>}
          <button className="btn danger" onClick={() => act("delete")}>{server.imported ? "Remove from list" : "Delete server"}</button>
        </div>
      </div>
      <Console server={server} />
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
