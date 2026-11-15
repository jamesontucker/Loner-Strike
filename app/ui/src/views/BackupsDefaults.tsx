// Settings ▸ Backups & defaults: everything that's different from Loner-Strike's defaults, grouped
// by area, each restorable; "Restore all defaults"; how much room the backups take, and clearing
// old ones. The backups themselves: app/src-tauri/src/safety.rs. Loaded lazily (AppSettings).

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { BusyButton, LoadError, SectionHead, useConfirm, useToast } from "../ui";
import { get, post, useBusy } from "./tools/common/api";
import { HistoryButton, sizeText, type SafetyFile } from "./tools/common/History";
import "./tools/common/history.css";

type Changed = { files: SafetyFile[]; usage: { bytes: number; files: number; versions: number; cap: number } };

const AREAS = ["Bot people", "Settings files", "Game modes", "Plugins", "Maps & nav", "Sprays & textures"];

export default function BackupsDefaults() {
  const q = useQuery({ queryKey: ["safety-changed"], queryFn: ({ signal }) => get<Changed>("/api/safety/changed", signal) });
  const qc = useQueryClient();
  const ask = useConfirm();
  const toast = useToast();
  const { working, run } = useBusy();
  const files = (q.data?.files ?? []).filter((f) => f.default);
  const usage = q.data?.usage;
  const groups = AREAS.map((a) => [a, files.filter((f) => f.area === a)] as const).filter(([, l]) => l.length);

  const done = (r: { error?: string; message?: string }) => {
    if (r.error) toast(r.error, true);
    else if (r.message) toast(r.message);
    qc.invalidateQueries();
  };
  const restore = async (list: SafetyFile[], label: string) => {
    const names = list.map((f) => f.rel);
    const text =
      list.length === 1
        ? `Put ${names[0]} back to Loner-Strike's default? ${list[0].default === "original" ? (list[0].defaultGone ? "It wasn't there before Loner-Strike's tools made it, so it's removed." : "That's how it was before Loner-Strike's tools first changed it.") : "That's how Loner-Strike ships it."} The file as it is now is kept in its history, so you can undo this.`
        : `Put these ${list.length} files back to Loner-Strike's defaults? ${names.join(", ")}. Each one as it is now is kept in its history, so you can undo this.`;
    if (!(await ask(text, { ok: list.length === 1 ? "Restore default" : "Restore defaults", danger: true }))) return;
    await run(label, async () => done(await post("/api/safety/defaults", { ids: list.map((f) => f.id) })));
  };
  const restoreAll = async () => {
    if (!(await ask(`Put all ${files.length} changed files back to Loner-Strike's defaults? That's every bot people file, setting, plugin list, map file and mode setting listed here. Each one as it is now is kept in its history.`, { ok: "Continue", danger: true }))) return;
    if (!(await ask(`Are you sure? ${files.length} file${files.length === 1 ? "" : "s"} change now: ${files.map((f) => f.name).slice(0, 12).join(", ")}${files.length > 12 ? ` and ${files.length - 12} more` : ""}.`, { ok: "Restore all defaults", danger: true }))) return;
    await run("Restoring all defaults…", async () => done(await post("/api/safety/defaults", { ids: files.map((f) => f.id) })));
  };
  const clear = async () => {
    if (!(await ask("Clear old backups? Every file keeps its original and its newest version; the versions in between are deleted for good.", { ok: "Clear old backups", danger: true }))) return;
    await run("Clearing…", async () => done(await post("/api/safety/clear")));
  };

  return (
    <section>
      <SectionHead docs="backups" title="Backups & defaults" note="Each time Loner-Strike or one of its tools changes a file in your game, the version before is kept: the last 20, and always the first one. Here's everything that's different from Loner-Strike's defaults. Each tool's own History button has the rest." />
      <div className="panel settings-list">
        {q.isError && !q.data && <LoadError text="Couldn't read the backups." onRetry={() => q.refetch()} />}
        {q.isPending && (
          <p className="muted" role="status">
            <span className="spinner" aria-hidden="true" /> Checking your files against Loner-Strike's defaults…
          </p>
        )}
        {working && (
          <p className="muted" role="status">
            <span className="spinner" aria-hidden="true" /> {working}
          </p>
        )}
        {q.data && !files.length && <p className="muted">Everything is the way Loner-Strike ships it.</p>}
        {groups.map(([area, list]) => (
          <div key={area} className="bk-group setting-line">
            <div className="row between">
              <h4>{area}</h4>
              {list.length > 1 && (
                <button className="btn small" disabled={!!working} onClick={() => restore(list, `Restoring ${area.toLowerCase()}…`)}>
                  Restore all {list.length}
                </button>
              )}
            </div>
            <ul className="bk-files">
              {list.map((f) => (
                <li key={f.id}>
                  <div>
                    <b>{f.name}</b>
                    <p className="muted mono">{f.rel}</p>
                  </div>
                  <div className="row nowrap">
                    {f.versions.length > 0 && <HistoryButton match={[f.rel]} label="History" title={`History of ${f.name}`} />}
                    <button className="btn" disabled={!!working} onClick={() => restore([f], "Restoring…")}>
                      Restore default
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          </div>
        ))}
        {files.length > 1 && (
          <div className="row between setting-line">
            <div>
              <b>Restore all defaults</b>
              <p className="muted">Every file listed above, at once. Asks twice.</p>
            </div>
            <button className="btn danger" disabled={!!working} onClick={restoreAll}>Restore all defaults</button>
          </div>
        )}
        <div className="row between setting-line">
          <div>
            <b>Backups</b> {usage && <span className="muted">{sizeText(usage.bytes)}</span>}
            <p className="muted">
              {usage ? `${usage.files} file${usage.files === 1 ? "" : "s"}, ${usage.versions} version${usage.versions === 1 ? "" : "s"} kept. The oldest go by themselves past ${sizeText(usage.cap)}.` : "Earlier versions of the files Loner-Strike changed."}
            </p>
          </div>
          <BusyButton busy={working === "Clearing…"} disabled={!usage?.versions || !!working} label="Clear old backups" busyLabel="Clearing…" onClick={clear} />
        </div>
      </div>
    </section>
  );
}
