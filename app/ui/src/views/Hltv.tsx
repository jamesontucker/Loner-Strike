import { useEffect, useRef, useState, type ReactNode } from "react";
import { copyText, getJson, ON_COMPUTER_ONLY, ON_THIS_COMPUTER, postJson, useActions } from "../api";
import { restartPending, type ServerInfo } from "../types";
import { BusyButton, Info, LoadError, SectionHead, Skeleton, Switch, useConfirm, useToast } from "../ui";
import { useDirty } from "../store";

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
  /** internet: the server's public address with HLTV's port (on a VPS, its IP when none is set) */
  addresses: { computer: string | null; network: string | null; internet?: string | null };
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
/** Servers whose HLTV settings were saved while it ran: its uptime then. Kept across tab
 *  switches; cleared once HLTV restarts or stops. */
const restartAfter = new Map<string, number>();

/** Why a number is out of its range, or "" when it's fine. */
const numberProblem = (text: string, min: number, max: number, what: string) => {
  const v = text.trim() === "" ? NaN : Number(text);
  return !Number.isInteger(v) || v < min || v > max ? `${what}: a whole number from ${min} to ${max}.` : "";
};

/** Pulls a typed number into its range when the field is left (an empty one waits for Save). */
const clampText = (text: string, min: number, max: number) => {
  const v = Math.round(Number(text));
  return text.trim() === "" || isNaN(v) ? text : String(Math.min(max, Math.max(min, v)));
};

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
  // edits not saved yet: polls don't overwrite them, and leaving the tab asks first
  const dirty = useRef(false);
  const [isDirty, setIsDirty] = useState(false);
  const markDirty = (v: boolean) => {
    dirty.current = v;
    setIsDirty(v);
  };
  useDirty("hltv", isDirty);
  const [failed, setFailed] = useState("");
  // the number fields as typed, checked on Save
  const [nums, setNums] = useState<{ spectators: string; port: string } | null>(null);
  const [problem, setProblem] = useState("");
  const [restartAt, setRestartAt] = useState<number | undefined>(() => restartAfter.get(server.id));

  const load = async () => {
    const r = await getJson<Status & { error?: string }>(`/api/servers/${server.id}/hltv`).catch(() => null);
    if (!r || !r.settings) return setFailed(r?.error || "Couldn't reach the app.");
    setFailed("");
    setSt(r);
    if (!dirty.current) {
      setForm(r.settings);
      setNums({ spectators: String(r.settings.max_spectators), port: String(r.settings.port) });
    }
  };
  useEffect(() => {
    markDirty(false);
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
    markDirty(true);
    setForm((f) => (f ? { ...f, ...patch } : f));
  };
  const setNum = (patch: Partial<{ spectators: string; port: string }>) => {
    markDirty(true);
    setProblem("");
    setNums((n) => (n ? { ...n, ...patch } : n));
  };

  const needRestart = (r: Reply) => {
    if (!r.restart || !st) return;
    restartAfter.set(server.id, st.uptime);
    setRestartAt(st.uptime);
  };
  const clearRestart = () => {
    restartAfter.delete(server.id);
    setRestartAt(undefined);
  };
  // HLTV restarted (its uptime started again) or stopped: the new settings are in use
  useEffect(() => {
    if (restartAt !== undefined && st && (st.state !== "running" || st.uptime < restartAt)) clearRestart();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [st, restartAt]);

  const save = () => {
    if (!form || !nums || !st) return;
    const portMin = st.remote ? 27015 : 1024;
    const portMax = st.remote ? 27020 : 65000;
    const bad = numberProblem(nums.spectators, 0, 255, "Spectators") || numberProblem(nums.port, portMin, portMax, "Port");
    setProblem(bad);
    if (bad) return;
    const next = { ...form, max_spectators: Number(nums.spectators), port: Number(nums.port) };
    run(key("save"), async () => {
      const r = await post(`/api/servers/${server.id}/hltv`, next);
      if (r.error) return toast(r.error, true);
      markDirty(false);
      needRestart(r);
      toast(r.restart ? "Saved. Restart HLTV to use the new settings." : "Saved");
      load();
    }, `Saving HLTV settings for ${server.name}…`);
  };
  // the on/off switch saves just that: other edits wait for Save
  const setEnabled = (v: boolean) => run(key("save"), async () => {
    if (!st) return;
    const r = await post(`/api/servers/${server.id}/hltv`, { ...st.settings, enabled: v });
    if (r.error) return toast(r.error, true);
    setForm((f) => (f ? { ...f, enabled: v } : f));
    toast(v ? "HLTV is on. It starts and stops with the server." : "HLTV is off");
    load();
  }, `${v ? "Turning on" : "Turning off"} HLTV on ${server.name}…`);

  // one start, stop or restart at a time
  const acting = busy(key("act:start")) || busy(key("act:stop")) || busy(key("act:restart"));
  const act = (action: "start" | "stop" | "restart") => {
    if (acting) return;
    run(key(`act:${action}`), async () => {
      const r = await post(`/api/servers/${server.id}/hltv/${action}`);
      if (r.error) return toast(r.error, true);
      if (action !== "stop") clearRestart();
      toast(action === "stop" ? "HLTV stopped" : "Starting HLTV…");
      setTimeout(load, 1200);
    }, `${action === "stop" ? "Stopping" : action === "restart" ? "Restarting" : "Starting"} HLTV on ${server.name}…`);
  };

  if (!st || !form || !nums) {
    return (
      <div className="stack">
        <SectionHead title="Spectators & demos" />
        {failed ? <LoadError text={failed} onRetry={load} /> : <Skeleton rows={4} />}
      </div>
    );
  }

  const running = st.state === "running";
  const saved = st.settings;
  // settings change the server's machine: only from the computer running Loner-Strike
  const ro = !ON_THIS_COMPUTER;
  // waiting for a restart: what the app keeps (survives reloads), or a save seen here
  const restartWaits = restartAt !== undefined || restartPending(server).hltv;
  return (
    <div className="stack hltv">
      <SectionHead
        title="Spectators & demos"
        note="HLTV lets people watch this server's matches live, a little behind the action, without taking a player slot. It can also record every map to a demo file you can replay later."
      />
      {failed && <p className="muted small-note" role="status">Can't reach the server. Showing what it last reported.</p>}
      {restartWaits && running && (
        <div className="files-bar" role="status">
          <span>New HLTV settings are saved. They're used after HLTV restarts.</span>
          <span className="files-bar-actions">
            <BusyButton className="btn small primary" busy={busy(key("act:restart"))} disabled={acting} label="Restart now" busyLabel="Restarting…" onClick={() => act("restart")} />
          </span>
        </div>
      )}

      {!st.available && (
        <div className="banner">
          <div>{server.imported ? "This server's folder doesn't have HLTV. Add HLTV to the folder (it comes with Valve's server files), then restart the server." : !ON_THIS_COMPUTER ? `This server's folder doesn't have HLTV. ${ON_COMPUTER_ONLY}` : "This server's folder doesn't have HLTV. To get it, open Overview and click Update under Maintenance."}</div>
        </div>
      )}

      <div className="hltv-grid">
        <div className="panel">
          <div className="row between">
            <h3>Broadcast</h3>
            <span className={`live${running ? " on" : ""}`}>{running ? "Running" : st.state === "starting" ? "Starting" : saved.enabled ? "Stopped" : "Off"}</span>
          </div>
          {/* saves by itself, unlike the settings beside it */}
          <div className="setting">
            <span className="name">
              HLTV
              <Info text="Lets people watch without joining a team, and can record demos. Starts and stops with the server." />
              <span className="note">Saves straight away. It starts and stops with the server</span>
            </span>
            <Switch on={form.enabled} disabled={saving || ro} onChange={setEnabled} label="HLTV" />
          </div>
          {st.error && <p className="err-text">{st.error}</p>}
          {!saved.enabled ? (
            <p className="muted">HLTV is off. Turn it on above and it starts and stops with the server.</p>
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
            <p className="muted">The server is running but HLTV isn't. Start it below; if it stops again, the HLTV console at the bottom says why.</p>
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
          {ro && <p className="muted small-note">{ON_COMPUTER_ONLY}</p>}
          <fieldset className="plain" disabled={ro}>
          <label className="field">
            <span>Name</span>
            <input className="search" maxLength={32} placeholder={st.default_name} value={form.name} onChange={(e) => set({ name: e.target.value })} />
            <small>Shown on the scoreboard and in the server browser.</small>
          </label>
          <div className="field">
            <span>Delay<Info text="How far behind the live game watchers see. A longer delay stops them telling a team where the enemy is." /></span>
            <div className="seg" role="group" aria-label="Delay">
              {(DELAYS.includes(form.delay) ? DELAYS : [...DELAYS, form.delay].sort((a, b) => a - b)).map((d) => (
                <button key={d} type="button" aria-pressed={form.delay === d} onClick={() => set({ delay: d })}>{delayText(d)}</button>
              ))}
            </div>
            <small>How far behind the game the broadcast runs, so watchers can't help a team. Under 10 seconds turns off the automatic camera.</small>
          </div>
          <div className="row hltv-numbers">
            <label className="field">
              <span>Spectators<Info text="The most people who can watch at once, without taking player slots. At 0 nobody can watch, but HLTV still records demos." /></span>
              <input className="search narrow" type="number" min={0} max={255} value={nums.spectators} onChange={(e) => setNum({ spectators: e.target.value })} onBlur={() => setNums({ ...nums, spectators: clampText(nums.spectators, 0, 255) })} />
            </label>
            <label className="field">
              <span>Port<Info text="HLTV listens on its own port, separate from the server's. Change it only if something else already uses it." /></span>
              <input className="search narrow" type="number" min={st.remote ? 27015 : 1024} max={st.remote ? 27020 : 65000} value={nums.port} onChange={(e) => setNum({ port: e.target.value })} onBlur={() => setNums({ ...nums, port: clampText(nums.port, st.remote ? 27015 : 1024, st.remote ? 27020 : 65000) })} />
            </label>
          </div>
          {problem && <p className="err-text small-note">{problem}</p>}
          <p className="muted small-note">Each spectator uses about 3 KB/s of upload. The port is {st.default_port} unless you change it.</p>
          <label className="field">
            <span>Spectator password<Info text="Watchers type this to connect. It's separate from the server's join password." /></span>
            <input className="search" maxLength={32} placeholder="None: anyone with the address can watch" value={form.password} onChange={(e) => set({ password: e.target.value })} spellCheck={false} autoComplete="off" />
          </label>
          <div className="setting">
            <span className="name">Record demos<span className="note">Saves every map to a demo file, with the same delay as the broadcast</span></span>
            <Switch on={form.record} onChange={(v) => set({ record: v })} label="Record demos" />
          </div>
          </fieldset>
          {!ro && (
            <div className="row">
              <BusyButton className="btn primary" busy={saving} disabled={!isDirty} label="Save changes" busyLabel="Saving…" onClick={save} />
              {isDirty && !saving && <span className="muted small-note">Unsaved changes</span>}
            </div>
          )}
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
    if (await copyText(`connect ${text}`)) toast("Copied");
    else toast("Couldn't copy. Select the text instead.", true);
  };
  const port = st.settings.port;
  if (st.remote && st.addresses.internet) {
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
      {st.addresses.internet && (
        <div className="hltv-address">
          <code className="hltv-cmd">connect {st.addresses.internet}</code>
          <button className="btn small" onClick={() => copy(st.addresses.internet!)}>Copy</button>
          <span className="muted">from the internet</span>
        </div>
      )}
      {st.addresses.network && (
        <div className="hltv-address">
          <code className="hltv-cmd">connect {st.addresses.network}</code>
          <button className="btn small" onClick={() => copy(st.addresses.network!)}>Copy</button>
          <span className="muted">from this network</span>
        </div>
      )}
      {st.addresses.computer && (
        <div className="hltv-address">
          <code className="hltv-cmd">connect {st.addresses.computer}</code>
          <button className="btn small" onClick={() => copy(st.addresses.computer!)}>Copy</button>
          <span className="muted">from this computer</span>
        </div>
      )}
      {!st.addresses.network && !st.addresses.computer && <p className="muted small-note">The address isn't known yet. It shows here once HLTV is up.</p>}
      {st.visibility !== "lan" && (
        <p className="muted small-note">People outside your network use {st.addresses.internet ? "the internet address above" : `your internet address with port ${port}`}. Open UDP port {port} on your router for them, the same way as the server's own port.</p>
      )}
    </div>
  );
}

function Demos({ server, st, reload, ask }: { server: ServerInfo; st: Status; reload: () => void; ask: ReturnType<typeof useConfirm> }) {
  const toast = useToast();
  const { run, busy } = useActions();
  const key = (d: Demo) => `hltv:${server.id}:delete:${d.name}`;
  const remove = async (d: Demo) => {
    if (!(await ask(`Delete ${d.name}? It can't be brought back.`, { ok: "Delete the demo", danger: true }))) return;
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
    const cmd = `viewdemo ${d.name.replace(/\.dem$/i, "")}`;
    toast(`${d.name} is in your game's ${server.game} folder. Type ${cmd} in the console.`, false, { label: "Copy", run: () => void copyText(cmd) });
  }, `Copying ${d.name} to your game…`);
  const show = async (name = "") => {
    const r = await post(`/api/servers/${server.id}/hltv-folder`, { name });
    if (r.error) toast(r.error.includes("forbidden") ? ON_COMPUTER_ONLY : r.error, true);
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
                      <BusyButton className="btn" busy={busy(fetchKey(d))} label="Copy to my game" busyLabel="Copying…" onClick={() => fetchDemo(d)} />
                    ) : ON_THIS_COMPUTER ? (
                      <button className="btn" onClick={() => show(d.name)}>Show</button>
                    ) : (
                      <a className="btn" href={`/api/servers/${encodeURIComponent(server.id)}/hltv/demos/${encodeURIComponent(d.name)}`} download={d.name}>Download</a>
                    )}
                    {ON_THIS_COMPUTER && <BusyButton className="btn danger" busy={busy(key(d))} disabled={live} label="Delete" busyLabel="Deleting…" onClick={() => remove(d)} />}
                  </div>
                </li>
              );
            })}
          </ul>
          <p className="muted small-note">{st.remote ? "Copy to my game puts a demo in your game's folder. Then type viewdemo followed by its name in the console." : `To watch one, put it in your game's ${server.game} folder and type viewdemo followed by its name in the console.`}</p>
        </>
      )}
    </div>
  );
}
