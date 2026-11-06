import { useCallback, useEffect, useState } from "react";
import { clock, getJson } from "./api";
import { useGameState } from "./api";
import { Icon } from "./icons";
import type { AppInfo, MapInfo, Online } from "./types";
import { ToastProvider, useCommand } from "./ui";
import { Addons } from "./views/Addons";
import { AppSettings } from "./views/AppSettings";
import { Bots } from "./views/Bots";
import { Friends } from "./views/Friends";
import { GetMore } from "./views/GetMore";
import { Maps } from "./views/Maps";
import { PlayIdle, PlayLive } from "./views/Play";
import { Players } from "./views/Players";
import { SettingsPage } from "./views/Settings";
import { anySetUp, Setup, type Scan } from "./views/Setup";

type Section = "play" | "maps" | "bots" | "match" | "fun" | "players" | "addons" | "friends" | "more" | "settings";

const NAV: { id: Section; label: string; icon: string; group?: string; live?: boolean }[] = [
  { id: "play", label: "Play", icon: "play" },
  { id: "maps", label: "Maps", icon: "maps" },
  { id: "bots", label: "Bots", icon: "bots", live: true },
  { id: "match", label: "Match", icon: "match", live: true },
  { id: "fun", label: "Fun", icon: "fun", live: true },
  { id: "players", label: "Players", icon: "players", live: true },
  { id: "addons", label: "Add-ons", icon: "addons", live: true },
  { id: "friends", label: "Play with friends", icon: "friends", group: "Online" },
  { id: "more", label: "Get more", icon: "more", group: "Library" },
  { id: "settings", label: "Settings", icon: "settings" },
];

function loadSection(): Section {
  try {
    const s = localStorage.getItem("ls.section") as Section | null;
    return s && NAV.some((n) => n.id === s) ? s : "play";
  } catch {
    return "play";
  }
}

export default function App() {
  return (
    <ToastProvider>
      <Shell />
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

  const setSection = (s: Section) => {
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

  useEffect(() => {
    reloadApp();
    reloadMaps();
    reloadScan();
  }, [reloadApp, reloadMaps, reloadScan]);

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

  const players = online?.players.length ?? 0;

  // First run (nothing set up yet), or opened from Settings
  if (scan && (setupOpen || !anySetUp(scan))) {
    return <Setup scan={scan} onClose={anySetUp(scan) ? () => setSetupOpen(false) : undefined} />;
  }
  const needsGame = NAV.find((n) => n.id === section)?.live && !online;

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="drag-strip" data-tauri-drag-region />
        <div className="brand">Loner-Strike</div>
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
          {app?.game ? (app.game.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero") : "No game found"}
          {app?.version && <span> · v{app.version}</span>}
        </div>
      </aside>

      <div className="main">
        <Strip data={online} reason={state && !state.online ? state.reason : ""} app={app} refresh={refresh} />
        <main className="content">
          {state === null ? (
            <p className="muted">Connecting…</p>
          ) : needsGame ? (
            <Waiting label={NAV.find((n) => n.id === section)!.label} onPlay={() => setSection("play")} />
          ) : (
            <>
              {section === "play" &&
                (online ? <PlayLive data={online} maps={maps} refresh={refresh} openMaps={() => setSection("maps")} /> : <PlayIdle app={app} reason={state.online ? "" : state.reason} maps={maps} />)}
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
              {section === "more" && <GetMore onInstalled={() => reloadMaps(true)} />}
              {section === "settings" && <AppSettings app={app} reloadApp={reloadApp} scan={scan} openSetup={() => setSetupOpen(true)} />}
            </>
          )}
        </main>
      </div>

      {/* phones: the five most used sections, the rest under More */}
      <nav className="tabbar" aria-label="Sections">
        {TABBAR.map((id) => {
          const n = NAV.find((x) => x.id === id)!;
          return (
            <button key={id} aria-current={section === id && !moreOpen ? "page" : undefined} onClick={() => setSection(id)}>
              <Icon name={n.icon} size={22} />
              <span>{n.label}</span>
              {id === "players" && players > 0 && <i className="tab-count">{players}</i>}
            </button>
          );
        })}
        <button aria-current={moreOpen || !TABBAR.includes(section) ? "page" : undefined} onClick={() => setMoreOpen((o) => !o)}>
          <Icon name="menu" size={22} />
          <span>More</span>
          {app?.update && <i className="tab-dot" />}
        </button>
      </nav>
      {moreOpen && (
        <div className="sheet-backdrop bottom" onClick={() => setMoreOpen(false)}>
          <div className="more-sheet" onClick={(e) => e.stopPropagation()}>
            {NAV.filter((n) => !TABBAR.includes(n.id)).map((n) => (
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

const TABBAR: Section[] = ["play", "maps", "bots", "players"];

function Strip({ data, reason, app, refresh }: { data: Online | null; reason: string; app: AppInfo | null; refresh: () => void }) {
  const run = useCommand(refresh);
  if (!data) {
    return (
      <header className="strip" data-tauri-drag-region>
        <div className="strip-meta idle" data-tauri-drag-region>
          {app?.game ? `${app.game.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero"} isn't running` : reason}
        </div>
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
        <div className="thumb" style={{ backgroundImage: `url(/thumb/${encodeURIComponent(st.map)}.png)` }} aria-hidden="true" />
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
        <button className="btn" onClick={() => run("gm_restartmap")}>Restart map</button>
        <button className="btn" onClick={() => run("gm_do", ["endround"])}>End round</button>
        <span className="live on">Live</span>
      </div>
    </header>
  );
}

function MatchButtons({ refresh }: { refresh: () => void }) {
  const run = useCommand(refresh);
  return (
    <div className="row">
      <button className="btn primary" onClick={() => run("gm_match", ["restart"])}>Restart round</button>
      <button className="btn" onClick={() => run("gm_do", ["endround"])}>End round now</button>
      <button className="btn" onClick={() => run("gm_do", ["addtime"])}>Add 10 minutes</button>
      <button className="btn danger" onClick={() => confirm("Reset every match and fun setting for this mode to its defaults?") && run("gm_match", ["reset"])}>
        Reset this mode's settings
      </button>
    </div>
  );
}

function Waiting({ label, onPlay }: { label: string; onPlay: () => void }) {
  return (
    <div className="waiting">
      <h2>{label}</h2>
      <p className="muted">This needs a game running.</p>
      <button className="btn primary" onClick={onPlay}>Go to Play</button>
    </div>
  );
}
