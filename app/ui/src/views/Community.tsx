import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { MatchMode } from "./MatchMode";
import { getJson, postJson, targetKey, useActions, useQueuedSave } from "../api";
import { BusyButton, Info, SectionHead, Skeleton, Switch, useConfirm, useToast } from "../ui";
import { ShareList } from "./ShareList";

type FilterMode = "off" | "replace" | "block";
type Settings = { filter: FilterMode; words: string[]; chatlog: boolean; reports: boolean; stats_bots: boolean };
type Gag = { id: string; name: string; until: number; chat: boolean; voice: boolean };
type Data = { settings: Settings; gags: Gag[]; now: number; open_reports: number; plugin: boolean; dedicated: boolean };
type StatRow = {
  pos: number;
  rank: number;
  name: string;
  auth: string;
  steam: boolean;
  k: number;
  d: number;
  kd: number;
  hs: number;
  hs_pct: number;
  acc: number;
  shots: number;
  hits: number;
  dmg: number;
  tk: number;
  time: number;
  body: { head: number; chest: number; stomach: number; arms: number; legs: number };
  bomb: { planted: number; exploded: number; defused: number; defusing: number };
};
type Stats = { players: StatRow[]; bots_hidden: number; updated: number; reset_pending: boolean; csx: boolean; rank_by: number; online: boolean; has_file: boolean };
type ChatLine = { t: number; id: string; name: string; team: string; scope: string; status: string; text: string };
type Report = { t: number; from_id: string; from: string; about_id: string; about: string; map: string; reason: string; done: boolean };
type Err = { ok: false; error: string };
const failed = (r: unknown): r is Err => !!r && typeof r === "object" && "error" in r && !!(r as Err).error;
const OFFLINE: Err = { ok: false, error: "Couldn't reach the app." };

const STEAMID = /^STEAM_[0-5]:[01]:\d{1,10}$/;
export const MUTE_TIMES: [number, string][] = [[10, "10 minutes"], [30, "30 minutes"], [60, "1 hour"], [1440, "1 day"], [10080, "1 week"], [0, "Until removed"]];
const MUTE_WHAT: [string, string][] = [["chat", "Chat"], ["voice", "Voice"], ["both", "Chat and voice"]];
/** Common English swearing, for a quick start. The host can change it. */
const STARTER = ["fuck", "fucking", "shit", "bitch", "cunt", "asshole", "bastard", "motherfucker", "retard", "faggot", "nigger", "whore", "slut"];

type Tab = "stats" | "matches" | "chat" | "reports" | "mod";
const TABS: [Tab, string][] = [["stats", "Stats"], ["matches", "Matches"], ["chat", "Chat log"], ["reports", "Reports"], ["mod", "Gags & filter"]];

function clockTime(t: number) {
  const d = new Date(t * 1000);
  const today = new Date();
  const time = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  return d.toDateString() === today.toDateString() ? time : `${d.toLocaleDateString([], { day: "numeric", month: "short" })} ${time}`;
}

function played(s: number) {
  if (!s) return "-";
  if (s < 3600) return `${Math.max(1, Math.round(s / 60))} min`;
  const h = Math.floor(s / 3600);
  return h < 100 ? `${h} h ${Math.round((s % 3600) / 60)} min` : `${h} h`;
}

function left(until: number, now: number) {
  if (!until) return "until removed";
  const m = Math.max(1, Math.ceil((until - now) / 60));
  if (m < 60) return `${m} min left`;
  if (m < 1440 * 2) return `${Math.floor(m / 60)} h ${m % 60} min left`;
  return `${Math.round(m / 1440)} days left`;
}

/** Action keys for this page, per game or server. */
const ck = (what: string) => `community:${targetKey()}:${what}`;
const settingKey = (s: Partial<Settings>) => ck(`settings:${Object.keys(s).join(",")}`);
const SAVING: Record<keyof Settings, string> = {
  filter: "Saving the swear filter…",
  words: "Saving the word list…",
  chatlog: "Saving the chat log setting…",
  reports: "Saving the reports setting…",
  stats_bots: "Saving the stats setting…",
};

const teamClass = (t: string) => (t === "T" ? "t" : t === "CT" ? "ct" : "");

/** Stats and ranks, the chat log, player reports, and mutes with the swear filter, for the game or server being managed. */
export function Community({ target }: { target: string }) {
  const [d, setD] = useState<Data | null>(null);
  const [error, setError] = useState("");
  const [chatQuery, setChatQuery] = useState("");
  const [tab, setTabRaw] = useState<Tab>(() => {
    try {
      const t = localStorage.getItem("ls.communityTab") as Tab;
      return TABS.some(([id]) => id === t) ? t : "stats";
    } catch {
      return "stats";
    }
  });
  const setTab = (t: Tab) => {
    setTabRaw(t);
    try {
      localStorage.setItem("ls.communityTab", t);
    } catch {
      /* private window */
    }
  };

  const reload = useCallback(() => {
    getJson<Data | Err>("/api/community")
      .then((r) => (failed(r) ? setError(r.error) : (setD(r), setError(""))))
      .catch(() => setError("Couldn't reach the app."));
  }, []);

  useEffect(() => {
    setD(null);
    setError("");
    reload();
  }, [target, reload]);

  // a failed reload keeps the page; the error only replaces it before anything loaded
  if (!d) return error ? <p className="muted">{error}</p> : <Skeleton rows={6} />;

  const showChat = (q: string) => {
    setChatQuery(q);
    setTab("chat");
  };

  return (
    <div className="stack narrow community">
      {!d.plugin && (
        <p className="panel extras-warn">
          This game doesn't have the community plugin yet. Run Setup again from Settings to add it. Until then your settings are saved here, but the game doesn't use them.
        </p>
      )}
      <div className="chips admin-tabs" role="tablist" aria-label="Community sections">
        {TABS.map(([id, label]) => (
          <button key={id} role="tab" className="chip" aria-selected={tab === id} onClick={() => setTab(id)}>
            {label}
            {id === "reports" && d.open_reports > 0 && <i className="tab-n">{d.open_reports}</i>}
            {id === "mod" && d.gags.length > 0 && <i className="tab-n">{d.gags.length}</i>}
          </button>
        ))}
      </div>
      {tab === "stats" && <StatsTab key={`stats-${target}`} d={d} setD={setD} />}
      {tab === "matches" && <MatchMode key={`match-${target}`} target={target} />}
      {tab === "chat" && <ChatTab key={`chat-${target}`} d={d} setD={setD} initial={chatQuery} />}
      {tab === "reports" && <ReportsTab key={`rep-${target}`} d={d} setD={setD} reload={reload} showChat={showChat} />}
      {tab === "mod" && <ModTab key={`mod-${target}`} d={d} setD={setD} reload={reload} />}
    </div>
  );
}

type SetData = (d: Data) => void;

/** For pages that refresh by themselves: an error is shown once, not on every refresh. */
function usePollError() {
  const toast = useToast();
  const last = useRef<string | null>(null);
  return useCallback(
    (error: string | null) => {
      if (error && error !== last.current) toast(error, true);
      last.current = error;
    },
    [toast],
  );
}

/** Saves a change of settings; the reply is the new state. */
function useSettings(d: Data, setD: SetData) {
  const toast = useToast();
  const queue = useQueuedSave<Settings>();
  const { run } = useActions();
  return async (s: Partial<Settings>, done?: string) => {
    let ok = false;
    await run(settingKey(s), async () => {
      const r = await queue(d.settings, (cur) => ({ ...cur, ...s }), (full) => postJson<Data | Err>("/api/community/settings", full).catch(() => OFFLINE));
      if (failed(r)) return toast(r.error, true);
      setD(r);
      if (done) toast(done);
      ok = true;
    }, SAVING[Object.keys(s)[0] as keyof Settings] ?? "Saving…");
    return ok;
  };
}

/** Bans for good through the Admins page's ban (asks first). */
function useBan() {
  const ask = useConfirm();
  const toast = useToast();
  const { run } = useActions();
  return async (id: string, name: string) => {
    if (!STEAMID.test(id)) return toast("Only players with a SteamID can be banned. LAN players and bots can't.", true);
    if (!(await ask(`Ban ${name || id} for good? They're kicked if they're playing. You can unban them on the Admins page.`, { ok: "Ban", danger: true }))) return;
    await run(ck(`ban:${id}`), async () => {
      const r = await postJson<{ error?: string }>("/api/admins/ban", { id, minutes: 0 }).catch(() => OFFLINE);
      if (failed(r)) toast(r.error, true);
      else toast(`${name || id} is banned`);
    }, `Banning ${name || id}…`);
  };
}

/** A Ban button for one player, busy while that ban is sent. */
function BanButton({ id, name, label = "Ban" }: { id: string; name: string; label?: string }) {
  const ban = useBan();
  const { busy } = useActions();
  return <BusyButton className="btn danger" busy={busy(ck(`ban:${id}`))} label={label} busyLabel="Banning…" onClick={() => ban(id, name)} />;
}

// ---- mute controls (also used by the Players page) -----------------------------------

/** Mute someone by SteamID: how long, and chat, voice or both. Bots and LAN players have no SteamID, so they never get these. */
export function MuteControl({ steamid, name, onDone, compact }: { steamid: string; name: string; onDone?: (d: Data) => void; compact?: boolean }) {
  const toast = useToast();
  const [minutes, setMinutes] = useState(30);
  const [what, setWhat] = useState("chat");
  const { run, busy } = useActions();
  if (!STEAMID.test(steamid)) return null;
  const mute = () => run(ck(`mute:${steamid}`), async () => {
    const r = await postJson<Data | Err>("/api/community/gag", { id: steamid, name, minutes, chat: what !== "voice", voice: what !== "chat" }).catch(() => OFFLINE);
    if (failed(r)) return toast(r.error, true);
    toast(minutes ? `${name || steamid} is muted for ${MUTE_TIMES.find((m) => m[0] === minutes)?.[1]}` : `${name || steamid} is muted until you unmute them`);
    onDone?.(r);
  }, `Muting ${name || steamid}…`);
  return (
    <div className={`row cm-mute${compact ? " compact" : ""}`}>
      <select value={minutes} onChange={(e) => setMinutes(Number(e.target.value))} aria-label={`How long to mute ${name}`}>
        {MUTE_TIMES.map(([m, l]) => <option key={m} value={m}>{l}</option>)}
      </select>
      <select value={what} onChange={(e) => setWhat(e.target.value)} aria-label="Mute what">
        {MUTE_WHAT.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
      </select>
      <BusyButton busy={busy(ck(`mute:${steamid}`))} label="Mute" busyLabel="Muting…" onClick={mute} />
    </div>
  );
}

// ---- stats ------------------------------------------------------------------------------

type Sort = "rank" | "k" | "kd" | "hs_pct" | "acc" | "time";
const SORTS: [Sort, string][] = [["rank", "Rank"], ["k", "Kills"], ["kd", "K/D"], ["hs_pct", "Headshots"], ["acc", "Accuracy"], ["time", "Time played"]];
const PAGE = 100;

function StatsTab({ d, setD }: { d: Data; setD: SetData }) {
  const toast = useToast();
  const ask = useConfirm();
  const save = useSettings(d, setD);
  const { run, busy: isBusy } = useActions();
  const [s, setS] = useState<Stats | null>(null);
  const [busy, setBusy] = useState(false);
  const [q, setQ] = useState("");
  const [sort, setSort] = useState<Sort>("rank");
  const [open, setOpen] = useState<string | null>(null);
  const [shown, setShown] = useState(PAGE);

  const load = useCallback(async (refresh: boolean) => {
    setBusy(true);
    const r = await getJson<Stats | Err>(`/api/community/stats${refresh ? "?refresh=1" : ""}`).catch(() => OFFLINE);
    setBusy(false);
    if (failed(r)) return toast(r.error, true);
    setS(r);
  }, [toast]);

  useEffect(() => {
    load(true);
  }, [load]);

  const rows = useMemo(() => {
    if (!s) return [];
    const words = q.trim().toLowerCase().split(/\s+/).filter(Boolean);
    const list = s.players.filter((p) => words.every((w) => `${p.name} ${p.auth}`.toLowerCase().includes(w)));
    return sort === "rank" ? list : [...list].sort((a, b) => b[sort] - a[sort] || a.pos - b.pos);
  }, [s, q, sort]);

  const reset = async () => {
    if (!(await ask("Reset everyone's stats and ranks? This can't be undone. /rank and /top15 in the game start over too.", { ok: "Reset stats", danger: true }))) return;
    await run(ck("stats:reset"), async () => {
      const r = await postJson<{ ok?: boolean; message?: string; error?: string }>("/api/community/stats/reset").catch(() => OFFLINE);
      if (failed(r)) return toast(r.error, true);
      toast(r.message || "Stats reset");
      load(false);
    }, "Resetting stats…");
  };

  return (
    <section>
      <SectionHead title="Stats & ranks" note="Everyone who has played here, ranked the same way as /rank and /top15 in the game. Bots are left out.">
        <BusyButton busy={busy} label="Refresh" busyLabel="Refreshing…" onClick={() => load(true)} />
      </SectionHead>
      {!s ? (
        <Skeleton rows={5} />
      ) : (
        <>
          {s.reset_pending && <p className="panel cm-note">Stats clear when the map changes. Until then the old ones show here.</p>}
          {!s.csx && <p className="panel cm-note">Stats need the CSX module, which AMX Mod X includes. Run Setup again from Settings if it's missing.</p>}
          <input className="search full" type="search" placeholder="Search names or SteamIDs" value={q} onChange={(e) => (setQ(e.target.value), setShown(PAGE))} aria-label="Search players" />
          <div className="row cm-sort">
            <span className="muted">Sort by<Info text="Rank is the order /rank and /top15 use: kills minus deaths and team kills. HS is the share of kills that were headshots, Acc. the share of shots that hit." /></span>
            <div className="seg" role="group" aria-label="Sort by">
              {SORTS.map(([id, l]) => (
                <button key={id} type="button" aria-pressed={sort === id} onClick={() => setSort(id)}>{l}</button>
              ))}
            </div>
          </div>
          {rows.length === 0 ? (
            <p className="empty">
              {s.players.length
                ? "Nobody matches that."
                : s.has_file
                  ? "No ranked players yet. Stats count once someone has played a round."
                  : "No stats yet. They show up after a map ends, or when you press Refresh while the game is running."}
            </p>
          ) : (
            <ol className="cm-stats" aria-label="Players by rank">
              <li className="cm-stat head" aria-hidden="true">
                <span className="cm-pos">#</span>
                <span className="cm-name">Player</span>
                <span>Kills</span>
                <span>Deaths</span>
                <span>K/D</span>
                <span>HS</span>
                <span>Acc.</span>
                <span>Played</span>
              </li>
              {rows.slice(0, shown).map((p) => {
                const key = `${p.rank}-${p.auth}-${p.name}`;
                return (
                  <li key={key} className="cm-stat-item">
                    <button className="cm-stat" aria-expanded={open === key} onClick={() => setOpen(open === key ? null : key)}>
                      <span className="cm-pos">{p.pos}</span>
                      <span className="cm-name">{p.name}</span>
                      <span data-label="Kills">{p.k}</span>
                      <span data-label="Deaths">{p.d}</span>
                      <span data-label="K/D">{p.kd.toFixed(2)}</span>
                      <span data-label="HS">{p.hs_pct}%</span>
                      <span data-label="Acc.">{p.shots ? `${p.acc}%` : "-"}</span>
                      <span data-label="Played">{played(p.time)}</span>
                    </button>
                    {open === key && <StatDetail p={p} d={d} setD={setD} />}
                  </li>
                );
              })}
            </ol>
          )}
          {rows.length > shown && (
            <div className="row center">
              <button className="btn" onClick={() => setShown(shown + PAGE)}>Show more ({rows.length - shown} left)</button>
            </div>
          )}
          <p className="muted small-note">
            {s.updated ? `Updated ${clockTime(s.updated)}. ` : ""}
            {s.players.length} {s.players.length === 1 ? "player" : "players"}
            {s.bots_hidden ? `, ${s.bots_hidden} bot ${s.bots_hidden === 1 ? "entry" : "entries"} left out` : ""}.
            {s.rank_by === 0 ? " Ranks go by name (csstats_rank 0), so a name change starts a new entry." : ""}
          </p>
        </>
      )}
      <div className="settings cm-settings">
        <Toggle
          label="Count games against bots"
          note="Kills and deaths against bots count toward everyone's stats. Turn it off to rank only games between people. From the next map."
          on={d.settings.stats_bots}
          busy={isBusy(settingKey({ stats_bots: true }))}
          set={(v) => save({ stats_bots: v })}
        />
        <div className="setting">
          <span className="name">Start over<span className="note">Clears every player's stats and rank, here and in the game's /rank and /top15.</span></span>
          <div><BusyButton className="btn danger" busy={isBusy(ck("stats:reset"))} label="Reset stats" busyLabel="Resetting…" onClick={reset} /></div>
        </div>
      </div>
    </section>
  );
}

function StatDetail({ p, d, setD }: { p: StatRow; d: Data; setD: SetData }) {
  const hits = p.body.head + p.body.chest + p.body.stomach + p.body.arms + p.body.legs;
  const parts: [string, number][] = [["Head", p.body.head], ["Chest", p.body.chest], ["Stomach", p.body.stomach], ["Arms", p.body.arms], ["Legs", p.body.legs]];
  const muted = d.gags.find((g) => g.id === p.auth);
  return (
    <div className="cm-detail">
      <dl className="cm-facts">
        <div><dt>Server rank</dt><dd>{p.rank}</dd></div>
        <div><dt>Headshots</dt><dd>{p.hs}</dd></div>
        <div><dt>Damage</dt><dd>{p.dmg.toLocaleString()}</dd></div>
        <div><dt>Shots / hits</dt><dd>{p.shots.toLocaleString()} / {p.hits.toLocaleString()}</dd></div>
        <div><dt>Team kills</dt><dd>{p.tk}</dd></div>
        <div><dt>Bombs</dt><dd>{p.bomb.planted} planted, {p.bomb.exploded} went off, {p.bomb.defused} defused</dd></div>
      </dl>
      {hits > 0 && (
        <div className="cm-body" aria-label="Where their shots hit">
          {parts.map(([label, n]) => (
            <div key={label} className="cm-bar">
              <span>{label}</span>
              <span className="cm-bar-track"><i style={{ width: `${Math.round((n / hits) * 100)}%` }} /></span>
              <span className="cm-bar-n">{Math.round((n / hits) * 100)}%</span>
            </div>
          ))}
        </div>
      )}
      <p className="muted small-note">
        <span className="mono">{p.auth}</span>
        {muted ? ` · muted, ${left(muted.until, d.now)}` : ""}
      </p>
      {p.steam && (
        <div className="row">
          <MuteControl steamid={p.auth} name={p.name} onDone={setD} />
          <BanButton id={p.auth} name={p.name} />
        </div>
      )}
    </div>
  );
}

// ---- chat log ---------------------------------------------------------------------------

const STATUS: Record<string, string> = { muted: "muted, not sent", filtered: "words starred out", blocked: "not sent: swearing", report: "report" };

function ChatTab({ d, setD, initial }: { d: Data; setD: SetData; initial: string }) {
  const save = useSettings(d, setD);
  const { busy } = useActions();
  const [q, setQ] = useState(initial);
  const [lines, setLines] = useState<ChatLine[] | null>(null);
  const [more, setMore] = useState(false);
  const [limit, setLimit] = useState(200);
  const [open, setOpen] = useState<string | null>(null);

  const pollError = usePollError();
  const latest = useRef(0);
  const load = useCallback(async () => {
    const mine = ++latest.current;
    const r = await getJson<{ lines: ChatLine[]; more: boolean } | Err>(`/api/community/chat?q=${encodeURIComponent(q.trim())}&limit=${limit}`).catch(() => OFFLINE);
    if (mine !== latest.current) return;
    if (failed(r)) return pollError(r.error);
    pollError(null);
    setLines(r.lines);
    setMore(r.more);
  }, [q, limit, pollError]);

  useEffect(() => {
    const t = setTimeout(load, 250);
    // new lines come in while the page is open
    const every = setInterval(() => !document.hidden && load(), 10000);
    return () => {
      clearTimeout(t);
      clearInterval(every);
    };
  }, [load]);

  return (
    <section>
      <SectionHead title="Chat log" note="What people said, newest first, with the time and their SteamID. Select a line to mute or ban who said it." />
      <div className="settings cm-settings top">
        <Toggle label="Keep a chat log" note="Every chat line is saved on this computer. Older lines are dropped as it grows." on={d.settings.chatlog} busy={busy(settingKey({ chatlog: true }))} set={(v) => save({ chatlog: v })} />
      </div>
      <input className="search full" type="search" placeholder="Search names, SteamIDs or words" value={q} onChange={(e) => (setQ(e.target.value), setLimit(200))} aria-label="Search the chat log" />
      {!lines ? (
        <Skeleton rows={6} />
      ) : lines.length === 0 ? (
        <p className="empty">{q.trim() ? "Nothing matches that." : "Nothing said yet. Chat shows up here as people talk."}</p>
      ) : (
        <ul className="cm-chat" aria-label="Chat lines">
          {lines.map((c, i) => {
            const key = `${c.t}-${c.id}-${i}`;
            const muted = d.gags.find((g) => g.id === c.id);
            return (
              <li key={key}>
                <button className="cm-line" aria-expanded={open === key} onClick={() => setOpen(open === key ? null : key)}>
                  <time>{clockTime(c.t)}</time>
                  <span className="cm-said">
                    <b className={teamClass(c.team)}>{c.name}</b>
                    {c.scope === "team" && <span className="quiet"> (team)</span>}
                    {c.status && <i className={`cm-status ${c.status}`}>{STATUS[c.status] ?? c.status}</i>}
                    <span className="cm-text">{c.text}</span>
                  </span>
                </button>
                {open === key && (
                  <div className="cm-line-actions">
                    <p className="muted small-note">
                      <span className="mono">{c.id}</span>
                      {muted ? ` · muted, ${left(muted.until, d.now)}` : ""}
                    </p>
                    {STEAMID.test(c.id) ? (
                      <div className="row">
                        <MuteControl steamid={c.id} name={c.name} onDone={setD} />
                        <BanButton id={c.id} name={c.name} />
                        <button className="btn" onClick={() => setQ(c.id)}>Everything they said</button>
                      </div>
                    ) : (
                      <p className="muted small-note">No SteamID (a LAN player), so they can't be muted or banned from here. Admins in the game can use amx_gag.</p>
                    )}
                  </div>
                )}
              </li>
            );
          })}
        </ul>
      )}
      {more && (
        <div className="row center">
          <button className="btn" onClick={() => setLimit(Math.min(1000, limit + 200))} disabled={limit >= 1000}>Show older lines</button>
        </div>
      )}
    </section>
  );
}

// ---- reports ------------------------------------------------------------------------------

function ReportsTab({ d, setD, reload, showChat }: { d: Data; setD: SetData; reload: () => void; showChat: (q: string) => void }) {
  const toast = useToast();
  const ask = useConfirm();
  const save = useSettings(d, setD);
  const { run, busy } = useActions();
  const [list, setList] = useState<Report[] | null>(null);
  const [open, setOpen] = useState<string | null>(null);

  const pollError = usePollError();
  const load = useCallback(async () => {
    const r = await getJson<{ reports: Report[] } | Err>("/api/community/reports").catch(() => OFFLINE);
    if (failed(r)) return pollError(r.error);
    pollError(null);
    setList(r.reports);
  }, [pollError]);

  useEffect(() => {
    load();
    const every = setInterval(() => !document.hidden && load(), 15000);
    return () => clearInterval(every);
  }, [load]);

  const reportKey = (r?: Report) => ck(r ? `report:${r.t}-${r.from_id}-${r.about}` : "reports:clear");
  const act = (action: string, r?: Report) => run(reportKey(r), async () => {
    const res = await postJson<{ reports: Report[] } | Err>("/api/community/reports", { action, t: r?.t ?? 0, from_id: r?.from_id ?? "", about: r?.about ?? "" }).catch(() => OFFLINE);
    if (failed(res)) return toast(res.error, true);
    setList(res.reports);
    reload();
  }, action === "clear_done" ? "Removing handled reports…" : action === "done" ? "Marking the report handled…" : "Reopening the report…");

  const handled = list?.filter((r) => r.done).length ?? 0;

  return (
    <section>
      <SectionHead title="Reports" note="Players report someone with /report <name> <what happened>. You get a desktop notification for each one." />
      <div className="settings cm-settings top">
        <Toggle label="Let players report" note="Say /report in the game. One report a minute per player. Admins playing see them too." on={d.settings.reports} busy={busy(settingKey({ reports: true }))} set={(v) => save({ reports: v })} />
      </div>
      {!list ? (
        <Skeleton rows={4} />
      ) : list.length === 0 ? (
        <p className="empty">No reports. That's good news.</p>
      ) : (
        <ul className="people cm-reports">
          {list.map((r) => {
            const key = `${r.t}-${r.from_id}-${r.about}`;
            return (
              <li key={key} className={r.done ? "done" : ""}>
                <div className="person">
                  <span className="person-name">
                    {r.from} reported {r.about}
                    {r.done && <i className="role-badge">Handled</i>}
                  </span>
                  <span className="cm-reason">{r.reason || "No reason given."}</span>
                  <span className="person-sub">
                    {clockTime(r.t)}
                    {r.map ? ` · ${r.map}` : ""}
                    {" · "}
                    <span className="mono">{r.about_id}</span>
                  </span>
                  {open === key && STEAMID.test(r.about_id) && (
                    <div className="row cm-report-actions">
                      <MuteControl steamid={r.about_id} name={r.about} onDone={setD} />
                      <BanButton id={r.about_id} name={r.about} label={`Ban ${r.about}`} />
                    </div>
                  )}
                </div>
                <div className="row cm-report-buttons">
                  <button className="btn" onClick={() => showChat(STEAMID.test(r.about_id) ? r.about_id : r.about)}>Their chat</button>
                  {STEAMID.test(r.about_id) && <button className="btn" aria-expanded={open === key} onClick={() => setOpen(open === key ? null : key)}>Mute or ban</button>}
                  <BusyButton busy={busy(reportKey(r))} label={r.done ? "Reopen" : "Mark handled"} busyLabel="Saving…" onClick={() => act(r.done ? "open" : "done", r)} />
                </div>
              </li>
            );
          })}
        </ul>
      )}
      {handled > 0 && (
        <div className="row">
          <BusyButton
            busy={busy(reportKey())}
            label="Remove handled reports"
            busyLabel="Removing…"
            onClick={async () => (await ask(`Remove the ${handled} handled ${handled === 1 ? "report" : "reports"}?`, { ok: "Remove" })) && act("clear_done")}
          />
        </div>
      )}
    </section>
  );
}

// ---- gags and the swear filter ---------------------------------------------------------------

function ModTab({ d, setD, reload }: { d: Data; setD: SetData; reload: () => void }) {
  const toast = useToast();
  const ask = useConfirm();
  const save = useSettings(d, setD);
  const { run, busy } = useActions();
  const [id, setId] = useState("");
  const [draft, setDraft] = useState<string[] | null>(null);
  const [word, setWord] = useState("");
  const words = draft ?? d.settings.words;
  const dirty = draft !== null && draft.join("\n") !== d.settings.words.join("\n");

  const unmute = async (g: Gag) => {
    if (!(await ask(`Unmute ${g.name || g.id}?`, { ok: "Unmute" }))) return;
    await run(ck(`unmute:${g.id}`), async () => {
      const r = await postJson<Data | Err>("/api/community/ungag", { id: g.id }).catch(() => OFFLINE);
      if (failed(r)) return toast(r.error, true);
      setD(r);
      toast(`${g.name || g.id} can talk again`);
    }, `Unmuting ${g.name || g.id}…`);
  };
  const addWord = () => {
    const parts = word.toLowerCase().split(/[\s,]+/).filter(Boolean);
    if (!parts.length) return;
    const bad = parts.find((w) => w.length < 2 || w.length > 31);
    if (bad) return toast(`"${bad}" is too short or too long. Words are 2 to 31 letters.`, true);
    setDraft([...new Set([...words, ...parts])].sort());
    setWord("");
  };
  const saveWords = async () => {
    if (await save({ words }, "Word list saved")) setDraft(null);
  };

  return (
    <>
      <section>
        <SectionHead title="Muted players" note="A muted player can't use chat, voice or both until it runs out, even if they leave and come back. Their commands like /rtv still work. Bots are never muted." />
        <ul className="people">
          {d.gags.map((g) => (
            <li key={g.id}>
              <div className="person">
                <span className="person-name">{g.name || <span className="mono">{g.id}</span>}</span>
                <span className="person-sub">
                  {g.chat && g.voice ? "Chat and voice" : g.chat ? "Chat" : "Voice"} · {left(g.until, d.now)}
                  {g.name ? <> · <span className="mono">{g.id}</span></> : null}
                </span>
              </div>
              <BusyButton busy={busy(ck(`unmute:${g.id}`))} label="Unmute" busyLabel="Unmuting…" onClick={() => unmute(g)} />
            </li>
          ))}
          {!d.gags.length && <li className="empty">Nobody is muted.</li>}
        </ul>
        <div className="panel stack-tight">
          <label className="field">
            <span>Mute by SteamID<Info text="Mutes them for everyone on the server. Players can still mute someone just for themselves from the scoreboard." /></span>
            <input className="search" placeholder="SteamID, like STEAM_0:1:12345" value={id} onChange={(e) => setId(e.target.value)} spellCheck={false} />
            <small>Or select someone on the Players page, in the chat log or in the stats.</small>
          </label>
          {STEAMID.test(id.trim()) ? (
            <MuteControl steamid={id.trim()} name="" onDone={(r) => (setD(r), setId(""))} />
          ) : (
            id.trim() && <p className="err-text">That isn't a SteamID yet. It looks like STEAM_0:1:12345.</p>
          )}
        </div>
        <p className="muted small-note">
          Admins and moderators in the game: <span className="mono">amx_gag &lt;name&gt; &lt;minutes&gt; [chat|voice|both]</span> and <span className="mono">amx_ungag &lt;name&gt;</span> in the console. 0 minutes means until removed.
        </p>
      </section>

      <section>
        <SectionHead title="Swear filter" note="Words that get starred out, or stop the message. A word also catches longer words that start with it, so fuck catches fucking. Commands like /rtv are never touched." />
        <ShareList list="filter" onShared={() => { setDraft(null); reload(); }} />
        <div className="settings">
          <div className="setting">
            <span className="name">When someone swears<Info text="Star it out still sends the message, with the word hidden behind stars. Don't send it drops the whole message and tells the sender why." /><span className="note">{words.length ? `${words.length} ${words.length === 1 ? "word" : "words"} on the list.` : "Add words below first."}</span></span>
            <div className="seg" role="group" aria-label="When someone swears">
              {([["off", "Nothing"], ["replace", "Star it out"], ["block", "Don't send it"]] as [FilterMode, string][]).map(([m, l]) => (
                <button key={m} type="button" aria-pressed={d.settings.filter === m} disabled={busy(settingKey({ filter: m }))} onClick={() => save({ filter: m })}>{l}</button>
              ))}
            </div>
          </div>
        </div>
        <div className="panel stack-tight">
          <div className="row">
            <input
              className="search grow"
              placeholder="Add a word"
              value={word}
              onChange={(e) => setWord(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addWord()}
              spellCheck={false}
              aria-label="Word to add"
            />
            <button className="btn" disabled={!word.trim()} onClick={addWord}>Add</button>
          </div>
          {words.length ? (
            <div className="chips cm-words" aria-label="Filtered words">
              {words.map((w) => (
                <button key={w} className="chip" aria-label={`Remove ${w}`} onClick={() => setDraft(words.filter((x) => x !== w))}>
                  {w}
                  <Glyph d="M6 6l12 12M18 6L6 18" />
                </button>
              ))}
            </div>
          ) : (
            <div className="row">
              <span className="muted">The list is empty.</span>
              <button className="btn" onClick={() => setDraft(STARTER)}>Use a starter list</button>
            </div>
          )}
          <div className="row">
            <BusyButton className="btn primary" busy={busy(settingKey({ words }))} disabled={!dirty} label="Save word list" busyLabel="Saving…" onClick={saveWords} />
            {dirty && <button className="btn" onClick={() => setDraft(null)}>Undo changes</button>}
            {words.length > 0 && <button className="btn danger push" onClick={() => setDraft([])}>Remove all</button>}
          </div>
          <p className="muted small-note">Tap a word to take it off the list. Changes count as soon as you save, no map change needed.</p>
        </div>
      </section>
    </>
  );
}

// ---- small pieces ---------------------------------------------------------------------------

function Glyph({ d }: { d: string }) {
  return (
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2.4} strokeLinecap="round" aria-hidden="true">
      <path d={d} />
    </svg>
  );
}

function Toggle({ label, note, on, set, busy }: { label: string; note: ReactNode; on: boolean; set: (v: boolean) => void; busy?: boolean }) {
  return (
    <div className="setting">
      <span className="name">{label}<span className="note">{note}</span></span>
      <Switch on={on} onChange={set} label={label} disabled={busy} />
    </div>
  );
}
