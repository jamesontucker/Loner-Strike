import { FilesUpdate } from "./views/FilesUpdate";
import { useCallback, useEffect, useState } from "react";
import { clock, getJson, ON_THIS_COMPUTER, postJson, setTarget, withTarget, useActions, useActivity } from "./api";
import { useGameState } from "./api";
import { Icon } from "./icons";
import type { AppInfo, MapInfo, Online, ServerList } from "./types";
import { applyScale, ConfirmProvider, savedScale, ToastProvider, useToast } from "./ui";
import { Addons } from "./views/Addons";
import { AppSettings } from "./views/AppSettings";
import { Bots } from "./views/Bots";
import { Admins } from "./views/Admins";
import { Friends } from "./views/Friends";
import { GetMore } from "./views/GetMore";
import { Maps } from "./views/Maps";
import { Mods } from "./views/Mods";
import { PlayIdle, PlayLive } from "./views/Play";
import { Players } from "./views/Players";
import { QuickBar } from "./views/QuickBar";
import { ServerPage, Servers } from "./views/Servers";
import { Community } from "./views/Community";
import { ServerExtras } from "./views/ServerExtras";
import { Sounds } from "./views/Sounds";
import { MatchButtons, SettingsPage } from "./views/Settings";
import { Welcome } from "./views/Welcome";
import { anySetUp, Setup, type Scan } from "./views/Setup";

type Section = "play" | "maps" | "bots" | "match" | "fun" | "players" | "addons" | "friends" | "servers" | "admins" | "community" | "extras" | "welcome" | "more" | "mods" | "sounds" | "settings";

const NAV: { id: Section; label: string; icon: string; group?: string; live?: boolean }[] = [
  { id: "play", label: "Play", icon: "play" },
  { id: "maps", label: "Maps", icon: "maps" },
  { id: "bots", label: "Bots", icon: "bots", live: true },
  { id: "match", label: "Match", icon: "match", live: true },
  { id: "fun", label: "Fun", icon: "fun", live: true },
  { id: "players", label: "Players", icon: "players", live: true },
  { id: "addons", label: "Add-ons", icon: "addons", live: true },
  { id: "friends", label: "Play with friends", icon: "friends", group: "Online" },
  { id: "admins", label: "Admins", icon: "admins" },
  { id: "community", label: "Stats & moderation", icon: "stats" },
  { id: "welcome", label: "Welcome page", icon: "welcome" },
  { id: "extras", label: "Rotation & messages", icon: "extras" },
  { id: "more", label: "Get more", icon: "more", group: "Library" },
  { id: "mods", label: "Mods", icon: "addons" },
  { id: "sounds", label: "Sounds", icon: "sound" },
  { id: "settings", label: "Settings", icon: "settings" },
  { id: "servers", label: "Servers", icon: "server", group: "Hosting" },
];

function loadSection(): Section {
  try {
    const s = localStorage.getItem("ls.section") as Section | null;
    return s && NAV.some((n) => n.id === s) ? s : "play";
  } catch {
    return "play";
  }
}

applyScale(savedScale());

export default function App() {
  return (
    <ToastProvider>
      <ConfirmProvider>
        <Shell />
      </ConfirmProvider>
    </ToastProvider>
  );
}

function Shell() {
  const { state, refresh } = useGameState();
  const [section, setSectionRaw] = useState<Section>(loadSection);
  const [app, setApp] = useState<AppInfo | null>(null);
  const [maps, setMaps] = useState<MapInfo[]>([]);
  const [moreOpen, setMoreOpen] = useState(false);
  const [scan, setScan] = useState<Scan | null>(null);
  const [setupOpen, setSetupOpen] = useState(false);
  // "Set up later" on the first run: the app without the local setup
  const [setupLater, setSetupLater] = useState(() => {
    try {
      return localStorage.getItem("ls.setupLater") === "1";
    } catch {
      return false;
    }
  });
  const [servers, setServers] = useState<ServerList | null>(null);
  // the dedicated server open in the Servers area; everything else is always your game
  const [openServer, setOpenServer] = useState<string | null>(null);
  const toast = useToast();
  const { run: once, busy } = useActions();

  const setSection = (s: Section) => {
    if (s !== "servers" && openServer) enterServer(null);
    window.dispatchEvent(new Event("ls-nav"));
    setSectionRaw(s);
    setMoreOpen(false);
    document.querySelector(".content")?.scrollTo({ top: 0 });
    try {
      localStorage.setItem("ls.section", s);
    } catch {
      /* private window */
    }
  };
  const reloadApp = useCallback(() => {
    getJson<AppInfo>("/api/app").then(setApp).catch(() => setApp(null));
  }, []);
  const reloadMaps = useCallback((fresh = false) => {
    getJson<MapInfo[]>(`/api/maps${fresh ? "?refresh" : ""}`)
      .then((m) => setMaps(Array.isArray(m) ? m : []))
      .catch(() => setMaps([]));
  }, []);

  const reloadScan = useCallback(() => {
    getJson<Scan>("/api/setup/scan").then(setScan).catch(() => setScan(null));
  }, []);

  const reloadServers = useCallback(() => {
    // a failed poll keeps the last list, so an open server page doesn't vanish
    getJson<ServerList>("/api/servers").then(setServers).catch(() => null);
  }, []);

  useEffect(() => {
    reloadApp();
    reloadMaps();
    reloadScan();
    reloadServers();
    const t = setInterval(() => !document.hidden && reloadServers(), 5000);
    return () => clearInterval(t);
  }, [reloadApp, reloadMaps, reloadScan, reloadServers]);

  // opens a server's own page (its requests name it), or goes back to your game
  const enterServer = (id: string | null) => {
    setTarget(id);
    setOpenServer(id);
    setMaps([]);
    refresh();
    reloadApp();
    reloadMaps(true);
  };
  // CS 1.6 / Condition Zero: which of your games the app is about
  const switchGame = (game: string) => {
    if (app?.game?.id === game) return;
    once("app:switch-game", async () => {
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/servers/select", { id: `local:${game}` }).catch(() => ({ error: "Couldn't reach the app." }));
      if (r.error) return toast(r.error, true);
      reloadApp();
      reloadMaps(true);
      refresh();
    }, `Switching to ${game === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero"}…`);
  };
  const current = openServer ? servers?.servers.find((x) => x.id === openServer) ?? null : null;
  // the open server was deleted (or its folder went away): back to your own game
  useEffect(() => {
    if (openServer && servers && !current) enterServer(null);
  });
  // CS 1.6 and Condition Zero on this computer, CS 1.6 first
  const localGames = (["cstrike", "czero"] as const).filter((g) => scan?.installs.some((i) => i.games.some((x) => x.id === g)));
  const myGame = openServer ? null : app?.game?.id;

  // the macOS menu bar: "section:<id>" or "updates"
  useEffect(() => {
    const onMenu = async (e: Event) => {
      const what = (e as CustomEvent<string>).detail;
      if (what.startsWith("section:")) {
        const id = what.slice(8) as Section;
        if (NAV.some((n) => n.id === id)) setSection(id);
      } else if (what === "updates") {
        setSection("settings");
        // same key as Settings' Check now button, so it shows there too
        await once("app:update:check", async () => {
          const r = await postJson<{ available?: boolean; version?: string; error?: string }>("/api/app/update/check").catch((): { available?: boolean; version?: string; error?: string } => ({ error: "Couldn't check for updates." }));
          if (r.error) toast(r.error, true);
          else toast(r.available ? `Loner-Strike ${r.version} is out. Install it from Settings.` : "You're on the latest version.");
          reloadApp();
        }, "Checking for updates…");
      }
    };
    window.addEventListener("ls-menu", onMenu);
    return () => window.removeEventListener("ls-menu", onMenu);
  });

  // ⌘1-⌘9 switch sections
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey) || e.altKey) return;
      const n = Number(e.key);
      if (n >= 1 && n <= 9 && NAV[n - 1]) {
        e.preventDefault();
        setSection(NAV[n - 1].id);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  const online = state?.online ? (state as Online) : null;
  // re-scan maps when the bots finish learning one
  const navKey = online ? `${online.state.map}:${online.state.nav}` : "";
  useEffect(() => {
    if (navKey) reloadMaps(true);
  }, [navKey, reloadMaps]);

  const players = openServer ? 0 : online?.players.length ?? 0;
  const tabbar = tabbarFor(!!servers?.servers.length);

  // First run (nothing set up yet), or opened from Settings
  if (scan && (setupOpen || (!anySetUp(scan) && !setupLater))) {
    const close = () => {
      setSetupOpen(false);
      setSetupLater(true);
      try {
        localStorage.setItem("ls.setupLater", "1");
      } catch {
        /* private window */
      }
    };
    const done = () => {
      setSetupOpen(false);
      reloadScan();
      reloadApp();
      reloadMaps(true);
      refresh();
    };
    return <Setup scan={scan} onClose={close} onDone={done} onRescan={reloadScan} />;
  }
  const needsGame = section !== "servers" && NAV.find((n) => n.id === section)?.live && !online;
  // the game is open here but isn't hosting a map (its main menu, or someone else's server)
  const gameOpen = !!state && !state.online && !!state.game_open;
  const gameName = app?.game?.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero";

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="drag-strip" data-tauri-drag-region />
        <div className="brand">Loner-Strike</div>
        {localGames.length > 1 && (
          <div className="game-switch" role="group" aria-label="Your game">
            {localGames.map((g) => (
              <button key={g} title={g === "cstrike" ? "Counter-Strike 1.6" : "Counter-Strike: Condition Zero"} aria-pressed={myGame === g} disabled={!!openServer || busy("app:switch-game")} onClick={() => switchGame(g)}>
                {g === "cstrike" ? "CS 1.6" : "CZ"}
              </button>
            ))}
          </div>
        )}
        <nav className="nav" aria-label="Sections">
          {NAV.map((n) => (
            <div key={n.id}>
              {n.group && <div className="nav-group">{n.group}</div>}
              <button className="nav-item" aria-current={section === n.id ? "page" : undefined} onClick={() => setSection(n.id)}>
                <Icon name={n.icon} />
                {n.label}
                {n.id === "players" && players > 0 && <span className="count">{players}</span>}
                {n.id === "settings" && app?.update && <span className="dot" aria-label="Update available" />}
              </button>
            </div>
          ))}
        </nav>
        <div className="side-foot">
          {current ? `Server: ${current.name}` : app?.game ? (app.game.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero") : "No game found"}
          {app?.version && <span> · v{app.version}</span>}
          <div className="made-by">
            by <a href="https://x.com/nanoshrine" target="_blank" rel="noreferrer">nanoshrine</a>
          </div>
        </div>
      </aside>

      <div className="main">
        <Strip data={online} reason={state && !state.online ? state.reason : ""} app={app} server={current?.name} gameOpen={gameOpen} />
        {online && <QuickBar data={online} maps={maps} refresh={refresh} reloadMaps={() => reloadMaps(true)} />}
        <main className="content">
          <HomeScreenHint />
          {!setupOpen && <FilesUpdate app={app} scan={scan} gameOpen={gameOpen || !!online} reloadApp={reloadApp} />}
          {state === null ? (
            <p className="muted">Connecting…</p>
          ) : needsGame ? (
            <Waiting label={NAV.find((n) => n.id === section)!.label} game={gameName} gameOpen={gameOpen} onPlay={() => setSection("play")} setUp={!app?.game || !!app.game.installed?.includes("core")} openSetup={() => setSetupOpen(true)} />
          ) : (
            <>
              {section === "play" &&
                (online ? (
                  <PlayLive data={online} maps={maps} refresh={refresh} openMaps={() => setSection("maps")} />
                ) : (
                  <PlayIdle app={app} reason={state.online ? "" : state.reason} gameOpen={gameOpen} maps={maps} openSetup={() => setSetupOpen(true)} />
                ))}
              {section === "maps" && <Maps data={online} maps={maps} reload={() => reloadMaps(true)} refresh={refresh} />}
              {section === "bots" && online && <Bots data={online} refresh={refresh} />}
              {section === "match" && online && (
                <SettingsPage title="Match" note={`Saved for ${online.modes.find((m) => m.id === online.state.mode)?.name ?? "this mode"}. Round changes take effect next round.`} data={online} cat={2} refresh={refresh}>
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
                    reloadMaps={() => reloadMaps(true)}
                    reloadApp={reloadApp}
                    onChanged={reloadServers}
                    onBack={() => enterServer(null)}
                  />
                ) : (
                  <Servers scan={scan} list={servers} onOpen={enterServer} onChanged={reloadServers} />
                ))}
              {section === "admins" && <Admins data={online} installed={app?.game?.installed} />}
              {section === "welcome" && <Welcome target="local" />}
              {section === "sounds" && <Sounds target="local" />}
              {section === "extras" && <ServerExtras target="local" />}
              {section === "community" && <Community target="local" />}
              {section === "more" && <GetMore onInstalled={() => reloadMaps(true)} />}
              {section === "mods" && <Mods app={app} scan={scan} running={!!online} onChanged={() => { reloadApp(); reloadScan(); }} openSetup={() => setSetupOpen(true)} />}
              {section === "settings" && <AppSettings app={app} reloadApp={reloadApp} scan={scan} openSetup={() => setSetupOpen(true)} switchTo={(id) => switchGame(id.replace(/^local:/, ""))} />}
            </>
          )}
        </main>
      </div>

      {/* phones: the five most used sections, the rest under More */}
      <nav className="tabbar" aria-label="Sections">
        {tabbar.map((id) => {
          const n = NAV.find((x) => x.id === id)!;
          return (
            <button key={id} aria-current={section === id && !moreOpen ? "page" : undefined} onClick={() => setSection(id)}>
              <Icon name={n.icon} size={22} />
              <span>{n.label}</span>
              {id === "players" && players > 0 && <i className="tab-count">{players}</i>}
            </button>
          );
        })}
        <button aria-current={moreOpen || !tabbar.includes(section) ? "page" : undefined} onClick={() => setMoreOpen((o) => !o)}>
          <Icon name="menu" size={22} />
          <span>More</span>
          {app?.update && <i className="tab-dot" />}
        </button>
      </nav>
      {moreOpen && (
        <div className="sheet-backdrop bottom" onClick={() => setMoreOpen(false)}>
          <div className="more-sheet" onClick={(e) => e.stopPropagation()}>
            {NAV.filter((n) => !tabbar.includes(n.id)).map((n) => (
              <button key={n.id} className="nav-item" aria-current={section === n.id ? "page" : undefined} onClick={() => setSection(n.id)}>
                <Icon name={n.icon} />
                {n.label}
                {n.id === "settings" && app?.update && <span className="dot" />}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

/** The phone's bottom bar: the most used sections, and Servers for people who host. */
const tabbarFor = (hosting: boolean): Section[] => (hosting ? ["play", "maps", "players", "servers"] : ["play", "maps", "bots", "players"]);

/** What's in progress anywhere in the app, so it's visible from any page. */
function Working() {
  const acts = useActivity().filter(([, label]) => label);
  if (!acts.length) return null;
  return (
    <span className="working" role="status">
      <span className="spinner" aria-hidden="true" />
      {acts[0][1]}
      {acts.length > 1 ? ` (+${acts.length - 1} more)` : ""}
    </span>
  );
}

function Strip({ data, reason, app, server, gameOpen }: { data: Online | null; reason: string; app: AppInfo | null; server?: string; gameOpen: boolean }) {
  if (!data) {
    return (
      <header className="strip" data-tauri-drag-region>
        <div className="strip-meta idle" data-tauri-drag-region>
          {server
            ? `${server} isn't running`
            : app?.game
              ? `${app.game.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero"} ${gameOpen ? "is open, but no game is loaded" : "isn't running"}`
              : reason}
        </div>
        <Working />
        <span className="live">Offline</span>
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
        <span className="live on">Live</span>
      </div>
    </header>
  );
}

function Waiting({ label, game, gameOpen, onPlay, setUp, openSetup }: { label: string; game: string; gameOpen: boolean; onPlay: () => void; setUp: boolean; openSetup: () => void }) {
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
          {!gameOpen && <button className="btn primary" onClick={onPlay}>Start a game</button>}
        </>
      ) : (
        <>
          <p className="muted">Loner-Strike isn't set up on this game yet, so there's nothing to control.</p>
          <button className="btn primary" onClick={openSetup}>Set it up</button>
        </>
      )}
    </div>
  );
}

/** On a phone in the browser: how to put the panel on the home screen, so it opens like an app. */
function HomeScreenHint() {
  const [gone, setGone] = useState(() => {
    try {
      return localStorage.getItem("ls.homeHint") === "1";
    } catch {
      return false;
    }
  });
  const standalone = window.matchMedia("(display-mode: standalone)").matches || (navigator as { standalone?: boolean }).standalone === true;
  if (ON_THIS_COMPUTER || standalone || gone || window.innerWidth > 760) return null;
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
        Put Loner-Strike on your home screen: {ios ? <>tap <b>Share</b>, then <b>Add to Home Screen</b></> : <>tap <b>⋮</b>, then <b>Add to Home screen</b></>}. It opens like an app.
      </div>
      <button className="btn" onClick={hide}>Got it</button>
    </div>
  );
}
