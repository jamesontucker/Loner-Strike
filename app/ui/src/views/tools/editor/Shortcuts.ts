// P5: the editor's keys. Three key sets (Loner-Strike's own, Hammer-style, TrenchBroom-style)
// plus your own changes on top, kept on this computer. One capture-phase handler runs the
// command a key belongs to; keys the old handler in MapEditor.tsx knows but that you moved
// elsewhere are swallowed so they don't do the old thing too.
//
// Key names: "Mod" is ⌘ on a Mac and Ctrl elsewhere; then Alt, Shift, and the key ("D",
// "1", "Delete", "Space", "[", "?", "F9"). Several keys for one command: a list.

import { useEffect } from "react";
import { create } from "zustand";
import { useTools } from "../../../stores/tools";
import { commands, type Command } from "./commands";
import { useP5 } from "./p5store";
import { useEditor } from "./store";
import { NAV_KEYS, useNav } from "./navigation";
import { navWantsKey } from "./NavCamera";

export const MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

export type PresetId = "loner" | "hammer" | "trenchbroom";
export const PRESETS: { id: PresetId; label: string; tip: string }[] = [
  { id: "loner", label: "Loner-Strike", tip: "One letter per tool, Ctrl for edits" },
  { id: "hammer", label: "Hammer-style", tip: "Shift+letter tools, Ctrl+T tie, Ctrl+M transform, F9 compile, four views" },
  { id: "trenchbroom", label: "TrenchBroom-style", tip: "Letter tools, Ctrl+I isolate, Ctrl+Space maximise" },
];

type Keys = Record<string, string[]>;

/** Loner-Strike's own keys (what the editor always had, plus the new commands). */
const LONER: Keys = {
  "file.new": ["Mod+Alt+N"],
  "file.open": ["Mod+O"],
  "file.save": ["Mod+S"],
  "file.saveAs": ["Mod+Shift+S"],
  "file.export": ["Mod+E"],
  "edit.undo": ["Mod+Z"],
  "edit.redo": ["Mod+Shift+Z", "Mod+Y"],
  "edit.cut": ["Mod+X"],
  "edit.copy": ["Mod+C"],
  "edit.paste": ["Mod+V"],
  "edit.pasteSpecial": ["Mod+Shift+V"],
  "edit.duplicate": ["Mod+D"],
  "edit.duplicateHere": ["Mod+Shift+D"],
  "edit.delete": ["Delete", "Backspace"],
  "edit.find": ["Mod+F"],
  "edit.transform": ["Mod+M"],
  "edit.group": ["Mod+G"],
  "edit.ungroup": ["Mod+Shift+G"],
  "edit.tie": ["Mod+T"],
  "edit.hollow": ["Mod+H"],
  "edit.snap": ["Mod+B"],
  "sel.all": ["Mod+A"],
  "sel.none": ["Mod+Shift+A"],
  "sel.invert": ["Mod+I"],
  "sel.goto": ["Mod+J"],
  "sel.hide": ["H"],
  "sel.isolate": ["Shift+H"],
  "sel.showAll": ["U"],
  "sel.lock": ["L"],
  "sel.unlockAll": ["Shift+L"],
  "view.layout.single": ["Alt+1"],
  "view.layout.quad": ["Alt+2"],
  "view.layout.one3": ["Alt+3"],
  "view.layout.two": ["Alt+4"],
  "view.maximise": ["Shift+Space"],
  "view.3d": ["Shift+1"],
  "view.top": ["Shift+2"],
  "view.front": ["Shift+3"],
  "view.side": ["Shift+4"],
  "view.frame": ["Z"],
  "view.frameAll": ["Shift+Z"],
  "view.fly": ["F"],
  "view.gridDown": ["["],
  "view.gridUp": ["]"],
  "view.gridShow": ["Shift+G"],
  "tool.select": ["Q"],
  "tool.move": ["W"],
  "tool.rotate": ["E"],
  "tool.scale": ["R"],
  "tool.vertex": ["V"],
  "tool.box": ["B"],
  "tool.shape": ["C"],
  "tool.clip": ["X"],
  "tool.entity": ["N"],
  "tool.texture": ["T"],
  "tool.measure": ["M"],
  "help.palette": ["Mod+K", "Mod+Shift+P"],
  "help.cheats": ["?"],
  "map.inspect": ["Alt+Enter"],
  "map.playtest": ["F5"],
  "view.snap": ["Shift+W"],
  "edit.applyTexture": ["Shift+T"],
};

/** Hammer 3.5-style (changes from ours). */
const HAMMER: Keys = {
  ...LONER,
  "file.new": ["Mod+N"],
  "edit.redo": ["Mod+Y", "Mod+Shift+Z"],
  "edit.ungroup": ["Mod+U"],
  "edit.toWorld": ["Mod+Shift+W"],
  "edit.carve": ["Mod+Shift+C"],
  "edit.mirrorX": ["Mod+L"],
  "edit.mirrorZ": ["Mod+I"],
  "sel.invert": [],
  "sel.goto": ["Mod+Shift+G"],
  "edit.find": ["Mod+Shift+F"],
  "view.maximise": ["Shift+Z"],
  "view.frameAll": [],
  "view.frame": ["Mod+Shift+E"],
  "view.texLock": ["Shift+L"],
  "sel.unlockAll": [],
  "map.compile": ["F9"],
  "map.inspect": ["Alt+Enter"],
  "tool.select": ["Shift+S"],
  "tool.move": [],
  "tool.rotate": [],
  "tool.scale": [],
  "tool.entity": ["Shift+E"],
  "tool.box": ["Shift+B"],
  "tool.texture": ["Shift+A"],
  "tool.clip": ["Shift+X"],
  "tool.vertex": ["Shift+V"],
  "tool.measure": [],
  "tool.shape": [],
  "sel.isolate": [],
  "view.gridShow": [],
};

/** TrenchBroom-style (changes from ours). */
const TRENCHBROOM: Keys = {
  ...LONER,
  "edit.ungroup": ["Mod+Shift+G"],
  "sel.hide": ["Mod+H"],
  "sel.isolate": ["Mod+I"],
  "sel.showAll": ["Mod+Shift+H"],
  "sel.invert": [],
  "edit.hollow": [],
  "edit.mirrorX": ["Mod+F"],
  "edit.mirrorZ": ["Mod+Alt+F"],
  "edit.find": [],
  "view.maximise": ["Mod+Space"],
  "view.frame": ["Mod+U"],
  "tool.select": [],
  "tool.move": [],
  "tool.rotate": ["R"],
  "tool.scale": ["T"],
  "tool.clip": ["C"],
  "tool.vertex": ["V"],
  "tool.shape": [],
  "tool.texture": [],
  "tool.box": ["B"],
};

const BASES: Record<PresetId, Keys> = { loner: LONER, hammer: HAMMER, trenchbroom: TRENCHBROOM };

type KeyState = {
  preset: PresetId;
  /** your own keys per command, on top of the preset ([] = no key) */
  mine: Keys;
  setPreset: (p: PresetId) => void;
  setMine: (id: string, keys: string[] | null) => void;
  reset: () => void;
};

const STORE = "ls.editor.keys";
function load(): { preset: PresetId; mine: Keys } {
  try {
    const v = JSON.parse(localStorage.getItem(STORE) ?? "null");
    const preset = PRESETS.some((p) => p.id === v?.preset) ? (v.preset as PresetId) : "loner";
    const mine: Keys = {};
    if (v?.mine && typeof v.mine === "object") for (const [k, l] of Object.entries(v.mine)) if (Array.isArray(l)) mine[k] = l.filter((x) => typeof x === "string").slice(0, 4);
    return { preset, mine };
  } catch {
    return { preset: "loner", mine: {} };
  }
}
function save(s: { preset: PresetId; mine: Keys }) {
  try {
    localStorage.setItem(STORE, JSON.stringify(s));
  } catch {
    /* private window */
  }
}

export const useKeySet = create<KeyState>((set, get) => ({
  ...load(),
  setPreset: (preset) => {
    set({ preset, mine: {} });
    save({ preset, mine: {} });
  },
  setMine: (id, keys) => {
    const mine = { ...get().mine };
    if (keys == null) delete mine[id];
    else mine[id] = keys;
    set({ mine });
    save({ preset: get().preset, mine });
  },
  reset: () => {
    set({ mine: {} });
    save({ preset: get().preset, mine: {} });
  },
}));

/** The keys a command has now. */
export function keysOf(id: string, st: Pick<KeyState, "preset" | "mine"> = useKeySet.getState()): string[] {
  // your own keys, then the navigation preset's (F focuses in Unreal), then the key set's
  return st.mine[id] ?? NAV_KEYS[useNav.getState().preset][id] ?? BASES[st.preset][id] ?? [];
}

/** Every key in use and its command. */
function keyMap(st: Pick<KeyState, "preset" | "mine"> = useKeySet.getState()): Map<string, string> {
  const ids = new Set([...Object.keys(BASES[st.preset]), ...Object.keys(st.mine), ...Object.keys(NAV_KEYS[useNav.getState().preset])]);
  const m = new Map<string, string>();
  for (const id of ids) for (const k of keysOf(id, st)) if (!m.has(norm(k))) m.set(norm(k), id);
  return m;
}

/** Which commands share a key (for the shortcut editor's warning). */
export function clashes(st: Pick<KeyState, "preset" | "mine"> = useKeySet.getState()): Map<string, string[]> {
  const ids = new Set([...Object.keys(BASES[st.preset]), ...Object.keys(st.mine)]);
  const by = new Map<string, string[]>();
  for (const id of ids) for (const k of keysOf(id, st)) by.set(norm(k), [...(by.get(norm(k)) ?? []), id]);
  return new Map([...by].filter(([, l]) => l.length > 1));
}

const SYMBOL = /^[^A-Za-z0-9]$/;

/** One key name, tidied: modifiers in a fixed order. */
export function norm(k: string): string {
  const parts = k.split("+");
  // "Mod++" (plus key) keeps its plus
  const key = parts.pop() || "+";
  const mods = new Set(parts.map((p) => p.toLowerCase()));
  const out: string[] = [];
  if (mods.has("mod") || mods.has("ctrl") || mods.has("cmd")) out.push("Mod");
  if (mods.has("alt")) out.push("Alt");
  if (mods.has("shift") && !SYMBOL.test(key)) out.push("Shift");
  out.push(key.length === 1 ? key.toUpperCase() : key);
  return out.join("+");
}

/** The key name of a key press (null for a lone modifier). */
export function keyName(e: KeyboardEvent): string | null {
  if (["Control", "Meta", "Alt", "Shift", "OS"].includes(e.key)) return null;
  let key: string;
  if (/^Key[A-Z]$/.test(e.code)) key = e.code.slice(3);
  else if (/^Digit[0-9]$/.test(e.code)) key = e.code.slice(5);
  else if (e.code === "Space") key = "Space";
  else if (e.key.length === 1) key = e.key.toUpperCase();
  else key = e.key;
  const mods: string[] = [];
  if (e.ctrlKey || e.metaKey) mods.push("Mod");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey && !(SYMBOL.test(key) && key !== " ")) mods.push("Shift");
  return [...mods, key].join("+");
}

/** How a key reads on this computer: ⌘ on a Mac. */
export function showKey(k: string): string {
  return norm(k)
    .replace(/^Mod\+/, MAC ? "⌘" : "Ctrl+")
    .replace(/Alt\+/, MAC ? "⌥" : "Alt+")
    .replace(/Shift\+/, MAC ? "⇧" : "Shift+");
}

const isField = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName));

/** Ours, by default, that the old handler in MapEditor.tsx also does. */
const OLD_HANDLER = new Set(Object.values(LONER).flat().map(norm));

let cache: { list: Command[]; at: number } | null = null;
function commandById(id: string): Command | undefined {
  // the list is cheap, but a key held down repeats: keep it for a moment
  const now = performance.now();
  if (!cache || now - cache.at > 500) cache = { list: commands(), at: now };
  return cache.list.find((c) => c.id === id);
}

/** Runs a command by id (if it can run now). */
export function runCommand(id: string): boolean {
  const c = commands().find((x) => x.id === id);
  if (!c || (c.when && !c.when())) return false;
  void c.run();
  return true;
}

let wasdTold = false;

/** The keys of a command in the current key set, as shown (" (⌘H)"), or "". */
export function keyHint(id: string): string {
  const k = keysOf(id).slice(0, 2).map(showKey);
  return k.length ? ` (${k.join(" or ")})` : "";
}

/** P5: every button with `data-cmd="<command id>"` shows that command's keys (from the key set
 *  in use, your own keys included) at the end of its tooltip; `data-tip` is the tooltip
 *  without them (else its title is used). Set when the pointer or the focus comes to it. */
function useKeyTitles() {
  useEffect(() => {
    const show = (e: Event) => {
      const el = (e.target as HTMLElement | null)?.closest?.<HTMLElement>("[data-cmd]");
      if (!el) return;
      const added = el.dataset.keyAdded ?? "";
      const base = el.dataset.tip ?? (added && el.title.endsWith(added) ? el.title.slice(0, -added.length) : el.title);
      const suffix = keyHint(el.dataset.cmd!);
      el.title = `${base}${suffix}`;
      el.dataset.keyAdded = suffix;
    };
    document.addEventListener("pointerover", show, true);
    document.addEventListener("focusin", show, true);
    return () => {
      document.removeEventListener("pointerover", show, true);
      document.removeEventListener("focusin", show, true);
    };
  }, []);
}

/** Mounted once by the menu bar: keys to commands. */
export function useShortcuts() {
  useKeyTitles();
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      // the camera's keys while looking round (navigation presets)
      if (navWantsKey(e)) return;
      const held = keyName(e);
      if (e.repeat && held && keyMap().has(held) && !isField(e.target)) {
        // a held key runs its command once (the grid keys may step on)
        const id = keyMap().get(held)!;
        if (id !== "view.gridDown" && id !== "view.gridUp") {
          e.preventDefault();
          e.stopImmediatePropagation();
          return;
        }
      }
      const s = useEditor.getState();
      const p5 = useP5.getState();
      const name = keyName(e);
      if (!name) return;
      // the palette's own keys, and fields, come first
      if (isField(e.target) || s.dialog || p5.dialog) return;
      // a sheet from elsewhere (confirm, toast actions) is up
      if (document.querySelector(".sheet-backdrop")) return;
      const flying = s.view === "3d" && useTools.getState().camera === "fly";
      const id = keyMap().get(name);
      // Unreal: W/E/R pick tools unless right mouse is held; say so once
      if (id && !flying && useNav.getState().preset === "unreal" && s.view === "3d" && /^[WASDQE]$/.test(name) && !wasdTold) {
        wasdTold = true;
        s.notify("Hold right mouse to fly — W/E/R pick tools.");
      }
      if (id) {
        const c = commandById(id);
        if (!c) return;
        if (flying && !c.fly) return;
        e.preventDefault();
        e.stopImmediatePropagation();
        if (!c.when || c.when()) void c.run();
        return;
      }
      // a key moved away from its old job: the old handler mustn't do it
      if (OLD_HANDLER.has(name) && !flying) {
        e.preventDefault();
        e.stopImmediatePropagation();
      }
    };
    window.addEventListener("keydown", down, true);
    return () => window.removeEventListener("keydown", down, true);
  }, []);
}
