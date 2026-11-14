// Crash tracking for a server (agent MCP): a badge ("3 crashes in 24 h") on the server's card and
// page, and on the Health tab the list (details and Copy), a warning for a module known to crash
// servers (the official Ham Sandwich), and Discord alerts through the server's status channel.
// Server: app/src-tauri/src/crashes.rs (the capture on the servers is Y's: docs/notes/Y.md).

import { useEffect, useState } from "react";
import { create } from "zustand";
import { copyText, getJson, postJson } from "../api";
import type { ServerInfo } from "../types";
import { BusyButton, Switch, useToast } from "../ui";

export type Crash = {
  id: string;
  time: number;
  when: string;
  source: string;
  file: string;
  kind: string;
  map?: string | null;
  mode?: string | null;
  fault?: string | null;
  address?: string | null;
  module?: string | null;
  offset?: string | null;
  uptimeSec?: number | null;
  players?: number | null;
  bots?: number | null;
  stack: string[];
  marks: string[];
  lastLines: string[];
  atStop: boolean;
  note?: string | null;
  build?: Record<string, string>;
};
type Warning = { id: string; module: string; file?: string | null; message: string; fix: string };
type Report = { last24h: number; total: number; crashes: Crash[]; warnings: Warning[]; capture: string; alerts: { discord: boolean; discordLinked: boolean }; error?: string };
type Summary = Record<string, { last24h: number; warnings: number }>;

/** The crash counts of every server, for the badges: one request for the whole list. */
const useSummary = create<{ s: Summary; at: number; load: () => void }>((set, get) => ({
  s: {},
  at: 0,
  load: () => {
    if (Date.now() - get().at < 60_000) return;
    set({ at: Date.now() });
    void getJson<Summary>("/api/crashes/summary")
      .then((s) => set({ s }))
      .catch(() => undefined);
  },
}));

/** "3 crashes in 24 h" (and a warning mark for a known-bad module); nothing when all is well. */
export function CrashBadge({ id, inline }: { id: string; inline?: boolean }) {
  const s = useSummary((x) => x.s[id]);
  const load = useSummary((x) => x.load);
  useEffect(() => {
    load();
    // (the app reads the servers in the background; ask again in a minute)
    const t = window.setInterval(load, 61_000);
    return () => window.clearInterval(t);
  }, [load]);
  if (!s || (!s.last24h && !s.warnings)) return null;
  const parts = [s.last24h ? `${s.last24h} crash${s.last24h === 1 ? "" : "es"} in 24 h` : "", s.warnings ? "Crash-prone module" : ""].filter(Boolean);
  return (
    <span className={`live off-line crash-badge${inline ? " inline" : ""}`} title="See Health for the details">
      {parts.join(" · ")}
    </span>
  );
}

/** One crash as text, for Copy (pasted into Discord or a bug report). */
export function crashText(server: string, c: Crash) {
  const lines = [`${server}: ${c.kind} at ${c.when}${c.map ? ` on ${c.map}` : ""}${c.mode ? ` (${c.mode})` : ""}`];
  if (c.fault) lines.push(`${c.fault}${c.address ? ` at ${c.address}` : ""}${c.module ? ` in ${c.module}${c.offset ? `+${c.offset}` : ""}` : ""}`);
  if (c.build) lines.push(`build: ${Object.entries(c.build).map(([k, v]) => `${k} ${v}`).join(", ")}`);
  if (c.stack.length) lines.push("stack:", ...c.stack.map((s) => `  ${s}`));
  if (c.marks.length) lines.push("last marks:", ...c.marks.map((s) => `  ${s}`));
  if (c.lastLines.length) lines.push("last log lines:", ...c.lastLines.map((s) => `  ${s}`));
  lines.push(`(${c.source}: ${c.file})`);
  return lines.join("\n");
}

function CrashRow({ server, c }: { server: string; c: Crash }) {
  const toast = useToast();
  const [open, setOpen] = useState(false);
  const what = [c.fault, c.module ? `in ${c.module}` : ""].filter(Boolean).join(" ");
  return (
    <li className={c.atStop ? "muted" : undefined}>
      <div className="row between">
        <div>
          <b>{c.when}</b>
          {c.map ? ` · ${c.map}` : ""}
          {c.kind !== "crash" ? ` · ${c.kind}` : ""}
          {what ? <span className="muted"> · {what}</span> : null}
          {c.atStop && <p className="muted small-note">{c.note ?? "At a stop: not counted."}</p>}
        </div>
        <div className="row">
          <button type="button" className="btn small" aria-expanded={open} onClick={() => setOpen(!open)}>
            {open ? "Hide" : "Details"}
          </button>
          <button type="button" className="btn small" onClick={async () => toast((await copyText(crashText(server, c))) ? "Copied." : "Couldn't copy it.", false)}>
            Copy
          </button>
        </div>
      </div>
      {open && <pre className="crash-detail" style={{ maxHeight: 320, overflow: "auto", whiteSpace: "pre-wrap", fontSize: 12 }}>{crashText(server, c)}</pre>}
    </li>
  );
}

/** The Health tab's crash panel. */
export function CrashPanel({ server }: { server: ServerInfo }) {
  const toast = useToast();
  const [r, setR] = useState<Report | null>(null);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [all, setAll] = useState(false);
  const load = async (fresh: boolean) => {
    setLoading(true);
    try {
      const v = await getJson<Report>(`/api/servers/${server.id}/crashes${fresh ? "?fresh=1" : ""}`);
      if (v.error) setError(v.error);
      else {
        setR(v);
        setError("");
      }
    } catch (e) {
      const status = (e as { status?: number })?.status;
      setError(status === 404 || status === 403 ? "This copy of Loner-Strike can't read crashes yet. Restart it, then check again." : "Couldn't reach the app.");
    } finally {
      setLoading(false);
    }
  };
  useEffect(() => {
    void load(false);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [server.id]);
  const alerts = async (on: boolean) => {
    setSaving(true);
    try {
      const v = await postJson<Report>(`/api/servers/${server.id}/crashes/alerts`, { discord: on }).catch((): Report => ({ error: "Couldn't reach the app." }) as Report);
      if (v.error) return toast(v.error, true);
      setR(v);
      toast(on ? "New crashes will be posted to the server's Discord channel." : "Crashes won't be posted to Discord.");
    } finally {
      setSaving(false);
    }
  };
  const shown = r ? (all ? r.crashes : r.crashes.slice(0, 10)) : [];
  return (
    <div className="panel">
      <div className="row between">
        <h3>Crashes</h3>
        <BusyButton className="btn small" busy={loading && !!r} label="Check again" busyLabel="Checking…" onClick={() => void load(true)} />
      </div>
      {error && <p className="err-text small-note">{error}</p>}
      {!r && !error && (
        <p className="muted small-note">
          <span className="spinner" aria-hidden="true" />
          {server.host ? `Reading the crash files on ${server.host}…` : "Reading the server's crash files…"}
        </p>
      )}
      {r && (
        <>
          {r.warnings.map((w) => (
            <div key={w.id} className="banner" role="alert" style={{ borderLeftColor: "#d9826b" }}>
              <div>
                <b>{w.module}</b>: {w.message} <span className="muted">{w.fix}</span>
              </div>
            </div>
          ))}
          <p className="muted small-note">
            {r.total === 0
              ? "No crashes recorded."
              : `${r.last24h} in the last 24 hours, ${r.total} recorded in all${r.capture === "bot brain's crash log" ? " (from the bot brain's crash log)" : ""}.`}
          </p>
          {shown.length > 0 && (
            <ul className="upkeep crash-list">
              {shown.map((c) => (
                <CrashRow key={c.id} server={server.name} c={c} />
              ))}
            </ul>
          )}
          {r.crashes.length > 10 && (
            <button type="button" className="btn small" onClick={() => setAll(!all)}>
              {all ? "Show the newest 10" : `Show all ${r.crashes.length}`}
            </button>
          )}
          <div className="row between">
            <div>
              <b>Post crashes to Discord</b>
              <p className="muted small-note">
                {r.alerts.discordLinked ? "New crashes go to the server's Discord status channel while Loner-Strike is running." : "Link the server's Discord status channel first (Website and Discord)."}
              </p>
            </div>
            {saving ? (
              <span className="muted small-note">
                <span className="spinner" aria-hidden="true" />
                Saving…
              </span>
            ) : (
              <Switch on={r.alerts.discord} label="Post crashes to Discord" disabled={!r.alerts.discordLinked && !r.alerts.discord} onChange={(v) => void alerts(v)} />
            )}
          </div>
        </>
      )}
    </div>
  );
}
