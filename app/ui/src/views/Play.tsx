import { useEffect, useRef, useState } from "react";
import { getJson, postJson } from "../api";
import { Icon } from "../icons";
import type { AppInfo, MapInfo, Mode, Online } from "../types";
import { Info, SectionHead, useCommand, useConfirm, useToast } from "../ui";
import { MapCard } from "./Maps";
import { MatchMode } from "./MatchMode";

// How well bots handle each mode (shown on the cards)
const BOT_RATING: Record<string, "great" | "ok" | "weak" | "none"> = {
  hns: "ok", deathrun: "ok", surf: "ok", soccerjam: "weak", basebuilder: "weak", kreedz: "none",
};
const BOT_NOTE = { great: "Bots play this well", ok: "Bots are so-so", weak: "Best with a friend", none: "Solo, no bots" };

const GAME_NAME = (id?: string) => (id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero");

export function PlayIdle({ app, reason, gameOpen, maps, openSetup }: { app: AppInfo | null; reason: string; gameOpen: boolean; maps: MapInfo[]; openSetup: () => void }) {
  const toast = useToast();
  const [starting, setStarting] = useState(false);
  const game = GAME_NAME(app?.game?.id);
  const [pick, setPick] = useState("");
  const [botsPick, setBotsPick] = useState<number | null>(null);
  const [opts, setOpts] = useState<StartOptions | null>(null);
  const [modePick, setModePick] = useState<string | null>(null);
  // only what was changed here is saved; the rest stays as the mode has it
  const [changed, setChanged] = useState<Record<string, string>>({});
  useEffect(() => {
    getJson<StartOptions>("/api/app/start-options").then((o) => setOpts(Array.isArray(o.modes) ? o : null)).catch(() => setOpts(null));
  }, [app?.game?.id]);
  const launchMenu = async () => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/app/launch", { menu: true }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) toast(r.error, true);
    else toast(`Opening ${game}…`);
  };
  const launch = async (map?: string, bots?: number) => {
    if (starting) return;
    setStarting(true);
    // never chosen before: start with the defaults shown, so they're saved like a choice
    const fresh = Object.fromEntries(Object.entries(DEFAULTS).filter(([k]) => !changed[k] && mode?.settings[k] == null));
    const start = mode ? { mode: mode.id, settings: { ...fresh, ...changed } } : {};
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/app/launch", { ...(map ? { map } : {}), ...(bots !== undefined ? { bots } : {}), ...start }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) {
      toast(r.error, true);
      setStarting(false);
    } else {
      toast(`Starting ${game}…`);
      setTimeout(() => setStarting(false), 20000);
    }
  };
  const bots = app?.bots ?? null;
  const modes = opts ? [...opts.modes].sort((a, b) => a.name.localeCompare(b.name)) : [];
  const mode = modes.find((m) => m.id === (modePick ?? opts?.mode)) ?? modes.find((m) => m.id === "classic") ?? modes[0];
  const value = (cvar: string) => changed[cvar] ?? mode?.settings[cvar] ?? DEFAULTS[cvar] ?? null;
  const change = (cvar: string, v: string) => setChanged((c) => ({ ...c, [cvar]: v }));
  const pickMode = (id: string) => {
    setModePick(id);
    // time limit and friendly fire belong to the mode: show the new mode's own (bot settings stay)
    setChanged((c): Record<string, string> => Object.fromEntries(Object.entries(c).filter(([k]) => k === "bot_difficulty" || k === "gm_botstyle")));
  };
  if (app?.game && !app.game.installed?.includes("core")) {
    return (
      <div className="idle">
        <div className="idle-hero">
          <h1>Not set up yet.</h1>
          <p>Loner-Strike adds the bots, the modes and the in-game menu to {game}. Setup takes a couple of minutes, and everything it changes is backed up.</p>
          <button className="btn launch" onClick={openSetup}>
            <Icon name="play" size={22} />
            Set it up
          </button>
          <div className="last">
            Or <button className="link inline" onClick={() => launch()}>start {game} without it</button>.
          </div>
        </div>
      </div>
    );
  }
  const recent = maps.filter((m) => m.recent >= 0).sort((a, b) => a.recent - b.recent).slice(0, 10);
  const favs = maps.filter((m) => m.fav && !m.hidden).sort((a, b) => a.name.localeCompare(b.name));
  const stock = app?.game?.id === "czero" ? "de_dust2_cz" : "de_dust2";
  const map = pick || app?.lastmap || recent[0]?.name || favs[0]?.name || (maps.some((m) => m.name === stock) ? stock : maps[0]?.name) || stock;
  const botCount = botsPick ?? bots ?? 9;

  if (!app?.game) {
    return (
      <div className="idle">
        <div className="idle-hero">
          <h1>No game yet.</h1>
          <p>{reason || "Loner-Strike couldn't find Counter-Strike 1.6 or Condition Zero on this computer."}</p>
        </div>
      </div>
    );
  }

  return (
    <div className="idle">
      <div className="idle-hero">
        <h1>Start a game</h1>
        <p>
          {gameOpen
            ? `${game} is open, but no game is loaded. Start one from its New Game menu, or quit it and pick everything here.`
            : `${game} isn't running. Pick a mode, a map and the bots, and Loner-Strike starts it and connects by itself.`}
        </p>
        <div className="start-box">
          {modes.length > 0 && mode && (
            <label className="field">
              <span>Mode<Info text="The game mode the game starts in. It remembers its own settings; you can switch modes once you're in, too." /></span>
              <select value={mode.id} onChange={(e) => pickMode(e.target.value)}>
                {modes.map((m) => <option key={m.id} value={m.id}>{m.name}</option>)}
              </select>
            </label>
          )}
          <div className="field">
            <span id="start-map">Map<Info text="Type to search. Maps made for the mode you picked come first, then the ones you played recently and your favourites." /></span>
            <MapPicker value={map} maps={maps} mode={mode?.id} modeName={mode?.name} recent={recent} favs={favs} onPick={setPick} labelledBy="start-map" />
          </div>
        </div>
        <div className="start-box start-settings">
          <div className="field">
            <span>Bots<Info text="How many bots join. You can add or remove bots once you're in." /></span>
            <div className="seg" role="group" aria-label="Bots">
              {[0, 5, 9, 15].map((n) => <button key={n} aria-pressed={botCount === n} onClick={() => setBotsPick(n)}>{n === 0 ? "None" : n}</button>)}
            </div>
          </div>
          {mode && botCount > 0 && (
            <div className="field">
              <span>Bot skill<Info text="How smart the bots are: how aware they are, their tactics, how they move as a team. Their aim and reactions come from the bot style. Hard suits most bot styles." /></span>
              <div className="seg" role="group" aria-label="Bot skill">
                {["Easy", "Normal", "Hard", "Expert"].map((l, i) => <button key={l} aria-pressed={value("bot_difficulty") === String(i)} onClick={() => change("bot_difficulty", String(i))}>{l}</button>)}
              </div>
            </div>
          )}
          {mode && botCount > 0 && (
            <label className="field">
              <span>Bot style<Info text="How the bots aim, react and behave: from new players to suspiciously good. Mixed lobby gives every bot its own level, like a real server." /></span>
              <select value={value("gm_botstyle") ?? DEFAULTS.gm_botstyle} onChange={(e) => change("gm_botstyle", e.target.value)}>
                {BOT_STYLES.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
              </select>
              <span className="field-note">{STYLE_NOTE[value("gm_botstyle") ?? DEFAULTS.gm_botstyle]}</span>
            </label>
          )}
          {mode && (
            <label className="field">
              <span>Time limit<Info text="How long a map lasts before the next one. None plays until you change it." /></span>
              <select value={value("mp_timelimit") ?? ""} onChange={(e) => change("mp_timelimit", e.target.value)}>
                {value("mp_timelimit") === null && <option value="">As the mode has it</option>}
                {TIME_LIMITS.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
                {value("mp_timelimit") !== null && !TIME_LIMITS.some(([v]) => v === value("mp_timelimit")) && <option value={value("mp_timelimit")!}>{value("mp_timelimit")} min</option>}
              </select>
            </label>
          )}
          {mode && (
            <div className="field">
              <span>Friendly fire<Info text="Whether you can hurt your own team." /></span>
              <div className="seg" role="group" aria-label="Friendly fire">
                {[["0", "Off"], ["1", "On"]].map(([v, l]) => <button key={v} aria-pressed={(value("mp_friendlyfire") ?? "0") === v} onClick={() => change("mp_friendlyfire", v)}>{l}</button>)}
              </div>
            </div>
          )}
        </div>
        <button className="btn launch" disabled={starting} onClick={() => launch(map, botCount)}>
          <Icon name="play" size={22} />
          {starting ? "Starting…" : mode ? `Play ${mode.name} on ${map}` : `Play ${map}`}
        </button>
        <div className="last">
          You can change all of this once you're in, here or with F3 in the game.{" "}
          <button className="link inline" onClick={() => launchMenu()}>Just open {game}</button>
        </div>
      </div>
      {(recent.length > 0 || favs.length > 0) && (
        <section>
          <h2>{recent.length > 0 ? "Recently played" : "Favourites"}</h2>
          <div className="maps">
            {(recent.length > 0 ? recent.slice(0, 6) : favs.slice(0, 6)).map((m) => (
              <MapCard key={m.name} map={m} onClick={() => launch(m.name, botCount)} />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

export function PlayLive({ data, maps, refresh, openMaps }: { data: Online; maps: MapInfo[]; refresh: () => void; openMaps: () => void }) {
  const ask = useConfirm();
  const run = useCommand(refresh);
  const st = data.state;
  const recent = maps.filter((m) => m.recent >= 0 && m.name.toLowerCase() !== st.map.toLowerCase()).sort((a, b) => a.recent - b.recent).slice(0, 6);
  const [switching, setSwitching] = useState<Mode | null>(null);

  return (
    <div className="stack">
      {st.mode === "match" && <MatchMode />}
      {switching && <SwitchMode mode={switching} current={st.mode} map={st.map} maps={maps} onClose={() => setSwitching(null)} onGo={(map) => { setSwitching(null); if (switching.id === st.mode) { if (map) run("gm_map", [map]); } else run("gm_mode", map ? [switching.id, map] : [switching.id]); }} />}
      {data.presets.length > 0 && (
        <section>
          <SectionHead title="Presets" note="One click sets the mode, its settings and the bots." />
          <div className="presets">
            {data.presets.map((p) => (
              <button
                key={p.id}
                className="preset"
                aria-pressed={st.preset === p.id}
                onClick={async () => (await ask(`Play "${p.name}"? This resets that mode's settings to the preset.`, { ok: "Play it" })) && run("gm_preset", [p.id])}
              >
                <span className="preset-name">{p.name}</span>
                <span className="preset-desc">{p.desc}</span>
              </button>
            ))}
          </div>
        </section>
      )}
      <section>
        <SectionHead title="Game mode" note="Switching reloads the map. Each mode remembers its own settings." />
        <div className="modes">
          {data.modes.map((m) => {
            const rating = BOT_RATING[m.id] ?? "great";
            return (
              <button
                key={m.id}
                className="mode"
                aria-pressed={st.mode === m.id}
                onClick={() => setSwitching(m)}
              >
                <span className="mode-icon">
                  <Icon name={m.id} size={26} />
                </span>
                <span className="mode-name">{m.name}</span>
                <span className="mode-desc">{m.desc}</span>
                <span className={`bots-note ${rating}`}>{BOT_NOTE[rating]}</span>
              </button>
            );
          })}
        </div>
      </section>
      {recent.length > 0 && (
        <section>
          <SectionHead title="Recently played">
            <button className="link" onClick={openMaps}>All maps</button>
          </SectionHead>
          <div className="maps">
            {recent.map((m) => (
              <MapCard key={m.name} map={m} onClick={() => run("gm_map", [m.name])} />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

type StartOptions = { modes: { id: string; name: string; settings: Record<string, string | null> }[]; mode: string };
// What each bot style is like
const STYLE_NOTE: Record<string, string> = {
  "0": "The game's own bots: instant reactions, perfect aim.",
  "4": "New players: slow to react, they spray and miss a lot.",
  "1": "A typical public server: a few sharp players, most average, some bad.",
  "8": "Everyone different, from new to very good. A regular keeps their level.",
  "2": "Solid players who work as a team.",
  "3": "Experienced: quick, accurate, short bursts.",
  "5": "Very good and focused. They bunnyhop between fights.",
  "6": "Suspiciously good: most bullets find you and they notice you from behind.",
  "7": "Everything maxed. Every bullet near you is a headshot.",
};
// What the screen shows (and starts the game with) when nothing's been chosen yet
const DEFAULTS: Record<string, string> = { bot_difficulty: "2", gm_botstyle: "8" };
// gm_botstyle values, in the order the in-game menu shows them
const BOT_STYLES: [string, string][] = [["0", "Classic"], ["4", "Newbie lobby"], ["1", "Pub server"], ["8", "Mixed lobby"], ["2", "Matchmaking"], ["3", "Veterans"], ["5", "Locked In"], ["6", "Hackers"], ["7", "Gods"]];
const TIME_LIMITS: [string, string][] = [["0", "None"], ["10", "10 min"], ["20", "20 min"], ["30", "30 min"], ["45", "45 min"], ["60", "60 min"]];

/** A map list you can type into: the mode's maps, recent and favourites first. */
function MapPicker({ value, maps, mode, modeName, recent, favs, onPick, labelledBy }: { value: string; maps: MapInfo[]; mode?: string; modeName?: string; recent: MapInfo[]; favs: MapInfo[]; onPick: (m: string) => void; labelledBy: string }) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const [active, setActive] = useState(0);
  const box = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (!open) return;
    const away = (e: Event) => !box.current?.contains(e.target as Node) && setOpen(false);
    document.addEventListener("pointerdown", away);
    return () => document.removeEventListener("pointerdown", away);
  }, [open]);
  const have = maps.filter((m) => !m.hidden);
  const query = q.trim().toLowerCase();
  const prefixes = (mode && MODE_PREFIX[mode]) || [];
  const groups: [string, MapInfo[]][] = query
    ? [["", have.filter((m) => m.name.toLowerCase().includes(query)).sort((a, b) => a.name.toLowerCase().indexOf(query) - b.name.toLowerCase().indexOf(query) || a.name.localeCompare(b.name))]]
    : [
        [`Maps for ${modeName ?? "this mode"}`, prefixes.length ? have.filter((m) => prefixes.some((p) => m.name.toLowerCase().startsWith(p))).sort((a, b) => a.name.localeCompare(b.name)) : []],
        ["Recently played", recent],
        ["Favourites", favs],
        ["All maps", [...have].sort((a, b) => a.name.localeCompare(b.name))],
      ];
  const shown = groups.map(([label, list]) => [label, list.slice(0, query ? 80 : 200)] as [string, MapInfo[]]).filter(([, l]) => l.length);
  const flat = shown.flatMap(([, l]) => l);
  const choose = (name: string) => {
    onPick(name);
    setOpen(false);
    setQ("");
    button.current?.focus();
  };
  const key = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setActive((a) => Math.max(0, Math.min(flat.length - 1, a + (e.key === "ArrowDown" ? 1 : -1))));
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (flat[active]) choose(flat[active].name);
    } else if (e.key === "Escape") {
      setOpen(false);
      button.current?.focus();
    }
  };
  useEffect(() => {
    box.current?.querySelector(".map-picker-list .active")?.scrollIntoView({ block: "nearest" });
  }, [active]);
  let n = -1;
  return (
    <div className="map-picker" ref={box}>
      <button ref={button} type="button" className="map-picker-button" aria-haspopup="listbox" aria-expanded={open} aria-labelledby={labelledBy} onClick={() => { setOpen((o) => !o); setActive(Math.max(0, flat.findIndex((m) => m.name === value))); }}>
        <span>{value}</span>
        <span className="caret" aria-hidden="true">▾</span>
      </button>
      {open && (
        <div className="map-picker-pop">
          <input className="search" type="search" autoFocus placeholder={`Search ${have.length} maps`} value={q} onChange={(e) => { setQ(e.target.value); setActive(0); }} onKeyDown={key} aria-label="Search maps" />
          <div className="map-picker-list" role="listbox" aria-label="Maps">
            {flat.length === 0 && <p className="muted small-note">No map called that. Get more from Get more.</p>}
            {shown.map(([label, list]) => (
              <div key={label || "results"}>
                {label && <div className="map-picker-group">{label}</div>}
                {list.map((m) => {
                  n++;
                  const i = n;
                  return (
                    <button key={label + m.name} type="button" role="option" aria-selected={m.name === value} className={i === active ? "active" : ""} onMouseEnter={() => setActive(i)} onClick={() => choose(m.name)}>
                      {m.name}
                      {m.fav && <span className="muted" aria-label="favourite"> ★</span>}
                    </button>
                  );
                })}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// Map names that suit a mode, for modes without their own map list
const MODE_PREFIX: Record<string, string[]> = {
  zombie: ["zm_", "zp_", "ze_"], zp5: ["zm_", "zp_", "ze_"], hns: ["hns_", "hide_", "hide"], deathrun: ["deathrun_", "dr_"],
  kreedz: ["kz_", "bkz_", "xc_", "kzm_"], surf: ["surf_"], soccerjam: ["soccerjam", "sj_"], basebuilder: ["bb_", "basebuilder"],
  paintball: ["pb_", "paintball", "cspaintball"], knife: ["ka_", "knife", "35hp"], awp: ["awp_"], scoutz: ["scoutz", "scout"],
  pistols: ["aim_", "fy_", "pistol"], deagle: ["aim_", "fy_", "deagle"], classic: ["de_", "cs_", "as_"], war3: ["de_", "cs_"], shero: ["de_", "cs_"],
};

/** Switching mode, with the map to play it on: the mode's own maps first, or the current one. */
function SwitchMode({ mode, current, map, maps, onClose, onGo }: { mode: Mode; current: string; map: string; maps: MapInfo[]; onClose: () => void; onGo: (map: string | null) => void }) {
  const [listed, setListed] = useState<string[] | null>(null);
  // null: not chosen yet; "": keep the current map
  const [pick, setPick] = useState<string | null>(null);
  const [q, setQ] = useState("");
  useEffect(() => {
    getJson<string[]>(`/api/modes/${encodeURIComponent(mode.id)}/maps`).then((l) => setListed(Array.isArray(l) ? l : [])).catch(() => setListed([]));
  }, [mode.id]);
  useEffect(() => {
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onClose]);
  const have = maps.filter((m) => !m.hidden);
  const byName = new Map(have.map((m) => [m.name.toLowerCase(), m]));
  const prefixes = MODE_PREFIX[mode.id] ?? [];
  const suggested = listed?.length
    ? listed.map((n) => byName.get(n.toLowerCase())).filter((m): m is MapInfo => !!m)
    : have.filter((m) => prefixes.some((p) => m.name.toLowerCase().startsWith(p)));
  const query = q.trim().toLowerCase();
  const shown = (query ? have.filter((m) => m.name.toLowerCase().includes(query)) : suggested).slice(0, 60);
  const fits = !listed?.length && (!prefixes.length || prefixes.some((p) => map.toLowerCase().startsWith(p)));
  const same = mode.id === current;
  const chosen = pick === "" ? null : pick ?? (same || fits ? null : suggested[0]?.name ?? null);
  return (
    <div className="sheet-backdrop" onClick={onClose}>
      <div className="sheet wide switch-mode" role="dialog" aria-label={`Play ${mode.name}`} onClick={(e) => e.stopPropagation()}>
        <div className="sheet-body">
          <div className="row between">
            <h2>{mode.name}</h2>
            <span className="muted">{mode.desc}</span>
          </div>
          <p className="muted">{same ? "You're playing this mode. Pick a map to change to." : "Pick the map to play it on. The mode remembers its own settings."}</p>
          <div className="toolbar">
            {!same && (
              <button className="chip" aria-pressed={chosen === null} onClick={() => setPick("")}>
                Keep {map}{fits ? "" : " (may not suit this mode)"}
              </button>
            )}
            <input className="search grow" type="search" placeholder="Search all maps" value={q} onChange={(e) => setQ(e.target.value)} />
          </div>
          {!query && (
            <p className="muted small-note">
              {listed === null ? "Finding maps for this mode…" : listed.length ? `${mode.name}'s own map list` : suggested.length ? "Maps that look made for this mode" : "No maps that look made for this mode yet. Search, or get some from Get more."}
            </p>
          )}
          <div className="maps grid switch-maps">
            {shown.map((m) => (
              <MapCard key={m.name} map={m} selected={chosen === m.name} onClick={() => setPick(m.name)} />
            ))}
          </div>
          <div className="row">
            <button className="btn primary" disabled={same && !chosen} onClick={() => onGo(chosen)}>
              {same ? `Change to ${chosen ?? "a map"}` : `Play ${mode.name} on ${chosen ?? map}`}
            </button>
            <button className="btn" onClick={onClose}>Cancel</button>
          </div>
        </div>
      </div>
    </div>
  );
}
