import { useEffect, useRef, useState } from "react";
import { postJson, targetKey, useActions } from "../api";
import { useNav } from "../store";
import { sharedRole } from "../queries";
import { botsNote, botTarget, type MapInfo, type Online } from "../types";
import { Menu, type MenuItem } from "../menu";
import { useCommand, useToast } from "../ui";

type Item = MenuItem;

/** One-click actions while a game is live, under the top bar, grouped so they fit. */
export function QuickBar({ data, maps, refresh, reloadMaps }: { data: Online; maps: MapInfo[]; refresh: () => void; reloadMaps: () => void }) {
  const run = useCommand(refresh);
  const { run: once, busy } = useActions();
  const toast = useToast();
  const go = useNav((s) => s.go);
  const onServer = useNav((s) => s.server !== null);
  const [announcing, setAnnouncing] = useState(false);
  // a Moderator's map picker (they have no Maps page)
  const [picking, setPicking] = useState(false);
  const [pick, setPick] = useState("");
  const announceButton = useRef<HTMLButtonElement>(null);
  const [text, setText] = useState("");
  const st = data.state;
  const me = data.players.find((p) => p.host);
  const career = !!st.career;
  const here = maps.find((m) => m.name.toLowerCase() === st.map.toLowerCase());

  const randomMap = () => {
    const pool = maps.filter((m) => !m.hidden && m.name.toLowerCase() !== st.map.toLowerCase());
    if (!pool.length) return;
    const name = pool[Math.floor(Math.random() * pool.length)].name;
    // same key as the Maps page, so a map change is only sent once
    once(`maps:${targetKey()}:change`, () => run("gm_map", [name]), `Changing the map to ${name}…`);
  };
  const favourite = async () => {
    const r = await postJson<{ error?: string }>("/api/mapflag", { flag: "fav", map: st.map, on: !here?.fav }).catch(() => ({ error: "Couldn't save that." }));
    if (r.error) return toast(r.error, true);
    toast(here?.fav ? "Removed from favourites" : "Added to favourites");
    reloadMaps();
  };
  // the bot count asked for (the saved target, not the bots playing right now)
  const target = botTarget(st);
  const note = botsNote(st);
  // the bot count just asked for, shown until the game reports it (or a few seconds pass)
  const [want, setWant] = useState<{ n: number; at: number } | null>(null);
  useEffect(() => {
    if (!want) return;
    if (want.n === target) return setWant(null);
    const t = setTimeout(() => setWant(null), Math.max(0, 3000 - (Date.now() - want.at)));
    return () => clearTimeout(t);
  }, [want, target]);
  const botCount = want?.n ?? target;
  const botsKey = `quick:${targetKey()}:bots`;
  const botsBusy = busy(botsKey);
  const setBots = (n: number) => once(botsKey, async () => {
    setWant({ n, at: Date.now() });
    if (!(await run("gm_bots", ["count", String(n)]))) setWant(null);
  }, n > botCount ? "Adding a bot…" : "Removing a bot…");
  const closeAnnounce = () => {
    setAnnouncing(false);
    announceButton.current?.focus();
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
    { label: "Add 10 minutes", on: () => run("gm_do", ["addtime"]), show: st.timelimit > 0 },
  ];
  const botMenu: Item[] = [
    { label: "Peaceful bots", on: () => run("gm_set", ["bot_zombie", st.peaceful ? "0" : "1"]), pressed: !!st.peaceful, title: "Bots stop attacking. Turn it off to fight again." },
    // (on a server's page its own Bots tab has them; this would leave the server)
    ...(onServer ? [] : [{ label: "All bot settings", on: () => go("bots") }]),
  ];
  const map: Item[] = [
    { label: "Restart map", on: () => once(`quick:${targetKey()}:restartmap`, () => run("gm_restartmap"), "Restarting the map…") },
    { label: "Random map", on: randomMap },
    { label: "Change map…", on: () => setPicking(true), show: sharedRole() === "moderator" },
    { label: here?.fav ? "Favourite map" : "Favourite this map", on: favourite, pressed: !!here?.fav, show: !!here && sharedRole() !== "moderator" },
    {
      label: "Start a mode vote",
      on: () => once(`quick:${targetKey()}:modevote`, () => run("gm_votemode"), "Starting a mode vote…"),
      show: data.settings.some((s) => s.cvar === "gm_modevote") && sharedRole() !== "moderator",
      title: "Everyone votes for the next mode, then a map for it",
    },
  ];
  const you: Item[] = me
    ? [
        { label: "God mode", on: () => run("gm_player", [String(me.uid), "god"]), pressed: !!me.god },
        { label: "Noclip", on: () => run("gm_player", [String(me.uid), "noclip"]), pressed: !!me.noclip },
        { label: "Respawn me", on: () => run("gm_player", [String(me.uid), "respawn"]) },
        { label: "Heal me", on: () => run("gm_do", ["heal"]), show: !!me.alive },
        { label: "Give me $16,000", on: () => run("gm_do", ["money"]), show: !career },
        { label: "Save my spot", on: () => run("gm_pos", ["save"]) },
        { label: "Back to my spot", on: () => run("gm_pos", ["load"]), disabled: !data.me?.haspos },
      ]
    : [];

  // a Moderator on a shared server: change or restart the map, and Announce
  const mod = sharedRole() === "moderator";
  return (
    <div className="quick" aria-label="Quick actions">
      <div className="quick-inner">
        {/* toggles: a double click would undo itself, so one at a time */}
        {!mod && <button className="qb" aria-pressed={!!st.paused} disabled={busy(`quick:${targetKey()}:pause`)} onClick={() => once(`quick:${targetKey()}:pause`, () => run("gm_pause"), st.paused ? "Resuming…" : "Pausing…")}>{st.paused ? "Resume" : "Pause"}</button>}
        {!career && !mod && <Menu label="Round" items={round} />}
        {!career && <Menu label="Map" items={map} />}
        {!career && !mod && (
          <div className="qstep" role="group" aria-label="Bots">
            <button className="qb" aria-label="One bot fewer" disabled={botsBusy || botCount <= 0} aria-busy={botsBusy || undefined} onClick={() => setBots(Math.max(0, botCount - 1))}>−</button>
            <Menu
              label={`${botCount} bot${botCount === 1 ? "" : "s"}`}
              ariaLabel={`${botCount} bot${botCount === 1 ? "" : "s"}${note && !want ? `, ${note.toLowerCase()}` : ""}: bot options`}
              groups={[{ items: botMenu, note: note && !want ? note : undefined }]}
              active={!!st.peaceful}
            />
            <button className="qb" aria-label="One more bot" disabled={botsBusy || botCount >= st.maxplayers - 1} aria-busy={botsBusy || undefined} onClick={() => setBots(Math.min(st.maxplayers - 1, botCount + 1))}>+</button>
          </div>
        )}
        {!mod && <Menu label="You" items={you} active={!!me?.god || !!me?.noclip} />}
        <button ref={announceButton} className="qb" aria-pressed={announcing} onClick={() => setAnnouncing((a) => !a)}>Announce</button>
        {/* records your own game: a server has none (its HLTV records it) */}
        {targetKey() === "local" && (
          <button className="qb" aria-pressed={!!st.recording} title="Records the game as you see it to a demo file in your game folder. Watch it later with viewdemo in the console." disabled={busy(`quick:${targetKey()}:demo`)} onClick={() => once(`quick:${targetKey()}:demo`, () => run("gm_do", ["demo"]), st.recording ? "Stopping the recording…" : "Starting a recording…")}>{st.recording ? "● Stop recording" : "Record demo"}</button>
        )}
      </div>
      {picking && (
        <form className="quick-announce" onSubmit={(e) => { e.preventDefault(); if (pick) { once(`maps:${targetKey()}:change`, () => run("gm_map", [pick]), `Changing the map to ${pick}…`); setPicking(false); } }} onKeyDown={(e) => e.key === "Escape" && setPicking(false)}>
          <select className="grow" autoFocus value={pick} onChange={(e) => setPick(e.target.value)} aria-label="Map">
            <option value="">Choose a map</option>
            {maps.filter((m) => !m.hidden).map((m) => <option key={m.name} value={m.name}>{m.name}</option>)}
          </select>
          <button className="btn primary" type="submit" disabled={!pick}>Change map</button>
          <button className="btn" type="button" onClick={() => setPicking(false)}>Cancel</button>
        </form>
      )}
      {announcing && (
        <form className="quick-announce" onSubmit={(e) => { e.preventDefault(); announce(); }} onKeyDown={(e) => e.key === "Escape" && closeAnnounce()}>
          <input className="search grow" autoFocus maxLength={120} placeholder="A message everyone sees in the middle of the screen" value={text} onChange={(e) => setText(e.target.value)} />
          <button className="btn primary" type="submit" disabled={!text.trim()}>Send</button>
          <button className="btn" type="button" onClick={closeAnnounce}>Cancel</button>
        </form>
      )}
    </div>
  );
}
