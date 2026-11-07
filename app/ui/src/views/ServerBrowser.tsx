import { useEffect, useMemo, useRef, useState } from "react";
import { getJsonAt, ON_THIS_COMPUTER, postJson, useActions } from "../api";
import { BusyButton, Sheet, Skeleton, useToast } from "../ui";

/** Find servers, after Steam's old server browser: Internet (the Loner-Strike server list,
 *  the servers hosts chose to share), Favorites, History and LAN. Every server is asked from
 *  this computer, so the latency is yours. */

type Tab = "internet" | "favorites" | "history" | "lan";
type Info = { name: string; map: string; game: string; players: number; bots: number; max_players: number; password: boolean; secure: boolean };
type Row = { address: string; info: Info | null; ping: number | null; answered: boolean; last_played?: number };
type Player = { name: string; score: number; time: number };
type Sort = { key: "name" | "game" | "players" | "map" | "ping" | "last"; down: boolean };
type Filters = { game: "" | "cstrike" | "czero"; map: string; people: boolean; notFull: boolean; noPassword: boolean; ping: number };

const TABS: [Tab, string][] = [["internet", "Internet"], ["favorites", "Favorites"], ["history", "History"], ["lan", "LAN"]];
const GAMES: Record<string, string> = { cstrike: "Counter-Strike", czero: "Condition Zero" };
const PINGS = [0, 50, 100, 150, 250];
const NO_FILTERS: Filters = { game: "", map: "", people: false, notFull: false, noPassword: false, ping: 0 };
const EMPTY: Record<Tab, string> = {
  internet: "No servers are on the list right now. Host one under Servers and turn on On the server list in its settings.",
  favorites: "No favorites yet. Add a server by its address, or pick one in another tab and press Add to favorites.",
  history: "Servers you join from here show up here.",
  lan: "No games found on your network.",
};

const humans = (i: Info) => i.players - i.bots;
const ago = (unix: number) => {
  const m = Math.round((Date.now() / 1000 - unix) / 60);
  return m < 1 ? "Just now" : m < 60 ? `${m} min ago` : m < 1440 ? `${Math.round(m / 60)} h ago` : `${Math.round(m / 1440)} d ago`;
};

export function ServerBrowser() {
  const toast = useToast();
  const { run, busy } = useActions();
  const [tab, setTab] = useState<Tab>("internet");
  const [lists, setLists] = useState<Partial<Record<Tab, Row[]>>>({});
  const [errors, setErrors] = useState<Partial<Record<Tab, string>>>({});
  const [favorites, setFavorites] = useState<string[]>([]);
  const [picked, setPicked] = useState<string | null>(null);
  const [sort, setSort] = useState<Sort>({ key: "players", down: true });
  const [filters, setFilters] = useState<Filters>(NO_FILTERS);
  const [showFilters, setShowFilters] = useState(false);
  const [details, setDetails] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const loading = busy(`browser:${tab}`);
  const asked = useRef<Set<Tab>>(new Set());

  const refresh = (t: Tab = tab) =>
    run(`browser:${t}`, async () => {
      type Reply = { servers?: Row[]; favorites?: string[]; error?: string };
      const r = await getJsonAt<Reply>(`/api/browser/list?tab=${t}`, null).catch((): Reply => ({ error: "Couldn't reach the app." }));
      setLists((l) => ({ ...l, [t]: r.servers ?? [] }));
      setErrors((e) => ({ ...e, [t]: r.error ?? "" }));
      if (r.favorites) setFavorites(r.favorites);
    }, t === "lan" ? "Looking for games on your network…" : "Asking the servers…");

  // each tab asks its servers the first time it's opened
  useEffect(() => {
    if (!ON_THIS_COMPUTER || asked.current.has(tab)) return;
    asked.current.add(tab);
    refresh(tab);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tab]);

  const rows = lists[tab] ?? [];
  const shown = useMemo(() => {
    const ok = (r: Row) => {
      const i = r.info;
      if (!i) return !(filters.game || filters.map || filters.people || filters.notFull || filters.noPassword || filters.ping);
      if (filters.game && i.game !== filters.game) return false;
      if (filters.map && !i.map.toLowerCase().includes(filters.map.trim().toLowerCase())) return false;
      if (filters.people && humans(i) === 0) return false;
      if (filters.notFull && i.players >= i.max_players) return false;
      if (filters.noPassword && i.password) return false;
      if (filters.ping && (r.ping === null || r.ping > filters.ping)) return false;
      return true;
    };
    const key = (r: Row): string | number => {
      const i = r.info;
      switch (sort.key) {
        case "name": return (i?.name ?? r.address).toLowerCase();
        case "game": return i?.game ?? "";
        case "players": return i ? humans(i) * 100 + i.players : -1;
        case "map": return i?.map.toLowerCase() ?? "";
        case "ping": return r.ping ?? 99999;
        case "last": return r.last_played ?? 0;
      }
    };
    return rows.filter(ok).sort((a, b) => {
      const [x, y] = [key(a), key(b)];
      const c = x < y ? -1 : x > y ? 1 : 0;
      return sort.down ? -c : c;
    });
  }, [rows, filters, sort]);
  const row = rows.find((r) => r.address === picked) ?? null;
  const isFavorite = !!picked && favorites.includes(picked);

  const connect = (r: Row, password = "") =>
    run("browser:connect", async () => {
      type Reply = { ok?: boolean; error?: string };
      const res = await postJson<Reply>("/api/browser/connect", { address: r.address, password, game: r.info?.game ?? "cstrike" }).catch((): Reply => ({ error: "Couldn't reach the app." }));
      if (res.error) return toast(res.error, true);
      toast(`Joining ${r.info?.name ?? r.address}`);
      setDetails(null);
    }, `Joining ${r.info?.name ?? r.address}…`);
  // a password-protected server asks for it in its info window first
  const join = (r: Row) => (r.info?.password ? setDetails(r.address) : connect(r));

  const favorite = (address: string, on: boolean) =>
    run("browser:favorite", async () => {
      type Reply = { favorites?: string[]; error?: string };
      const res = await postJson<Reply>("/api/browser/favorite", { address, on }).catch((): Reply => ({ error: "Couldn't reach the app." }));
      if (res.error) return toast(res.error, true);
      if (res.favorites) setFavorites(res.favorites);
      toast(on ? "Added to favorites" : "Removed from favorites");
      // the Favorites tab asks again the next time it's open
      asked.current.delete("favorites");
      if (tab === "favorites") refresh("favorites");
    }, on ? "Adding to favorites…" : "Removing from favorites…");

  if (!ON_THIS_COMPUTER) {
    return (
      <div className="stack">
        <h2>Find servers</h2>
        <p className="muted">Find servers is on the computer, where the game is.</p>
      </div>
    );
  }

  const head = (key: Sort["key"], label: string, cls: string) => (
    <button type="button" role="columnheader" className={cls} aria-sort={sort.key === key ? (sort.down ? "descending" : "ascending") : "none"} onClick={() => setSort((s) => ({ key, down: s.key === key ? !s.down : key === "players" || key === "last" }))}>
      {label}
      {sort.key === key && <span aria-hidden="true">{sort.down ? " ▾" : " ▴"}</span>}
    </button>
  );
  const hidden = rows.length - shown.length;

  return (
    <div className="stack browser">
      <h2>Find servers</h2>
      <div className="browser-window">
        <div className="browser-tabs" role="tablist" aria-label="Server lists">
          {TABS.map(([id, label]) => (
            <button key={id} type="button" role="tab" aria-selected={tab === id} onClick={() => { setTab(id); setPicked(null); }}>{label}</button>
          ))}
        </div>
        <div className="browser-table" role="grid" aria-label={`${TABS.find((t) => t[0] === tab)![1]} servers`} aria-busy={loading}>
          <div className={`browser-row head${tab === "history" ? " with-last" : ""}`} role="row">
            <span className="c-lock" role="columnheader" aria-label="Password"><Lock /></span>
            <span className="c-vac" role="columnheader" aria-label="Valve Anti-Cheat"><Shield /></span>
            {head("name", "Servers", "c-name")}
            {head("game", "Game", "c-game")}
            {head("players", "Players", "c-players")}
            {head("map", "Map", "c-map")}
            {head("ping", "Latency", "c-ping")}
            {tab === "history" && head("last", "Last played", "c-last")}
          </div>
          <div className="browser-body">
            {loading && rows.length === 0 ? (
              <div className="browser-note"><Skeleton rows={3} /></div>
            ) : errors[tab] ? (
              <p className="browser-note err-text">{errors[tab]}</p>
            ) : shown.length === 0 ? (
              <p className="browser-note">{rows.length ? "No servers match the filters." : EMPTY[tab]}</p>
            ) : (
              shown.map((r) => {
                const i = r.info;
                return (
                  <div
                    key={r.address}
                    role="row"
                    tabIndex={0}
                    aria-selected={picked === r.address}
                    className={`browser-row${r.answered ? "" : " quiet"}${tab === "history" ? " with-last" : ""}`}
                    onClick={() => setPicked(r.address)}
                    onDoubleClick={() => join(r)}
                    onKeyDown={(e) => { if (e.key === "Enter") join(r); }}
                  >
                    <span className="c-lock" role="gridcell">{i?.password && <Lock label="Password" />}</span>
                    <span className="c-vac" role="gridcell">{i?.secure && <Shield label="Valve Anti-Cheat" />}</span>
                    <span className="c-name" role="gridcell" title={r.address}>{i?.name || r.address}</span>
                    <span className="c-game" role="gridcell">{i ? GAMES[i.game] ?? i.game : ""}</span>
                    <span className="c-players" role="gridcell" title={i ? `${humans(i)} people, ${i.bots} bots` : undefined}>{i ? `${i.players} / ${i.max_players}` : ""}</span>
                    <span className="c-map" role="gridcell">{i?.map ?? ""}</span>
                    <span className="c-ping" role="gridcell">{r.answered && r.ping !== null ? r.ping : r.answered ? "" : "Not answering"}</span>
                    {tab === "history" && <span className="c-last" role="gridcell">{r.last_played ? ago(r.last_played) : ""}</span>}
                  </div>
                );
              })
            )}
          </div>
        </div>

        {showFilters && <FilterBar filters={filters} setFilters={setFilters} />}

        <div className="browser-bar">
          <button type="button" className="btn small" aria-expanded={showFilters} onClick={() => setShowFilters((v) => !v)}>{showFilters ? "Hide filters" : "Change filters"}</button>
          <span className="muted small-note browser-count" role="status">
            {loading ? (tab === "lan" ? "Looking on your network…" : "Asking the servers…") : `${shown.length} ${shown.length === 1 ? "server" : "servers"}${hidden ? `, ${hidden} hidden by filters` : ""}`}
          </span>
          <div className="row browser-actions">
            {tab === "favorites" && <button type="button" className="btn small" onClick={() => setAdding(true)}>Add a server</button>}
            {picked && (
              <BusyButton className="btn small" busy={busy("browser:favorite")} label={isFavorite ? "Remove from favorites" : "Add to favorites"} busyLabel="Saving…" onClick={() => favorite(picked, !isFavorite)} />
            )}
            <button type="button" className="btn small" disabled={!row} onClick={() => row && setDetails(row.address)}>Server info</button>
            <BusyButton className="btn small" busy={loading} label="Refresh all" busyLabel="Refreshing…" onClick={() => refresh()} />
            <BusyButton className="btn primary small" busy={busy("browser:connect")} disabled={!row} label="Connect" busyLabel="Joining…" onClick={() => row && join(row)} />
          </div>
        </div>
      </div>

      {details && <ServerInfo address={details} row={rows.find((r) => r.address === details)} onClose={() => setDetails(null)} onConnect={connect} joining={busy("browser:connect")} />}
      {adding && <AddServer onClose={() => setAdding(false)} onAdd={(a) => { setAdding(false); favorite(a, true); }} />}
    </div>
  );
}

function FilterBar({ filters, setFilters }: { filters: Filters; setFilters: (f: Filters) => void }) {
  const set = (p: Partial<Filters>) => setFilters({ ...filters, ...p });
  return (
    <div className="browser-filters">
      <label className="field">
        <span>Game</span>
        <select value={filters.game} onChange={(e) => set({ game: e.target.value as Filters["game"] })}>
          <option value="">All</option>
          <option value="cstrike">Counter-Strike</option>
          <option value="czero">Condition Zero</option>
        </select>
      </label>
      <label className="field">
        <span>Map</span>
        <input className="search" value={filters.map} onChange={(e) => set({ map: e.target.value })} placeholder="de_dust2" spellCheck={false} />
      </label>
      <label className="field">
        <span>Latency</span>
        <select value={filters.ping} onChange={(e) => set({ ping: Number(e.target.value) })}>
          {PINGS.map((p) => <option key={p} value={p}>{p ? `Less than ${p}` : "All"}</option>)}
        </select>
      </label>
      <div className="browser-checks">
        <label><input type="checkbox" checked={filters.people} onChange={(e) => set({ people: e.target.checked })} /> Has people playing</label>
        <label><input type="checkbox" checked={filters.notFull} onChange={(e) => set({ notFull: e.target.checked })} /> Server not full</label>
        <label><input type="checkbox" checked={filters.noPassword} onChange={(e) => set({ noPassword: e.target.checked })} /> No password</label>
      </div>
    </div>
  );
}

/** Server info, like Steam's: the details and who's on, asked again on Refresh. */
function ServerInfo({ address, row, onClose, onConnect, joining }: { address: string; row?: Row; onClose: () => void; onConnect: (r: Row, password?: string) => void; joining: boolean }) {
  type Reply = { info?: Info; ping?: number; players?: Player[] | null; error?: string };
  const [d, setD] = useState<Reply | null>(null);
  const [password, setPassword] = useState("");
  const { run, busy } = useActions();
  const load = () =>
    run(`browser:info:${address}`, async () => {
      setD(await getJsonAt<Reply>(`/api/browser/server?address=${encodeURIComponent(address)}`, null).catch((): Reply => ({ error: "Couldn't reach the app." })));
    }, "Asking the server…");
  useEffect(() => {
    load();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [address]);
  const info = d?.info ?? row?.info ?? null;
  const time = (s: number) => (s >= 3600 ? `${Math.floor(s / 3600)}:${String(Math.floor((s % 3600) / 60)).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}` : `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`);
  return (
    <Sheet label={`Server info: ${info?.name ?? address}`} onClose={onClose} className="sheet wide">
      <div className="sheet-body stack">
        <h2>{info?.name ?? address}</h2>
        {d?.error && <p className="err-text">{d.error}</p>}
        <dl className="browser-facts">
          <div><dt>Address</dt><dd>{address}</dd></div>
          {info && <div><dt>Game</dt><dd>{GAMES[info.game] ?? info.game}</dd></div>}
          {info && <div><dt>Map</dt><dd>{info.map}</dd></div>}
          {info && <div><dt>Players</dt><dd>{info.players} / {info.max_players}{info.bots ? ` (${info.bots} bots)` : ""}</dd></div>}
          {d?.ping !== undefined && <div><dt>Latency</dt><dd>{d.ping}</dd></div>}
          {info && <div><dt>Valve Anti-Cheat</dt><dd>{info.secure ? "Secure" : "Not secure"}</dd></div>}
        </dl>
        <div className="browser-table">
          <div className="browser-row head players-row" role="row">
            <span role="columnheader">Player name</span><span role="columnheader">Score</span><span role="columnheader">Time</span>
          </div>
          <div className="browser-body short">
            {!d ? <div className="browser-note"><Skeleton rows={2} /></div> : (d.players ?? []).length === 0 ? (
              <p className="browser-note">{d.error ? "" : "Nobody's on right now."}</p>
            ) : (
              d.players!.map((p, n) => (
                <div key={`${p.name}:${n}`} className="browser-row players-row" role="row">
                  <span role="gridcell">{p.name}</span><span role="gridcell">{p.score}</span><span role="gridcell">{time(p.time)}</span>
                </div>
              ))
            )}
          </div>
        </div>
        {info?.password && (
          <label className="field">
            <span>Password</span>
            <input className="search" type="password" value={password} onChange={(e) => setPassword(e.target.value)} autoComplete="off" spellCheck={false} />
          </label>
        )}
        <div className="row">
          <BusyButton className="btn primary" busy={joining} disabled={!info || (info.password && !password.trim())} label="Join game" busyLabel="Joining…" onClick={() => onConnect(row ?? { address, info, ping: d?.ping ?? null, answered: !!d?.info }, password)} />
          <BusyButton className="btn" busy={busy(`browser:info:${address}`)} label="Refresh" busyLabel="Refreshing…" onClick={load} />
          <button type="button" className="btn" onClick={onClose}>Close</button>
        </div>
      </div>
    </Sheet>
  );
}

function AddServer({ onClose, onAdd }: { onClose: () => void; onAdd: (address: string) => void }) {
  const [text, setText] = useState("");
  const ok = /^[A-Za-z0-9.-]+(:\d{1,5})?$/.test(text.trim().replace(/^steam:\/\/connect\//, ""));
  return (
    <Sheet label="Add a server" onClose={onClose}>
      <form className="sheet-body stack" onSubmit={(e) => { e.preventDefault(); if (ok) onAdd(text.trim()); }}>
        <h2>Add a server</h2>
        <label className="field">
          <span>Server address</span>
          <input className="search" autoFocus value={text} onChange={(e) => setText(e.target.value)} placeholder="play.example.com or 1.2.3.4:27015" spellCheck={false} autoComplete="off" />
          <small>The port is 27015 unless you add another.</small>
        </label>
        <div className="row">
          <button type="submit" className="btn primary" disabled={!ok}>Add to favorites</button>
          <button type="button" className="btn" onClick={onClose}>Cancel</button>
        </div>
      </form>
    </Sheet>
  );
}

function Lock({ label }: { label?: string }) {
  return (
    <svg width="9" height="11" viewBox="0 0 9 11" role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
      <path d="M2 5V3.2a2.5 2.5 0 0 1 5 0V5h1v6H1V5zm1.2 0h2.6V3.2a1.3 1.3 0 0 0-2.6 0z" fill="currentColor" />
    </svg>
  );
}

function Shield({ label }: { label?: string }) {
  return (
    <svg width="10" height="11" viewBox="0 0 10 11" role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
      <path d="M5 0l5 1.8v3.4C10 8 7.8 10 5 11 2.2 10 0 8 0 5.2V1.8z" fill="currentColor" />
    </svg>
  );
}
