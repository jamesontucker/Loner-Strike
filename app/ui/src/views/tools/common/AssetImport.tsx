// One piece every asset picker shares (Creature Studio's model and sound pickers, Game Mode
// Studio's model, sprite and sound inputs, the map editor's Choose… for game-level keys):
// Import… and drag-and-drop of a file from the computer, "From your map projects" (files
// imported into a map project, copied into the game when picked), a refresh button and a
// rescan when the window comes back. The server checks every file the same way as the
// Content Browser's import (tools/assets.rs): the real type, sizes, safe names, never over a
// game file or one that's there (a taken name asks for another).

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { BusyButton, useAskText, useToast } from "../../../ui";
import { post, tryGet } from "./api";
import { matches, searchWords } from "./search";
import { fileData } from "../editor/contentApi";
import { MakeSpriteButton } from "./SpriteMaker"; // T1fix: the Sprite maker
import "./assetImport.css";

export type ImportKind = "model" | "sprite" | "sound";
/** What's known about where a file came from and what it allows. */
export type AssetCredit = { verdict?: "ok" | "ask" | "no" | "stock" | "unknown"; authors?: { name: string }[]; page?: { url: string; name: string } | null; note?: string } | null;
export type Imported = { ok?: boolean; path?: string; files?: string[]; credit?: AssetCredit; cancelled?: boolean; taken?: { name: string; suggestion: string }; error?: string };
/** `project` is the project file (sent back as it is to copy one in); `projectName` is for people */
export type ProjectFile = { project: string; projectName?: string; path: string; name: string; size: number; credit?: AssetCredit };

const EXT: Record<ImportKind, string> = { model: ".mdl", sprite: ".spr", sound: ".wav" };
export const ACCEPT: Record<ImportKind, string> = { model: ".mdl", sprite: ".spr", sound: ".wav" };
export const IMPORT_NOTE = "Files you add here go into your game, so creatures and modes can use them. Sharing packages them for you.";

export const importAsset = (body: Record<string, unknown>) => post<Imported>("/api/tools/assets/import", body);
export const adoptAsset = (body: Record<string, unknown>) => post<Imported>("/api/tools/assets/adopt", body);
export const projectFiles = (kind: ImportKind, signal?: AbortSignal) => tryGet<{ files: ProjectFile[] }>(`/api/tools/assets/project-files?kind=${kind}`, signal);

/** Everything a picker needs to bring a file in. `onPicked` gets the game-relative path
 *  (models/…, sprites/…, sound/…) and selects it; `onRescan` refreshes the picker's own list. */
export function useAssetImport(kind: ImportKind, onPicked: (path: string, credit?: AssetCredit) => void, onRescan?: () => void) {
  const toast = useToast();
  const askText = useAskText();
  const qc = useQueryClient();
  const [busy, setBusy] = useState<"import" | "copy" | null>(null);
  const [hot, setHot] = useState(false);

  /** Sends a request; on a taken name asks for another and sends it again with that name. */
  const send = async (call: (rename?: string) => Promise<Imported>): Promise<void> => {
    let rename: string | undefined;
    for (let tries = 0; tries < 4; tries++) {
      const r = await call(rename);
      if (r.cancelled) return;
      if (r.taken) {
        const ext = EXT[kind];
        const typed = await askText(
          `There's already a ${r.taken.name} in your game. Bring it in under another name (it never replaces a file).`,
          { label: "New name", value: r.taken.suggestion, max: 60 },
          {
            ok: "Use this name",
            check: (v) => {
              const n = v.trim();
              if (!n) return "Type a name.";
              if (!/^[A-Za-z0-9_][A-Za-z0-9_.-]*$/.test(n.replace(/\.(mdl|spr|wav)$/i, ""))) return "Use letters, numbers, - and _.";
              const full = n.toLowerCase().endsWith(ext) ? n : `${n}${ext}`;
              if (full.toLowerCase() === r.taken!.name.toLowerCase()) return `${full} is taken.`;
              return null;
            },
          },
        );
        if (typed === null) return;
        rename = typed.trim().toLowerCase().endsWith(ext) ? typed.trim() : `${typed.trim()}${ext}`;
        continue;
      }
      if (r.error || !r.ok || !r.path) return void toast(r.error || "Couldn't bring that file in.", true);
      onPicked(r.path, r.credit);
      onRescan?.();
      void qc.invalidateQueries({ queryKey: ["tools", "assets", "project-files", kind] });
      toast(`${r.path.replace(/^.*\//, "")} is in your game now.`);
      return;
    }
  };

  const importFile = async (f?: File) => {
    if (busy) return;
    if (f && !f.name.toLowerCase().endsWith(EXT[kind])) return void toast(`That isn't a ${EXT[kind]} file.`, true);
    setBusy("import");
    try {
      const data = f ? await fileData(f) : null;
      await send((rename) => importAsset(f ? { kind, name: f.name, data, rename } : { kind, pick: true, rename }));
    } finally {
      setBusy(null);
    }
  };

  const adopt = async (p: ProjectFile) => {
    if (busy) return;
    setBusy("copy");
    try {
      await send((rename) => adoptAsset({ kind, project: p.project, path: p.path, rename }));
    } finally {
      setBusy(null);
    }
  };

  // a file dragged from the computer onto the picker
  const dropProps = {
    onDragOver: (e: React.DragEvent) => {
      if (!e.dataTransfer.types.includes("Files")) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
      if (!hot) setHot(true);
    },
    onDragLeave: () => setHot(false),
    onDrop: (e: React.DragEvent) => {
      setHot(false);
      const f = e.dataTransfer.files?.[0];
      if (!f) return;
      e.preventDefault();
      e.stopPropagation();
      void importFile(f);
    },
  };

  return { busy, hot, importFile, adopt, dropProps };
}

/** Calls `refresh` when the window comes back (a file may have been added meanwhile). */
export function useFocusRefresh(refresh: () => void) {
  useEffect(() => {
    const back = () => !document.hidden && refresh();
    window.addEventListener("focus", back);
    document.addEventListener("visibilitychange", back);
    return () => {
      window.removeEventListener("focus", back);
      document.removeEventListener("visibilitychange", back);
    };
  }, [refresh]);
}

/** The row under a picker's search: Import… and refresh, and the note. */
export function ImportBar({ kind, imp, onRefresh, refreshing }: { kind: ImportKind; imp: ReturnType<typeof useAssetImport>; onRefresh: () => void; refreshing?: boolean }) {
  return (
    <div className="ai-bar">
      <BusyButton className="btn small" busy={imp.busy === "import"} disabled={!!imp.busy} label="Import…" busyLabel="Importing…" onClick={() => void imp.importFile()} title={`Add a ${EXT[kind]} file from your computer (or drop one here)`} />
      {kind === "sprite" && (
        <MakeSpriteButton
          onMade={async (f) => {
            await imp.importFile(f);
            return true;
          }}
        />
      )}
      <button type="button" className="btn small icon ai-refresh" aria-label="Refresh the list" title="Look again" disabled={refreshing} onClick={onRefresh}>
        {refreshing ? <span className="spinner" aria-hidden="true" /> : "↻"}
      </button>
      {imp.busy === "copy" && (
        <span className="muted ai-busy" role="status">
          <span className="spinner" aria-hidden="true" /> Copying into the game…
        </span>
      )}
    </div>
  );
}

/** "From your map projects": files imported into a map project, listed with the picker's
 *  search; picking one copies it into the game, then selects it. */
export function ProjectFilesGroup({ kind, q, imp }: { kind: ImportKind; q: string; imp: ReturnType<typeof useAssetImport> }) {
  const files = useQuery({ queryKey: ["tools", "assets", "project-files", kind], queryFn: ({ signal }) => projectFiles(kind, signal), staleTime: 15_000 });
  const shown = useMemo(() => {
    const w = searchWords(q);
    return (files.data?.files ?? []).filter((f) => matches(w, f.path, f.projectName ?? f.project)).slice(0, 40);
  }, [files.data, q]);
  if (!shown.length) return null;
  return (
    <div className="ai-projects" role="group" aria-label="From your map projects">
      <h5 className="ai-group">From your map projects</h5>
      <ul className="cs-pop-list">
        {shown.map((f) => (
          <li key={`${f.project}/${f.path}`}>
            <button type="button" className="cs-pick" disabled={!!imp.busy} onClick={() => void imp.adopt(f)} title={`Copies ${f.path} from ${f.projectName ?? f.project} into your game`}>
              <span>{f.path.replace(/^(models|sprites|sound)\//, "")}</span>
              <span className="muted">{f.projectName ?? f.project.split(/[\\/]/).pop()}</span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

/** Nothing matches: offer Import. */
export function EmptyImport({ kind, imp, what }: { kind: ImportKind; imp: ReturnType<typeof useAssetImport>; what: string }) {
  return (
    <p className="muted cs-none ai-empty">
      No {what} matches.{" "}
      <button type="button" className="link inline" disabled={!!imp.busy} onClick={() => void imp.importFile()}>
        Import a {EXT[kind]} file
      </button>
    </p>
  );
}

const VERDICT: Record<string, string> = { ok: "OK to share", ask: "Ask the author", no: "Not allowed to share", stock: "The game's own", unknown: "Not checked yet" };
/** Where a picked file came from and what it allows, when known. */
export function CreditLine({ credit }: { credit?: AssetCredit }) {
  if (!credit) return null;
  const who = credit.authors?.map((a) => a.name).filter(Boolean).join(", ");
  const parts: ReactNode[] = [];
  if (credit.verdict) parts.push(<strong key="v">{VERDICT[credit.verdict] ?? credit.verdict}</strong>);
  if (who) parts.push(<span key="w"> · by {who}</span>);
  if (credit.page?.url)
    parts.push(
      <span key="p">
        {" "}
        ·{" "}
        <a href={credit.page.url} target="_blank" rel="noreferrer">
          {credit.page.name || "its page"} ↗
        </a>
      </span>,
    );
  if (credit.note) parts.push(<span key="n"> · {credit.note}</span>);
  return <p className="cs-note ai-credit">{parts}</p>;
}

/** The drop highlight and the note, around a picker's popover contents. */
export function ImportZone({ imp, children }: { imp: ReturnType<typeof useAssetImport>; children: ReactNode }) {
  return (
    <div className={`ai-zone${imp.hot ? " hot" : ""}`} {...imp.dropProps}>
      {children}
      <p className="muted ai-note">{IMPORT_NOTE}</p>
    </div>
  );
}
