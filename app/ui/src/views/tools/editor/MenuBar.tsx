// P5: the editor's menu bar (File, Edit, View, Select, Tools, Map, Help), the layout picker,
// and the pieces that hang off it: keys (Shortcuts.ts), the command palette, the cheat sheet,
// the key editor and the P5 dialogs.

import { useEffect, useRef, useState } from "react";
import { useConfirm } from "../../../ui";
import { CommandPalette, CheatSheet, KeyEditor } from "./CommandPalette";
import { commands, fileHooks, type Command, type Menu } from "./commands";
import { LAYOUTS, setLayout, usePanes } from "./panes";
import { P5Dialogs } from "./P5Dialogs";
import { useP5 } from "./p5store";
import { keysOf, showKey, useKeySet, useShortcuts } from "./Shortcuts";
import { selectionBounds, useEditor } from "./store";
import "./p5.css";

const MENUS: Menu[] = ["File", "Edit", "View", "Select", "Tools", "Map", "Help"];

export function MenuBar() {
  const [open, setOpen] = useState<Menu | null>(null);
  const bar = useRef<HTMLDivElement>(null);
  const ask = useConfirm();
  const layout = usePanes((s) => s.layout);
  useShortcuts();
  // starting over asks first when there are unsaved changes (like the start screen's buttons)
  useEffect(() => {
    fileHooks.replace = async (what) => {
      const d = useEditor.getState().doc;
      if (d?.dirty && !(await ask(`“${d.name}” has unsaved changes. Leave them and ${what === "new" ? "start a new map" : "open another"}?`, { ok: "Leave them", danger: true }))) return;
      await useEditor.getState().file(what);
    };
  }, [ask]);
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
            {open === m && <MenuList menu={m} onDone={() => setOpen(null)} />}
          </div>
        ))}
        <button type="button" className="ed-menu-find" title="Find any command (Ctrl+K)" onClick={() => useP5.getState().set({ dialog: "palette", paletteText: "" })}>
          <span aria-hidden="true">⌕</span> Find a command <kbd>{showKey(keysOf("help.palette")[0] ?? "Mod+K")}</kbd>
        </button>
        <div className="seg ed-layouts" role="group" aria-label="Views layout">
          {LAYOUTS.map((l) => (
            <button key={l.id} type="button" aria-pressed={layout === l.id} title={`${l.tip}${keysOf(`view.layout.${l.id}`)[0] ? ` (${showKey(keysOf(`view.layout.${l.id}`)[0])})` : ""}`} onClick={() => setLayout(l.id)}>
              <LayoutIcon id={l.id} />
              <span className="ed-layout-label">{l.label}</span>
            </button>
          ))}
        </div>
      </div>
      <Overlays />
    </>
  );
}

function MenuList({ menu, onDone }: { menu: Menu; onDone: () => void }) {
  useKeySet((s) => s.mine); // keys shown follow your changes
  const list = commands().filter((c) => c.menu === menu && !c.hidden);
  const extra: Command[] =
    menu === "Edit"
      ? [{ id: "p5.arrange", label: "Line up and spread out…", menu: "Edit", run: () => useP5.getState().set({ dialog: "transform" }), when: () => useEditor.getState().sel.brushes.length + useEditor.getState().sel.entities.length > 1 }]
      : [];
  const items = menu === "Edit" ? [...list.slice(0, list.findIndex((c) => c.id === "edit.mirrorX")), ...extra, ...list.slice(list.findIndex((c) => c.id === "edit.mirrorX"))] : list;
  return (
    <ul className="ed-menu-list" role="menu" aria-label={menu}>
      {items.map((c) => {
        const ok = !c.when || c.when();
        const k = keysOf(c.id)[0];
        return (
          <li key={c.id} role="none" className={c.sep ? "sep" : undefined}>
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
  if (!b) return <span className="ed-selsize"> · {n} picked</span>;
  const size = b[1].map((x, i) => Math.round((x - b[0][i]) * 100) / 100);
  return (
    <span className="ed-selsize mono" title="Width (x) × depth (y) × height (z) of what's picked">
      {" · "}
      {n} picked · {size.join(" × ")}
    </span>
  );
}
