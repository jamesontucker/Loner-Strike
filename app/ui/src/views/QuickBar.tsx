import { useEffect, useRef, useState } from "react";
import { postJson } from "../api";
import type { MapInfo, Online } from "../types";
import { useCommand, useToast } from "../ui";

type Item = { label: string; on: () => void; pressed?: boolean; disabled?: boolean; show?: boolean };

/** A button that opens a short list of actions under it. */
function Menu({ label, items, active }: { label: string; items: Item[]; active?: boolean }) {
  const [open, setOpen] = useState(false);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: Event) => !box.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);
  const shown = items.filter((i) => i.show !== false);
  if (!shown.length) return null;
  return (
    <div className="qmenu" ref={box}>
      <button className="qb" aria-haspopup="menu" aria-expanded={open} aria-pressed={active || undefined} onClick={() => setOpen((o) => !o)}>
        {label} <span className="caret" aria-hidden="true">▾</span>
      </button>
      {open && (
        <div className="qmenu-list" role="menu">
          {shown.map((i) => (
            <button
              key={i.label}
              role="menuitemcheckbox"
              aria-checked={i.pressed ?? false}
              disabled={i.disabled}
              onClick={() => {
                i.on();
                setOpen(false);
              }}
            >
              <span className="tick" aria-hidden="true">{i.pressed ? "✓" : ""}</span>
              {i.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/** One-click actions while a game is live, under the top bar, grouped so they fit. */
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

  const round: Item[] = [
    { label: "Restart round", on: () => run("gm_match", ["restart"]) },
    { label: "End round", on: () => run("gm_do", ["endround"]) },
    { label: "+10 minutes", on: () => run("gm_do", ["addtime"]), show: st.timelimit > 0 },
    { label: "Peaceful bots", on: () => run("gm_set", ["bot_zombie", st.peaceful ? "0" : "1"]), pressed: !!st.peaceful },
  ];
  const map: Item[] = [
    { label: "Restart map", on: () => run("gm_restartmap") },
    { label: "Random map", on: randomMap },
    { label: here?.fav ? "Favourite map" : "Favourite this map", on: favourite, pressed: !!here?.fav, show: !!here },
  ];
  const you: Item[] = me
    ? [
        { label: "God mode", on: () => run("gm_player", [String(me.uid), "god"]), pressed: !!me.god },
        { label: "Noclip", on: () => run("gm_player", [String(me.uid), "noclip"]), pressed: !!me.noclip },
        { label: "Respawn me", on: () => run("gm_player", [String(me.uid), "respawn"]) },
        { label: "Heal me", on: () => run("gm_do", ["heal"]), show: !!me.alive },
        { label: "$16,000", on: () => run("gm_do", ["money"]), show: !career },
        { label: "Save my spot", on: () => run("gm_pos", ["save"]) },
        { label: "Back to my spot", on: () => run("gm_pos", ["load"]), disabled: !data.me?.haspos },
      ]
    : [];

  return (
    <div className="quick" aria-label="Quick actions">
      <div className="quick-inner">
        <button className="qb" aria-pressed={!!st.paused} onClick={() => run("gm_pause")}>{st.paused ? "Resume" : "Pause"}</button>
        {!career && <Menu label="Round" items={round} active={!!st.peaceful} />}
        {!career && <Menu label="Map" items={map} />}
        {!career && (
          <div className="qstep" role="group" aria-label="Bots">
            <button className="qb" aria-label="One bot fewer" onClick={() => run("gm_bots", ["count", String(Math.max(0, st.bots - 1))])}>−</button>
            <span>{st.bots} bot{st.bots === 1 ? "" : "s"}</span>
            <button className="qb" aria-label="One more bot" onClick={() => run("gm_bots", ["count", String(Math.min(st.maxplayers - 1, st.bots + 1))])}>+</button>
          </div>
        )}
        <Menu label="You" items={you} active={!!me?.god || !!me?.noclip} />
        <button className="qb" aria-pressed={announcing} onClick={() => setAnnouncing((a) => !a)}>Announce</button>
        <button className="qb" aria-pressed={!!st.recording} onClick={() => run("gm_do", ["demo"])}>{st.recording ? "● Stop recording" : "Record demo"}</button>
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
