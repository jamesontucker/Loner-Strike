import { useEffect, useState } from "react";
import { copyText, getJson, getJsonAt, postJson, useActions } from "../api";
import { queryClient, reloadServers } from "../queries";
import type { ServerInfo, ServerList } from "../types";
import { BusyButton, Info, Sheet, Switch, useConfirm, useToast } from "../ui";

/**
 * Shared control (control/ in the app): friends with Loner-Strike help run a server from
 * their own app, as an Admin or a Moderator. The owner's side is the Share control tab;
 * the friend's is Add a shared server.
 */

type Role = "admin" | "moderator";
type Member = { id: string; name: string; role: Role; steam: string; added: string; seen: number | null; invited_by: string; via_app?: boolean };
type Activity = { id?: string; at: number; who: string; member: string; role: Role; path: string; body: Record<string, unknown> | null; can_undo?: boolean; undone?: boolean };
type OpenInvite = { id: string; role: Role; steam: string; expires: number; by: string; via_app?: boolean };
/** mode "server": a Linux server whose shared control runs on the server itself (helpers reach it at url) */
type Control = {
  members?: Member[]; invites?: OpenInvite[]; addresses?: string[]; port?: number; listening?: number | null; activity?: Activity[]; error?: string;
  mode?: "server"; url?: string | null; domain?: string; tunnel?: boolean; reachable?: boolean; reach_error?: string | null;
};
type Reply = { ok?: boolean; error?: string; invite?: string; id?: string; name?: string | null; role?: Role; address?: string; port?: number; need_restart_game?: boolean; left_bans?: boolean };

const post = (path: string, body: unknown = {}) => postJson<Reply>(path, body).catch((): Reply => ({ error: "Couldn't reach the app." }));

export const ROLE_NAME: Record<Role, string> = { admin: "Admin", moderator: "Moderator" };
const ROLE_NOTE: Record<Role, string> = {
  admin: "Settings, modes, rules and bots, maps and sounds, the rotation, restarts, kicks and bans, and inviting Moderators",
  moderator: "Kick, ban (up to 7 days), mute and gag players, change or restart the map, and see the console and chat",
};
const OWNER_ONLY = "Passwords and webhooks, the machine and its files, start options, your own plugins, backups, and deleting the server always stay with you.";

/** A change's name in the log, for Undo: its own id (older entries: who and when) */
const keyOf = (a: Activity) => a.id ?? `${a.member}:${a.at}`;

/** "20261107-142233" -> "7 Nov 2026" */
function made(stamp: string) {
  const m = /^(\d{4})(\d{2})(\d{2})/.exec(stamp ?? "");
  return m ? new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3])).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" }) : "";
}

/** "in 23 hours", "in 40 minutes" */
function until(unix: number) {
  const s = unix - Date.now() / 1000;
  if (s <= 0) return "now";
  return s < 3600 ? `in ${Math.max(1, Math.round(s / 60))} minutes` : `in ${Math.round(s / 3600)} hours`;
}

/** A new invite: hidden like a password until the eye shows it; Copy copies it either way. */
function InviteBox({ invite, note }: { invite: string; note: string }) {
  const toast = useToast();
  const [shown, setShown] = useState(false);
  const copy = async () => toast((await copyText(invite)) ? "Invite copied" : "Couldn't copy it: show it, then select it and copy it yourself", false);
  return (
    <div className="invite-box">
      <div className="reveal">
        <code className="mono invite-code" aria-label={shown ? "Invite" : "Invite, hidden"}>{shown ? invite : `LS1.${"•".repeat(24)}`}</code>
        <button type="button" className="btn small" aria-pressed={shown} aria-label={shown ? "Hide the invite" : "Show the invite"} title={shown ? "Hide" : "Show"} onClick={() => setShown((v) => !v)}>
          <svg width="18" height="14" viewBox="0 0 18 14" aria-hidden="true">
            <path d="M1 7c2-3.6 4.8-5.5 8-5.5S15 3.4 17 7c-2 3.6-4.8 5.5-8 5.5S3 10.6 1 7z" />
            <circle cx="9" cy="7" r="2.4" />
            {shown && <path d="M2.5 1.5l13 11" />}
          </svg>
        </button>
      </div>
      <div className="row">
        <button className="btn primary" onClick={copy}>Copy invite</button>
        <span className="muted small-note">{note}</span>
      </div>
    </div>
  );
}

/** "3 minutes ago", "yesterday", "12 Nov" */
export function ago(unix: number | null | undefined) {
  if (!unix) return "never";
  const s = Date.now() / 1000 - unix;
  if (s < 90) return "just now";
  if (s < 3600) return `${Math.round(s / 60)} minutes ago`;
  if (s < 86400) return `${Math.round(s / 3600)} hours ago`;
  if (s < 172800) return "yesterday";
  return new Date(unix * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

const CMD_WORDS: Record<string, string> = {
  gm_map: "Changed the map to", gm_mode: "Switched the mode to", gm_restartmap: "Restarted the map", gm_announce: "Said to everyone:", gm_set: "Set",
  gm_bots: "Changed the bots:", gm_addon: "Turned an add-on", gm_preset: "Loaded the preset", gm_pause: "Paused or unpaused the game", gm_team: "Changed teams:",
  gm_votemode: "Started a mode vote", gm_player: "Player action:", gm_do: "Did", gm_match: "5v5 match:", gm_nav: "Bot navigation:",
};

/** What a change in the activity log was, in words. */
function describe(a: Activity) {
  const b = (a.body ?? {}) as Record<string, unknown>;
  const p = a.path;
  const args = Array.isArray(b.args) ? (b.args as unknown[]).map(String).join(" ") : "";
  if (p === "/api/cmd") {
    const cmd = String(b.cmd ?? "");
    return `${CMD_WORDS[cmd] ?? `Sent ${cmd}`}${args ? ` ${args}` : ""}`.trim();
  }
  if (/^\/api\/servers\/[^/]+$/.test(p)) return `Changed the server's settings: ${Object.keys(b).join(", ") || "nothing"}`;
  const action = p.match(/^\/api\/servers\/[^/]+\/(start|stop|restart)$/)?.[1];
  if (action) return { start: "Started the server", stop: "Stopped the server", restart: "Restarted the server" }[action]!;
  if (p === "/api/admins/ban") return `Banned ${b.id ?? "a player"} ${Number(b.minutes) > 0 ? `for ${b.minutes} minutes` : "for good"}`;
  if (p === "/api/admins/unban") return `Lifted the ban on ${b.id ?? "a player"}`;
  if (p === "/api/admins/kick") return "Kicked a player";
  if (p === "/api/community/gag") return `Muted ${b.name ?? "a player"}`;
  if (p === "/api/community/ungag") return "Unmuted a player";
  if (p === "/api/gb/install") return "Installed a download from GameBanana";
  if (p === "/api/gb/uninstall") return "Removed a GameBanana download";
  if (p.startsWith("/api/extras")) return "Changed the rotation, schedule or rules";
  if (p === "/api/welcome") return "Changed the welcome page";
  if (p.startsWith("/api/sounds")) return "Changed the sounds";
  if (p.startsWith("/api/servercontent")) return "Changed the skins and models";
  if (p === "/api/app/mode-settings") return `Set ${b.cvar ?? "a setting"} to ${b.value ?? ""} for ${b.mode ?? "a mode"}`;
  if (p === "/api/community/reports") return "Marked a report handled or reopened it";
  if (p.startsWith("/api/community")) return "Changed stats and moderation";
  if (p === "/api/mapflag") return "Changed a map's favourite or hidden mark";
  if (p === "/api/control/redeem") return "Accepted an invite";
  if (p === "/api/control/invite") return "Made an invite for a Moderator";
  if (/^\/api\/servers\/[^/]+\/console\/send$/.test(p)) return `Typed in the console: ${String(b.command ?? "").slice(0, 80)}`;
  if (p === "/api/modevote") return "Changed the modes in the mode vote";
  if (p.startsWith("/api/scrim")) return "Changed a 5v5 match";
  return p;
}

/** A Linux server's shared control: where helpers' apps reach it, and whether they can. */
function ServerReach({ c }: { c: Control }) {
  if (!c.url) {
    return <p className="muted small-note">Helpers connect to the server itself at its website status address, so this computer can be off. Set up Website status for this server in Overview first.</p>;
  }
  return (
    <div className="stack">
      <p className="muted small-note">
        Helpers connect to the server itself at <code className="mono">{c.url}</code>, so this computer can be off. Only invite people you trust.
      </p>
      {c.tunnel && (
        <p className="muted small-note">
          With a Cloudflare tunnel, add this in Cloudflare too: on the tunnel's public hostname {c.domain}, path <code className="mono">control/*</code>, service <code className="mono">http://localhost:{c.port}</code>.
        </p>
      )}
      {!c.reachable && (
        <p className="err-text small-note">Helpers' apps can't reach it there yet{c.reach_error ? ` (${c.reach_error})` : ""}. Check Website status, then open this tab again.</p>
      )}
    </div>
  );
}

/** The owner's tab: who helps run this server, inviting someone, and what they changed. */
export function ShareControl({ server }: { server: ServerInfo }) {
  const toast = useToast();
  const ask = useConfirm();
  const [c, setC] = useState<Control | null>(null);
  const [role, setRole] = useState<Role>("moderator");
  const [steam, setSteam] = useState("");
  const [addresses, setAddresses] = useState("");
  const [invite, setInvite] = useState("");
  // the new invite's id: cancelling that one (not another) clears it
  const [inviteId, setInviteId] = useState("");
  const [suggest, setSuggest] = useState<string[]>([]);
  // a notification when a helper starts, stops or restarts it, or adds or removes a download
  const [helperNotes, setHelperNotesState] = useState(server.notify_helpers !== false);
  const setHelperNotes = (v: boolean) => run(`${server.id}:notify`, async () => {
    setHelperNotesState(v);
    const r = await post(`/api/servers/${server.id}`, { notify_helpers: v });
    if (r.error) {
      toast(r.error, true);
      // it may have been saved before the error: the switch shows what's saved
      await reloadServers();
      const now = queryClient.getQueryData<ServerList>(["servers"])?.servers.find((x) => x.id === server.id);
      return setHelperNotesState(now ? now.notify_helpers !== false : !v);
    }
    void reloadServers();
  });
  const { run, busy } = useActions();
  const load = () =>
    getJsonAt<Control>(`/api/control/server/${server.id}`, null)
      .then((r) => {
        setC(r);
        if (r.addresses?.length) setAddresses((a) => a || r.addresses!.join(", "));
      })
      .catch(() => setC({ error: "Couldn't reach the app." }));
  useEffect(() => {
    load();
    // addresses this computer has that friends might reach: its network, virtual LANs
    Promise.all([getJson<{ ip?: string | null }>("/api/app/lan").catch(() => ({ ip: null })), getJson<Record<string, string>>("/api/app/vlans").catch(() => ({}))]).then(([lan, vlans]) => {
      const ips = [...Object.values(vlans).map((v) => String(v).split(":")[0]), lan.ip ?? ""].filter(Boolean);
      setSuggest([...new Set(ips)]);
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [server.id]);

  const list = addresses.split(/[\s,]+/).map((a) => a.trim()).filter(Boolean);
  // a Linux server: helpers reach the server itself, at its website status address
  const onServer = c?.mode === "server";
  const make = () => run(`${server.id}:invite`, async () => {
    if (!onServer && !list.length) return toast("Add at least one address friends reach this computer at.", true);
    const r = await post(`/api/control/server/${server.id}/invite`, { role, steam: steam.trim(), addresses: list });
    if (r.error) return toast(r.error, true);
    setInvite(r.invite ?? "");
    setInviteId(r.id ?? "");
    setSteam("");
    load();
  }, "Making an invite…");
  const setMemberRole = (m: Member, r: Role) => run(`member:${m.id}`, async () => {
    const x = await post(`/api/control/member/${m.id}`, { role: r, server: server.id });
    if (x.error) return toast(x.error, true);
    toast(`${m.name} is now ${r === "admin" ? "an Admin" : "a Moderator"}`);
    load();
  });
  const remove = (m: Member) => run(`member:${m.id}`, async () => {
    if (!(await ask(`Take away ${m.name}'s access to ${server.name}? Their app stops working on it straight away.`, { ok: "Take away access", danger: true }))) return;
    const x = await post(`/api/control/member/${m.id}/remove`, { server: server.id });
    if (x.error) return toast(x.error, true);
    toast(x.left_bans ? `${m.name} no longer has access. ${server.name} stopped sharing bans with your other servers, so changes made while helpers ran it stay there; turn sharing back on in its Admins tab.` : `${m.name} no longer has access`);
    load();
    // they could still join with the password their game kept: a new one shuts them out
    if (server.password && (await ask(`Change ${server.name}'s join password too? ${m.name}'s game may still have the old one.`, { ok: "Make a new password" }))) {
      const pw = Array.from(crypto.getRandomValues(new Uint8Array(8)), (b) => "abcdefghjkmnpqrstuvwxyz23456789"[b % 31]).join("");
      const r = await post(`/api/servers/${server.id}`, { password: pw });
      if (r.error) return toast(r.error, true);
      toast(`New join password: ${pw}. Join in this app uses it for you; tell the people you play with.`);
      void reloadServers();
    }
  });
  const cancel = (i: OpenInvite) => run(`invite:${i.id}`, async () => {
    if (!(await ask("Cancel this invite? Whoever has it can't use it any more.", { ok: "Cancel the invite", danger: true }))) return;
    const r = await post(`/api/control/invite/${i.id}/cancel`, { server: server.id });
    if (r.error) return toast(r.error, true);
    toast("Invite cancelled");
    if (i.id === inviteId) setInvite("");
    load();
  });
  const undo = (a: Activity) => run(`undo:${keyOf(a)}`, async () => {
    const r = await post(`/api/control/server/${server.id}/undo`, { key: keyOf(a) });
    if (r.error) return toast(r.error, true);
    toast("Put back");
    load();
  }, "Putting it back…");
  const openPort = () => run("share:upnp", async () => {
    const ok = await ask(
      `Open TCP port ${c?.port ?? 27030} on your router? Only people with an invite get in, but invite only people you trust. It closes within an hour after you quit Loner-Strike.`,
      { ok: "Open the port", danger: true },
    );
    if (!ok) return;
    const r = await post("/api/control/open-port");
    if (r.error) return toast(r.error, true);
    setAddresses((a) => (a.split(/[\s,]+/).includes(r.address!) ? a : [r.address, a].filter(Boolean).join(", ")));
    toast(`Your router forwards port ${r.port} to this computer. Your internet address ${r.address} was added.`);
    load();
  }, "Asking your router…");
  const [portText, setPortText] = useState("");
  const setPort = () => run("share:port", async () => {
    const p = Number(portText);
    if (!Number.isInteger(p) || p < 1024 || p > 65535 || (p >= 27015 && p <= 27020)) return toast("Pick a port from 1024 to 65535 that isn't a game port (27015 to 27020).", true);
    if (members.length && !(await ask(`Move sharing to port ${p}? Everyone who has access now loses it until you send them a new invite.`, { ok: "Move it", danger: true }))) return;
    const r = await post("/api/control/port", { port: p });
    if (r.error) return toast(r.error, true);
    setPortText("");
    toast(`Friends' apps now reach this computer on port ${p}. Send new invites: older ones, and people who had access, use the old port.`);
    load();
  });

  const members = c?.members ?? [];
  return (
    <div className="stack">
      <section>
        <div className="section-head"><h2>Shared control</h2>
          <p>Friends who have Loner-Strike can help run {server.name} from their own app, as an Admin or a Moderator. {OWNER_ONLY}</p>
        </div>
        {c?.error && <p className="err-text">{c.error}</p>}
        <div className="panel">
          <div className="row between">
            <h3 style={{ margin: 0 }}>People who can run it</h3>
            <label className="check-row">
              <Switch on={helperNotes} label="Tell me when helpers start, stop or restart it, or change downloads" disabled={busy(`${server.id}:notify`)} onChange={setHelperNotes} />
              <span className="small-note">Tell me when helpers start, stop or restart it, or change downloads</span>
            </label>
          </div>
          {!members.length && <p className="muted">Nobody yet. Invite someone below.</p>}
          {members.map((m) => (
            <div key={m.id} className="row between share-person">
              <div>
                <b>{m.name}</b> <span className={`tag role-${m.role}`}>{ROLE_NAME[m.role]}</span>
                <p className="muted small-note">
                  {m.steam ? `${m.steam} · ` : ""}added {made(m.added)} · last here {ago(m.seen)}{m.invited_by && m.invited_by !== "owner" ? " · invited by an Admin" : ""}
                  {onServer && m.via_app ? " · connects through this computer" : ""}
                </p>
              </div>
              <div className="row">
                <select aria-label={`${m.name}'s role`} value={m.role} disabled={busy(`member:${m.id}`)} onChange={(e) => setMemberRole(m, e.target.value as Role)}>
                  <option value="admin">Admin</option>
                  <option value="moderator">Moderator</option>
                </select>
                <BusyButton className="btn small danger" busy={busy(`member:${m.id}`)} label="Remove" busyLabel="Removing…" onClick={() => remove(m)} />
              </div>
            </div>
          ))}
        </div>
      </section>

      <section className="panel">
        <h3>Invite someone</h3>
        <div className="methods" role="radiogroup" aria-label="Their role">
          {(["admin", "moderator"] as Role[]).map((r) => (
            <button key={r} type="button" className="method" role="radio" aria-checked={role === r} onClick={() => setRole(r)}>
              <span className="method-name">{ROLE_NAME[r]}</span>
              <span className="method-blurb">{ROLE_NOTE[r]}</span>
            </button>
          ))}
        </div>
        <div className="row" style={{ marginTop: 12, alignItems: "start" }}>
          <label className="field" style={{ flex: "1 1 240px" }}>
            <span>Their SteamID (optional)<Info text="Makes them an in-game admin on this server too, with the same powers. It looks like STEAM_0:1:12345: they find it by typing status in the game's console." /></span>
            <input className="search" placeholder="STEAM_0:1:12345" value={steam} onChange={(e) => setSteam(e.target.value.trim())} spellCheck={false} />
          </label>
          {!onServer && <div className="field" style={{ flex: "2 1 320px" }}>
            <label htmlFor="share-addresses">Where friends reach this computer<Info text={`Their Loner-Strike connects to this computer on TCP port ${c?.port ?? 27030}. Use your internet address (with that port forwarded on your router), a virtual LAN address you're both on (ZeroTier, Radmin VPN, Hamachi), or a tunnel's https:// address. Several, separated by commas, are tried in order.`} /></label>
            <input id="share-addresses" className="search" placeholder="Your internet address, or a virtual LAN address" value={addresses} onChange={(e) => setAddresses(e.target.value)} spellCheck={false} />
            <span className="row">
              <BusyButton className="btn small" busy={busy("share:upnp")} label="Open the port on my router" busyLabel="Asking…" onClick={openPort} />
              <span className="muted small-note">Port {c?.port ?? 27030} (TCP){c?.listening ? "" : " · not listening yet"}</span>
              <input className="search narrow" type="number" min={1024} max={65535} placeholder="Change" aria-label="Sharing port" value={portText} onChange={(e) => setPortText(e.target.value)} />
              {portText && <BusyButton className="btn small" busy={busy("share:port")} label="Use it" busyLabel="Moving…" onClick={setPort} />}
            </span>
            {suggest.length > 0 && (
              <small>
                On this computer:{" "}
                {suggest.map((s, i) => (
                  <span key={s}>
                    {i > 0 && ", "}
                    <button type="button" className="link inline" onClick={() => setAddresses((a) => (a.split(/[\s,]+/).includes(s) ? a : [a, s].filter(Boolean).join(", ")))}>{s}</button>
                  </span>
                ))}
              </small>
            )}
          </div>}
        </div>
        {onServer ? (
          <ServerReach c={c!} />
        ) : (
          <p className="muted small-note">
            Friends reach this computer while Loner-Strike is open. Windows may ask to let Loner-Strike through its firewall the first time: allow it, or their app can't connect. Only invite people you trust.
          </p>
        )}
        <div className="row">
          <BusyButton className="btn primary" busy={busy(`${server.id}:invite`)} disabled={onServer && !c?.url} label="Make an invite" busyLabel="Making…" onClick={make} />
        </div>
        {invite && <InviteBox invite={invite} note="Send it to them however you like. It works once, for one person, for 24 hours. They paste it in Servers, Add a shared server. It's only shown now: if it gets out, cancel it below." />}
        {!!c?.invites?.length && (
          <div className="share-invites">
            <b>Invites not used yet</b>
            {c.invites.map((i) => (
              <div key={i.id || i.expires} className="row between share-person">
                <span>
                  <span className={`tag role-${i.role}`}>{ROLE_NAME[i.role]}</span>
                  <span className="muted small-note"> {i.steam ? `${i.steam} · ` : ""}made by {i.by} · runs out {until(i.expires)}</span>
                </span>
                {i.id && <BusyButton className="btn small danger" busy={busy(`invite:${i.id}`)} label="Cancel" busyLabel="Cancelling…" onClick={() => cancel(i)} />}
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="panel">
        <h3>What people changed</h3>
        {!(c?.activity ?? []).length && <p className="muted">Nothing yet.</p>}
        <div className="share-log">
          {(c?.activity ?? []).map((a) => (
            <div key={keyOf(a)} className="share-log-row">
              <span className="muted">{ago(a.at)}</span>
              <span><b>{a.who}</b> <span className={`tag role-${a.role}`}>{ROLE_NAME[a.role]}</span></span>
              <span>{describe(a)}</span>
              <span>
                {a.undone ? <span className="tag">Undone</span> : a.can_undo ? <BusyButton className="btn small" busy={busy(`undo:${keyOf(a)}`)} label="Undo" busyLabel="Undoing…" onClick={() => undo(a)} /> : null}
              </span>
            </div>
          ))}
        </div>
      </section>
    </div>
  );
}

/** The friend's side: paste an invite, and the server joins your list. */
export function AddShared({ initial = "", onClose, onAdded }: { initial?: string; onClose: () => void; onAdded: (id: string) => void }) {
  const toast = useToast();
  const [invite, setInvite] = useState(initial);
  const [name, setName] = useState(() => {
    try {
      return localStorage.getItem("ls.sharedName") ?? "";
    } catch {
      return "";
    }
  });
  const [problem, setProblem] = useState("");
  // your Steam name, unless you typed one before
  useEffect(() => {
    if (name) return;
    getJson<Reply>("/api/control/whoami").then((r) => r.name && setName((n) => n || r.name!)).catch(() => {});
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const { run, busy } = useActions();
  const looks = invite.trim().startsWith("LS1.");
  const add = () => run("shared:add", async () => {
    setProblem("");
    try {
      localStorage.setItem("ls.sharedName", name.trim());
    } catch {
      // not remembered
    }
    const r = await post("/api/control/accept", { invite: invite.trim(), name: name.trim() });
    if (r.error) return setProblem(r.error);
    toast(`${r.name} is in your servers. You're ${r.role === "admin" ? "an Admin" : "a Moderator"} there.`);
    onAdded(r.id!);
  }, "Connecting to the owner's Loner-Strike…");
  return (
    <Sheet label="Add a shared server" onClose={busy("shared:add") ? undefined : onClose}>
      <div className="sheet-body">
        <h2>Add a shared server</h2>
        <p className="muted">Someone who runs a server with Loner-Strike sent you an invite. Paste it here. For a server on their own computer, their Loner-Strike has to be open.</p>
        <label className="field">
          <span>Invite</span>
          <textarea className="search invite-input" rows={3} placeholder="LS1.…" value={invite} onChange={(e) => { setInvite(e.target.value); setProblem(""); }} spellCheck={false} aria-invalid={!!problem || (!!invite.trim() && !looks)} />
          {invite.trim() && !looks && <small className="err-text">That doesn't look like an invite. It starts with LS1.</small>}
        </label>
        <label className="field">
          <span>Your name<Info text="What the owner sees in their list of people, and next to what you change." /></span>
          <input className="search" maxLength={32} placeholder="Like your Steam name" value={name} onChange={(e) => setName(e.target.value)} />
        </label>
        {problem && <p className="err-text" role="alert">{problem}</p>}
        <div className="row">
          <BusyButton className="btn primary" busy={busy("shared:add")} disabled={!looks} label="Add the server" busyLabel="Connecting…" onClick={add} />
          <button className="btn" disabled={busy("shared:add")} onClick={onClose}>Cancel</button>
        </div>
      </div>
    </Sheet>
  );
}

/** A shared server's Admin inviting a Moderator (the owner's app makes the invite). */
export function InviteModerator({ server }: { server: ServerInfo }) {
  const toast = useToast();
  const [steam, setSteam] = useState("");
  const [invite, setInvite] = useState("");
  const { run, busy } = useActions();
  const make = () => run(`${server.id}:invite`, async () => {
    const r = await post("/api/control/invite", { steam: steam.trim() });
    if (r.error) return toast(r.error, true);
    setInvite(r.invite ?? "");
    setSteam("");
  }, "Making an invite…");
  return (
    <section className="panel">
      <h3>Invite a Moderator</h3>
      <p className="muted measure">As an Admin you can bring in Moderators to help: {ROLE_NOTE.moderator.toLowerCase()}. The owner sees who you invite, and can remove them.</p>
      <label className="field">
        <span>Their SteamID (optional)<Info text="Makes them an in-game Moderator on this server too. It looks like STEAM_0:1:12345." /></span>
        <input className="search" placeholder="STEAM_0:1:12345" value={steam} onChange={(e) => setSteam(e.target.value.trim())} spellCheck={false} />
      </label>
      <div className="row">
        <BusyButton className="btn primary" busy={busy(`${server.id}:invite`)} label="Make an invite" busyLabel="Making…" onClick={make} />
      </div>
      {invite && <InviteBox invite={invite} note="It works once, for one person, for 24 hours. They paste it in Servers, Add a shared server. If it gets out, ask the owner to cancel it." />}
    </section>
  );
}
