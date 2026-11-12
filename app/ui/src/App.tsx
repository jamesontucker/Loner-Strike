import { QueryClientProvider, useQuery } from "@tanstack/react-query";
import { Suspense, useEffect, useRef, useState } from "react";
import { clock, getJsonAt, isSignedOut, ON_THIS_COMPUTER, postJson, SETUP_ON_COMPUTER, SIGNED_OUT, withTarget, useActions, useActivity } from "./api";
import { gameShort, gameTitle } from "./games";
import { Icon } from "./icons";
import { forgetLive, queryClient, reloadAppInfo, useOpenReports, reloadMaps, reloadScan, reloadServers, useAppInfo, useGameState, useMaps, useScan, useServers } from "./queries";
import { hasUnsavedEdits, isPageEntry, useNav, type Section } from "./store";
import { usePlayDrafts } from "./stores/playDraft";
import { waitsForRestart, type AppInfo, type GameState, type Online, type ServerInfo } from "./types";
import { applyScale, closeSheetsAbove, ConfirmProvider, lazyPage, LoadError, PageBoundary, savedScale, Sheet, Skeleton, ToastProvider, useConfirm, useToast } from "./ui";
import { FilesUpdate } from "./views/FilesUpdate";
import { Addons } from "./views/Addons";
import { AppSettings, noteUpdateChecked } from "./views/AppSettings";
import { Bots } from "./views/Bots";
import { SMARTER_CHAT, SmarterChat } from "./views/SmarterChat";
import { Radio, RadioButton } from "./views/Radio";
import { Friends } from "./views/Friends";
import { Maps } from "./views/Maps";
import { Mods } from "./views/Mods";
import { PlayIdle, PlayLive } from "./views/Play";
import { Players } from "./views/Players";
import { QuickBar } from "./views/QuickBar";
import { Sounds } from "./views/Sounds";
import { FUN_NOTE, MatchButtons, RULES_NOTE, SavedSettingsPage, SettingsPage } from "./views/Settings";
import { Welcome } from "./views/Welcome";
import { WhatsNew } from "./views/WhatsNew";
import { anySetUp, Setup } from "./views/Setup";

// The bigger pages load the first time they're opened, so the app starts on a smaller bundle
const Servers = lazyPage(() => import("./views/Servers").then((m) => ({ default: m.Servers })));
const ServerPage = lazyPage(() => import("./views/Servers").then((m) => ({ default: m.ServerPage })));
const Community = lazyPage(() => import("./views/Community").then((m) => ({ default: m.Community })));
const ServerExtras = lazyPage(() => import("./views/ServerExtras").then((m) => ({ default: m.ServerExtras })));
const Admins = lazyPage(() => import("./views/Admins").then((m) => ({ default: m.Admins })));
const ServerBrowser = lazyPage(() => import("./views/ServerBrowser").then((m) => ({ default: m.ServerBrowser })));
const GetMore = lazyPage(() => import("./views/GetMore").then((m) => ({ default: m.GetMore })));
const MySkins = lazyPage(() => import("./views/GetMore").then((m) => ({ default: m.MySkins })));

// Your own game's pages, grouped like a server's tabs (Game, People), then the rest.
// ⌘1-⌘9 follow this order.
const NAV: { id: Section; label: string; icon: string; group?: string; live?: boolean }[] = [
  { id: "play", label: "Play", icon: "play", group: "Game" },
  { id: "bots", label: "Bots", icon: "bots", live: true },
  { id: "match", label: "Rules", icon: "rules", live: true },
  { id: "fun", label: "Fun", icon: "fun", live: true },
  { id: "addons", label: "Add-ons", icon: "addons", live: true },
  { id: "extras", label: "Rotation & extras", icon: "extras" },
  { id: "players", label: "Players", icon: "players", live: true, group: "People" },
  { id: "community", label: "Moderation", icon: "moderation" },
  { id: "stats", label: "Stats", icon: "stats" },
  { id: "admins", label: "Admins", icon: "admins" },
  { id: "welcome", label: "Welcome page", icon: "welcome" },
  { id: "friends", label: "Play with friends", icon: "friends", group: "Online" },
  { id: "browser", label: "Find servers", icon: "browser" },
  { id: "maps", label: "Maps", icon: "maps", group: "My library" },
  { id: "mods", label: "Mods", icon: "mods" },
  { id: "skins", label: "Skins & models", icon: "skins" },
  { id: "sounds", label: "Sounds", icon: "sound" },
  { id: "more", label: "Get more", icon: "more" },
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
          {/* one radio for the whole app, setup included: what's playing carries on from
              setup into the panel (on this computer; phones control the game, not music) */}
          {ON_THIS_COMPUTER && <Radio />}
        </ConfirmProvider>
      </ToastProvider>
    </QueryClientProvider>
  );
}

function Shell() {
  const { state, reconnecting, lost, refresh } = useGameState();
  const { section, server: openServer, moreOpen, setupOpen, setupIntro, setupLater, go, openServer: enterServer, openServerTab, setMoreOpen, setSetupOpen, laterSetup } = useNav();
  const appQ = useAppInfo();
  // the game's own maps for every picker; the Maps page can show the other game's too
  const mapsQ = useMaps();
  const allMaps = useMaps({ other: true }).data ?? [];
  const scanQ = useScan();
  const serversQ = useServers();
  const app = appQ.data ?? null;
  const maps = mapsQ.data ?? [];
  const scan = scanQ.data ?? null;
  const servers = serversQ.data ?? null;
  // Your own game while a server is open: the sidebar's own-game pages and the game
  // switcher show its state, not the server's (the same cache entries as when it's open)
  const localStateQ = useQuery({
    queryKey: ["state", null],
    queryFn: ({ signal }) => getJsonAt<GameState>("/api/state", null, signal),
    refetchInterval: 5000,
    retry: 0,
    enabled: !!openServer,
  });
  const localAppQ = useQuery({ queryKey: ["app", null], queryFn: ({ signal }) => getJsonAt<AppInfo>("/api/app", null, signal), enabled: !!openServer });
  const localState = openServer ? localStateQ.data ?? null : state;
  const localOnline = localState?.online ? (localState as Online) : null;
  const localGameOpen = !!localState && !localState.online && !!localState.game_open;
  const localApp = openServer ? localAppQ.data ?? null : app;
  const toast = useToast();
  const ask = useConfirm();
  const answered = useRef(false);
  // setup copies files on the computer; from a phone it can only be pointed at
  const openSetup = () => (ON_THIS_COMPUTER ? setSetupOpen(true) : toast(SETUP_ON_COMPUTER));
  const { run: once } = useActions();
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
      // (switching while a game is going: the app then controls the other game, and starting
      // one there would close the game being played)
      if ((!!localOnline || localGameOpen) && !(await ask(`Switch to ${gameTitle(game)}? The game that's open keeps going, but the app shows ${gameTitle(game)} until you switch back.`, { ok: "Switch" }))) return;
      const r = hl
        ? await postJson<{ ok?: boolean; error?: string }>("/api/app/game", { hl, game })
        : await postJson<{ ok?: boolean; error?: string }>("/api/servers/select", { id: `local:${game}` });
      if (r.error) return toast(r.error, true);
      // a start sent to the other game isn't this one's, and neither is its live state
      usePlayDrafts.getState().endLaunch("local");
      forgetLive();
      // (lists that belong to one game are dropped, not just refreshed, so the other game's
      // maps or admins never show for a moment while the new ones load)
      for (const k of ["maps", "admins", "reports", "state"]) queryClient.removeQueries({ queryKey: [k] });
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
  const myGame = localApp?.game?.id;
  /** Leaves the open server for your own game (its Play page). */
  const backToMyGame = () => nav("play");
  /** The game switcher: switches your game, or with a server open, goes back to your game
   *  first (then switches, if it was the other game). */
  const pickGame = async (g: string) => {
    if (openServer) {
      await backToMyGame();
      // (stayed for unsaved edits, or your game isn't known yet: just back)
      if (useNav.getState().server || !myGame) return;
    }
    if (myGame !== g) switchGame(null, g);
  };

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
          if (r.error) toast(r.error, true);
          else {
            noteUpdateChecked();
            toast(r.available ? `Loner-Strike ${r.version} is out. Install it from Settings.` : "You're on the latest version.");
          }
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

  // who's in the open game or server (the phone's Players), and in your own game (the sidebar's)
  const players = online?.players.length ?? 0;
  const useActivityKeys = useActivity().map(([k]) => k);
  const myPlayers = localOnline?.players.length ?? 0;
  // things to look at, shown as a small count on their section
  const reports = useOpenReports().data ?? 0;
  const attention: Partial<Record<Section, { n: number; why: string }>> = {
    servers: {
      n: (servers?.servers ?? []).filter((x) =>
        x.unreachable || x.state === "crashed" || x.state === "failed" || (x.state === "running" && waitsForRestart(x))
        // (updates waiting that aren't going in already)
        || (!x.shared && !x.updating && !x.update_queued && (x.outdated?.length ?? 0) > 0 && x.state !== "setting-up")).length,
      why: "to look at: servers with updates waiting, not answering, stopped by a crash, not finished setting up or waiting for a restart",
    },
    mods: { n: localApp?.game?.outdated?.length ?? 0, why: "to update: parts of Loner-Strike with updates waiting" },
    community: { n: reports, why: "to read: player reports nobody has looked at yet" },
    settings: { n: localApp?.update ? 1 : 0, why: "update for Loner-Strike" },
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
  // Bots, Rules, Fun and Add-ons also work without a game: they change the next game's settings
  const worksSaved = (id: Section) => ["bots", "match", "fun", "addons"].includes(id) && !!localApp?.game?.installed?.includes("core");
  const needsGame = section !== "servers" && isLive(section) && !online && !worksSaved(section);
  // the game is open here but isn't hosting a map (its main menu, or someone else's server)
  const gameOpen = !!state && !state.online && !!state.game_open;
  const gameName = gameTitle(app?.game?.id);
  // the sidebar's pages are your own game's, even with a server open: greyed by whether
  // YOUR game is hosting one, not the server
  const navButton = (n: (typeof NAV)[number], cls = "nav-item", sub = false) => (
    <button
      key={n.id}
      className={`${cls}${n.live && !localOnline && !worksSaved(n.id) ? " needs-game" : ""}`}
      aria-current={section === n.id ? "page" : undefined}
      title={n.live && !localOnline && !worksSaved(n.id) ? "Needs a game you're hosting" : undefined}
      onClick={() => nav(n.id)}
    >
      <Icon name={n.icon} />
      {n.label}
      {/* the title above doesn't show on touch: the More sheet says it */}
      {sub && n.live && !localOnline && !worksSaved(n.id) && <small className="nav-sub">Needs a game</small>}
      {n.id === "players" && myPlayers > 0 && <span className="count">{myPlayers}</span>}
      {pill(n.id)}
    </button>
  );
  // your own game's pages (everything above Hosting), and the rest
  const hostingAt = NAV.findIndex((n) => n.id === "servers");
  const myGameName = localApp?.game ? gameTitle(localApp.game.id) : null;

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="drag-strip" data-tauri-drag-region />
        <div className="brand">Loner-Strike</div>
        {localGames.length > 1 && (
          <div className="game-switch" role="group" aria-label="Your game">
            {localGames.map((g) => (
              <button key={g} title={openServer ? `Back to your game${myGame && myGame !== g ? `, then switch to ${gameTitle(g)}` : ""}` : g === "cstrike" ? "Counter-Strike 1.6" : "Counter-Strike: Condition Zero"} aria-pressed={myGame === g} disabled={useActivityKeys.some((k) => k.startsWith("app:game:"))} onClick={() => void pickGame(g)}>
                {g === "cstrike" ? "CS 1.6" : "CZ"}
              </button>
            ))}
          </div>
        )}
        <nav className="nav" aria-label="Sections">
          {/* with a server open, these still go to your own game: said, and a little dimmer */}
          {openServer && (
            <div className="nav-mine-head" title={`These pages are your own game${myGameName ? ` (${myGameName})` : ""}, not ${current?.name ?? "the server"}`}>
              My game{myGameName && <span>{gameShort(localApp?.game?.id)}</span>}
            </div>
          )}
          <div className={openServer ? "nav-mine away" : "nav-mine"}>
            {NAV.slice(0, hostingAt).map((n) => (
              <div key={n.id}>
                {n.group && <div className="nav-group">{n.group}</div>}
                {navButton(n)}
              </div>
            ))}
          </div>
          {NAV.slice(hostingAt).filter((n) => n.id !== "settings").map((n) => (
            <div key={n.id}>
              {n.group && <div className="nav-group">{n.group}</div>}
              {n.id === "servers" ? (
                <ServersNav
                  servers={servers?.servers ?? []}
                  section={section}
                  openId={openServer}
                  pill={pill("servers")}
                  onOverview={() => nav("servers")}
                  onNew={() => nav("servers", "new")}
                  onOpen={async (id) => {
                    // the same checks as the other sections, then straight to that server's page
                    if (document.getElementById("root")?.hasAttribute("inert")) return;
                    if (hasUnsavedEdits() && !(await ask("Leave without saving your changes?", { ok: "Leave", danger: true }))) return;
                    if (section !== "servers") go("servers");
                    enterServer(id);
                  }}
                />
              ) : (
                navButton(n)
              )}
            </div>
          ))}
        </nav>
        {/* Settings stays in view however short the window is */}
        <div className="nav-pinned">{navButton(NAV.find((n) => n.id === "settings")!)}</div>
        <div className="side-foot">
          <div>
            <a href="https://lonerstrike.com" target="_blank" rel="noreferrer">Website</a>
            <span aria-hidden="true"> · </span>
            <a href="https://lonerstrike.com/docs" target="_blank" rel="noreferrer">Docs</a>
            {localApp?.version && <><span aria-hidden="true"> · </span>v{localApp.version}</>}
          </div>
          <div className="made-by">
            Made by <a href="https://nanoshrine.com" target="_blank" rel="noreferrer">nanoshrine</a>
          </div>
        </div>
      </aside>

      <div className="main">
        <Strip data={online} reconnecting={reconnecting} lost={lost} reason={state && !state.online ? state.reason : ""} app={app} server={current?.name} srv={current} gameOpen={gameOpen} />
        <Where server={current} game={myGameName} listing={section === "servers" && !current} onBack={() => void backToMyGame()} />
        {online && <QuickBar data={online} maps={maps} refresh={refresh} reloadMaps={refreshMaps} />}
        <main className="content">
          {/* the app stopped answering: said once, at the top, with a way to try again */}
          {lost && state && !state.online && <LoadError title={state.reason === SIGNED_OUT ? "Not signed in" : "Can't reach Loner-Strike"} text={state.reason} onRetry={() => queryClient.invalidateQueries()} />}
          <HomeScreenHint />
          <WhatsNew version={app?.version} />
          {/* (not on the servers list: it's about your servers, which show their own updates) */}
          {!setupOpen && !current?.shared && !(section === "servers" && !openServer) && <FilesUpdate key={openServer ?? "local"} app={app} scan={scan} gameOpen={gameOpen || !!online} reloadApp={reloadApp} server={current?.name} />}
          {state === null ? (
            <Skeleton rows={4} />
          ) : needsGame && !lost ? (
            <>
              <Waiting label={NAV.find((n) => n.id === section)!.label} game={gameName} gameOpen={gameOpen} reason={state.online ? "" : state.reason} onPlay={() => go("play")} setUp={!app?.game || !!app.game.installed?.includes("core")} openSetup={openSetup} />
              {/* set up and try Smarter chat before a game is running */}
              {SMARTER_CHAT && section === "bots" && !openServer && (
                <SmarterChat />
              )}
            </>
          ) : (
            <PageBoundary key={`${section}:${openServer ?? localApp?.game?.id ?? ""}`}>
            <Suspense fallback={<Skeleton rows={4} />}>
              {section === "play" &&
                (online ? (
                  <PlayLive data={online} maps={maps} refresh={refresh} openMaps={() => go("maps")} />
                ) : (
                  <PlayIdle app={app} reason={state.online ? "" : state.reason} gameOpen={gameOpen} maps={maps} openSetup={openSetup} />
                ))}
              {section === "maps" && <Maps current={online?.state.map} live={!!online} maps={allMaps} loading={mapsQ.isLoading} reload={refreshMaps} refresh={refresh} getMore={() => go("more")} />}
              {section === "bots" && (online || !lost) && <Bots data={online} refresh={refresh} />}
              {section === "match" && !online && !lost && <SavedSettingsPage cat={2} />}
              {section === "match" && online && (
                <SettingsPage title="Rules" note={RULES_NOTE(online)} data={online} cat={2} refresh={refresh} pickMode>
                  <MatchButtons refresh={refresh} data={online} />
                </SettingsPage>
              )}
              {section === "fun" && (online ? <SettingsPage title="Fun & effects" note={FUN_NOTE(online)} data={online} cat={3} refresh={refresh} pickMode /> : !lost && <SavedSettingsPage cat={3} />)}
              {section === "players" && online && <Players data={online} refresh={refresh} />}
              {section === "addons" && (online || !lost) && <Addons data={online} refresh={refresh} />}
              {section === "friends" && <Friends data={online} refresh={refresh} />}
              {section === "browser" && <ServerBrowser />}
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
                    maps={allMaps}
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
              {section === "community" && <Community target="local" part="moderation" />}
              {section === "stats" && <Community target="local" part="stats" />}
              {section === "more" && <GetMore onInstalled={refreshMaps} />}
              {section === "skins" && <MySkins />}
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
            // with a server open, Players is that server's Players tab (the server stays open)
            <button key={id} aria-current={section === id && !moreOpen ? "page" : undefined} onClick={() => (id === "players" && openServer ? openServerTab("players") : nav(id))}>
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
          {current && <p className="more-mine">These open your own game, not {current.name}.</p>}
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

/** How many servers the sidebar lists before "All servers" */
const SIDE_SERVERS = 8;
const SERVERS_FOLD = "ls.serversOpen";

/**
 * Servers in the sidebar: a header that only folds the list (open unless folded; opening a
 * server unfolds it), then Overview (the list page), each server (straight to its page)
 * and New server.
 */
function ServersNav({ servers, section, openId, pill, onOverview, onNew, onOpen }: { servers: ServerInfo[]; section: Section; openId: string | null; pill: React.ReactNode; onOverview: () => void; onNew: () => void; onOpen: (id: string) => void }) {
  const [folded, setFolded] = useState(() => {
    try {
      return localStorage.getItem(SERVERS_FOLD) === "0";
    } catch {
      return false;
    }
  });
  // opening a server (from its page or the list) unfolds it, so the open one shows
  useEffect(() => {
    if (openId) setFolded(false);
  }, [openId]);
  const open = !folded;
  const toggle = () => {
    setFolded(open);
    try {
      localStorage.setItem(SERVERS_FOLD, open ? "0" : "1");
    } catch {
      // storage off: it just isn't remembered
    }
  };
  const here = section === "servers";
  // yours first, then ones friends shared with you
  const own = servers.filter((s) => !s.shared);
  const shared = servers.filter((s) => s.shared);
  const shown = own.slice(0, SIDE_SERVERS);
  const sharedShown = shared.slice(0, Math.max(0, SIDE_SERVERS - shown.length));
  const dot = (s: ServerInfo) => (s.shared && !s.shared.reachable ? "warn" : s.unreachable ? "warn" : s.state === "running" ? "on" : s.state === "failed" || s.state === "crashed" ? "warn" : "");
  const say = (s: ServerInfo) => (s.unreachable ? "can't reach it" : s.state === "running" ? "running" : s.state === "setting-up" ? "setting up" : s.state === "failed" ? "not finished" : s.state === "crashed" ? "crashed" : "stopped");
  return (
    <>
      <button className="nav-item nav-fold" aria-expanded={open} aria-controls="side-servers" onClick={toggle}>
        <Icon name="server" />
        Servers
        {pill}
        <svg className="fold-caret" aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
          <path d={open ? "M6 9l6 6 6-6" : "M9 6l6 6-6 6"} />
        </svg>
      </button>
      {open && (
        <div className="side-servers" id="side-servers">
          <button className="side-server overview" aria-current={here && !openId ? "page" : undefined} onClick={onOverview}>
            <svg aria-hidden="true" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
              <rect x="4" y="4" width="7" height="7" />
              <rect x="13" y="4" width="7" height="7" />
              <rect x="4" y="13" width="7" height="7" />
              <rect x="13" y="13" width="7" height="7" />
            </svg>
            <span className="side-server-name">All servers</span>
          </button>
          {shown.map((s) => (
            <button key={s.id} className="side-server" aria-current={here && openId === s.id ? "page" : undefined} title={`${s.name} · ${say(s)}`} onClick={() => onOpen(s.id)}>
              <span className={`side-dot ${dot(s)}`} aria-hidden="true" />
              <span className="side-server-name">{s.name}</span>
              <span className="sr-only">, {say(s)}</span>
            </button>
          ))}
          {sharedShown.length > 0 && <div className="side-div">Shared</div>}
          {sharedShown.map((s) => (
            <button key={s.id} className="side-server" aria-current={here && openId === s.id ? "page" : undefined} title={`${s.name} · shared with you · ${s.shared!.role === "admin" ? "Admin" : "Moderator"} · ${say(s)}`} onClick={() => onOpen(s.id)}>
              <span className={`side-dot ${dot(s)}`} aria-hidden="true" />
              <span className="side-server-name">{s.name}</span>
              <span className={`side-role role-${s.shared!.role}`} aria-hidden="true">{s.shared!.role === "admin" ? "A" : "M"}</span>
              <span className="sr-only">, shared with you, {s.shared!.role}, {say(s)}</span>
            </button>
          ))}
          {servers.length > shown.length + sharedShown.length && (
            <button className="side-server more" onClick={onOverview}>
              <span className="side-server-name">All servers ({servers.length})</span>
            </button>
          )}
          {ON_THIS_COMPUTER && (
            <button className="side-server add" onClick={onNew}>
              <svg aria-hidden="true" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round">
                <path d="M12 5v14M5 12h14" />
              </svg>
              <span className="side-server-name">New server</span>
            </button>
          )}
        </div>
      )}
    </>
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

function Strip({ data, reconnecting, lost, reason, app, server, srv, gameOpen }: { data: Online | null; reconnecting: boolean; lost: boolean; reason: string; app: AppInfo | null; server?: string; srv?: ServerInfo | null; gameOpen: boolean }) {
  if (!data) {
    return (
      <header className="strip" data-tauri-drag-region>
        <div className="strip-meta idle" data-tauri-drag-region>
          {/* the bar under this names the game or server, and the badge on the right says Not
              running / Not hosting: only what neither says goes here */}
          {lost ? reason : server || app?.game ? "" : reason}
        </div>
        <div className="strip-actions">
          {ON_THIS_COMPUTER && <RadioButton />}
          <Working />
          <span className={`live${lost ? " off-line" : ""}`}>{lost ? "Can't reach it" : gameOpen ? "Not hosting" : "Not running"}</span>
        </div>
        <Working phone />
      </header>
    );
  }
  const st = data.state;
  const score = (team: string) => data.players.filter((p) => p.team === team).reduce((n, p) => n + p.frags, 0);
  const mode = data.modes.find((m) => m.id === st.mode)?.name ?? st.mode;
  // modes with rounds show rounds won from 0:0 (so it doesn't flip from kills after the first round)
  const byRounds = !NO_ROUNDS.has(st.mode) || (st.rounds_t ?? 0) + (st.rounds_ct ?? 0) > 0;
  const tScore = (byRounds ? st.rounds_t : score("T")) ?? 0;
  const ctScore = (byRounds ? st.rounds_ct : score("CT")) ?? 0;
  // the map and the players open that page, for your game or the open server
  const openPage = (p: "maps" | "players") => (useNav.getState().server ? useNav.getState().openServerTab(p as Section) : useNav.getState().go(p));
  return (
    <header className="strip" data-tauri-drag-region>
      <div className="strip-map">
        <div className="thumb" style={{ backgroundImage: `url(${withTarget(`/thumb/${encodeURIComponent(st.map)}.png`)})` }} aria-hidden="true" />
        <div>
          <button type="button" className="map-name strip-link" title="Maps" onClick={() => openPage("maps")}>{st.map}</button>
          <div className="strip-meta">
            {mode} ·{" "}
            <button type="button" className="strip-link" title="Players" onClick={() => openPage("players")}>
              {st.humans} {st.humans === 1 ? "person" : "people"}, {st.bots} {st.bots === 1 ? "bot" : "bots"}
            </button>
            <span className="strip-slots"> of {st.maxplayers}</span>
            {st.timelimit > 0 && st.timeleft >= 0 ? ` · ${clock(st.timeleft)} left` : ""}
            {st.nextmap ? <span className="strip-next"> · next {st.nextmap}</span> : null}
            {st.paused ? <span className="strip-pill">Paused</span> : null}
            {st.recording ? <span className="strip-pill rec">Recording</span> : null}
          </div>
        </div>
      </div>
      {/* rounds won, or (a mode without rounds, like deathmatch) kills by team */}
      <div className="score" role="group" aria-label={`${byRounds ? "Rounds won" : "Kills"}: T ${tScore}, CT ${ctScore}`} title={`${byRounds ? "Rounds won" : "Kills by team"}: T ${tScore}, CT ${ctScore}`}>
        <span className="score-side score-side-t">T</span>
        <span className="score-kind">{byRounds ? "rounds" : "kills"}</span>
        <span className="score-side score-side-ct">CT</span>
        <span className="score-n t">{shortCount(tScore)}</span>
        <span className="vs" aria-hidden="true">:</span>
        <span className="score-n ct">{shortCount(ctScore)}</span>
      </div>
      <div className="strip-actions">
        {/* a server: how long it's been up, and its updates (to the Health tab) */}
        {srv && !srv.shared && srv.updating ? <span className="strip-pill">Updating…</span>
          : srv && !srv.shared && (srv.outdated?.length ?? 0) > 0 ? (
            <button type="button" className="strip-pill strip-pill-btn" title="Its Health tab" onClick={() => useNav.getState().openServerTab("health" as Section)}>Update waiting</button>
          ) : null}
        {srv && srv.state === "running" && srv.uptime > 0 && <span className="strip-up">{srvUptime(srv.uptime)}</span>}
        {ON_THIS_COMPUTER && <RadioButton />}
        <Working />
        <span className={`live${reconnecting ? "" : " on"}`}>{reconnecting ? "Reconnecting…" : "Live"}</span>
      </div>
      <Working phone />
    </header>
  );
}

/** At most four characters, so a long game's score never widens the top bar: 999, 1.2k, 12k. */
function shortCount(n: number) {
  if (n < 1000) return String(n);
  if (n < 10000) return `${(Math.floor(n / 100) / 10).toString()}k`;
  return n < 1e6 ? `${Math.floor(n / 1000)}k` : "1m+";
}

/** Modes that never end a round (respawn or climb): the top bar counts kills instead. */
const NO_ROUNDS = new Set(["csdm", "csdm_ffa", "gungame", "gungame_team", "kreedz"]);

/** "up 2 h 5 min", for a server's top bar. */
function srvUptime(s: number) {
  if (s < 60) return "just started";
  if (s < 3600) return `up ${Math.floor(s / 60)} min`;
  if (s < 86400) return `up ${Math.floor(s / 3600)} h ${Math.floor((s % 3600) / 60)} min`;
  return `up ${Math.floor(s / 86400)} d ${Math.floor((s % 86400) / 3600)} h`;
}

/** Which game the page controls, under the top bar on every page: your own game, or the
 *  open server with a way back. */
function Where({ server, game, listing, onBack }: { server: ServerInfo | null; game: string | null; listing?: boolean; onBack: () => void }) {
  if (server) {
    return (
      <div className="where away">
        <Icon name="server" size={16} />
        <span className="where-text">
          Server: <b>{server.name}</b>
          {server.shared && <span className="where-note"> · shared with you as {server.shared.role === "admin" ? "admin" : "moderator"}</span>}
        </span>
        <button className="btn small where-back" onClick={onBack}>Back to my game</button>
      </div>
    );
  }
  // (the servers list is about your servers, not your game)
  if (listing) {
    return (
      <div className="where">
        <Icon name="server" size={16} />
        <span className="where-text"><b>All servers</b></span>
      </div>
    );
  }
  return (
    <div className="where">
      <Icon name="play" size={16} />
      <span className="where-text">
        Your game{game && <> · <b>{game}</b></>}
      </span>
    </div>
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
              ? `This works in a game you host. ${game} is open, but not hosting a game: start a game from its New Game menu, or ${ON_THIS_COMPUTER ? "pick one on Play and Loner-Strike restarts it" : "close it on the computer and start one from Play"}.`
              : "This works in a game you host. Start one from Play and this page fills in."}
          </p>
          {/* the plain "nothing loaded" reason only repeats the line above */}
          {reason && !gameOpen && !reason.startsWith("No game is loaded") && <p className="muted small-note">{reason}</p>}
          <button className={gameOpen ? "btn" : "btn primary"} onClick={onPlay}>{gameOpen ? "Go to Play" : "Start a game"}</button>
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
