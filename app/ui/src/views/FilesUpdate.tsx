import { useState } from "react";
import { ON_THIS_COMPUTER, postJson, targetKey, useActions } from "../api";
import type { AppInfo } from "../types";
import { gameTitle } from "../games";
import { BusyButton, Sheet, useToast } from "../ui";
import type { Scan } from "./Setup";
import { JobProgress } from "./JobProgress";

/** After an app update: the parts of Loner-Strike in this game whose files changed, and
 *  mods the player had before Loner-Strike (without its bot fixes). One button puts the
 *  current versions in; settings, admins and maps stay. */
export function FilesUpdate({ app, scan, gameOpen, reloadApp, server }: { app: AppInfo | null; scan: Scan | null; gameOpen: boolean; reloadApp: () => void; server?: string }) {
  const toast = useToast();
  const [running, setRunning] = useState(false);
  const [skip, setSkip] = useState(false);
  const [open, setOpen] = useState(false);
  const { run, busy } = useActions();
  // one update at a time, whichever button started it
  const key = `files:${targetKey()}:update`;
  const updating = busy(key);
  const g = app?.game;
  const outdated = g?.outdated ?? [];
  const theirs = g?.unmanaged ?? [];
  if (!g || skip || (!outdated.length && !theirs.length && !running)) return null;

  const name = (id: string) => (id === "core" ? "Loner-Strike's own plugins" : scan?.catalog.find((c) => c.id === id)?.name ?? id);
  const blocked = gameOpen && !g.dedicated;
  // one part, or all of them ("core" alone: setup's own parts are always refreshed)
  const update = (ids: string[]) => run(key, async () => {
    const components = ids.filter((c) => c !== "core" && c !== "foundation");
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/setup/install", { hl: g.hl, games: [g.id], components, repair: true }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    setRunning(true);
  }, ids.length === 1 ? `Updating ${name(ids[0])}…` : `Updating ${them}…`);
  const row = (id: string, label: string) => (
    <li key={id} className="files-update-row">
      <span>{name(id)}</span>
      <button className="btn small" disabled={blocked || updating} onClick={() => { setOpen(false); update([id]); }}>{label}</button>
    </li>
  );
  const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
  // whose files these are: your game and each server are updated separately
  const whose = server ? server : `Your game (${gameTitle(g.id)})`;
  const them = server ? server : "your game";
  // what the bar says: Loner-Strike's own parts, mods you had before it, or both
  const summary =
    outdated.length && theirs.length
      ? `${whose}: ${plural(outdated.length, "part", "parts")} of Loner-Strike and ${theirs.length} of your own mods have updates waiting.`
      : outdated.length
        ? `${whose}: ${plural(outdated.length, "part", "parts")} of Loner-Strike ${outdated.length === 1 ? "has" : "have"} updates waiting.`
        : `${whose}: ${plural(theirs.length, "mod is", "mods are")} here without the bot fixes.`;
  // a server is updated on this computer, then uploaded; your game and other servers aren't touched
  const scope = server ? " Your game and other servers are updated separately." : " Each server is updated from its own page.";
  if (running)
    return (
      <div className="panel files-update">
        <JobProgress
          title={`Updating ${them}…`}
          // a failed update stays on screen with its reason until closed
          onEnd={(ok) => { if (ok) setRunning(false); toast(ok ? `Updated ${them}` : "The update stopped. The bar shows why.", !ok); reloadApp(); }}
          onClose={() => setRunning(false)}
        />
      </div>
    );
  return (
    <>
      <div className="files-bar" role="status">
        <span>
          {summary}
          {blocked ? ` Close ${gameTitle(g.id)} to update.` : ""}
          <span className="muted">{scope}</span>
        </span>
        <span className="files-bar-actions">
          {/* the files are on the computer: a phone only says so */}
          {ON_THIS_COMPUTER ? (
            <>
              <button className="btn small" onClick={() => setOpen(true)}>Details</button>
              <BusyButton className="btn small primary" busy={updating} disabled={blocked} label="Update all" busyLabel="Updating…" onClick={() => update([...outdated, ...theirs])} />
            </>
          ) : (
            <span className="muted">Update on the computer</span>
          )}
          <button className="files-bar-close" aria-label="Hide until next time" onClick={() => setSkip(true)}>×</button>
        </span>
      </div>
      {open && (
        <Sheet label={`Updates for ${them}`} onClose={() => setOpen(false)} className="sheet files-sheet">
            <div className="sheet-body">
            <h2>Updates for {them}</h2>
            {server && <p className="muted small-note">Made here on your computer, then uploaded to the server if it's on another machine. Your game is updated separately.</p>}
            {outdated.length > 0 && (
              <>
                <p className="muted">This version of Loner-Strike has newer files for these. Your settings, admins and maps stay.</p>
                <ul className="files-update-list">{outdated.map((id) => row(id, "Update"))}</ul>
              </>
            )}
            {theirs.length > 0 && (
              <>
                <p className="muted">These were in your game before Loner-Strike, without its bot fixes. Its fixed version goes in; yours is backed up and comes back if you remove it.</p>
                <ul className="files-update-list">{theirs.map((id) => row(id, "Use the fixed version"))}</ul>
              </>
            )}
            {blocked && <p className="muted small-note">Close {gameTitle(g.id)} first.</p>}
            <div className="row">
              <BusyButton className="btn primary" busy={updating} disabled={blocked} label="Update all" busyLabel="Updating…" onClick={() => { setOpen(false); update([...outdated, ...theirs]); }} />
              <button className="btn" onClick={() => setOpen(false)}>Close</button>
            </div>
            </div>
        </Sheet>
      )}
    </>
  );
}
