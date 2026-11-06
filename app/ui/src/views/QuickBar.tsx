import { useState } from "react";
import { postJson } from "../api";
import type { MapInfo, Online } from "../types";
import { useCommand, useToast } from "../ui";

/** One-click actions while a game is live, under the top bar (as in the original panel). */
export function QuickBar({ data, maps, refresh, reloadMaps }: { data: Online; maps: MapInfo[]; refresh: () => void; reloadMaps: () => void }) {
  const run = useCommand(refresh);
  const toast = useToast();
  const [announcing, setAnnouncing] = useState(false);
  const [text, setText] = useState("");
  const st = data.state;
  const me = data.players.find((p) => p.host);
  const career = !!st.career;
  const here = maps.find((m) => m.name.toLowerCase() === st.map.toLowerCase());

  const randomMap = () => {
    const pool = maps.filter((m) => !m.hidden && m.name.toLowerCase() !== st.map.toLowerCase());
    if (pool.length) run("gm_map", [pool[Math.floor(Math.random() * pool.length)].name]);
  };
  const favourite = async () => {
    const r = await postJson<{ error?: string }>("/api/mapflag", { flag: "fav", map: st.map, on: !here?.fav }).catch(() => ({ error: "Couldn't save that." }));
    if (r.error) return toast(r.error, true);
    toast(here?.fav ? "Removed from favourites" : "Added to favourites");
    reloadMaps();
  };
  const announce = async () => {
    if (text.trim() && (await run("gm_announce", [text.trim()]))) {
      setText("");
      setAnnouncing(false);
    }
  };

  type Q = { label: string; on?: () => void; pressed?: boolean; show?: boolean; disabled?: boolean };
  const items: Q[] = [
    { label: "Restart round", on: () => run("gm_match", ["restart"]), show: !career },
    { label: st.paused ? "Resume" : "Pause", on: () => run("gm_pause"), pressed: !!st.paused },
    { label: "Restart map", on: () => run("gm_restartmap"), show: !career },
    { label: "Random map", on: randomMap, show: !career },
    { label: here?.fav ? "Favourite map" : "Favourite this map", on: favourite, pressed: !!here?.fav, show: !!here },
    { label: "End round", on: () => run("gm_do", ["endround"]), show: !career },
    { label: "+10 minutes", on: () => run("gm_do", ["addtime"]), show: !career && st.timelimit > 0 },
    { label: "Bots −", on: () => run("gm_bots", ["count", String(Math.max(0, st.bots - 1))]), show: !career },
    { label: "Bots +", on: () => run("gm_bots", ["count", String(Math.min(st.maxplayers - 1, st.bots + 1))]), show: !career },
    { label: "Peaceful bots", on: () => run("gm_set", ["bot_zombie", st.peaceful ? "0" : "1"]), pressed: !!st.peaceful, show: !career },
    { label: "God mode", on: () => me && run("gm_player", [String(me.uid), "god"]), pressed: !!me?.god, show: !!me },
    { label: "Noclip", on: () => me && run("gm_player", [String(me.uid), "noclip"]), pressed: !!me?.noclip, show: !!me },
    { label: "Respawn me", on: () => me && run("gm_player", [String(me.uid), "respawn"]), show: !!me },
    { label: "Heal me", on: () => run("gm_do", ["heal"]), show: !!me?.alive },
    { label: "$16,000", on: () => run("gm_do", ["money"]), show: !!me && !career },
    { label: "Save my spot", on: () => run("gm_pos", ["save"]), show: !!me },
    { label: "Back to my spot", on: () => run("gm_pos", ["load"]), show: !!me, disabled: !data.me?.haspos },
    { label: "Announce…", on: () => setAnnouncing((a) => !a), pressed: announcing },
    { label: st.recording ? "Stop recording" : "Record demo", on: () => run("gm_do", ["demo"]), pressed: !!st.recording },
  ];

  return (
    <div className="quick" aria-label="Quick actions">
      <div className="quick-inner">
        {items
          .filter((q) => q.show !== false)
          .map((q) => (
            <button key={q.label} className="qb" aria-pressed={q.pressed === undefined ? undefined : q.pressed} disabled={q.disabled} onClick={q.on}>
              {q.label}
            </button>
          ))}
        {!career && <span className="qb-count">{st.bots} bot{st.bots === 1 ? "" : "s"}</span>}
      </div>
      {announcing && (
        <form className="quick-announce" onSubmit={(e) => { e.preventDefault(); announce(); }}>
          <input className="search grow" autoFocus maxLength={120} placeholder="A message everyone sees in the middle of the screen" value={text} onChange={(e) => setText(e.target.value)} />
          <button className="btn primary" type="submit" disabled={!text.trim()}>Send</button>
          <button className="btn" type="button" onClick={() => setAnnouncing(false)}>Cancel</button>
        </form>
      )}
    </div>
  );
}
