import { useEffect, useMemo, useRef, useState } from "react";
import { getJson, postJson } from "../api";
import { Icon } from "../icons";
import { useToast } from "../ui";

// ---- what the setup API returns -------------------------------------------------
export type SetupGame = { id: "cstrike" | "czero"; name: string; installed: boolean; components: string[]; foreign: boolean };
export type SetupInstall = { hl: string; kind: string; bottle: string | null; games: SetupGame[] };
type CatalogItem = {
  id: string;
  name: string;
  kind: "mode" | "addon";
  description?: string;
  games?: string[];
  bots?: "great" | "ok" | "weak" | "none";
};
export type Scan = { installs: SetupInstall[]; catalog: CatalogItem[]; native_mac: boolean };
type Step = { id: string; label: string; state: "pending" | "running" | "done" | "error"; detail?: string; game?: string };
type Job = { id?: string; state: "idle" | "running" | "done" | "error"; kind?: string; steps: Step[]; progress: number; error?: string; failed_compiles?: string[]; notes?: string[] };

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

export function Setup({ scan, onClose }: { scan: Scan; onClose?: () => void }) {
  const toast = useToast();
  const [stage, setStage] = useState<Stage>("welcome");
  const [installIdx, setInstallIdx] = useState(0);
  const install = scan.installs[installIdx];
  const [games, setGames] = useState<string[]>(() => (install?.games.some((g) => g.id === "cstrike") ? ["cstrike"] : install?.games.slice(0, 1).map((g) => g.id) ?? []));
  const [preset, setPreset] = useState<"all" | "basics" | "pick">("all");
  const usable = useMemo(() => scan.catalog.filter((c) => !c.games || c.games.some((g) => games.includes(g))), [scan.catalog, games]);
  const [picked, setPicked] = useState<Set<string>>(() => new Set(scan.catalog.map((c) => c.id)));
  const [job, setJob] = useState<Job | null>(null);
  const timer = useRef<number>();

  const chosen = preset === "all" ? usable : preset === "basics" ? [] : usable.filter((c) => picked.has(c.id));

  const start = async () => {
    const r = await postJson<{ ok?: boolean; job?: string; error?: string }>("/api/setup/install", {
      hl: install.hl,
      games,
      components: chosen.map((c) => c.id),
    }).catch((): { ok?: boolean; job?: string; error?: string } => ({ error: "Setup couldn't start." }));
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

  const finish = async (play: boolean) => {
    const g = games[0];
    await postJson("/api/app/game", { hl: install.hl, game: g }).catch(() => null);
    if (play) await postJson("/api/app/launch", {}).catch(() => null);
    // the app restarts into the new setup
  };

  if (!scan.installs.length) return <NoGame nativeMac={scan.native_mac} />;

  return (
    <div className="setup">
      <div className="setup-card">
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
            <img src="/logo.svg" alt="" width={112} height={112} />
            <h1>Loner-Strike</h1>
            <p className="tagline">Re-live the golden days of Counter-Strike locally or with the friends you don't have.</p>
            <p>Setup adds bots that play like people, 20 classic community modes and this control panel to your game. Everything it changes is backed up and can be removed again.</p>
            <p className="muted">Close Counter-Strike before you start.</p>
            <div className="row">
              <button className="btn primary big-btn" onClick={() => setStage("games")}>Set it up</button>
              {onClose && <button className="btn" onClick={onClose}>Not now</button>}
            </div>
          </section>
        )}

        {stage === "games" && (
          <section className="setup-pane">
            <h2>Which game?</h2>
            {scan.installs.length > 1 && (
              <label className="field">
                <span>Steam install</span>
                <select value={installIdx} onChange={(e) => { setInstallIdx(+e.target.value); setGames([]); }}>
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
            <p className="muted">It downloads from the original projects (GameBanana for maps), compiles the plugins, and backs up every file it replaces. Make sure Counter-Strike is closed.</p>
            <Nav back={() => setStage("content")} next={start} nextLabel="Install" />
          </section>
        )}

        {stage === "install" && (
          <section className="setup-pane">
            <h2>{job?.state === "error" ? "Setup stopped" : "Installing…"}</h2>
            <progress className="setup-progress" max={1} value={job?.progress ?? 0} />
            <ul className="job-steps">
              {(job?.steps ?? []).map((s) => (
                <li key={`${s.game}-${s.id}`} className={s.state}>
                  <span className="dot-state" aria-hidden="true">{s.state === "done" ? "✓" : s.state === "error" ? "!" : s.state === "running" ? "•" : ""}</span>
                  <span>
                    {s.label}
                    {s.game && games.length > 1 ? <small> · {s.game === "cstrike" ? "CS 1.6" : "CZ"}</small> : null}
                    {s.detail && <small className="muted"> {s.detail}</small>}
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
            <h1>You're set.</h1>
            <p>Start the game and Loner-Strike connects by itself.</p>
            <ul className="tips">
              <li><b>F3</b> in game opens the menu: modes, maps, bots and settings. <b>F4</b> is the quick menu.</li>
              <li>The first time you play a map, the bots spend a few minutes learning it.</li>
              <li>Change modes, maps and bots here any time, or from your phone (Settings → Use on your phone).</li>
            </ul>
            {!!job?.failed_compiles?.length && <p className="err-text">These didn't build, so they won't load: {job.failed_compiles.join(", ")}.</p>}
            {job?.notes?.map((n) => <p key={n} className="muted">{n}</p>)}
            <div className="row">
              <button className="btn primary big-btn" onClick={() => finish(true)}>Play {games[0] === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero"}</button>
              <button className="btn" onClick={() => finish(false)}>Open the panel</button>
            </div>
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

function NoGame({ nativeMac }: { nativeMac: boolean }) {
  return (
    <div className="setup">
      <div className="setup-card">
        <section className="setup-pane welcome">
          <img src="/logo.svg" alt="" width={96} height={96} />
          <h1>No game found</h1>
          {nativeMac ? (
            <p>You have the Mac version of Half-Life, which mods can't run on. Install the Windows version of Steam in CrossOver, install Counter-Strike 1.6 or Condition Zero there, then reopen Loner-Strike.</p>
          ) : (
            <p>Loner-Strike needs Counter-Strike 1.6 or Condition Zero from Steam{navigator.platform.startsWith("Mac") ? ", installed through Steam in CrossOver" : ""}. Install one, then reopen Loner-Strike.</p>
          )}
        </section>
      </div>
    </div>
  );
}
