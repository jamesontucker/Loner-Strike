import { useEffect, useState } from "react";
import { getJson, ON_THIS_COMPUTER, postJson, useActions } from "../api";
import type { ServerInfo } from "../types";
import { BusyButton, Info, LoadError, SectionHead, Skeleton, Switch, useConfirm, useToast } from "../ui";

/** Advanced Bot Interaction (voice.rs): the bots hear what people say on voice. */
type Status = {
  installed: boolean;
  installing: boolean;
  progress: number;
  model_bytes: number;
  enabled: boolean;
  /** a server on this computer (a Linux server gets it in a later update) */
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
      if (live) t = window.setTimeout(tick, 3000);
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
    }, "Downloading the speech model…");

  const uninstall = async () => {
    if (!(await ask("Remove Advanced Bot Interaction? Voice turns off on every server and the speech model is deleted from this computer. You can install it again any time.", { ok: "Remove", danger: true }))) return;
    run(key("uninstall"), async () => {
      const r = await post("/api/voice/uninstall");
      if (r.error) toast(r.error, true);
      else toast("Removed.");
      await load();
    }, "Removing…");
  };

  const toggle = (on: boolean) =>
    run(key("save"), async () => {
      const r = await post(`/api/servers/${server.id}/voice`, { enabled: on });
      if (r.error) toast(r.error, true);
      await load();
    }, on ? "Turning voice on…" : "Turning voice off…");

  return (
    <div className="voice-addon">
      <SectionHead
        title="Advanced Bot Interaction"
        note="The bots hear what people say on voice chat and answer, the same way as typed team chat: rush b, two long, flash for me, someone buy me an awp."
      />
      <div className="panel">
        <p className="muted">
          Speech is turned into text on this computer and given to the bots. Nothing is recorded, kept or sent anywhere. Only teammates' voice
          reaches them, as in the game.
        </p>
        {!st.installed ? (
          <div className="row between">
            <div>
              <b>Install</b>
              <p className="muted">Downloads the speech model ({mb(st.model_bytes)}) once, for every server on this computer.</p>
            </div>
            {ON_THIS_COMPUTER && (
              <BusyButton
                className="btn primary"
                busy={busy(key("install")) || st.installing}
                label="Install"
                busyLabel={st.progress > 0 ? `Downloading ${Math.round(st.progress * 100)}%…` : "Downloading…"}
                onClick={install}
              />
            )}
          </div>
        ) : (
          <>
            {!st.supported ? (
              <p className="muted">Voice for servers on a Linux machine comes in a later update. It works on servers on this computer.</p>
            ) : (
              <div className="setting">
                <div className="name">
                  Bots understand voice chat
                  <span className="note">
                    {st.enabled
                      ? st.running
                        ? "On, and listening"
                        : "On. It starts with the server"
                      : "Off"}
                  </span>
                </div>
                <Switch on={st.enabled} label="Bots understand voice chat" disabled={!ON_THIS_COMPUTER || busy(key("save"))} onChange={toggle} />
              </div>
            )}
            {st.error && <p className="error">{st.error}</p>}
            {st.enabled && st.log.length > 0 && (
              <details className="advanced">
                <summary>What it heard</summary>
                <pre className="console small">{st.log.join("\n")}</pre>
              </details>
            )}
            {ON_THIS_COMPUTER && (
              <div className="row">
                <BusyButton className="btn danger" busy={busy(key("uninstall"))} label="Remove" busyLabel="Removing…" onClick={uninstall} />
                <Info text={`Turns voice off on every server and deletes the speech model (${mb(st.model_bytes)}).`} />
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
