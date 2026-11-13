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
import { BusyLabel, ChecksPanel, deleteSelection, duplicate, Inspector, Outline, TextureBrowser, Toolbar, ToolOptions, TOOLS } from "./Panels";
import { selectionIds } from "./EditorScene";
import { EMPTY_SEL, useEditor, type ViewId } from "./store";
import "./editor.css";

type Tab = "inspect" | "outline" | "textures" | "checks";

const VIEWS: { id: ViewId; label: string; key: string }[] = [
  { id: "3d", label: "3D", key: "1" },
  { id: "top", label: "Top", key: "2" },
  { id: "front", label: "Front", key: "3" },
  { id: "side", label: "Side", key: "4" },
];

const isField = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

export default function MapEditor(_: ToolProps) {
  const doc = useEditor((s) => s.doc);
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
  const replace = async (what: "new" | "open" | "import") => {
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
  if (doc === null) return <Start onPick={replace} busy={fileBusy} error={loadError} />;
  return <Editor replace={replace} />;
}

function Start({ onPick, busy, error }: { onPick: (w: "new" | "open" | "import") => void; busy: string | null; error: string | null }) {
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
          <BusyLabel busy={!!busy} label={busy ?? ""} />
        </div>
        <p className="small-note muted">A new map starts as an open-sky room with two spawns for each team.</p>
      </section>
    </div>
  );
}

function Editor({ replace }: { replace: (w: "new" | "open" | "import") => Promise<void> }) {
  const doc = useEditor((s) => s.doc)!;
  const view = useEditor((s) => s.view);
  const fileBusy = useEditor((s) => s.fileBusy);
  const working = useEditor((s) => s.working);
  const checks = useEditor((s) => s.checks);
  const [tab, setTab] = useState<Tab>("inspect");
  const [lost, setLost] = useState(0);
  const has3d = useMemo(() => canDraw3d(), []);
  // the camera frames the map once per document, not after every edit
  const bounds = useMemo(() => doc.bounds, [doc.docId]); // eslint-disable-line react-hooks/exhaustive-deps

  useKeys(replace);

  const problems = checks?.issues.filter((i) => i.level === "error").length ?? 0;
  const warnings = checks?.issues.filter((i) => i.level === "warning").length ?? 0;
  const s = useEditor.getState();

  return (
    <div className="tools-page editor-page">
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
      <div className="editor-main">
        <Toolbar />
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
                <EditorScene key={`${doc.docId}-${lost}-${view === "3d" ? "3d" : "flat"}`} bounds={bounds} docId={doc.docId} onLost={() => setLost((n) => n + 1)} />
              </SceneBoundary>
            )}
          </div>
          <Status working={working > 0} />
        </div>
        <aside className="tools-side editor-side">
          <div className="seg editor-tabs" role="tablist" aria-label="Panels">
            {(
              [
                ["inspect", "Inspect"],
                ["outline", "Outline"],
                ["textures", "Textures"],
                ["checks", problems ? `Checks (${problems})` : warnings ? `Checks (${warnings})` : "Checks"],
              ] as [Tab, string][]
            ).map(([id, label]) => (
              <button key={id} type="button" role="tab" aria-pressed={tab === id} aria-selected={tab === id} className={id === "checks" && problems ? "editor-tab-bad" : undefined} onClick={() => setTab(id)}>
                {label}
              </button>
            ))}
          </div>
          {tab === "inspect" && <Inspector />}
          {tab === "outline" && <Outline />}
          {tab === "textures" && <TextureBrowser />}
          {tab === "checks" && <ChecksPanel />}
        </aside>
      </div>
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
  const camHint = view !== "3d" ? "Drag with the right button to pan, scroll to zoom." : camera === "fly" ? "Fly: WASD to move, Esc lets go of the mouse. F: orbit." : "Drag to turn, right-drag to pan, scroll to zoom. F: fly.";
  return (
    <div className="tools-status">
      <span className="mono">{cursor ? cursor.join(" ") : "–"}</span>
      <span className="tools-hint">
        {hover?.label ? hover.label : `${t?.label}: ${t?.tip}`} {camHint}
      </span>
      <span>
        {slow ? (
          <BusyLabel busy label="Saving…" />
        ) : (
          stats && `${stats.brushes} brushes · ${stats.entities} entities`
        )}
      </span>
    </div>
  );
}

/** Keyboard: tools, views, undo/redo, delete, duplicate, nudge, save. */
function useKeys(replace: (w: "new" | "open" | "import") => Promise<void>) {
  const replaceRef = useRef(replace);
  replaceRef.current = replace;
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      const s = useEditor.getState();
      if (e.key === "Alt") s.set({ alt: true });
      if (isField(e.target)) return;
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
      if (mod && k === "a") {
        e.preventDefault();
        s.select({ brushes: [...s.brushes.keys()].filter((id) => s.brushes.get(id)?.entity == null), entities: s.doc?.entities.map((x) => x.id) ?? [], face: null });
        return;
      }
      if (mod || e.altKey) return;
      // while flying, WASD moves the camera
      const flying = s.view === "3d" && useTools.getState().camera === "fly";
      if (e.key === "Escape") {
        if (!flying) s.select(EMPTY_SEL);
        return;
      }
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        deleteSelection();
        return;
      }
      const view = { "1": "3d", "2": "top", "3": "front", "4": "side" }[e.key] as ViewId | undefined;
      if (view) return s.set({ view });
      if (k === "f" && s.view === "3d") return useTools.getState().setCamera(useTools.getState().camera === "fly" ? "orbit" : "fly");
      if (e.key === "[" || e.key === "]") {
        const grids = [1, 2, 4, 8, 16, 32, 64, 128, 256];
        const i = grids.indexOf(s.grid);
        return s.set({ grid: grids[Math.max(0, Math.min(grids.length - 1, (i < 0 ? 4 : i) + (e.key === "]" ? 1 : -1)))] });
      }
      if (flying) return;
      const tool = TOOLS.find((t) => t.key.toLowerCase() === k);
      if (tool) return s.set({ tool: tool.id });
      if (k === "h") {
        const ids = selectionIds().brushes;
        if (ids.length) void s.cmd({ op: "hollow", brushes: ids, thickness: s.thickness });
        return;
      }
      // arrows nudge what's picked one grid step (Page Up/Down: up and down)
      const nudge: Record<string, [number, number, number]> = { ArrowLeft: [-1, 0, 0], ArrowRight: [1, 0, 0], ArrowUp: [0, 1, 0], ArrowDown: [0, -1, 0], PageUp: [0, 0, 1], PageDown: [0, 0, -1] };
      const n = nudge[e.key];
      if (n) {
        const ids = selectionIds();
        if (!ids.brushes.length && !ids.entities.length) return;
        e.preventDefault();
        void s.cmd({ op: "transform", ...ids, translate: n.map((x) => x * s.grid), lock: s.textureLock });
      }
    };
    const up = (e: KeyboardEvent) => {
      if (e.key === "Alt") useEditor.getState().set({ alt: false });
    };
    const blur = () => useEditor.getState().set({ alt: false });
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    window.addEventListener("blur", blur);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
      window.removeEventListener("blur", blur);
    };
  }, []);
}
