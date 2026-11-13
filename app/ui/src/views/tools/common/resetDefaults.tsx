// "Reset to Loner-Strike's default" for a tool: every file the tool writes goes back to what
// Loner-Strike ships (or, for a file it doesn't ship, to the file before the tools first changed
// it), through the safety net (safety.rs /api/safety/defaults), so it can be undone from History.
// One confirm sheet lists exactly which files change. After it, the tool's page starts over so
// it reads the files again.
//
//   const reset = useResetDefaults("people");          // the files historyFor("people") names
//   reset.open()  ->  confirm sheet  ->  restored, page restarts
//   {reset.sheet}                                       // render it somewhere in the page
//
// The action bar (ActionBar `reset`) does all of this for you.

import { useQueryClient } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";
import { useToolArg } from "../../../stores/tools";
import { Sheet, useToast } from "../../../ui";
import { historyFor, type SafetyFile } from "./History";
import { post, tryGet, useBusy } from "./api";
import { useShell } from "./shell";

export type ResetFile = { id: string; rel: string; name: string; to: "shipped" | "original" | "remove" };

/** path ends with `*` allowed, like safety.rs path_matches */
function matchesEnd(pattern: string, rel: string) {
  const re = new RegExp(`(^|/)${pattern.split("*").map((x) => x.replace(/[.+?^${}()|[\]\\]/g, "\\$&")).join("[^/]*")}$`, "i");
  return re.test(rel.replace(/\\/g, "/"));
}

/** The files `match` names that differ from their default now. */
export async function filesToReset(match: string[], tools: string[] = []): Promise<{ files: ResetFile[]; error?: string }> {
  if (!match.length) return { files: [] };
  const q = `/api/safety/files?match=${encodeURIComponent(match.join(","))}&tool=${encodeURIComponent(tools.join(","))}`;
  const [kept, changed] = await Promise.all([tryGet<{ files: SafetyFile[] }>(q), tryGet<{ files: SafetyFile[] }>("/api/safety/changed")]);
  if (kept.error && changed.error) return { files: [], error: kept.error };
  const ours = (f: SafetyFile) => match.some((m) => matchesEnd(m, f.rel));
  const out: ResetFile[] = [];
  const seen = new Set<string>();
  for (const f of [...(kept.files ?? []), ...(changed.files ?? [])]) {
    if (!f.changed || !f.default || !ours(f) || seen.has(f.rel)) continue;
    seen.add(f.rel);
    out.push({ id: f.id, rel: f.rel, name: f.name, to: f.default === "shipped" ? "shipped" : f.defaultGone ? "remove" : "original" });
  }
  return { files: out };
}

const TO_WORDS: Record<ResetFile["to"], string> = {
  shipped: "back to Loner-Strike's default",
  original: "back to the file before Loner-Strike's tools changed it",
  remove: "removed (it wasn't there before)",
};

/** Reset for one tool. `match` overrides the files (default: what History lists for the tool);
 *  `dirty`: the page has unsaved changes (the sheet says they go too). */
export function useResetDefaults(tool: string, opts: { match?: string[]; dirty?: boolean; label?: string } = {}) {
  const arg = useToolArg();
  const toast = useToast();
  const qc = useQueryClient();
  const { working, run } = useBusy();
  const [files, setFiles] = useState<ResetFile[] | null>(null);
  const match = opts.match ?? historyFor(tool, arg)?.match ?? [];

  const open = () =>
    run("Checking…", async () => {
      const r = await filesToReset(match);
      if (r.error) return toast(r.error, true);
      if (!r.files.length) return toast(match.length ? "Everything here is already Loner-Strike's default." : "This tool has no files to reset.");
      setFiles(r.files);
    });

  const confirm = () =>
    run("Resetting…", async () => {
      const list = files ?? [];
      const r = await post<{ message?: string }>("/api/safety/defaults", { ids: list.map((f) => f.id) });
      setFiles(null);
      if (r.error) return toast(r.error, true);
      toast(r.message || "Back to Loner-Strike's defaults.");
      qc.invalidateQueries();
      window.dispatchEvent(new CustomEvent("ls-files-restored"));
      // the page reads its files again (unsaved edits were in the sheet's warning)
      useShell.setState((s) => ({ n: s.n + 1 }));
    });

  const sheet: ReactNode = files && (
    <Sheet className="sheet tl-reset" label="Reset to Loner-Strike's default" onClose={() => setFiles(null)}>
      <div className="sheet-body">
        <h2>Reset to Loner-Strike's default?</h2>
        <p>
          {files.length === 1 ? "This file changes:" : `These ${files.length} files change:`}
        </p>
        <ul className="tl-reset-files">
          {files.map((f) => (
            <li key={f.id}>
              <span className="mono">{f.rel}</span>
              <span className="muted">{TO_WORDS[f.to]}</span>
            </li>
          ))}
        </ul>
        <p className="muted">
          {opts.dirty ? "Changes you haven't saved are dropped too. " : ""}
          The files as they are now are kept first: History (at the top of the page) or Undo puts them back.
        </p>
        <div className="row ask-buttons">
          <button type="button" className="btn" autoFocus onClick={() => setFiles(null)}>
            Cancel
          </button>
          <button type="button" className="btn danger strong" disabled={!!working} aria-busy={working === "Resetting…"} onClick={confirm}>
            {working === "Resetting…" ? (
              <>
                <span className="spinner" aria-hidden="true" /> Resetting…
              </>
            ) : (
              "Reset"
            )}
          </button>
        </div>
      </div>
    </Sheet>
  );

  return { open, sheet, working, available: match.length > 0, label: opts.label ?? "Reset to Loner-Strike's default…" };
}
