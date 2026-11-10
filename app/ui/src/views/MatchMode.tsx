import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { cmd, getJson, isAbort, isSharedId, ON_THIS_COMPUTER, postJson, SETUP_ON_COMPUTER, useActions, useActivity, useQueuedSave } from "../api";
import { useGameState } from "../queries";
import { useNav } from "../store";
import { openTab } from "./goto";
import { BusyButton, Info, LoadError, SectionHead, Skeleton, Switch, useConfirm, useToast } from "../ui";

type Settings = {
  max_rounds: number;
  overtime: boolean;
  knife: boolean;
  bots: boolean;
  players: number;
  pauses: number;
  pause_seconds: number;
};
type Player = { name: string; team: "CT" | "T" | "SPEC"; bot: number; ready: number; captain: number };
type Status = {
  phase: "none" | "warmup" | "knife" | "vote" | "live" | "over";
  map: string;
  ready: number;
  needed: number;
  ct: number;
  t: number;
  round: number;
  maxrounds: number;
  half: number;
  overtime: number;
  paused: number;
  pause_pending: number;
  pause_left: number;
  vote_side: string;
  players: Player[];
};
type Team = { name: string; score: number; players: string[] };
type Match = { date: string; ts: number; map: string; maxrounds: number; rounds: number; overtime: number; winner: number; teams: Team[] };
type Data = { settings: Settings; status: Status | null; matches: Match[]; current_mode: string; installed: boolean; plugin: boolean };
type Reply = Data | { ok: false; error: string };
const failed = (r: Reply): r is { ok: false; error: string } => "error" in r && !!r.error;

const MODE = "match";
const PLAYERS = [2, 4, 6, 8, 10];
const PAUSES = [0, 1, 2, 3, 4];
const PAUSE_SECONDS = [30, 60, 90, 120, 180];
const PHASE: Record<Status["phase"], string> = {
  none: "Starting up",
  warmup: "Warmup",
  knife: "Knife round",
  vote: "Picking sides",
  live: "Live",
  over: "Match over",
};
const COMMANDS: [string, string][] = [
  ["/ready", "ready to start (/notready takes it back)"],
  ["/score", "the score so far"],
  ["/pause, /unpause", "captains and admins; the pause starts at the next freeze time"],
  ["/start", "admins: start without waiting for everyone"],
  ["/restart", "admins: the match again from 0 - 0"],
  ["/warmup", "admins: stop the match"],
];

/** Where to add something setup installs: Mods ("Run full setup") for your own game; on a
 *  server, Repair in its Mods tab (puts every part of Loner-Strike back, keeping its settings). */
export function setupPlace(server: string | null) {
  if (!ON_THIS_COMPUTER) return SETUP_ON_COMPUTER;
  return server ? "To add it, open this server's Mods tab and click Repair. Its settings, admins and maps stay." : "To add it, use Run full setup in Mods.";
}

/** Something setup adds is missing: says so, where to add it, and (for your own game) a way there. */
export function NeedsSetup({ text, after, className = "panel extras-warn" }: { text: ReactNode; after?: ReactNode; className?: string }) {
  const server = useNav((s) => s.server);
  const go = useNav((s) => s.go);
  return (
    <div className={`${className} needs-setup`}>
      <p>
        {text} {setupPlace(server)}
        {after ? <> {after}</> : null}
      </p>
      {ON_THIS_COMPUTER && !isSharedId(server) && <button className="btn" onClick={() => (server ? openTab("mods") : go("mods"))}>Open Mods</button>}
    </div>
  );
}

/** Match mode: settings, the match on now, and past results. */
export function MatchMode({ target = "local" }: { target?: string } = {}) {
  const toast = useToast();
  const ask = useConfirm();
  const [d, setD] = useState<Data | null>(null);
  const [error, setError] = useState("");
  const { run, busy: isBusy } = useActions();
  const key = (what: string) => `matchmode:${target}:${what}`;
  const local = target === "local";
  // one match action at a time
  const busy = useActivity().some(([k]) => k.startsWith(key("act:")));
  const loading = useRef(false);

  const refresh = useCallback(async () => {
    if (loading.current) return;
    loading.current = true;
    try {
      const r = await getJson<Reply>("/api/scrim");
      if (failed(r)) setError(r.error);
      else {
        setError("");
        setD(r);
      }
    } catch (e) {
      if (!isAbort(e)) setError("Couldn't reach the app.");
    } finally {
      loading.current = false;
    }
  }, []);

  useEffect(() => {
    setD(null);
    refresh();
    const t = setInterval(() => !document.hidden && refresh(), 3000);
    return () => clearInterval(t);
  }, [refresh, target]);

  const queue = useQueuedSave<Settings>();
  const post = async (path: string, body: unknown, done?: string) => {
    const r = await postJson<Reply>(path, body).catch(() => ({ ok: false as const, error: "Couldn't reach the app." }));
    if (failed(r)) {
      toast(r.error, true);
      return false;
    }
    setD(r);
    if (done) toast(done);
    return true;
  };
  const save = (s: Partial<Settings>) => d && queue(d.settings, (cur) => ({ ...cur, ...s }), (full) => post("/api/scrim/settings", full, "Saved"));
  const act = async (action: string, done: string, confirm?: string) => {
    if (busy) return;
    if (confirm && !(await ask(confirm, { ok: CONFIRM_OK[action] ?? "Go ahead", danger: true }))) return;
    await run(key(`act:${action}`), () => post("/api/scrim/action", { action }, done), `${ACTING[action] ?? "Sending"}…`);
  };
  // people playing lose their round when the map restarts; with no player list, always ask
  const { state } = useGameState();
  const people = state?.online ? state.players.filter((p) => !p.bot && !p.host).length : null;
  const switchMode = async () => {
    if (people !== 0 && !(await ask(people ? `Switch to Match mode? The map restarts for the ${people === 1 ? "player" : `${people} players`} on it.` : "Switch to Match mode? The map restarts.", { ok: "Switch and restart" }))) return;
    await switchNow();
  };
  const switchNow = () => run(key("act:switch"), async () => {
    try {
      await cmd("gm_mode", [MODE]);
      toast("Switching to Match mode. The map reloads.");
    } catch (e) {
      toast((e as Error).message, true);
    }
    setTimeout(refresh, 4000);
  }, "Switching to Match mode…");

  if (!d)
    return (
      <div className="stack narrow match">
        <SectionHead title="Match mode" />
        {error ? <LoadError text={error} onRetry={() => (setError(""), refresh())} /> : <Skeleton rows={6} />}
      </div>
    );

  return (
    <div className="stack narrow match">
      {!d.installed && (
        <NeedsSetup className="panel match-note" text={`${local ? "This game" : "This server"} doesn't have Match mode yet.`} after="Your settings here are kept until then." />
      )}
      <Now d={d} local={local} busy={busy} acting={(a) => isBusy(key(`act:${a}`))} act={act} switchMode={switchMode} />
      <MatchSettings s={d.settings} save={save} target={target} />
      <History matches={d.matches} />
      <section>
        <SectionHead title="In-game commands" note="Players type these in chat. A dot works too, like .ready" />
        <dl className="match-commands">
          {COMMANDS.map(([c, what]) => (
            <div key={c}>
              <dt>{c}</dt>
              <dd>{what}</dd>
            </div>
          ))}
        </dl>
      </section>
    </div>
  );
}

type Act = (action: string, done: string, confirm?: string) => void;
/** What the top bar says while a match action is sent. */
const ACTING: Record<string, string> = {
  start: "Starting the match",
  unpause: "Ending the pause",
  pause: "Calling a pause",
  restart: "Restarting the match",
  warmup: "Stopping the match",
};
/** The confirm button for an action that asks first. */
const CONFIRM_OK: Record<string, string> = { restart: "Restart match", warmup: "Stop match" };

function Now({ d, local, busy, acting, act, switchMode }: { d: Data; local: boolean; busy: boolean; acting: (action: string) => boolean; act: Act; switchMode: () => void }) {
  const on = d.current_mode === MODE;
  const s = d.status;
  if (!on) {
    return (
      <section>
        <SectionHead title="Now" />
        <div className="panel match-now">
          <p>Match mode isn't on. Switch to it when your players are on the server; everyone gets a warmup until they type /ready.</p>
          {d.installed && (
            <BusyButton className="btn primary" busy={acting("switch")} disabled={busy} label="Switch to Match mode" busyLabel="Switching…" onClick={switchMode} />
          )}
        </div>
      </section>
    );
  }
  if (!s) {
    return (
      <section>
        <SectionHead title="Now" />
        {d.plugin ? (
          <p className="panel match-now muted">{local ? "The game isn't running" : "The server isn't running"}, or Match mode hasn't answered yet.</p>
        ) : (
          <NeedsSetup className="panel match-now" text={`${local ? "This game" : "This server"} doesn't have Match mode yet.`} />
        )}
      </section>
    );
  }
  const live = s.phase === "live" || s.phase === "over";
  const team = (side: "CT" | "T") => s.players.filter((p) => p.team === side);
  const spec = s.players.filter((p) => p.team === "SPEC");
  return (
    <section>
      <SectionHead title="Now" note={s.map ? `On ${s.map}` : undefined} />
      <div className="panel match-now">
        <div className="match-phase">
          <span className={`match-tag ${s.paused ? "paused" : s.phase}`}>{s.paused ? "Paused" : PHASE[s.phase]}</span>
          {s.phase === "warmup" && <span>{s.ready} of {s.needed} ready</span>}
          {s.phase === "vote" && <span>{s.vote_side === "CT" ? "The CTs" : "The Terrorists"} won the knife round and are picking a side</span>}
          {s.phase === "live" && (
            <span>
              Round {s.round} of {s.maxrounds}
              {s.overtime ? ", overtime" : s.half > 1 ? ", second half" : ", first half"}
            </span>
          )}
          {s.pause_pending ? <span>A pause starts at the next freeze time</span> : null}
          {s.paused && s.pause_left > 0 ? <span>{s.pause_left} seconds before anyone can end it</span> : null}
        </div>
        {live && (
          <div className="match-score" aria-label={`CT ${s.ct}, T ${s.t}`}>
            <span className="ct">CT</span>
            <b>{s.ct}</b>
            <span className="dash">-</span>
            <b>{s.t}</b>
            <span className="t">T</span>
          </div>
        )}
        <div className="match-teams">
          <Roster title="Counter-Terrorists" players={team("CT")} warmup={s.phase === "warmup"} />
          <Roster title="Terrorists" players={team("T")} warmup={s.phase === "warmup"} />
        </div>
        {spec.length > 0 && <p className="muted match-spec">Watching: {spec.map((p) => p.name).join(", ")}</p>}
        <div className="row match-actions">
          {s.phase === "warmup" && (
            <BusyButton className="btn primary" busy={acting("start")} disabled={busy} label="Start now" busyLabel="Starting…" onClick={() => act("start", "Starting the match")} />
          )}
          {s.phase === "live" &&
            (s.paused || s.pause_pending ? (
              <BusyButton className="btn primary" busy={acting("unpause")} disabled={busy} label="End the pause" busyLabel="Ending…" onClick={() => act("unpause", "Pause ended")} />
            ) : (
              <BusyButton busy={acting("pause")} disabled={busy} label="Pause" busyLabel="Pausing…" onClick={() => act("pause", "Pause called")} />
            ))}
          {s.phase !== "warmup" && s.phase !== "none" && (
            <>
              <BusyButton
                busy={acting("restart")}
                disabled={busy}
                label={s.phase === "knife" || s.phase === "vote" ? "Redo the knife round" : "Restart the match"}
                busyLabel="Restarting…"
                onClick={() => act("restart", "Restarted", s.phase === "live" ? "Restart the match from 0 - 0?" : undefined)}
              />
              <BusyButton
                className="btn danger"
                busy={acting("warmup")}
                disabled={busy}
                label="Stop the match"
                busyLabel="Stopping…"
                onClick={() => act("warmup", "Back to warmup", s.phase === "live" ? "Stop the match and go back to warmup? The score is lost." : undefined)}
              />
            </>
          )}
        </div>
      </div>
    </section>
  );
}

function Roster({ title, players, warmup }: { title: string; players: Player[]; warmup: boolean }) {
  return (
    <div className="match-roster">
      <h3>{title}</h3>
      {players.length === 0 ? (
        <p className="muted">Nobody yet</p>
      ) : (
        <ul>
          {players.map((p, i) => (
            <li key={`${p.name}-${i}`}>
              <span className="match-name">{p.name}</span>
              {p.captain ? <><span className="match-pill">Captain</span><Info text="Usually the player on this side who typed /ready first. The captain calls the team's tactical pauses." /></> : null}
              {p.bot ? <span className="match-pill dim">Bot</span> : null}
              {warmup && !p.bot && <span className={`match-pill ${p.ready ? "ok" : "dim"}`}>{p.ready ? "Ready" : "Not ready"}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function MatchSettings({ s, save: send, target }: { s: Settings; save: (s: Partial<Settings>) => unknown; target: string }) {
  // each setting is off while its own change saves
  const { run, busy } = useActions();
  const key = (s: Partial<Settings>) => `matchmode:${target}:setting:${Object.keys(s).join(",")}`;
  const save = (s: Partial<Settings>) => run(key(s), async () => { await send(s); }, "Saving match settings…");
  const saving = (k: keyof Settings) => busy(key({ [k]: true }));
  return (
    <section>
      <SectionHead
        title="Match settings"
        note="These apply from the next match. Round time, start money, freeze time and friendly fire are the mode's own settings, like in any other mode: $800, 1:45 and friendly fire on to begin with."
      />
      <div className="settings">
        <Choice
          label="Length"
          note="First to 16 (MR15) or first to 13 (MR12). Teams swap sides at half-time."
          values={[30, 24]}
          current={s.max_rounds}
          busy={saving("max_rounds")}
          show={(v) => (v === 30 ? "MR15, 30 rounds" : "MR12, 24 rounds")}
          pick={(v) => save({ max_rounds: v })}
        />
        <Toggle label="Overtime" note="On a tie: blocks of 6 rounds with $10,000, first to 4 wins. Off: a tie is a draw." on={s.overtime} busy={saving("overtime")} set={(v) => save({ overtime: v })} />
        <Toggle label="Knife round" note="Knives only before the match. The winners vote to stay or switch sides." on={s.knife} busy={saving("knife")} set={(v) => save({ knife: v })} />
        <Choice
          label="Players to start"
          note="Both teams together. With bots on, the match starts once everyone playing is ready (bots always are), up to this many."
          values={PLAYERS}
          current={s.players}
          busy={saving("players")}
          show={(v) => `${Math.floor(v / 2)}v${Math.ceil(v / 2)}`}
          pick={(v) => save({ players: v })}
        />
        <Toggle
          label="Bots"
          note="On: bots play as your bot settings say (how many, and Fixed, Fill server or Per human), like in every other mode, and always count as ready. Off: people only. Changes right away."
          on={s.bots}
          busy={saving("bots")}
          set={(v) => save({ bots: v })}
        />
        <Choice label="Pauses per team" note="Tactical pauses a captain can call in one match. Admins can always pause." values={PAUSES} current={s.pauses} busy={saving("pauses")} show={(v) => (v ? `${v}` : "None")} pick={(v) => save({ pauses: v })} />
        <Choice
          label="Pause length"
          note="After this, anyone can end a team's pause by typing /unpause."
          values={PAUSE_SECONDS}
          current={s.pause_seconds}
          busy={saving("pause_seconds")}
          show={(v) => (v < 60 ? `${v} sec` : v % 60 ? `${Math.floor(v / 60)}:${String(v % 60).padStart(2, "0")}` : `${v / 60} min`)}
          pick={(v) => save({ pause_seconds: v })}
        />
      </div>
    </section>
  );
}

function History({ matches }: { matches: Match[] }) {
  return (
    <section>
      <SectionHead title="Past matches" note={matches.length ? "The last 50, newest first." : undefined} />
      {matches.length === 0 ? (
        <p className="muted">No finished matches yet. Results show up here when a match ends.</p>
      ) : (
        <ul className="match-history">
          {matches.filter((m) => m.teams?.length >= 2).map((m, i) => (
            <li key={`${m.ts}-${i}`} className="panel">
              <div className="match-history-head">
                <b>{m.map}</b>
                <span className="muted">
                  {m.date}
                  {m.overtime ? ", overtime" : ""}
                </span>
              </div>
              <div className="match-history-teams">
                <Side team={m.teams[0]} won={m.winner === 1} />
                <span className="match-history-score">
                  {m.teams[0].score} - {m.teams[1].score}
                  {m.winner === 0 && <small>Draw</small>}
                </span>
                <Side team={m.teams[1]} won={m.winner === 2} />
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}

function Side({ team, won }: { team: Team; won: boolean }) {
  return (
    <div className={`match-side${won ? " won" : ""}`}>
      <b>{team.name}</b>
      {won && <span className="match-pill ok">Won</span>}
      <span className="muted">{team.players.length ? team.players.join(", ") : "No names"}</span>
    </div>
  );
}

function Toggle({ label, note, on, set, busy }: { label: string; note: ReactNode; on: boolean; set: (v: boolean) => void; busy?: boolean }) {
  return (
    <div className="setting">
      <span className="name">{label}<span className="note">{note}</span></span>
      <Switch on={on} onChange={set} label={label} disabled={busy} />
    </div>
  );
}

function Choice({ label, note, values, current, show, pick, busy }: { label: string; note: string; values: number[]; current: number; show: (v: number) => string; pick: (v: number) => void; busy?: boolean }) {
  const all = values.includes(current) ? values : [...values, current].sort((a, b) => a - b);
  return (
    <div className="setting">
      <span className="name">{label}<span className="note">{note}</span></span>
      <div className="seg" role="group" aria-label={label}>
        {all.map((v) => (
          <button key={v} type="button" aria-pressed={current === v} disabled={busy} onClick={() => pick(v)}>{show(v)}</button>
        ))}
      </div>
    </div>
  );
}
