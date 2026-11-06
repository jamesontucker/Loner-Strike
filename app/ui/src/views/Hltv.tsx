import { useEffect, useRef, useState, type ReactNode } from "react";
import { getJson, ON_THIS_COMPUTER, postJson } from "../api";
import type { ServerInfo } from "../types";
import { SectionHead, Skeleton, Switch, useConfirm, useToast } from "../ui";

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
  addresses: { computer: string; network: string | null };
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
  const [saving, setSaving] = useState(false);
  const dirty = useRef(false);

  const load = async () => {
    const r = await getJson<Status & { error?: string }>(`/api/servers/${server.id}/hltv`).catch(() => null);
    if (!r || !r.settings) return;
    setSt(r);
    if (!dirty.current) setForm(r.settings);
  };
  useEffect(() => {
    dirty.current = false;
    setSt(null);
    setForm(null);
    load();
    const t = setInterval(() => !document.hidden && load(), 4000);
    return () => clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [server.id]);

  const set = (patch: Partial<Settings>) => {
    dirty.current = true;
    setForm((f) => (f ? { ...f, ...patch } : f));
  };

  const save = async (next: Settings | null = form) => {
    if (!next) return;
    setSaving(true);
    const r = await post(`/api/servers/${server.id}/hltv`, next);
    setSaving(false);
    if (r.error) return toast(r.error, true);
    dirty.current = false;
    toast(r.restart ? "Saved. Restart HLTV to use the new settings." : "Saved");
    load();
  };

  const act = async (action: "start" | "stop" | "restart") => {
    const r = await post(`/api/servers/${server.id}/hltv/${action}`);
    if (r.error) return toast(r.error, true);
    toast(action === "stop" ? "HLTV stopped" : "Starting HLTV…");
    setTimeout(load, 1200);
  };

  if (!st || !form) {
    return (
      <div className="stack">
        <SectionHead title="HLTV" />
        <Skeleton rows={4} />
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
                  <button className="btn" onClick={() => act("restart")}>Restart HLTV</button>
                  <button className="btn danger" onClick={() => act("stop")}>Stop HLTV</button>
                </>
              ) : (
                st.state !== "starting" && <button className="btn primary" onClick={() => act("start")} disabled={!st.available}>Start HLTV</button>
              )}
            </div>
          )}
        </div>

        <div className="panel">
          <h3>Settings</h3>
          <div className="setting">
            <span className="name">HLTV<span className="note">Starts and stops with the server</span></span>
            <Switch on={form.enabled} onChange={(v) => { const next = { ...form, enabled: v }; setForm(next); save(next); }} label="HLTV" />
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
              <span>Spectators</span>
              <input className="search narrow" type="number" min={0} max={255} value={form.max_spectators} onChange={(e) => set({ max_spectators: Math.max(0, Math.round(+e.target.value)) })} />
            </label>
            <label className="field">
              <span>Port</span>
              <input className="search narrow" type="number" min={1024} max={65000} value={form.port} onChange={(e) => set({ port: Math.round(+e.target.value) })} />
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
            <button className="btn primary" disabled={saving} onClick={() => save()}>{saving ? "Saving…" : "Save"}</button>
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
        <button className="btn small" onClick={() => copy(st.addresses.computer)}>Copy</button>
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
  const remove = async (d: Demo) => {
    if (!(await ask(`Delete ${d.name}?`, { ok: "Delete", danger: true }))) return;
    const r = await post(`/api/servers/${server.id}/hltv/delete-demo`, { name: d.name });
    if (r.error) return toast(r.error, true);
    toast("Deleted");
    reload();
  };
  const show = async (name = "") => {
    const r = await post(`/api/servers/${server.id}/hltv-folder`, { name });
    if (r.error) toast(r.error.includes("forbidden") ? "That only works on the computer running Loner-Strike." : r.error, true);
  };
  return (
    <div className="panel">
      <div className="row between">
        <h3>Demos</h3>
        {ON_THIS_COMPUTER && st.demos.length > 0 && <button className="btn" onClick={() => show()}>Open folder</button>}
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
                    {ON_THIS_COMPUTER ? (
                      <button className="btn" onClick={() => show(d.name)}>Show</button>
                    ) : (
                      <a className="btn" href={`/api/servers/${encodeURIComponent(server.id)}/hltv/demos/${encodeURIComponent(d.name)}`} download={d.name}>Download</a>
                    )}
                    <button className="btn danger" disabled={live} onClick={() => remove(d)}>Delete</button>
                  </div>
                </li>
              );
            })}
          </ul>
          <p className="muted small-note">To watch one, put it in your game's {server.game} folder and type viewdemo followed by its name in the console.</p>
        </>
      )}
    </div>
  );
}
