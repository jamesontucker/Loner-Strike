import { useState } from "react";
import type { Online, Player } from "../types";
import { SectionHead, useCommand } from "../ui";

const WEAPONS: [string, string][] = [
  ["weapon_ak47", "AK-47"], ["weapon_m4a1", "M4A1"], ["weapon_awp", "AWP"], ["weapon_deagle", "Desert Eagle"],
  ["weapon_scout", "Scout"], ["weapon_m249", "M249"], ["weapon_mp5navy", "MP5"], ["weapon_p90", "P90"],
  ["weapon_xm1014", "Auto shotgun"], ["weapon_sg552", "SG-552"], ["weapon_aug", "AUG"], ["weapon_g3sg1", "G3SG1"],
  ["weapon_elite", "Dual Elites"], ["nades", "Grenade pack"], ["armor", "Kevlar + helmet"],
];

export function Players({ data, refresh }: { data: Online; refresh: () => void }) {
  const run = useCommand(refresh);
  const [open, setOpen] = useState<number | null>(null);
  const [gun, setGun] = useState(WEAPONS[0][0]);
  const teams: [string, string, string][] = [["T", "Terrorists", "t"], ["CT", "Counter-Terrorists", "ct"], ["SPEC", "Spectators", "spec"]];

  const act = (p: Player, a: string) => {
    if (a === "kick" && !confirm(`Kick ${p.name}?`)) return;
    run("gm_player", [String(p.uid), a], true);
  };

  return (
    <div className="stack">
      <div>
        <SectionHead title="Players" note="Select someone for god mode, weapons, teleporting and more." />
        <div className="row">
          <button className="btn" onClick={() => run("gm_player", ["all", "respawn"])}>Respawn the dead</button>
          <button className="btn" onClick={() => run("gm_player", ["all", "heal"])}>Heal everyone</button>
          <span className="spacer" />
          <button className="btn" onClick={() => run("gm_team", ["t"])}>Join Terrorists</button>
          <button className="btn" onClick={() => run("gm_team", ["ct"])}>Join CTs</button>
          <button className="btn" onClick={() => run("gm_team", ["spec"])}>Spectate</button>
          <button className="btn" onClick={() => confirm("Swap everyone to the other team? The round restarts.") && run("gm_team", ["swap"])}>
            Swap teams
          </button>
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
                      <span className={`hp${p.hp && p.hp < 35 ? " low" : ""}`} aria-label={`${p.hp} health`}>
                        <i style={{ width: `${Math.min(100, p.hp)}%` }} />
                      </span>
                      <span className="num">${p.money.toLocaleString()}</span>
                      <span className="num kd">{p.frags}/{p.deaths}</span>
                    </button>
                    {open === p.uid && (
                      <div className="pactions">
                        <div className="row">
                          <button className="btn" onClick={() => act(p, "god")}>{p.god ? "God mode off" : "God mode"}</button>
                          <button className="btn" onClick={() => act(p, "noclip")}>{p.noclip ? "Noclip off" : "Noclip"}</button>
                          <button className="btn" onClick={() => act(p, "heal")}>Heal + armor</button>
                          <button className="btn" onClick={() => act(p, "money")}>Give $16,000</button>
                          <button className="btn" onClick={() => act(p, "respawn")}>Respawn</button>
                          <button className="btn" onClick={() => act(p, "team")}>Switch team</button>
                        </div>
                        <div className="row">
                          <select value={gun} onChange={(e) => setGun(e.target.value)} aria-label="Weapon">
                            {WEAPONS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
                          </select>
                          <button className="btn" onClick={() => run("gm_player", [String(p.uid), "give", gun], true)}>Give</button>
                        </div>
                        {!p.host && (
                          <div className="row">
                            <button className="btn" onClick={() => act(p, "bring")}>Bring to me</button>
                            <button className="btn" onClick={() => act(p, "goto")}>Go to them</button>
                            <button className="btn" onClick={() => act(p, "freeze")}>{p.frozen ? "Unfreeze" : "Freeze"}</button>
                            <button className="btn" onClick={() => act(p, "slap")}>Slap</button>
                            <button className="btn danger" onClick={() => act(p, "slay")}>Slay</button>
                            <button className="btn danger" onClick={() => act(p, "kick")}>Kick</button>
                          </div>
                        )}
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
