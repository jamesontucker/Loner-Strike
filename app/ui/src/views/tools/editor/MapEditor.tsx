// Map editor (Beta): a modern take on Hammer over agent E's brush core. One 3D view first
// (top/front/side are 1-4 away), a toolbar, push/pull faces, W/E/R gizmos with snapping
// (Alt ignores it), live checks, undo for every gesture, save/open and .map import/export.
// The document lives in the app (tools/editor.rs); this page draws it and sends commands.

import { useEffect, useMemo, useRef, useState } from "react";
import { useConfirm, useToast } from "../../../ui";
import { useTools } from "../../../stores/tools";
import type { ToolProps } from "../registry";
import { canDraw3d, SceneBoundary } from "../viewer/Scene";
import { EditorScene } from "./EditorScene";
import { BusyLabel, deleteSelection, duplicate, hideSelection, isolateSelection, showAll, Toolbar, ToolOptions, TOOLS } from "./Panels";
import { selectionIds } from "./EditorScene";
import { Dialogs } from "./Dialogs";
// P4: prefab library, outside changes (MCP agents), Publish
import { mapNameOf } from "./ComponentPanel";
import { useOutsideChanges } from "./extras";
import { useToolCrumbs } from "../common/shell";
import { useReportAddress } from "../common/address";
import { useDirty } from "../../../store";
import { useNav } from "../../../store";
import { useToolArg } from "../../../stores/tools";
// P5: menu bar + keys + palette, outliner, project files, undo history, selection size
import { MenuBar, SelectionSize } from "./MenuBar";
import { useP5 } from "./p5store";
import { DockArea, DockDropZones, DockFloats, useDockColumns } from "./Dock";
import { showPanel } from "./dockState";
import { useNav as useNavPreset } from "./navigation";
import { importOther } from "./commands";
import * as api from "./editorApi";
import type { RecentFile, Recovery } from "./editorApi";
import { EMPTY_SEL, facesOf, useEditor, type ViewId } from "./store";

/** How often unsaved changes are written to the autosave file. */
const AUTOSAVE_MS = 3 * 60 * 1000;
import "./editor.css";

type Tab = "inspect" | "outline" | "textures" | "library" | "checks" | "compile" | "project" | "history"; // P5: project, history

const VIEWS: { id: ViewId; label: string; key: string }[] = [
  { id: "3d", label: "3D", key: "Shift+1" },
  { id: "top", label: "Top", key: "Shift+2" },
  { id: "front", label: "Front", key: "Shift+3" },
  { id: "side", label: "Side", key: "Shift+4" },
];

const isField = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

export default function MapEditor(_: ToolProps) {
  const doc = useEditor((s) => s.doc);
  // P5: the bare "editor" address is the start screen (the "Map editor" crumb, Back), even with a
  // map open in the app; "editor:<map>" is the open map. A new page per address (the shell
  // starts the page over), so this is decided once, when the page opens.
  const toolArg = useToolArg();
  const [atStart, setAtStart] = useState(() => !toolArg);
  const startedWith = useRef<number | null | undefined>(undefined);
  if (atStart && doc !== undefined && startedWith.current === undefined) startedWith.current = doc?.docId ?? null;
  useEffect(() => {
    // something on the start screen opened a map (new, open, import, recent, recover)
    if (atStart && startedWith.current !== undefined && doc && doc.docId !== startedWith.current) setAtStart(false);
  }, [atStart, doc]);
  const showing = !!doc && !atStart;
  useReportAddress("editor", showing ? `editor:${doc.name.replace(/[^A-Za-z0-9_.-]+/g, "_") || "map"}` : "editor");
  // P4 (P6's request): the open map's name in the breadcrumb, and a check before leaving with unsaved changes
  useToolCrumbs("editor", showing ? [{ label: doc.name }] : null);
  useDirty("editor", showing && !!doc?.dirty);
  const loadError = useEditor((s) => s.loadError);
  const fileBusy = useEditor((s) => s.fileBusy);
  const toast = useToast();
  const ask = useConfirm();

  useEffect(() => {
    useEditor.getState().setNotify((t, e) => toast(t, e));
  }, [toast]);
  useEffect(() => {
    // the flat map view of the viewer isn't one of ours: the editor has its own
    if (useTools.getState().camera === "top") useTools.getState().setCamera("orbit");
    void useEditor.getState().load();
  }, []);

  /** Starting over (new, open, import) asks first when there are unsaved changes. */
  const replace = async (what: "new" | "open" | "import" | "recover" | number) => {
    const d = useEditor.getState().doc;
    if (d?.dirty && !(await ask(`“${d.name}” has unsaved changes. Leave them and ${what === "new" ? "start a new map" : "open another"}?`, { ok: "Leave them", danger: true }))) return;
    await useEditor.getState().file(what);
  };

  if (doc === undefined) {
    return (
      <div className="tools-page">
        <p className="muted">
          <span className="spinner" aria-hidden="true" />
          Opening the editor…
        </p>
      </div>
    );
  }
  if (doc === null || atStart) return <Start onPick={replace} busy={fileBusy} error={loadError} open={doc ? { name: doc.name, dirty: doc.dirty, go: () => setAtStart(false) } : null} />;
  return <Editor replace={replace} />;
}

type Pick = "new" | "open" | "import" | "recover" | number;

function Start({ onPick, busy, error, open }: { onPick: (w: Pick) => void; busy: string | null; error: string | null; open?: { name: string; dirty: boolean; go: () => void } | null }) {
  const [recent, setRecent] = useState<RecentFile[]>([]);
  const [rec, setRec] = useState<Recovery | null>(null);
  useEffect(() => {
    void api.getRecent().then((r) => !r.error && setRecent(r.files ?? []));
    void api.getRecovery().then((r) => !r.error && setRec(r));
  }, []);
  const when = rec?.time ? new Date(rec.time * 1000).toLocaleString() : "";
  return (
    <div className="tools-page">
      <section className="panel editor-start">
        <h3>
          Map editor <span className="tag tools-beta">Beta</span>
        </h3>
        <p className="muted measure">
          Build Counter-Strike maps with brushes, like Hammer, in one 3D view. Draw boxes, push and pull their faces, place spawns and lights, paint textures. Your map is checked as you go. Save it as a project, or export a .map for any GoldSrc compiler.
        </p>
        {error && <p className="banner">{error}</p>}
        {rec?.found && (
          <div className="banner editor-recover" role="status">
            <span>
              Unsaved work from “{rec.name || "Untitled"}” was kept{when ? ` (${when})` : ""}: {rec.brushes ?? 0} brushes, {rec.entities ?? 0} entities. The editor may have closed before it was saved.
            </span>
            <span className="editor-recover-actions">
              <button type="button" className="btn small primary" disabled={!!busy} onClick={() => onPick("recover")}>
                Recover it
              </button>
              <button type="button" className="btn small" disabled={!!busy} onClick={() => void api.discardRecovery().then(() => setRec(null))}>
                Throw it away
              </button>
            </span>
          </div>
        )}
        {/* P5: the map still open in the app (from the "Map editor" crumb or Back) */}
        {open && (
          <div className="banner editor-recover" role="status">
            <span>
              “{open.name}” is still open{open.dirty ? " (with unsaved changes)" : ""}.
            </span>
            <span className="editor-recover-actions">
              <button type="button" className="btn small primary" disabled={!!busy} onClick={open.go}>
                Back to {open.name}
              </button>
            </span>
          </div>
        )}
        <div className="editor-start-actions">
          <button type="button" className="btn primary" disabled={!!busy} onClick={() => onPick("new")}>
            New map
          </button>
          <button type="button" className="btn" disabled={!!busy} onClick={() => onPick("open")}>
            Open a project
          </button>
          <button type="button" className="btn" disabled={!!busy} onClick={() => onPick("import")}>
            Import a .map
          </button>
          {/* P5: Hammer .rmf / J.A.C.K. .jmf */}
          <button type="button" className="btn" disabled={!!busy} title="Brushes, entities, groups and visgroups come over" onClick={() => void importOther()}>
            Import from Hammer / J.A.C.K.
          </button>
          <BusyLabel busy={!!busy} label={busy ?? ""} />
        </div>
        <p className="small-note muted">A new map starts as an open-sky room with two spawns for each team.</p>
        {recent.length > 0 && (
          <div className="editor-recent-files">
            <h4>Recent</h4>
            <ul>
              {recent.map((f) => (
                <li key={f.index}>
                  <button type="button" className="linklike" disabled={!!busy || !f.found} title={f.found ? `Open ${f.name}` : "Not there any more"} onClick={() => onPick(f.index)}>
                    {f.name}
                  </button>{" "}
                  <span className="muted small-note">
                    {f.folder}
                    {f.project ? "" : " · .map"}
                    {f.found ? "" : " · missing"}
                  </span>
                </li>
              ))}
            </ul>
          </div>
        )}
      </section>
    </div>
  );
}

function Editor({ replace }: { replace: (w: Pick) => Promise<void> }) {
  const doc = useEditor((s) => s.doc)!;
  const view = useEditor((s) => s.view);
  const fileBusy = useEditor((s) => s.fileBusy);
  const working = useEditor((s) => s.working);
  // P5: panels live in the dock (dockPanels.tsx); a tab request shows that panel wherever it is
  const setTab = (t: Tab) => {
    useP5.getState().set({ tab: t });
    showPanel(t);
  };
  const dockCols = useDockColumns(); // P5
  const [lost, setLost] = useState(0);
  const has3d = useMemo(() => canDraw3d(), []);
  // the camera frames the map once per document, not after every edit
  const bounds = useMemo(() => doc.bounds, [doc.docId]); // eslint-disable-line react-hooks/exhaustive-deps

  useKeys(replace);
  useAutosave();
  useOutsideChanges(); // P4
  // P4: another page can open a tab ("editor:prefabs")
  const arg = useToolArg();
  useEffect(() => {
    if (arg === "prefabs" || arg === "components") setTab("library");
    else if (arg === "compile" || arg === "checks") setTab(arg);
  }, [arg]);
  const [recent, setRecent] = useState<RecentFile[]>([]);
  useEffect(() => {
    void api.getRecent().then((r) => !r.error && setRecent(r.files ?? []));
  }, [doc.docId, doc.saved]);

  const s = useEditor.getState();

  return (
    <div className="tools-page editor-page">
      <MenuBar /> {/* P5 */}
      <div className="tools-bar editor-bar">
        <span className="map-title" title={doc.saved ? "Saved as a project" : "Not saved yet"}>
          {doc.name}
          {doc.dirty && <span className="editor-dirty" aria-label="Unsaved changes" title="Unsaved changes" />}
        </span>
        <span className="tag tools-beta">Beta</span>
        <div className="editor-files">
          <button type="button" className="btn small" disabled={!!fileBusy} onClick={() => void replace("new")}>New</button>
          <button type="button" className="btn small" disabled={!!fileBusy} onClick={() => void replace("open")}>Open</button>
          <button type="button" className="btn small" disabled={!!fileBusy} onClick={() => void replace("import")}>Import .map</button>
          <button type="button" className="btn small" disabled={!!fileBusy} title="Save the project (Ctrl+S)" onClick={() => void s.file("save")}>Save</button>
          <button type="button" className="btn small" disabled={!!fileBusy} onClick={() => void s.file("saveAs")}>Save as</button>
          <button type="button" className="btn small" disabled={!!fileBusy} title="Write a .map file for a map compiler" onClick={() => void s.file("export")}>Export .map</button>
          {recent.length > 0 && (
            <select
              className="editor-recent-select"
              value=""
              disabled={!!fileBusy}
              aria-label="Open a recent file"
              onChange={(e) => e.target.value !== "" && void replace(Number(e.target.value))}
            >
              <option value="">Recent…</option>
              {recent.map((f) => (
                <option key={f.index} value={f.index} disabled={!f.found}>
                  {f.name}
                  {f.found ? "" : " (missing)"}
                </option>
              ))}
            </select>
          )}
          <button type="button" className="btn small" title="Sky, WADs, sun and rules" onClick={() => s.set({ dialog: "props" })}>Map properties</button>
          {/* P4: Tools > Community's Publish, for the map compiled from this one */}
          <button type="button" className="btn small" title="Make a package to share this map in Tools > Community (compile it into the game first)" onClick={() => useNav.getState().go("tools", `community:publish:${mapNameOf(doc.name)}`)}>Publish…</button>
          <BusyLabel busy={!!fileBusy} label={fileBusy ?? ""} />
        </div>
        <span className="push" />
        <button type="button" className="btn small" disabled={!doc.undo} title={doc.undo ? `Undo ${doc.undo} (Ctrl+Z)` : "Nothing to undo"} onClick={() => void s.undo()}>
          Undo
        </button>
        <button type="button" className="btn small" disabled={!doc.redo} title={doc.redo ? `Redo ${doc.redo} (Ctrl+Shift+Z)` : "Nothing to redo"} onClick={() => void s.redo()}>
          Redo
        </button>
        <div className="seg" role="group" aria-label="View">
          {VIEWS.map((v) => (
            <button key={v.id} type="button" aria-pressed={view === v.id} title={`${v.label} view (${v.key})`} onClick={() => s.set({ view: v.id })}>
              {v.label}
            </button>
          ))}
        </div>
      </div>
      <div className="editor-main" style={dockCols}>
        <Toolbar />
        <DockArea side="left" />
        <div className="tools-stage">
          <ToolOptions />
          <div className="tools-canvas">
            {!has3d ? (
              <div className="tools-overlay">Your system can't show 3D, so the editor can't open here.</div>
            ) : (
              <SceneBoundary
                key={lost}
                fallback={
                  <div className="tools-overlay">
                    The 3D view stopped.
                    <button type="button" className="btn" onClick={() => setLost((n) => n + 1)}>
                      Try again
                    </button>
                  </div>
                }
              >
                <EditorScene key={`${doc.docId}-${lost}`} /* P5: views switch inside their panes now */ bounds={bounds} docId={doc.docId} onLost={() => setLost((n) => n + 1)} />
              </SceneBoundary>
            )}
          </div>
          <DockArea side="bottom" />
          <Status working={working > 0} />
        </div>
        <DockArea side="right" /> {/* P5: dockable panels (dockPanels.tsx lists them) */}
      </div>
      <Dialogs />
      <DockFloats />
      <DockDropZones />
    </div>
  );
}

function Status({ working }: { working: boolean }) {
  const tool = useEditor((s) => s.tool);
  const cursor = useEditor((s) => s.cursor);
  const hover = useTools((s) => s.hover);
  const camera = useTools((s) => s.camera);
  const view = useEditor((s) => s.view);
  const stats = useEditor((s) => s.doc?.stats);
  const t = TOOLS.find((x) => x.id === tool);
  // show "Saving…" only for commands that take a moment
  const [slow, setSlow] = useState(false);
  useEffect(() => {
    if (!working) return setSlow(false);
    const id = setTimeout(() => setSlow(true), 200);
    return () => clearTimeout(id);
  }, [working]);
  const autosaved = useEditor((s) => s.autosavedAt);
  const navPreset = useNavPreset((n) => n.preset); // P5: the hint follows the navigation preset
  const camHint =
    view !== "3d"
      ? "Drag on empty space to box-pick. Right-drag to pan, scroll to zoom."
      : navPreset === "unreal"
        ? "Hold right mouse to look, WASD/QE to fly, scroll for speed. Alt+drag orbits, middle-drag pans. F: focus."
        : navPreset === "hammer"
          ? "Z: mouse-look (WASD). Drag to look, right-drag to move, middle-drag to pan."
          : camera === "fly"
            ? "Fly: WASD to move, Esc lets go of the mouse. F: orbit."
            : "Drag to turn, right-drag to pan, scroll to zoom. F: fly.";
  return (
    <div className="tools-status">
      <span className="mono">
        {cursor ? cursor.join(" ") : "–"}
        <SelectionSize /> {/* P5 */}
      </span>
      <span className="tools-hint">
        {hover?.label ? hover.label : `${t?.label}: ${t?.tip}`} {camHint}
      </span>
      <span>
        {slow ? (
          <BusyLabel busy label="Saving…" />
        ) : (
          stats && `${stats.brushes} brushes · ${stats.entities} entities${autosaved ? ` · autosaved ${autosaved}` : ""}`
        )}
      </span>
    </div>
  );
}

/** Every few minutes, unsaved changes go to the autosave file (recovered after a crash). */
function useAutosave() {
  useEffect(() => {
    const id = setInterval(async () => {
      const s = useEditor.getState();
      if (!s.doc?.dirty || s.fileBusy) return;
      const r = await api.autosave();
      if (r.saved) useEditor.setState({ autosavedAt: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) });
    }, AUTOSAVE_MS);
    return () => clearInterval(id);
  }, []);
}

/** Copy the selection to the clipboard as .map text (also kept here, in case the system clipboard can't be read). */
export async function copySelection(cut = false) {
  const s = useEditor.getState();
  const ids = selectionIds();
  if (!ids.brushes.length && !ids.entities.length) return;
  const r = await api.copySelection(ids.brushes, ids.entities);
  if (r.error) return s.notify(r.error, true);
  s.set({ clip: r.text });
  try {
    await navigator.clipboard.writeText(r.text);
  } catch {
    /* the editor keeps its own copy */
  }
  if (cut) {
    const d = await s.cmd({ op: "delete", brushes: s.sel.brushes, entities: s.sel.entities });
    if (!d.error) s.select(EMPTY_SEL);
  } else s.notify(`Copied ${r.brushes} brush${r.brushes === 1 ? "" : "es"}${r.entities ? ` and ${r.entities} entit${r.entities === 1 ? "y" : "ies"}` : ""}. Paste into this map or another.`);
}

export async function paste() {
  const s = useEditor.getState();
  let text: string | null = null;
  try {
    text = await navigator.clipboard.readText();
  } catch {
    /* fall back to the editor's own copy */
  }
  if (!text || !text.includes("{")) text = s.clip;
  if (!text) return s.notify("Copy some brushes or entities first.", true);
  // pasted back into the map it came from: one grid step over, so it isn't hidden in the original
  const off = text === s.clip && s.sel.brushes.length + s.sel.entities.length > 0 ? [s.grid, s.grid, 0] : [0, 0, 0];
  const r = await s.cmd({ op: "paste", text, offset: off });
  if (r.created) {
    const owned = new Set(r.created.entities);
    s.select({ brushes: r.created.brushes.filter((b) => !owned.has(useEditor.getState().brushes.get(b)?.entity ?? -1)), entities: r.created.entities, face: null });
  }
}

/** Keyboard: tools, views, bookmarks, undo/redo, copy/paste, hide, delete, duplicate, nudge, save. */
function useKeys(replace: (w: Pick) => Promise<void>) {
  const replaceRef = useRef(replace);
  replaceRef.current = replace;
  useEffect(() => {
    // Ctrl+1-9 store a camera here (before the app's own Ctrl/⌘+number section keys)
    const capture = (e: KeyboardEvent) => {
      if (isField(e.target) || useEditor.getState().dialog) return;
      const n = /^Digit([1-9])$/.exec(e.code)?.[1];
      if (n && e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey) {
        e.preventDefault();
        e.stopImmediatePropagation();
        const s = useEditor.getState();
        s.set({ bookmark: { n: Number(n), store: true, seq: (s.bookmark?.seq ?? 0) + 1 } });
      }
    };
    const down = (e: KeyboardEvent) => {
      const s = useEditor.getState();
      if (e.key === "Alt") s.set({ alt: true });
      if (isField(e.target) || s.dialog) return;
      const mod = e.ctrlKey || e.metaKey;
      const k = e.key.toLowerCase();
      if (mod && k === "z") {
        e.preventDefault();
        void (e.shiftKey ? s.redo() : s.undo());
        return;
      }
      if (mod && k === "y") {
        e.preventDefault();
        void s.redo();
        return;
      }
      if (mod && k === "s") {
        e.preventDefault();
        void s.file(e.shiftKey ? "saveAs" : "save");
        return;
      }
      if (mod && k === "d") {
        e.preventDefault();
        void duplicate();
        return;
      }
      if (mod && (k === "c" || k === "x")) {
        if (window.getSelection()?.toString()) return;
        e.preventDefault();
        void copySelection(k === "x");
        return;
      }
      if (mod && k === "v") {
        e.preventDefault();
        if (e.shiftKey) s.set({ dialog: "paste" });
        else void paste();
        return;
      }
      if (mod && k === "h") {
        e.preventDefault();
        const ids = selectionIds().brushes;
        if (ids.length) void s.cmd({ op: "hollow", brushes: ids, thickness: s.thickness });
        return;
      }
      if (mod && k === "a") {
        e.preventDefault();
        s.select({ brushes: [...s.brushes.keys()].filter((id) => s.brushes.get(id)?.entity == null && !s.hidden.brushes.has(id)), entities: s.doc?.entities.filter((x) => !s.hidden.entities.has(x.id)).map((x) => x.id) ?? [], face: null });
        return;
      }
      if (mod || e.altKey) return;
      // while flying, WASD moves the camera
      const flying = s.view === "3d" && useTools.getState().camera === "fly";
      if (e.key === "Escape") {
        if (!flying) {
          if (s.tool === "vertex" && s.verts.length) s.set({ verts: [] });
          else s.select(EMPTY_SEL);
        }
        return;
      }
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        deleteSelection();
        return;
      }
      const digit = /^Digit([1-9])$/.exec(e.code)?.[1];
      if (digit && e.shiftKey) {
        const view = (["3d", "top", "front", "side"] as ViewId[])[Number(digit) - 1];
        if (view) s.set({ view });
        return;
      }
      if (digit) {
        // go to a stored camera (in the 3D view)
        s.set({ view: "3d", bookmark: { n: Number(digit), store: false, seq: (s.bookmark?.seq ?? 0) + 1 } });
        return;
      }
      if (k === "f" && s.view === "3d") return useTools.getState().setCamera(useTools.getState().camera === "fly" ? "orbit" : "fly");
      if (e.key === "[" || e.key === "]") {
        const grids = [1, 2, 4, 8, 16, 32, 64, 128, 256];
        const i = grids.indexOf(s.grid);
        return s.set({ grid: grids[Math.max(0, Math.min(grids.length - 1, (i < 0 ? 4 : i) + (e.key === "]" ? 1 : -1)))] });
      }
      if (flying) return;
      if (k === "h") return e.shiftKey ? isolateSelection() : hideSelection();
      if (k === "u") return showAll();
      const tool = TOOLS.find((t) => t.key.toLowerCase() === k);
      if (tool && !e.shiftKey) return s.set({ tool: tool.id });
      // arrows nudge one grid step (Page Up/Down: up and down): corners, textures, or what's picked
      const nudge: Record<string, [number, number, number]> = { ArrowLeft: [-1, 0, 0], ArrowRight: [1, 0, 0], ArrowUp: [0, 1, 0], ArrowDown: [0, -1, 0], PageUp: [0, 0, 1], PageDown: [0, 0, -1] };
      const n = nudge[e.key];
      if (!n) return;
      if (s.tool === "vertex" && s.verts.length) {
        e.preventDefault();
        const d = n.map((x) => x * s.grid);
        void s.cmd({ op: "move_vertices", picks: s.verts.map((v) => [v.brush, v.p]), delta: d }).then((r) => {
          if (!r.error) useEditor.setState({ verts: s.verts.map((v) => ({ brush: v.brush, p: v.p.map((x, i) => x + d[i]) as typeof v.p })) });
        });
        return;
      }
      const faces = facesOf(s.sel);
      if (s.tool === "texture" && faces.length && !n[2]) {
        e.preventDefault();
        void s.cmd({ op: "align", faces: faces.map((f) => [f.brush, f.face]), action: "shift", u: -n[0] * s.grid, v: n[1] * s.grid });
        return;
      }
      const ids = selectionIds();
      if (!ids.brushes.length && !ids.entities.length) return;
      e.preventDefault();
      void s.cmd({ op: "transform", ...ids, translate: n.map((x) => x * s.grid), lock: s.textureLock });
    };
    const up = (e: KeyboardEvent) => {
      if (e.key === "Alt") useEditor.getState().set({ alt: false });
    };
    const blur = () => useEditor.getState().set({ alt: false });
    window.addEventListener("keydown", capture, true);
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    window.addEventListener("blur", blur);
    return () => {
      window.removeEventListener("keydown", capture, true);
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
      window.removeEventListener("blur", blur);
    };
  }, []);
}

