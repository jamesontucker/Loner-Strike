import { memo, useDeferredValue, useMemo, useState } from "react";
import { addFromFile, currentTarget, getJsonAt, isSharedId, ON_THIS_COMPUTER, postJson, targetKey, useActions, useStable, withTarget } from "../api";
import { useAppInfo, useMaps, useServers, useSetupJob, waitForSetupJob } from "../queries";
import { useNav } from "../store";
import { usePlayDrafts } from "../stores/playDraft";
import type { MapInfo } from "../types";
import { BusyButton, Info, LoadError, SectionHead, Sheet, Switch, useCommand, useConfirm, useToast } from "../ui";
import { JobProgress } from "./JobProgress";
import { CATS, CopyFromGame, CopyToServers } from "./MapCopy";

/** Where a map came from and whether it's hidden or new to the bots: one menu, so the chips stay about map types. */
const SHOW: [string, string][] = [
  ["all", "All maps"], ["stock", "Came with the game"], ["download", "Downloaded"], ["added", "Added by you"],
  ["nonav", "Bots haven't learned it"], ["hidden", "Hidden"],
];
const TEACH_INFO = "A hidden server on this computer loads each map and a bot walks all of it, a few minutes per map. It runs in the background at low priority; the game doesn't need to be open. What the bots learn goes to your servers too.";
const RELEARN_INFO = "Bots learn the picked maps again even if they know them already, replacing what they learned before. Useful when bots get stuck on a map.";
const SEND_INFO = "Sends what bots learned about this map (its .nav file) to all your servers now. Only that file goes: a server without the map itself still needs the map.";
const SHOW_INFO = "Hidden maps stay installed but are left out of lists, the in-game map menu and map votes. Bots spend a few minutes learning a map the first time it's played. In Counter-Strike 1.6, Condition Zero's own maps (which 1.6 can't load) show only when you pick them here.";
const SORTS: [string, string][] = [["name", "A to Z"], ["name-desc", "Z to A"], ["played", "Recently played"], ["favs", "Favourites first"], ["type", "By type"], ["source", "Downloaded first"]];
const SOURCE_ORDER = ["download", "added", "stock"];
const SOURCE = { stock: "Came with the game", download: "Downloaded from GameBanana", added: "Added by you" };
/** Where a map came from: one added with "Add a map file" is filed as a download (with a negative mod) but is yours. */
const sourceOf = (m: MapInfo): MapInfo["source"] => (m.source === "download" && (m.mod ?? 0) < 0 ? "added" : m.source);
/** A map only in the other game's folder that this game can't load: a Condition Zero map
 *  in Counter-Strike 1.6 (Condition Zero loads 1.6's maps too). */
const cantLoad = (m: MapInfo) => !!m.other && m.otherGame === "Condition Zero";
/** What a map from the other game's folder says about itself: short on its card, whole on its sheet. */
const otherNote = (m: MapInfo): [string, string] =>
  cantLoad(m)
    ? ["For Condition Zero", "For Condition Zero: Counter-Strike 1.6 can't load it"]
    : [`From ${m.otherGame ?? "the other game"}`, "From Counter-Strike 1.6. Usually works in Condition Zero"];
/** Cards drawn at first: a few thousand at once makes the page slow; the rest come with Show more. */
const PAGE = 150;

/** One collator for every map sort: localeCompare builds a new one on each call. */
export const COLLATOR = new Intl.Collator();

/** What a map without a screenshot shows: its type and name, like a map-loading screen. */
function NoPicture({ name }: { name: string }) {
  const i = name.indexOf("_");
  const kind = i > 0 && i <= 5 ? name.slice(0, i) : "";
  return (
    <div className="no-pic" aria-hidden="true">
      {kind && <span className="no-pic-kind">{kind}</span>}
      <span className="no-pic-name">{kind ? name.slice(i + 1) : name}</span>
    </div>
  );
}

/** A map's card. Memoised: the Maps page has hundreds of them, so the handlers take the
 *  map instead of being made per card, and a card only redraws when its own map changes. */
export const MapCard = memo(function MapCard({ map, onClick, current, onFav, selected }: { map: MapInfo; onClick: (map: MapInfo) => void; current?: boolean; onFav?: (map: MapInfo) => void; selected?: boolean }) {
  // the picture can fail while the app is still looking for one: a newer answer in the
  // map list (thumb changes) tries again, with ?v= so the browser doesn't reuse the 404
  const [brokenFor, setBrokenFor] = useState<MapInfo["thumb"] | undefined>(undefined);
  const broken = brokenFor === map.thumb;
  const card = (
    <button className={`map-card${selected ? " selected" : ""}`} onClick={() => onClick(map)} aria-current={current ? "true" : undefined} aria-pressed={selected === undefined ? undefined : selected} title={map.title || map.name}>
      <div className={`map-pic${map.thumb === "radar" ? " radar" : ""}`}>
        {(map.thumb !== "radar" || broken) && <NoPicture name={map.name} />}
        {map.thumb && !broken && <img src={withTarget(`/thumb/${encodeURIComponent(map.name)}.png?v=${map.thumb}`)} alt="" loading="lazy" onError={() => setBrokenFor(map.thumb)} />}
        {!onFav && map.fav && <span className="fav-star" aria-label="Favourite">★</span>}
      </div>
      <span className="map-label">{map.name}</span>
      {map.other && <span className="map-sub" title={otherNote(map)[1]}>{otherNote(map)[0]}</span>}
    </button>
  );
  if (!onFav) return card;
  // the star is its own button next to the card's, laid over the picture
  return (
    <div className="card-wrap">
      {card}
      <div className="card-pic-overlay">
        <button
          type="button"
          className={`fav-toggle${map.fav ? " on" : ""}`}
          aria-label={map.fav ? `Remove ${map.name} from favourites` : `Add ${map.name} to favourites`}
          title={map.fav ? "Remove from favourites" : "Add to favourites"}
          onClick={() => onFav(map)}
        >
          {map.fav ? "★" : "☆"}
        </button>
      </div>
    </div>
  );
});

/** `current`: the map being played; `live`: a game is running to change maps on. Not the
 *  whole game state, which is new every 2 seconds. */
/** getMore: a server's way to the map browser. Your own game's Get maps opens Get more on maps
 *  (Back comes back here). */
export function Maps({ current: currentMap, live, maps, loading, reload: reloadProp, refresh: refreshProp, getMore }: { current?: string; live: boolean; maps: MapInfo[]; loading?: boolean; reload: () => void; refresh: () => void; getMore?: () => void }) {
  const reload = useStable(reloadProp);
  const refresh = useStable(refreshProp);
  const ask = useConfirm();
  const run = useCommand(refresh);
  const toast = useToast();
  const go = useNav((s) => s.go);
  const getMaps = currentTarget() ? getMore : () => go("more", "maps");
  const game = useAppInfo().data?.game?.id;
  /** No game running: the map goes into Play's Start a game form, and Play opens. */
  const startOn = (name: string) => {
    if (game) usePlayDrafts.getState().set(game, { map: name });
    setPicked(null);
    go("play");
  };
  // only for the error state and Try again: the list itself comes from the caller
  const mapsQ = useMaps();
  const isLoading = loading ?? mapsQ.isLoading;
  const [cat, setCat] = useState("all");
  const [show, setShow] = useState("all");
  const [q, setQ] = useState("");
  // typing stays quick: the list filters a moment behind the box
  const search = useDeferredValue(q);
  const [sort, setSortRaw] = useState(() => {
    try {
      return localStorage.getItem("ls.mapSort") || "name";
    } catch {
      return "name";
    }
  });
  // how many cards to draw: back to one page as soon as the filters change (in the same
  // render, so a big list never draws at the old limit first)
  const filterKey = `${cat}|${show}|${search}|${sort}`;
  const [lim, setLim] = useState({ key: filterKey, n: PAGE });
  const limit = lim.key === filterKey ? lim.n : PAGE;
  const setLimit = (n: number) => setLim({ key: filterKey, n });
  const setSort = (v: string) => {
    setSortRaw(v);
    try {
      localStorage.setItem("ls.mapSort", v);
    } catch {
      /* private window */
    }
  };
  const [picked, setPicked] = useState<MapInfo | null>(null);
  const { run: once, busy } = useActions();
  const key = (what: string) => `maps:${targetKey()}:${what}`;
  const changeMap = (name: string) => once(key("change"), () => run("gm_map", [name]), `Changing the map to ${name}…`);
  const current = currentMap?.toLowerCase();

  // teaching bots maps: this computer's own game only (from its window or the owner's phone)
  const canTeach = !currentTarget();
  // copying a map file to a server: from the computer's window only
  const canCopy = ON_THIS_COMPUTER && canTeach;
  const [picking, setPicking] = useState(false);
  const [chosen, setChosen] = useState<Set<string>>(() => new Set());
  const [relearn, setRelearn] = useState(false);
  // copying maps between your game and servers: on the computer only
  const server = currentTarget();
  // a server a friend shared with you: maps come from GameBanana there, not from this computer
  const shared = isSharedId(server);
  const serversQ = useServers(ON_THIS_COMPUTER);
  const serverList = serversQ.data?.servers ?? [];
  const hasServers = serverList.some((s) => s.ready);
  const serverName = serverList.find((s) => s.id === server)?.name ?? "this server";
  const [copyMap, setCopyMap] = useState<string | null>(null);
  const [copyFrom, setCopyFrom] = useState(false);
  const jobQ = useSetupJob();
  const learning = jobQ.data?.state === "running" && jobQ.data.kind === "navlearn";
  const togglePick = useStable((m: MapInfo) => !cantLoad(m) && setChosen((c) => {
    const n = new Set(c);
    if (n.has(m.name)) n.delete(m.name);
    else n.add(m.name);
    return n;
  }));
  const stopPicking = () => {
    setPicking(false);
    setChosen(new Set());
  };
  const teach = () => once(key("learn"), async () => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/navs/learn", { maps: [...chosen], relearn }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    stopPicking();
    const j = await waitForSetupJob();
    const res = await getJsonAt<{ result?: { message?: string } | null }>("/api/navs/learn", null).catch(() => null);
    const msg = res?.result?.message;
    if (!j) toast("Lost track of the bots learning maps. Check the maps list to see what they learned.", true);
    else if (j.state === "done") toast(msg || "Bots learned the maps.");
    else toast(msg || j.error || "Bots couldn't learn those maps.", true);
    reload();
  }, "Teaching bots maps…");
  const stopTeaching = () => once(key("learn-stop"), async () => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/navs/learn/stop").catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) toast(r.error, true);
  }, "Stopping…");
  const sendNav = (name: string) => once(key(`send:${name}`), async () => {
    type Sent = { ok?: boolean; error?: string; message?: string; sent?: number };
    const r = await postJson<Sent>("/api/navs/send", { map: name }).catch((): Sent => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    toast(r.message || "Sent.");
  }, `Sending ${name} to your servers…`);

  // the other game's maps (a Condition Zero map in CS 1.6, a CS 1.6 map in Condition
  // Zero) only show when asked for
  const otherGame = maps.find((m) => m.other)?.otherGame;
  const shows = otherGame ? [...SHOW, ["other", `${otherGame} maps`] as [string, string]] : SHOW;
  const shown = useMemo(() => {
    const query = search.trim().toLowerCase();
    let list = show === "other" ? maps.filter((m) => m.other) : maps.filter((m) => !m.other && (show === "hidden" ? m.hidden : !m.hidden));
    if (show === "nonav") list = list.filter((m) => !m.nav);
    else if (show === "stock" || show === "download" || show === "added") list = list.filter((m) => sourceOf(m) === show);
    if (cat === "fav") list = list.filter((m) => m.fav);
    else if (cat === "recent") list = list.filter((m) => m.recent >= 0).sort((a, b) => a.recent - b.recent);
    else if (cat !== "all") list = list.filter((m) => m.cat === cat);
    if (query) list = list.filter((m) => `${m.name} ${m.title ?? ""} ${m.by}`.toLowerCase().includes(query));
    if (cat !== "recent") {
      const byName = (a: MapInfo, b: MapInfo) => COLLATOR.compare(a.name, b.name);
      const played = (m: MapInfo) => (m.recent >= 0 ? m.recent : 1e9);
      const order: Record<string, (a: MapInfo, b: MapInfo) => number> = {
        name: byName,
        "name-desc": (a, b) => byName(b, a),
        played: (a, b) => played(a) - played(b) || byName(a, b),
        favs: (a, b) => Number(b.fav) - Number(a.fav) || byName(a, b),
        type: (a, b) => COLLATOR.compare(a.cat, b.cat) || byName(a, b),
        source: (a, b) => SOURCE_ORDER.indexOf(sourceOf(a)) - SOURCE_ORDER.indexOf(sourceOf(b)) || byName(a, b),
      };
      list = [...list].sort(order[sort] ?? byName);
    }
    return list;
  }, [maps, cat, show, search, sort]);
  // other filters start from the top again

  const remove = async (map: MapInfo) => {
    if (!map.mod) return;
    const others = (map.pack ?? []).filter((n) => n.toLowerCase() !== map.name.toLowerCase());
    const msg = others.length
      ? `${map.name} came in "${map.modName ?? "a pack"}" with ${others.length} other map${others.length === 1 ? "" : "s"} (${others.slice(0, 6).join(", ")}${others.length > 6 ? " and more" : ""}). Removing it removes the whole download. Continue?`
      : `Remove ${map.name}? Its files are deleted.`;
    if (!(await ask(msg, { ok: "Remove", danger: true }))) return;
    await once(key(`remove:${map.mod}`), async () => {
      const r = await postJson<{ ok?: boolean; error?: string; maps?: string[] }>("/api/gb/uninstall", { mod: map.mod, section: "maps" }).catch((): { ok?: boolean; error?: string; maps?: string[] } => ({ error: "Couldn't remove it." }));
      if (r.error) return toast(r.error, true);
      toast(`Removed ${(r.maps ?? [map.name]).join(", ")}`);
      setPicked(null);
      reload();
    }, `Removing ${map.name}…`);
  };

  const flag = (map: MapInfo, f: "fav" | "hidden", on: boolean) => once(key(`flag:${f}:${map.name}`), async () => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/mapflag", { flag: f, map: map.name, on }).catch(() => ({ error: "Couldn't save that." }));
    if (r.error) return toast(r.error, true);
    if (f === "hidden") toast(on ? `${map.name} is hidden. Find it with Show: Hidden.` : `${map.name} is back in the list.`);
    if (f === "fav") toast(on ? `${map.name} added to favourites` : `${map.name} removed from favourites`);
    setPicked(null);
    reload();
  }, f === "fav" ? (on ? `Adding ${map.name} to favourites…` : `Removing ${map.name} from favourites…`) : on ? `Hiding ${map.name}…` : `Showing ${map.name} again…`);

  // the same function for every card, so the cards' memo holds
  const toggleFav = useStable((m: MapInfo) => flag(m, "fav", !m.fav));

  return (
    <div className="stack">
      <div>
        <SectionHead title="Maps" note={live ? "Pick a map to play it in the current mode." : currentTarget() ? "Start the server to change maps." : "Pick a map to start a game on it."}>
          {canTeach && !picking && (
            <span className="row">
              <BusyButton busy={learning || busy(key("learn"))} label="Teach bots maps" busyLabel="Bots are learning…" onClick={() => setPicking(true)} />
              <Info text={TEACH_INFO} />
            </span>
          )}
          {getMaps && <button className="btn" onClick={getMaps}>Get maps</button>}
          {ON_THIS_COMPUTER && server && !shared && (
            <button className="btn" onClick={() => setCopyFrom(true)}>Copy maps from your game</button>
          )}
          {ON_THIS_COMPUTER && !shared && (
            <BusyButton busy={busy(key("add-file"))} label="Add a map file" busyLabel="Adding…" onClick={() => once(key("add-file"), async () => {
              const r = await addFromFile("maps");
              if (r) toast(r.message, !r.ok);
              if (r?.ok) reload();
            }, "Adding a map file…")} />
          )}
          <span className="random-map" title={live ? undefined : "Start the game to change maps."}>
            <BusyButton busy={busy(key("change"))} disabled={!live} label="Random map" busyLabel="Changing…" onClick={() => {
              const pool = shown.filter((m) => !m.hidden && !m.other && m.name.toLowerCase() !== current);
              if (!live) return;
              if (!pool.length) return toast("No other map matches these filters.", true);
              changeMap(pool[Math.floor(Math.random() * pool.length)].name);
            }} />
          </span>
        </SectionHead>
        <div className="toolbar">
          <input className="search" type="search" aria-label="Search maps" placeholder="Search maps, authors or names like “cobble”" value={q} onChange={(e) => setQ(e.target.value)} />
          <label className="sort">
            <span>Show</span>
            <select value={show} onChange={(e) => setShow(e.target.value)}>
              {shows.map(([id, label]) => <option key={id} value={id}>{label}</option>)}
            </select>
          </label>
          <Info text={SHOW_INFO} />
          <label className="sort">
            <span>Sort</span>
            <select value={sort} onChange={(e) => setSort(e.target.value)} disabled={cat === "recent"}>
              {SORTS.map(([id, label]) => <option key={id} value={id}>{label}</option>)}
            </select>
          </label>
          <div className="chips" role="group" aria-label="Map type">
            {CATS.map(([id, label]) => (
              <button key={id} className="chip" aria-pressed={cat === id} onClick={() => setCat(id)}>{label}</button>
            ))}
          </div>
        </div>
      </div>
      {canTeach && learning && (
        <div className="panel stack-tight">
          <JobProgress title="Teaching bots maps…" onEnd={() => reload()} />
          <div className="row">
            <BusyButton className="btn" busy={busy(key("learn-stop"))} label="Stop" busyLabel="Stopping…" onClick={stopTeaching} />
            <span className="muted small-note">The map in progress is left unfinished. Maps already learned are kept.</span>
          </div>
        </div>
      )}
      {canTeach && picking && (
        <div className="panel stack-tight">
          <h3>Teach bots maps</h3>
          <p className="muted measure">Pick the maps below, then start. {chosen.size ? `${chosen.size} map${chosen.size === 1 ? "" : "s"} picked.` : "No maps picked yet."}</p>
          <div className="row">
            <button className="btn" onClick={() => setChosen(new Set(shown.filter((m) => !m.nav && !m.other).map((m) => m.name)))}>Pick the ones bots don't know</button>
            <button className="btn" disabled={!chosen.size} onClick={() => setChosen(new Set())}>Clear</button>
          </div>
          <div className="setting">
            <span className="name">Re-learn<Info text={RELEARN_INFO} /></span>
            <Switch on={relearn} label="Re-learn maps bots already know" onChange={setRelearn} />
          </div>
          <div className="row">
            <BusyButton className="btn primary" busy={busy(key("learn"))} disabled={!chosen.size || learning} label="Start teaching" busyLabel="Starting…" onClick={teach} />
            <button className="btn" onClick={stopPicking}>Cancel</button>
          </div>
        </div>
      )}
      {!maps.length && mapsQ.isError && !isLoading ? (
        <LoadError text="Couldn't load the maps." onRetry={() => void mapsQ.refetch()} />
      ) : !maps.length && !isLoading ? (
        <div className="panel">
          <h3>No maps yet</h3>
          <p className="muted measure">
            {currentTarget()
              ? "This server has no maps. Get some with Get maps, or check the server's game files."
              : "Loner-Strike can't find any maps in your game. Get some from Get more, or add a map file."}
          </p>
          {getMaps && <div className="row"><button className="btn primary" onClick={getMaps}>Get maps</button></div>}
        </div>
      ) : (
        <div className="maps grid">
          {shown.slice(0, limit).map((m) => (
            picking
              ? <MapCard key={m.name} map={m} selected={chosen.has(m.name)} onClick={togglePick} />
              : <MapCard key={m.name} map={m} current={m.name.toLowerCase() === current} onClick={setPicked} onFav={toggleFav} />
          ))}
          {!shown.length && (maps.length ? <p className="muted">No maps match. <button type="button" className="link inline" onClick={() => { setQ(""); setCat("all"); setShow("all"); }}>Show all maps</button></p> : Array.from({ length: 10 }, (_, i) => <div key={i} className="map-card ghost" aria-hidden="true"><div className="map-pic" /><span className="ghost-label" /></div>))}
        </div>
      )}
      {shown.length > limit && (
        <div className="row center">
          <button className="btn" onClick={() => setLimit(limit + PAGE * 2)}>Show more ({shown.length - limit} left)</button>
        </div>
      )}

      {picked && (
        <Sheet label={picked.name} onClose={() => setPicked(null)}>
          <div className={`sheet-pic${picked.thumb === "radar" ? " radar" : ""}`}>
            {picked.thumb !== "radar" && <NoPicture name={picked.name} />}
            {picked.thumb && <img src={withTarget(`/thumb/${encodeURIComponent(picked.name)}.png?v=${picked.thumb}`)} alt="" onError={(e) => (e.currentTarget.style.display = "none")} />}
          </div>
          <div className="sheet-body">
            <h2 className="map-title">{picked.name}</h2>
            <p className="muted">
              {picked.title && picked.title !== picked.name ? `${picked.title} · ` : ""}
              {picked.by !== "Other" ? `by ${picked.by}` : ""}
              {!picked.nav && " · Bots haven't learned this map yet. They learn it the first time it's played (a few minutes)."}
            </p>
            <p className="muted">
              {SOURCE[sourceOf(picked)]}
              {picked.source === "download" && (picked.mod ?? 0) > 0 && picked.modName ? `: ${picked.modName}` : ""}
              {picked.pack && picked.pack.length > 1 ? ` (a pack of ${picked.pack.length} maps)` : ""}
            </p>
            {picked.other && <p className="muted">{otherNote(picked)[1]}.</p>}
            {!live && currentTarget() && !cantLoad(picked) && <p className="muted">Start the server to play it.</p>}
            {!live && !currentTarget() && picked.hidden && <p className="muted">Show it again to start a game on it.</p>}
            <div className="row">
              {cantLoad(picked) ? null : live ? (
                <BusyButton className="btn primary" busy={busy(key("change"))} label={`Play ${picked.name}`} busyLabel="Changing…" onClick={() => { setPicked(null); changeMap(picked.name); }} />
              ) : (
                !currentTarget() && !picked.hidden && <button className="btn primary" onClick={() => startOn(picked.name)}>Start a game on {picked.name}</button>
              )}
              <BusyButton busy={busy(key(`flag:fav:${picked.name}`))} label={picked.fav ? "Remove from favourites" : "Favourite"} busyLabel="Saving…" onClick={() => flag(picked, "fav", !picked.fav)} />
              <BusyButton busy={busy(key(`flag:hidden:${picked.name}`))} label={picked.hidden ? "Show again" : "Hide"} busyLabel={picked.hidden ? "Showing…" : "Hiding…"} onClick={() => flag(picked, "hidden", !picked.hidden)} />
              {canTeach && picked.nav && (
                <span className="row">
                  <BusyButton busy={busy(key(`send:${picked.name}`))} label="Send to servers" busyLabel="Sending…" onClick={() => sendNav(picked.name)} />
                  <Info text={SEND_INFO} />
                </span>
              )}
              {canCopy && hasServers && (
                <button className="btn" onClick={() => { setCopyMap(picked.name); setPicked(null); }}>Copy to a server</button>
              )}
              {ON_THIS_COMPUTER && picked.source === "download" && picked.mod && (
                <BusyButton className="btn danger" busy={busy(key(`remove:${picked.mod}`))} label="Remove" busyLabel="Removing…" onClick={() => remove(picked)} />
              )}
              <button className="btn" onClick={() => setPicked(null)}>Close</button>
            </div>
          </div>
        </Sheet>
      )}
      {canCopy && copyMap && <CopyToServers map={copyMap} onClose={() => setCopyMap(null)} />}
      {ON_THIS_COMPUTER && server && copyFrom && <CopyFromGame server={server} name={serverName} onClose={() => setCopyFrom(false)} onDone={reload} />}
    </div>
  );
}
