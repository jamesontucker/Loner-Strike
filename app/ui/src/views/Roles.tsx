// Community roles in the app (P5; docs/notes/Roles.md, the routes are MCP's): on a server's
// Shared control page the "Loner-Strike roles" card (claim, the Discord owner code, the three
// switches) and the moderator applications (Create moderator invite, Revoke); an Admin of a
// shared server sees the applications only. Also the supporters' VIP settings (Admins › VIP &
// donations) and a short "Link your Steam account to Discord" help.

import { useCallback, useEffect, useState } from "react";
import { copyText, getJson, postJson, useActions } from "../api";
import type { ServerInfo } from "../types";
import { BusyButton, Info, LoadError, Switch, useConfirm, useToast } from "../ui";
import { InviteBox } from "./ShareControl";
import { DocsLink } from "./DocsLink";
import { actionsFor, answerPairs, codeShown, inTime, isState, sortApplications, STATE_TONE, STATE_WORD, type Application, type RolesServer, type RolesSettings } from "./communityRoles";
import "./roles.css";

type Answer = { ok?: boolean; error?: string };
const post = <T extends Answer>(path: string, body: unknown = {}) => postJson<T>(path, body).catch((): T => ({ error: "Couldn't reach the app." }) as T);
const get = <T extends Answer>(path: string) => getJson<T>(path).catch((): T => ({ error: "Couldn't reach the app." }) as T);

const DISCORD = "https://discord.gg/K9eBHVVwZE";

/** How /link works, for players and for the people who run servers. */
export function LinkHelp({ compact }: { compact?: boolean }) {
  return (
    <details className="panel roles-link" open={!compact}>
      <summary>Link your Steam account to Discord</summary>
      <ol>
        <li>
          On any Loner-Strike server, say <b>/link</b> in the game chat. You get a code like <code>ABCD-EFGH</code>, good for 10 minutes.
        </li>
        <li>
          In the{" "}
          <a href={DISCORD} target="_blank" rel="noreferrer">
            Loner-Strike Discord
          </a>
          {" "}server, type <code>/link code:ABCD-EFGH</code> in any channel (not in a DM to the bot). Only you see the reply.
        </li>
      </ol>
      <p className="muted small-note">
        That ties your Discord account to your SteamID, one to one. Supporter VIP and applying to moderate a server need it. <code>/link</code> alone in Discord says what you're
        linked to; <code>/unlink</code> undoes it. A server that isn't claimed, or whose owner's Loner-Strike is closed, can't make codes: use an official server then. To moderate
        an official server, link on an official server.
      </p>
    </details>
  );
}

/** The owner's card: claim the server, the Discord owner code, the switches, unclaim. */
export function RolesCard({ server }: { server: ServerInfo }) {
  const toast = useToast();
  const ask = useConfirm();
  const { run, busy } = useActions();
  const [s, setS] = useState<RolesServer | null>(null);
  const [problem, setProblem] = useState("");
  const base = `/api/roles/server/${encodeURIComponent(server.id)}`;
  const load = useCallback(async () => {
    const r = await get<RolesServer & Answer>(base);
    if (r.error && r.claimed === undefined) setProblem(r.error);
    else {
      setProblem("");
      setS(r);
    }
  }, [base]);
  useEffect(() => {
    void load();
  }, [load]);

  const claim = () =>
    run(`roles:${server.id}:claim`, async () => {
      setProblem("");
      const r = await post<Answer & { owner_code?: string; expires?: number }>(`${base}/claim`);
      if (r.error) return setProblem(r.error);
      toast("Claimed. Finish it in the Loner-Strike Discord with the code below.");
      await load();
      if (r.owner_code) setS((x) => (x ? { ...x, owner_code: r.owner_code, owner_code_expires: r.expires ?? x.owner_code_expires } : x));
    }, "Claiming the server…");
  const newCode = () =>
    run(`roles:${server.id}:code`, async () => {
      const r = await post<Answer & { owner_code?: string; expires?: number }>(`${base}/owner-code`);
      if (r.error) return toast(r.error, true);
      setS((x) => (x ? { ...x, owner_code: r.owner_code ?? null, owner_code_expires: r.expires ?? null } : x));
    }, "Making a code…");
  const setSetting = (k: keyof RolesSettings, v: boolean) =>
    run(`roles:${server.id}:${k}`, async () => {
      const now = s?.settings;
      if (!now) return;
      const r = await post<Answer & { settings?: RolesSettings }>(`${base}/settings`, { ...now, [k]: v });
      if (r.error) return toast(r.error, true);
      setS((x) => (x ? { ...x, settings: r.settings ?? { ...now, [k]: v } } : x));
    }, "Saving…");
  const unclaim = async () => {
    if (
      !(await ask(
        `Unclaim ${server.name}? Open applications close and players can't make link codes here. Moderators you added keep their access until you remove them under People who can run it.`,
        { ok: "Unclaim", danger: true },
      ))
    )
      return;
    return run(`roles:${server.id}:unclaim`, async () => {
      const r = await post(`${base}/unclaim`);
      if (r.error) return toast(r.error, true);
      toast(`${server.name} is unclaimed.`);
      await load();
    }, "Unclaiming…");
  };
  const copyCode = async (c: string) => toast((await copyText(`/server claim code:${codeShown(c)}`)) ? "Copied. Paste it in the Loner-Strike Discord." : "Couldn't copy it.");

  return (
    <section className="panel roles-card" aria-label="Loner-Strike roles">
      <div className="row">
        <h3 style={{ margin: 0 }}>Loner-Strike roles</h3>
        <DocsLink to="moderate" what="Loner-Strike roles" />
        {s?.claimed && <span className="tag roles-chip good">Claimed</span>}
        {s?.official && <span className="tag roles-chip">Official server</span>}
      </div>
      <p className="muted measure">
        Players link their Steam account to Discord with <b>/link</b> here, apply to moderate it in the Loner-Strike Discord, and supporters can get their VIP. Claiming proves
        you run this server: Loner-Strike's list reads a setting your app puts on it for a moment.
      </p>
      {problem && (
        <p className="err-text" role="alert">
          {problem}
        </p>
      )}
      {!s && !problem && (
        <p className="muted small-note">
          <span className="spinner" aria-hidden="true" />
          Reading…
        </p>
      )}
      {s && !s.claimed && (
        <div className="row">
          <BusyButton className="btn primary" busy={busy(`roles:${server.id}:claim`)} label="Claim this server" busyLabel="Claiming…" onClick={claim} />
          <span className="muted small-note">The server has to be running and reachable from the internet (not LAN only).</span>
        </div>
      )}
      {s?.claimed && (
        <>
          {!s.owner_linked && (
            <div className="roles-code">
              <p>
                <b>One more step:</b> in the{" "}
                <a href={DISCORD} target="_blank" rel="noreferrer">
                  Loner-Strike Discord
                </a>
                , type the line below, so applications reach you by DM.
              </p>
              {s.owner_code ? (
                <div className="row">
                  <code className="mono">/server claim code:{codeShown(s.owner_code)}</code>
                  <button type="button" className="btn small" onClick={() => void copyCode(s.owner_code!)}>
                    Copy
                  </button>
                  <span className="muted small-note">{inTime(s.owner_code_expires) ? `Works once, ends ${inTime(s.owner_code_expires)}.` : "Works once, for 10 minutes."}</span>
                  <BusyButton className="btn small" busy={busy(`roles:${server.id}:code`)} label="New code" busyLabel="Making…" onClick={newCode} />
                </div>
              ) : (
                <BusyButton className="btn small" busy={busy(`roles:${server.id}:code`)} label="Get a code" busyLabel="Making…" onClick={newCode} />
              )}
            </div>
          )}
          {s.owner_linked && <p className="small-note">Your Discord account is linked as the owner: applications come to you by DM.</p>}
          {s.settings && (
            <div className="settings">
              <RoleSwitch
                label="Take moderator applications"
                note="Players can apply to moderate this server from the Loner-Strike Discord."
                on={s.settings.applications}
                busy={busy(`roles:${server.id}:applications`)}
                set={(v) => setSetting("applications", v)}
              />
              <RoleSwitch
                label="Let Loner-Strike Discord moderators decide"
                note="Applications also go to the Discord's moderators, who can approve or deny them. You can always decide yourself."
                on={s.settings.discord_mods_decide}
                busy={busy(`roles:${server.id}:discord_mods_decide`)}
                set={(v) => setSetting("discord_mods_decide", v)}
              />
              <RoleSwitch
                label="Honor Loner-Strike supporters"
                note="Supporters on this server get the supporter VIP (Admins › VIP & donations). Your app asks only about the players on it now."
                on={s.settings.honor_supporters}
                busy={busy(`roles:${server.id}:honor_supporters`)}
                set={(v) => setSetting("honor_supporters", v)}
              />
            </div>
          )}
          <div className="row">
            <BusyButton className="btn small danger" busy={busy(`roles:${server.id}:unclaim`)} label="Unclaim" busyLabel="Unclaiming…" onClick={() => void unclaim()} />
          </div>
        </>
      )}
    </section>
  );
}

function RoleSwitch({ label, note, on, set, busy }: { label: string; note: string; on: boolean; set: (v: boolean) => void; busy: boolean }) {
  return (
    <div className="setting">
      <span className="name">
        {label}
        <span className="note">{note}</span>
      </span>
      <Switch on={on} onChange={set} label={label} disabled={busy} />
    </div>
  );
}

/** Approved applicants and the Moderators who came from them: invite, its state, revoke.
 *  The owner and a shared-control Admin see the same list (an Admin's requests go to the owner's app). */
export function Applications({ server }: { server: ServerInfo }) {
  const toast = useToast();
  const ask = useConfirm();
  const { run, busy } = useActions();
  const [list, setList] = useState<Application[] | null>(null);
  const [problem, setProblem] = useState("");
  /** an invite to pass on by hand, when the bot couldn't DM it (by application id) */
  const [handOver, setHandOver] = useState<Record<string, string>>({});
  const base = `/api/roles/server/${encodeURIComponent(server.id)}/applications`;
  const load = useCallback(async () => {
    const r = await get<Answer & { applications?: Application[] }>(base);
    if (r.error) return setProblem(r.error);
    setProblem("");
    setList(sortApplications((r.applications ?? []).filter((a) => a && typeof a.id === "string" && isState(a.state))));
  }, [base]);
  useEffect(() => {
    void load();
  }, [load]);

  const invite = (a: Application) =>
    run(`roles:app:${a.id}`, async () => {
      const r = await post<Answer & { dm?: boolean; invite?: string }>(`${base}/${encodeURIComponent(a.id)}/invite`, { name: a.discord_name });
      if (r.error) return toast(r.error, true);
      if (r.dm === false && r.invite) {
        setHandOver((h) => ({ ...h, [a.id]: r.invite! }));
        toast(`${a.discord_name} has Discord DMs closed. Copy the invite and send it another way.`);
      } else toast(`Invite sent to ${a.discord_name} on Discord.`);
      await load();
    }, "Sending invite…");
  const revoke = async (a: Application) => {
    const what = a.state === "active" ? `Remove ${a.discord_name} as a Moderator? Their app access and in-game role go straight away, and the Discord role too.` : `Cancel ${a.discord_name}'s invite? It stops working.`;
    if (!(await ask(what, { ok: a.state === "active" ? "Remove" : "Cancel the invite", danger: true }))) return;
    return run(`roles:app:${a.id}`, async () => {
      const r = await post(`${base}/${encodeURIComponent(a.id)}/revoke`, { name: a.discord_name });
      if (r.error) return toast(r.error, true);
      setHandOver(({ [a.id]: _gone, ...rest }) => rest);
      toast(a.state === "active" ? `${a.discord_name} isn't a Moderator any more.` : "Invite canceled.");
      await load();
    }, "Revoking…");
  };

  return (
    <section className="panel roles-apps" aria-label="Moderator applications">
      <div className="row">
        <h3 style={{ margin: 0 }}>Moderator applications</h3>
        <span className="push" />
        <DocsLink to="moderate" what="moderator applications" />
        <BusyButton className="btn small" busy={busy(`roles:apps:${server.id}`)} label="Check again" busyLabel="Checking…" onClick={() => run(`roles:apps:${server.id}`, load, "Checking applications…")} />
      </div>
      <p className="muted measure">
        People who applied in the Loner-Strike Discord to moderate this server. Once one is approved, <b>Create moderator invite</b> makes a Moderator invite for their SteamID
        (one use, 24 hours) and the Discord bot sends it to them.
      </p>
      {problem && (
        <p className="err-text" role="alert">
          {problem}
        </p>
      )}
      {!list && !problem && (
        <p className="muted small-note">
          <span className="spinner" aria-hidden="true" />
          Reading applications…
        </p>
      )}
      {list && list.length === 0 && <p className="muted">No applications yet.</p>}
      {list && list.length > 0 && (
        <ul className="roles-list">
          {list.map((a) => {
            const can = actionsFor(a);
            const left = inTime(a.invite?.expires);
            return (
              <li key={a.id} className="roles-app">
                <div className="row">
                  <b>{a.discord_name || "Someone"}</b>
                  <span className={`tag roles-chip ${STATE_TONE[a.state]}`}>{STATE_WORD[a.state]}</span>
                  <span className="mono small-note">{a.steamid}</span>
                  <span className="muted small-note">
                    {a.official_link ? "Linked on an official server" : a.linked_on ? `Linked on ${a.linked_on}` : ""}
                    {!a.official_link && a.linked_on && <Info text="A link made on a host's server only proves what that server saw." />}
                  </span>
                  <span className="push" />
                  {can.invite && <BusyButton className="btn small primary" busy={busy(`roles:app:${a.id}`)} label="Create moderator invite" busyLabel="Sending invite…" onClick={() => invite(a)} />}
                  {can.revoke && (
                    <BusyButton className="btn small danger" busy={busy(`roles:app:${a.id}`)} label={a.state === "active" ? "Revoke" : "Cancel invite"} busyLabel="Revoking…" onClick={() => void revoke(a)} />
                  )}
                </div>
                {a.state === "invited" && <p className="small-note muted">{left ? `Waiting for them to accept; the invite ends ${left}.` : "Waiting for them to accept."}</p>}
                {a.state === "approved" && a.invite?.state === "expired" && <p className="small-note muted">Their last invite ran out unused. You can make another.</p>}
                {a.decided_by && <p className="small-note muted">Decided by {a.decided_by}.</p>}
                {handOver[a.id] && <InviteBox invite={handOver[a.id]} note="Their Discord DMs are closed: send it another way. It works once, for their SteamID, for 24 hours." />}
                {(a.availability || answerPairs(a.answers).length > 0) && (
                  <details>
                    <summary>Their application</summary>
                    <dl className="roles-answers">
                      {a.availability && (
                        <>
                          <dt>When they're around</dt>
                          <dd>{a.availability}</dd>
                        </>
                      )}
                      {answerPairs(a.answers).map((p) => (
                        <div key={p.q}>
                          <dt>{p.q}</dt>
                          <dd>{p.a}</dd>
                        </div>
                      ))}
                    </dl>
                  </details>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

/** The supporters' VIP (Admins › VIP & donations): off until the host turns it on. */
type Supporters = Answer & { claimed?: boolean; official?: boolean; honor?: boolean; tag?: boolean; scoreboard?: boolean; reserve?: boolean; slots?: number; joinmsg?: boolean };

export function SupporterVip() {
  const toast = useToast();
  const { run, busy } = useActions();
  const [d, setD] = useState<Supporters | null>(null);
  const [asked, setAsked] = useState(0);
  useEffect(() => {
    let gone = false;
    void get<Supporters>("/api/admins/supporters").then((r) => !gone && setD(r));
    return () => {
      gone = true;
    };
  }, [asked]);
  const set = (p: Partial<Supporters>) =>
    run(`supporters:${Object.keys(p)[0]}`, async () => {
      const r = await post<Supporters>("/api/admins/supporters", p);
      if (r.error) return toast(r.error, true);
      setD((x) => ({ ...x, ...p, ...r, error: undefined }));
    }, "Saving supporter VIP…");
  const b = (k: keyof Supporters) => busy(`supporters:${k}`);
  if (!d)
    return (
      <p className="muted small-note">
        <span className="spinner" aria-hidden="true" />
        Reading…
      </p>
    );
  if (d.error && d.honor === undefined)
    return (
      <LoadError
        title="The supporters' VIP settings didn't load"
        text={d.error === "Couldn't reach the app." ? "Loner-Strike didn't answer. If it's still starting, wait a moment, then try again." : d.error}
        onRetry={() => (setD(null), setAsked((n) => n + 1))}
      />
    );
  const on = !!(d.official || d.honor);
  return (
    <div className="roles-vip">
      <h3>Loner-Strike supporters</h3>
      <p className="muted measure">
        People who support Loner-Strike get a cosmetic VIP on the official servers. You can honor it on yours too: your app asks Loner-Strike only about the players on the server
        right now. It never gives admin powers, gear, money or abilities.
      </p>
      <div className="settings">
        {!d.official && (
          <div className="setting">
            <span className="name">
              Honor Loner-Strike supporters
              <span className="note">{d.claimed ? "Off unless you turn it on." : "Claim the server first, under Shared control › Loner-Strike roles."}</span>
            </span>
            <Switch on={!!d.honor} onChange={(v) => set({ honor: v })} label="Honor Loner-Strike supporters" disabled={b("honor") || !d.claimed} />
          </div>
        )}
        {d.official && <p className="small-note">This is an official server: supporters always get their VIP here.</p>}
        {on && (
          <>
            <SupPerk label="[VIP] tag" note="In chat, in front of their name." on={!!d.tag} busy={b("tag")} set={(v) => set({ tag: v })} />
            <SupPerk label="Scoreboard mark" note="The scoreboard's VIP column." on={!!d.scoreboard} busy={b("scoreboard")} set={(v) => set({ scoreboard: v })} />
            <SupPerk label="Join message" note={'"<name>, a Loner-Strike supporter, joined."'} on={!!d.joinmsg} busy={b("joinmsg")} set={(v) => set({ joinmsg: v })} />
            <div className="setting">
              <span className="name">
                Reserved slots
                <span className="note">Kept free for supporters and your own VIPs. Someone else who takes one is moved out, or a bot leaves to make room.</span>
              </span>
              <div className="seg" role="group" aria-label="Reserved slots">
                {[0, 1, 2].map((n) => (
                  <button key={n} type="button" aria-pressed={(d.slots ?? 0) === n} disabled={b("slots")} onClick={() => set({ slots: n, reserve: n > 0 })}>
                    {n === 0 ? "None" : n}
                  </button>
                ))}
              </div>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function SupPerk({ label, note, on, set, busy }: { label: string; note: string; on: boolean; set: (v: boolean) => void; busy: boolean }) {
  return (
    <div className="setting">
      <span className="name">
        {label}
        <span className="note">{note}</span>
      </span>
      <Switch on={on} onChange={set} label={`Supporters: ${label}`} disabled={busy} />
    </div>
  );
}
