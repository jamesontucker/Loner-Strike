import { useEffect, useRef, useState, type ReactNode } from "react";
import { ControlRoute } from "./ShareControl";
import { copyText, getJson, ON_COMPUTER_ONLY, ON_THIS_COMPUTER, postJson, useActions } from "../api";
import type { ServerInfo } from "../types";
import { BusyButton, Info, LoadError, SectionHead, Skeleton, Switch, useToast } from "../ui";
import { useDirty } from "../store";

type Mode = "caddy" | "tunnel";
type Settings = { enabled: boolean; domain: string; origins: string[]; counter: boolean; mode: Mode; fastdl: boolean };
type Live = { online: boolean; name?: string; map?: string; players?: number; bots?: number; max_players?: number };
type Status = {
  host: string | null;
  settings: Settings;
  /** a tunnel token is saved on this computer (the token itself never comes back) */
  token_saved: boolean;
  /** the app runs its own cloudflared for this server */
  own_tunnel: boolean;
  /** a Cloudflare Tunnel set up by hand on the machine (cloudflared.service) */
  existing_tunnel: "running" | "stopped" | null;
  /** the status service's port on the machine (a tunnel's hostname points at it) */
  port: number;
  suggested_domain: string | null;
  state: "off" | "running" | "not_answering";
  reason: string | null;
  live: Live | null;
  status_url: string | null;
  visit_url: string | null;
  /** the address players download from with fast downloads on */
  fastdl_url: string | null;
  /** which Discord links are saved (the links never come back), and the address people join */
  discord: { live: boolean; busy: boolean; address: string | null };
};
type Reply = { ok?: boolean; error?: string; port?: number; port_moved?: boolean };
type Hook = "live" | "busy";
/** a Discord link: newly pasted text, or "remove" to take the saved one away */
type Form = { domain: string; origins: string; counter: boolean; mode: Mode; fastdl: boolean; token: string; live: string; busy: string; drop: Record<Hook, boolean> };

const post = (path: string, body: unknown = {}) => postJson<Reply>(path, body).catch((): Reply => ({ error: "Couldn't reach the app." }));
const USUAL_PORT = 8090;
/** shared control listens this far above the status service (control/agent.rs PORT_OFFSET) */
const CONTROL_OFFSET = 1000;

/** The sites that usually go with a status address: status.example.com gives
 *  https://example.com and https://www.example.com. */
function sitesFor(domain: string): string[] {
  const labels = domain.trim().toLowerCase().replace(/^https?:\/\//, "").replace(/[/.]+$/, "").split(".").filter(Boolean);
  if (labels.length < 2) return [];
  const parent = (labels.length > 2 ? labels.slice(1) : labels).join(".");
  return [`https://${parent}`, `https://www.${parent}`];
}

const originList = (text: string) => text.split(/[\s,]+/).map((o) => o.trim()).filter(Boolean);

/** The website status page for a server on a Linux machine: live name, map and players for
 *  the host's own site, and an optional visit counter. */
export function WebStatus({ server }: { server: ServerInfo }) {
  const toast = useToast();
  const { run, busy } = useActions();
  const [st, setSt] = useState<Status | null>(null);
  const [form, setForm] = useState<Form | null>(null);
  const [failed, setFailed] = useState("");
  const key = (what: string) => `webstatus:${server.id}:${what}`;
  const installing = busy(key("install"));
  const turningOff = busy(key("off"));
  const working = installing || turningOff;
  const dirty = useRef(false);
  const [isDirty, setIsDirty] = useState(false);
  const markDirty = (v: boolean) => {
    dirty.current = v;
    setIsDirty(v);
  };
  useDirty("webstatus", isDirty);

  const load = async () => {
    const r = await getJson<Status & { error?: string }>(`/api/servers/${server.id}/webstatus`).catch(() => null);
    if (!r || !r.settings) return setFailed(r?.error || "Couldn't reach the app.");
    setFailed("");
    setSt(r);
    if (!dirty.current) {
      const s = r.settings;
      // nothing entered yet: the usual sites for the address, ready to change
      const origins = s.origins.length ? s.origins : s.domain ? sitesFor(s.domain) : [];
      setForm({ domain: s.domain, origins: origins.join("\n"), counter: s.counter, mode: s.mode, fastdl: s.fastdl, token: "", live: "", busy: "", drop: { live: false, busy: false } });
    }
  };
  useEffect(() => {
    markDirty(false);
    setSt(null);
    setForm(null);
    let t = 0;
    let live = true;
    // it asks the internet each time, so not too often
    const tick = async () => {
      if (!document.hidden) await load();
      if (live) t = window.setTimeout(tick, 15000);
    };
    tick();
    return () => {
      live = false;
      clearTimeout(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [server.id]);

  const set = (patch: Partial<Form>) => {
    markDirty(true);
    setForm((f) => (f ? { ...f, ...patch } : f));
  };
  // the address picked and no sites yet: the usual ones for it
  const fillSites = () => {
    if (form && !form.origins.trim()) {
      const sites = sitesFor(form.domain);
      if (sites.length) set({ origins: sites.join("\n") });
    }
  };

  // a saved secret, for the eye (the app only answers on the computer)
  const secret = async (kind: string) => {
    const r = await postJson<{ value?: string | null }>(`/api/servers/${server.id}/webstatus/secret`, { kind }).catch(() => ({ value: null }));
    return r.value ?? null;
  };
  const install = () => {
    if (!form || !st || working) return;
    const again = st.settings.enabled;
    run(key("install"), async () => {
      // a Discord link goes only when it's new or taken away; otherwise the saved one stays
      const hook = (h: Hook) => (form[h].trim() ? form[h] : form.drop[h] ? "" : undefined);
      const r = await post(`/api/servers/${server.id}/webstatus`, { enabled: true, domain: form.domain, origins: originList(form.origins), counter: form.counter, mode: form.mode, fastdl: form.fastdl, token: form.token, discord_live: hook("live"), discord_busy: hook("busy") });
      if (r.error) {
        toast(r.error, true);
        load();
        return;
      }
      markDirty(false);
      if (r.port_moved && form.mode === "tunnel") toast(`The status service moved to port ${r.port}. In Cloudflare, point the hostname at http://localhost:${r.port}.`, true);
      else toast(again ? "The status page is updated" : form.mode === "caddy" ? "The status page is set up. The HTTPS certificate can take a minute." : "The status page is set up");
      load();
    }, again ? `Updating the status page on ${server.name}…` : `Setting up the status page on ${server.name}…`);
  };
  const turnOff = () => {
    if (!form || !st || working) return;
    run(key("off"), async () => {
      // only turns it off: edits not saved yet stay in the form, unsaved
      const s = st.settings;
      const r = await post(`/api/servers/${server.id}/webstatus`, { enabled: false, domain: s.domain, origins: s.origins, counter: s.counter, mode: s.mode, fastdl: s.fastdl });
      if (r.error) return toast(r.error, true);
      toast(dirty.current ? "The status page is off. Your other changes aren't saved yet." : "The status page is off");
      load();
    }, `Turning off the status page on ${server.name}…`);
  };

  if (!st || !form) {
    return (
      <div className="stack">
        <SectionHead title="Website status" />
        {failed ? <LoadError text={failed} onRetry={load} /> : <Skeleton rows={4} />}
      </div>
    );
  }

  const saved = st.settings;
  const domain = form.domain.trim().toLowerCase() || "your status address";
  const tunnel = form.mode === "tunnel";
  const usesExisting = tunnel && st.existing_tunnel === "running" && !st.own_tunnel;
  const service = `http://localhost:${st.port}`;
  // it sets up a service on the server's machine and keeps a tunnel token: from the computer only
  const ro = !ON_THIS_COMPUTER;
  const tag = installing ? "Setting up" : turningOff ? "Turning off" : st.state === "running" ? "Running" : st.state === "not_answering" ? "Not answering" : "Off";

  return (
    <div className="stack hltv">
      <SectionHead
        title="Website status"
        note="A small service on the server that tells your website this server's name, map and players, so the site can show them live. It can also count visits, without cookies and without keeping anyone's address."
      />
      {failed && <p className="muted small-note" role="status">Can't reach the app. Showing what it last reported.</p>}

      <div className="hltv-grid">
        <div className="panel">
          <div className="row between">
            <h3>Status page</h3>
            <span className={`live${st.state === "running" && !working ? " on" : ""}`}>{tag}</span>
          </div>
          {!saved.enabled ? (
            <p className="muted">{ro ? "It's off." : "It's off. Fill in the settings and press Install."}</p>
          ) : st.state === "running" ? (
            <>
              <dl className="hltv-facts">
                <Fact label="Game">
                  {st.live?.online ? `${st.live.name}, ${st.live.players} of ${st.live.max_players} playing on ${st.live.map}${st.live.bots ? ` (${st.live.bots} bots)` : ""}` : "The page works. The game server isn't answering it right now, so your site shows it as offline."}
                </Fact>
              </dl>
              <Health id={server.id} />
              <div className="hltv-addresses">
                {st.status_url && <CopyRow label="Status address" value={st.status_url} />}
                {st.visit_url && <CopyRow label="Visit counter address" value={st.visit_url} />}
                {saved.fastdl && st.fastdl_url && <CopyRow label="Fast downloads" value={st.fastdl_url} />}
                <p className="muted small-note">
                  Paste the status address where your site asks for it.{st.visit_url ? " The counter address goes where it asks for a visit counter." : ""}
                </p>
              </div>
            </>
          ) : (
            <>
              {st.reason && <p className="err-text">{st.reason}</p>}
              <p className="muted small-note">Checked from this computer every 15 seconds.</p>
            </>
          )}
          {saved.enabled && !ro && (
            <div className="row">
              <BusyButton className="btn danger" busy={turningOff} disabled={working} label="Turn off" busyLabel="Turning off…" onClick={turnOff} />
            </div>
          )}
        </div>

        <div className="panel">
          <h3>Settings</h3>
          {ro && <p className="muted small-note">{ON_COMPUTER_ONLY}</p>}
          <fieldset className="plain" disabled={ro}>
          <label className="field">
            <span>Address<Info text="A name for the status page, on a domain you own. It needs its own name, separate from the one players connect to." /></span>
            <input className="search" placeholder={st.suggested_domain ?? "status.example.com"} value={form.domain} onChange={(e) => set({ domain: e.target.value })} onBlur={fillSites} spellCheck={false} autoComplete="off" />
          </label>

          <div className="field">
            <span>How it's reached</span>
            <div className="seg" role="group" aria-label="How it's reached">
              <button type="button" aria-pressed={!tunnel} onClick={() => set({ mode: "caddy" })}>Its own HTTPS (Caddy)</button>
              <button type="button" aria-pressed={tunnel} onClick={() => set({ mode: "tunnel" })}>Cloudflare Tunnel</button>
            </div>
            <small>
              {tunnel
                ? "Cloudflare carries the page to the server through a tunnel, so no ports are opened."
                : "The server answers on its own, with a free HTTPS certificate. Opens TCP ports 80 and 443."}
            </small>
          </div>

          {!tunnel ? (
            <div className="hltv-addresses">
              <p className="muted small-note">
                Add a DNS record for {domain}: type A, pointing at {st.host ?? "the server's IP address"}. With Cloudflare, use the grey cloud (DNS only), or the orange cloud with SSL/TLS mode set to Full.
              </p>
              <p className="muted small-note">
                The app opens TCP 80 and 443 on the machine. If your provider has its own firewall (Hetzner's Cloud Firewall, for example), allow TCP 80 and 443 there too. Caddy gets the certificate by itself once the name points at the server.
              </p>
            </div>
          ) : usesExisting ? (
            <div className="hltv-addresses">
              <p className="muted">Using the Cloudflare Tunnel already on this machine.</p>
              <p className="muted small-note">In Cloudflare, the tunnel's public hostname {domain} should point at <code>{service}</code>.</p>
              {st.port !== USUAL_PORT && <PortNote port={st.port} />}
              <ControlRoute domain={domain} port={st.port + CONTROL_OFFSET} />
            </div>
          ) : (
            <div className="hltv-addresses">
              {st.existing_tunnel === "stopped" && (
                <p className="err-text small-note">There's a Cloudflare Tunnel on this machine, but it isn't running. Start it with sudo systemctl start cloudflared, or paste a token below for a new one.</p>
              )}
              <ol className="muted small-note">
                <li>In Cloudflare, open Zero Trust, then Networks, then Tunnels, and create a tunnel of type Cloudflared.</li>
                <li>Add a public hostname: {domain}, with the service <code>{service}</code>.</li>
                <li>On the Install connector page, copy the token (the long text after cloudflared service install) and paste it here.</li>
              </ol>
              {st.port !== USUAL_PORT && <PortNote port={st.port} />}
              <ControlRoute domain={domain} port={st.port + CONTROL_OFFSET} />
              <label className="field">
                <span>Tunnel token<Info text="Kept on this computer and on the server, readable only by their administrators. It isn't shown again after it's saved." /></span>
                <SecretInput placeholder={st.token_saved ? "Saved" : "Paste the token"} value={form.token} onChange={(v) => set({ token: v })} label="Tunnel token" saved={st.token_saved ? () => secret("token") : undefined} />
              </label>
            </div>
          )}

          <div className="field">
            <span>Sites that may show it<Info text="Your site's address. Browsers only let these sites read the status. One per line." /></span>
            <textarea className="invite" style={{ minHeight: 84 }} placeholder="https://example.com" value={form.origins} onChange={(e) => set({ origins: e.target.value })} spellCheck={false} aria-label="Sites that may show it" />
            <PagesPreviews onAdd={(o) => set({ origins: [...originList(form.origins).filter((x) => x !== o), o].join("\n") })} />
          </div>

          <div className="setting">
            <span className="name">Visit counter<span className="note">Counts each visit to your site. Keeps no addresses and sets no cookies</span></span>
            <Switch on={form.counter} disabled={working} onChange={(v) => set({ counter: v })} label="Visit counter" />
          </div>

          <div className="setting">
            <span className="name">Fast downloads<span className="note">Players get missing maps, sounds and models from here in seconds, instead of slowly from the game. Sets the server's download address for you</span></span>
            <Switch on={form.fastdl} disabled={working} onChange={(v) => set({ fastdl: v })} label="Fast downloads" />
          </div>

          <div className="field">
            <span>Discord<Info text="Uses Discord webhooks: in a channel's settings, open Integrations, then Webhooks, make one and copy its URL. Kept on this computer and on the server, readable only by their administrators. Not shown again after it's saved." /></span>
            <small>
              Keeps a message in a channel up to date with who's playing, and posts when people come on to play, at most once an hour.
              {!st.discord.address && " Set a public address for the server first, so the messages can say how to join."}
            </small>
          </div>
          <HookField label="Live status message" what="the channel for the live message" reveal={() => secret("live")} saved={st.discord.live} value={form.live} drop={form.drop.live} disabled={working} onChange={(v) => set({ live: v })} onDrop={(d) => set({ drop: { ...form.drop, live: d } })} />
          <HookField label="When people come on" what="the channel people chat in" reveal={() => secret("busy")} saved={st.discord.busy} value={form.busy} drop={form.drop.busy} disabled={working} onChange={(v) => set({ busy: v })} onDrop={(d) => set({ drop: { ...form.drop, busy: d } })} />

          </fieldset>
          {!ro && (
          <div className="row">
            <BusyButton
              className="btn primary"
              busy={installing}
              disabled={working}
              label={saved.enabled ? "Save and update" : "Install"}
              busyLabel={saved.enabled ? "Updating the status page…" : "Setting up the status page…"}
              onClick={install}
            />
            {isDirty && !working && <span className="muted small-note">Unsaved changes</span>}
          </div>
          )}
          {installing && <p className="muted small-note">Setting up can take a minute or two the first time, while it installs {tunnel && !usesExisting ? "cloudflared" : tunnel ? "the service" : "Caddy"}.</p>}
        </div>
      </div>
    </div>
  );
}

/** One Discord webhook link: pasted once, then only "Saved" with a way to take it away. */
function HookField({ label, what, saved, value, drop, disabled, onChange, onDrop, reveal }: { label: string; what: string; saved: boolean; value: string; drop: boolean; disabled: boolean; onChange: (v: string) => void; onDrop: (d: boolean) => void; reveal: () => Promise<string | null> }) {
  return (
    <label className="field">
      <span>{label}</span>
      <div className="row">
        <SecretInput placeholder={drop ? "Removed when you save" : saved ? "Saved" : `Webhook URL for ${what}`} value={value} disabled={disabled || drop} onChange={onChange} label={label} saved={saved ? reveal : undefined} />
        {saved && !value && (
          <button type="button" className="btn small" disabled={disabled} onClick={() => onDrop(!drop)}>
            {drop ? "Keep" : "Remove"}
          </button>
        )}
      </div>
    </label>
  );
}

/** A field for a secret, hidden as you paste it, with an eye to check what's there. A saved
 *  secret never comes back from the app, so there's only ever what was just typed to show. */
function SecretInput({ value, onChange, placeholder, label, disabled, saved }: { value: string; onChange: (v: string) => void; placeholder: string; label: string; disabled?: boolean; saved?: () => Promise<string | null> }) {
  const [shown, setShown] = useState(false);
  // the saved one, asked for when the eye opens on an empty field (shown, not put in the form)
  const [peek, setPeek] = useState<string | null>(null);
  const toggle = async () => {
    if (shown) return setShown(false);
    if (!value && saved) setPeek(await saved());
    setShown(true);
  };
  const showing = shown && !value && peek ? peek : value;
  return (
    <div className="reveal">
      <input className="search" type={shown ? "text" : "password"} placeholder={placeholder} value={showing} readOnly={showing !== value} disabled={disabled} onChange={(e) => onChange(e.target.value)} spellCheck={false} autoComplete="off" aria-label={label} />
      <button type="button" className="btn small" disabled={disabled || (!value && !saved)} aria-pressed={shown} aria-label={shown ? `Hide ${label.toLowerCase()}` : `Show ${label.toLowerCase()}`} title={shown ? "Hide" : "Show"} onClick={toggle}>
        <svg width="18" height="14" viewBox="0 0 18 14" aria-hidden="true">
          <path d="M1 7c2-3.6 4.8-5.5 8-5.5S15 3.4 17 7c-2 3.6-4.8 5.5-8 5.5S3 10.6 1 7z" />
          <circle cx="9" cy="7" r="2.4" />
          {shown && <path d="M2.5 1.5l13 11" />}
        </svg>
      </button>
    </div>
  );
}

function PortNote({ port }: { port: number }) {
  return <p className="muted small-note">Port {USUAL_PORT} is taken by another server's status page on this machine, so this one uses port {port}. Use <code>http://localhost:{port}</code> in Cloudflare.</p>;
}

/** Adds https://*.<project>.pages.dev, for a site on Cloudflare Pages and its previews. */
function PagesPreviews({ onAdd }: { onAdd: (origin: string) => void }) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const clean = name.trim().toLowerCase().replace(/\.pages\.dev$/, "");
  const ok = /^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$/.test(clean);
  if (!open) return <button type="button" className="btn small" onClick={() => setOpen(true)}>Add Cloudflare Pages previews</button>;
  return (
    <div className="row">
      <input className="search" placeholder="Pages project name" value={name} onChange={(e) => setName(e.target.value)} spellCheck={false} autoComplete="off" aria-label="Pages project name" />
      <button type="button" className="btn small" disabled={!ok} onClick={() => { onAdd(`https://*.${clean}.pages.dev`); setOpen(false); setName(""); }}>Add</button>
      <button type="button" className="btn small" onClick={() => setOpen(false)}>Cancel</button>
    </div>
  );
}

type Check = { ok: boolean; why?: string };
type HealthReply = { control?: Check | null; fastdl?: Check | null; discord?: Record<string, Check>; error?: string };

/** The rest of what's set up, tried from outside when the page opens: shared control, fast
 * downloads, the Discord links. */
function Health({ id }: { id: string }) {
  const [h, setH] = useState<HealthReply | null>(null);
  const [checking, setChecking] = useState(false);
  const check = async () => {
    setChecking(true);
    try {
      setH(await getJson<HealthReply>(`/api/servers/${id}/webstatus/health`));
    } catch {
      setH({ error: "Couldn't reach the app." });
    } finally {
      setChecking(false);
    }
  };
  useEffect(() => {
    void check();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id]);
  const rows: [string, Check][] = [];
  if (h?.control) rows.push(["Shared control", h.control]);
  if (h?.fastdl) rows.push(["Fast downloads", h.fastdl]);
  for (const [k, c] of Object.entries(h?.discord ?? {})) rows.push([k === "live" ? "Discord live message" : "Discord posts", c]);
  if (!checking && h && !h.error && !rows.length) return null;
  return (
    <div className="health">
      {h?.error && <p className="err-text small-note">{h.error}</p>}
      {!h && checking && <p className="muted small-note"><span className="spinner" aria-hidden="true" />Checking the rest from outside…</p>}
      {rows.length > 0 && (
        <dl className="hltv-facts">
          {rows.map(([label, c]) => (
            <Fact key={label} label={label}>
              {c.ok ? <span className="live on">Working</span> : <span className="err-text">Not working: {c.why}</span>}
            </Fact>
          ))}
        </dl>
      )}
      {h && <BusyButton className="btn" busy={checking} label="Check again" busyLabel="Checking…" onClick={() => void check()} />}
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

function CopyRow({ label, value }: { label: string; value: string }) {
  const toast = useToast();
  const copy = async () => {
    if (await copyText(value)) toast("Copied");
    else toast("Couldn't copy. Select the text instead.", true);
  };
  return (
    <div className="copy-row">
      <span className="muted">{label}</span>
      <div>
        <code className="hltv-cmd">{value}</code>
        <button className="btn small" onClick={copy}>Copy</button>
      </div>
    </div>
  );
}
