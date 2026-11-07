import { useState } from "react";
import { currentTarget, targetKey, useActions } from "../api";
import type { Online, Player } from "../types";
import { BusyButton, SectionHead, useCommand, useConfirm } from "../ui";
import { MuteControl } from "./MuteControl";

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

export function Players({ data, refresh }: { data: Online; refresh: () => void }) {
  const ask = useConfirm();
  const run = useCommand(refresh);
  const { run: once, busy } = useActions();
  const key = (what: string) => `players:${targetKey()}:${what}`;
  const all = (what: string, command: string, args: string[], label: string) => once(key(what), () => run(command, args), label);
  const [open, setOpen] = useState<number | null>(null);
  const [gun, setGun] = useState(WEAPONS[0][0]);
  const teams: [string, string, string][] = [["T", "Terrorists", "t"], ["CT", "Counter-Terrorists", "ct"], ["SPEC", "Spectators", "spec"]];

  const act = async (p: Player, a: string) => {
    if (a === "kick" && !(await ask(`Kick ${p.name}?`, { ok: "Kick", danger: true }))) return;
    const doing = a === "freeze" && p.frozen ? "Unfreezing" : DOING[a]?.[1] ?? "Sending to";
    once(key(`${p.uid}:${a}`), () => run("gm_player", [String(p.uid), a], true), `${doing} ${p.name}…`);
  };
  // a button for one player's action
  const pb = (p: Player, a: string, label: string, className?: string) => (
    <BusyButton className={className} busy={busy(key(`${p.uid}:${a}`))} label={label} busyLabel={a === "freeze" && p.frozen ? "Unfreezing…" : DOING[a]?.[0] ?? "Sending…"} onClick={() => act(p, a)} />
  );

  return (
    <div className="stack">
      <div>
        <SectionHead title="Players" note="Select someone for god mode, weapons, teleporting and more." />
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
                <div className="prow prow-head" aria-hidden="true">
                  <span>Name</span>
                  <span>Health</span>
                  <span className="num">Money</span>
                  <span className="num kd">K/D</span>
                </div>
              )}
              <ul>
                {list.map((p) => (
                  <li key={p.uid} className={p.alive ? "" : "dead"}>
                    <button className="prow" aria-expanded={open === p.uid} onClick={() => setOpen(open === p.uid ? null : p.uid)}>
                      <span className="pname">
                        {p.name}
                        {p.host ? <small>you</small> : null}
                        {p.god ? <i className="flag">god</i> : null}
                        {p.noclip ? <i className="flag">noclip</i> : null}
                        {p.frozen ? <i className="flag">frozen</i> : null}
                      </span>
                      <span className={`hp${p.hp && p.hp < 35 ? " low" : ""}`} role="meter" aria-label="Health" aria-valuenow={Math.max(0, p.hp)} aria-valuemin={0} aria-valuemax={Math.max(100, p.hp)}>
                        <i style={{ width: `${Math.min(100, p.hp)}%` }} />
                      </span>
                      <span className="num"><span className="ls-sr">Money </span>${p.money.toLocaleString()}</span>
                      <span className="num kd"><span className="ls-sr">Kills and deaths </span>{p.frags}/{p.deaths}</span>
                    </button>
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
                            {pb(p, "kick", "Kick", "btn danger")}
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
