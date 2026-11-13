import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { copyText, getJsonAt, postJson, useActions } from "../api";
import { queryClient } from "../queries";
import { BusyButton, Info, LoadError, SectionHead, Switch, useConfirm, useToast } from "../ui";

// Settings > AI agents: Loner-Strike's Tools for coding agents (Claude Code and others) over
// MCP. The server side is app/src-tauri/src/mcp/; how to connect is in docs/MCP.md.

type Row = { n: number; at: number; tool: string; detail: string; outcome: "done" | "refused" | "failed"; note: string };
type Agents = {
  enabled: boolean;
  changes: boolean;
  running: boolean;
  port: number | null;
  url: string | null;
  token: string;
  error: string | null;
  keep: boolean;
  port_pref: number;
  port_note: string | null;
  setups: Setup[];
  log: Row[];
};
type Setup = { client: string; label: string; kind: "stdio" | "http"; where: string; text: string };

const KEY = ["mcpAgents"];
const OUTCOME: Record<Row["outcome"], string> = { done: "Done", refused: "Not allowed", failed: "Didn't work" };

/** The AI agents section (computer only: it shows the token). */
export function AgentSettings() {
  const q = useQuery({
    queryKey: KEY,
    queryFn: ({ signal }) => getJsonAt<Agents>("/api/mcp", null, signal),
    // the activity log fills in while agents work
    refetchInterval: (query) => (query.state.data?.enabled ? 4000 : false),
  });
  const a = q.data ?? null;
  const { run, busy } = useActions();
  const toast = useToast();
  const ask = useConfirm();
  const [showToken, setShowToken] = useState(false);
  const [client, setClient] = useState(() => {
    try { return localStorage.getItem("ls.mcpClient") || "claude-code"; } catch { return "claude-code"; }
  });
  const [http, setHttp] = useState(false);
  const [portText, setPortText] = useState<string | null>(null);

  type Answer = Partial<Agents> & { error?: string | null };
  const post = (path: string, body: unknown = {}) => postJson<Answer>(path, body).catch((): Answer => ({ error: "Couldn't reach the app." }));
  const save = (key: string, body: { enabled?: boolean; changes?: boolean; keep?: boolean; port?: number }, label: string) =>
    run(key, async () => {
      const r = await post("/api/mcp", body);
      if (r.enabled === undefined) return toast(r.error ?? "Couldn't change that setting.", true);
      queryClient.setQueryData(KEY, r);
      if (r.error) toast(r.error, true);
    }, label);
  const setOn = (enabled: boolean) => save("mcp:on", { enabled }, enabled ? "Turning on AI agents…" : "Turning off AI agents…");
  const setChanges = async (changes: boolean) => {
    if (changes && !(await ask("Let AI agents change things? They can then edit maps and navs in the tools and start, stop and change your servers. You're asked here before anything that overwrites a file or stops people playing.", { ok: "Allow changes" }))) return;
    await save("mcp:changes", { changes }, "Saving the setting…");
  };
  const newToken = async () => {
    if (!(await ask("Make a new token? HTTP setups with the old one stop working until you copy the setup again. The stdio bridge picks up the new one by itself.", { ok: "New token", danger: true }))) return;
    await run("mcp:token", async () => {
      const r = await post("/api/mcp/token");
      if (r.error && !r.token) return toast(r.error, true);
      queryClient.setQueryData(KEY, r);
      toast("New token made.");
    }, "Making a new token…");
  };
  const clearLog = () => run("mcp:clear", async () => {
    const r = await post("/api/mcp/log/clear");
    if (r.token) queryClient.setQueryData(KEY, r);
  }, "Clearing…");
  const copy = async (s: Setup | undefined) => {
    if (s && (await copyText(s.text))) toast(s.where.startsWith("Run") ? "Copied. Paste it in a terminal." : `Copied. Paste it into ${s.where.split(" (")[0]}.`);
    else toast("Couldn't copy. Select the text and copy it by hand.", true);
  };
  const pickClient = (c: string) => {
    setClient(c);
    try { localStorage.setItem("ls.mcpClient", c); } catch { /* private window */ }
  };
  const clients = (a?.setups ?? []).filter((s) => s.kind === "stdio");
  const hasHttp = (a?.setups ?? []).some((s) => s.client === client && s.kind === "http");
  const setup = (a?.setups ?? []).find((s) => s.client === client && s.kind === (http && hasHttp ? "http" : "stdio")) ?? clients[0];
  const savePort = () => {
    const n = Number(portText);
    setPortText(null);
    if (!a || !portText || n === a.port_pref) return;
    if (!Number.isInteger(n) || n < 1024 || n > 65535) return toast("Pick a port from 1024 to 65535.", true);
    save("mcp:port", { port: n }, "Moving to the new port…");
  };
  const log = [...(a?.log ?? [])].reverse();

  return (
    <section>
      <SectionHead title="AI agents" note="Let coding agents like Claude Code use Loner-Strike's Tools (Beta): read maps, check them for bots, build in the map editor, fix navs, run your servers. Only programs on this computer with the token can connect." />
      <div className="panel settings-list agent-settings">
        {q.isError && !a && <LoadError text="Couldn't load this setting." onRetry={() => q.refetch()} />}
        <div className="row between setting-line toggle">
          <div>
            <b>Let AI agents connect</b>
            <p className="muted">Off unless you turn it on. Agents can only look until you also allow changes.</p>
          </div>
          <Switch on={!!a?.enabled} label="Let AI agents connect" disabled={!a || busy("mcp:on")} onChange={setOn} />
        </div>
        {a?.enabled && (
          <>
            <div className="row between setting-line toggle">
              <div>
                <b>Allow changes</b>
                <p className="muted">Edit maps and navs, start and stop servers. You're asked before anything that overwrites a file or stops people playing.</p>
              </div>
              <Switch on={a.changes} label="Allow AI agents to change things" disabled={busy("mcp:changes")} onChange={setChanges} />
            </div>
            {a.error && <p className="err-text">{a.error}</p>}
            {a.running && setup && (
              <div className="setting-line stack-tight">
                <div className="row between">
                  <div>
                    <b>Connect an agent</b>
                    <p className="muted">Set it up once. The usual setup starts Loner-Strike's bridge, which finds the app by itself, so it keeps working after restarts.</p>
                  </div>
                  <div className="row nowrap">
                    <select value={setup.client} onChange={(e) => pickClient(e.target.value)} aria-label="Your agent">
                      {clients.map((c) => <option key={c.client} value={c.client}>{c.label}</option>)}
                    </select>
                    <button className="btn primary" onClick={() => copy(setup)}>Copy</button>
                  </div>
                </div>
                <p className="muted">{setup.where}</p>
                <pre className="mono small agent-command" style={{ padding: "8px 10px", background: "rgba(0,0,0,.18)", borderRadius: 2, whiteSpace: "pre-wrap", margin: 0 }}>{setup.text}</pre>
                {hasHttp && (
                  <label className="row nowrap">
                    <input type="checkbox" checked={http} onChange={(e) => setHttp(e.target.checked)} /> Connect over HTTP with the token instead
                    <Info text="For agents that can't start programs, or if you prefer HTTP. The address and token are in the setup: keep the same token across restarts below, or copy it again after each restart." />
                  </label>
                )}
              </div>
            )}
            {a.running && (
              <div className="setting-line stack-tight">
                <div className="row between toggle">
                  <div>
                    <b>Keep the same token across restarts</b>
                    <p className="muted">For HTTP setups. Off: a new token each time Loner-Strike starts, so an old one stops working by itself.</p>
                  </div>
                  <Switch on={a.keep} label="Keep the same token across restarts" disabled={busy("mcp:keep")} onChange={(keep) => save("mcp:keep", { keep }, "Saving the setting…")} />
                </div>
                <div className="row between">
                  <p className="muted">
                    Port{" "}
                    <input className="search narrow" type="number" min={1024} max={65535} aria-label="Port for AI agents" value={portText ?? String(a.port_pref)} disabled={busy("mcp:port")} onChange={(e) => setPortText(e.target.value)} onBlur={savePort} onKeyDown={(e) => e.key === "Enter" && savePort()} />
                    {"  "}Token: <span className="mono">{showToken ? a.token : "•".repeat(12)}</span>{" "}
                    <button className="link inline" onClick={() => setShowToken((s) => !s)}>{showToken ? "Hide" : "Show"}</button>
                  </p>
                  <BusyButton className="btn" busy={busy("mcp:token")} label="New token" busyLabel="Making…" onClick={newToken} />
                </div>
                {a.port_note && <p className="muted">{a.port_note}</p>}
              </div>
            )}
            <div className="setting-line stack-tight">
              <div className="row between">
                <div>
                  <b>What agents did</b>
                  <p className="muted">Since Loner-Strike started, newest first.</p>
                </div>
                {log.length > 0 && <BusyButton className="btn small" busy={busy("mcp:clear")} label="Clear" busyLabel="Clearing…" onClick={clearLog} />}
              </div>
              {log.length === 0 ? (
                <p className="muted">Nothing yet.</p>
              ) : (
                <ul className="agent-log" style={{ listStyle: "none", margin: 0, padding: 0, maxHeight: 260, overflowY: "auto", display: "grid", gap: 4 }}>
                  {log.map((r) => (
                    <li key={r.n} className={`agent-log-${r.outcome}`}>
                      <span className="muted">{new Date(r.at * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" })}</span>{" "}
                      <b>{r.tool}</b>
                      {r.detail && <span className="mono small"> {r.detail}</span>}
                      {r.outcome !== "done" && <span className={r.outcome === "failed" ? "err-text" : "muted"}> {OUTCOME[r.outcome]}{r.note ? `: ${r.note}` : ""}</span>}
                    </li>
                  ))}
                </ul>
              )}
            </div>
          </>
        )}
      </div>
    </section>
  );
}
export default AgentSettings;
