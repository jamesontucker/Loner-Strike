import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { getJson, getJsonAt, ON_THIS_COMPUTER, postJson, SETUP_ON_COMPUTER, targetKey, useActions, useActivity } from "../api";
import { BOT_NOTE, BOT_RATING, BOT_RATING_INFO, BOT_STYLE_NOTE, BOT_STYLES, gameTitle } from "../games";
import { Icon } from "../icons";
import { reloadAppInfo, reloadScan, useTarget } from "../queries";
import { useNav } from "../store";
import { usePlayDraft, usePlayDrafts } from "../stores/playDraft";
import type { AppInfo, MapInfo, Mode, Online } from "../types";
import { BusyButton, Info, LoadError, SectionHead, Sheet, Skeleton, useCommand, useConfirm, useToast } from "../ui";
import { COLLATOR, MapCard } from "./Maps";
import { MatchMode } from "./MatchMode";

/** A start that hasn't brought the game up after this long can be sent again (CrossOver
 *  can take a minute to open the game). */
const LAUNCH_CEILING_MS = 90000;

export function PlayIdle({ app, reason, gameOpen, maps, openSetup }: { app: AppInfo | null; reason: string; gameOpen: boolean; maps: MapInfo[]; openSetup: () => void }) {
  const toast = useToast();
  const ask = useConfirm();
  const { run, busy } = useActions();
  const t = targetKey();
  const key = (what: string) => `play:${t}:${what}`;
  // after a start is sent, the button keeps saying Starting… until the game comes up
  // (stores/playDraft ends it), the request fails, or LAUNCH_CEILING_MS passes
  const launchedAt = usePlayDrafts((s) => s.launching[t]);
  const { startLaunch, endLaunch } = usePlayDrafts.getState();
  const [, tick] = useState(0);
  const overdue = launchedAt !== undefined && Date.now() - launchedAt >= LAUNCH_CEILING_MS;
  useEffect(() => {
    if (launchedAt === undefined || overdue) return;
    const timer = setTimeout(() => tick((n) => n + 1), launchedAt + LAUNCH_CEILING_MS - Date.now() + 50);
    return () => clearTimeout(timer);
  }, [launchedAt, overdue]);
  const starting = busy(key("launch")) || (launchedAt !== undefined && !overdue);
  const game = gameTitle(app?.game?.id);
  const gameId = app?.game?.id ?? "";
  // the form, kept per game across pages and restarts
  const [draft, setDraft] = usePlayDraft(gameId);
  const changed = draft.changed;
  const optsQ = useQuery({
    queryKey: ["startOptions", gameId],
    queryFn: () => getJson<StartOptions>("/api/app/start-options"),
    enabled: !!gameId,
  });
  const opts = optsQ.data && Array.isArray(optsQ.data.modes) ? optsQ.data : null;
  const launchMenu = () => run(key("open"), async () => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/app/launch", { menu: true }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) toast(r.error, true);
    else toast(`Opening ${game}…`);
  }, `Opening ${game}…`);
  // an open game ignores a new launch, so starting means closing it first (asked)
  const restart = gameOpen && ON_THIS_COMPUTER;
  const launch = async (map?: string, bots?: number) => {
    if (starting) return;
    if (restart && !(await ask(`${game} is open at its menu. Close it and start ${map ? `on ${map}` : "the game"}?`, { ok: "Restart and start" }))) return;
    // never chosen before: start with the defaults shown, so they're saved like a choice
    const fresh = Object.fromEntries(Object.entries(DEFAULTS).filter(([k]) => !changed[k] && mode?.settings[k] == null));
    const start = mode ? { mode: mode.id, settings: { ...fresh, ...changed } } : {};
    return run(key("launch"), async () => {
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/app/launch", { ...(map ? { map } : {}), ...(bots !== undefined ? { bots } : {}), ...start, ...(restart ? { restart: true } : {}) }).catch(() => ({ error: "Couldn't reach the app." }));
      if (r.error) {
        endLaunch(t);
        toast(r.error, true);
      } else {
        toast(`Starting ${game}…`);
        startLaunch(t);
      }
    }, `${restart ? "Restarting" : "Starting"} ${game}${map ? ` on ${map}` : ""}…`);
  };
  const bots = app?.bots ?? null;
  const modes = opts ? [...opts.modes].sort((a, b) => a.name.localeCompare(b.name)) : [];
  const mode = modes.find((m) => m.id === (draft.mode ?? opts?.mode)) ?? modes.find((m) => m.id === "classic") ?? modes[0];
  const value = (cvar: string) => changed[cvar] ?? mode?.settings[cvar] ?? DEFAULTS[cvar] ?? null;
  const change = (cvar: string, v: string) => setDraft({ changed: { ...changed, [cvar]: v } });
  const pickMode = (id: string) =>
    // time limit and friendly fire belong to the mode: show the new mode's own (bot settings stay)
    setDraft({ mode: id, changed: Object.fromEntries(Object.entries(changed).filter(([k]) => k === "bot_difficulty" || k === "gm_botstyle")) });
  if (app?.game && !app.game.installed?.includes("core")) {
    return (
      <div className="idle">
        <div className="idle-hero">
          <h1>Not set up yet</h1>
          <p>Loner-Strike adds the bots, the modes and the Game Mode Menu to {game}. Setup takes a couple of minutes, and everything it changes is backed up.</p>
          {ON_THIS_COMPUTER ? (
            <button className="btn launch" onClick={openSetup}>
              <Icon name="play" size={22} />
              Set it up
            </button>
          ) : (
            <p className="muted">{SETUP_ON_COMPUTER}</p>
          )}
          <div className="last">
            Or <button className="link inline" disabled={starting} aria-busy={starting} onClick={() => launch()}>{starting ? "starting…" : overdue ? "still starting? Start again" : `start ${game} without it`}</button>.
          </div>
        </div>
      </div>
    );
  }
  const recent = maps.filter((m) => m.recent >= 0).sort((a, b) => a.recent - b.recent).slice(0, 10);
  const favs = maps.filter((m) => m.fav && !m.hidden).sort((a, b) => COLLATOR.compare(a.name, b.name));
  const stock = app?.game?.id === "czero" ? "de_dust2_cz" : "de_dust2";
  // a picked map that's gone since (deleted, or hidden) falls back like no pick
  const pick = draft.map && (!maps.length || maps.some((m) => m.name === draft.map && !m.hidden)) ? draft.map : "";
  const map = pick || app?.lastmap || recent[0]?.name || favs[0]?.name || (maps.some((m) => m.name === stock) ? stock : maps[0]?.name) || stock;
  const botCount = draft.bots ?? bots ?? 9;

  if (!app?.game) {
    return (
      <div className="idle">
        <div className="idle-hero">
          <h1>No game yet</h1>
          <p>{reason || "Loner-Strike couldn't find Counter-Strike 1.6 or Condition Zero on this computer."}</p>
          <div className="row">
            <BusyButton className="btn primary" busy={busy(key("rescan"))} label="Look again" busyLabel="Looking…" onClick={() => run(key("rescan"), async () => { await Promise.all([reloadScan(), reloadAppInfo()]); }, "Looking for the game…")} />
            {ON_THIS_COMPUTER && <button className="btn" onClick={() => useNav.getState().go("settings")}>Choose the game folder</button>}
          </div>
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
            ? `${game} is open, but no game is loaded. Pick everything here and Loner-Strike restarts it into your game, or start one from its New Game menu.`
            : `${game} isn't running. Pick a mode, a map and the bots, and Loner-Strike starts it and connects by itself.`}
        </p>
        <div className="start-box">
          {optsQ.isError && !opts && (
            <LoadError text="Couldn't load the modes and bot settings. Starting now uses the game's last settings." onRetry={() => optsQ.refetch()} />
          )}
          {optsQ.isLoading && <div className="start-skeleton"><Skeleton rows={2} /></div>}
          {modes.length > 0 && mode && (
            <label className="field">
              <span>Mode<Info text="The mode the game starts in. Each mode remembers its own settings, and you can switch once you're in." /></span>
              <select value={mode.id} onChange={(e) => pickMode(e.target.value)}>
                {modes.map((m) => <option key={m.id} value={m.id}>{m.name}</option>)}
              </select>
            </label>
          )}
          <div className="field">
            <span id="start-map">Map<Info text="Maps made for this mode come first, then recent ones and favourites. Open the list and type to search." /></span>
            <MapPicker value={map} maps={maps} mode={mode?.id} modeName={mode?.name} recent={recent} favs={favs} onPick={(m) => setDraft({ map: m })} labelledBy="start-map" />
          </div>
        </div>
        <div className="start-box start-settings">
          <div className="field">
            <span>Bots<Info text="How many bots play. Each one takes a player slot, so leave room for friends." /></span>
            <div className="seg" role="group" aria-label="Bots">
              {[0, 5, 9, 15].map((n) => <button key={n} aria-pressed={botCount === n} onClick={() => setDraft({ bots: n })}>{n === 0 ? "None" : n}</button>)}
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
              <span className="field-note">{BOT_STYLE_NOTE[value("gm_botstyle") ?? DEFAULTS.gm_botstyle]}</span>
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
              <span>Friendly fire<Info text="On: bullets and grenades hurt your own team." /></span>
              <div className="seg" role="group" aria-label="Friendly fire">
                {[["0", "Off"], ["1", "On"]].map(([v, l]) => <button key={v} aria-pressed={(value("mp_friendlyfire") ?? "0") === v} onClick={() => change("mp_friendlyfire", v)}>{l}</button>)}
              </div>
            </div>
          )}
        </div>
        <BusyButton
          className="btn launch"
          busy={starting}
          label={<><Icon name="play" size={22} />{overdue ? "Still starting? Start again" : restart ? `Restart ${game} and play ${mode ? `${mode.name} on ` : ""}${map}` : mode ? `Play ${mode.name} on ${map}` : `Play ${map}`}</>}
          busyLabel={restart ? "Restarting…" : "Starting…"}
          onClick={() => launch(map, botCount)}
        />
        <div className="last">
          You can change all of this once you're in, here or with F3 in the game.{" "}
          <button className="link inline" disabled={busy(key("open"))} aria-busy={busy(key("open"))} onClick={() => launchMenu()}>{busy(key("open")) ? `Opening ${game}…` : `Open ${game} to its main menu`}</button>
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
  const { run: once } = useActions();
  const t = targetKey();
  const active = useActivity();
  // a mode switch or preset in progress (only one at a time), and a map change (same key as the Maps page)
  const modeBusy = active.find(([k]) => k.startsWith(`play:${t}:mode:`))?.[0].slice(`play:${t}:mode:`.length) ?? null;
  const presetBusy = active.find(([k]) => k.startsWith(`play:${t}:preset:`))?.[0].slice(`play:${t}:preset:`.length) ?? null;
  const changeMap = (map: string) => once(`maps:${t}:change`, () => run("gm_map", [map]), `Changing the map to ${map}…`);
  const switchMode = (m: Mode, map: string | null) => {
    if (modeBusy) return;
    once(`play:${t}:mode:${m.id}`, () => run("gm_mode", map ? [m.id, map] : [m.id]), `Switching to ${m.name}${map ? ` on ${map}` : ""}…`);
  };

  return (
    <div className="stack">
      {st.mode === "match" && <MatchMode target={targetKey()} />}
      {switching && <SwitchMode mode={switching} current={st.mode} map={st.map} maps={maps} onClose={() => setSwitching(null)} onGo={(map) => { setSwitching(null); if (switching.id === st.mode) { if (map) changeMap(map); } else switchMode(switching, map); }} />}
      {data.presets.length > 0 && (
        <section>
          <SectionHead title="Presets" note="One click sets the mode, its settings and the bots." />
          <div className="presets">
            {data.presets.map((p) => (
              <button
                key={p.id}
                className="preset"
                aria-pressed={st.preset === p.id}
                disabled={!!presetBusy}
                aria-busy={presetBusy === p.id}
                onClick={async () => (await ask(`Play "${p.name}"? This resets that mode's settings to the preset.`, { ok: "Play it" })) && once(`play:${t}:preset:${p.id}`, () => run("gm_preset", [p.id]), `Loading the "${p.name}" preset…`)}
              >
                <span className="preset-name">{presetBusy === p.id && <span className="spinner" aria-hidden="true" />}{p.name}</span>
                <span className="preset-desc">{p.desc}</span>
              </button>
            ))}
          </div>
        </section>
      )}
      <section>
        <SectionHead title="Game mode" note={<>Switching reloads the map. Each mode remembers its own settings. Each card says how well bots <span className="nowrap">play it.<Info text={BOT_RATING_INFO} /></span></>} />
        <div className="modes">
          {[...data.modes].sort((a, b) => a.name.localeCompare(b.name)).map((m) => {
            const rating = BOT_RATING[m.id] ?? "great";
            return (
              <button
                key={m.id}
                className="mode"
                aria-pressed={st.mode === m.id}
                disabled={!!modeBusy}
                aria-busy={modeBusy === m.id}
                onClick={() => setSwitching(m)}
              >
                <span className="mode-icon">
                  <Icon name={m.id} size={26} />
                </span>
                <span className="mode-name">{modeBusy === m.id && <span className="spinner" aria-hidden="true" />}{m.name}</span>
                <span className="mode-desc">{m.desc}</span>
                <span className={`bots-note ${rating}`} title={BOT_RATING_INFO}>{BOT_NOTE[rating]}</span>
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
              <MapCard key={m.name} map={m} onClick={async () => (await ask(`Change the map to ${m.name}? Everyone playing moves to it.`, { ok: "Change map" })) && changeMap(m.name)} />
            ))}
          </div>
        </section>
      )}
    </div>
  );
}

type StartOptions = { modes: { id: string; name: string; settings: Record<string, string | null> }[]; mode: string };
// What the screen shows (and starts the game with) when nothing's been chosen yet
const DEFAULTS: Record<string, string> = { bot_difficulty: "2", gm_botstyle: "8" };
const TIME_LIMITS: [string, string][] = [["0", "None"], ["10", "10 min"], ["20", "20 min"], ["30", "30 min"], ["45", "45 min"], ["60", "60 min"]];

/** A touch screen: the search box doesn't take focus by itself (that opens the keyboard over the list). */
const TOUCH = window.matchMedia("(pointer: coarse)").matches;

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
  // sorted once per map list, not on every render (this re-renders with the game's state)
  const have = useMemo(() => maps.filter((m) => !m.hidden).sort((a, b) => COLLATOR.compare(a.name, b.name)), [maps]);
  const query = q.trim().toLowerCase();
  const forMode = useMemo(() => {
    const prefixes = (mode && MODE_PREFIX[mode]) || [];
    return prefixes.length ? have.filter((m) => prefixes.some((p) => m.name.toLowerCase().startsWith(p))) : [];
  }, [have, mode]);
  // (sort is stable, so matches at the same place stay in name order)
  const found = useMemo(() => (query ? have.filter((m) => m.name.toLowerCase().includes(query)).sort((a, b) => a.name.toLowerCase().indexOf(query) - b.name.toLowerCase().indexOf(query)) : []), [have, query]);
  const groups: [string, MapInfo[]][] = query
    ? [["", found]]
    : [
        [`Maps for ${modeName ?? "this mode"}`, forMode],
        ["Recently played", recent],
        ["Favourites", favs],
        ["All maps", have],
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
  const optId = (i: number) => `map-opt-${labelledBy}-${i}`;
  return (
    <div className="map-picker" ref={box}>
      <button ref={button} type="button" className="map-picker-button" aria-haspopup="listbox" aria-expanded={open} aria-labelledby={labelledBy} onClick={() => { setOpen((o) => !o); setActive(Math.max(0, flat.findIndex((m) => m.name === value))); }}>
        <span>{value}</span>
        <span className="caret" aria-hidden="true">▾</span>
      </button>
      {open && (
        <div className="map-picker-pop">
          <input className="search" type="search" autoFocus={!TOUCH} placeholder={`Search ${have.length} maps`} value={q} onChange={(e) => { setQ(e.target.value); setActive(0); }} onKeyDown={key} aria-label="Search maps" role="combobox" aria-expanded="true" aria-controls={`${labelledBy}-list`} aria-autocomplete="list" aria-activedescendant={flat[active] ? optId(active) : undefined} />
          <div className="map-picker-list" id={`${labelledBy}-list`} role="listbox" aria-label="Maps">
            {flat.length === 0 && <p className="muted small-note">No map matches. Find more in Get more.</p>}
            {shown.map(([label, list]) => (
              <div key={label || "results"}>
                {label && <div className="map-picker-group">{label}</div>}
                {list.map((m) => {
                  n++;
                  const i = n;
                  return (
                    <button key={label + m.name} id={optId(i)} type="button" role="option" tabIndex={-1} aria-selected={m.name === value} className={i === active ? "active" : ""} onMouseEnter={() => setActive(i)} onClick={() => choose(m.name)}>
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
  const target = useTarget();
  const listedQ = useQuery({
    queryKey: ["modeMaps", target, mode.id],
    queryFn: async ({ signal }) => {
      const l = await getJsonAt<string[]>(`/api/modes/${encodeURIComponent(mode.id)}/maps`, target, signal);
      return Array.isArray(l) ? l : [];
    },
    staleTime: 60000,
  });
  // a list that didn't load falls back to maps whose names fit the mode
  const listed = listedQ.data ?? (listedQ.isError ? [] : null);
  // null: not chosen yet; "": keep the current map
  const [pick, setPick] = useState<string | null>(null);
  const [q, setQ] = useState("");
  const have = maps.filter((m) => !m.hidden);
  const byName = new Map(have.map((m) => [m.name.toLowerCase(), m]));
  const prefixes = MODE_PREFIX[mode.id] ?? [];
  const suggested = listed?.length
    ? listed.map((n) => byName.get(n.toLowerCase())).filter((m): m is MapInfo => !!m)
    : have.filter((m) => prefixes.some((p) => m.name.toLowerCase().startsWith(p)));
  const query = q.trim().toLowerCase();
  // which maps: the mode's own (when it has any), every map, favourites or recently played;
  // the search narrows whichever is picked
  const [show, setShow] = useState<"mode" | "all" | "fav" | "recent" | null>(null);
  const filters: ["mode" | "all" | "fav" | "recent", string][] = [
    ...(suggested.length ? [["mode", listed?.length ? `${mode.name}'s maps` : "Made for this mode"] as ["mode", string]] : []),
    ["all", "All"],
    ["fav", "Favourites"],
    ["recent", "Recent"],
  ];
  const which = show && filters.some(([id]) => id === show) ? show : listed === null ? "mode" : filters[0][0];
  const pool =
    which === "mode" ? suggested
    : which === "fav" ? have.filter((m) => m.fav)
    : which === "recent" ? have.filter((m) => m.recent >= 0).sort((a, b) => a.recent - b.recent)
    : have;
  const shown = (query ? pool.filter((m) => m.name.toLowerCase().includes(query) || (m.title ?? "").toLowerCase().includes(query)) : pool).slice(0, 60);
  const emptyNote =
    which === "fav" ? "No favourite maps yet. Star maps on the Maps page."
    : which === "recent" ? "No maps played yet."
    : "No maps yet. Get some from Get more.";
  const fits = !listed?.length && (!prefixes.length || prefixes.some((p) => map.toLowerCase().startsWith(p)));
  // a map built for one particular mode (a SoccerJam pitch, a zombie or climb map): its goals,
  // traps or course are part of the map and stay whatever mode is loaded
  const madeForOther = Object.entries(MODE_PREFIX).some(
    ([id, ps]) => id !== mode.id && !["classic", "war3", "shero", "pistols", "deagle", "awp", "scoutz", "knife"].includes(id) && ps.some((p) => map.toLowerCase().startsWith(p)),
  );
  const same = mode.id === current;
  const chosen = pick === "" ? null : pick ?? (same || fits ? null : suggested[0]?.name ?? null);
  return (
    <Sheet label={`Play ${mode.name}`} onClose={onClose} className="sheet wide switch-mode">
        <div className="sheet-body">
          <div className="row between">
            <h2>{mode.name}</h2>
            <span className="muted">{mode.desc}</span>
          </div>
          <p className="muted">{same ? "You're playing this mode. Pick a map to change to." : "Pick the map to play it on. The mode remembers its own settings."}</p>
          <div className="toolbar">
            {!same && (
              <button className="chip" aria-pressed={chosen === null} onClick={() => setPick("")}>
                Keep {map}{fits ? "" : madeForOther ? " (built for another mode)" : " (may not suit this mode)"}
              </button>
            )}
            <input className="search grow" type="search" placeholder={`Search ${filters.find(([id]) => id === which)?.[1].toLowerCase() ?? "maps"}`} aria-label="Search maps" value={q} onChange={(e) => setQ(e.target.value)} />
          </div>
          <div className="chips" role="tablist" aria-label="Which maps">
            {filters.map(([id, label]) => (
              <button key={id} role="tab" className="chip" aria-selected={which === id} onClick={() => setShow(id)}>
                {label}
              </button>
            ))}
          </div>
          {listed === null && which === "mode" && <p className="muted small-note">Finding maps for this mode…</p>}
          {listed !== null && !suggested.length && which === "all" && !query && <p className="muted small-note">No maps look made for this mode yet, so here's every map.</p>}
          {shown.length === 0 && listed !== null && <p className="muted small-note">{query ? `No map matches "${q.trim()}" here.` : emptyNote}</p>}
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
    </Sheet>
  );
}
