import { useState } from "react";
import { ON_THIS_COMPUTER, postJson, targetKey, useActions } from "../api";
import type { AppInfo } from "../types";
import { gameTitle } from "../games";
import { BusyButton, Sheet, useToast } from "../ui";
import type { Scan } from "./Setup";
import { JobProgress } from "./JobProgress";

/** One game (or server) with parts waiting: Loner-Strike's own (outdated) and mods the
 *  player had before Loner-Strike, without its bot fixes (theirs). */
type Behind = { id: string; hl: string; dedicated: boolean; outdated: string[]; theirs: string[] };

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;
/** "6 parts", "2 mods without the bot fixes", "6 parts, 2 mods" */
const count = (g: Behind) =>
  g.outdated.length && g.theirs.length
    ? `${plural(g.outdated.length, "part", "parts")}, ${plural(g.theirs.length, "mod", "mods")}`
    : g.outdated.length
      ? plural(g.outdated.length, "part", "parts")
      : `${plural(g.theirs.length, "mod", "mods")} without the bot fixes`;
/** "a, b and c" */
const and = (xs: string[]) => (xs.length < 2 ? xs.join("") : `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`);
/** what's waiting, so a dismissed bar comes back when something new is */
const signature = (gs: Behind[]) => gs.map((g) => `${g.id}:${[...g.outdated, ...g.theirs].sort().join(",")}`).join("|");
const DISMISS = () => `ls.files-bar.dismissed:${targetKey()}`;
function dismissedSig() {
  try { return localStorage.getItem(DISMISS()); } catch { return null; }
}

/** After an app update: the parts of Loner-Strike whose files changed, in the picked game and
 *  (Parity A) the other game set up in the same Half-Life folder, or in one server. One bar for
 *  all of them; Update all does each game in turn. Settings, admins and maps stay. */
export function FilesUpdate({ app, scan, gameOpen, reloadApp, server, withOthers }: { app: AppInfo | null; scan: Scan | null; gameOpen: boolean; reloadApp: () => void; server?: string; withOthers?: boolean }) {
  const toast = useToast();
  // the update in progress: the game now, and the ones after it
  const [current, setCurrent] = useState<{ g: Behind; ids: string[]; i: number; n: number; names: string[] } | null>(null);
  const [queue, setQueue] = useState<{ g: Behind; ids: string[] }[]>([]);
  const [dismissed, setDismissed] = useState(dismissedSig);
  const [open, setOpen] = useState(false);
  const { run, busy } = useActions();
  // one update at a time, whichever button started it
  const key = `files:${targetKey()}:update`;
  const updating = busy(key) || !!current;

  const a = app?.game;
  const games: Behind[] = [];
  if (a) {
    const g = { id: a.id, hl: a.hl, dedicated: !!a.dedicated, outdated: a.outdated ?? [], theirs: a.unmanaged ?? [] };
    if (g.outdated.length || g.theirs.length) games.push(g);
    if (withOthers && !a.dedicated)
      for (const o of a.others ?? []) if (o.outdated.length) games.push({ id: o.id, hl: o.hl, dedicated: false, outdated: o.outdated, theirs: [] });
  }
  const sig = signature(games);
  if (!current && (!games.length || dismissed === sig)) return null;

  const name = (id: string) => (id === "core" ? "Loner-Strike's own plugins" : id === "foundation" ? "The bot brain (ReGameDLL)" : scan?.catalog.find((c) => c.id === id)?.name ?? id);
  const title = (g: Behind) => (server ? server : gameTitle(g.id));
  const blocked = gameOpen && games.some((g) => !g.dedicated);
  const all = (g: Behind) => [...g.outdated, ...g.theirs];

  // start one game's update; true when the app took it
  const start = async (g: Behind, ids: string[], i: number, n: number, names: string[]) => {
    const components = ids.filter((c) => c !== "core" && c !== "foundation");
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/setup/install", { hl: g.hl, games: [g.id], components, repair: true }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) {
      toast(r.error, true);
      return false;
    }
    setCurrent({ g, ids, i, n, names });
    return true;
  };
  // each game in turn: the next starts when the one before finishes
  const update = (list: { g: Behind; ids: string[] }[]) =>
    run(key, async () => {
      if (await start(list[0].g, list[0].ids, 1, list.length, list.map((x) => title(x.g)))) setQueue(list.slice(1));
    }, `Updating ${title(list[0].g)}…`);
  const ended = async (ok: boolean) => {
    const done = current!;
    if (ok && queue.length) {
      const [next, ...rest] = queue;
      setQueue(rest);
      if (await start(next.g, next.ids, done.i + 1, done.n, done.names)) return reloadApp();
    }
    setCurrent(null);
    setQueue([]);
    toast(ok ? `Updated ${and(done.names)}` : "The update stopped. The bar shows why.", !ok);
    reloadApp();
  };

  if (current)
    return (
      <div className="panel files-update">
        <JobProgress
          key={`${current.g.id}-${current.i}`}
          title={`Updating ${title(current.g)}…${current.n > 1 ? ` (${current.i} of ${current.n})` : ""}`}
          // a failed update stays on screen with its reason until closed
          onEnd={(ok) => void ended(ok)}
          onClose={() => { setCurrent(null); setQueue([]); }}
        />
      </div>
    );

  const whose = and(games.map((g) => `${title(g)} (${count(g)})`));
  // (QA4-CORE: a disabled button says why when pointed at)
  const closeFirst = `Close ${and([...new Set(games.map((g) => gameTitle(g.id)))])} first`;
  const scope = server ? " Your game and other servers are updated separately." : " Your servers update on their own.";
  const dismiss = () => {
    try { localStorage.setItem(DISMISS(), sig); } catch { /* hidden for this session only */ }
    setDismissed(sig);
  };
  return (
    <>
      <div className="files-bar" role="status">
        <span>
          Updates waiting: {whose}.
          {blocked ? ` Close ${and([...new Set(games.map((g) => gameTitle(g.id)))])} to update.` : ""}
          <span className="muted">{scope}</span>
        </span>
        <span className="files-bar-actions">
          {/* the files are on the computer: a phone only says so */}
          {ON_THIS_COMPUTER ? (
            <>
              <button className="btn small" onClick={() => setOpen(true)}>Details</button>
              <BusyButton className="btn small primary" busy={updating} disabled={blocked} title={blocked ? closeFirst : undefined} label="Update all" busyLabel="Updating…" onClick={() => update(games.map((g) => ({ g, ids: all(g) })))} />
            </>
          ) : (
            <span className="muted">Update on the computer</span>
          )}
          <button className="files-bar-close" aria-label="Hide until new updates" onClick={dismiss}>×</button>
        </span>
      </div>
      {open && (
        <Sheet label="Updates waiting" onClose={() => setOpen(false)} className="sheet files-sheet">
          <div className="sheet-body">
            <h2>Updates waiting</h2>
            <p className="muted small-note">
              {server
                ? "Made here on your computer, then uploaded to the server if it's on another machine."
                : "Newer files from this version of Loner-Strike. Your settings, admins and maps stay."}
            </p>
            {games.map((g) => (
              <section key={g.id} className="files-update-game">
                <div className="files-update-row">
                  <h3>{title(g)}</h3>
                  <button className="btn small" disabled={blocked || updating} title={blocked ? closeFirst : undefined} onClick={() => { setOpen(false); update([{ g, ids: all(g) }]); }}>Update</button>
                </div>
                <ul className="files-update-list">
                  {g.outdated.map((id) => <li key={id}>{name(id)}</li>)}
                  {/* yours is backed up and comes back if you remove the fixed one */}
                  {g.theirs.map((id) => <li key={id}>{name(id)} <span className="muted">(your copy, swapped for the fixed version)</span></li>)}
                </ul>
              </section>
            ))}
            {blocked && <p className="muted small-note">Close the game first.</p>}
            <div className="row">
              <BusyButton className="btn primary" busy={updating} disabled={blocked} title={blocked ? closeFirst : undefined} label="Update all" busyLabel="Updating…" onClick={() => { setOpen(false); update(games.map((g) => ({ g, ids: all(g) }))); }} />
              <button className="btn" onClick={() => setOpen(false)}>Close</button>
            </div>
          </div>
        </Sheet>
      )}
    </>
  );
}
