// P5: the command palette (Ctrl+K / Ctrl+Shift+P: every command, with its key, plus the
// map's entities by name), the keys cheat sheet (?) and the key editor (Hammer-style and
// TrenchBroom-style key sets, or your own).

import { useEffect, useMemo, useRef, useState } from "react";
import { useTools } from "../../../stores/tools";
import { Sheet } from "../../../ui";
import { commands, frame, selectIds, type Command, type Menu } from "./commands";
import { setLayout } from "./panes";
import { useP5 } from "./p5store";
import { clashes, keyName, keysOf, norm, PRESETS, showKey, useKeySet, type PresetId } from "./Shortcuts";
import { keyOf, originOf, useEditor } from "./store";

const MENUS: Menu[] = ["File", "Edit", "View", "Select", "Tools", "Map", "Help"];

type Item = { kind: "cmd"; c: Command } | { kind: "ent"; id: number; label: string; note: string };

function words(q: string) {
  return q.toLowerCase().split(/\s+/).filter(Boolean);
}

export function CommandPalette({ onClose }: { onClose: () => void }) {
  const [q, setQ] = useState("");
  const [at, setAt] = useState(0);
  const list = useRef<HTMLUListElement>(null);
  const all = useMemo(() => commands(), []);
  const doc = useEditor((s) => s.doc);
  const items: Item[] = useMemo(() => {
    const w = words(q);
    const cmds = all.filter((c) => {
      const text = `${c.label} ${c.hint ?? ""} ${c.menu}`.toLowerCase();
      return w.every((x) => text.includes(x));
    });
    // commands whose label starts with what's typed come first
    cmds.sort((a, b) => Number(!a.label.toLowerCase().startsWith(q.toLowerCase())) - Number(!b.label.toLowerCase().startsWith(q.toLowerCase())));
    const out: Item[] = cmds.map((c) => ({ kind: "cmd", c }));
    if (w.length && q.trim().length >= 2 && doc) {
      const ents = doc.entities
        .filter((e) => {
          const text = `${e.classname} ${keyOf(e.keys, "targetname") ?? ""} ${e.id}`.toLowerCase();
          return w.every((x) => text.includes(x));
        })
        .slice(0, 8)
        .map((e): Item => ({ kind: "ent", id: e.id, label: keyOf(e.keys, "targetname") ? `${e.classname} “${keyOf(e.keys, "targetname")}”` : e.classname, note: `Entity ${e.id}` }));
      out.push(...ents);
    }
    return out;
  }, [q, all, doc]);
  useEffect(() => setAt(0), [q]);
  useEffect(() => {
    list.current?.querySelector<HTMLElement>(`[data-i="${at}"]`)?.scrollIntoView({ block: "nearest" });
  }, [at]);

  const go = (it: Item | undefined) => {
    if (!it) return;
    if (it.kind === "cmd") {
      if (it.c.when && !it.c.when()) return;
      onClose();
      void it.c.run();
    } else {
      onClose();
      selectIds({ brushes: [], entities: [it.id] });
      const e = useEditor.getState().doc?.entities.find((x) => x.id === it.id);
      const o = e && originOf(e.keys);
      if (o) useTools.getState().look(o, 64);
      else frame(false);
    }
  };

  return (
    <Sheet label="Find a command" onClose={onClose} className="sheet ed-palette">
      <input
        className="ed-palette-input"
        autoFocus
        type="search"
        placeholder="Type a command or an entity's name…"
        value={q}
        aria-label="Find a command"
        onChange={(e) => setQ(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setAt((i) => Math.min(items.length - 1, i + 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setAt((i) => Math.max(0, i - 1));
          } else if (e.key === "Enter") {
            e.preventDefault();
            go(items[at]);
          }
        }}
      />
      <ul className="ed-palette-list" ref={list} role="listbox" aria-label="Commands">
        {items.length === 0 && <li className="muted ed-palette-empty">Nothing matches “{q}”.</li>}
        {items.map((it, i) => {
          const off = it.kind === "cmd" && it.c.when ? !it.c.when() : false;
          const k = it.kind === "cmd" ? keysOf(it.c.id)[0] : undefined;
          return (
            <li key={it.kind === "cmd" ? it.c.id : `e${it.id}`} data-i={i} role="option" aria-selected={i === at} aria-disabled={off}>
              <button type="button" className={i === at ? "on" : undefined} disabled={off} onPointerMove={() => setAt(i)} onClick={() => go(it)}>
                <span className="ed-palette-label">
                  {it.kind === "cmd" ? it.c.label : it.label}
                  <span className="muted small-note"> {it.kind === "cmd" ? (it.c.hint ? `· ${it.c.hint}` : "") : `· ${it.note}`}</span>
                </span>
                <span className="ed-palette-side">
                  <span className="muted small-note">{it.kind === "cmd" ? it.c.menu : "Go to"}</span>
                  {k && <kbd>{showKey(k)}</kbd>}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      <p className="small-note muted">↑ ↓ to choose, Enter to run, Esc to close. Greyed out: pick something first.</p>
    </Sheet>
  );
}

/** Keys the old handler does that aren't commands (they can't be changed). */
const FIXED: [string, string][] = [
  ["Arrows, Page Up/Down", "Nudge what's picked one grid step (corners in the vertex tool, textures in the texture tool)"],
  ["Alt (hold)", "Ignore the grid while you drag"],
  ["Esc", "Clear what's picked, or forget half-made clip points"],
  ["1-9 / Ctrl+1-9", "Go to / keep a camera mark (3D view)"],
  ["Tab / Enter", "Clip tool: which side stays / cut"],
  ["Shift-drag the gizmo", "Drag copies, leave the originals"],
  ["Double-click a pane's header", "Maximise it / put it back"],
  // P4: box select, prefabs and components
  ["Drag on empty space", "Box select (flat views: from anywhere but a picked brush). Right: fully inside, left: touching"],
  ["Shift / Ctrl+box", "Add to / take away from the pick"],
  ["Esc while boxing", "Cancel the box"],
  ["Texture tool, box", "Pick faces"],
  ["Drag a Library card", "Place a prefab or component (Alt: off the grid)"],
  ["Shift+click (Place)", "Keep placing; Esc stops"],
  ["Vertex tool", "Drag corners (yellow), edges (blue), faces (purple)"],
];

export function CheatSheet({ onClose }: { onClose: () => void }) {
  const st = useKeySet();
  const all = commands();
  const preset = PRESETS.find((p) => p.id === st.preset);
  return (
    <Sheet label="Keys" onClose={onClose} className="sheet ed-cheats">
      <h2>Keys · Map editor</h2>
      <p className="small-note muted">
        Key set: {preset?.label}
        {Object.keys(st.mine).length ? ", with your own changes" : ""}.{" "}
        <button type="button" className="linklike" onClick={() => useP5Set("keys")}>
          Change keys…
        </button>
      </p>
      <div className="ed-cheats-cols">
        {MENUS.map((m) => {
          const rows = all.filter((c) => c.menu === m && keysOf(c.id).length);
          if (!rows.length) return null;
          return (
            <section key={m}>
              <h3>{m}</h3>
              <dl>
                {rows.map((c) => (
                  <div key={c.id}>
                    <dt>
                      {keysOf(c.id).map((k, i) => (
                        <span key={k}>
                          {i > 0 && <span className="muted"> or </span>}
                          <kbd>{showKey(k)}</kbd>
                        </span>
                      ))}
                    </dt>
                    <dd>{c.label}</dd>
                  </div>
                ))}
              </dl>
            </section>
          );
        })}
        <section>
          <h3>Always</h3>
          <dl>
            {FIXED.map(([k, what]) => (
              <div key={k}>
                <dt>
                  <kbd>{k}</kbd>
                </dt>
                <dd>{what}</dd>
              </div>
            ))}
          </dl>
        </section>
      </div>
      <div className="row">
        <span className="push" />
        <button type="button" className="btn small" onClick={() => window.print()}>
          Print
        </button>
        <button type="button" className="btn small primary" onClick={onClose}>
          Done
        </button>
      </div>
    </Sheet>
  );
}

const useP5Set = (d: "keys") => useP5.getState().set({ dialog: d });

export function KeyEditor({ onClose }: { onClose: () => void }) {
  const st = useKeySet();
  const [q, setQ] = useState("");
  const [grab, setGrab] = useState<string | null>(null);
  const all = commands();
  const clash = clashes(st);
  const w = words(q);
  const rows = all.filter((c) => w.every((x) => `${c.label} ${c.menu} ${keysOf(c.id, st).join(" ")}`.toLowerCase().includes(x)));

  // the next key pressed becomes the command's key
  useEffect(() => {
    if (!grab) return;
    const on = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopImmediatePropagation();
      if (e.key === "Escape") return setGrab(null);
      const k = keyName(e);
      if (!k) return;
      const cur = keysOf(grab, useKeySet.getState());
      if (!cur.map(norm).includes(norm(k))) useKeySet.getState().setMine(grab, [...cur, k]);
      setGrab(null);
    };
    window.addEventListener("keydown", on, true);
    return () => window.removeEventListener("keydown", on, true);
  }, [grab]);

  const pick = (p: PresetId) => {
    st.setPreset(p);
    // Hammer people expect the four views
    if (p === "hammer") setLayout("quad");
  };

  return (
    <Sheet label="Change keys" onClose={grab ? undefined : onClose} className="sheet ed-keyedit">
      <h2>Keys</h2>
      <div className="row wrap">
        <span className="muted">Start from</span>
        <div className="seg" role="group" aria-label="Key set">
          {PRESETS.map((p) => (
            <button key={p.id} type="button" aria-pressed={st.preset === p.id} title={p.tip} onClick={() => pick(p.id)}>
              {p.label}
            </button>
          ))}
        </div>
        <button type="button" className="btn small" disabled={!Object.keys(st.mine).length} onClick={() => st.reset()} title="Forget your own changes">
          Put back {PRESETS.find((p) => p.id === st.preset)?.label}'s keys
        </button>
      </div>
      <p className="small-note muted">{PRESETS.find((p) => p.id === st.preset)?.tip}. Picking Hammer-style also switches to the four views. Kept on this computer.</p>
      <input type="search" placeholder="Find a command or key" value={q} onChange={(e) => setQ(e.target.value)} aria-label="Find a command or key" />
      {clash.size > 0 && (
        <p className="small-note ed-keyedit-clash">
          {clash.size} key{clash.size === 1 ? " does" : "s do"} two jobs: {[...clash.keys()].slice(0, 4).map(showKey).join(", ")}. The first command in the list wins.
        </p>
      )}
      <div className="ed-keyedit-list">
        {rows.map((c) => {
          const ks = keysOf(c.id, st);
          return (
            <div key={c.id} className="ed-keyedit-row">
              <span>
                {c.label} <span className="muted small-note">{c.menu}</span>
              </span>
              <span className="ed-keyedit-keys">
                {ks.map((k) => (
                  <span key={k} className={`ed-keychip${clash.has(norm(k)) ? " clash" : ""}`}>
                    <kbd>{showKey(k)}</kbd>
                    <button type="button" aria-label={`Remove ${showKey(k)}`} title="Remove this key" onClick={() => st.setMine(c.id, ks.filter((x) => x !== k))}>
                      ×
                    </button>
                  </span>
                ))}
                <button type="button" className="btn small" aria-pressed={grab === c.id} onClick={() => setGrab(grab === c.id ? null : c.id)}>
                  {grab === c.id ? "Press a key… (Esc: stop)" : "Add key"}
                </button>
                {st.mine[c.id] && (
                  <button type="button" className="linklike small-note" title="Back to the key set's key" onClick={() => st.setMine(c.id, null)}>
                    reset
                  </button>
                )}
              </span>
            </div>
          );
        })}
      </div>
      <div className="row">
        <span className="push" />
        <button type="button" className="btn small primary" disabled={!!grab} onClick={onClose}>
          Done
        </button>
      </div>
    </Sheet>
  );
}
