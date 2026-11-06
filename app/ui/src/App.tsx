import { useState } from "react";
import { Icon, Mark } from "./icons";
import { BOT_NOTE, LIVE, MODES, PLAYERS, RECENT_MAPS } from "./mock";

type Section = "play" | "bots" | "match" | "fun" | "players" | "addons" | "friends" | "more" | "settings";

const NAV: { id: Section; label: string; group?: string }[] = [
  { id: "play", label: "Play" },
  { id: "bots", label: "Bots" },
  { id: "match", label: "Match" },
  { id: "fun", label: "Fun" },
  { id: "players", label: "Players" },
  { id: "addons", label: "Add-ons" },
  { id: "friends", label: "Play with friends", group: "Online" },
  { id: "more", label: "Get more", group: "Library" },
  { id: "settings", label: "Settings" },
];

const GAMES = { czero: "Condition Zero", cstrike: "Counter-Strike 1.6" } as const;
type Game = keyof typeof GAMES;

function clock(s: number) {
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

export default function App() {
  const [section, setSection] = useState<Section>("play");
  const [running, setRunning] = useState(false);
  const [game, setGame] = useState<Game>("czero");

  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="drag-strip" data-tauri-drag-region />
        <div className="brand">
          <Mark />
          <span>Loner-Strike</span>
        </div>

        <nav className="nav" aria-label="Sections">
          {NAV.map((n) => (
            <div key={n.id}>
              {n.group && <div className="nav-group">{n.group}</div>}
              <button
                className="nav-item"
                aria-current={section === n.id ? "page" : undefined}
                onClick={() => setSection(n.id)}
              >
                <Icon name={n.id} />
                {n.label}
                {n.id === "players" && running && <span className="count">{LIVE.bots + LIVE.humans}</span>}
              </button>
            </div>
          ))}
        </nav>

        <div className="game-switch" role="group" aria-label="Game">
          {(Object.keys(GAMES) as Game[]).map((g) => (
            <button key={g} aria-pressed={game === g} onClick={() => setGame(g)}>
              {g === "czero" ? "CZ" : "1.6"}
            </button>
          ))}
        </div>
      </aside>

      <div className="main">
        <header className="strip" data-tauri-drag-region>
          {running ? (
            <>
              <div className="strip-map">
                <div className="thumb" aria-hidden="true" />
                <div>
                  <div className="map-name">{LIVE.map}</div>
                  <div className="strip-meta">
                    {MODES.find((m) => m.id === LIVE.mode)?.name} · {LIVE.humans + LIVE.bots}/{LIVE.maxplayers} players ·{" "}
                    {clock(LIVE.timeleft)} left
                  </div>
                </div>
              </div>
              <div className="score" aria-label="Score">
                <span className="t">{LIVE.scoreT}</span>
                <span className="vs">:</span>
                <span className="ct">{LIVE.scoreCT}</span>
              </div>
              <div className="strip-actions">
                <button className="btn">Restart map</button>
                <button className="btn">End round</button>
                <span className="live on">Live</span>
              </div>
            </>
          ) : (
            <>
              <div className="strip-meta idle">{GAMES[game]} isn't running</div>
              <span className="live">Offline</span>
            </>
          )}
        </header>

        <main className="content">
          {section === "play" && (running ? <PlayLive /> : <PlayIdle game={game} onLaunch={() => setRunning(true)} />)}
          {section === "players" && <Players />}
          {section !== "play" && section !== "players" && <Placeholder name={NAV.find((n) => n.id === section)!.label} />}
        </main>
      </div>

      <button className="mock-toggle" onClick={() => setRunning((r) => !r)}>
        Mockup: game {running ? "running" : "off"}
      </button>
    </div>
  );
}

function PlayIdle({ game, onLaunch }: { game: Game; onLaunch: () => void }) {
  return (
    <div className="idle">
      <div className="idle-hero">
        <h1>Ready when you are.</h1>
        <p>Start {GAMES[game]} and Loner-Strike connects by itself. Your bots, regulars and settings carry over.</p>
        <button className="btn launch" onClick={onLaunch}>
          <Icon name="play" size={22} />
          Play {GAMES[game]}
        </button>
        <div className="last">
          Last time: <strong>Deathmatch</strong> on <strong>de_dust2</strong> with 14 bots
        </div>
      </div>

      <section>
        <h2>Pick a mode to start in</h2>
        <ModeGrid />
      </section>
    </div>
  );
}

function PlayLive() {
  return (
    <div className="stack">
      <section>
        <div className="section-head">
          <h2>Game mode</h2>
          <p>Switching reloads the map. Each mode remembers its own settings.</p>
        </div>
        <ModeGrid current={LIVE.mode} />
      </section>
      <section>
        <div className="section-head">
          <h2>Recently played</h2>
          <button className="link">All maps</button>
        </div>
        <div className="maps">
          {RECENT_MAPS.map((m) => (
            <button key={m} className="map-card">
              <div className="map-pic" aria-hidden="true" />
              <span>{m}</span>
            </button>
          ))}
        </div>
      </section>
    </div>
  );
}

function ModeGrid({ current }: { current?: string }) {
  return (
    <div className="modes">
      {MODES.map((m) => (
        <button key={m.id} className="mode" aria-pressed={current === m.id}>
          <span className="mode-icon">
            <Icon name={m.id} size={26} />
          </span>
          <span className="mode-name">{m.name}</span>
          <span className="mode-desc">{m.desc}</span>
          <span className={`bots-note ${m.bots}`}>{BOT_NOTE[m.bots]}</span>
        </button>
      ))}
    </div>
  );
}

function Players() {
  return (
    <div className="stack">
      <div className="section-head">
        <h2>Players</h2>
        <p>Select someone for god mode, weapons, teleporting and more.</p>
      </div>
      <div className="teams">
        {(["T", "CT"] as const).map((team) => (
          <div key={team} className={`team ${team.toLowerCase()}`}>
            <h3>{team === "T" ? "Terrorists" : "Counter-Terrorists"}</h3>
            <ul>
              {PLAYERS.filter((p) => p.team === team).map((p) => (
                <li key={p.name}>
                  <span className="pname">
                    {p.name}
                    {!p.bot && <small>you</small>}
                  </span>
                  <span className="kd">
                    {p.frags}/{p.deaths}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </div>
    </div>
  );
}

function Placeholder({ name }: { name: string }) {
  return (
    <div className="placeholder">
      <h2>{name}</h2>
      <p>Same controls as today's panel, moved into this layout in the rebuild.</p>
    </div>
  );
}
