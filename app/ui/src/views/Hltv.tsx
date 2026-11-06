import { useEffect, useRef, useState, type ReactNode } from "react";
import { getJson, ON_THIS_COMPUTER, postJson, useActions } from "../api";
import type { ServerInfo } from "../types";
import { BusyButton, Info, SectionHead, Skeleton, Switch, useConfirm, useToast } from "../ui";

type Settings = { enabled: boolean; port: number; delay: number; max_spectators: number; name: string; password: string; record: boolean };
type Demo = { name: string; size: number; modified: number };
type Status = {
  available: boolean;
  settings: Settings;
  default_name: string;
  default_port: number;
  state: "running" | "starting" | "stopped";
  error: string | null;
  uptime: number;
  connected: boolean;
  spectators: number | null;
  slots: number | null;
  recording: string | null;
  server_running: boolean;
  /** on a Linux server: the address to watch from is the machine's */
  remote?: boolean;
  addresses: { computer: string | null; network: string | null; internet?: string };
  visibility: "lan" | "unlisted" | "listed";
  demos: Demo[];
  console: string[];
};
type Reply = { ok?: boolean; error?: string; restart?: boolean };

const post = (path: string, body: unknown = {}) => postJson<Reply>(path, body).catch((): Reply => ({ error: "Couldn't reach the app." }));
const DELAYS = [0, 10, 30, 60, 90, 120];

function size(b: number) {
  if (b < 1024 * 1024) return `${Math.max(1, Math.round(b / 1024))} KB`;
  return `${(b / 1024 / 1024).toFixed(b < 100 * 1024 * 1024 ? 1 : 0)} MB`;
}
function when(s: number) {
  return s ? new Date(s * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" }) : "";
}
function delayText(s: number) {
  return s === 0 ? "None" : s < 60 ? `${s} s` : `${s / 60} min`.replace(".5 min", " min 30 s");
}

/** HLTV for a dedicated server: a delayed spectator relay that can record demos. */
export function Hltv({ server }: { server: ServerInfo }) {
  const toast = useToast();
  const ask = useConfirm();
  const [st, setSt] = useState<Status | null>(null);
  const [form, setForm] = useState<Settings | null>(null);
  const { run, busy } = useActions();
  const key = (what: string) => `hltv:${server.id}:${what}`;
  const saving = busy(key("save"));
  const dirty = useRef(false);
  const [failed, setFailed] = useState("");

  const load = async () => {
    const r = await getJson<Status & { error?: string }>(`/api/servers/${server.id}/hltv`).catch(() => null);
    if (!r || !r.settings) return setFailed(r?.error || "Couldn't reach the app.");
    setFailed("");
    setSt(r);
    if (!dirty.current) setForm(r.settings);
  };
  useEffect(() => {
    dirty.current = false;
    setSt(null);
    setForm(null);
    // the next check only after the last one answered (a slow server doesn't pile them up)
    let t = 0;
    let live = true;
    const tick = async () => {
      if (!document.hidden) await load();
      if (live) t = window.setTimeout(tick, 4000);
    };
    tick();
    return () => {
      live = false;
      clearTimeout(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [server.id]);

  const set = (patch: Partial<Settings>) => {
    dirty.current = true;
    setForm((f) => (f ? { ...f, ...patch } : f));
  };

  const save = (next: Settings | null = form) => run(key("save"), async () => {
    if (!next) return;
    const r = await post(`/api/servers/${server.id}/hltv`, next);
    if (r.error) return toast(r.error, true);
    dirty.current = false;
    toast(r.restart ? "Saved. Restart HLTV to use the new settings." : "Saved");
    load();
  }, `Saving HLTV settings for ${server.name}…`);

  // one start, stop or restart at a time
  const acting = busy(key("act:start")) || busy(key("act:stop")) || busy(key("act:restart"));
  const act = (action: "start" | "stop" | "restart") => {
    if (acting) return;
    run(key(`act:${action}`), async () => {
      const r = await post(`/api/servers/${server.id}/hltv/${action}`);
      if (r.error) return toast(r.error, true);
      toast(action === "stop" ? "HLTV stopped" : "Starting HLTV…");
      setTimeout(load, 1200);
    }, `${action === "stop" ? "Stopping" : action === "restart" ? "Restarting" : "Starting"} HLTV on ${server.name}…`);
  };

  if (!st || !form) {
    return (
      <div className="stack">
        <SectionHead title="HLTV" />
        {failed ? <p className="muted">{failed}</p> : <Skeleton rows={4} />}
      </div>
    );
  }

  const running = st.state === "running";
  const saved = st.settings;
  return (
    <div className="stack hltv">
      <SectionHead
        title="HLTV"
        note="Lets people watch this server's matches live, a little behind the action, without taking a player slot. It can also record every map to a demo file you can replay later."
      />

      {!st.available && (
        <div className="banner">
          <div>This server's folder has no hltv.exe. Update Valve's server files in Overview, under Maintenance, to get it.</div>
        </div>
      )}

      <div className="hltv-grid">
        <div className="panel">
          <div className="row between">
            <h3>Broadcast</h3>
            <span className={`live${running ? " on" : ""}`}>{running ? "Running" : st.state === "starting" ? "Starting" : saved.enabled ? "Stopped" : "Off"}</span>
          </div>
          {st.error && <p className="err-text">{st.error}</p>}
          {!saved.enabled ? (
            <p className="muted">HLTV is off. Turn it on in the settings and it starts and stops with the server.</p>
          ) : running ? (
            <>
              <dl className="hltv-facts">
                <Fact label="Watching">{st.spectators === null ? "Not known yet" : `${st.spectators} of ${st.slots ?? saved.max_spectators}`}</Fact>
                <Fact label="Game">{st.connected ? `Following the match, ${delayText(saved.delay).toLowerCase()} behind` : "Connecting to the server…"}</Fact>
                <Fact label="Recording">{st.recording ?? (saved.record ? "Starts with the next map" : "Off")}</Fact>
              </dl>
              <Addresses st={st} />
            </>
          ) : st.state === "starting" ? (
            <p className="muted">Waiting for the server to finish loading its map. HLTV starts right after.</p>
          ) : st.server_running ? (
            <p className="muted">The server is running but HLTV isn't.</p>
          ) : (
            <p className="muted">It starts with the server.</p>
          )}
          {saved.enabled && st.server_running && (
            <div className="row">
              {running ? (
                <>
                  <BusyButton busy={busy(key("act:restart"))} disabled={acting} label="Restart HLTV" busyLabel="Restarting…" onClick={() => act("restart")} />
                  <BusyButton className="btn danger" busy={busy(key("act:stop"))} disabled={acting} label="Stop HLTV" busyLabel="Stopping…" onClick={() => act("stop")} />
                </>
              ) : (
                st.state !== "starting" && <BusyButton className="btn primary" busy={busy(key("act:start"))} disabled={acting || !st.available} label="Start HLTV" busyLabel="Starting…" onClick={() => act("start")} />
              )}
            </div>
          )}
        </div>

        <div className="panel">
          <h3>Settings</h3>
          <div className="setting">
            <span className="name">HLTV<span className="note">Starts and stops with the server</span></span>
            <Switch on={form.enabled} disabled={saving} onChange={(v) => { const next = { ...form, enabled: v }; setForm(next); save(next); }} label="HLTV" />
          </div>
          <label className="field">
            <span>Name</span>
            <input className="search" maxLength={32} placeholder={st.default_name} value={form.name} onChange={(e) => set({ name: e.target.value })} />
            <small>Shown on the scoreboard and in the server browser.</small>
          </label>
          <div className="field">
            <span>Delay</span>
            <div className="seg" role="group" aria-label="Delay">
              {(DELAYS.includes(form.delay) ? DELAYS : [...DELAYS, form.delay].sort((a, b) => a - b)).map((d) => (
                <button key={d} type="button" aria-pressed={form.delay === d} onClick={() => set({ delay: d })}>{delayText(d)}</button>
              ))}
            </div>
            <small>How far behind the game the broadcast runs, so watchers can't help a team. Under 10 seconds turns off the automatic camera.</small>
          </div>
          <div className="row hltv-numbers">
            <label className="field">
              <span>Spectators<Info text="The most people who can watch at once. Watchers don't take player slots. 0 lets nobody watch, but it can still record demos." /></span>
              <input className="search narrow" type="number" min={0} max={255} value={form.max_spectators} onChange={(e) => set({ max_spectators: Math.max(0, Math.round(+e.target.value)) })} />
            </label>
            <label className="field">
              <span>Port<Info text="HLTV listens on its own port, separate from the server's. Change it only if something else already uses it." /></span>
              <input className="search narrow" type="number" min={st.remote ? 27015 : 1024} max={st.remote ? 27020 : 65000} value={form.port} onChange={(e) => set({ port: Math.round(+e.target.value) })} />
            </label>
          </div>
          <p className="muted small-note">Each spectator uses about 3 KB/s of upload. The port is {st.default_port} unless you change it.</p>
          <label className="field">
            <span>Spectator password</span>
            <input className="search" maxLength={32} placeholder="None: anyone with the address can watch" value={form.password} onChange={(e) => set({ password: e.target.value })} spellCheck={false} autoComplete="off" />
          </label>
          <div className="setting">
            <span className="name">Record demos<span className="note">Saves every map to a demo file, with the same delay as the broadcast</span></span>
            <Switch on={form.record} onChange={(v) => set({ record: v })} label="Record demos" />
          </div>
          <div className="row">
            <BusyButton className="btn primary" busy={saving} label="Save" busyLabel="Saving…" onClick={() => save()} />
          </div>
        </div>
      </div>

      <Demos server={server} st={st} reload={load} ask={ask} />

      {saved.enabled && (
        <div className="panel">
          <h3>HLTV console</h3>
          <pre className="console hltv-console">{st.console.length ? st.console.join("\n") : "Nothing yet. It fills in once HLTV starts."}</pre>
        </div>
      )}
    </div>
  );
}

function Fact({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </div>
  );
}

function Addresses({ st }: { st: Status }) {
  const toast = useToast();
  const copy = async (text: string) => {
    try {
      await navigator.clipboard.writeText(`connect ${text}`);
      toast("Copied");
    } catch {
      toast("Couldn't copy. Select the text instead.", true);
    }
  };
  const port = st.settings.port;
  if (st.addresses.internet) {
    const at = st.addresses.internet;
    return (
      <div className="hltv-addresses">
        <p className="muted">To watch, open the game's console and type:</p>
        {st.settings.password && <code className="hltv-cmd">password {st.settings.password}</code>}
        <div className="hltv-address">
          <code className="hltv-cmd">connect {at}</code>
          <button className="btn small" onClick={() => copy(at)}>Copy</button>
        </div>
        <p className="muted small-note">Setup opened UDP 27015 to 27020 on the machine. If your provider has its own firewall, open port {port} there too.</p>
      </div>
    );
  }
  return (
    <div className="hltv-addresses">
      <p className="muted">To watch, open the game's console and type:</p>
      {st.settings.password && <code className="hltv-cmd">password {st.settings.password}</code>}
      {st.addresses.network && (
        <div className="hltv-address">
          <code className="hltv-cmd">connect {st.addresses.network}</code>
          <button className="btn small" onClick={() => copy(st.addresses.network!)}>Copy</button>
          <span className="muted">from this network</span>
        </div>
      )}
      <div className="hltv-address">
        <code className="hltv-cmd">connect {st.addresses.computer}</code>
        <button className="btn small" onClick={() => copy(st.addresses.computer ?? "")}>Copy</button>
        <span className="muted">from this computer</span>
      </div>
      {st.visibility !== "lan" && (
        <p className="muted small-note">People outside your network use your internet address with port {port}. Open UDP port {port} on your router for them, the same way as the server's own port.</p>
      )}
    </div>
  );
}

function Demos({ server, st, reload, ask }: { server: ServerInfo; st: Status; reload: () => void; ask: ReturnType<typeof useConfirm> }) {
  const toast = useToast();
  const { run, busy } = useActions();
  const key = (d: Demo) => `hltv:${server.id}:delete:${d.name}`;
  const remove = async (d: Demo) => {
    if (!(await ask(`Delete ${d.name}?`, { ok: "Delete", danger: true }))) return;
    await run(key(d), async () => {
      const r = await post(`/api/servers/${server.id}/hltv/delete-demo`, { name: d.name });
      if (r.error) return toast(r.error, true);
      toast("Deleted");
      reload();
    }, `Deleting ${d.name}…`);
  };
  const fetchKey = (d: Demo) => `hltv:${server.id}:fetch:${d.name}`;
  const fetchDemo = (d: Demo) => run(fetchKey(d), async () => {
    const r = await post(`/api/servers/${server.id}/hltv/fetch-demo`, { name: d.name });
    if (r.error) return toast(r.error, true);
    toast(`${d.name} is in your game's ${server.game} folder. Type viewdemo ${d.name.replace(/\.dem$/i, "")} in the console.`);
  }, `Downloading ${d.name}…`);
  const show = async (name = "") => {
    const r = await post(`/api/servers/${server.id}/hltv-folder`, { name });
    if (r.error) toast(r.error.includes("forbidden") ? "That only works on the computer running Loner-Strike." : r.error, true);
  };
  return (
    <div className="panel">
      <div className="row between">
        <h3>Demos</h3>
        {ON_THIS_COMPUTER && !st.remote && st.demos.length > 0 && <button className="btn" onClick={() => show()}>Open folder</button>}
      </div>
      {st.demos.length === 0 ? (
        <p className="muted">
          No demos yet. {st.settings.record ? "They appear here as each map is recorded." : "Turn on Record demos and each map is saved here."}
        </p>
      ) : (
        <>
          <ul className="hltv-demos">
            {st.demos.map((d) => {
              const live = st.recording === d.name;
              return (
                <li key={d.name}>
                  <div className="hltv-demo-name">
                    <b>{d.name}</b>
                    <span className="muted">{size(d.size)} · {when(d.modified)}{live ? " · recording now" : ""}</span>
                  </div>
                  <div className="row">
                    {ON_THIS_COMPUTER && st.remote ? (
                      <BusyButton className="btn" busy={busy(fetchKey(d))} label="Get it" busyLabel="Downloading…" onClick={() => fetchDemo(d)} />
                    ) : ON_THIS_COMPUTER ? (
                      <button className="btn" onClick={() => show(d.name)}>Show</button>
                    ) : (
                      <a className="btn" href={`/api/servers/${encodeURIComponent(server.id)}/hltv/demos/${encodeURIComponent(d.name)}`} download={d.name}>Download</a>
                    )}
                    <BusyButton className="btn danger" busy={busy(key(d))} disabled={live} label="Delete" busyLabel="Deleting…" onClick={() => remove(d)} />
                  </div>
                </li>
              );
            })}
          </ul>
          <p className="muted small-note">{st.remote ? "Get it copies a demo into your game's folder. Then type viewdemo followed by its name in the console." : `To watch one, put it in your game's ${server.game} folder and type viewdemo followed by its name in the console.`}</p>
        </>
      )}
    </div>
  );
}
