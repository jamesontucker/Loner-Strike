import { useEffect, useState } from "react";
import { currentTarget, getJson, isAbort, ON_THIS_COMPUTER, postJson, targetKey, useActions, useActivity, useStable } from "../api";
import { Icon } from "../icons";
import { reloadAppInfo, reloadScan, useSetupJob, waitForSetupJob } from "../queries";
import { useNav } from "../store";
import type { AppInfo } from "../types";
import { BusyButton, Info, LoadError, SectionHead, Skeleton, SteamProgress, useConfirm, useToast } from "../ui";
import type { Scan } from "./Setup";
import { BOT_NOTE, BOT_RATING, BOT_RATING_INFO } from "../games";


// Part of Loner-Strike's own plugin: always there once the basics are installed
const BUILT_IN: { id: string; name: string; kind: "mode" | "addon"; description: string }[] = [
  { id: "classic", name: "Classic", kind: "mode", description: "Bomb defusal and hostage rescue, the way the game plays" },
  { id: "csdm", name: "Deathmatch", kind: "mode", description: "Teams, instant respawn, pick your guns" },
  { id: "csdm_ffa", name: "Free for all", kind: "mode", description: "Everyone for themselves, instant respawn" },
  { id: "knife", name: "Knife arena", kind: "mode", description: "Knives only" },
  { id: "pistols", name: "Pistols only", kind: "mode", description: "No primaries" },
  { id: "deagle", name: "Deagle only", kind: "mode", description: "Free Deagle every spawn" },
  { id: "awp", name: "AWP only", kind: "mode", description: "Free AWP every spawn" },
  { id: "scoutz", name: "Scoutz & Knivez", kind: "mode", description: "Scouts, low gravity, air control" },
  { id: "surf", name: "Surf", kind: "mode", description: "Air control, free guns, long rounds" },
  { id: "match", name: "Match (5v5)", kind: "mode", description: "Scrims and pick-up games: ready up, knife for sides, MR15 with half-time" },
  { id: "quakesounds", name: "Quake sounds", kind: "addon", description: "Killing spree and headshot announcers" },
];
const SORTS: [string, string][] = [["name", "A to Z"], ["installed", "Installed first"], ["kind", "Modes first"], ["bots", "Best with bots"]];
const FILTERS: [string, string][] = [["all", "All"], ["mode", "Game modes"], ["addon", "Add-ons"], ["installed", "Installed"]];
const RANK = { great: 0, ok: 1, weak: 2, none: 3 } as const;
/** Add-ons switched on and off from Sounds rather than Add-ons (one switch each). */
const ON_SOUNDS = ["quakesounds", "roundmusic"];
const stored = (k: string, d: string) => {
  try {
    return localStorage.getItem(k) || d;
  } catch {
    return d;
  }
};
const store = (k: string, v: string) => {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* private window */
  }
};

/** Install or remove game modes and add-ons one at a time, on the game or server being managed. */
export function Mods({ app, scan, running, onChanged: onChangedProp, openSetup }: { app: AppInfo | null; scan: Scan | null; running: boolean; onChanged: () => void; openSetup: () => void }) {
  const onChanged = useStable(onChangedProp);
  const ask = useConfirm();
  const toast = useToast();
  const go = useNav((s) => s.go);
  // the setup job while one runs (the action itself waits for the end)
  const jobQ = useSetupJob();
  const job = jobQ.data?.state === "running" ? jobQ.data : null;
  const { run, busy: acting } = useActions();
  // the mod being installed or removed here ("core" for the basics), kept app-wide so it
  // still shows after leaving the page and coming back
  const prefix = `mods:${targetKey()}:change:`;
  const busy = useActivity().find(([k]) => k.startsWith(prefix))?.[0].slice(prefix.length) ?? null;
  const [sort, setSortRaw] = useState(() => stored("ls.modsSort", "name"));
  const [filter, setFilterRaw] = useState(() => stored("ls.modsFilter", "all"));
  const setSort = (v: string) => (setSortRaw(v), store("ls.modsSort", v));
  const setFilter = (v: string) => (setFilterRaw(v), store("ls.modsFilter", v));
  const game = app?.game;

  if (!scan) return <Skeleton />;
  if (!game)
    return (
      <div className="panel">
        <h3>No game yet</h3>
        <p className="muted measure">Loner-Strike couldn't find Counter-Strike 1.6 or Condition Zero on this computer.</p>
        <div className="row">
          <BusyButton className="btn primary" busy={acting("mods:rescan")} label="Look again" busyLabel="Looking…" onClick={() => run("mods:rescan", async () => { await Promise.all([reloadScan(), reloadAppInfo()]); }, "Looking for the game…")} />
          {ON_THIS_COMPUTER && <button className="btn" onClick={() => go("settings", "game")}>Choose the game folder</button>}
        </div>
      </div>
    );
  const installed = new Set(game.installed ?? []);
  const setUp = installed.has("core");
  type Item = { id: string; name: string; kind: "mode" | "addon"; description?: string; builtin: boolean; on: boolean; rating: "great" | "ok" | "weak" | "none" | null };
  const ratingFor = (kind: string, id: string) => (kind === "mode" ? BOT_RATING[id.replace(/^mode-/, "")] ?? "great" : null);
  const all: Item[] = [
    ...BUILT_IN.map((b) => ({ ...b, builtin: true, on: setUp, rating: ratingFor(b.kind, b.id) })),
    ...scan.catalog
      .filter((c) => !c.games || c.games.includes(game.id))
      .map((c) => ({ id: c.id, name: c.name, kind: c.kind, description: c.description, builtin: false, on: installed.has(c.id), rating: ratingFor(c.kind, c.id) })),
  ];
  const items = all
    .filter((i) => filter === "all" || (filter === "installed" ? i.on : i.kind === filter))
    .sort((a, b) => {
      const byName = a.name.localeCompare(b.name);
      if (sort === "installed") return Number(b.on) - Number(a.on) || byName;
      if (sort === "kind") return (a.kind === b.kind ? 0 : a.kind === "mode" ? -1 : 1) || byName;
      if (sort === "bots") return (a.rating ? RANK[a.rating] : 9) - (b.rating ? RANK[b.rating] : 9) || byName;
      return byName;
    });
  const where = game.dedicated ? "this server" : game.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero";

  const unbuilt = game.unbuilt ?? [];
  const change = async (id: string | null, remove: boolean, name?: string, again = false) => {
    if (busy) return;
    if (remove && !(await ask(id ? `Remove ${name ?? id}? Its files go, and anything it replaced is put back.` : `Remove Loner-Strike from ${where}? Everything it changed is put back.`, { ok: "Remove", danger: true }))) return;
    // again: an installed mod put in once more (its files fresh, its plugins built again)
    const body = { hl: game.hl, games: [game.id], ...(id ? { components: [id] } : remove ? {} : { components: [] }), ...(again ? { repair: true } : {}) };
    const label = id ? `${remove ? "Removing" : again ? "Reinstalling" : "Installing"} ${name ?? id}…` : remove ? `Removing Loner-Strike from ${where}…` : again ? `Repairing Loner-Strike in ${where}…` : "Installing the basics…";
    await run(`${prefix}${id ?? "core"}:${remove ? "remove" : again ? "repair" : "install"}`, async () => {
      const r = await postJson<{ ok?: boolean; error?: string }>(remove ? "/api/setup/uninstall" : "/api/setup/install", body).catch(() => ({ error: "Couldn't reach the app." }));
      if (r.error) return toast(r.error, true);
      const j = await waitForSetupJob();
      const what = id ? name ?? id : remove ? `Loner-Strike from ${where}` : "the basics";
      if (!j) toast("Lost track of it. Reload this page to see if it finished.", true);
      else if (j.state === "done" && !id && again) toast(`Repaired.${running || !game.dedicated ? " Restart the game so it loads." : ""}`);
      else if (j.state === "done") toast(`${remove ? "Removed" : again ? "Reinstalled" : "Installed"} ${what}${running ? ". It changes on the next map." : ""}`);
      else toast(j.error || (remove ? `Couldn't remove ${what}.` : `Couldn't install ${what}.`), true);
      onChanged();
    }, label);
  };

  const now = job?.steps.find((s) => s.state === "running");
  // where an installed add-on is switched on and off (your own game only: a server has no Add-ons page)
  const own = !game.dedicated && !currentTarget();
  const switchAt = (c: { id: string; kind: string; on: boolean }) => {
    if (!own || !c.on || c.kind !== "addon") return null;
    const sounds = ON_SOUNDS.includes(c.id.replace(/^addon-/, ""));
    return (
      <>
        <span className="muted small-note">Turn it on or off in {sounds ? "Sounds" : "Add-ons"}</span>
        <button className="btn" onClick={() => go(sounds ? "sounds" : "addons")}>{sounds ? "Open Sounds" : "Open Add-ons"}</button>
      </>
    );
  };
  return (
    <div className="stack">
      <SectionHead title="Mods" note={`Game modes and add-ons on ${where}. Everything is backed up, so removing something puts the original files back.`}>
        {ON_THIS_COMPUTER && !game.dedicated && (
          <>
            <button className="btn" onClick={openSetup}>Run full setup</button>
            <Info text="Set up another game, or add or remove many modes at once. Nothing already set up is lost." />
          </>
        )}
      </SectionHead>
      {/* installing copies files on the computer: a phone looks, it doesn't change */}
      {!ON_THIS_COMPUTER && <p className="muted">Install and remove mods on the computer running Loner-Strike.</p>}

      {busy && (
        <SteamProgress
          value={job?.progress ?? 0}
          title={busy.endsWith(":remove") ? "Removing…" : busy === "core:repair" ? "Repairing…" : busy.endsWith(":repair") ? "Reinstalling…" : "Installing…"}
          text={now ? `${now.label}${now.detail ? ` · ${now.detail}` : ""}` : "Getting ready…"}
        />
      )}
      {running && !game.dedicated && !busy && <div className="banner"><div>The game is running. Changes load on the next map; restart the game if something doesn't show up.</div></div>}

      {!setUp ? (
        <div className="panel">
          <h3>Loner-Strike isn't on {where} yet</h3>
          <p className="muted measure">The basics come first: Metamod-P, AMX Mod X, ReGameDLL and the Game Mode Menu, with the built-in modes. Then add whatever you like below.</p>
          {ON_THIS_COMPUTER && (
            <div className="row">
              <BusyButton className="btn primary" busy={busy === "core:install"} disabled={!!busy} label="Install the basics" busyLabel="Installing…" onClick={() => change(null, false)} />
            </div>
          )}
        </div>
      ) : (
        <>
        <div className="toolbar mods-toolbar">
          <div className="chips" role="group" aria-label="Show">
            {FILTERS.map(([id, label]) => <button key={id} className="chip" aria-pressed={filter === id} onClick={() => setFilter(id)}>{label}</button>)}
            <Info text="A game mode changes how the whole game plays, and you switch between modes. An add-on, like the grappling hook, works on top of whichever mode is on." />
            {sort === "bots" && <Info text={BOT_RATING_INFO} />}
          </div>
          <label className="sort">
            <span>Sort</span>
            <select value={sort} onChange={(e) => setSort(e.target.value)}>
              {SORTS.map(([id, label]) => <option key={id} value={id}>{label}</option>)}
            </select>
          </label>
        </div>
        <div className="mod-list">
          {items.map((c) => {
            const { on, rating } = c;
            return (
              <div key={c.id} className={`mod${on ? " on" : ""}`}>
                <Icon name={c.id.replace(/^(mode|addon)-/, "")} size={22} />
                <span className="mod-name">{c.name}<small>{c.kind === "addon" ? "Add-on" : "Game mode"}{!c.builtin && on ? " · installed" : ""}</small></span>
                {c.description && <span className="mod-desc">{c.description}</span>}
                <div className="row">
                  {c.builtin ? (
                    <span className="muted small-note">Comes with Loner-Strike</span>
                  ) : !ON_THIS_COMPUTER ? (
                    on && <span className="muted small-note">Installed</span>
                  ) : on ? (
                    <>
                      {unbuilt.includes(c.id) && <span className="err-text small-note">Didn't build, so it won't load</span>}
                      <BusyButton className={`btn${unbuilt.includes(c.id) ? " primary" : ""}`} busy={busy === `${c.id}:repair`} disabled={!!busy} label={unbuilt.includes(c.id) ? "Try again" : "Reinstall"} busyLabel={unbuilt.includes(c.id) ? "Trying again…" : "Reinstalling…"} onClick={() => change(c.id, false, c.name, true)} />
                      <BusyButton className="btn danger" busy={busy === `${c.id}:remove`} disabled={!!busy} label="Remove" busyLabel="Removing…" onClick={() => change(c.id, true, c.name)} />
                    </>
                  ) : (
                    <BusyButton className="btn primary" busy={busy === `${c.id}:install`} disabled={!!busy} label="Install" busyLabel="Installing…" onClick={() => change(c.id, false, c.name)} />
                  )}
                  {switchAt(c)}
                  {rating && <span className={`bots-note ${rating}`} title={BOT_RATING_INFO}>{BOT_NOTE[rating]}</span>}
                </div>
              </div>
            );
          })}
          {!items.length && <p className="muted">Nothing matches this filter.</p>}
        </div>
        </>
      )}

      {setUp && <OwnPlugins />}

      {/* the basics put in again where they are (a Steam update, a file check or an antivirus can undo
          them): settings, presets and add-ons stay */}
      {setUp && ON_THIS_COMPUTER && (
        <div className="panel row between">
          <div>
            <b>Repair Loner-Strike</b>
            <p className="muted">No bots, or the app can't connect? This puts Metamod, AMX Mod X, the bot brain and the menu back in {where}. Your settings and mods stay.</p>
          </div>
          <BusyButton className="btn" busy={busy === "core:repair"} disabled={!!busy} label="Repair" busyLabel="Repairing…" onClick={() => change(null, false, undefined, true)} />
        </div>
      )}

      {/* your own game: one place to remove it all, in Settings; a server keeps its own button here */}
      {setUp && ON_THIS_COMPUTER && (
        <div className="panel row between">
          <p className="muted">Take Loner-Strike off {where} completely: every mod, the Game Mode Menu and the bot setup.</p>
          {game.dedicated ? (
            <BusyButton className="btn danger" busy={busy === "core:remove"} disabled={!!busy} label="Remove Loner-Strike" busyLabel="Removing…" onClick={() => change(null, true)} />
          ) : (
            <button className="link inline" onClick={() => go("settings", "setup")}>Remove it in Settings</button>
          )}
        </div>
      )}
    </div>
  );
}

/** AMX Mod X plugins the player adds themselves: a .amxx, or a .sma compiled here. */
function OwnPlugins() {
  const toast = useToast();
  const ask = useConfirm();
  const [list, setList] = useState<string[] | null>(null);
  const [failed, setFailed] = useState(false);
  const { run, busy } = useActions();
  const key = (what: string) => `mods:${targetKey()}:plugin:${what}`;
  const load = () => {
    setFailed(false);
    getJson<{ plugins: string[] }>("/api/mods/plugins").then((r) => setList(r.plugins ?? [])).catch((e) => !isAbort(e) && setFailed(true));
  };
  useEffect(load, []);
  type R = { ok?: boolean; error?: string; cancelled?: boolean; name?: string; plugins?: string[] };
  const add = () => run(key("add"), async () => {
    const r = await postJson<R>("/api/mods/plugins/add").catch((): R => ({ error: "Couldn't reach the app." }));
    if (r.cancelled) return;
    if (r.error) return toast(r.error, true);
    setList(r.plugins ?? []);
    toast(`Added ${r.name}. It loads on the next map.`);
  }, "Adding a plugin…");
  const remove = async (name: string) => {
    if (!(await ask(`Remove ${name}? Its files are deleted and it stops loading on the next map.`, { ok: "Remove", danger: true }))) return;
    await run(key(`remove:${name}`), async () => {
      const r = await postJson<R>("/api/mods/plugins/remove", { name }).catch((): R => ({ error: "Couldn't reach the app." }));
      if (r.error) return toast(r.error, true);
      setList(r.plugins ?? []);
      toast(`Removed ${name}`);
    }, `Removing ${name}…`);
  };
  return (
    <section>
      <SectionHead title="Your own plugins" note="AMX Mod X plugins you have as a file. They load in every mode. A .sma source is compiled for you.">
        {ON_THIS_COMPUTER && <BusyButton busy={busy(key("add"))} label="Add a plugin" busyLabel="Adding…" onClick={add} />}
      </SectionHead>
      {list === null && failed ? (
        <LoadError text="Couldn't load your plugins." onRetry={load} />
      ) : list === null ? (
        <Skeleton rows={2} />
      ) : list.length ? (
        <ul className="people">
          {list.map((n) => (
            <li key={n}>
              <div className="person">
                <span className="person-name">{n}</span>
                <span className="person-sub mono">addons/amxmodx/plugins/{n}.amxx</span>
              </div>
              {ON_THIS_COMPUTER && <BusyButton className="btn danger" busy={busy(key(`remove:${n}`))} label="Remove" busyLabel="Removing…" onClick={() => remove(n)} />}
            </li>
          ))}
        </ul>
      ) : (
        <p className="muted">None yet.{ON_THIS_COMPUTER ? "" : " Add them from the computer running Loner-Strike."}</p>
      )}
    </section>
  );
}
