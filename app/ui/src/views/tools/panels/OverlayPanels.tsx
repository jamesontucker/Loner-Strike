// The Inspector for the bots' data: a picked nav area (with what bots learned there) and a
// picked player in a replay (with the brain's "why" at that moment, from the trace).

import { useMemo } from "react";
import { copyText } from "../../../api";
import { useTools } from "../../../stores/tools";
import { useToast } from "../../../ui";
import { attrWords, stateAt, traceRow, type Know, type Nav, type Replay, type Trace } from "../overlays";
import { fmtVec } from "../viewer/coords";
import { knowFacts } from "../viewer/KnowLayer";
import { TEAM_COLOUR } from "../viewer/ReplayLayer";

const DIRS = ["North", "East", "South", "West"];

export function AreaPanel({ nav, know, index }: { nav: Nav; know: Know | null; index: number }) {
  const { select, look } = useTools.getState();
  const toast = useToast();
  const a = nav.areas[index];
  if (!a) return null;
  const goTo = (id: number) => {
    const j = nav.index.get(id);
    if (j === undefined) return;
    select({ kind: "area", index: j });
    look(nav.centres[j], 200);
  };
  const oneWayOut = nav.oneWay.filter(([f]) => f === a.id).map(([, t]) => t);
  const oneWayIn = nav.oneWay.filter(([, t]) => t === a.id).map(([f]) => f);
  const moves = attrWords(nav, a.attr);
  const size = `${Math.round(a.se[0] - a.nw[0])} × ${Math.round(a.se[1] - a.nw[1])}`;
  const knowOk = know && know.count === nav.areas.length && know.keyMatches !== false;
  return (
    <section className="tools-panel" aria-label="Picked">
      <h3>Nav area {a.id}</h3>
      {a.place > 0 && <p className="small-note muted">{nav.places[a.place - 1]}</p>}
      <div className="row">
        <button className="btn small" onClick={() => look(nav.centres[index], 200)}>Look at</button>
        <button className="btn small" onClick={async () => toast((await copyText(fmtVec(nav.centres[index]))) ? `Centre copied: ${fmtVec(nav.centres[index])}` : "Couldn't copy.")}>Copy centre</button>
        <button className="btn small" onClick={() => select(null)}>Clear</button>
      </div>
      <dl className="tools-keys">
        <dt>Size</dt>
        <dd>{size} units</dd>
        <dt>Height</dt>
        <dd className="mono">{Math.round(Math.min(a.nw[2], a.se[2], a.neZ, a.swZ))} to {Math.round(Math.max(a.nw[2], a.se[2], a.neZ, a.swZ))}</dd>
        <dt>Moves</dt>
        <dd>{moves.length ? moves.join(", ") : "walk"}</dd>
        <dt>Group</dt>
        <dd>{nav.island[index] === 0 ? "connected to the rest" : <span className="tools-warn">cut off from the rest</span>}</dd>
        <dt>Hiding spots</dt>
        <dd>{a.hiding.length}</dd>
        {knowOk && knowFacts(know, index).map(([k, v]) => (
          <div key={k} className="tools-key">
            <dt>{k}</dt>
            <dd>{v}</dd>
          </div>
        ))}
      </dl>
      <h4>Leads to</h4>
      <dl className="tools-keys">
        {a.conns.map((ids, d) => (
          <div key={d} className="tools-key">
            <dt>{DIRS[d]}</dt>
            <dd className="mono">
              {ids.length ? ids.map((id, k) => (
                <span key={id}>
                  {k > 0 && " "}
                  <button type="button" className={`link inline${oneWayOut.includes(id) ? " tools-oneway" : ""}`} title={oneWayOut.includes(id) ? "One way: there's no way back from it" : undefined} onClick={() => goTo(id)}>{id}</button>
                </span>
              )) : <span className="muted">—</span>}
            </dd>
          </div>
        ))}
      </dl>
      {oneWayIn.length > 0 && (
        <p className="small-note muted">
          One-way in from{" "}
          {oneWayIn.map((id, k) => (
            <span key={id}>{k > 0 && ", "}<button type="button" className="link inline" onClick={() => goTo(id)}>{id}</button></span>
          ))}
          .
        </p>
      )}
    </section>
  );
}

export function BotPanel({ replay, trace, id }: { replay: Replay; trace: Trace | null; id: number }) {
  const time = useTools((s) => s.time);
  const { select, look, setFollow } = useTools.getState();
  const p = replay.players.find((x) => x.id === id);
  const s = p ? stateAt(replay, p, time) : null;
  const row = trace ? traceRow(trace, id, time) : null;
  const notes = useMemo(() => (trace ? trace.why.filter((w) => w.id === id && w.t <= time + trace.offset).slice(-5).reverse() : []), [trace, id, time]);
  if (!p) return null;
  const task = row && trace ? trace.tasks[Number(row.task)] : undefined;
  const fmtT = (t: number) => `${Math.max(0, Math.round(time + (trace?.offset ?? 0) - t))} s ago`;
  return (
    <section className="tools-panel" aria-label="Picked">
      <h3 style={{ color: TEAM_COLOUR[p.team] }}>{p.name}</h3>
      <p className="small-note muted">{p.bot ? "Bot" : "Player"} · {p.team === 1 ? "Terrorist" : p.team === 2 ? "Counter-Terrorist" : "no team"}{s ? ` · ${s.hp} hp` : " · dead"}</p>
      <div className="row">
        {s && <button className="btn small" onClick={() => look(s.pos, 120)}>Look at</button>}
        {s && <button className="btn small" onClick={() => setFollow(id)}>Follow</button>}
        <button className="btn small" onClick={() => select(null)}>Clear</button>
      </div>
      {!trace ? (
        <p className="small-note muted">{replay.hasTrace ? "Reading the bot's thoughts…" : "This recording has no bot trace, so there's no “why” to show."}</p>
      ) : row ? (
        <>
          <h4>Thinking</h4>
          <dl className="tools-keys">
            <dt>Doing</dt>
            <dd>{String(row.state)}{task ? ` · ${task}` : ""}</dd>
            {row.why ? (
              <div className="tools-key">
                <dt>Why</dt>
                <dd>{String(row.why)}{Number(row.why_ago) ? <span className="muted"> ({Math.round(Number(row.why_ago))} s ago)</span> : null}</dd>
              </div>
            ) : null}
            {row.look ? (
              <div className="tools-key">
                <dt>Looking</dt>
                <dd>{String(row.look)}</dd>
              </div>
            ) : null}
            {Number(row.enemy) > 0 && (
              <div className="tools-key">
                <dt>Enemy</dt>
                <dd>{replay.players.find((x) => x.id === Number(row.enemy))?.name ?? `#${row.enemy}`}{Number(row.attacking) ? " (attacking)" : ""}</dd>
              </div>
            )}
          </dl>
          <p className="small-note muted">The yellow line is where it's heading; a yellow post is a noise it heard.</p>
        </>
      ) : (
        <p className="small-note muted">No trace for this moment{trace.matches ? "" : " (the trace doesn't match this recording)"}.</p>
      )}
      {notes.length > 0 && (
        <>
          <h4>Recent notes</h4>
          <ul className="tools-links">
            {notes.map((n, i) => <li key={i}>{n.why} <span className="muted small">{fmtT(n.t)}</span></li>)}
          </ul>
        </>
      )}
    </section>
  );
}
