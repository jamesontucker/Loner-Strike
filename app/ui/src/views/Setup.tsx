import { useEffect, useMemo, useRef, useState, type CSSProperties } from "react";
import { useQuery } from "@tanstack/react-query";
import { getJsonAt, postJson, useActions } from "../api";
import { BOT_NOTE, BOT_RATING, BOT_RATING_INFO, gameShort, gameTitle } from "../games";
import { Icon } from "../icons";
import { useSetupJob, waitForSetupJob } from "../queries";
import { useNav } from "../store";
import { BusyButton, Info, SteamProgress, useToast } from "../ui";
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
/** First-run setup, or (fromSettings) setup run again from Settings: then it starts at
 *  the game choice and its way out is Cancel. */
export function Setup({ scan, fromSettings, onClose, onDone, onRescan }: { scan: Scan; fromSettings?: boolean; onClose: () => void; onDone: () => void; onRescan: () => void }) {
  const toast = useToast();
  // opened with Watch the intro: the way out closes it, it isn't "later"
  const intro = useNav((n) => n.setupIntro);
  // from Settings, each step before the install can be left
  const cancel = fromSettings ? onClose : undefined;
  // a setup already running (started before a reload, or from Mods): straight to its progress
  const jobQ = useSetupJob();
  const [stage, setStage] = useState<Stage>(() => (jobQ.data?.state === "running" ? "install" : fromSettings ? "games" : "welcome"));
  useEffect(() => {
    if (jobQ.data?.state === "running" && stage !== "install") setStage("install");
    // only the first answer counts: later ones are this setup's own job
  }, [jobQ.isFetched]);
  const [installIdx, setInstallIdx] = useState(0);
  const install = scan.installs[installIdx];
  // every game this Steam has, CS 1.6 first
  const [games, setGames] = useState<string[]>(() => (["cstrike", "czero"] as const).filter((id) => install?.games.some((g) => g.id === id)));
  const [preset, setPreset] = useState<"all" | "basics" | "pick">("all");
  const usable = useMemo(() => scan.catalog.filter((c) => !c.games || c.games.some((g) => games.includes(g))), [scan.catalog, games]);
  const [picked, setPicked] = useState<Set<string>>(() => new Set(scan.catalog.map((c) => c.id)));
  const job = stage === "install" || stage === "done" ? jobQ.data ?? null : null;
  // the app stopped answering while setup ran: say so instead of a bar that never moves
  const [lost, setLost] = useState(false);
  // setup can't change the game's files while it runs: checked on the review step
  const runningQ = useQuery({
    queryKey: ["setupGameOpen"],
    queryFn: ({ signal }) => getJsonAt<{ running: boolean }>("/api/setup/running", null, signal),
    enabled: stage === "review",
    refetchInterval: 2000,
  });
  const gameOpen = stage === "review" && !!runningQ.data?.running;

  const chosen = preset === "all" ? usable : preset === "basics" ? [] : usable.filter((c) => picked.has(c.id));

  const { run, busy } = useActions();
  const start = () => run("setup:install", async () => {
    const r = await postJson<{ ok?: boolean; job?: string; error?: string }>("/api/setup/install", {
      hl: install.hl,
      games,
      components: chosen.map((c) => c.id),
    }).catch((): { ok?: boolean; job?: string; error?: string } => ({ error: "Setup couldn't start." }));
    if (!r.ok) return toast(r.error || "Setup couldn't start.", true);
    setLost(false);
    // the new job, not the last one's end, before the progress shows
    await jobQ.refetch();
    setStage("install");
  }, "Starting setup…");

  // follow the job to its end; the progress itself comes from useSetupJob
  useEffect(() => {
    if (stage !== "install" || lost) return;
    let gone = false;
    waitForSetupJob().then((j) => {
      if (gone) return;
      if (!j) setLost(true);
      else if (j.state === "done") setStage("done");
    });
    return () => {
      gone = true;
    };
  }, [stage, lost]);

  const finish = () => run("setup:finish", async () => {
    await postJson("/api/app/game", { hl: install.hl, game: games[0] }).catch(() => null);
    onDone();
  }, "Opening the panel…");

  if (!scan.installs.length) return <NoGame nativeMac={scan.native_mac} onRescan={onRescan} onClose={onClose} />;

  return (
    <div className="setup">
      <div className="setup-card">
        {!fromSettings && <ThemeMusic />}
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
            <p className="tagline">Play Counter-Strike like it's 2004,<br />alone or with the friends you don't have.</p>
            <p>Setup adds bots that play like people, 22 classic community modes and this control panel to your game. Everything it changes is backed up and can be removed again.</p>
            <p className="muted">Close Counter-Strike before you start.</p>
            <div className="row">
              <button className="btn primary big-btn" onClick={() => setStage("games")}>Set it up</button>
              <button className="btn" onClick={onClose}>{intro ? "Close" : "Set up later"}</button>
            </div>
            <p className="muted later">Later, you can add or remove any of it under Mods.</p>
            <p className="made-by setup-by">by <a href="https://x.com/nanoshrine" target="_blank" rel="noreferrer">nanoshrine</a></p>
          </section>
        )}

        {stage === "games" && (
          <section className="setup-pane">
            <h2>Which game?</h2>
            {scan.installs.length > 1 && (
              <label className="field">
                <span>Steam install<Info text="Loner-Strike found more than one Steam on this computer; on a Mac each CrossOver bottle with Steam counts. Pick the one your Counter-Strike is in." /></span>
                <select value={installIdx} onChange={(e) => { const n = +e.target.value; setInstallIdx(n); setGames((["cstrike", "czero"] as const).filter((id) => scan.installs[n].games.some((g) => g.id === id))); }}>
                  {scan.installs.map((i, n) => <option key={i.hl} value={n}>{i.kind === "crossover" ? `CrossOver bottle "${i.bottle}"` : "Steam"}: {i.hl}</option>)}
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
                    <span className="game-title">{gameTitle(id)}</span>
                    <span className="muted">
                      {!g ? "Not installed in this Steam" : g.installed ? "Set up already; picking it updates it" : g.foreign ? "Has other mods installed; Loner-Strike works alongside them" : "Ready to set up"}
                    </span>
                    <span className="check" aria-hidden="true">{on ? "✓" : ""}</span>
                  </button>
                );
              })}
            </div>
            <p className="muted">Both use the same bots. Condition Zero adds more bot voices and its own versions of the maps.</p>
            {IS_MAC && <p className="muted">On a Mac, Counter-Strike runs through CrossOver (Windows Steam in a CrossOver bottle), which is where Loner-Strike found your games. With Parallels instead, use the Windows version of Loner-Strike inside Windows.</p>}
            <Nav back={fromSettings ? onClose : () => setStage("welcome")} backLabel={fromSettings ? "Cancel" : "Back"} next={() => setStage("content")} canNext={games.length > 0} />
          </section>
        )}

        {stage === "content" && (
          <section className="setup-pane">
            <h2>What to install</h2>
            <div className="seg presets-seg" role="group" aria-label="How much">
              <button aria-pressed={preset === "all"} onClick={() => setPreset("all")}>Everything ({usable.length})</button>
              <button aria-pressed={preset === "basics"} onClick={() => setPreset("basics")}>Only the basics</button>
              <button aria-pressed={preset === "pick"} onClick={() => setPreset("pick")}>Let me pick</button>
            </div>
            <p className="muted">The basics are always included: {BASICS}. You can add or remove anything later under Mods.</p>
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
                      {ratingOf(c) && <span className={`bots-note ${ratingOf(c)}`} title={BOT_RATING_INFO}>{BOT_NOTE[ratingOf(c)!]}</span>}
                    </button>
                  );
                })}
              </div>
            )}
            <Nav back={() => setStage("games")} next={() => setStage("review")} cancel={cancel} />
          </section>
        )}

        {stage === "review" && (
          <section className="setup-pane">
            <h2>Ready to install</h2>
            <ul className="review">
              <li><b>Game{games.length > 1 ? "s" : ""}:</b> {games.map(gameTitle).join(" and ")}</li>
              <li><b>Mods:</b> Metamod-P, AMX Mod X and ReGameDLL (the pieces that let mods run)</li>
              <li><b>Loner-Strike:</b> bot styles, regular bots (the same names keep coming back, like on a real server), the Game Mode Menu (F3 in the game) and this app</li>
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
            <Nav back={() => setStage("content")} next={start} nextLabel="Install" canNext={!gameOpen} busy={busy("setup:install")} busyLabel="Starting…" cancel={cancel} />
          </section>
        )}

        {stage === "install" && (
          <section className="setup-pane">
            {lost && job?.state !== "error" && (
              <>
                <h2>Lost contact with setup</h2>
                <p className="err-text">Loner-Strike stopped answering, so there's no telling how far setup got.</p>
                <p className="muted">Try again to check on it. If the app restarted, run setup again: whatever finished is kept.</p>
                <div className="row">
                  <button className="btn primary" onClick={() => { setLost(false); jobQ.refetch(); }}>Try again</button>
                  <button className="btn" onClick={onClose}>Close</button>
                </div>
              </>
            )}
            {!(lost && job?.state !== "error") && <>
            <h2>{job?.state === "error" ? "Setup stopped" : "Installing"}</h2>
            {job?.state !== "error" && <p className="muted">Keep Loner-Strike open until this finishes.</p>}
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
                    {s.game && games.length > 1 ? <small> · {gameShort(s.game)}</small> : null}
                  </span>
                </li>
              ))}
            </ul>
            {job?.state === "error" && (
              <>
                <p className="err-text">{job.error}</p>
                <p className="muted">Nothing that finished is lost. Try again to carry on.</p>
                <div className="row">
                  <button className="btn primary" onClick={() => setStage("review")}>Back to review</button>
                  <button className="btn" onClick={onClose}>Close</button>
                </div>
              </>
            )}
            </>}
          </section>
        )}

        {stage === "done" && (
          <section className="setup-pane welcome">
            <div className="done-mark" aria-hidden="true"><img src={logo} alt="" /></div>
            <h1>You're set</h1>
            <p>Start the game and Loner-Strike connects by itself.</p>
            <ul className="done-tips">
              <li><span className="kbd">F3</span><span>In the game: modes, maps, bots and settings. <b>F4</b> is the quick menu.</span></li>
              <li><span className="kbd">Maps</span><span>The first time you play a map, the bots spend a few minutes learning it.</span></li>
              <li><span className="kbd">Phone</span><span>Run everything from here, or from your phone (see Use on your phone in Settings).</span></li>
            </ul>
            {!!job?.failed_compiles?.length && <p className="err-text">These didn't build, so they won't load: {job.failed_compiles.join(", ")}.</p>}
            {job?.notes?.map((n) => <p key={n} className="muted">{n}</p>)}
            <BusyButton className="btn primary big-btn" busy={busy("setup:finish")} label="Open the panel" busyLabel="Opening…" onClick={finish} />
          </section>
        )}
      </div>
    </div>
  );
}

function order(s: Stage) {
  return ["welcome", "games", "content", "review", "install", "done"].indexOf(s);
}

function Nav({ back, backLabel = "Back", next, canNext = true, nextLabel = "Continue", busy = false, busyLabel = "", cancel }: { back: () => void; backLabel?: string; next: () => void; canNext?: boolean; nextLabel?: string; busy?: boolean; busyLabel?: string; cancel?: () => void }) {
  return (
    <div className="setup-nav">
      <div className="row">
        <button className="btn" disabled={busy} onClick={back}>{backLabel}</button>
        {cancel && <button className="btn" disabled={busy} onClick={cancel}>Cancel</button>}
      </div>
      <BusyButton className="btn primary" busy={busy} disabled={!canNext} label={nextLabel} busyLabel={busyLabel} onClick={next} />
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
            <p>You have the Mac version of Half-Life, which mods can't run on. Install the Windows version of Steam in CrossOver, install Counter-Strike 1.6 or Condition Zero there, then reopen Loner-Strike. With Parallels, install the game and the Windows version of Loner-Strike inside Windows instead.</p>
          ) : (
            <p>Loner-Strike needs Counter-Strike 1.6 or Condition Zero from Steam{navigator.platform.startsWith("Mac") ? ", installed through Steam in CrossOver (or, with Parallels, use the Windows version of Loner-Strike inside Windows)" : ""}. Install one, then reopen Loner-Strike.</p>
          )}
          <div className="row">
            <BusyButton className="btn primary" busy={looking} label="Look again" busyLabel="Looking…" onClick={() => { setLooking(true); onRescan(); setTimeout(() => setLooking(false), 1500); }} />
            <button className="btn" onClick={onClose}>Continue anyway</button>
          </div>
        </section>
      </div>
    </div>
  );
}

/** The game's own menu music, quietly, while setting up: Counter-Strike's theme, then
 *  Half-Life's, round and round; and the menu's own hover and click sounds on buttons.
 *  Only if the player has the game (their copy, never shipped with the app); it starts
 *  on the first click, since the window can't play sound before that. */
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
  // the playlist's "next random track" (null with only one track)
  const skipTrack = useRef<(() => void) | null>(null);
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
    let cancelled = false;
    // the playlist: both menu themes and the Half-Life tracks picked for it, shuffled (a
    // random one first), reshuffled each time round, never the same one twice in a row.
    // Played straight from the app (not loaded whole), and only the ones this game has.
    const ALL = ["", "hl", "hl01", "hl02", "hl12", "hl13", "hl17"].map((t) => (t ? `/api/app/theme.mp3?track=${t}` : "/api/app/theme.mp3"));
    Promise.all(ALL.map((u) => fetch(u, { method: "HEAD" }).then((r) => (r.ok ? u : null)).catch(() => null))).then((found) => {
      if (cancelled) return;
      const tracks = found.filter((u): u is string => !!u);
      if (!tracks.length) return;
      const shuffle = (last?: string) => {
        const o = [...tracks];
        for (let k = o.length - 1; k > 0; k--) {
          const j = Math.floor(Math.random() * (k + 1));
          [o[k], o[j]] = [o[j], o[k]];
        }
        if (o.length > 1 && o[0] === last) o.push(o.shift()!);
        return o;
      };
      let order = shuffle();
      let i = 0;
      const a = new Audio(order[0]);
      a.loop = tracks.length === 1;
      const next = () => {
        const last = order[i];
        i++;
        if (i >= order.length) {
          order = shuffle(last);
          i = 0;
        }
        a.src = order[i];
        a.volume = level.current;
        if (wanted.current) a.play().catch(() => {});
      };
      a.addEventListener("ended", next);
      // a track that won't play is skipped
      a.addEventListener("error", () => tracks.length > 1 && next());
      skipTrack.current = tracks.length > 1 ? next : null;
      audio.current = a;
      setReady(true);
      if (wanted.current) start(a);
    });
    // the menu's hover and click sounds, on every button while setup is open. Web Audio,
    // not <audio>: the window only lets an <audio> play from a click, and a hover never
    // is one; an audio context, once the first click has woken it, plays both at once.
    const Ctx = window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    // ("interactive": the shortest delay the machine can do, for UI sounds)
    const ctx = Ctx ? new Ctx({ latencyHint: "interactive" }) : null;
    const sounds: Record<string, AudioBuffer> = {};
    const loadSound = (name: string, key: string) =>
      fetch(`/api/app/menu-sound/${name}`)
        .then((r) => (r.ok ? r.arrayBuffer() : null))
        .then((buf) => (buf && ctx ? ctx.decodeAudioData(buf) : null))
        .then((b) => b && (sounds[key] = b))
        .catch(() => {});
    loadSound("rollover", "over");
    loadSound("press", "press");
    loadSound("click", "click");
    const play = (which: string, loud: number) => {
      const b = sounds[which];
      if (!ctx || !b || !wanted.current || ctx.state !== "running") return;
      const src = ctx.createBufferSource();
      const gain = ctx.createGain();
      gain.gain.value = Math.min(1, level.current * loud * 1.6);
      src.buffer = b;
      src.connect(gain).connect(ctx.destination);
      src.start();
    };
    const wake = () => {
      if (ctx && ctx.state !== "running") ctx.resume().catch(() => {});
    };
    window.addEventListener("pointerdown", wake);
    window.addEventListener("keydown", wake);
    const BUTTONS = "button:not(:disabled), a.btn, [role=radio]";
    // only when a mouse comes onto a button from outside it (not moving between its
    // parts, not a button redrawn under a still cursor, never a finger)
    const over = (e: PointerEvent) => {
      if (e.pointerType !== "mouse") return;
      const b = (e.target as Element | null)?.closest(BUTTONS);
      const from = e.relatedTarget as Node | null;
      if (b && !(from && b.contains(from))) play("over", 0.8);
    };
    // like the game's menus: one sound as the button goes down, another as it comes up
    const press = (e: PointerEvent) => {
      wake();
      if ((e.target as Element | null)?.closest(BUTTONS)) play("press", 0.9);
    };
    const click = (e: PointerEvent) => {
      if ((e.target as Element | null)?.closest(BUTTONS)) play("click", 1);
    };
    document.addEventListener("pointerover", over);
    document.addEventListener("pointerdown", press);
    document.addEventListener("pointerup", click);
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
      document.removeEventListener("pointerover", over);
      document.removeEventListener("pointerdown", press);
      document.removeEventListener("pointerup", click);
      window.removeEventListener("pointerdown", wake);
      window.removeEventListener("keydown", wake);
      ctx?.close().catch(() => {});
      audio.current?.pause();
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
      <button className="music-toggle" aria-pressed={on} onClick={toggle} title={on ? "Turn the music and menu sounds off" : "Play the menu music and sounds"}>
        <span aria-hidden="true">♪</span> {on ? "Audio on" : "Audio off"}
      </button>
      {on && skipTrack.current && (
        <button className="music-toggle icon" onClick={() => skipTrack.current?.()} title="Shuffle: play another track" aria-label="Shuffle: play another track">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
            <path d="M16 3h5v5M4 20 21 3M21 16v5h-5M15 15l6 6M4 4l5 5" />
          </svg>
        </button>
      )}
      {on && (
        <input
          className="music-volume"
          type="range"
          min={0.05}
          max={1}
          step={0.05}
          value={volume}
          onChange={(e) => setVolume(Number(e.target.value))}
          aria-label="Volume"
          style={{ "--fill": `${Math.round(((volume - 0.05) / 0.95) * 100)}%` } as CSSProperties}
        />
      )}
    </div>
  );
}
