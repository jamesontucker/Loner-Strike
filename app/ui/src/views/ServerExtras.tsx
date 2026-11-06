import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { getJson, ON_THIS_COMPUTER, postJson, targetKey, useActions, useQueuedSave } from "../api";
import type { MapInfo } from "../types";
import { BusyButton, Info, SectionHead, Skeleton, Switch, useConfirm, useToast } from "../ui";
import { ShareList } from "./ShareList";

type Color = "default" | "green" | "red" | "blue" | "grey";
type Ad = { text: string; color: Color };
/** Modes A to Z, for the pickers. */
const byName = <T extends { name: string }>(l: T[]) => [...l].sort((a, b) => a.name.localeCompare(b.name));

type Settings = {
  rotation_random: boolean;
  rtv_cycle: boolean;
  ads: { on: boolean; minutes: number; random: boolean; prefix: string; messages: Ad[] };
  afk: { on: boolean; spec: number; kick: number };
  ping: { on: boolean; max: number; checks: number };
  fastdl: string;
  joins: { on: boolean; country: boolean; skip_bots: boolean };
  votes: { kick: boolean; ban: boolean; percent: number; min_players: number; ban_minutes: number; cooldown: number; kick_bots: boolean };
  empty: { on: boolean; minutes: number; map: string; mode: string };
};
type Data = {
  settings: Settings;
  rotation: { file: string; main: string[]; modes: Record<string, string[]> };
  modes: { id: string; name: string }[];
  current_mode: string;
  plugin: boolean;
  dedicated: boolean;
  geoip: boolean;
  schedule: { on: boolean; time: string } | null;
};
type Reply = Data | { ok: false; error: string };
const failed = (r: Reply | { error?: string }): r is { ok: false; error: string } => "error" in r && !!r.error;

const COLORS: { id: Color; name: string; css: string }[] = [
  { id: "default", name: "Normal", css: "var(--chat-default)" },
  { id: "green", name: "Green", css: "var(--chat-green)" },
  { id: "red", name: "Red", css: "var(--chat-red)" },
  { id: "blue", name: "Blue", css: "var(--chat-blue)" },
  { id: "grey", name: "Grey", css: "var(--chat-grey)" },
];
const PLACEHOLDERS: [string, string][] = [
  ["{server}", "the server's name"],
  ["{map}", "the map"],
  ["{nextmap}", "the next map"],
  ["{players}", "players, like 9/16"],
  ["{timeleft}", "time left on the map"],
];
const SAMPLE: Record<string, string> = { "{server}": "Friday night frags", "{map}": "de_dust2", "{nextmap}": "cs_office", "{players}": "9/16", "{timeleft}": "12:30" };
const EXAMPLES = ["Join our Discord: discord.gg/yourserver", "Rules: no cheating, no spawn camping, be nice.", "Say rtv to vote for a new map. Next up: {nextmap}", "Like the server? Say /donate to help keep it running."];
const MINUTES = [1, 2, 3, 5, 10, 15];
const AFK_SPEC = [60, 90, 120, 180, 300];
const AFK_KICK = [0, 60, 120, 180, 300];
const PING_MAX = [150, 200, 250, 300, 400];
const PING_CHECKS = [3, 5, 10];
const VOTE_PERCENT = [51, 60, 66, 75];
const VOTE_MIN = [2, 3, 4, 6];
const BAN_MINUTES = [15, 30, 60, 180, 1440];
const VOTE_COOLDOWN = [0, 2, 3, 5, 10];
const EMPTY_MINUTES = [2, 5, 10, 30];
const MAP_NAME = /^[A-Za-z0-9_.-]{1,31}$/;
const URL_OK = /^https?:\/\/[^\s/."'`;\\<>{}|^][^\s"'`;\\<>{}|^]*$/;

const mins = (m: number) => (m >= 60 && m % 60 === 0 ? `${m / 60} ${m === 60 ? "hour" : "hours"}` : `${m} min`);
const secs = (s: number) => (s < 60 ? `${s} sec` : s % 60 ? `${Math.floor(s / 60)} min ${s % 60} sec` : `${s / 60} min`);

/** Small line icons (no arrow characters in the copy). */
function Glyph({ d }: { d: string }) {
  return (
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d={d} />
    </svg>
  );
}
/** Action keys for this page, per game or server. */
const xk = (what: string) => `extras:${targetKey()}:${what}`;

const UP = "M6 15l6-6 6 6";
const DOWN = "M6 9l6 6 6-6";
const CROSS = "M6 6l12 12M18 6L6 18";

/** Map rotation, chat messages, away and ping kicks, and fast downloads for the game or server being managed. */
type ExtrasTab = "rotation" | "chat" | "kickers" | "people" | "schedule" | "downloads";
const EXTRAS_TABS: [ExtrasTab, string][] = [
  ["rotation", "Map rotation"],
  ["chat", "Chat messages"],
  ["kickers", "Kickers"],
  ["people", "Joins & votes"],
  ["schedule", "Schedule"],
  ["downloads", "Fast downloads"],
];

export function ServerExtras({ target }: { target: string }) {
  const toast = useToast();
  const [d, setD] = useState<Data | null>(null);
  const [error, setError] = useState("");
  const [maps, setMaps] = useState<MapInfo[]>([]);
  // bumped when the chat messages start coming from a shared list: load everything again
  const [version, setVersion] = useState(0);
  const [tab, setTabRaw] = useState<ExtrasTab>(() => {
    try {
      const t = localStorage.getItem("ls.extrasTab") as ExtrasTab;
      return EXTRAS_TABS.some(([id]) => id === t) ? t : "rotation";
    } catch {
      return "rotation";
    }
  });
  const setTab = (t: ExtrasTab) => {
    setTabRaw(t);
    try {
      localStorage.setItem("ls.extrasTab", t);
    } catch {
      /* private window */
    }
  };

  useEffect(() => {
    setD(null);
    setError("");
    getJson<Reply>("/api/extras")
      .then((r) => (failed(r) ? setError(r.error) : setD(r)))
      .catch(() => setError("Couldn't reach the app."));
    getJson<MapInfo[]>("/api/maps")
      .then((m) => setMaps(Array.isArray(m) ? m : []))
      .catch(() => setMaps([]));
  }, [target, version]);

  const queue = useQueuedSave<Settings>();
  /** Sends one change; the reply is the new state. */
  const send = async (path: string, body: unknown, done?: string) => {
    const r = await postJson<Reply>(path, body).catch(() => ({ ok: false as const, error: "Couldn't reach the app." }));
    if (failed(r)) {
      toast(r.error, true);
      return false;
    }
    setD(r);
    if (done) toast(done);
    return true;
  };
  const saveSettings = (s: Partial<Settings>, done?: string) => (d ? queue(d.settings, (cur) => ({ ...cur, ...s }), (full) => send("/api/extras/settings", full, done)) : Promise.resolve(false));

  if (error) return <p className="muted">{error}</p>;
  if (!d) return <Skeleton rows={6} />;

  return (
    <div className="stack narrow extras">
      {!d.plugin && (
        <p className="panel extras-warn">
          This game doesn't have the extras plugin yet. Run Setup again from Settings to add it. Until then your settings are saved here, but the game doesn't use them.
        </p>
      )}
      <div className="chips admin-tabs" role="tablist" aria-label="Rotation and chat sections">
        {EXTRAS_TABS.map(([id, label]) => (
          <button key={id} role="tab" className="chip" aria-selected={tab === id} onClick={() => setTab(id)}>{label}</button>
        ))}
      </div>
      {tab === "rotation" && <Rotation key={`rot-${target}`} d={d} maps={maps} send={send} saveSettings={saveSettings} />}
      {tab === "chat" && <Messages key={`ads-${target}-${version}`} d={d} saveSettings={saveSettings} onShared={() => setVersion((v) => v + 1)} />}
      {tab === "kickers" && <Kickers d={d} saveSettings={saveSettings} />}
      {tab === "people" && <JoinsVotes d={d} saveSettings={saveSettings} />}
      {tab === "schedule" && <Schedule key={`sched-${target}`} d={d} maps={maps} send={send} saveSettings={saveSettings} />}
      {tab === "downloads" && <FastDownloads key={`dl-${target}`} d={d} saveSettings={saveSettings} />}
    </div>
  );
}

type SaveSettings = (s: Partial<Settings>, done?: string) => Promise<boolean>;

// ---- map rotation -------------------------------------------------------------------

function Rotation({ d, maps, send, saveSettings }: { d: Data; maps: MapInfo[]; send: (p: string, b: unknown, done?: string) => Promise<boolean>; saveSettings: SaveSettings }) {
  const ask = useConfirm();
  const { run, busy } = useActions();
  const [scope, setScope] = useState("");
  const [draft, setDraft] = useState<string[] | null>(null);
  const [query, setQuery] = useState("");
  const dragFrom = useRef<number | null>(null);
  const [dragOver, setDragOver] = useState<number | null>(null);

  const own = scope ? d.rotation.modes[scope] : d.rotation.main;
  const saved = own ?? [];
  const list = draft ?? saved;
  const dirty = draft !== null && draft.join("\n") !== saved.join("\n");
  const modeName = (id: string) => d.modes.find((m) => m.id === id)?.name ?? id;
  const installed = useMemo(() => new Set(maps.map((m) => m.name.toLowerCase())), [maps]);
  const inList = new Set(list.map((m) => m.toLowerCase()));
  const q = query.trim().toLowerCase();
  const matches = q
    ? maps
        .filter((m) => !inList.has(m.name.toLowerCase()) && MAP_NAME.test(m.name) && (m.name.toLowerCase().includes(q) || (m.title ?? "").toLowerCase().includes(q)))
        .slice(0, 12)
    : [];

  const edit = (next: string[]) => setDraft(next);
  const move = (from: number, to: number) => {
    if (to < 0 || to >= list.length || from === to) return;
    const next = [...list];
    const [m] = next.splice(from, 1);
    next.splice(to, 0, m);
    edit(next);
  };
  const add = (name: string) => {
    if (inList.has(name.toLowerCase())) return;
    edit([...list, name]);
  };
  const shuffle = () => {
    const next = [...list];
    for (let i = next.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [next[i], next[j]] = [next[j], next[i]];
    }
    edit(next);
  };
  const pickScope = async (s: string) => {
    if (dirty && !(await ask("Leave without saving your changes to this rotation?", { ok: "Leave" }))) return;
    setScope(s);
    setDraft(null);
    setQuery("");
  };
  const rotKey = (what: string) => xk(`rotation:${scope || "main"}:${what}`);
  const rotBusy = busy(rotKey("save")) || busy(rotKey("drop"));
  const save = () => run(rotKey("save"), async () => {
    if (await send("/api/extras/rotation", { mode: scope, maps: list }, "Rotation saved. It counts from the next map change.")) setDraft(null);
  }, `Saving the ${scope ? `${modeName(scope)} ` : ""}rotation…`);
  const dropOwn = async () => {
    if (!(await ask(`Remove ${modeName(scope)}'s own list? It goes back to the main rotation.`, { ok: "Remove", danger: true }))) return;
    await run(rotKey("drop"), async () => {
      if (await send("/api/extras/rotation", { mode: scope, maps: [] }, `${modeName(scope)} uses the main rotation again.`)) setDraft(null);
    }, `Removing ${modeName(scope)}'s own rotation…`);
  };
  const missing = list.filter((m) => maps.length > 0 && !installed.has(m.toLowerCase()));

  return (
    <section>
      <SectionHead
        title="Map rotation"
        note={
          <>
            The maps that come next, in order, when a map's time runs out. A map vote still wins, and map presets still pick the mode (a surf_ map plays Surf). Players can
            say <b>nextmap</b> to see what's coming.
          </>
        }
      />
      <div className="panel stack-tight">
        <label className="field rot-scope">
          <span>Which rotation<Info text="The main rotation is used by every mode. A mode can also have its own list, which takes over while that mode is on, so surf maps only come up in Surf." /></span>
          <select value={scope} onChange={(e) => pickScope(e.target.value)}>
            <option value="">Main rotation (every mode, {d.rotation.file})</option>
            {byName(d.modes).map((m) => (
              <option key={m.id} value={m.id}>
                {m.name}
                {d.rotation.modes[m.id] ? " (own list)" : ""}
                {m.id === d.current_mode ? " (playing now)" : ""}
              </option>
            ))}
          </select>
          <small>
            {scope
              ? own
                ? `While ${modeName(scope)} is on, the next map comes from this list instead of the main rotation.`
                : `${modeName(scope)} uses the main rotation. Give it its own list for maps made for it, like ${scope === "surf" ? "surf_ski_2" : scope === "zombie" ? "zm_ maps" : "its special maps"}.`
              : "Used by every mode without its own list. This is the game's own map cycle, so map votes from the admin menu use it too."}
          </small>
        </label>

        {scope && !own && draft === null ? (
          <div className="row">
            <button className="btn" onClick={() => setDraft([])}>Give {modeName(scope)} its own list</button>
          </div>
        ) : (
          <>
            {list.length === 0 ? (
              <p className="muted rot-empty">No maps yet. Add some below.</p>
            ) : (
              <ol className="rot-list" aria-label="Maps in the rotation">
                {list.map((m, i) => (
                  <li
                    key={`${i}-${m}`}
                    className={`rot-item${dragOver === i ? " drop" : ""}`}
                    draggable
                    onDragStart={(e) => {
                      dragFrom.current = i;
                      e.dataTransfer.effectAllowed = "move";
                    }}
                    onDragOver={(e) => {
                      e.preventDefault();
                      setDragOver(i);
                    }}
                    onDragLeave={() => setDragOver((o) => (o === i ? null : o))}
                    onDrop={(e) => {
                      e.preventDefault();
                      if (dragFrom.current !== null) move(dragFrom.current, i);
                      dragFrom.current = null;
                      setDragOver(null);
                    }}
                    onDragEnd={() => {
                      dragFrom.current = null;
                      setDragOver(null);
                    }}
                  >
                    <span className="rot-n">{i + 1}</span>
                    <span className="rot-name">
                      {m}
                      {maps.length > 0 && !installed.has(m.toLowerCase()) && <span className="err-text"> not installed</span>}
                    </span>
                    <span className="rot-actions">
                      <button className="btn icon" aria-label={`Move ${m} up`} disabled={i === 0} onClick={() => move(i, i - 1)}><Glyph d={UP} /></button>
                      <button className="btn icon" aria-label={`Move ${m} down`} disabled={i === list.length - 1} onClick={() => move(i, i + 1)}><Glyph d={DOWN} /></button>
                      <button className="btn icon" aria-label={`Remove ${m}`} onClick={() => edit(list.filter((_, j) => j !== i))}><Glyph d={CROSS} /></button>
                    </span>
                  </li>
                ))}
              </ol>
            )}

            <label className="field rot-add">
              <span>Add a map</span>
              <input className="search" placeholder="Type part of a map's name" value={query} onChange={(e) => setQuery(e.target.value)} spellCheck={false} />
            </label>
            {q && (
              <div className="chips rot-matches">
                {matches.length ? (
                  matches.map((m) => (
                    <button key={m.name} className="chip" onClick={() => add(m.name)} title={m.title || undefined}>
                      {m.name}
                    </button>
                  ))
                ) : (
                  <span className="muted">No installed map matches, or it's already in the list.</span>
                )}
              </div>
            )}
            {missing.length > 0 && <p className="err-text">Remove the maps that aren't installed before saving.</p>}

            <div className="row">
              <BusyButton className="btn primary" busy={busy(rotKey("save"))} disabled={rotBusy || !dirty || missing.length > 0 || (!scope && list.length === 0)} label="Save rotation" busyLabel="Saving…" onClick={save} />
              <button className="btn" disabled={list.length < 2} onClick={shuffle}>Shuffle</button>
              {dirty && <button className="btn" onClick={() => setDraft(null)}>Undo changes</button>}
              {scope && own && <BusyButton className="btn danger push" busy={busy(rotKey("drop"))} disabled={rotBusy} label="Use the main rotation" busyLabel="Removing…" onClick={dropOwn} />}
            </div>
            <p className="muted small-note">Drag maps to reorder them, or use the buttons. {list.length} {list.length === 1 ? "map" : "maps"}.</p>
          </>
        )}
      </div>
      <div className="settings">
        <Toggle
          label="Random order"
          note="Each map picks a random one from the list instead of the next, skipping the ones played just before."
          on={d.settings.rotation_random}
          set={(v) => saveSettings({ rotation_random: v })}
        />
        <Toggle
          label="Map votes use the rotation"
          note="Rock the vote and nominations only offer maps from the rotation, instead of every map bots can play."
          on={d.settings.rtv_cycle}
          set={(v) => saveSettings({ rtv_cycle: v })}
        />
      </div>
    </section>
  );
}

// ---- chat messages --------------------------------------------------------------------

function Messages({ d, saveSettings, onShared }: { d: Data; saveSettings: SaveSettings; onShared: () => void }) {
  const ads = d.settings.ads;
  const { run, busy } = useActions();
  const pick = (what: string, p: Partial<typeof ads>, label: string) => run(xk(`ads:${what}`), () => saveSettings({ ads: { ...ads, ...p } }), label);
  const [draft, setDraft] = useState<{ prefix: string; messages: Ad[] } | null>(null);
  const [focus, setFocus] = useState(0);
  const cur = draft ?? { prefix: ads.prefix, messages: ads.messages };
  const dirty = draft !== null && JSON.stringify(draft) !== JSON.stringify({ prefix: ads.prefix, messages: ads.messages });
  const set = (p: Partial<typeof cur>) => setDraft({ ...cur, ...p });
  const setMsg = (i: number, p: Partial<Ad>) => set({ messages: cur.messages.map((m, j) => (j === i ? { ...m, ...p } : m)) });
  const move = (from: number, to: number) => {
    if (to < 0 || to >= cur.messages.length) return;
    const next = [...cur.messages];
    const [m] = next.splice(from, 1);
    next.splice(to, 0, m);
    set({ messages: next });
    setFocus(to);
  };
  const addMsg = (text = "") => {
    set({ messages: [...cur.messages, { text, color: "default" }] });
    setFocus(cur.messages.length);
  };
  const insert = (ph: string) => {
    if (!cur.messages.length) return addMsg(ph);
    const i = Math.min(focus, cur.messages.length - 1);
    setMsg(i, { text: `${cur.messages[i].text}${cur.messages[i].text.endsWith(" ") || !cur.messages[i].text ? "" : " "}${ph}`.slice(0, 150) });
  };
  const save = () => run(xk("ads:messages"), async () => {
    const messages = cur.messages.map((m) => ({ ...m, text: m.text.trim() })).filter((m) => m.text);
    if (await saveSettings({ ads: { ...ads, prefix: cur.prefix, messages } }, "Messages saved")) setDraft(null);
  }, "Saving chat messages…");
  const fill = (t: string) => Object.entries(SAMPLE).reduce((s, [k, v]) => s.split(k).join(v), t);
  const shown = cur.messages.filter((m) => m.text.trim());

  return (
    <section>
      <SectionHead title="Chat messages" note="Messages that show up in chat every few minutes, like a real server's: your Discord, the rules, a donate reminder. Only while someone is playing." />
      <ShareList list="messages" onShared={onShared} />
      <div className="settings">
        <Toggle label="Show messages" note={ads.messages.length ? `${ads.messages.length} ${ads.messages.length === 1 ? "message" : "messages"}.` : "Write some below first."} on={ads.on} set={(v) => saveSettings({ ads: { ...ads, on: v } })} />
        <div className="setting">
          <span className="name">How often<span className="note">One message each time.</span></span>
          <div className="seg" role="group" aria-label="How often">
            {MINUTES.map((m) => (
              <button key={m} type="button" aria-pressed={ads.minutes === m} disabled={busy(xk("ads:minutes"))} onClick={() => pick("minutes", { minutes: m }, "Saving how often messages show…")}>{m} min</button>
            ))}
          </div>
        </div>
        <div className="setting">
          <span className="name">Order</span>
          <div className="seg" role="group" aria-label="Order">
            <button type="button" aria-pressed={!ads.random} disabled={busy(xk("ads:random"))} onClick={() => pick("random", { random: false }, "Saving the message order…")}>In order</button>
            <button type="button" aria-pressed={ads.random} disabled={busy(xk("ads:random"))} onClick={() => pick("random", { random: true }, "Saving the message order…")}>Random</button>
          </div>
        </div>
      </div>

      <div className="panel stack-tight ads-edit">
        <label className="field">
          <span>Prefix</span>
          <input className="search" maxLength={24} placeholder="[Server]" value={cur.prefix} onChange={(e) => set({ prefix: e.target.value })} />
          <small>Shown in green before every message. Leave it empty for none.</small>
        </label>
        {cur.messages.length === 0 && (
          <div className="stack-tight">
            <p className="muted">No messages yet. Start from an example, or write your own.</p>
            <div className="chips">
              {EXAMPLES.map((e) => (
                <button key={e} className="chip ad-example" onClick={() => addMsg(e)}>{e}</button>
              ))}
            </div>
          </div>
        )}
        <ol className="ad-list" aria-label="Messages">
          {cur.messages.map((m, i) => (
            <li key={i} className="ad-item">
              <input
                className="search grow"
                maxLength={150}
                value={m.text}
                placeholder="Your message"
                aria-label={`Message ${i + 1}`}
                onFocus={() => setFocus(i)}
                onChange={(e) => setMsg(i, { text: e.target.value })}
              />
              <div className="ad-tools">
                <select value={m.color} aria-label={`Color of message ${i + 1}`} onChange={(e) => setMsg(i, { color: e.target.value as Color })}>
                  {COLORS.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
                </select>
                <button className="btn icon" aria-label={`Move message ${i + 1} up`} disabled={i === 0} onClick={() => move(i, i - 1)}><Glyph d={UP} /></button>
                <button className="btn icon" aria-label={`Move message ${i + 1} down`} disabled={i === cur.messages.length - 1} onClick={() => move(i, i + 1)}><Glyph d={DOWN} /></button>
                <button className="btn icon" aria-label={`Remove message ${i + 1}`} onClick={() => set({ messages: cur.messages.filter((_, j) => j !== i) })}><Glyph d={CROSS} /></button>
              </div>
            </li>
          ))}
        </ol>
        <div className="row">
          <button className="btn" disabled={cur.messages.length >= 40} onClick={() => addMsg()}>Add a message</button>
        </div>
        <div className="field">
          <span>Fill-ins</span>
          <div className="chips">
            {PLACEHOLDERS.map(([k, label]) => (
              <button key={k} className="chip" title={`Fills in ${label}`} onClick={() => insert(k)}>{k}</button>
            ))}
          </div>
          <small>Tap one to add it to the message you're editing. The game fills them in when the message shows.</small>
        </div>
        {shown.length > 0 && (
          <div className="field">
            <span>Preview</span>
            <div className="chat-preview" aria-label="How the messages look in chat">
              {shown.map((m, i) => (
                <p key={i}>
                  {cur.prefix.trim() && <span style={{ color: "var(--chat-green)" }}>{cur.prefix.trim()} </span>}
                  <span style={{ color: COLORS.find((c) => c.id === m.color)?.css }}>{fill(m.text)}</span>
                </p>
              ))}
            </div>
            <small>With example details. Red and blue show as the team colors.</small>
          </div>
        )}
        <div className="row">
          <BusyButton className="btn primary" busy={busy(xk("ads:messages"))} disabled={!dirty} label="Save messages" busyLabel="Saving…" onClick={save} />
          {dirty && <button className="btn" onClick={() => setDraft(null)}>Undo changes</button>}
        </div>
      </div>
    </section>
  );
}

// ---- away and ping kicks ------------------------------------------------------------------

function Kickers({ d, saveSettings }: { d: Data; saveSettings: SaveSettings }) {
  const { afk, ping } = d.settings;
  const host = d.dedicated ? "" : " You, the host, never are.";
  return (
    <section>
      <SectionHead title="Kickers" note={`Keep slots free for people who are actually playing. Bots are never moved or kicked, and neither is anyone with immunity (owners).${host}`} />
      <div className="settings">
        <Toggle label="Away players" note="Someone who doesn't press a key or move the mouse for a while goes to spectator, then can be kicked." on={afk.on} set={(v) => saveSettings({ afk: { ...afk, on: v } })} />
        {afk.on && (
          <>
            <Choice label="Move to spectator after" note="Of doing nothing while alive. They get a warning first." values={AFK_SPEC} current={afk.spec} show={secs} pick={(v) => saveSettings({ afk: { ...afk, spec: v } })} />
            <Choice
              label="Then kick after"
              note="Unless they join a team again. Someone who never picks a team waits both times."
              values={AFK_KICK}
              current={afk.kick}
              show={(v) => (v ? secs(v) : "Never")}
              pick={(v) => saveSettings({ afk: { ...afk, kick: v } })}
            />
          </>
        )}
        <Toggle label="High ping" note="Someone whose connection stays slow gets warnings, then a kick. Not while a map is loading, or right after they join." on={ping.on} set={(v) => saveSettings({ ping: { ...ping, on: v } })} />
        {ping.on && (
          <>
            <Choice label="Ping limit" note="Their average ping over the last 30 seconds." values={PING_MAX} current={ping.max} show={(v) => `${v} ms`} pick={(v) => saveSettings({ ping: { ...ping, max: v } })} />
            <Choice
              label="Kick after"
              note="Checks in a row over the limit, 10 seconds apart. Admins and moderators are never kicked for ping."
              values={PING_CHECKS}
              current={ping.checks}
              show={(v) => `${v} checks`}
              pick={(v) => saveSettings({ ping: { ...ping, checks: v } })}
            />
          </>
        )}
      </div>
    </section>
  );
}

// ---- joins and votes ------------------------------------------------------------------------

function JoinsVotes({ d, saveSettings }: { d: Data; saveSettings: SaveSettings }) {
  const { joins, votes } = d.settings;
  const host = d.dedicated ? "" : ", you (the host)";
  return (
    <>
      <section>
        <SectionHead title="Join messages" note="A line in chat when someone joins or leaves, like Alex (Germany) joined." />
        <div className="settings">
          <Toggle label="Join and leave messages" note="Not on map changes, only when someone really comes or goes." on={joins.on} set={(v) => saveSettings({ joins: { ...joins, on: v } })} />
          {joins.on && (
            <>
              {d.geoip ? (
                <Toggle
                  label="Show the country"
                  note="Players on your own network show none, and people joining through the app's internet link may not either."
                  on={joins.country}
                  set={(v) => saveSettings({ joins: { ...joins, country: v } })}
                />
              ) : (
                <div className="setting">
                  <span className="name">
                    Country<span className="note">This game doesn't have AMX Mod X's country database, so messages show without a country. Running Setup again adds it.</span>
                  </span>
                </div>
              )}
              <Toggle label="Leave out bots" note="Bots come and go a lot. Leave this on to keep chat quiet." on={joins.skip_bots} set={(v) => saveSettings({ joins: { ...joins, skip_bots: v } })} />
            </>
          )}
        </div>
      </section>
      <section>
        <SectionHead
          title="Player votes"
          note={`Players say /votekick or /voteban, pick someone from a menu, and everyone else votes for 20 seconds. Owners with immunity${host} and the TV relay can't be voted on.`}
        />
        <div className="settings">
          <Toggle label="Vote kick" note="Someone kicked this way can join again straight away." on={votes.kick} set={(v) => saveSettings({ votes: { ...votes, kick: v } })} />
          <Toggle label="Vote ban" note="Keeps them out for a while. Never for good: that's for you to decide." on={votes.ban} set={(v) => saveSettings({ votes: { ...votes, ban: v } })} />
          {(votes.kick || votes.ban) && (
            <>
              <Choice
                label="Yes votes needed"
                note="Out of everyone who can vote, not counting the player being voted on."
                values={VOTE_PERCENT}
                current={votes.percent}
                show={(v) => `${v}%`}
                pick={(v) => saveSettings({ votes: { ...votes, percent: v } })}
              />
              <Choice
                label="Players needed"
                note="Real players on the server before anyone can start a vote."
                values={VOTE_MIN}
                current={votes.min_players}
                show={(v) => `${v}`}
                pick={(v) => saveSettings({ votes: { ...votes, min_players: v } })}
              />
              <Choice
                label="Time between votes"
                note="After one vote ends, before the next can start."
                values={VOTE_COOLDOWN}
                current={votes.cooldown}
                show={(v) => (v ? `${v} min` : "None")}
                pick={(v) => saveSettings({ votes: { ...votes, cooldown: v } })}
              />
            </>
          )}
          {votes.ban && (
            <Choice label="Ban length" note="By Steam account, or by address for players without one." values={BAN_MINUTES} current={votes.ban_minutes} show={mins} pick={(v) => saveSettings({ votes: { ...votes, ban_minutes: v } })} />
          )}
          {votes.kick && (
            <Toggle
              label="Bots can be vote-kicked"
              note="Handy when a bot is spoiling the game. The bot count goes down by one. Bots can never be banned."
              on={votes.kick_bots}
              set={(v) => saveSettings({ votes: { ...votes, kick_bots: v } })}
            />
          )}
        </div>
      </section>
    </>
  );
}

// ---- schedule ------------------------------------------------------------------------------

function Schedule({ d, maps, send, saveSettings }: { d: Data; maps: MapInfo[]; send: (p: string, b: unknown, done?: string) => Promise<boolean>; saveSettings: SaveSettings }) {
  const empty = d.settings.empty;
  const [time, setTime] = useState<string | null>(null);
  const { run, busy } = useActions();
  const pickEmpty = (what: "map" | "mode", v: string) => run(xk(`empty:${what}`), () => saveSettings({ empty: { ...empty, [what]: v } }), what === "map" ? `Saving the map to go back to…` : "Saving the mode to go back to…");
  const names = useMemo(() => maps.map((m) => m.name).filter((n) => MAP_NAME.test(n)).sort((a, b) => a.localeCompare(b)), [maps]);
  const options = empty.map && !names.some((n) => n.toLowerCase() === empty.map.toLowerCase()) ? [empty.map, ...names] : names;
  const firstMap = d.rotation.main.find((m) => names.includes(m)) ?? names[0] ?? "de_dust2";
  const turnOn = (on: boolean) => saveSettings({ empty: { ...empty, on, map: empty.map || firstMap } });
  const sched = d.schedule;
  const at = time ?? sched?.time ?? "05:00";
  const saveRestart = (on: boolean, t = at) =>
    send("/api/extras/schedule", { on, time: t }, on ? `The server restarts every day at ${t}.` : "Daily restart is off").then((ok) => ok && setTime(null));

  return (
    <>
      <section>
        <SectionHead title="When the server empties" note="Once the last real player has left, wait a little, then go back to a map and mode of your choice. Bots don't count as players." />
        <div className="settings">
          <Toggle
            label="Go back to a set map"
            note={d.dedicated ? "So the next people to join find the server the way you like it." : "Your own game closes when you leave it, so this mostly matters on a dedicated server."}
            on={empty.on}
            set={turnOn}
          />
          {empty.on && (
            <>
              <Choice label="After" note="With nobody playing." values={EMPTY_MINUTES} current={empty.minutes} show={(v) => `${v} min`} pick={(v) => saveSettings({ empty: { ...empty, minutes: v } })} />
              <div className="setting">
                <span className="name">Map</span>
                <select className="sched-select" aria-label="Map to go back to" value={empty.map} disabled={busy(xk("empty:map"))} onChange={(e) => pickEmpty("map", e.target.value)}>
                  {options.map((n) => (
                    <option key={n} value={n}>{n}</option>
                  ))}
                </select>
              </div>
              {d.modes.length > 0 && (
                <div className="setting">
                  <span className="name">Mode<span className="note">Keep the mode, or switch to one.</span></span>
                  <select className="sched-select" aria-label="Mode to go back to" value={empty.mode} disabled={busy(xk("empty:mode"))} onChange={(e) => pickEmpty("mode", e.target.value)}>
                    <option value="">Keep the current mode</option>
                    {byName(d.modes).map((m) => (
                      <option key={m.id} value={m.id}>{m.name}</option>
                    ))}
                  </select>
                </div>
              )}
            </>
          )}
        </div>
      </section>
      {sched && (
        <section>
          <SectionHead title="Daily restart" note="Long-running servers stay smoother with a fresh start once a day. Players get a warning in chat 5 minutes and 1 minute before." />
          <div className="settings">
            <Toggle label="Restart every day" note="Only while the server is running and Loner-Strike is open on this computer." on={sched.on} set={(v) => saveRestart(v)} />
            <div className="setting">
              <span className="name">At<span className="note">This computer's time. Pick a quiet hour.</span></span>
              <div className="row sched-time">
                <input type="time" className="search" aria-label="Restart time" value={at} onChange={(e) => setTime(e.target.value)} />
                <BusyButton
                  busy={busy(xk("schedule:time"))}
                  disabled={time === null || time === sched.time || !/^\d{2}:\d{2}$/.test(at)}
                  label="Save time"
                  busyLabel="Saving…"
                  onClick={() => run(xk("schedule:time"), () => saveRestart(sched.on, at), "Saving the restart time…")}
                />
              </div>
            </div>
          </div>
        </section>
      )}
    </>
  );
}

// ---- fast downloads ------------------------------------------------------------------------

function FastDownloads({ d, saveSettings }: { d: Data; saveSettings: SaveSettings }) {
  const toast = useToast();
  const [url, setUrl] = useState<string | null>(null);
  const { run, busy } = useActions();
  const value = url ?? d.settings.fastdl;
  const ok = !value.trim() || URL_OK.test(value.trim());
  const save = () => run(xk("fastdl"), async () => {
    if (await saveSettings({ fastdl: value.trim() }, value.trim() ? "Saved. It's used from the next map." : "Fast downloads are off")) setUrl(null);
  }, "Saving the fast download address…");
  const exportFiles = () => run(xk("export"), async () => {
    type Done = { ok?: boolean; cancelled?: boolean; files?: number; mb?: number; dir?: string; error?: string };
    const r = await postJson<Done>("/api/extras/export").catch((): Done => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    if (r.cancelled) return;
    toast(r.files ? `Copied ${r.files} files (${r.mb} MB). Upload that folder's contents to your web server.` : "There's no custom content to copy: everything here comes with the game.");
  }, "Copying files for your web server…");
  return (
    <section>
      <SectionHead title="Fast downloads" />
      <div className="panel stack-tight">
        <p className="measure">
          Players who don't have a custom map, sound or model download it from the game server, which is slow. With fast downloads they get it from your web server over
          HTTP instead, in seconds. The folder on the web server has to look like the game folder: <b>maps/</b>, <b>sound/</b>, <b>models/</b>, <b>sprites/</b> and so on.
        </p>
        <label className="field">
          <span>Web address of that folder<Info text="Sets the game's sv_downloadurl. Any host that serves plain files works, like cheap web hosting or a public storage bucket." /></span>
          <input className="search" type="url" spellCheck={false} placeholder="https://dl.example.com/czero" value={value} onChange={(e) => setUrl(e.target.value)} />
          <small className={ok ? undefined : "err-text"}>{ok ? "Leave it empty to turn fast downloads off." : "Start with http:// or https://, with no spaces or quotes."}</small>
        </label>
        <div className="row">
          <BusyButton className="btn primary" busy={busy(xk("fastdl"))} disabled={url === null || url.trim() === d.settings.fastdl || !ok} label="Save" busyLabel="Saving…" onClick={save} />
          {ON_THIS_COMPUTER && (
            <BusyButton busy={busy(xk("export"))} label="Copy files for your web server" busyLabel="Copying…" onClick={exportFiles} />
          )}
        </div>
        {ON_THIS_COMPUTER && <p className="muted small-note">Copies the custom maps and what they need, plus the sounds and models of your game modes, into a folder you pick, ready to upload.</p>}
      </div>
    </section>
  );
}

// ---- small pieces ------------------------------------------------------------------------

// Each setting is off while its own change saves; different settings still queue up.
function Toggle({ label, note, on, set }: { label: string; note: ReactNode; on: boolean; set: (v: boolean) => unknown }) {
  const { run, busy } = useActions();
  const k = xk(`setting:${label}`);
  return (
    <div className="setting">
      <span className="name">{label}<span className="note">{note}</span></span>
      <Switch on={on} onChange={(v) => run(k, async () => { await set(v); }, `Saving ${label.toLowerCase()}…`)} label={label} disabled={busy(k)} />
    </div>
  );
}

function Choice({ label, note, values, current, show, pick }: { label: string; note: string; values: number[]; current: number; show: (v: number) => string; pick: (v: number) => unknown }) {
  const { run, busy } = useActions();
  const k = xk(`setting:${label}`);
  // a value set elsewhere still shows as picked
  const all = values.includes(current) ? values : [...values, current].sort((a, b) => (a || Infinity) - (b || Infinity));
  return (
    <div className="setting">
      <span className="name">{label}<span className="note">{note}</span></span>
      <div className="seg" role="group" aria-label={label}>
        {all.map((v) => (
          <button key={v} type="button" aria-pressed={current === v} disabled={busy(k)} onClick={() => run(k, async () => { await pick(v); }, `Saving ${label.toLowerCase()}…`)}>{show(v)}</button>
        ))}
      </div>
    </div>
  );
}
