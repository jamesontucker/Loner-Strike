// P5: the editor's one bar: the menus (File, Edit, View, Select, Map, Tools, Help) and Find a
// command on the left, the map's name, then the layout and view pickers and the real buttons
// (Save, Playtest, Publish) on the right; everything the old button row had is in a menu with
// its keys (Recent files in File). Also the pieces that hang off it: keys (Shortcuts.ts), the
// command palette, the cheat sheet, the key editor, the P5 dialogs and the first-map tour.

import { useEffect, useRef, useState } from "react";
import { useConfirm } from "../../../ui";
import { CommandPalette, CheatSheet, KeyEditor } from "./CommandPalette";
import { askHooks, commands, fileHooks, importOther, playtestNow, publishMap, type Command, type Menu } from "./commands";
import * as api from "./editorApi";
import type { RecentFile } from "./editorApi";
import { BusyLabel } from "./Panels";
import { FirstMapTour } from "./Tour";
import { ContentDrawer } from "./ContentBrowser";
import { LAYOUTS, setLayout, usePanes } from "./panes";
import { P5Dialogs } from "./P5Dialogs";
import { useP5 } from "./p5store";
import { keysOf, showKey, useKeySet, useShortcuts } from "./Shortcuts";
import { selectionBounds, useEditor } from "./store";
import "./p5.css";
import { NAV_PRESETS, setNavPreset, useNav } from "./navigation";

const MENUS: Menu[] = ["File", "Edit", "View", "Select", "Map", "Tools", "Help"];

const VIEWS: { id: "3d" | "top" | "front" | "side"; label: string }[] = [
  { id: "3d", label: "3D" },
  { id: "top", label: "Top" },
  { id: "front", label: "Front" },
  { id: "side", label: "Side" },
];

export function MenuBar() {
  const [open, setOpen] = useState<Menu | null>(null);
  const bar = useRef<HTMLDivElement>(null);
  const ask = useConfirm();
  const layout = usePanes((s) => s.layout);
  const doc = useEditor((s) => s.doc);
  const view = useEditor((s) => s.view);
  const fileBusy = useEditor((s) => s.fileBusy);
  const [recent, setRecent] = useState<RecentFile[]>([]);
  useEffect(() => {
    if (open === "File") void api.getRecent().then((r) => !r.error && setRecent(r.files ?? []));
  }, [open]);
  useShortcuts();
  // starting over asks first when there are unsaved changes (like the start screen's buttons)
  useEffect(() => {
    askHooks.ask = (text, ok) => ask(text, { ok });
    fileHooks.replace = async (what) => {
      const d = useEditor.getState().doc;
      if (d?.dirty && !(await ask(`“${d.name}” has unsaved changes. Leave them and ${what === "new" ? "start a new map" : "open another"}?`, { ok: "Leave them", danger: true }))) return;
      if (what === "other") await importOther();
      else await useEditor.getState().file(what);
    };
  }, [ask]);
  const openRecent = async (f: RecentFile) => {
    const d = useEditor.getState().doc;
    if (d?.dirty && !(await ask(`“${d.name}” has unsaved changes. Leave them and open ${f.name}?`, { ok: "Leave them", danger: true }))) return;
    await useEditor.getState().file(f.index);
  };
  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => !bar.current?.contains(e.target as Node) && setOpen(null);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(null);
    window.addEventListener("pointerdown", away, true);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("pointerdown", away, true);
      window.removeEventListener("keydown", esc);
    };
  }, [open]);

  return (
    <>
      <div className="ed-menubar" ref={bar} role="menubar" aria-label="Editor menu">
        {MENUS.map((m) => (
          <div key={m} className="ed-menu">
            <button
              type="button"
              role="menuitem"
              aria-haspopup="menu"
              aria-expanded={open === m}
              className="ed-menu-title"
              onClick={() => setOpen(open === m ? null : m)}
              onPointerEnter={() => open && open !== m && setOpen(m)}
            >
              {m}
            </button>
            {open === m && <MenuList menu={m} onDone={() => setOpen(null)} recent={m === "File" ? recent : []} openRecent={openRecent} />}
          </div>
        ))}
        {doc && (
          <span className="ed-bar-name" title={doc.saved ? "Saved as a project" : "Not saved yet"}>
            <span className="map-title">{doc.name}</span>
            {doc.dirty && <span className="editor-dirty" aria-label="Unsaved changes" title="Unsaved changes" />}
            <span className="tag tools-beta">Beta</span>
          </span>
        )}
        <button type="button" className="ed-menu-find" title="Find any command by name" onClick={() => useP5.getState().set({ dialog: "palette", paletteText: "" })}>
          <span aria-hidden="true">⌕</span> <span className="ed-menu-find-label">Find a command</span><span className="ed-menu-find-short">Find</span> <kbd>{showKey(keysOf("help.palette")[0] ?? "Mod+K")}</kbd>
        </button>
        <BusyLabel busy={!!fileBusy} label={fileBusy ?? ""} />
        <div className="seg ed-layouts" role="group" aria-label="Views layout">
          {LAYOUTS.map((l) => (
            <button key={l.id} type="button" aria-pressed={layout === l.id} title={`${l.tip}${keysOf(`view.layout.${l.id}`)[0] ? ` (${showKey(keysOf(`view.layout.${l.id}`)[0])})` : ""}`} onClick={() => setLayout(l.id)}>
              <LayoutIcon id={l.id} />
              <span className="ed-layout-label">{l.label}</span>
            </button>
          ))}
        </div>
        <div className="seg ed-views" role="group" aria-label="View">
          {VIEWS.map((v) => (
            <button key={v.id} type="button" aria-pressed={view === v.id} data-cmd={`view.${v.id}`} data-tip={`${v.label} view`} title={`${v.label} view`} onClick={() => useEditor.getState().set({ view: v.id })}>
              {v.label}
            </button>
          ))}
        </div>
        <span className="ed-bar-actions">
          <button type="button" className="btn small" disabled={!!fileBusy} data-cmd="file.save" title="Save the project" onClick={() => void useEditor.getState().file("save")}>
            Save
          </button>
          <button type="button" className="btn small primary" data-cmd="map.playtest" title="Compile it, put it in the game and play it with bots, as the Compile panel is set" onClick={() => playtestNow()}>
            Playtest
          </button>
          <button type="button" className="btn small" data-cmd="map.publish" title="Make a package to share this map in Tools > Community (compile it into the game first)" onClick={() => void publishMap()}>
            Publish…
          </button>
        </span>
      </div>
      <NavChoice />
      <FirstMapTour />
      <ContentDrawer />
      <Overlays />
    </>
  );
}

function MenuList({ menu, onDone, recent, openRecent }: { menu: Menu; onDone: () => void; recent: RecentFile[]; openRecent: (f: RecentFile) => void }) {
  useKeySet((s) => s.mine); // keys shown follow your changes
  const list = commands().filter((c) => c.menu === menu && !c.hidden);
  const extra: Command[] =
    menu === "Edit"
      ? [{ id: "p5.arrange", label: "Line up and spread out…", menu: "Edit", run: () => useP5.getState().set({ dialog: "transform" }), when: () => useEditor.getState().sel.brushes.length + useEditor.getState().sel.entities.length > 1 }]
      : [];
  const items = menu === "Edit" ? [...list.slice(0, list.findIndex((c) => c.id === "edit.mirrorX")), ...extra, ...list.slice(list.findIndex((c) => c.id === "edit.mirrorX"))] : list;
  // File: the recent files (they were a list in the old button row), after Open
  if (menu === "File" && recent.length) {
    const at = items.findIndex((c) => c.id === "file.open") + 1;
    const rec: Command[] = recent.slice(0, 10).map((f) => ({
      id: `file.recent.${f.index}`,
      label: `${f.name}${f.found ? "" : " (missing)"}`,
      menu: "File",
      hint: f.folder ? `In ${f.folder}` : undefined,
      when: () => f.found,
      run: () => openRecent(f),
    }));
    items.splice(at, 0, { id: "file.recent.head", label: "Recent files", menu: "File", sep: true, run: () => {}, when: () => false }, ...rec);
  }
  return (
    <ul className="ed-menu-list" role="menu" aria-label={menu}>
      {items.map((c) => {
        const ok = !c.when || c.when();
        const k = keysOf(c.id)[0];
        return (
          <li key={c.id} role="none" className={[c.sep && "sep", c.id === "file.recent.head" ? "ed-menu-head" : c.id.startsWith("file.recent.") && "ed-menu-recent"].filter(Boolean).join(" ") || undefined}>
            <button
              type="button"
              role="menuitem"
              disabled={!ok}
              title={c.hint}
              onClick={() => {
                onDone();
                void c.run();
              }}
            >
              <span>{c.label}</span>
              {k && <kbd>{showKey(k)}</kbd>}
            </button>
          </li>
        );
      })}
    </ul>
  );
}

function LayoutIcon({ id }: { id: string }) {
  const r = (x: number, y: number, w: number, h: number, k: number) => <rect key={k} x={x} y={y} width={w} height={h} rx="0.8" />;
  const shapes =
    id === "single" ? [r(1, 1, 14, 12, 0)] : id === "two" ? [r(1, 1, 6.5, 12, 0), r(8.5, 1, 6.5, 12, 1)] : id === "quad" ? [r(1, 1, 6.5, 5.5, 0), r(8.5, 1, 6.5, 5.5, 1), r(1, 7.5, 6.5, 5.5, 2), r(8.5, 7.5, 6.5, 5.5, 3)] : [r(1, 1, 9, 12, 0), r(11, 1, 4, 3.4, 1), r(11, 5.3, 4, 3.4, 2), r(11, 9.6, 4, 3.4, 3)];
  return (
    <svg viewBox="0 0 16 14" width="16" height="14" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.2">
      {shapes}
    </svg>
  );
}

function Overlays() {
  const dialog = useP5((s) => s.dialog);
  const close = () => useP5.getState().set({ dialog: null });
  return (
    <>
      {dialog === "palette" && <CommandPalette onClose={close} />}
      {dialog === "cheats" && <CheatSheet onClose={close} />}
      {dialog === "keys" && <KeyEditor onClose={close} />}
      <P5Dialogs />
    </>
  );
}

/** Size of what's picked, for the status bar. */
export function SelectionSize() {
  const sel = useEditor((s) => s.sel);
  const brushes = useEditor((s) => s.brushes);
  const doc = useEditor((s) => s.doc);
  void brushes;
  void doc;
  const n = sel.brushes.length + sel.entities.length;
  if (!n) return null;
  const b = selectionBounds();
  const size = b ? b[1].map((x, i) => Math.round((x - b[0][i]) * 100) / 100) : null;
  if (!size || size.every((x) => x === 0)) return <span className="ed-selsize"> · {n} picked</span>;
  return (
    <span className="ed-selsize mono" title="Width (x) × depth (y) × height (z) of what's picked">
      {" · "}
      {n} picked · {size.join(" × ")}
    </span>
  );
}

/** First open: how should the 3D view move? (View > Move like… changes it later.) */
function NavChoice() {
  const chosen = useNav((s) => s.chosen);
  const preset = useNav((s) => s.preset);
  if (chosen) return null;
  return (
    <div className="banner ed-nav-choice" role="status">
      <span>How should the 3D view move?</span>
      <div className="seg" role="group" aria-label="Camera navigation">
        {NAV_PRESETS.map((n) => (
          <button key={n.id} type="button" aria-pressed={preset === n.id} title={n.tip} onClick={() => setNavPreset(n.id)}>
            {n.label}
            {n.id === "unreal" ? " (hold right mouse + WASD)" : n.id === "blender" ? " (drag to orbit)" : " (Z to look)"}
          </button>
        ))}
      </div>
      <span className="muted small-note">Change it any time in View. The keys sheet (?) lists the moves.</span>
    </div>
  );
}
