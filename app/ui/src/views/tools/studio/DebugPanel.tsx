// The Playtest panel: is the game talking to us, where the world is paused (with that node's
// inputs and the live values), Resume, Disable all breakpoints, and the recent events.

import { useEffect, useState } from "react";
import { useStudio } from "./store";
import { nodeDef } from "./registry";
import { describe } from "./debugState";
import { Values } from "./Inspector";
import { allVariables } from "./pins";
import { updateVariable } from "./graphOps";
import * as api from "./studioApi";
import type { VarValue } from "./debugState";
import type { Graph } from "./schema";

/** Who a live value belongs to, for a live tweak: 0 the mode's, a player's index, 1 / 2 a team's. */
const whoOf = (x: VarValue) => (x.v.endsWith(":T") ? 1 : x.v.endsWith(":CT") ? 2 : x.p);

/** A live value with Change (debug builds take it at once, no recompile) and Use as start value. */
function LiveValue({ x, graph, playerName }: { x: VarValue; graph: Graph | null; playerName: string }) {
  const id = x.v.split(":")[0];
  const v = graph ? allVariables(graph).find((y) => y.id === id) : undefined;
  const en = v?.type.startsWith("enum:") ? graph?.enums?.find((e) => `enum:${e.id}` === v.type) : undefined;
  const editable = !!v && (["number", "bool", "text"].includes(v.type) || !!en);
  const shown = typeof x.value === "boolean" ? (x.value ? "yes" : "no") : String(x.value);
  const [draft, setDraft] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const playing = useStudio((s) => s.playing);
  const start = () => {
    setErr("");
    if (!v) return;
    if (v.type === "bool") setDraft(x.value === true || x.value === "yes" ? "1" : "0");
    else if (en) setDraft(String(Math.max(0, en.values.indexOf(shown))));
    else setDraft(shown);
  };
  const send = async () => {
    if (!v || draft === null || !playing) return;
    setBusy(true);
    const r = await api.debugCommand({ mode: playing, set: { var: v.id, who: whoOf(x), value: draft, graph } }).catch((e: Error) => ({ error: e.message }));
    setBusy(false);
    if (r && "error" in r && r.error) setErr(r.error);
    else setDraft(null);
  };
  const asStart = () => {
    if (!v) return;
    const d = v.type === "number" ? Number(x.value) : v.type === "bool" ? x.value === true || x.value === "yes" : en ? Math.max(0, en.values.indexOf(shown)) : String(x.value);
    useStudio.getState().edit((g) => updateVariable(g, v.id, { default: d as never }));
  };
  return (
    <div>
      <dt>
        {v?.name ?? x.v}
        {x.v.includes(":") ? ` (${x.v.split(":")[1]})` : ""}
        {x.p ? ` (${playerName})` : ""}
      </dt>
      <dd>
        {draft === null ? (
          <>
            {shown}
            {editable && (
              <>
                {" "}
                <button type="button" className="st-link" onClick={start} title="Change it in the running game now (no recompile)">
                  Change
                </button>{" "}
                <button type="button" className="st-link" onClick={asStart} title="Make this the variable's start value in the graph">
                  Use as start value
                </button>
              </>
            )}
          </>
        ) : (
          <form
            className="st-live-edit"
            onSubmit={(e) => {
              e.preventDefault();
              void send();
            }}
          >
            {v?.type === "bool" ? (
              <select className="st-field" value={draft} onChange={(e) => setDraft(e.target.value)} aria-label={`New value for ${v.name}`}>
                <option value="1">yes</option>
                <option value="0">no</option>
              </select>
            ) : en ? (
              <select className="st-field" value={draft} onChange={(e) => setDraft(e.target.value)} aria-label={`New value for ${v?.name}`}>
                {en.values.map((val, i) => (
                  <option key={val} value={i}>
                    {val}
                  </option>
                ))}
              </select>
            ) : (
              <input className="st-field" autoFocus type={v?.type === "number" ? "number" : "text"} maxLength={60} value={draft} onChange={(e) => setDraft(e.target.value)} aria-label={`New value for ${v?.name}`} />
            )}
            <button type="submit" className="btn small primary" disabled={busy}>
              {busy ? (
                <>
                  <span className="spinner" /> Setting…
                </>
              ) : (
                "Set"
              )}
            </button>
            <button type="button" className="btn small" onClick={() => setDraft(null)}>
              Cancel
            </button>
            {err && <span className="st-bad-text">{err}</span>}
          </form>
        )}
      </dd>
    </div>
  );
}

/** "You set Score (Bob) to 5" for the log. */
function setText(ev: { v?: unknown; p?: unknown; value?: unknown }, graph: Graph | null): string {
  const v = graph ? allVariables(graph).find((y) => y.id === ev.v) : undefined;
  return `You set ${v?.name ?? String(ev.v)} to ${String(ev.value)}${v?.scope === "player" ? ` (player ${String(ev.p)})` : v?.scope === "team" ? ` (${ev.p === 1 ? "T" : "CT"})` : ""}`;
}

export function DebugPanel({ onResume, onBpsOff, onRefresh, onStop, focus }: { onResume: () => void; onBpsOff: (off: boolean) => void; onRefresh: () => void; onStop: () => void; focus: (id: string) => void }) {
  const graph = useStudio((s) => s.graph);
  const d = useStudio((s) => s.debug);
  const bpsOff = useStudio((s) => s.bpsOff);
  const since = useStudio((s) => s.playingSince);
  const [, tick] = useState(0);
  useEffect(() => {
    if (d.seen > 0) return;
    const t = setInterval(() => tick((n) => n + 1), 1000);
    return () => clearInterval(t);
  }, [d.seen]);
  const waited = Math.floor((Date.now() - since) / 1000);
  const title = (id: string) => nodeDef(graph?.nodes.find((n) => n.id === id)?.type ?? "")?.title ?? id;
  const connected = d.seen > 0;
  return (
    <section className="st-debug" aria-label="Playtest">
      <div className="st-debug-bar">
        <span className={`st-dot${connected ? " on" : ""}${d.paused ? " paused" : ""}`} />
        <strong>{d.paused ? "Paused" : connected ? `Playing${d.map ? ` on ${d.map}` : ""}` : `Waiting for the game… ${waited} s`}</strong>
        {!connected && (
          <span className="st-muted">
            {waited < 20
              ? "Starting the game and loading the map; this connects when the mode is running."
              : waited < 90
                ? "Still starting: the first start (Steam, CrossOver, the map) can take a minute."
                : "Nothing yet: check the game's window (an error, or the main menu?). Stop debugging to give up."}
          </span>
        )}
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
            <p className="st-muted">Nodes light up as they run; the wires show the path the last flow took. Add a breakpoint (the dot on a node, or F9) to pause the world there. (A Playtest build records every node it runs: don't judge the mode's speed by it; Install makes the fast build.)</p>
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
              {d.vars.values.length > 0 && (
                <dl className="st-values">
                  {d.vars.values.map((x) => (
                    <LiveValue key={`${x.v}-${x.p}`} x={x} graph={graph} playerName={d.vars!.players.find((p) => p.id === x.p)?.name ?? `player ${x.p}`} />
                  ))}
                </dl>
              )}
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
                  <span className="st-muted">{ev.t.toFixed(1)}s</span> {ev.e === "var_set" ? setText(ev as unknown as { v?: unknown; p?: unknown; value?: unknown }, graph) : describe(ev, title)}
                </li>
              ))}
          </ol>
        </div>
      </div>
    </section>
  );
}
