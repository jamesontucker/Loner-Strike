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
  const g = app?.game;
  const outdated = g?.outdated ?? [];
  const theirs = g?.unmanaged ?? [];
  if (!g || skip || (!outdated.length && !theirs.length && !running)) return null;

  const name = (id: string) => (id === "core" ? "Loner-Strike's own plugins" : scan?.catalog.find((c) => c.id === id)?.name ?? id);
  const update = async () => {
    const components = [...outdated, ...theirs].filter((c) => c !== "core" && c !== "foundation");
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/setup/install", { hl: g.hl, games: [g.id], components, repair: true }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    setRunning(true);
  };
  return (
    <div className="panel files-update">
      {running ? (
        <JobProgress onEnd={(ok) => { setRunning(false); toast(ok ? "Up to date" : "That stopped. The bar shows why.", !ok); reloadApp(); }} />
      ) : (
        <>
          <h3>{outdated.length ? "Fixes for your game files" : "Mods without the bot fixes"}</h3>
          {outdated.length > 0 && (
            <p className="muted">
              This version of Loner-Strike has newer files for {outdated.map(name).join(", ")}. Put them in to get the latest fixes. Your settings, admins and maps stay.
            </p>
          )}
          {theirs.length > 0 && (
            <p className="muted">
              {theirs.map(name).join(", ")} {theirs.length === 1 ? "was" : "were"} already in your game before Loner-Strike, without its bot fixes. Loner-Strike can put its fixed version in; yours is backed up and comes back if you remove it.
            </p>
          )}
          {gameOpen && !g.dedicated && <p className="muted small-note">Close {g.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero"} first.</p>}
          <div className="row">
            <button className="btn primary" disabled={gameOpen && !g.dedicated} onClick={update}>{outdated.length ? "Update now" : "Put in the fixed versions"}</button>
            <button className="btn" onClick={() => setSkip(true)}>Later</button>
          </div>
        </>
      )}
    </div>
  );
}
