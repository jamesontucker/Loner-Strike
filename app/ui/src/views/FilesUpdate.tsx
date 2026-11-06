import { useState } from "react";
import { postJson } from "../api";
import type { AppInfo } from "../types";
import { useToast } from "../ui";
import type { Scan } from "./Setup";
import { JobProgress } from "./Servers";

/** After an app update: the parts of Loner-Strike in this game whose files changed, and
 *  mods the player had before Loner-Strike (without its bot fixes). One button puts the
 *  current versions in; settings, admins and maps stay. */
export function FilesUpdate({ app, scan, gameOpen, reloadApp }: { app: AppInfo | null; scan: Scan | null; gameOpen: boolean; reloadApp: () => void }) {
  const toast = useToast();
  const [running, setRunning] = useState(false);
  const [skip, setSkip] = useState(false);
  const [open, setOpen] = useState(false);
  const g = app?.game;
  const outdated = g?.outdated ?? [];
  const theirs = g?.unmanaged ?? [];
  if (!g || skip || (!outdated.length && !theirs.length && !running)) return null;

  const name = (id: string) => (id === "core" ? "Loner-Strike's own plugins" : scan?.catalog.find((c) => c.id === id)?.name ?? id);
  const blocked = gameOpen && !g.dedicated;
  // one part, or all of them ("core" alone: setup's own parts are always refreshed)
  const update = async (ids: string[]) => {
    const components = ids.filter((c) => c !== "core" && c !== "foundation");
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/setup/install", { hl: g.hl, games: [g.id], components, repair: true }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    setRunning(true);
  };
  const row = (id: string, label: string) => (
    <li key={id} className="files-update-row">
      <span>{name(id)}</span>
      <button className="btn small" disabled={blocked} onClick={() => { setOpen(false); update([id]); }}>{label}</button>
    </li>
  );
  const count = outdated.length + theirs.length;
  if (running)
    return (
      <div className="panel files-update">
        <JobProgress onEnd={(ok) => { setRunning(false); toast(ok ? "Up to date" : "That stopped. The bar shows why.", !ok); reloadApp(); }} />
      </div>
    );
  return (
    <>
      <div className="files-bar" role="status">
        <span>
          {outdated.length ? `${count} ${count === 1 ? "part" : "parts"} of Loner-Strike ${count === 1 ? "has" : "have"} fixes waiting.` : `${count} ${count === 1 ? "mod is" : "mods are"} in your game without the bot fixes.`}
          {blocked ? ` Close ${g.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero"} to update.` : ""}
        </span>
        <span className="files-bar-actions">
          <button className="btn small" onClick={() => setOpen(true)}>Details</button>
          <button className="btn small primary" disabled={blocked} onClick={() => update([...outdated, ...theirs])}>Update all</button>
          <button className="files-bar-close" aria-label="Hide until next time" onClick={() => setSkip(true)}>×</button>
        </span>
      </div>
      {open && (
        <div className="sheet-backdrop" onClick={() => setOpen(false)}>
          <div className="sheet files-sheet" role="dialog" aria-label="Fixes for your game files" onClick={(e) => e.stopPropagation()}>
            <div className="sheet-body">
            <h2>Fixes for your game files</h2>
            {outdated.length > 0 && (
              <>
                <p className="muted">This version of Loner-Strike has newer files for these. Your settings, admins and maps stay.</p>
                <ul className="files-update-list">{outdated.map((id) => row(id, "Update"))}</ul>
              </>
            )}
            {theirs.length > 0 && (
              <>
                <p className="muted">These were in your game before Loner-Strike, without its bot fixes. Its fixed version goes in; yours is backed up and comes back if you remove it.</p>
                <ul className="files-update-list">{theirs.map((id) => row(id, "Put in the fixed one"))}</ul>
              </>
            )}
            {blocked && <p className="muted small-note">Close {g.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero"} first.</p>}
            <div className="row">
              <button className="btn primary" disabled={blocked} onClick={() => { setOpen(false); update([...outdated, ...theirs]); }}>Update all</button>
              <button className="btn" onClick={() => setOpen(false)}>Close</button>
            </div>
            </div>
          </div>
        </div>
      )}
    </>
  );
}
