// History & restore: every file the app changed keeps its earlier versions (app/src-tauri/src/safety.rs).
// One component for every tool and for Settings ▸ Backups & defaults:
//
//   <ToolHistory tool="people" />            the "History" button in a tool's title row (Tools.tsx)
//   <HistoryButton match={["maps/x.nav"]} />  the same for any list of files (path ends, * allowed)
//   <HistoryPanel match={...} tools={...} />  the list itself
//
// Per file: its versions (when, which tool, what changed), Compare (text files), Restore, and
// "Restore Loner-Strike's default". Nothing changes without a confirm saying exactly what, and
// every restore keeps the file as it is first, so it can be undone the same way.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { useToolArg } from "../../../stores/tools";
import { Sheet, useConfirm, useToast } from "../../../ui";
import { get, post, useBusy } from "./api";
import "./history.css";

export type SafetyVersion = {
  seq: number;
  time: number;
  tool: string;
  reason: string;
  existed: boolean;
  size: number;
  original: boolean;
  kept: boolean;
  created?: boolean;
  deleted?: boolean;
  added?: number;
  removed?: number;
  same?: boolean;
  text?: boolean;
};
export type SafetyFile = {
  id: string;
  rel: string;
  name: string;
  area: string;
  exists: boolean;
  size?: number;
  text?: boolean;
  default: "shipped" | "original" | null;
  changed: boolean;
  defaultGone?: boolean;
  versions: SafetyVersion[];
};

/** Which files each tool writes: path ends (see safety.rs path_matches) and/or the tool's name
 *  in the backups. A tool that writes nothing has no entry (and no History button). */
export function historyFor(tool: string, arg: string | null): { match: string[]; tools: string[] } | null {
  const map = arg && /^[\w.+-]+$/.test(arg) ? arg : null;
  const GM = "addons/amxmodx/configs/gamemodes/";
  switch (tool) {
    case "people":
      return { match: [`${GM}people.ini`, `${GM}regular_names.ini`, "addons/amxmodx/data/gamemodes/regulars.ini"], tools: ["Bot people"] };
    case "playercfg":
      return { match: ["userconfig.cfg"], tools: ["Crosshair & config"] };
    case "spray":
      return { match: ["tempdecal.wad"], tools: ["Spray maker"] };
    case "doctor":
      return { match: map ? [`maps/${map}.res`] : [], tools: ["Map doctor"] };
    case "callouts":
      return { match: map ? [`maps/${map}.nav`] : [], tools: ["Callouts"] };
    case "navfix":
      return { match: map ? [`maps/${map}.nav`, `maps/${map}.lsv`] : [], tools: ["Nav fixer"] };
    case "radar":
      return { match: map ? [`overviews/${map}.bmp`, `overviews/${map}.tga`, `overviews/${map}.txt`] : [], tools: ["Radar maker"] };
    case "plugins":
      return { match: ["addons/amxmodx/configs/plugins-studio.ini"], tools: ["Plugin Studio"] };
    case "assets":
      return { match: [], tools: ["Textures & models"] };
    case "editor":
      return { match: [], tools: ["Map editor", "Compile"] };
    case "demos":
      return { match: [], tools: ["Demos"] };
    case "community":
      return { match: [], tools: ["Downloads"] };
    case "creatures":
      return { match: [`${GM}creatures.ini`, `${GM}waves.ini`], tools: ["Creature Studio"] };
    case "studio":
      return { match: [], tools: ["Game Mode Studio"] };
    default:
      return null;
  }
}

// ---- words -------------------------------------------------------------------------------------

const DAY = 86_400_000;
/** "Today 14:05", "Yesterday 09:12", "3 Nov 14:05", "3 Nov 2025" */
export function whenText(secs: number): string {
  const d = new Date(secs * 1000);
  const now = new Date();
  const time = d.toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  if (d.getTime() >= startOfToday) return `Today ${time}`;
  if (d.getTime() >= startOfToday - DAY) return `Yesterday ${time}`;
  if (d.getFullYear() === now.getFullYear()) return `${d.toLocaleDateString(undefined, { day: "numeric", month: "short" })} ${time}`;
  return d.toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

export function sizeText(n: number): string {
  if (n < 1024) return `${n} byte${n === 1 ? "" : "s"}`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`;
  // a round limit like 2,000,000,000 bytes reads as "2 GB", not "1907.3 MB" (QA4-CORE)
  if (n >= 1e9 && n % 1e8 === 0) return `${+(n / 1e9).toFixed(1)} GB`;
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(1)} GB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

/** What the change made just after this version did, in a few words. */
function changeText(v: SafetyVersion, afterSize: number | null): string {
  if (v.created) return "made the file";
  if (v.deleted) return "deleted the file";
  if (!v.existed) return "the file wasn't there";
  if (v.same) return "no visible change";
  if (v.added !== undefined && v.removed !== undefined) {
    const parts = [];
    if (v.added) parts.push(`+${v.added} line${v.added === 1 ? "" : "s"}`);
    if (v.removed) parts.push(`−${v.removed} line${v.removed === 1 ? "" : "s"}`);
    return parts.length ? parts.join(", ") : "lines moved";
  }
  if (afterSize !== null && afterSize !== v.size) {
    const d = afterSize - v.size;
    return `${d > 0 ? "+" : "−"}${sizeText(Math.abs(d))}`;
  }
  return sizeText(v.size);
}

function defaultWords(f: SafetyFile): string {
  if (f.default === "shipped") return `Put ${f.rel} back the way Loner-Strike ships it?`;
  if (f.defaultGone) return `Remove ${f.rel}? It wasn't there before Loner-Strike's tools made it.`;
  return `Put ${f.rel} back the way it was before Loner-Strike's tools first changed it?`;
}

const KEPT_NOTE = "The file as it is now is kept in its history, so you can undo this.";

// ---- comparing ---------------------------------------------------------------------------------

export type DiffLine = { kind: " " | "+" | "-"; text: string };

/** A line diff from `a` to `b` (longest common subsequence). null when too big to compare. */
export function lineDiff(a: string, b: string, maxCells = 6_000_000): DiffLine[] | null {
  const x = a.replace(/\r\n/g, "\n").replace(/\n$/, "").split("\n");
  const y = b.replace(/\r\n/g, "\n").replace(/\n$/, "").split("\n");
  // the same start and end aren't part of the table
  let s = 0;
  while (s < x.length && s < y.length && x[s] === y[s]) s++;
  let e = 0;
  while (e < x.length - s && e < y.length - s && x[x.length - 1 - e] === y[y.length - 1 - e]) e++;
  const xs = x.slice(s, x.length - e);
  const ys = y.slice(s, y.length - e);
  const n = xs.length;
  const m = ys.length;
  if ((n + 1) * (m + 1) > maxCells) return null;
  const t = new Uint32Array((n + 1) * (m + 1));
  for (let i = n - 1; i >= 0; i--)
    for (let j = m - 1; j >= 0; j--) t[i * (m + 1) + j] = xs[i] === ys[j] ? t[(i + 1) * (m + 1) + j + 1] + 1 : Math.max(t[(i + 1) * (m + 1) + j], t[i * (m + 1) + j + 1]);
  const out: DiffLine[] = x.slice(0, s).map((text) => ({ kind: " ", text }));
  let i = 0;
  let j = 0;
  while (i < n && j < m) {
    if (xs[i] === ys[j]) {
      out.push({ kind: " ", text: xs[i] });
      i++;
      j++;
    } else if (t[(i + 1) * (m + 1) + j] >= t[i * (m + 1) + j + 1]) out.push({ kind: "-", text: xs[i++] });
    else out.push({ kind: "+", text: ys[j++] });
  }
  while (i < n) out.push({ kind: "-", text: xs[i++] });
  while (j < m) out.push({ kind: "+", text: ys[j++] });
  for (const text of x.slice(x.length - e)) out.push({ kind: " ", text });
  return out;
}

/** Only the changed lines and 3 around each; the rest folded into "… n lines the same". */
function folded(d: DiffLine[]): (DiffLine | { fold: number })[] {
  const keep = d.map(() => false);
  d.forEach((l, i) => {
    if (l.kind !== " ") for (let k = Math.max(0, i - 3); k <= Math.min(d.length - 1, i + 3); k++) keep[k] = true;
  });
  const out: (DiffLine | { fold: number })[] = [];
  let run = 0;
  d.forEach((l, i) => {
    if (keep[i]) {
      if (run) out.push({ fold: run });
      run = 0;
      out.push(l);
    } else run++;
  });
  if (run) out.push({ fold: run });
  return out;
}

type VersionText = { then: string | null; now: string | null; thenExists: boolean; nowExists: boolean; binary: boolean };

function Compare({ file, v, onClose }: { file: SafetyFile; v: SafetyVersion; onClose: () => void }) {
  const q = useQuery({ queryKey: ["safety-version", file.id, v.seq], queryFn: ({ signal }) => get<VersionText>(`/api/safety/version?id=${encodeURIComponent(file.id)}&seq=${v.seq}`, signal) });
  const diff = useMemo(() => (q.data && !q.data.binary ? lineDiff(q.data.now ?? "", q.data.then ?? "") : null), [q.data]);
  const changes = diff ? diff.filter((l) => l.kind !== " ").length : 0;
  return (
    <Sheet className="sheet wide hist-sheet" label={`Compare ${file.name}`} onClose={onClose}>
      <div className="sheet-body">
        <h2>Compare {file.name}</h2>
        <p className="muted">
          What restoring the version from {whenText(v.time)} would change. <span className="hist-minus">Red lines</span> would go, <span className="hist-plus">green lines</span> would come back.
        </p>
        {q.isPending && <p className="muted" role="status"><span className="spinner" aria-hidden="true" /> Reading both versions…</p>}
        {q.isError && <p className="hist-error" role="alert">{(q.error as Error).message}</p>}
        {q.data && q.data.binary && <p className="muted">This isn't a text file, so it can't be compared line by line. It's {sizeText(v.size)} in that version{file.exists ? ` and ${sizeText(file.size ?? 0)} now` : ", and not there now"}.</p>}
        {q.data && !q.data.binary && !q.data.thenExists && <p className="muted">The file wasn't there then: restoring this version removes it.</p>}
        {q.data && !q.data.binary && q.data.thenExists && !diff && <p className="muted">These versions are too big to compare here.</p>}
        {diff && changes === 0 && <p className="muted">This version is the same as the file now.</p>}
        {diff && changes > 0 && (
          <pre className="hist-diff" tabIndex={0} aria-label="Differences">
            {folded(diff).map((l, i) =>
              "fold" in l ? (
                <span key={i} className="hist-fold">… {l.fold} line{l.fold === 1 ? "" : "s"} the same{"\n"}</span>
              ) : (
                <span key={i} className={l.kind === "+" ? "hist-add" : l.kind === "-" ? "hist-del" : undefined}>
                  {l.kind === " " ? "  " : `${l.kind === "+" ? "+" : "−"} `}
                  {l.text}
                  {"\n"}
                </span>
              ),
            )}
          </pre>
        )}
        <div className="row">
          <button className="btn" autoFocus onClick={onClose}>Close</button>
        </div>
      </div>
    </Sheet>
  );
}

// ---- the list ----------------------------------------------------------------------------------

function useSafetyActions(refresh: () => void) {
  const ask = useConfirm();
  const toast = useToast();
  const qc = useQueryClient();
  const { working, run } = useBusy();
  const done = (r: { error?: string; message?: string }) => {
    if (r.error) toast(r.error, true);
    else if (r.message) toast(r.message);
    refresh();
    // pages showing these files read them again
    qc.invalidateQueries();
    window.dispatchEvent(new CustomEvent("ls-files-restored"));
  };
  const restore = async (f: SafetyFile, v: SafetyVersion) => {
    const what = v.existed ? `Put ${f.name} back the way it was before “${v.reason}” (${whenText(v.time)})?` : `Remove ${f.name}? It wasn't there before “${v.reason}” (${whenText(v.time)}).`;
    if (!(await ask(`${what} ${KEPT_NOTE}`, { ok: v.existed ? "Restore" : "Remove it" }))) return;
    await run("Restoring…", async () => done(await post("/api/safety/restore", { id: f.id, seq: v.seq })));
  };
  const defaults = async (files: SafetyFile[]) => {
    const list = files.filter((f) => f.changed && f.default);
    if (!list.length) return;
    const text =
      list.length === 1
        ? `${defaultWords(list[0])} ${KEPT_NOTE}`
        : `Put these ${list.length} files back to Loner-Strike's defaults? ${list.map((f) => f.rel).join(", ")}. ${KEPT_NOTE}`;
    if (!(await ask(text, { ok: list.length === 1 ? "Restore default" : "Restore defaults", danger: true }))) return;
    await run("Restoring defaults…", async () => done(await post("/api/safety/defaults", { ids: list.map((f) => f.id) })));
  };
  return { working, restore, defaults };
}

function FileHistory({ f, actions, open }: { f: SafetyFile; actions: ReturnType<typeof useSafetyActions>; open?: boolean }) {
  const [compare, setCompare] = useState<SafetyVersion | null>(null);
  const [all, setAll] = useState(false);
  const shown = all ? f.versions : f.versions.slice(0, 6);
  return (
    <details className="hist-file" open={open}>
      <summary>
        <span className="hist-name">{f.name}</span>
        {!f.exists && <span className="tag hist-gone">Not there now</span>}
        {f.changed && f.default === "shipped" && <span className="tag">Changed from Loner-Strike's default</span>}
        {f.changed && f.default === "original" && <span className="tag">Changed</span>}
        <span className="muted hist-count">{f.versions.length} earlier version{f.versions.length === 1 ? "" : "s"}</span>
      </summary>
      <p className="muted mono hist-rel">{f.rel}</p>
      {f.changed && f.default && (
        <div className="row">
          <button className="btn small" disabled={!!actions.working} onClick={() => actions.defaults([f])}>
            {f.default === "shipped" ? "Restore Loner-Strike's default" : f.defaultGone ? "Remove it (it wasn't there before)" : "Restore the original"}
          </button>
        </div>
      )}
      <ol className="hist-versions">
        {shown.map((v, i) => {
          const afterSize = i === 0 ? (f.exists ? f.size ?? null : null) : f.versions[i - 1].size;
          return (
            <li key={v.seq} className="hist-version">
              <div className="hist-what">
                <span className="hist-when">{whenText(v.time)}</span>
                <span>
                  <b>{v.tool}</b>: {v.reason}
                </span>
                <span className="muted">{changeText(v, afterSize)}</span>
                {v.original && <span className="tag" title="The file before Loner-Strike's tools first changed it. Always kept.">Original</span>}
              </div>
              <div className="row nowrap hist-buttons">
                {v.existed && v.text && (f.text ?? true) && (
                  <button className="btn small" onClick={() => setCompare(v)} aria-label={`Compare the version from ${whenText(v.time)} with now`}>
                    Compare
                  </button>
                )}
                <button className="btn small" disabled={!!actions.working || !v.kept} title={v.kept ? undefined : "This version was too big to keep a copy of."} onClick={() => actions.restore(f, v)} aria-label={`Restore the version from ${whenText(v.time)}`}>
                  {v.existed ? "Restore" : "Remove"}
                </button>
              </div>
            </li>
          );
        })}
      </ol>
      {f.versions.length > shown.length && (
        <button className="btn small" onClick={() => setAll(true)}>Show all {f.versions.length}</button>
      )}
      {compare && <Compare file={f} v={compare} onClose={() => setCompare(null)} />}
    </details>
  );
}

/** The history of the files matching `match` (path ends) or changed by `tools`. */
export function HistoryPanel({ match, tools = [], none }: { match: string[]; tools?: string[]; none?: string }) {
  const key = `/api/safety/files?match=${encodeURIComponent(match.join(","))}&tool=${encodeURIComponent(tools.join(","))}`;
  const q = useQuery({ queryKey: ["safety-files", key], queryFn: ({ signal }) => get<{ files: SafetyFile[] }>(key, signal) });
  const actions = useSafetyActions(() => q.refetch());
  const files = q.data?.files ?? [];
  const changed = files.filter((f) => f.changed && f.default);
  return (
    <div className="hist" aria-busy={q.isFetching || !!actions.working}>
      {actions.working && (
        <p className="muted" role="status">
          <span className="spinner" aria-hidden="true" /> {actions.working}
        </p>
      )}
      {q.isPending && <p className="muted" role="status"><span className="spinner" aria-hidden="true" /> Reading the history…</p>}
      {q.isError && <p className="hist-error" role="alert">{(q.error as Error).message}</p>}
      {q.data && !files.length && <p className="muted">{none ?? "Nothing changed here yet. Each time this tool saves a file, the version before is kept here, so you can always go back."}</p>}
      {changed.length > 1 && (
        <div className="row">
          <button className="btn" disabled={!!actions.working} onClick={() => actions.defaults(changed)}>
            Restore defaults for all {changed.length}
          </button>
        </div>
      )}
      {files.map((f) => (
        <FileHistory key={f.id} f={f} actions={actions} open={files.length === 1} />
      ))}
    </div>
  );
}

/** A "History" button opening the history of these files in a sheet. */
/** A clock with an arrow going back, drawn like the tool header's other icons (Focus, Docs). */
const HistoryIcon = () => (
  <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    <path d="M3 12a9 9 0 1 0 3-6.7L3 8M3 3v5h5M12 7v5l3 2" />
  </svg>
);

/** `small`: the tool header's size, with an icon, like Focus and Docs beside it. */
export function HistoryButton({ match, tools, label = "History", title = "History & restore", small }: { match: string[]; tools?: string[]; label?: string; title?: string; small?: boolean }) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button className={small ? "btn small tools-history-btn" : "btn"} onClick={() => setOpen(true)} title="Earlier versions of the files this tool changed: compare, restore, or go back to Loner-Strike's defaults">
        {small && <HistoryIcon />}
        {label}
      </button>
      {open && (
        <Sheet className="sheet wide hist-sheet" label={title} onClose={() => setOpen(false)}>
          <div className="sheet-body">
            <h2>{title}</h2>
            <p className="muted">Every time a file is saved, the version before is kept (the last 20, and always the first one). Restoring keeps the current file too.</p>
            <HistoryPanel match={match} tools={tools} />
            <div className="row">
              <button className="btn" onClick={() => setOpen(false)}>Close</button>
            </div>
          </div>
        </Sheet>
      )}
    </>
  );
}

/** The History button for a tool (nothing for a tool that doesn't write files). */
export function ToolHistory({ tool }: { tool: string }) {
  const arg = useToolArg();
  const h = historyFor(tool, arg);
  if (!h || (!h.match.length && !h.tools.length)) return null;
  return <HistoryButton match={h.match} tools={h.tools} small />;
}
