import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { getJson, postJson } from "../api";
import { Icon } from "../icons";
import { SteamProgress, useToast } from "../ui";
import logo from "../assets/logo.svg";

// ---- what the setup API returns -------------------------------------------------
export type SetupGame = { id: "cstrike" | "czero"; name: string; installed: boolean; components: string[]; foreign: boolean };
export type SetupInstall = { hl: string; kind: string; bottle: string | null; games: SetupGame[] };
export type CatalogItem = {
  id: string;
  name: string;
  kind: "mode" | "addon";
  description?: string;
  games?: string[];
  bots?: "great" | "ok" | "weak" | "none";
};
export type Scan = { installs: SetupInstall[]; catalog: CatalogItem[]; native_mac: boolean };
export type Step = { id: string; label: string; state: "pending" | "running" | "done" | "error"; detail?: string; game?: string };
export type Job = { id?: string; state: "idle" | "running" | "done" | "error"; kind?: string; steps: Step[]; progress: number; error?: string; failed_compiles?: string[]; notes?: string[] };

const BOT_NOTE = { great: "Bots play this well", ok: "Bots are so-so", weak: "Best with a friend", none: "Solo, no bots" };
// Same ratings as the mode cards on the Play screen
const BOT_RATING: Record<string, "great" | "ok" | "weak" | "none"> = {
  hns: "ok", deathrun: "ok", surf: "ok", soccerjam: "weak", basebuilder: "weak", kreedz: "none",
};
const ratingOf = (c: CatalogItem) => {
  if (c.kind !== "mode") return null;
  const mode = c.id.replace(/^mode-/, "");
  return BOT_RATING[mode] ?? "great";
};
const BASICS = "Classic, Deathmatch, Free for all, knife/pistol/deagle/AWP/scout modes and Surf";

export const anySetUp = (scan: Scan | null) => !!scan?.installs.some((i) => i.games.some((g) => g.installed));

type Stage = "welcome" | "games" | "content" | "review" | "install" | "done";

const IS_MAC = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

/** onClose: back to the app without setting up; onDone: setup finished. */
export function Setup({ scan, onClose, onDone, onRescan }: { scan: Scan; onClose: () => void; onDone: () => void; onRescan: () => void }) {
  const toast = useToast();
  const [stage, setStage] = useState<Stage>("welcome");
  const [installIdx, setInstallIdx] = useState(0);
  const install = scan.installs[installIdx];
  // every game this Steam has, CS 1.6 first
  const [games, setGames] = useState<string[]>(() => (["cstrike", "czero"] as const).filter((id) => install?.games.some((g) => g.id === id)));
  const [preset, setPreset] = useState<"all" | "basics" | "pick">("all");
  const usable = useMemo(() => scan.catalog.filter((c) => !c.games || c.games.some((g) => games.includes(g))), [scan.catalog, games]);
  const [picked, setPicked] = useState<Set<string>>(() => new Set(scan.catalog.map((c) => c.id)));
  const [job, setJob] = useState<Job | null>(null);
  // setup can't change the game's files while it runs: checked on the review step
  const [gameOpen, setGameOpen] = useState(false);
  useEffect(() => {
    if (stage !== "review") return;
    const check = () => getJson<{ running: boolean }>("/api/setup/running").then((r) => setGameOpen(!!r.running)).catch(() => setGameOpen(false));
    check();
    const t = setInterval(check, 2000);
    return () => clearInterval(t);
  }, [stage]);
  const timer = useRef<number>();

  const chosen = preset === "all" ? usable : preset === "basics" ? [] : usable.filter((c) => picked.has(c.id));

  const starting = useRef(false);
  const start = async () => {
    if (starting.current) return;
    starting.current = true;
    const r = await postJson<{ ok?: boolean; job?: string; error?: string }>("/api/setup/install", {
      hl: install.hl,
      games,
      components: chosen.map((c) => c.id),
    }).catch((): { ok?: boolean; job?: string; error?: string } => ({ error: "Setup couldn't start." }));
    starting.current = false;
    if (!r.ok) return toast(r.error || "Setup couldn't start.", true);
    setStage("install");
  };

  useEffect(() => {
    if (stage !== "install") return;
    const tick = async () => {
      const j = await getJson<Job>("/api/setup/job").catch(() => null);
      if (!j) return;
      setJob(j);
      if (j.state === "done") setStage("done");
    };
    tick();
    timer.current = window.setInterval(tick, 700);
    return () => window.clearInterval(timer.current);
  }, [stage]);

  const finish = async () => {
    await postJson("/api/app/game", { hl: install.hl, game: games[0] }).catch(() => null);
    onDone();
  };

  if (!scan.installs.length) return <NoGame nativeMac={scan.native_mac} onRescan={onRescan} onClose={onClose} />;

  return (
    <div className="setup">
      <div className="setup-card">
        <ThemeMusic />
        <ol className="setup-steps" aria-label="Setup steps">
          {(["games", "content", "review", "install"] as Stage[]).map((s, i) => (
            <li key={s} className={stage === s ? "now" : order(stage) > order(s) ? "done" : ""}>
              <span>{i + 1}</span>
              {{ games: "Game", content: "What to install", review: "Review", install: "Install" }[s as "games"]}
            </li>
          ))}
        </ol>

        {stage === "welcome" && (
          <section className="setup-pane welcome">
            <img src={logo} alt="" width={112} height={112} />
            <h1>Loner-Strike</h1>
            <p className="tagline">Re-live the golden days of Counter-Strike locally or with the friends you don't have.</p>
            <p>Setup adds bots that play like people, 20 classic community modes and this control panel to your game. Everything it changes is backed up and can be removed again.</p>
            <p className="muted">Close Counter-Strike before you start.</p>
            <div className="row">
              <button className="btn primary big-btn" onClick={() => setStage("games")}>Set it up</button>
              <button className="btn" onClick={onClose}>Set up later</button>
            </div>
            <p className="muted later">Later, everything here is in the app under Mods and Settings.</p>
            <p className="made-by setup-by">by <a href="https://x.com/nanoshrine" target="_blank" rel="noreferrer">nanoshrine</a></p>
          </section>
        )}

        {stage === "games" && (
          <section className="setup-pane">
            <h2>Which game?</h2>
            {scan.installs.length > 1 && (
              <label className="field">
                <span>Steam install</span>
                <select value={installIdx} onChange={(e) => { const n = +e.target.value; setInstallIdx(n); setGames((["cstrike", "czero"] as const).filter((id) => scan.installs[n].games.some((g) => g.id === id))); }}>
                  {scan.installs.map((i, n) => <option key={i.hl} value={n}>{i.kind === "crossover" ? `CrossOver bottle "${i.bottle}"` : "Steam"} – {i.hl}</option>)}
                </select>
              </label>
            )}
            <div className="game-cards">
              {(["cstrike", "czero"] as const).map((id) => {
                const g = install.games.find((x) => x.id === id);
                const on = games.includes(id);
                return (
                  <button
                    key={id}
                    className="game-card"
                    aria-pressed={on}
                    disabled={!g}
                    onClick={() => setGames((cur) => (on ? cur.filter((x) => x !== id) : [...cur, id].sort((a) => (a === "cstrike" ? -1 : 1))))}
                  >
                    <span className="game-title">{id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero"}</span>
                    <span className="muted">
                      {!g ? "Not installed in this Steam" : g.installed ? "Set up already; picking it updates it" : g.foreign ? "Has other mods installed; Loner-Strike works alongside them" : "Ready to set up"}
                    </span>
                    <span className="check" aria-hidden="true">{on ? "✓" : ""}</span>
                  </button>
                );
              })}
            </div>
            <p className="muted">Both use the same bots. Condition Zero adds more bot voices and its own versions of the maps.</p>
            {IS_MAC && <p className="muted">On a Mac, Counter-Strike runs through CrossOver: Windows Steam installed in a CrossOver bottle. That's where Loner-Strike found your games.</p>}
            <Nav back={() => setStage("welcome")} next={() => setStage("content")} canNext={games.length > 0} />
          </section>
        )}

        {stage === "content" && (
          <section className="setup-pane">
            <h2>What to install</h2>
            <div className="seg presets-seg" role="group" aria-label="How much">
              <button aria-pressed={preset === "all"} onClick={() => setPreset("all")}>Everything ({usable.length})</button>
              <button aria-pressed={preset === "basics"} onClick={() => setPreset("basics")}>Just the basics</button>
              <button aria-pressed={preset === "pick"} onClick={() => setPreset("pick")}>Let me pick</button>
            </div>
            <p className="muted">The basics are always included: {BASICS}. You can add or remove anything later from Settings.</p>
            {preset !== "basics" && (
              <div className="pick-grid">
                {usable.map((c) => {
                  const on = preset === "all" || picked.has(c.id);
                  return (
                    <button
                      key={c.id}
                      className="pick"
                      aria-pressed={on}
                      disabled={preset === "all"}
                      onClick={() => setPicked((p) => { const n = new Set(p); if (n.has(c.id)) n.delete(c.id); else n.add(c.id); return n; })}
                    >
                      <Icon name={c.id.replace(/^(mode|addon)-/, "")} size={22} />
                      <span className="pick-name">{c.name}<small>{c.kind === "addon" ? "Add-on" : "Game mode"}</small></span>
                      {c.description && <span className="pick-desc">{c.description}</span>}
                      {ratingOf(c) && <span className={`bots-note ${ratingOf(c)}`}>{BOT_NOTE[ratingOf(c)!]}</span>}
                    </button>
                  );
                })}
              </div>
            )}
            <Nav back={() => setStage("games")} next={() => setStage("review")} />
          </section>
        )}

        {stage === "review" && (
          <section className="setup-pane">
            <h2>Ready to install</h2>
            <ul className="review">
              <li><b>Game{games.length > 1 ? "s" : ""}:</b> {games.map((g) => (g === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero")).join(" and ")}</li>
              <li><b>Mods:</b> Metamod-P, AMX Mod X and ReGameDLL (the pieces that let mods run)</li>
              <li><b>Loner-Strike:</b> bot styles, regulars, the in-game menu (F3) and this panel</li>
              <li><b>Modes and add-ons:</b> the basics{chosen.length ? ` plus ${chosen.map((c) => c.name).join(", ")}` : ""}</li>
            </ul>
            <p className="muted">It downloads from the original projects (GameBanana for maps), compiles the plugins, and backs up every file it replaces.</p>
            <p className="estimate">
              {preset === "basics"
                ? "About 15 MB to download. Usually under a minute."
                : preset === "all"
                  ? "About 100 MB to download. Usually 2 to 5 minutes."
                  : `Between 15 and 100 MB, depending on what you picked. A few minutes at most.`}
            </p>
            {gameOpen && (
              <div className="banner">
                <div>Counter-Strike is open. Close it to continue: setup can't change the game's files while it's running.</div>
              </div>
            )}
            <Nav back={() => setStage("content")} next={start} nextLabel="Install" canNext={!gameOpen} />
          </section>
        )}

        {stage === "install" && (
          <section className="setup-pane">
            <h2>{job?.state === "error" ? "Setup stopped" : "Installing"}</h2>
            <SteamProgress
              value={job?.progress ?? 0}
              title={job?.state === "error" ? "Setup stopped" : "Installing Loner-Strike…"}
              text={(() => { const now = job?.steps.find((x) => x.state === "running"); return now ? `${now.label}${now.detail ? ` · ${now.detail}` : ""}` : "Getting ready…"; })()}
              error={job?.state === "error"}
            />
            <ul className="job-steps">
              {(job?.steps ?? []).map((s) => (
                <li key={`${s.game}-${s.id}`} className={s.state}>
                  <span className="dot-state" aria-hidden="true">{s.state === "done" ? "✓" : s.state === "error" ? "!" : s.state === "running" ? "•" : ""}</span>
                  <span>
                    {s.label}
                    {s.game && games.length > 1 ? <small> · {s.game === "cstrike" ? "CS 1.6" : "CZ"}</small> : null}
                  </span>
                </li>
              ))}
            </ul>
            {job?.state === "error" && (
              <>
                <p className="err-text">{job.error}</p>
                <p className="muted">Nothing that finished is lost; run setup again to retry.</p>
                <div className="row"><button className="btn primary" onClick={() => setStage("review")}>Try again</button></div>
              </>
            )}
          </section>
        )}

        {stage === "done" && (
          <section className="setup-pane welcome">
            <div className="done-mark" aria-hidden="true">✓</div>
            <h1>You're set.</h1>
            <p>Start the game and Loner-Strike connects by itself.</p>
            <ul className="done-tips">
              <li><span className="kbd">F3</span><span>In the game: modes, maps, bots and settings. <b>F4</b> is the quick menu.</span></li>
              <li><span className="kbd">Maps</span><span>The first time you play a map, the bots spend a few minutes learning it.</span></li>
              <li><span className="kbd">Phone</span><span>Run everything from here, or from your phone (see Use on your phone in Settings).</span></li>
            </ul>
            {!!job?.failed_compiles?.length && <p className="err-text">These didn't build, so they won't load: {job.failed_compiles.join(", ")}.</p>}
            {job?.notes?.map((n) => <p key={n} className="muted">{n}</p>)}
            <button className="btn primary big-btn" onClick={finish}>Open the panel</button>
          </section>
        )}
      </div>
    </div>
  );
}

function order(s: Stage) {
  return ["welcome", "games", "content", "review", "install", "done"].indexOf(s);
}

function Nav({ back, next, canNext = true, nextLabel = "Continue" }: { back: () => void; next: () => void; canNext?: boolean; nextLabel?: string }) {
  return (
    <div className="setup-nav">
      <button className="btn" onClick={back}>Back</button>
      <button className="btn primary" disabled={!canNext} onClick={next}>{nextLabel}</button>
    </div>
  );
}

function NoGame({ nativeMac, onRescan, onClose }: { nativeMac: boolean; onRescan: () => void; onClose: () => void }) {
  const [looking, setLooking] = useState(false);
  return (
    <div className="setup">
      <div className="setup-card">
        <section className="setup-pane welcome">
          <img src={logo} alt="" width={96} height={96} />
          <h1>No game found</h1>
          {nativeMac ? (
            <p>You have the Mac version of Half-Life, which mods can't run on. Install the Windows version of Steam in CrossOver, install Counter-Strike 1.6 or Condition Zero there, then reopen Loner-Strike.</p>
          ) : (
            <p>Loner-Strike needs Counter-Strike 1.6 or Condition Zero from Steam{navigator.platform.startsWith("Mac") ? ", installed through Steam in CrossOver" : ""}. Install one, then reopen Loner-Strike.</p>
          )}
          <div className="row">
            <button className="btn primary" disabled={looking} onClick={() => { setLooking(true); onRescan(); setTimeout(() => setLooking(false), 1500); }}>{looking ? "Looking…" : "Look again"}</button>
            <button className="btn" onClick={onClose}>Continue anyway</button>
          </div>
        </section>
      </div>
    </div>
  );
}

/** The game's own menu music, quietly, while setting up. Only if the player has the
 *  game (it's their copy, never shipped with the app); it starts on the first click,
 *  since the window can't play sound before that. */
function ThemeMusic() {
  const [on, setOn] = useState(() => {
    try {
      return localStorage.getItem("ls.music") !== "off";
    } catch {
      return true;
    }
  });
  const [ready, setReady] = useState(false);
  const [volume, setVolumeRaw] = useState(() => {
    try {
      const v = Number(localStorage.getItem("ls.musicVol"));
      return v > 0 && v <= 1 ? v : 0.3;
    } catch {
      return 0.3;
    }
  });
  const level = useRef(volume);
  level.current = volume;
  const audio = useRef<HTMLAudioElement | null>(null);
  const wanted = useRef(on);
  wanted.current = on;

  // each fade takes over from the one before, so a quick off-on doesn't end up paused
  const fade = useRef(0);
  const fadeTo = (a: HTMLAudioElement, target: number) => {
    const mine = ++fade.current;
    const step = () => {
      if (mine !== fade.current) return;
      const d = target - a.volume;
      if (Math.abs(d) < 0.02) {
        a.volume = target;
        if (target === 0) a.pause();
        return;
      }
      a.volume = Math.min(1, Math.max(0, a.volume + Math.sign(d) * 0.02));
      setTimeout(step, 40);
    };
    step();
  };
  const start = (a: HTMLAudioElement) => {
    const mine = ++fade.current;
    if (a.paused) a.volume = 0;
    a.play().then(() => mine === fade.current && fadeTo(a, level.current)).catch(() => {});
  };

  useEffect(() => {
    let url = "";
    let cancelled = false;
    fetch("/api/app/theme.mp3")
      .then((r) => (r.ok ? r.blob() : null))
      .then((blob) => {
        if (!blob || cancelled) return;
        url = URL.createObjectURL(blob);
        const a = new Audio(url);
        a.loop = true;
        audio.current = a;
        setReady(true);
        if (wanted.current) start(a);
      })
      .catch(() => {});
    // the window may refuse to play before the first click: try again then
    const firstClick = () => {
      const a = audio.current;
      if (a && wanted.current && a.paused) start(a);
    };
    window.addEventListener("pointerdown", firstClick, { once: true });
    return () => {
      cancelled = true;
      fade.current++;
      window.removeEventListener("pointerdown", firstClick);
      audio.current?.pause();
      if (url) URL.revokeObjectURL(url);
    };
  }, []);

  if (!ready) return null;
  const toggle = () => {
    const next = !on;
    setOn(next);
    try {
      localStorage.setItem("ls.music", next ? "on" : "off");
    } catch {
      /* private window */
    }
    const a = audio.current;
    if (!a) return;
    if (next) start(a);
    else fadeTo(a, 0);
  };
  const setVolume = (v: number) => {
    setVolumeRaw(v);
    try {
      localStorage.setItem("ls.musicVol", String(v));
    } catch {
      /* private window */
    }
    if (audio.current && on) {
      fade.current++;
      audio.current.volume = v;
    }
  };
  return (
    <div className="music">
      <button className="music-toggle" aria-pressed={on} onClick={toggle} title={on ? "Turn the music off" : "Play the menu music"}>
        <span aria-hidden="true">♪</span> {on ? "Music on" : "Music off"}
      </button>
      {on && (
        <input
          className="music-volume"
          type="range"
          min={0.05}
          max={1}
          step={0.05}
          value={volume}
          onChange={(e) => setVolume(Number(e.target.value))}
          aria-label="Music volume"
          style={{ "--fill": `${Math.round(((volume - 0.05) / 0.95) * 100)}%` } as CSSProperties}
        />
      )}
    </div>
  );
}
