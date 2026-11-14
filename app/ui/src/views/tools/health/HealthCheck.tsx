// The map simulator: pick a map, bots play it on a hidden server for a few minutes, and a
// report says in plain words what went wrong (falls, stuck spots, errors), shows on the map
// where bots went, fought, died and never went, then rounds, sides and the numbers.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNav } from "../../../store";
import { useTools } from "../../../stores/tools";
import type { MapInfo } from "../../../types";
import { BusyButton, Info, LoadError, Segmented, Skeleton, SteamProgress, useToast } from "../../../ui";
import { Empty, Loading, Row, Rows } from "../common";
import { matches, searchWords } from "../common/search";
import type { ToolProps } from "../registry";
import { useReportAddress } from "../common/address";
import { useToolCrumbs } from "../common/shell";
import {
  healthOptions,
  healthProgress,
  healthReport,
  healthReports,
  recordingId,
  startHealth,
  stopHealth,
  type Finding,
  type Progress,
  type Report,
  type ReportSummary,
  type Spot,
} from "./healthApi";
import { SimMap } from "./SimMap";
import "./health.css";

const MINUTES = ["2", "5", "10", "15"];
/** the server name a run made from the game's own files has (health.rs Source) */
const FROM_GAME_NAME = "your game";
const BOTS = ["6", "12", "20"];

/** "4 min", "1 min 20 s", "45 s" */
function duration(secs: number) {
  const s = Math.max(0, Math.round(secs));
  if (s < 60) return `${s} s`;
  return s % 60 ? `${Math.floor(s / 60)} min ${s % 60} s` : `${s / 60} min`;
}

const when = (unix: number) => new Date(unix * 1000).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
const capital = (t: string) => t.charAt(0).toUpperCase() + t.slice(1);
const where = (s: Spot) => s.place || `the spot at ${s.at.map((v) => Math.round(v)).join(" ")}`;

/** Opens the map viewer on the map, looking at a spot. */
function openInViewer(map: string, s: Spot) {
  useNav.getState().go("tools", `map:${map}@${s.at.map((v) => Math.round(v)).join(",")},192`);
}

export default function HealthCheck({ maps }: ToolProps) {
  const qc = useQueryClient();
  const progress = useQuery({
    queryKey: ["tools", "health", "progress"],
    queryFn: healthProgress,
    refetchInterval: (q) => ((q.state.data as Progress | undefined)?.active ? 1000 : false),
  });
  const reports = useQuery({ queryKey: ["tools", "health", "reports"], queryFn: healthReports });
  // a report is its own page: "health:report:<id>" (so Back and the crumbs step out of it)
  const [open, setOpen] = useState<string | null>(() => useTools.getState().arg?.match(/^report:(.+)$/)?.[1] ?? null);
  const run = progress.data?.run ?? null;
  const active = !!progress.data?.active;

  // a run watched here that just finished opens its report
  const watching = useRef<string | null>(null);
  useEffect(() => {
    if (!run) return;
    if (active) {
      watching.current = run.id;
      return;
    }
    if (watching.current !== run.id) return;
    watching.current = null;
    qc.invalidateQueries({ queryKey: ["tools", "health", "reports"] });
    if (run.phase === "done" && run.report) setOpen(run.report);
  }, [run, active, qc]);

  if (open) return <ReportView id={open} onBack={() => setOpen(null)} />;

  return (
    <div className="tl-page health-page">
      {active && run ? <Running p={progress.data!} /> : <Setup maps={maps} onStarted={() => progress.refetch()} />}
      {!active && run && (run.phase === "failed" || run.phase === "stopped") && (
        <div className={`panel health-ended ${run.phase}`} role="status">
          <strong>{run.phase === "failed" ? `The simulation of ${run.map} didn't finish.` : `The simulation of ${run.map} was stopped.`}</strong>
          {run.error && <p>{run.error}</p>}
        </div>
      )}
      <div className="panel">
        <h3>Earlier simulations</h3>
        {reports.isPending ? (
          <Skeleton rows={2} />
        ) : reports.isError || reports.data?.error ? (
          <LoadError text={reports.data?.error ?? "Couldn't load the earlier simulations."} onRetry={() => reports.refetch()} />
        ) : (reports.data?.reports ?? []).length === 0 ? (
          <Empty plain text="None yet. Run a simulation above and its report shows up here, with a recording you can watch in Replay." />
        ) : (
          <ul className="health-list">
            {(reports.data?.reports ?? []).map((r) => (
              <SummaryRow key={r.id} r={r} onOpen={() => setOpen(r.id)} />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function Verdict({ v }: { v: Report["verdict"] }) {
  const text = v === "good" ? "Looks fine" : v === "warn" ? "Worth a look" : "Problems";
  return <span className={`health-verdict ${v}`}>{text}</span>;
}

function SummaryRow({ r, onOpen }: { r: ReportSummary; onOpen: () => void }) {
  return (
    <li>
      <button type="button" className="health-row" onClick={onOpen}>
        <span className="health-row-map">
          <strong>{r.map}</strong>
          <span className="muted">
            {r.modeName || r.mode} · {r.bots} bots · {r.minutes} min · {when(r.when)}
          </span>
        </span>
        <span className="health-row-head">{r.headline}</span>
        <Verdict v={r.verdict} />
      </button>
    </li>
  );
}

function Setup({ maps, onStarted }: { maps: MapInfo[]; onStarted: () => void }) {
  const toast = useToast();
  const [map, setMap] = useState<string>(() => (useTools.getState().arg?.startsWith("report:") ? null : useTools.getState().arg) ?? useTools.getState().map ?? "");
  useReportAddress("health", map ? `health:${map}` : null);
  const [filter, setFilter] = useState("");
  const [mode, setMode] = useState<string | null>(null);
  const [minutes, setMinutes] = useState("5");
  const [bots, setBots] = useState("12");
  const [server, setServer] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  const list = useMemo(() => {
    const w = searchWords(filter);
    return maps
      .filter((m) => !m.hidden && matches(w, m.name, m.title))
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [maps, filter]);

  const opts = useQuery({ queryKey: ["tools", "health", "options", map], queryFn: () => healthOptions(map), enabled: !!map });
  const o = opts.data && !opts.data.error ? opts.data : null;
  // the map's own mode (map presets) until one is picked
  useEffect(() => setMode(null), [map]);
  const pickedMode = mode ?? o?.mode ?? "classic";
  const blocked = o?.why ?? o?.busy ?? null;

  const go = async () => {
    if (!o) return;
    setStarting(true);
    const r = await startHealth({ map: o.map, mode: pickedMode, minutes: Number(minutes), bots: Number(bots), server: server ?? o.server });
    setStarting(false);
    if (r.error || !r.ok) return toast(r.error || "Couldn't start the simulation.", true);
    onStarted();
  };

  return (
    <div className="panel health-setup">
      <h3>Simulate a match</h3>
      <p className="muted measure">
        Bots play the map on a hidden server for a few minutes while you do something else. Then you get a report with the map from above: where bots went and never went, where fights happen, where they fall or get stuck, which side wins and how long rounds last, and any errors. The game and your servers need to be closed while it runs.
      </p>
      <Rows>
        <Row name="Map" wide>
          <input className="search" type="search" placeholder="Find a map" value={filter} onChange={(e) => setFilter(e.target.value)} aria-label="Find a map" />
          <select value={map} onChange={(e) => setMap(e.target.value)} aria-label="Map">
              <option value="" disabled>
                Choose a map
              </option>
              {list.map((m) => (
                <option key={m.name} value={m.name}>
                  {m.name}
                  {m.nav ? "" : " (bots don't know it yet)"}
                </option>
              ))}
          </select>
        </Row>
        {map && opts.isPending && <Loading className="health-wait" text="Reading the map…" />}
        {map && (opts.isError || opts.data?.error) && <LoadError text={opts.data?.error ?? "Couldn't read that map's details."} onRetry={() => opts.refetch()} />}
        {o && (
          <>
            {o.modes.length > 0 && (
              <Row name="Game mode" note="The map's own mode is picked for you">
                <select value={pickedMode} onChange={(e) => setMode(e.target.value)} aria-label="Game mode">
                  {o.modes.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.name}
                    </option>
                  ))}
                </select>
              </Row>
            )}
            <Row name="How long" note="Longer runs find rarer problems and give fairer round and side numbers">
              <Segmented label="How long" values={MINUTES.join(" ")} labels={MINUTES.map((m) => `${m} min`).join("|")} current={minutes} onPick={setMinutes} />
            </Row>
            <Row name="Bots">
              <Segmented label="Bots" values={BOTS.join(" ")} labels={BOTS.join("|")} current={bots} onPick={setBots} />
            </Row>
            {o.servers.length > 1 && (
              <Row name="Plugins and modes from" info="The simulation runs on a hidden copy of one of your servers on this computer, so the bots play with its plugins, modes and settings. Your server itself isn't touched.">
                <select value={server ?? o.server ?? ""} onChange={(e) => setServer(e.target.value)} aria-label="Server to copy">
                  {o.servers.map((s) => (
                    <option key={s.id} value={s.id}>
                      {s.name}
                    </option>
                  ))}
                </select>
              </Row>
            )}
          </>
        )}
      </Rows>
      {blocked && <p className="health-blocked">{blocked}</p>}
      <div className="row health-actions">
        <BusyButton className="btn primary" busy={starting} disabled={!o || !!blocked} label="Run the simulation" busyLabel="Starting…" onClick={go} />
        {o && !blocked && (
          <span className="muted">
            About {duration(Number(minutes) * 60 + 60)}{o.fromGame ? ", plus a few minutes to set the hidden server up" : ""}. {o.fromGame ?? (o.servers.length === 1 ? `Uses a hidden copy of your server “${o.servers[0].name}”.` : "")}
          </span>
        )}
        {!map && <span className="muted">Choose a map first.</span>}
      </div>
    </div>
  );
}

const PHASE: Record<string, string> = {
  preparing: "Getting the hidden server ready",
  starting: "Starting the server",
  playing: "Bots are playing",
  reading: "Reading the recording",
};

function Running({ p }: { p: Progress }) {
  const toast = useToast();
  const [stopping, setStopping] = useState(false);
  const run = p.run!;
  const stop = async () => {
    setStopping(true);
    const r = await stopHealth();
    if (r.error) {
      setStopping(false);
      toast(r.error, true);
    }
  };
  return (
    <div className="panel health-running" aria-live="polite">
      <SteamProgress
        value={p.fraction ?? 0}
        title={`Simulating ${run.map} · ${run.modeName || run.mode}`}
        text={run.detail || PHASE[run.phase] || "Working"}
        foot={p.left ? `About ${duration(p.left)} left` : "Almost done"}
      />
      <div className="row between health-running-foot">
        <span className="muted">
          {PHASE[run.phase] ?? ""} · {run.bots} bots for {run.minutes} min, {run.server === FROM_GAME_NAME ? "on a hidden temporary server made from your game" : `on a hidden copy of “${run.server}”`}. You can leave this page; it carries on.
        </span>
        <BusyButton className="btn" busy={stopping} label="Stop" busyLabel="Stopping…" onClick={stop} />
      </div>
    </div>
  );
}

/** "Watch in Replay" at the moment a spot's worst case began (a little before), following that bot. */
function WatchButton({ s, recording }: { s: Spot; recording: string | null }) {
  const toast = useToast();
  const [opening, setOpening] = useState(false);
  if (s.t == null || !recording) return null;
  const watch = async () => {
    setOpening(true);
    const rid = await recordingId(recording);
    setOpening(false);
    if (!rid) return toast("That recording isn't in the recordings folder any more.", true);
    useNav.getState().go("tools", `replay:${rid}@${Math.max(0, s.t! - 3).toFixed(1)}${s.bot ? `,${s.bot}` : ""}`);
  };
  return <BusyButton className="btn small" busy={opening} label="Watch in Replay" busyLabel="Opening…" onClick={watch} />;
}

function FindingRow({ f, map, recording }: { f: Finding; map: string; recording: string | null }) {
  return (
    <li className={`health-finding ${f.level}`}>
      <span className="health-dot" aria-hidden="true" />
      <span className="health-finding-text">{f.text}</span>
      {f.spot && (
        <span className="row health-finding-actions">
          <WatchButton s={f.spot} recording={recording} />
          <button type="button" className="btn small" onClick={() => openInViewer(map, f.spot!)}>
            Open in map viewer
          </button>
        </span>
      )}
    </li>
  );
}

function Stat({ label, value, note, help }: { label: string; value: string; note?: string; help?: string }) {
  return (
    <div className="health-stat">
      <span className="health-stat-label">
        {label}
        {help && <Info text={help} />}
      </span>
      <span className="health-stat-value">{value}</span>
      {note && <span className="health-stat-note">{note}</span>}
    </div>
  );
}

function SpotList({ title, spots, map, what, recording }: { title: string; spots: Spot[]; map: string; what: (s: Spot) => string; recording: string | null }) {
  if (spots.length === 0) return null;
  return (
    <div className="panel">
      <h3>{title}</h3>
      <ul className="health-spots">
        {spots.map((s, i) => (
          <li key={i}>
            <span className="health-spot-place">{capital(where(s))}</span>
            <span className="muted">{what(s)}</span>
            <WatchButton s={s} recording={recording} />
            <button type="button" className="btn small" onClick={() => openInViewer(map, s)}>
              Open in map viewer
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

const KNOWLEDGE: Record<string, string> = { loaded: "Loaded", rebuilt: "Didn't fit", none: "Not loaded" };
const pct = (v: number | null | undefined) => (v == null ? "—" : `${v.toFixed(v < 10 ? 1 : 0)}%`);

function ReportView({ id, onBack }: { id: string; onBack: () => void }) {
  const toast = useToast();
  const q = useQuery({ queryKey: ["tools", "health", "report", id], queryFn: () => healthReport(id) });
  const [replaying, setReplaying] = useState(false);
  useReportAddress("health", `health:report:${id}`);
  const reportMap = q.data?.report?.map;
  useToolCrumbs("health", reportMap ? [{ label: reportMap, address: `health:${reportMap}`, title: `Simulate ${reportMap} again` }, { label: "Report" }] : [{ label: "Report" }]);
  if (q.isPending) return <Skeleton rows={6} />;
  const r = q.data?.report;
  if (q.isError || !r)
    return (
      <div className="tl-page health-page">
        <button type="button" className="btn" onClick={onBack}>
          Back
        </button>
        <LoadError text={q.data?.error ?? "Couldn't load that report."} onRetry={() => q.refetch()} />
      </div>
    );

  const replay = async () => {
    if (!r.recording) return;
    setReplaying(true);
    const rid = await recordingId(r.recording);
    setReplaying(false);
    if (!rid) return toast("That recording isn't in the recordings folder any more.", true);
    useNav.getState().go("tools", `replay:${rid}`);
  };

  return (
    <div className="tl-page health-page">
      <div className="panel health-report-head">
        <div className="row between">
          <div>
            <h3>
              {r.map} <Verdict v={r.verdict} />
            </h3>
            <p className="muted">
              {r.modeName || r.mode} · {r.botsWanted} bots · {duration(r.seconds)} recorded · {when(r.when)}
              {r.server ? (r.server === FROM_GAME_NAME ? " · plugins and modes from your game" : ` · plugins and modes from “${r.server}”`) : ""}
            </p>
          </div>
          <div className="row">
            {r.recording && <BusyButton className="btn" busy={replaying} label="Watch in Replay" busyLabel="Opening…" onClick={replay} />}
            <button type="button" className="btn" onClick={onBack}>
              Back
            </button>
          </div>
        </div>
        <ul className="health-findings">
          {r.findings.map((f, i) => (
            <FindingRow key={i} f={f} map={r.map} recording={r.recording} />
          ))}
        </ul>
      </div>

      <div className="panel">
        <h3>The map</h3>
        <SimMap r={r} />
      </div>

      <RoundsPanel r={r} />
      <FightList r={r} />

      <div className="panel">
        <h3>The numbers</h3>
        <div className="health-stats">
          <Stat label="Bots that played" value={`${r.botsSeen} of ${r.botsWanted}`} />
          <Stat label="Kills a bot-minute" value={r.killsPerBotMin.toFixed(2)} note={`${r.kills} kills, ${Math.round(r.botMinutes)} bot-minutes`} help="Kills for every minute a bot was alive and in play. Fighting modes usually sit around 0.6 to 1.2." />
          <Stat label="Stuck" value={pct(r.stuckPct)} note="of bot time" help="Trying to move (walking keys held) but staying within a few steps of one spot for 8 seconds or more, with no enemy in sight. Holding an angle, camping, planting and defusing don't count." />
          <Stat label="Standing still" value={pct(r.stillPct)} note="with no enemy in sight" help="Some is normal: holding an angle, waiting for a teammate." />
          <Stat label="Map used" value={pct(r.coveragePct)} note={r.navAreas ? `of ${r.navAreas} nav areas` : "no nav"} help="How much of the bots' nav they walked over. Low means parts of the map are hard to reach or not worth going to." />
          <Stat label="Never left spawn" value={`${r.spawnSitters} of ${r.botRounds}`} note="times in a round" />
          <Stat label="Deaths to the map" value={`${r.worldDeaths} of ${r.deaths}`} note={`${r.falls} ${r.falls === 1 ? "fall" : "falls"}`} />
          <Stat label="Map knowledge" value={KNOWLEDGE[r.knowledge] ?? r.knowledge} help="What the bots learned about the map's cover and angles. 'Didn't fit' means the saved file is for a different version of the map or nav." />
          <Stat label="Plugin errors" value={String(r.pluginErrors)} />
        </div>
        {r.compared.length > 0 && <p className="muted health-compared">Compared with other {r.modeName || r.mode} maps simulated on this computer.</p>}
      </div>

      <SpotList title="Where bots fell" recording={r.recording} spots={r.fallSpots} map={r.map} what={(s) => `${s.count} ${s.count === 1 ? "time" : "times"}${s.drop > 0 ? ` · drops of up to ${Math.round(s.drop)} units` : ""}`} />
      <SpotList title="Where bots got stuck" recording={r.recording} spots={r.stuckSpots} map={r.map} what={(s) => `${duration(s.seconds)} in all · ${s.count} ${s.count === 1 ? "time" : "times"}`} />
      <SpotList title="Where the map killed bots" recording={r.recording} spots={r.hurtSpots} map={r.map} what={(s) => `${s.count} ${s.count === 1 ? "time" : "times"} (hurt areas, crushers, explosions)`} />

      {(r.pluginErrorList.length > 0 || r.fatal.length > 0) && (
        <div className="panel">
          <h3>From the server's console</h3>
          <ul className="health-console">
            {r.fatal.map((l, i) => (
              <li key={`f${i}`}>
                <strong>Engine:</strong> <code>{l}</code>
              </li>
            ))}
            {r.pluginErrorList.map((p) => (
              <li key={p.plugin}>
                <strong>{p.plugin}</strong> ({p.count} {p.count === 1 ? "error" : "errors"}): <code>{p.first}</code>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

/** Rounds and sides: who won how often, kills by side, how long rounds last. */
function RoundsPanel({ r }: { r: Report }) {
  const rounds = r.rounds ?? [];
  const t = rounds.filter((x) => x.winner === 1).length;
  const ct = rounds.filter((x) => x.winner === 2).length;
  const decided = t + ct;
  const tk = r.tKills ?? 0, ctk = r.ctKills ?? 0;
  if (rounds.length === 0 && tk + ctk === 0) return null;
  const secs = rounds.map((x) => x.seconds).sort((a, b) => a - b);
  const mid = secs.length ? secs[Math.floor(secs.length / 2)] : 0;
  const max = secs.length ? secs[secs.length - 1] : 0;
  return (
    <div className="panel">
      <h3>Rounds and sides</h3>
      {decided > 0 ? (
        <div className="sim-sides" role="img" aria-label={`Terrorists won ${t} rounds, CTs won ${ct}`}>
          <span className="sim-side t" style={{ flexGrow: Math.max(t, 0.001) }}>
            {t > 0 && `T ${t}`}
          </span>
          <span className="sim-side ct" style={{ flexGrow: Math.max(ct, 0.001) }}>
            {ct > 0 && `CT ${ct}`}
          </span>
        </div>
      ) : (
        <p className="muted">Who won each round wasn't recorded{rounds.length ? "" : ": no round finished"}. {rounds.length ? "Reports from before this version, and modes without round wins, leave it out." : ""}</p>
      )}
      <div className="health-stats sim-round-stats">
        {decided > 0 && <Stat label="Rounds won" value={`T ${t} · CT ${ct}`} note={`${Math.round((100 * t) / decided)}% to the Terrorists`} help="With bots on both sides, a big lean points at the map: one side runs further, or there's a choke it can't take." />}
        <Stat label="Kills by side" value={`T ${tk} · CT ${ctk}`} note="enemies killed" />
        {secs.length > 0 && <Stat label="Round length" value={duration(mid)} note={`middle of ${secs.length}; ${duration(secs[0])} to ${duration(max)}`} help="From the end of the freeze time to the round's end." />}
      </div>
      {secs.length > 1 && (
        <div className="sim-round-strip" aria-label="Each round's length, in order">
          {rounds.map((x, i) => (
            <span key={i} className={`sim-round w${x.winner}`} style={{ height: `${Math.max(6, (100 * x.seconds) / Math.max(1, max))}%` }} title={`Round ${i + 1}: ${duration(x.seconds)}${x.winner ? `, ${x.winner === 1 ? "T" : "CT"} win` : ""}`} />
          ))}
        </div>
      )}
    </div>
  );
}

/** Where fights end most: the map's choke points, as the bots play it. */
function FightList({ r }: { r: Report }) {
  const spots = r.fightSpots ?? [];
  if (spots.length === 0) return null;
  return (
    <div className="panel">
      <h3>
        Where fights happen <Info text="Where bots were killed by enemies, by place: the map's choke points, as bots play it. The split says which side lost more there." />
      </h3>
      <ul className="health-spots">
        {spots.map((s, i) => (
          <li key={i}>
            <span className="health-spot-place">{capital(s.place || `the spot at ${s.at.map((v) => Math.round(v)).join(" ")}`)}</span>
            <span className="muted">
              {s.deaths} {s.deaths === 1 ? "death" : "deaths"} · {s.tDeaths} T, {s.ctDeaths} CT
            </span>
            <button type="button" className="btn small" onClick={() => openInViewer(r.map, { at: s.at, place: s.place, count: s.deaths, seconds: 0, drop: 0 })}>
              Open in map viewer
            </button>
          </li>
        ))}
      </ul>
      {(r.unvisitedPlaces ?? []).length > 0 && (
        <p className="muted sim-never">
          Nobody went to: {r.unvisitedPlaces!.join(", ")}.
        </p>
      )}
    </div>
  );
}
