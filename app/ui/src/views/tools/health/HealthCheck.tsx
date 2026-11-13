// The map health check: pick a map, bots play it on a hidden server for a few minutes, and a
// report says in plain words what went wrong (falls, stuck spots, errors), then the numbers.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { useNav } from "../../../store";
import { useTools } from "../../../stores/tools";
import type { MapInfo } from "../../../types";
import { BusyButton, Info, LoadError, Segmented, Skeleton, SteamProgress, useToast } from "../../../ui";
import { Empty, Loading, Row, Rows } from "../common";
import type { ToolProps } from "../registry";
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
import "./health.css";

const MINUTES = ["2", "5", "10", "15"];
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
  const [open, setOpen] = useState<string | null>(null);
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
          <strong>{run.phase === "failed" ? `The check of ${run.map} didn't finish.` : `The check of ${run.map} was stopped.`}</strong>
          {run.error && <p>{run.error}</p>}
        </div>
      )}
      <div className="panel">
        <h3>Earlier checks</h3>
        {reports.isPending ? (
          <Skeleton rows={2} />
        ) : reports.isError || reports.data?.error ? (
          <LoadError text={reports.data?.error ?? "Couldn't load the earlier checks."} onRetry={() => reports.refetch()} />
        ) : (reports.data?.reports ?? []).length === 0 ? (
          <Empty plain text="None yet. Run a check above and its report shows up here, with a recording you can watch in Replay." />
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
  const [map, setMap] = useState<string>(() => useTools.getState().map ?? "");
  const [filter, setFilter] = useState("");
  const [mode, setMode] = useState<string | null>(null);
  const [minutes, setMinutes] = useState("5");
  const [bots, setBots] = useState("12");
  const [server, setServer] = useState<string | null>(null);
  const [starting, setStarting] = useState(false);

  const list = useMemo(() => {
    const f = filter.trim().toLowerCase();
    return maps
      .filter((m) => !m.hidden && (!f || m.name.toLowerCase().includes(f) || (m.title ?? "").toLowerCase().includes(f)))
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
    if (r.error || !r.ok) return toast(r.error || "Couldn't start the check.", true);
    onStarted();
  };

  return (
    <div className="panel health-setup">
      <h3>Check a map</h3>
      <p className="muted measure">
        Bots play the map on a hidden server for a few minutes while you do something else, then you get a report: where they fall, where they get stuck, how much of the map they use, and any errors. The game and your servers need to be closed while it runs.
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
            <Row name="How long" note="Longer runs find rarer problems">
              <Segmented label="How long" values={MINUTES.join(" ")} labels={MINUTES.map((m) => `${m} min`).join("|")} current={minutes} onPick={setMinutes} />
            </Row>
            <Row name="Bots">
              <Segmented label="Bots" values={BOTS.join(" ")} labels={BOTS.join("|")} current={bots} onPick={setBots} />
            </Row>
            {o.servers.length > 1 && (
              <Row name="Plugins and modes from" info="The check runs on a hidden copy of one of your servers on this computer, so the bots play with its plugins, modes and settings. Your server itself isn't touched.">
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
        <BusyButton className="btn primary" busy={starting} disabled={!o || !!blocked} label="Run the check" busyLabel="Starting…" onClick={go} />
        {o && !blocked && (
          <span className="muted">
            About {duration(Number(minutes) * 60 + 60)}. {o.servers.length === 1 ? `Uses a hidden copy of your server “${o.servers[0].name}”.` : ""}
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
        title={`Checking ${run.map} · ${run.modeName || run.mode}`}
        text={run.detail || PHASE[run.phase] || "Working"}
        foot={p.left ? `About ${duration(p.left)} left` : "Almost done"}
      />
      <div className="row between health-running-foot">
        <span className="muted">
          {PHASE[run.phase] ?? ""} · {run.bots} bots for {run.minutes} min, on a hidden copy of “{run.server}”. You can leave this page; it carries on.
        </span>
        <BusyButton className="btn" busy={stopping} label="Stop" busyLabel="Stopping…" onClick={stop} />
      </div>
    </div>
  );
}

function FindingRow({ f, map }: { f: Finding; map: string }) {
  return (
    <li className={`health-finding ${f.level}`}>
      <span className="health-dot" aria-hidden="true" />
      <span className="health-finding-text">{f.text}</span>
      {f.spot && (
        <button type="button" className="btn small" onClick={() => openInViewer(map, f.spot!)}>
          Open in map viewer
        </button>
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

function SpotList({ title, spots, map, what }: { title: string; spots: Spot[]; map: string; what: (s: Spot) => string }) {
  if (spots.length === 0) return null;
  return (
    <div className="panel">
      <h3>{title}</h3>
      <ul className="health-spots">
        {spots.map((s, i) => (
          <li key={i}>
            <span className="health-spot-place">{capital(where(s))}</span>
            <span className="muted">{what(s)}</span>
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
              {r.server ? ` · plugins and modes from “${r.server}”` : ""}
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
            <FindingRow key={i} f={f} map={r.map} />
          ))}
        </ul>
      </div>

      <div className="panel">
        <h3>The numbers</h3>
        <div className="health-stats">
          <Stat label="Bots that played" value={`${r.botsSeen} of ${r.botsWanted}`} />
          <Stat label="Kills a bot-minute" value={r.killsPerBotMin.toFixed(2)} note={`${r.kills} kills, ${Math.round(r.botMinutes)} bot-minutes`} help="Kills for every minute a bot was alive and in play. Fighting modes usually sit around 0.6 to 1.2." />
          <Stat label="Stuck" value={pct(r.stuckPct)} note="of bot time" help="Barely moving for 8 seconds or more with no enemy in sight, during play." />
          <Stat label="Standing still" value={pct(r.stillPct)} note="with no enemy in sight" help="Some is normal: holding an angle, waiting for a teammate." />
          <Stat label="Map used" value={pct(r.coveragePct)} note={r.navAreas ? `of ${r.navAreas} nav areas` : "no nav"} help="How much of the bots' nav they walked over. Low means parts of the map are hard to reach or not worth going to." />
          <Stat label="Never left spawn" value={`${r.spawnSitters} of ${r.botRounds}`} note="times in a round" />
          <Stat label="Deaths to the map" value={`${r.worldDeaths} of ${r.deaths}`} note={`${r.falls} ${r.falls === 1 ? "fall" : "falls"}`} />
          <Stat label="Map knowledge" value={KNOWLEDGE[r.knowledge] ?? r.knowledge} help="What the bots learned about the map's cover and angles. 'Didn't fit' means the saved file is for a different version of the map or nav." />
          <Stat label="Plugin errors" value={String(r.pluginErrors)} />
        </div>
        {r.compared.length > 0 && <p className="muted health-compared">Compared with other {r.modeName || r.mode} maps checked on this computer.</p>}
      </div>

      <SpotList title="Where bots fell" spots={r.fallSpots} map={r.map} what={(s) => `${s.count} ${s.count === 1 ? "time" : "times"}${s.drop > 0 ? ` · drops of up to ${Math.round(s.drop)} units` : ""}`} />
      <SpotList title="Where bots got stuck" spots={r.stuckSpots} map={r.map} what={(s) => `${duration(s.seconds)} in all · ${s.count} ${s.count === 1 ? "time" : "times"}`} />
      <SpotList title="Where the map killed bots" spots={r.hurtSpots} map={r.map} what={(s) => `${s.count} ${s.count === 1 ? "time" : "times"} (hurt areas, crushers, explosions)`} />

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
