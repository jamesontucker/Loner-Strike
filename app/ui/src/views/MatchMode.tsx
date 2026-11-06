import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { cmd, getJson, postJson } from "../api";
import { SectionHead, Skeleton, Switch, useConfirm, useToast } from "../ui";

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

/** Match mode: settings, the match on now, and past results. */
export function MatchMode({ target = "local" }: { target?: string } = {}) {
  const toast = useToast();
  const ask = useConfirm();
  const [d, setD] = useState<Data | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
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
    } catch {
      setError("Couldn't reach the app.");
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

  const post = async (path: string, body: unknown, done?: string) => {
    setBusy(true);
    const r = await postJson<Reply>(path, body).catch(() => ({ ok: false as const, error: "Couldn't reach the app." }));
    setBusy(false);
    if (failed(r)) {
      toast(r.error, true);
      return;
    }
    setD(r);
    if (done) toast(done);
  };
  const save = (s: Partial<Settings>) => d && post("/api/scrim/settings", { ...d.settings, ...s }, "Saved");
  const act = async (action: string, done: string, confirm?: string) => {
    if (confirm && !(await ask(confirm, { ok: "Yes", danger: true }))) return;
    post("/api/scrim/action", { action }, done);
  };
  const switchMode = async () => {
    setBusy(true);
    try {
      await cmd("gm_mode", [MODE]);
      toast("Switching to Match mode. The map reloads.");
    } catch (e) {
      toast((e as Error).message, true);
    }
    setBusy(false);
    setTimeout(refresh, 4000);
  };

  if (error && !d) return <p className="muted">{error}</p>;
  if (!d) return <Skeleton rows={6} />;

  return (
    <div className="stack narrow match">
      {!d.installed && (
        <p className="panel match-note">This game doesn't have Match mode yet. Run Setup again from Settings to add it. Your settings here are kept until then.</p>
      )}
      <Now d={d} busy={busy} act={act} switchMode={switchMode} />
      <MatchSettings s={d.settings} save={save} />
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

function Now({ d, busy, act, switchMode }: { d: Data; busy: boolean; act: Act; switchMode: () => void }) {
  const on = d.current_mode === MODE;
  const s = d.status;
  if (!on) {
    return (
      <section>
        <SectionHead title="Now" />
        <div className="panel match-now">
          <p>Match mode isn't on. Switch to it when your players are on the server; everyone gets a warmup until they type /ready.</p>
          {d.installed && (
            <button className="btn primary" disabled={busy} onClick={switchMode}>Switch to Match mode</button>
          )}
        </div>
      </section>
    );
  }
  if (!s) {
    return (
      <section>
        <SectionHead title="Now" />
        <p className="panel match-now muted">
          {d.plugin ? "The game isn't running, or the match plugin hasn't answered yet." : "The match plugin isn't installed. Run Setup again from Settings."}
        </p>
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
            <button className="btn primary" disabled={busy} onClick={() => act("start", "Starting the match")}>Start now</button>
          )}
          {s.phase === "live" &&
            (s.paused || s.pause_pending ? (
              <button className="btn primary" disabled={busy} onClick={() => act("unpause", "Pause ended")}>End the pause</button>
            ) : (
              <button className="btn" disabled={busy} onClick={() => act("pause", "Pause called")}>Pause</button>
            ))}
          {s.phase !== "warmup" && s.phase !== "none" && (
            <>
              <button className="btn" disabled={busy} onClick={() => act("restart", "Restarted", s.phase === "live" ? "Restart the match from 0 - 0?" : undefined)}>
                {s.phase === "knife" || s.phase === "vote" ? "Redo the knife round" : "Restart the match"}
              </button>
              <button className="btn danger" disabled={busy} onClick={() => act("warmup", "Back to warmup", s.phase === "live" ? "Stop the match and go back to warmup? The score is lost." : undefined)}>
                Stop the match
              </button>
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
              {p.captain ? <span className="match-pill">Captain</span> : null}
              {p.bot ? <span className="match-pill dim">Bot</span> : null}
              {warmup && !p.bot && <span className={`match-pill ${p.ready ? "ok" : "dim"}`}>{p.ready ? "Ready" : "Not ready"}</span>}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function MatchSettings({ s, save }: { s: Settings; save: (s: Partial<Settings>) => void }) {
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
          show={(v) => (v === 30 ? "MR15, 30 rounds" : "MR12, 24 rounds")}
          pick={(v) => save({ max_rounds: v })}
        />
        <Toggle label="Overtime" note="On a tie: blocks of 6 rounds with $10000, first to 4 wins. Off: a tie is a draw." on={s.overtime} set={(v) => save({ overtime: v })} />
        <Toggle label="Knife round" note="Knives only before the match. The winners vote to stay or switch sides." on={s.knife} set={(v) => save({ knife: v })} />
        <Choice
          label="Players to start"
          note="Both teams together. The match starts once this many are ready."
          values={PLAYERS}
          current={s.players}
          show={(v) => `${Math.floor(v / 2)}v${Math.ceil(v / 2)}`}
          pick={(v) => save({ players: v })}
        />
        <Toggle
          label="Fill with bots"
          note="Off: no bots in this mode. On: bots fill the empty spots during warmup and always count as ready. Changes right away."
          on={s.bots}
          set={(v) => save({ bots: v })}
        />
        <Choice label="Pauses per team" note="Tactical pauses a captain can call in one match. Admins can always pause." values={PAUSES} current={s.pauses} show={(v) => (v ? `${v}` : "None")} pick={(v) => save({ pauses: v })} />
        <Choice
          label="Pause length"
          note="After this, anyone can end a team's pause by typing /unpause."
          values={PAUSE_SECONDS}
          current={s.pause_seconds}
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
          {matches.map((m, i) => (
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

function Toggle({ label, note, on, set }: { label: string; note: ReactNode; on: boolean; set: (v: boolean) => void }) {
  return (
    <div className="setting">
      <span className="name">{label}<span className="note">{note}</span></span>
      <Switch on={on} onChange={set} label={label} />
    </div>
  );
}

function Choice({ label, note, values, current, show, pick }: { label: string; note: string; values: number[]; current: number; show: (v: number) => string; pick: (v: number) => void }) {
  const all = values.includes(current) ? values : [...values, current].sort((a, b) => a - b);
  return (
    <div className="setting">
      <span className="name">{label}<span className="note">{note}</span></span>
      <div className="seg" role="group" aria-label={label}>
        {all.map((v) => (
          <button key={v} type="button" aria-pressed={current === v} onClick={() => pick(v)}>{show(v)}</button>
        ))}
      </div>
    </div>
  );
}
