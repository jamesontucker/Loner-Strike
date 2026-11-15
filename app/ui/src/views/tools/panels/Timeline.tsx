// Replay controls: play/pause, speed, a scrubber with round and kill/fall marks, the
// players (click to pick, double-click to follow) and what just happened.

import { useEffect, useMemo, useRef } from "react";
import { clock } from "../../../api";
import { useTools } from "../../../stores/tools";
import { stateAt, type Replay } from "../overlays";
import { TEAM_COLOUR } from "../viewer/ReplayLayer";

const SPEEDS = [1, 2, 4, 8, 16];

export function Timeline({ replay }: { replay: Replay }) {
  const time = useTools((s) => s.time);
  const playing = useTools((s) => s.playing);
  const speed = useTools((s) => s.speed);
  const follow = useTools((s) => s.follow);
  const sel = useTools((s) => s.selection);
  const { setTime, setPlaying, setSpeed, setFollow, select } = useTools.getState();
  const span = Math.max(0.1, replay.end - replay.start);

  // start at the first sample
  useEffect(() => {
    if (useTools.getState().time < replay.start) setTime(replay.start);
  }, [replay, setTime]);

  // the clock: moves time on while playing, stops at the end
  const last = useRef(0);
  useEffect(() => {
    if (!playing) return;
    let raf = 0;
    last.current = performance.now();
    const tick = (now: number) => {
      const dt = Math.min(0.1, (now - last.current) / 1000);
      last.current = now;
      const s = useTools.getState();
      const t = s.time + dt * s.speed;
      if (t >= replay.end) {
        s.setTime(replay.end);
        s.setPlaying(false);
        return;
      }
      s.setTime(t);
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [playing, replay]);

  // Space plays and pauses
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement | null;
      if (e.code !== "Space" || (t && ["INPUT", "TEXTAREA", "SELECT", "BUTTON"].includes(t.tagName))) return;
      e.preventDefault();
      const s = useTools.getState();
      if (s.time >= replay.end) s.setTime(replay.start);
      s.setPlaying(!s.playing);
    };
    window.addEventListener("keydown", on);
    return () => window.removeEventListener("keydown", on);
  }, [replay]);

  const pct = (t: number) => `${((t - replay.start) / span) * 100}%`;
  const name = useMemo(() => new Map(replay.players.map((p) => [p.id, p.name])), [replay]);
  const recent = replay.kills.filter((k) => k.t <= time && k.t > time - 20).slice(-5).reverse();
  const players = [...replay.players].sort((a, b) => a.team - b.team || a.name.localeCompare(b.name));

  return (
    <section className="tools-panel" aria-label="Replay">
      <h3>Replay</h3>
      <div className="row">
        <button className="btn small primary" onClick={() => {
          if (time >= replay.end) setTime(replay.start);
          setPlaying(!playing);
        }}>{playing ? "Pause" : "Play"}</button>
        <span className="mono small">{clock(Math.floor(Math.max(0, time - replay.start)))} / {clock(Math.floor(span))}</span>
        <label className="sort push">
          <span>Speed</span>
          <select value={speed} onChange={(e) => setSpeed(Number(e.target.value))}>
            {SPEEDS.map((s) => <option key={s} value={s}>{s}×</option>)}
          </select>
        </label>
      </div>
      <div className="tools-scrub">
        <div className="tools-ticks" aria-hidden="true">
          {replay.rounds.map((r, i) => <i key={`r${i}`} className="round" style={{ left: pct(r.start) }} />)}
          {replay.kills.map((k, i) => <i key={`k${i}`} className={k.fall || k.cause === "world" || k.cause === "trigger_hurt" ? "fall" : "kill"} style={{ left: pct(k.t) }} />)}
        </div>
        <input type="range" aria-label="Time" min={replay.start} max={replay.end} step={0.1} value={Math.min(replay.end, Math.max(replay.start, time))} onChange={(e) => setTime(Number(e.target.value))} />
      </div>
      <p className="small-note muted">Red marks are kills, pink ones are falls. Space plays and pauses.</p>
      <h4>Players</h4>
      <ul className="tools-links tools-players">
        {players.map((p) => {
          const alive = !!stateAt(replay, p, time);
          const picked = sel?.kind === "bot" && sel.id === p.id;
          return (
            <li key={p.id} className={alive ? "" : "muted"}>
              <button type="button" className="link inline" style={{ color: TEAM_COLOUR[p.team] }} aria-pressed={picked} onClick={() => select({ kind: "bot", id: p.id })}>{p.name}</button>
              {alive && (
                <button type="button" className={`chip tools-follow${follow === p.id ? " on" : ""}`} aria-pressed={follow === p.id} title="Keep the camera on this player (Orbit view)" onClick={() => {
                  setFollow(follow === p.id ? null : p.id);
                  if (follow !== p.id && useTools.getState().camera !== "orbit") useTools.getState().setCamera("orbit");
                }}>{follow === p.id ? "Following" : "Follow"}</button>
              )}
              {!alive && <span className="small"> dead</span>}
            </li>
          );
        })}
      </ul>
      {recent.length > 0 && (
        <>
          <h4>Just now</h4>
          <ul className="tools-links">
            {recent.map((k, i) => (
              <li key={i}>
                <button type="button" className="link inline" onClick={() => {
                  setPlaying(false);
                  setTime(Math.max(replay.start, (k.fall?.fromT ?? k.t) - 2));
                  useTools.getState().look(k.at, 200);
                }}>
                  {k.fall ? `${name.get(k.victim) ?? "?"} fell (${Math.round(k.fall.drop)} units)` : k.cause === "world" || k.cause === "trigger_hurt" ? `${name.get(k.victim) ?? "?"} died to the map` : `${name.get(k.killer) ?? "?"} killed ${name.get(k.victim) ?? "?"}`}
                </button>
              </li>
            ))}
          </ul>
        </>
      )}
      <FallList replay={replay} names={name} />
    </section>
  );
}

/** Every fall in the recording, to jump to: the "where do they fall" question. */
function FallList({ replay, names }: { replay: Replay; names: Map<number, string> }) {
  const falls = replay.kills.filter((k) => k.fall || k.cause === "world" || k.cause === "trigger_hurt");
  if (!falls.length) return null;
  return (
    <details className="tools-groups">
      <summary>Falls and map deaths <span className="muted">{falls.length}</span></summary>
      <ul className="tools-links">
        {falls.map((k, i) => (
          <li key={i}>
            <button type="button" className="link inline" onClick={() => {
              const s = useTools.getState();
              s.setPlaying(false);
              s.setTime(Math.max(replay.start, (k.fall?.fromT ?? k.t) - 2));
              s.look(k.fall?.from ?? k.at, 300);
            }}>{clock(Math.floor(Math.max(0, k.t - replay.start)))} {names.get(k.victim) ?? "?"}{k.fall ? ` · dropped ${Math.round(k.fall.drop)}` : ` · ${k.cause}`}</button>
          </li>
        ))}
      </ul>
    </details>
  );
}
