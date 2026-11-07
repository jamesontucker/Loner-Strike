import { QueryClientProvider } from "@tanstack/react-query";
import { Suspense, useEffect, useRef, useState } from "react";
import { clock, isSignedOut, ON_THIS_COMPUTER, postJson, SETUP_ON_COMPUTER, SIGNED_OUT, withTarget, useActions, useActivity } from "./api";
import { gameTitle } from "./games";
import { Icon } from "./icons";
import { forgetLive, queryClient, reloadAppInfo, useOpenReports, reloadMaps, reloadScan, reloadServers, useAppInfo, useGameState, useMaps, useScan, useServers } from "./queries";
import { hasUnsavedEdits, isPageEntry, useNav, type Section } from "./store";
import { usePlayDrafts } from "./stores/playDraft";
import type { AppInfo, Online } from "./types";
import { applyScale, closeSheetsAbove, ConfirmProvider, lazyPage, LoadError, PageBoundary, savedScale, Sheet, Skeleton, ToastProvider, useConfirm, useToast } from "./ui";
import { FilesUpdate } from "./views/FilesUpdate";
import { Addons } from "./views/Addons";
import { AppSettings, noteUpdateChecked } from "./views/AppSettings";
import { Bots } from "./views/Bots";
import { Friends } from "./views/Friends";
import { Maps } from "./views/Maps";
import { Mods } from "./views/Mods";
import { PlayIdle, PlayLive } from "./views/Play";
import { Players } from "./views/Players";
import { QuickBar } from "./views/QuickBar";
import { Sounds } from "./views/Sounds";
import { MatchButtons, SettingsPage } from "./views/Settings";
import { Welcome } from "./views/Welcome";
import { anySetUp, Setup } from "./views/Setup";

// The bigger pages load the first time they're opened, so the app starts on a smaller bundle
const Servers = lazyPage(() => import("./views/Servers").then((m) => ({ default: m.Servers })));
const ServerPage = lazyPage(() => import("./views/Servers").then((m) => ({ default: m.ServerPage })));
const Community = lazyPage(() => import("./views/Community").then((m) => ({ default: m.Community })));
const ServerExtras = lazyPage(() => import("./views/ServerExtras").then((m) => ({ default: m.ServerExtras })));
const Admins = lazyPage(() => import("./views/Admins").then((m) => ({ default: m.Admins })));
const GetMore = lazyPage(() => import("./views/GetMore").then((m) => ({ default: m.GetMore })));

const NAV: { id: Section; label: string; icon: string; group?: string; live?: boolean }[] = [
  { id: "play", label: "Play", icon: "play" },
  { id: "maps", label: "Maps", icon: "maps" },
  { id: "bots", label: "Bots", icon: "bots", live: true },
  { id: "match", label: "Rules", icon: "match", live: true },
  { id: "fun", label: "Fun", icon: "fun", live: true },
  { id: "players", label: "Players", icon: "players", live: true },
  { id: "addons", label: "Add-ons", icon: "addons", live: true },
  { id: "friends", label: "Play with friends", icon: "friends", group: "Online" },
  { id: "admins", label: "Admins", icon: "admins" },
  { id: "community", label: "Stats & moderation", icon: "stats" },
  { id: "welcome", label: "Welcome page", icon: "welcome" },
  { id: "extras", label: "Rotation & rules", icon: "extras" },
  { id: "more", label: "Get more", icon: "more", group: "Library" },
  { id: "mods", label: "Mods", icon: "mods" },
  { id: "sounds", label: "Sounds", icon: "sound" },
  { id: "servers", label: "Servers", icon: "server", group: "Hosting" },
  // (the sidebar keeps it below the list, always in view)
  { id: "settings", label: "Settings", icon: "settings", group: "App" },
];

applyScale(savedScale());

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <ToastProvider>
        <ConfirmProvider>
          <Shell />
        </ConfirmProvider>
      </ToastProvider>
    </QueryClientProvider>
  );
}

function Shell() {
  const { state, reconnecting, lost, refresh } = useGameState();
  const { section, server: openServer, moreOpen, setupOpen, setupIntro, setupLater, go, openServer: enterServer, setMoreOpen, setSetupOpen, laterSetup } = useNav();
  const appQ = useAppInfo();
  const mapsQ = useMaps();
  const scanQ = useScan();
  const serversQ = useServers();
  const app = appQ.data ?? null;
  const maps = mapsQ.data ?? [];
  const scan = scanQ.data ?? null;
  const servers = serversQ.data ?? null;
  const toast = useToast();
  const ask = useConfirm();
  const answered = useRef(false);
  // setup copies files on the computer; from a phone it can only be pointed at
  const openSetup = () => (ON_THIS_COMPUTER ? setSetupOpen(true) : toast(SETUP_ON_COMPUTER));
  const { run: once, busy } = useActions();
  /** Goes to a section, asking first when a page has edits that aren't saved. */
  const nav = async (s: Section, focus?: string) => {
    // a dialog is open (the app behind it is inert): shortcuts and the menu wait
    if (document.getElementById("root")?.hasAttribute("inert")) return;
    if (hasUnsavedEdits() && !(await ask("Leave without saving your changes?", { ok: "Leave", danger: true }))) return;
    go(s, focus);
  };

  const reloadApp = () => void reloadAppInfo();
  const refreshMaps = () => void reloadMaps();
  const refreshServers = () => void reloadServers();
  const refreshAll = () => {
    reloadAppInfo();
    reloadMaps();
    refresh();
  };

  /** Switches your game: CS 1.6 or Condition Zero, in a given Steam library (hl) or the
   *  one already in use. The one way both the sidebar and Settings do it. */
  const switchGame = (hl: string | null, game: string) =>
    once(`app:game:${hl ?? ""}:${game}`, async () => {
      if (hasUnsavedEdits() && !(await ask("Switch game without saving your changes?", { ok: "Switch", danger: true }))) return;
      const r = hl
        ? await postJson<{ ok?: boolean; error?: string }>("/api/app/game", { hl, game })
        : await postJson<{ ok?: boolean; error?: string }>("/api/servers/select", { id: `local:${game}` });
      if (r.error) return toast(r.error, true);
      // a start sent to the other game isn't this one's, and neither is its live state
      usePlayDrafts.getState().endLaunch("local");
      forgetLive();
      await queryClient.invalidateQueries();
      reloadMaps();
    }, `Switching to ${gameTitle(game)}…`);

  const current = openServer ? servers?.servers.find((x) => x.id === openServer) ?? null : null;
  // the open server was deleted (or its folder went away): back to your own game
  useEffect(() => {
    if (openServer && servers && !current) enterServer(null);
  }, [openServer, servers, current, enterServer]);
  // CS 1.6 and Condition Zero on this computer, CS 1.6 first
  const localGames = (["cstrike", "czero"] as const).filter((g) => scan?.installs.some((i) => i.games.some((x) => x.id === g)));
  const myGame = openServer ? null : app?.game?.id;

  // the macOS menu bar: "section:<id>" or "updates"
  useEffect(() => {
    const onMenu = async (e: Event) => {
      const what = (e as CustomEvent<string>).detail;
      if (what.startsWith("section:")) {
        const id = what.slice(8) as Section;
        if (NAV.some((n) => n.id === id)) nav(id);
      } else if (what === "updates") {
        await nav("settings", "updates");
        // same key as Settings' Check now button, so it shows there too
        await once("app:update:check", async () => {
          const r = await postJson<{ available?: boolean; version?: string; error?: string }>("/api/app/update/check").catch((): { available?: boolean; version?: string; error?: string } => ({ error: "Couldn't check for updates." }));
          noteUpdateChecked();
          if (r.error) toast(r.error, true);
          else toast(r.available ? `Loner-Strike ${r.version} is out. Install it from Settings.` : "You're on the latest version.");
          reloadAppInfo();
        }, "Checking for updates…");
      }
    };
    window.addEventListener("ls-menu", onMenu);
    return () => window.removeEventListener("ls-menu", onMenu);
  });

  // Back (the phone's, the browser's, a swipe): the top sheet closes first, then the
  // server's page, then the page before. Each page and sheet has its own history entry
  // (store.ts, ui.tsx), so this only has to follow the entry Back landed on.
  useEffect(() => {
    // while asking about unsaved edits, the question's own history steps are ignored
    let asking = false;
    const onPop = async (e: PopStateEvent) => {
      if (asking) return;
      // sheets opened after the entry Back landed on close (that's all a Back over a
      // sheet does, and a sheet taking its own entry off again closes nothing else)
      if (closeSheetsAbove((e.state as { sheet?: number } | null)?.sheet ?? 0)) return;
      const to = e.state;
      if (!isPageEntry(to)) return;
      const now = useNav.getState();
      if (to.section === now.section && to.server === now.server) return;
      if (hasUnsavedEdits()) {
        asking = true;
        const leave = await ask("Leave without saving your changes?", { ok: "Leave", danger: true });
        // let the question's sheet finish taking its entry off before deciding
        await new Promise((r) => setTimeout(r, 50));
        asking = false;
        if (!leave) {
          // stay: this page goes back on top of the history
          history.pushState({ ls: 1, section: now.section, server: now.server }, "");
          return;
        }
        history.replaceState({ ls: 1, section: to.section, server: to.server }, "");
      }
      useNav.getState().restore(to.section, to.server);
    };
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, [ask]);

  // a phone opened from the QR code: the code isn't left in the address bar (the app keeps
  // the phone signed in with a cookie)
  useEffect(() => {
    if (!/[?&]t=/.test(location.search)) return;
    const params = new URLSearchParams(location.search);
    params.delete("t");
    const rest = params.toString();
    try {
      history.replaceState(history.state, "", `${location.pathname}${rest ? `?${rest}` : ""}${location.hash}`);
    } catch {
      /* leave it */
    }
  }, []);

  // ⌘1-⌘9 switch sections (in sidebar order), ⌘, opens Settings
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey) return;
      if (e.key === ",") {
        e.preventDefault();
        nav("settings");
        return;
      }
      const n = Number(e.key);
      if (n >= 1 && n <= 9 && NAV[n - 1]) {
        e.preventDefault();
        nav(NAV[n - 1].id);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  const online = state?.online ? (state as Online) : null;
  // re-scan maps when the bots finish learning one
  const navKey = online ? `${online.state.map}:${online.state.nav}` : "";
  useEffect(() => {
    if (navKey) reloadMaps();
  }, [navKey]);

  const players = openServer ? 0 : online?.players.length ?? 0;
  // things to look at, shown as a small count on their section
  const reports = useOpenReports().data ?? 0;
  const attention: Partial<Record<Section, { n: number; why: string }>> = {
    servers: {
      n: (servers?.servers ?? []).filter((x) => x.unreachable || x.state === "crashed" || x.state === "failed" || (x.state === "running" && x.pending_restart)).length,
      why: "to look at: servers not answering, stopped by a crash, not finished setting up or waiting for a restart",
    },
    mods: { n: app?.game?.outdated?.length ?? 0, why: "to update: parts of Loner-Strike with updates waiting" },
    community: { n: reports, why: "to read: player reports nobody has looked at yet" },
    settings: { n: app?.update ? 1 : 0, why: "update for Loner-Strike" },
  };
  const pill = (id: Section, cls = "attn") => {
    const a = attention[id];
    return a && a.n > 0 ? <span className={cls} title={`${a.n} ${a.why}`} aria-label={`${a.n} ${a.why}`}>{a.n}</span> : null;
  };
  const tabbar = tabbarFor(!!servers?.servers.length);

  // until the app has answered once: one splash, not a flash of "No game found" (later,
  // opening a server whose info isn't cached yet keeps the app on screen)
  if (appQ.data || appQ.isError) answered.current = true;
  if (!scanQ.data || !answered.current) {
    if (scanQ.isError) {
      return (
        <div className="splash">
          <LoadError
            title={isSignedOut(scanQ.error) ? "Not signed in" : "Loner-Strike isn't answering"}
            text={isSignedOut(scanQ.error) ? SIGNED_OUT : ON_THIS_COMPUTER ? "The app's own server didn't start. Restart Loner-Strike." : "Can't reach your computer. Check it's on and on the same Wi-Fi as this phone."}
            onRetry={() => queryClient.invalidateQueries()}
          />
        </div>
      );
    }
    return (
      <div className="splash" aria-busy="true">
        <span className="spinner" aria-hidden="true" /> Loading…
      </div>
    );
  }

  // First run (nothing set up yet), or opened from Settings
  if ((setupOpen && ON_THIS_COMPUTER) || (!anySetUp(scan) && !setupLater)) {
    // setup copies files on the computer: a phone can only wait for it
    if (!ON_THIS_COMPUTER) {
      return (
        <div className="splash">
          <div className="waiting">
            <h2>Finish setup on your computer</h2>
            <p className="muted">Loner-Strike isn't set up yet. Open it on the computer the game is on, set it up there, and this page fills in.</p>
            <div className="row">
              <button className="btn primary" onClick={() => reloadScan()}>Check again</button>
              <button className="btn" onClick={laterSetup}>Use it anyway</button>
            </div>
          </div>
        </div>
      );
    }
    const done = () => {
      // a first run lands on Play, ready to start a game; run again from Settings, it
      // goes back there
      const fromSettings = setupOpen;
      setSetupOpen(false);
      reloadScan();
      refreshAll();
      if (!fromSettings) go("play");
    };
    return <Setup scan={scan!} fromSettings={setupOpen && anySetUp(scan) && !setupIntro} onClose={laterSetup} onDone={done} onRescan={() => reloadScan()} />;
  }
  const isLive = (id: Section) => !!NAV.find((n) => n.id === id)?.live;
  const needsGame = section !== "servers" && isLive(section) && !online;
  // the game is open here but isn't hosting a map (its main menu, or someone else's server)
  const gameOpen = !!state && !state.online && !!state.game_open;
  const gameName = gameTitle(app?.game?.id);
  const navButton = (n: (typeof NAV)[number], cls = "nav-item", sub = false) => (
    <button
      key={n.id}
      className={`${cls}${n.live && !online ? " needs-game" : ""}`}
      aria-current={section === n.id ? "page" : undefined}
      title={n.live && !online ? "Needs a game you're hosting" : undefined}
      onClick={() => nav(n.id)}
    >
      <Icon name={n.icon} />
      {n.label}
      {/* the title above doesn't show on touch: the More sheet says it */}
      {sub && n.live && !online && <small className="nav-sub">Needs a game</small>}
      {n.id === "players" && players > 0 && <span className="count">{players}</span>}
      {pill(n.id)}
    </button>
  );

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="drag-strip" data-tauri-drag-region />
        <div className="brand">Loner-Strike</div>
        {localGames.length > 1 && (
          <div className="game-switch" role="group" aria-label="Your game">
            {localGames.map((g) => (
              <button key={g} title={g === "cstrike" ? "Counter-Strike 1.6" : "Counter-Strike: Condition Zero"} aria-pressed={myGame === g} disabled={!!openServer || busy(`app:game::${g}`)} onClick={() => myGame !== g && switchGame(null, g)}>
                {g === "cstrike" ? "CS 1.6" : "CZ"}
              </button>
            ))}
          </div>
        )}
        <nav className="nav" aria-label="Sections">
          {NAV.filter((n) => n.id !== "settings").map((n) => (
            <div key={n.id}>
              {n.group && <div className="nav-group">{n.group}</div>}
              {navButton(n)}
            </div>
          ))}
        </nav>
        {/* Settings stays in view however short the window is */}
        <div className="nav-pinned">{navButton(NAV.find((n) => n.id === "settings")!)}</div>
        <div className="side-foot">
          {current ? `Server: ${current.name}` : app?.game ? gameTitle(app.game.id) : appQ.isError ? "Couldn't load your game" : "No game found"}
          {app?.version && <span> · v{app.version}</span>}
          <div className="made-by">
            by <a href="https://x.com/nanoshrine" target="_blank" rel="noreferrer">nanoshrine</a>
          </div>
        </div>
      </aside>

      <div className="main">
        <Strip data={online} reconnecting={reconnecting} lost={lost} reason={state && !state.online ? state.reason : ""} app={app} server={current?.name} gameOpen={gameOpen} />
        {online && <QuickBar data={online} maps={maps} refresh={refresh} reloadMaps={refreshMaps} />}
        <main className="content">
          {/* the app stopped answering: said once, at the top, with a way to try again */}
          {lost && state && !state.online && <LoadError title={state.reason === SIGNED_OUT ? "Not signed in" : "Can't reach Loner-Strike"} text={state.reason} onRetry={() => queryClient.invalidateQueries()} />}
          <HomeScreenHint />
          {!setupOpen && <FilesUpdate key={openServer ?? "local"} app={app} scan={scan} gameOpen={gameOpen || !!online} reloadApp={reloadApp} />}
          {state === null ? (
            <Skeleton rows={4} />
          ) : needsGame && !lost ? (
            <Waiting label={NAV.find((n) => n.id === section)!.label} game={gameName} gameOpen={gameOpen} reason={state.online ? "" : state.reason} onPlay={() => go("play")} setUp={!app?.game || !!app.game.installed?.includes("core")} openSetup={openSetup} />
          ) : (
            <PageBoundary key={`${section}:${openServer ?? ""}:${app?.game?.id ?? ""}`}>
            <Suspense fallback={<Skeleton rows={4} />}>
              {section === "play" &&
                (online ? (
                  <PlayLive data={online} maps={maps} refresh={refresh} openMaps={() => go("maps")} />
                ) : (
                  <PlayIdle app={app} reason={state.online ? "" : state.reason} gameOpen={gameOpen} maps={maps} openSetup={openSetup} />
                ))}
              {section === "maps" && <Maps current={online?.state.map} live={!!online} maps={maps} loading={mapsQ.isLoading} reload={refreshMaps} refresh={refresh} />}
              {section === "bots" && online && <Bots data={online} refresh={refresh} />}
              {section === "match" && online && (
                <SettingsPage title="Rules" note={`Saved for ${online.modes.find((m) => m.id === online.state.mode)?.name ?? "this mode"}. Round changes take effect next round.`} data={online} cat={2} refresh={refresh}>
                  <MatchButtons refresh={refresh} />
                </SettingsPage>
              )}
              {section === "fun" && online && <SettingsPage title="Fun & effects" note="These apply immediately." data={online} cat={3} refresh={refresh} />}
              {section === "players" && online && <Players data={online} refresh={refresh} />}
              {section === "addons" && online && <Addons data={online} refresh={refresh} />}
              {section === "friends" && <Friends data={online} refresh={refresh} />}
              {section === "servers" &&
                (current ? (
                  <ServerPage
                    key={current.id}
                    server={current}
                    others={(servers?.servers ?? []).filter((o) => o.id !== current.id)}
                    data={online}
                    reason={state.online ? "" : state.reason}
                    app={app}
                    scan={scan}
                    maps={maps}
                    refresh={refresh}
                    reloadMaps={refreshMaps}
                    reloadApp={reloadApp}
                    onChanged={refreshServers}
                    onBack={() => enterServer(null)}
                  />
                ) : (
                  <Servers scan={scan} list={servers} loadError={serversQ.isError && !servers} onOpen={enterServer} onChanged={refreshServers} />
                ))}
              {section === "admins" && <Admins data={online} installed={app?.game?.installed} />}
              {section === "welcome" && <Welcome target="local" />}
              {section === "sounds" && <Sounds target="local" />}
              {section === "extras" && <ServerExtras target="local" />}
              {section === "community" && <Community target="local" />}
              {section === "more" && <GetMore onInstalled={refreshMaps} />}
              {section === "mods" && <Mods app={app} scan={scan} running={!!online} onChanged={() => { reloadApp(); reloadScan(); }} openSetup={openSetup} />}
              {section === "settings" && <AppSettings app={app} reloadApp={reloadApp} scan={scan} openSetup={openSetup} switchGame={switchGame} />}
            </Suspense>
            </PageBoundary>
          )}
        </main>
      </div>

      {/* phones: the most used sections, the rest under More */}
      <nav className="tabbar" aria-label="Sections">
        {tabbar.map((id) => {
          const n = NAV.find((x) => x.id === id)!;
          return (
            <button key={id} aria-current={section === id && !moreOpen ? "page" : undefined} onClick={() => nav(id)}>
              <Icon name={n.icon} size={22} />
              <span>{n.label}</span>
              {id === "players" && players > 0 && <i className="tab-count">{players}</i>}
              {pill(id, "tab-count attn-tab")}
            </button>
          );
        })}
        <button aria-current={moreOpen || !tabbar.includes(section) ? "page" : undefined} aria-expanded={moreOpen} onClick={() => setMoreOpen(!moreOpen)}>
          <Icon name="menu" size={22} />
          <span>More</span>
          {/* what needs a look in the sections under More, added up */}
          {(() => {
            const n = NAV.filter((x) => !tabbar.includes(x.id)).reduce((t, x) => t + (attention[x.id]?.n ?? 0), 0);
            return n > 0 ? <i className="tab-count attn-tab" aria-label={`${n} to look at`}>{n}</i> : null;
          })()}
        </button>
      </nav>
      {moreOpen && (
        <Sheet label="More sections" className="more-sheet" bottom keepApp onClose={() => setMoreOpen(false)}>
          {NAV.filter((n) => !tabbar.includes(n.id)).map((n, i, list) => (
            <div key={n.id}>
              {/* the sidebar's groups, so 13 items aren't one long list */}
              {(i === 0 || groupOf(n.id) !== groupOf(list[i - 1].id)) && groupOf(n.id) && <div className="nav-group">{groupOf(n.id)}</div>}
              {navButton(n, "nav-item", true)}
            </div>
          ))}
        </Sheet>
      )}
    </div>
  );
}

/** The sidebar group a section is listed under ("" for the first, untitled one). */
function groupOf(id: Section) {
  let g = "";
  for (const n of NAV) {
    if (n.group) g = n.group;
    if (n.id === id) return g;
  }
  return g;
}

/** The phone's bottom bar: the most used sections, and Servers for people who host. */
const tabbarFor = (hosting: boolean): Section[] => (hosting ? ["play", "maps", "players", "servers"] : ["play", "maps", "bots", "players"]);

/** What's in progress anywhere in the app, so it's visible from any page. `phone`: the
 *  slim line along the bottom of the top bar on phones (the bar itself has no room for it
 *  there); only one of the two shows, by screen size. */
function Working({ phone }: { phone?: boolean }) {
  const acts = useActivity().filter(([, label]) => label);
  if (!acts.length) return null;
  return (
    <span className={phone ? "working working-phone" : "working"} role={phone ? undefined : "status"}>
      <span className="spinner" aria-hidden="true" />
      <span className="working-label">
        {acts[0][1]}
        {acts.length > 1 ? ` (+${acts.length - 1} more)` : ""}
      </span>
    </span>
  );
}

function Strip({ data, reconnecting, lost, reason, app, server, gameOpen }: { data: Online | null; reconnecting: boolean; lost: boolean; reason: string; app: AppInfo | null; server?: string; gameOpen: boolean }) {
  if (!data) {
    return (
      <header className="strip" data-tauri-drag-region>
        <div className="strip-meta idle" data-tauri-drag-region>
          {lost
            ? reason
            : server
              ? `${server} isn't running`
              : app?.game
                ? `${gameTitle(app.game.id)} ${gameOpen ? "is open, but no game is loaded" : "isn't running"}`
                : reason}
        </div>
        <div className="strip-actions">
          <Working />
          <span className={`live${lost ? " off-line" : ""}`}>{lost ? "Can't reach it" : "Offline"}</span>
        </div>
        <Working phone />
      </header>
    );
  }
  const st = data.state;
  const score = (team: string) => data.players.filter((p) => p.team === team).reduce((n, p) => n + p.frags, 0);
  const mode = data.modes.find((m) => m.id === st.mode)?.name ?? st.mode;
  return (
    <header className="strip" data-tauri-drag-region>
      <div className="strip-map">
        <div className="thumb" style={{ backgroundImage: `url(${withTarget(`/thumb/${encodeURIComponent(st.map)}.png`)})` }} aria-hidden="true" />
        <div>
          <div className="map-name">{st.map}</div>
          <div className="strip-meta">
            {mode} · {st.humans + st.bots}/{st.maxplayers} players{st.timelimit > 0 && st.timeleft >= 0 ? ` · ${clock(st.timeleft)} left` : ""}
            {st.paused ? " · paused" : ""}
          </div>
        </div>
      </div>
      <div className="score" aria-label="Kills by team">
        <span className="t">{score("T")}</span>
        <span className="vs">:</span>
        <span className="ct">{score("CT")}</span>
      </div>
      <div className="strip-actions">
        <Working />
        <span className={`live${reconnecting ? "" : " on"}`}>{reconnecting ? "Reconnecting…" : "Live"}</span>
      </div>
      <Working phone />
    </header>
  );
}

function Waiting({ label, game, gameOpen, reason, onPlay, setUp, openSetup }: { label: string; game: string; gameOpen: boolean; reason: string; onPlay: () => void; setUp: boolean; openSetup: () => void }) {
  return (
    <div className="waiting">
      <h2>{label}</h2>
      {setUp ? (
        <>
          <p className="muted">
            {gameOpen
              ? `This works in a game you host. ${game} is open, but no map is loaded: start a game from its New Game menu, or quit it and start one from Play.`
              : "This works in a game you host. Start one from Play and this page fills in."}
          </p>
          {reason && !gameOpen && <p className="muted small-note">{reason}</p>}
          {!gameOpen && <button className="btn primary" onClick={onPlay}>Start a game</button>}
        </>
      ) : (
        <>
          <p className="muted">Loner-Strike isn't set up on this game yet, so there's nothing to control.</p>
          {ON_THIS_COMPUTER ? <button className="btn primary" onClick={openSetup}>Set it up</button> : <p className="muted">{SETUP_ON_COMPUTER}</p>}
        </>
      )}
    </div>
  );
}

/** The phone layout (styles.css uses the same): narrow, or a phone on its side. */
const PHONE_QUERY = "(max-width: 760px), (pointer: coarse) and (max-height: 500px)";

/** On a phone in the browser: how to put the panel on the home screen, so it opens like an app. */
function HomeScreenHint() {
  const [gone, setGone] = useState(() => {
    try {
      return localStorage.getItem("ls.homeHint") === "1";
    } catch {
      return false;
    }
  });
  const [narrow, setNarrow] = useState(() => window.matchMedia(PHONE_QUERY).matches);
  useEffect(() => {
    const m = window.matchMedia(PHONE_QUERY);
    const on = () => setNarrow(m.matches);
    m.addEventListener("change", on);
    return () => m.removeEventListener("change", on);
  }, []);
  const standalone = window.matchMedia("(display-mode: standalone)").matches || (navigator as { standalone?: boolean }).standalone === true;
  if (ON_THIS_COMPUTER || standalone || gone || !narrow) return null;
  const ios = /iPhone|iPad|iPod/.test(navigator.userAgent);
  const hide = () => {
    setGone(true);
    try {
      localStorage.setItem("ls.homeHint", "1");
    } catch {
      /* private window */
    }
  };
  return (
    <div className="banner home-hint">
      <div>
        Put Loner-Strike on your home screen: {ios ? <>tap <b>Share</b>, then <b>Add to Home Screen</b>. It opens like an app.</> : <>tap <b>⋮</b>, then <b>Add to Home screen</b>.</>}
      </div>
      <button className="btn" onClick={hide}>Got it</button>
    </div>
  );
}
