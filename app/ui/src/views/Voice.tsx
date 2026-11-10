import { useEffect, useState } from "react";
import { getJson, ON_COMPUTER_ONLY, ON_THIS_COMPUTER, postJson, useActions } from "../api";
import type { ServerInfo } from "../types";
import { BusyButton, Info, LoadError, SectionHead, Skeleton, Switch, useConfirm, useToast } from "../ui";

/** Advanced Bot Interaction (voice.rs): the bots hear what people say on voice. */
type Status = {
  installed: boolean;
  installing: boolean;
  progress: number;
  model_bytes: number;
  /** a server on a Linux machine: the model goes there when voice is turned on */
  remote: boolean;
  enabled: boolean;
  supported: boolean;
  running: boolean;
  error: string | null;
  /** what it heard and gave the bots, newest last */
  log: string[];
};
type Reply = Partial<Status> & { ok?: boolean; error?: string };

const post = (path: string, body: unknown = {}) => postJson<Reply>(path, body).catch((): Reply => ({ error: "Couldn't reach the app." }));
const mb = (b: number) => `${Math.round(b / 1024 / 1024)} MB`;

export function Voice({ server }: { server: ServerInfo }) {
  const toast = useToast();
  const ask = useConfirm();
  const { run, busy } = useActions();
  const [st, setSt] = useState<Status | null>(null);
  const [failed, setFailed] = useState("");
  const key = (what: string) => `voice:${server.id}:${what}`;

  const load = async () => {
    const r = await getJson<Status & { error?: string }>(`/api/servers/${server.id}/voice`).catch(() => null);
    if (!r || r.installed === undefined) return setFailed(r?.error || "Couldn't reach the app.");
    setFailed("");
    setSt(r);
  };
  useEffect(() => {
    setSt(null);
    let t = 0;
    let live = true;
    const tick = async () => {
      if (!document.hidden) await load();
      // (a Linux server's state comes over SSH: less often)
      if (live) t = window.setTimeout(tick, server.host ? 10000 : 3000);
    };
    tick();
    return () => {
      live = false;
      window.clearTimeout(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [server.id]);

  if (failed && !st) return <LoadError text={failed} onRetry={load} />;
  if (!st) return <Skeleton rows={3} />;

  const install = () =>
    run(key("install"), async () => {
      const r = await post("/api/voice/install");
      if (r.error) toast(r.error, true);
      else toast("Installed. Turn it on for this server below.");
      await load();
    }, "Downloading the models…");

  const uninstall = async () => {
    if (!(await ask("Remove bot voice commands? Voice turns off on every server and the models are deleted from this computer. You can install it again any time.", { ok: "Remove", danger: true }))) return;
    run(key("uninstall"), async () => {
      const r = await post("/api/voice/uninstall");
      if (r.error) toast(r.error, true);
      else toast("Bot voice commands removed.");
      await load();
    }, "Removing…");
  };

  const toggle = (on: boolean) =>
    run(key("save"), async () => {
      const r = await post(`/api/servers/${server.id}/voice`, { enabled: on });
      if (r.error) toast(r.error, true);
      await load();
    }, on ? (st?.remote ? "Setting up voice on the server…" : "Turning voice on…") : "Turning voice off…");

  return (
    <div className="voice-addon">
      <SectionHead
        title="Bot voice commands"
        note={'The bots hear what people say on voice chat and answer, the same way as typed team chat: "rush b", "two long", "flash for me", "someone buy me an AWP".'}
      />
      <div className="panel">
        <p className="muted">
          Speech is turned into text on {st.remote ? "the server's machine" : "this computer"} and given to the bots. No audio is kept or sent
          anywhere; the lines it heard go in the server's log. Only teammates' voice reaches the bots, as in the game.
        </p>
        {!st.installed ? (
          <div className="row between">
            <div>
              <b>Install</b>
              <p className="muted">Downloads the speech and understanding models ({mb(st.model_bytes)}) once, for every server on this computer.</p>
            </div>
            {ON_THIS_COMPUTER ? (
              <BusyButton
                className="btn primary"
                busy={busy(key("install")) || st.installing}
                label="Install"
                busyLabel={st.progress > 0 ? `Downloading ${Math.round(st.progress * 100)}%…` : "Downloading…"}
                onClick={install}
              />
            ) : (
              <p className="muted small-note">{ON_COMPUTER_ONLY}</p>
            )}
          </div>
        ) : (
          <>
            {st.remote && !st.enabled && (
              <p className="muted">Turning it on puts the voice programs on the server and downloads the models there ({mb(st.model_bytes)}). It runs with the server and can only talk to the server itself.</p>
            )}
            {(
              <div className="setting">
                <div className="name">
                  Bots understand voice chat
                  <span className="note">
                    {busy(key("save")) ? (
                      <>
                        <span className="spinner" aria-hidden="true" /> {st.remote ? "Setting up on the server. This can take a few minutes…" : "Saving…"}
                      </>
                    ) : st.enabled
                      ? st.running
                        ? "On, and listening"
                        : st.remote ? "On. It runs while the server does" : "On. It starts with the server"
                      : "Off"}
                    {!ON_THIS_COMPUTER && <> {ON_COMPUTER_ONLY}</>}
                  </span>
                </div>
                <Switch on={st.enabled} label="Bots understand voice chat" disabled={!ON_THIS_COMPUTER || busy(key("save"))} onChange={toggle} />
              </div>
            )}
            {st.error && <p className="err-text">{st.error}</p>}
            {failed && <p className="muted">Couldn't check on it just now. Showing what it last reported.</p>}
            {(st.enabled || st.error) && st.log.length > 0 && (
              <details className="advanced">
                <summary>What it heard (its log)</summary>
                <pre className="console small">{st.log.join("\n")}</pre>
              </details>
            )}
            {ON_THIS_COMPUTER && !st.remote && (
              <div className="row">
                <BusyButton className="btn danger" busy={busy(key("uninstall"))} label="Remove" busyLabel="Removing…" onClick={uninstall} />
                <Info text={`Turns voice off on every server and deletes the models (${mb(st.model_bytes)}).`} />
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
