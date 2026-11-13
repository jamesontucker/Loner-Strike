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
  command: string | null;
  stdio: string | null;
  log: Row[];
};

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
  const [stdio, setStdio] = useState(false);

  type Answer = Partial<Agents> & { error?: string | null };
  const post = (path: string, body: unknown = {}) => postJson<Answer>(path, body).catch((): Answer => ({ error: "Couldn't reach the app." }));
  const save = (key: string, body: Partial<Pick<Agents, "enabled" | "changes">>, label: string) =>
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
    if (!(await ask("Make a new token? Agents set up with the old one stop working until you add them again with the new command.", { ok: "New token", danger: true }))) return;
    await run("mcp:token", async () => {
      const r = await post("/api/mcp/token");
      if (r.error && !r.token) return toast(r.error, true);
      queryClient.setQueryData(KEY, r);
      toast("New token made. Copy the setup command again.");
    }, "Making a new token…");
  };
  const clearLog = () => run("mcp:clear", async () => {
    const r = await post("/api/mcp/log/clear");
    if (r.token) queryClient.setQueryData(KEY, r);
  }, "Clearing…");
  const copy = async (text: string | null, what: string) => {
    if (text && (await copyText(text))) toast(`${what} copied. Paste it in a terminal.`);
    else toast("Couldn't copy. Select the text and copy it by hand.", true);
  };
  const command = stdio ? a?.stdio : a?.command;
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
            {a.running && (
              <div className="setting-line stack-tight">
                <div className="row between">
                  <div>
                    <b>Connect Claude Code</b>
                    <p className="muted">Run this once in a terminal. The token changes each time Loner-Strike starts, so run the new one after a restart.</p>
                  </div>
                  <div className="row nowrap">
                    <button className="btn primary" onClick={() => copy(command ?? null, "Setup command")}>Copy setup command</button>
                    <Info text={<>Other agents: the address is <span className="mono">{a.url}</span> (Streamable HTTP) with the header <span className="mono">Authorization: Bearer</span> and the token. Agents that only start programs can use the stdio bridge instead.</>} />
                  </div>
                </div>
                <p className="mono small agent-command" style={{ padding: "8px 10px", background: "rgba(0,0,0,.18)", borderRadius: 2 }}>{command}</p>
                <div className="row">
                  <label className="row nowrap">
                    <input type="checkbox" checked={stdio} onChange={(e) => setStdio(e.target.checked)} /> Use the stdio bridge instead
                  </label>
                </div>
                <div className="row between">
                  <p className="muted">
                    Token: <span className="mono">{showToken ? a.token : "•".repeat(12)}</span>{" "}
                    <button className="link inline" onClick={() => setShowToken((s) => !s)}>{showToken ? "Hide" : "Show"}</button>
                  </p>
                  <BusyButton className="btn" busy={busy("mcp:token")} label="New token" busyLabel="Making…" onClick={newToken} />
                </div>
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
