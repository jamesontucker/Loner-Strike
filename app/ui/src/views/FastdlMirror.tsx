// Fast downloads from Loner-Strike's own mirror (dl.lonerstrike.com/fastdl/<server id>): only for
// the mirror's owner (the app shows it when their upload keys file is on this computer). The app
// prepares the server's custom files; scripts/r2-fastdl.sh uploads them; Check compares the mirror
// with the server, the way the game downloads (fastdl.rs).

import { useEffect, useState } from "react";
import { copyText, getJsonAt, postJsonAt, useActions } from "../api";
import { BusyButton, useToast } from "../ui";

type Info = { can_upload?: boolean; url?: string; prepared?: { files: number; made: number } | null; error?: string };
type Prepared = { ok?: boolean; files?: number; copied?: number; removed?: number; mb?: number; dir?: string; command?: string; needs_czero?: string[]; error?: string };
type Problem = { path: string; why: string };
type Checked = { ok?: boolean; url?: string; files?: number; checked?: number; extra?: number; uploaded?: boolean; problems?: Problem[]; cloudflare?: Record<string, number>; error?: string };

export function FastdlMirror({ id, current, save }: { id: string; current: string; save: (url: string, done: string) => Promise<boolean> }) {
  const toast = useToast();
  const { run, busy } = useActions();
  const [info, setInfo] = useState<Info | null>(null);
  const [prep, setPrep] = useState<Prepared | null>(null);
  const [check, setCheck] = useState<Checked | null>(null);
  useEffect(() => {
    let live = true;
    getJsonAt<Info>(`/api/fastdl/${encodeURIComponent(id)}`, null)
      .then((r) => live && setInfo(r))
      .catch(() => live && setInfo(null));
    return () => {
      live = false;
    };
  }, [id]);
  if (!info?.can_upload || !info.url) return null;
  const url = info.url;
  const on = current.replace(/\/+$/, "") === url;
  const k = (s: string) => `fastdl-mirror-${s}`;
  const toggle = () => run(k("use"), async () => {
    await save(on ? "" : url, on ? "Fast downloads are off" : "Saved. Players download from the mirror from the next map.");
  }, on ? "Turning the mirror off…" : "Pointing the server at the mirror…");
  const prepare = () => run(k("prep"), async () => {
    const r = await postJsonAt<Prepared>(`/api/fastdl/${encodeURIComponent(id)}/prepare`, {}, null).catch((): Prepared => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    setPrep(r);
  }, "Collecting the server's custom files…");
  const runCheck = () => run(k("check"), async () => {
    const r = await postJsonAt<Checked>(`/api/fastdl/${encodeURIComponent(id)}/check`, {}, null).catch((): Checked => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    setCheck(r);
  }, "Checking the mirror against the server…");
  const copy = async (text: string) => {
    if (await copyText(text)) toast("Copied");
    else toast("Couldn't copy. Select the text instead.", true);
  };
  return (
    <div className="panel stack-tight">
      <h3>Loner-Strike's mirror</h3>
      <p className="measure muted small-note">
        For Loner-Strike's own servers: players download this server's custom files from <code>{url}</code>, on Cloudflare. Prepare the files, upload them with
        the script on this Mac, then check.
      </p>
      <div className="row">
        <BusyButton className={on ? "btn" : "btn primary"} busy={busy(k("use"))} label={on ? "Stop using the mirror" : "Use the mirror"} busyLabel="Saving…" onClick={toggle} />
        <BusyButton busy={busy(k("prep"))} label="Prepare files" busyLabel="Collecting…" onClick={prepare} />
        <BusyButton busy={busy(k("check"))} label="Check fast downloads" busyLabel="Checking…" onClick={runCheck} />
      </div>
      {prep?.ok && (
        <div className="stack-tight">
          <p className="small-note">
            {prep.files ? `${prep.files} files (${prep.mb} MB) ready, ${prep.copied} copied just now${prep.removed ? `, ${prep.removed} gone from the server` : ""}.` : "There's no custom content: everything here comes with the game."}{" "}
            Upload them in Terminal, from the Loner-Strike repo:
          </p>
          {!!prep.needs_czero?.length && (
            <p className="small-note">
              Needs Condition Zero files: {prep.needs_czero.length} {prep.needs_czero.length === 1 ? "map" : "maps"} ({prep.needs_czero.slice(0, 8).join(", ")}
              {prep.needs_czero.length > 8 ? ", …" : ""}). The mirror never has Valve's own files, so CS 1.6 players see missing textures on those.
            </p>
          )}
          {prep.command && (
            <div className="copy-row">
              <div>
                <code className="hltv-cmd">{prep.command}</code>
                <button className="btn small" onClick={() => copy(prep.command!)}>Copy</button>
              </div>
            </div>
          )}
        </div>
      )}
      {check && (
        <div className="stack-tight">
          {check.ok ? (
            <p className="small-note">All {check.files} files match the server and download as the game expects ({check.checked} checked{check.extra ? `, ${check.extra} extra on the mirror` : ""}).</p>
          ) : (
            <>
              <p className="small-note err-text">{(check.problems ?? []).length} to fix:</p>
              <ul className="small-note">
                {(check.problems ?? []).slice(0, 20).map((p, i) => (
                  <li key={i}>{p.path ? <><code>{p.path}</code> {p.why}</> : p.why}</li>
                ))}
                {(check.problems ?? []).length > 20 && <li>…and {(check.problems ?? []).length - 20} more.</li>}
              </ul>
            </>
          )}
          {check.cloudflare && Object.keys(check.cloudflare).length > 0 && (
            <p className="muted small-note">Cloudflare cache: {Object.entries(check.cloudflare).map(([s, n]) => `${s.toLowerCase()} ${n}`).join(", ")}.</p>
          )}
        </div>
      )}
    </div>
  );
}
