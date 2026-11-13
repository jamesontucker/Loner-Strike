// The Playtest panel: is the game talking to us, where the world is paused (with that node's
// inputs and the live values), Resume, Disable all breakpoints, and the recent events.

import { useStudio } from "./store";
import { nodeDef } from "./registry";
import { describe } from "./debugState";
import { Values } from "./Inspector";

export function DebugPanel({ onResume, onBpsOff, onRefresh, onStop, focus }: { onResume: () => void; onBpsOff: (off: boolean) => void; onRefresh: () => void; onStop: () => void; focus: (id: string) => void }) {
  const graph = useStudio((s) => s.graph);
  const d = useStudio((s) => s.debug);
  const bpsOff = useStudio((s) => s.bpsOff);
  const title = (id: string) => nodeDef(graph?.nodes.find((n) => n.id === id)?.type ?? "")?.title ?? id;
  const connected = d.seen > 0;
  return (
    <section className="st-debug" aria-label="Playtest">
      <div className="st-debug-bar">
        <span className={`st-dot${connected ? " on" : ""}${d.paused ? " paused" : ""}`} />
        <strong>{d.paused ? "Paused" : connected ? `Playing${d.map ? ` on ${d.map}` : ""}` : "Waiting for the game…"}</strong>
        {!connected && <span className="st-muted">The game is starting the mode; this connects when the map has loaded.</span>}
        <span className="st-grow" />
        <label className="st-side-check st-inline">
          <input type="checkbox" checked={bpsOff} onChange={(e) => onBpsOff(e.target.checked)} />
          <span>Disable all breakpoints</span>
        </label>
        <button type="button" className="btn primary" disabled={!d.paused} onClick={onResume}>
          Resume
        </button>
        <button type="button" className="btn" onClick={onStop} title="Stop following the game (the game keeps running)">
          Stop debugging
        </button>
      </div>
      <div className="st-debug-body">
        <div className="st-debug-col">
          {d.paused ? (
            <>
              <h4>
                Paused at{" "}
                <button type="button" className="st-link" onClick={() => focus(d.paused!.node)}>
                  {title(d.paused.node)}
                </button>
              </h4>
              <p className="st-muted">Everyone is frozen and the round clock is stopped. It runs when you resume. (Only the world pauses: there's no stepping through the code.)</p>
              {d.paused.inputs && Object.keys(d.paused.inputs).length > 0 ? <Values values={d.paused.inputs} /> : <p className="st-muted">This node has no inputs.</p>}
            </>
          ) : (
            <p className="st-muted">Nodes light up as they run; the wires show the path the last flow took. Add a breakpoint (the dot on a node, or F9) to pause the world there.</p>
          )}
        </div>
        <div className="st-debug-col">
          <h4>
            Live values{" "}
            <button type="button" className="st-link" onClick={onRefresh}>
              Refresh
            </button>
          </h4>
          {d.vars ? (
            <>
              <div className="st-vars">
                <span>Round {d.vars.round}</span>
                <span>{d.vars.phase === "freeze" ? "Freeze time" : d.vars.phase === "ended" ? "Round over" : "Playing"}</span>
                <span>
                  {Math.floor(d.vars.timeLeft / 60)}:{String(d.vars.timeLeft % 60).padStart(2, "0")} left
                </span>
                <span>
                  Alive: {d.vars.aliveT} T · {d.vars.aliveCT} CT
                </span>
              </div>
              <table className="st-players">
                <thead>
                  <tr>
                    <th>Player</th>
                    <th>Team</th>
                    <th>Health</th>
                    <th>Weapon</th>
                  </tr>
                </thead>
                <tbody>
                  {d.vars.players.map((p) => (
                    <tr key={p.id} className={p.alive ? "" : "dead"}>
                      <td>
                        {p.name}
                        {p.bot ? <span className="st-muted"> (bot)</span> : null}
                      </td>
                      <td>{p.team}</td>
                      <td>{p.alive ? p.hp : "dead"}</td>
                      <td>{p.weapon || "–"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </>
          ) : (
            <p className="st-muted">Shown when the world is paused, or press Refresh.</p>
          )}
        </div>
        <div className="st-debug-col st-log">
          <h4>What happened</h4>
          <ol>
            {d.log
              .slice()
              .reverse()
              .slice(0, 40)
              .map((ev) => (
                <li key={`${ev.s}-${ev.t}`} className={ev.e === "node_error" ? "err" : ev.e === "breakpoint_hit" ? "paused" : ""}>
                  <span className="st-muted">{ev.t.toFixed(1)}s</span> {describe(ev, title)}
                </li>
              ))}
          </ol>
        </div>
      </div>
    </section>
  );
}
