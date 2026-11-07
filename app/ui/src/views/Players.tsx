import { useEffect, useRef, useState } from "react";
import { copyText, currentTarget, postJson, targetKey, useActions } from "../api";
import { Menu, type MenuGroup, type MenuItem } from "../menu";
import { setAdminList, useAdminList } from "../queries";
import type { AdminList, AdminReply, AdminRole, Online, Player } from "../types";
import { BusyButton, SectionHead, useAskText, useCommand, useConfirm, useToast } from "../ui";
import { ck, failed, MuteControl, OFFLINE, STEAMID, type Data as CommunityData, type Err } from "./MuteControl";

const WEAPONS: [string, string][] = [
  ["weapon_ak47", "AK-47"], ["weapon_m4a1", "M4A1"], ["weapon_awp", "AWP"], ["weapon_deagle", "Desert Eagle"],
  ["weapon_scout", "Scout"], ["weapon_m249", "M249"], ["weapon_mp5navy", "MP5"], ["weapon_p90", "P90"],
  ["weapon_xm1014", "Auto shotgun"], ["weapon_sg552", "SG-552"], ["weapon_aug", "AUG"], ["weapon_g3sg1", "G3SG1"],
  ["weapon_elite", "Dual Elites"], ["nades", "Grenade pack"], ["armor", "Kevlar and helmet"],
];

// What a player action says while it's sent: [on the button, in the top bar before the name]
const DOING: Record<string, [string, string]> = {
  god: ["Switching…", "Switching god mode for"], noclip: ["Switching…", "Switching noclip for"], heal: ["Healing…", "Healing"],
  money: ["Giving…", "Giving $16,000 to"], respawn: ["Respawning…", "Respawning"], team: ["Switching…", "Switching the team of"],
  give: ["Giving…", "Giving a weapon to"], bring: ["Bringing…", "Bringing"], goto: ["Going…", "Going to"], freeze: ["Freezing…", "Freezing"],
  slap: ["Slapping…", "Slapping"], slay: ["Slaying…", "Slaying"], kick: ["Kicking…", "Kicking"],
};

/** Ban lengths offered for someone playing now: [minutes (0 for good), how it reads after "Ban"] */
const BANS: [number, string][] = [[60, "for 1 hour"], [1440, "for 1 day"], [10080, "for 1 week"], [0, "forever"]];
const ROLE_NAME: Record<AdminRole, string> = { owner: "Owner", admin: "Admin", moderator: "Moderator", vip: "VIP" };

/** YYYY-MM-DD n days after day (as the Admins page counts them). */
function addDays(day: string, n: number) {
  const d = new Date(`${day}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + n);
  return d.toISOString().slice(0, 10);
}
const dayDate = (day: string) => new Date(`${day}T00:00:00Z`).toLocaleDateString([], { day: "numeric", month: "short", year: "numeric", timeZone: "UTC" });
const shortDate = (t: number) => new Date(t * 1000).toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" });

export function Players({ data, refresh }: { data: Online; refresh: () => void }) {
  const ask = useConfirm();
  const askText = useAskText();
  const toast = useToast();
  const run = useCommand(refresh);
  const { run: once, busy } = useActions();
  const key = (what: string) => `players:${targetKey()}:${what}`;
  const all = (what: string, command: string, args: string[], label: string) => once(key(what), () => run(command, args), label);
  const [open, setOpen] = useState<number | null>(null);
  const [gun, setGun] = useState(WEAPONS[0][0]);
  const teams: [string, string, string][] = [["T", "Terrorists", "t"], ["CT", "Counter-Terrorists", "ct"], ["SPEC", "Spectators", "spec"]];

  const admins = useAdminList();
  const roster = admins.data;
  // someone new joins: their visits and other names come from the list the app reads then
  const joined = data.players.filter((p) => !p.bot && p.steamid && STEAMID.test(p.steamid)).map((p) => p.steamid).sort().join(",");
  const lastJoined = useRef(joined);
  useEffect(() => {
    if (joined === lastJoined.current) return;
    lastJoined.current = joined;
    if (joined) admins.refetch();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [joined]);
  // the same action keys as the Admins page and the mute controls, so both show it running
  const adminKey = (what: string) => `admins:${targetKey()}:${what}`;
  const later = () => setTimeout(refresh, 250);

  const act = async (p: Player, a: string) => {
    if (a === "kick") return kick(p);
    const doing = a === "freeze" && p.frozen ? "Unfreezing" : DOING[a]?.[1] ?? "Sending to";
    once(key(`${p.uid}:${a}`), () => run("gm_player", [String(p.uid), a], true), `${doing} ${p.name}…`);
  };
  // a button for one player's action
  const pb = (p: Player, a: string, label: string, className?: string) => (
    <BusyButton className={className} busy={busy(key(`${p.uid}:${a}`))} label={label} busyLabel={a === "freeze" && p.frozen ? "Unfreezing…" : DOING[a]?.[0] ?? "Sending…"} onClick={() => act(p, a)} />
  );

  const kick = async (p: Player) => {
    // a bot never reads the reason
    const reason = p.bot
      ? (await ask(`Kick ${p.name}?`, { ok: "Kick", danger: true })) ? "" : null
      : await askText(`Kick ${p.name}?`, { label: "Reason (optional)", placeholder: "They see this as they leave", max: 64 }, { ok: "Kick", danger: true });
    if (reason === null) return;
    once(key(`${p.uid}:kick`), async () => {
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/admins/kick", { uid: p.uid, reason }).catch(() => OFFLINE);
      if (r.error) return toast(r.error, true);
      toast(`Kicked ${p.name}`);
      later();
    }, `Kicking ${p.name}…`);
  };
  /** Posts an Admins change; the answer is the new list, shared with the Admins page. */
  const sendAdmin = async (path: string, body: unknown): Promise<AdminList | null> => {
    const r = await postJson<AdminReply>(path, body).catch(() => null);
    if (!r) return toast("Couldn't reach the app.", true), null;
    if (!("people" in r)) return toast(r.error || "Couldn't save that. Try again.", true), null;
    setAdminList(r);
    return r;
  };
  const ban = async (p: Player, minutes: number) => {
    const id = p.steamid!;
    const how = BANS.find(([m]) => m === minutes)?.[1] ?? "forever";
    const more = minutes ? "A timed ban also ends if the game or server restarts." : "Unban them on the Admins page.";
    if (!(await ask(`Ban ${p.name} ${how}? They're kicked now. ${more}`, { ok: "Ban", danger: true }))) return;
    once(adminKey(`ban:${id}`), async () => {
      if (!(await sendAdmin("/api/admins/ban", { id, minutes }))) return;
      toast(`Banned ${p.name} ${how}`);
      later();
    }, `Banning ${p.name}…`);
  };
  const grant = async (p: Player, role: "admin" | "vip", days?: number) => {
    if (!roster) return;
    const id = p.steamid!;
    const existing = roster.people.find((x) => x.id === id);
    if (role === "admin" && !(await ask(`Make ${p.name} an admin? Admins can kick, ban, slay and change maps.`, { ok: "Make admin" }))) return;
    once(adminKey(`grant:${id}`), async () => {
      const r = await sendAdmin("/api/admins/person", { id, name: existing?.name || p.name, role, expires: days ? addDays(roster.today, days) : null, note: existing?.note ?? "", was: existing?.id });
      if (r) toast(role === "admin" ? `${p.name} is now an admin` : days ? `${p.name} is a VIP for ${days} days` : `${p.name} is now a VIP`);
    }, role === "admin" ? `Making ${p.name} an admin…` : `Making ${p.name} a VIP…`);
  };
  const unrole = async (p: Player, role: AdminRole) => {
    const id = p.steamid!;
    if (!(await ask(`Take away ${p.name}'s ${ROLE_NAME[role]} role? They go back to being a normal player.`, { ok: "Remove role", danger: true }))) return;
    once(adminKey(`remove:${id}`), async () => {
      if (await sendAdmin("/api/admins/remove", { id })) toast(`${p.name} is a normal player again`);
    }, `Removing ${p.name}'s role…`);
  };
  // the map's time left, as a mute's length (a map without a time limit gets an hour)
  const st = data.state;
  const mapMinutes = st.timelimit > 0 && st.timeleft > 0 ? Math.ceil(st.timeleft / 60) + 1 : 0;
  const muteChat = (p: Player) => {
    const id = p.steamid!;
    once(ck(`mute:${id}`), async () => {
      const r = await postJson<CommunityData | Err>("/api/community/gag", { id, name: p.name, minutes: mapMinutes || 60, chat: true, voice: false }).catch(() => OFFLINE);
      if (failed(r)) return toast(r.error, true);
      toast(mapMinutes ? `${p.name} can't chat until this map ends` : `${p.name} can't chat for an hour`);
    }, `Muting ${p.name}…`);
  };
  const copyId = async (p: Player) => {
    if (await copyText(p.steamid!)) toast("SteamID copied");
    else toast(`Couldn't copy it. Their SteamID is ${p.steamid}`, true);
  };

  /** The row's "…" menu: everything you can do to this player, grouped. */
  const menuFor = (p: Player): MenuGroup[] => {
    const id = p.steamid && STEAMID.test(p.steamid) ? p.steamid : "";
    const local = !currentTarget();
    const item = (a: string, label: string, extra: Partial<MenuItem> = {}): MenuItem => ({
      label,
      on: () => act(p, a),
      busy: busy(key(`${p.uid}:${a}`)),
      busyLabel: a === "freeze" && p.frozen ? "Unfreezing…" : DOING[a]?.[0] ?? "Sending…",
      ...extra,
    });
    const why = p.bot
      ? "Bots have no SteamID, so they can't be banned, muted or given a role."
      : "No SteamID (they show as STEAM_ID_LAN), so they can't be banned, muted or given a role. Players who join through Steam have one.";
    const role = id ? roster?.people.find((x) => x.id === id) : undefined;
    const seen = id ? roster?.seen.find((x) => x.id === id) : undefined;
    const roleBusy = busy(adminKey(`grant:${id}`)) || busy(adminKey(`remove:${id}`));
    const groups: MenuGroup[] = [
      {
        title: "Moderation",
        items: [
          // Kick and Ban are also on the row, which shows them running
          { label: "Kick", on: () => kick(p), disabled: busy(key(`${p.uid}:kick`)), danger: true },
          ...BANS.map(([m, how]): MenuItem => ({ label: `Ban ${how}`, on: () => ban(p, m), disabled: busy(adminKey(`ban:${id}`)), danger: true, show: !!id })),
          { label: mapMinutes ? "Mute chat for this map" : "Mute chat for 1 hour", on: () => muteChat(p), busy: busy(ck(`mute:${id}`)), busyLabel: "Muting…", show: !!id },
          item("slay", "Slay", { danger: true, show: !!p.alive }),
          item("slap", "Slap", { show: !!p.alive }),
        ],
        note: id ? undefined : why,
      },
    ];
    if (id) {
      const now = role && !role.expired ? role : undefined;
      groups.push({
        title: "Roles",
        items: roster
          ? [
              { label: "Make admin", on: () => grant(p, "admin"), busy: roleBusy, busyLabel: "Saving…", show: now?.role !== "admin" && now?.role !== "owner" },
              { label: "Make VIP for 30 days", on: () => grant(p, "vip", 30), busy: roleBusy, busyLabel: "Saving…" },
              { label: "Make VIP for good", on: () => grant(p, "vip"), busy: roleBusy, busyLabel: "Saving…", show: !(now?.role === "vip" && !now.expires) },
              { label: "Remove role", on: () => unrole(p, role!.role), busy: roleBusy, busyLabel: "Removing…", danger: true, show: !!role },
            ]
          : [],
        note: !roster
          ? admins.isError ? "Couldn't load roles. Open Admins to try again." : "Loading roles…"
          : now
            ? `${ROLE_NAME[now.role]}${now.expires ? ` until ${dayDate(now.expires)}` : ""}.${roster.users_ini ? "" : " AMX Mod X isn't set up for this game yet, so roles can't take effect."}`
            : roster.users_ini ? undefined : "AMX Mod X isn't set up for this game yet, so roles can't take effect.",
      });
    }
    groups.push({
      title: "Help and fun",
      items: [
        item("respawn", "Respawn"),
        item("heal", "Heal and armour", { show: !!p.alive }),
        item("money", "Give $16,000"),
        item("team", "Switch team"),
        item("freeze", p.frozen ? "Unfreeze" : "Freeze", { show: !!p.alive }),
        // on a server there's no "you" to bring them to
        item("bring", "Bring to me", { show: local && !!p.alive }),
        item("goto", "Go to them", { show: local && !!p.alive }),
      ],
    });
    if (id) {
      const aka = seen?.aka.filter((n) => n !== p.name) ?? [];
      groups.push({
        title: "Info",
        items: [{ label: "Copy SteamID", on: () => copyId(p) }],
        note: (
          <>
            <span className="mono">{id}</span>
            <br />
            {seen ? `Seen ${seen.visits} time${seen.visits === 1 ? "" : "s"}, first on ${shortDate(seen.first)}.` : roster ? "No earlier visits on record." : ""}
            {aka.length > 0 && <><br />Also played as {aka.slice(0, 5).join(", ")}{aka.length > 5 ? ` and ${aka.length - 5} more` : ""}.</>}
          </>
        ),
      });
    }
    return groups;
  };
  const roleOf = (p: Player) => {
    const r = p.steamid ? roster?.people.find((x) => x.id === p.steamid && !x.expired) : undefined;
    return r?.role;
  };
  /** Kick, Ban and the "…" menu at the end of a row. */
  const rowActions = (p: Player) => {
    if (p.host) return null;
    const id = p.steamid && STEAMID.test(p.steamid) ? p.steamid : "";
    return (
      <>
        <BusyButton className="btn small danger" busy={busy(key(`${p.uid}:kick`))} label="Kick" busyLabel="Kicking…" onClick={() => kick(p)} />
        {id && (
          <Menu
            className="btn small danger"
            label="Ban"
            ariaLabel={`Ban ${p.name}`}
            items={BANS.map(([m, how]) => ({ label: `Ban ${how}`, on: () => ban(p, m), busy: busy(adminKey(`ban:${id}`)), busyLabel: "Banning…" }))}
          />
        )}
        <Menu className="btn small pmore" label="…" caret={false} wide ariaLabel={`More actions for ${p.name}`} groups={menuFor(p)} />
      </>
    );
  };

  return (
    <div className="stack">
      <div>
        <SectionHead title="Players" note="Select someone for god mode, weapons and teleporting. Kick, ban, roles and the rest are under … on their row." />
        <div className="player-actions">
          <div className="row" role="group" aria-labelledby="pa-everyone">
            <span className="pa-label" id="pa-everyone">Everyone</span>
            <BusyButton busy={busy(key("all:respawn"))} label="Respawn the dead" busyLabel="Respawning…" onClick={() => all("all:respawn", "gm_player", ["all", "respawn"], "Respawning the dead…")} />
            <BusyButton busy={busy(key("all:heal"))} label="Heal everyone" busyLabel="Healing…" onClick={() => all("all:heal", "gm_player", ["all", "heal"], "Healing everyone…")} />
            <BusyButton
              busy={busy(key("team:swap"))}
              label="Swap teams"
              busyLabel="Swapping…"
              onClick={async () => (await ask("Swap everyone to the other team? The round restarts.", { ok: "Swap teams" })) && all("team:swap", "gm_team", ["swap"], "Swapping teams…")}
            />
          </div>
          {/* on a server there's no "you" in the game */}
          {!currentTarget() && (
            <div className="row" role="group" aria-labelledby="pa-you">
              <span className="pa-label" id="pa-you">You</span>
              <BusyButton busy={busy(key("team:t"))} label="Join Terrorists" busyLabel="Joining…" onClick={() => all("team:t", "gm_team", ["t"], "Joining the Terrorists…")} />
              <BusyButton busy={busy(key("team:ct"))} label="Join Counter-Terrorists" busyLabel="Joining…" onClick={() => all("team:ct", "gm_team", ["ct"], "Joining the Counter-Terrorists…")} />
              <BusyButton busy={busy(key("team:spec"))} label="Spectate" busyLabel="Moving…" onClick={() => all("team:spec", "gm_team", ["spec"], "Moving you to spectators…")} />
            </div>
          )}
        </div>
      </div>
      <div className="teams">
        {teams.map(([team, title, cls]) => {
          const list = data.players.filter((p) => p.team === team).sort((a, b) => b.frags - a.frags);
          if (!list.length && team === "SPEC") return null;
          return (
            <div key={team} className={`team ${cls}`}>
              <h3>
                {title} <span>{list.length}</span>
              </h3>
              {list.length > 0 && (
                <div className="prow-line" aria-hidden="true">
                  <div className="prow prow-head">
                    <span>Name</span>
                    <span>Health</span>
                    <span className="num">Money</span>
                    <span className="num kd">K/D</span>
                  </div>
                  <span className="prow-act" />
                </div>
              )}
              <ul>
                {list.map((p) => (
                  <li key={p.uid} className={p.alive ? "" : "dead"}>
                    <div className="prow-line">
                      <button className="prow" aria-expanded={open === p.uid} onClick={() => setOpen(open === p.uid ? null : p.uid)}>
                        <span className="pname">
                          {p.name}
                          {p.host ? <small>you</small> : null}
                          {p.god ? <i className="flag">god</i> : null}
                          {p.noclip ? <i className="flag">noclip</i> : null}
                          {p.frozen ? <i className="flag">frozen</i> : null}
                          {roleOf(p) && <i className={`role-badge ${roleOf(p)}`}>{ROLE_NAME[roleOf(p)!]}</i>}
                        </span>
                        <span className={`hp${p.hp && p.hp < 35 ? " low" : ""}`} role="meter" aria-label="Health" aria-valuenow={Math.max(0, p.hp)} aria-valuemin={0} aria-valuemax={Math.max(100, p.hp)}>
                          <i style={{ width: `${Math.min(100, p.hp)}%` }} />
                        </span>
                        <span className="num"><span className="ls-sr">Money </span>${p.money.toLocaleString()}</span>
                        <span className="num kd"><span className="ls-sr">Kills and deaths </span>{p.frags}/{p.deaths}</span>
                      </button>
                      <div className="prow-act">{rowActions(p)}</div>
                    </div>
                    {open === p.uid && (
                      <div className="pactions">
                        <div className="row">
                          {pb(p, "god", p.god ? "God mode off" : "God mode")}
                          {pb(p, "noclip", p.noclip ? "Noclip off" : "Noclip")}
                          {pb(p, "heal", "Heal and armour")}
                          {pb(p, "money", "Give $16,000")}
                          {pb(p, "respawn", "Respawn")}
                          {pb(p, "team", "Switch team")}
                        </div>
                        <div className="row">
                          <select value={gun} onChange={(e) => setGun(e.target.value)} aria-label="Weapon">
                            {WEAPONS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
                          </select>
                          <BusyButton busy={busy(key(`${p.uid}:give`))} label="Give" busyLabel="Giving…" onClick={() => once(key(`${p.uid}:give`), () => run("gm_player", [String(p.uid), "give", gun], true), `Giving ${WEAPONS.find(([v]) => v === gun)?.[1] ?? "a weapon"} to ${p.name}…`)} />
                        </div>
                        {!p.host && (
                          <div className="row">
                            {/* on a server there's no "you" to bring them to */}
                            {!currentTarget() && pb(p, "bring", "Bring to me")}
                            {!currentTarget() && pb(p, "goto", "Go to them")}
                            {pb(p, "freeze", p.frozen ? "Unfreeze" : "Freeze")}
                            {pb(p, "slap", "Slap")}
                            {pb(p, "slay", "Slay", "btn danger")}
                          </div>
                        )}
                        {!p.host && !p.bot && p.steamid && <MuteControl steamid={p.steamid} name={p.name} />}
                      </div>
                    )}
                  </li>
                ))}
                {!list.length && <li className="empty">Nobody yet</li>}
              </ul>
            </div>
          );
        })}
      </div>
      <Feed data={data} />
    </div>
  );
}

function Feed({ data }: { data: Online }) {
  const events = [...data.events].reverse().slice(0, 40);
  return (
    <div>
      <SectionHead title="Feed" note="Kills, chat and round results from this map, newest first." />
      <ul className="feed">
        {events.map((e) => (
          <li key={e.seq}>
            <time>{timeOf(e.time)}</time>
            <span>{line(e)}</span>
          </li>
        ))}
        {!events.length && <li className="empty">Nothing yet. Kills and chat show up here as they happen.</li>}
      </ul>
    </div>
  );
}

const team = (t: unknown) => (String(t).toLowerCase() === "t" ? "t" : String(t).toLowerCase() === "ct" ? "ct" : "");

function line(e: Record<string, unknown>) {
  const s = (k: string) => String(e[k] ?? "");
  if (s("kind") === "kill") {
    if (!s("killer")) return <><b className={team(e.vteam)}>{s("victim")}</b> <span className="quiet">died</span></>;
    return (
      <>
        <b className={team(e.kteam)}>{s("killer")}</b> <span className="wpn">{s("weapon")}</span> <b className={team(e.vteam)}>{s("victim")}</b>
        {Number(e.hs) ? <span className="hs">headshot</span> : null}
      </>
    );
  }
  if (s("kind") === "chat")
    return (
      <>
        <b className={team(e.team)}>{s("name")}</b>
        {Number(e.teamonly) ? <span className="quiet"> (team)</span> : null}: {s("text")}
      </>
    );
  return <span className="quiet">{s("text")}</span>;
}

function timeOf(t: number) {
  return t ? new Date(t * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) : "";
}
