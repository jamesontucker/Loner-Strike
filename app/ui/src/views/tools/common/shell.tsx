// The Tools shell around every tool: the breadcrumb trail (Tools › tool › map › page), Back,
// history (each address its own Back step: Alt+←, the mouse's back button, the Back button)
// and Focus mode (the tool fills the window; Shift+Mod+Enter, remembered per tool).
//
// Tools get it all from their address (common/address.ts) with no code. A tool with pages
// deeper than its map (a report, a creation) reports them as its address too, so Back steps
// out of them, and may name the crumbs with useToolCrumbs:
//
//   useReportAddress("health", `health:report:${id}`);
//   useToolCrumbs("health", [{ label: "de_dust2", address: "health:de_dust2" }, { label: "Report" }]);
//
// Clicking a crumb, Back and Forward open that address "exactly": the tool's page starts over
// on it (its start page for a bare "<tool>"), so a tool only has to read useToolArg() when it
// mounts. The app's unsaved-edits check (store.ts useDirty) is asked first.

import { useEffect, useMemo, useState, type ReactNode } from "react";
import { create } from "zustand";
import { hasUnsavedEdits, isPageEntry, useNav } from "../../../store";
import { useToolPage } from "../../../stores/toolPage";
import { useTools } from "../../../stores/tools";
import { useConfirm } from "../../../ui";
import type { Vec3 } from "../viewer/coords";
import { TOOLS } from "../registry";
import { currentAddress, lastAddress, parseAddress, subscribeAddress } from "./address";
import "./shell.css";

/** One step of the trail. With an address it's a link; the last one is where you are. */
export type Crumb = { label: string; address?: string; title?: string };

type Shell = {
  /** bumped to start the open tool's page over (Tools keys the page on it) */
  n: number;
  focus: boolean;
  /** the trail after the tool's name, per tool (null: worked out from the address) */
  crumbs: Record<string, Crumb[] | null>;
};
export const useShell = create<Shell>(() => ({ n: 0, focus: false, crumbs: {} }));

const MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);
export const FOCUS_KEY = MAC ? "⇧⌘Enter" : "Ctrl+Shift+Enter";
const BACK_KEY = MAC ? "⌥←" : "Alt+←";

/** A tool names the crumbs after its own name (e.g. its map, then a page in it). */
export function useToolCrumbs(tool: string, trail: Crumb[] | null) {
  const key = JSON.stringify(trail);
  useEffect(() => {
    useShell.setState((s) => ({ crumbs: { ...s.crumbs, [tool]: key === "null" ? null : (JSON.parse(key) as Crumb[]) } }));
  }, [tool, key]);
  useEffect(() => () => useShell.setState((s) => ({ crumbs: { ...s.crumbs, [tool]: null } })), [tool]);
}

/** Opens an address: "overview", "<tool>", "<tool>:<map>[@x,y,z[,size]]", "replay:<id>".
 *  A bare "<tool>" comes back to its last map, unless `exact` (then it's the tool's start
 *  page); `exact` also starts the tool's page over when it's the one already open. */
export function openAddress(focus: string, exact = false) {
  const t = useTools.getState();
  const a = parseAddress(focus);
  const what = a.tool;
  const same = useToolPage.getState().tool === what;
  // while the page loads and says where it is, the history entry is corrected, not added to
  const settle = (to: string) => {
    settling = { to, until: Date.now() + 2000 };
    // (a page that never says that address: written as it is when the wait is over)
    setTimeout(() => kick(), 2050);
  };
  if (!what || what === "overview") return t.setTool("");
  if (what === "map") {
    t.setTool("map");
    if (!a.arg) {
      if (exact) return t.openMap(null);
      // no map asked for and none open: the one looked at last time
      if (!t.map) {
        const last = parseAddress(lastAddress("map") ?? "map");
        if (last.map) t.openMap(last.map);
      }
      return;
    }
    if (a.map !== t.map) t.openMap(a.map);
    // the viewer looks there once the map is up (Cameras' LookAt waits for it)
    if (a.at) t.look(a.at as Vec3, a.size ?? 192);
  } else if (what === "replay") {
    t.setTool("replay");
    if (a.arg) t.openReplay(a.arg);
    else if (exact) t.openReplay(null);
  } else if (TOOLS.some((x) => x.id === what)) {
    // "<tool>:<map>@x,y,z": the map, and a spot to look at; no map asked for: the last one
    const b = a.arg || exact ? a : parseAddress(lastAddress(what) ?? what);
    settle(b.map ? `${what}:${b.map}` : what);
    t.setTool(what);
    useTools.setState({ arg: b.map || null });
    if (b.at) t.look(b.at, b.size ?? 192);
    if (exact && same) useShell.setState((s) => ({ n: s.n + 1 }));
  }
}

/** Where Tools is, without the spot: "overview", "doctor", "doctor:de_dust2". */
const here = () => (currentAddress() || "overview").replace(/@.*/, "");

/** Follows here() as it changes. */
function useHere(): string {
  const [a, setA] = useState(here);
  useEffect(() => subscribeAddress(() => setA(here())), []);
  return a;
}

type ToolsEntry = { ls: 1; section: "tools"; server: string | null; tools?: string; depth?: number };
const toolsEntry = (): ToolsEntry | null => {
  const h = history.state as unknown;
  return isPageEntry(h) && h.section === "tools" ? (h as ToolsEntry) : null;
};

// while an address opened from the history settles (a tool page loading, then saying its
// map), the entry is corrected in place instead of a new step being added
let settling: { to: string; until: number } | null = null;
/** writes the history entry for where Tools is now (set while Tools is open) */
let kick = () => {};

/** Goes to an address from a crumb or Back: asks first about unsaved edits. */
async function leaveTo(address: string, ask: ReturnType<typeof useConfirm>) {
  if (address === here()) return;
  if (hasUnsavedEdits()) {
    if (!(await ask("Leave without saving your changes?", { ok: "Leave", danger: true }))) return;
    // the question's sheet takes its history entry off as it closes; going before that lands
    // would let the history step bring this page straight back (App.tsx waits the same way)
    await new Promise((r) => setTimeout(r, 50));
  }
  openAddress(address, true);
}

/** Back: the step before in Tools, or up one crumb when Tools was opened here. */
function goBack(up: string | null, ask: ReturnType<typeof useConfirm>) {
  if ((toolsEntry()?.depth ?? 0) > 0) history.back();
  else if (up) void leaveTo(up, ask);
}

const isField = (t: EventTarget | null) => t instanceof HTMLElement && (t.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(t.tagName) || !!t.closest(".monaco-editor"));

/** Mounted once by Tools: the history steps, Back/Forward keys and buttons, the focus key. */
export function useToolsShell(startFrom: (address: string | null) => void) {
  const ask = useConfirm();
  const tool = useToolPage((s) => s.tool);
  const focus = useShell((s) => s.focus);

  // where Tools opens: the step Back (or a reload) landed on, else what the caller picks
  useEffect(() => {
    const h = toolsEntry();
    if (!useNav.getState().focus && h?.tools) {
      openAddress(h.tools, true);
    } else startFrom(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // every address is a step in the history
  useEffect(() => {
    const write = () => {
      if (useNav.getState().section !== "tools") return;
      const addr = here();
      const h = toolsEntry();
      // (a page loading says less than where it's going for a moment: not a step)
      if (settling) {
        if (addr !== settling.to && Date.now() < settling.until) return;
        settling = null;
      }
      try {
        if (!h) {
          history.replaceState({ ls: 1, section: "tools", server: useNav.getState().server, tools: addr, depth: 0 } satisfies ToolsEntry, "");
          return;
        }
        if (h.tools === addr) return;
        if (!h.tools) history.replaceState({ ...h, tools: addr, depth: 0 }, "");
        else history.pushState({ ls: 1, section: "tools", server: h.server, tools: addr, depth: (h.depth ?? 0) + 1 } satisfies ToolsEntry, "");
      } catch {
        // history unavailable
      }
    };
    // once the page's effects have all run (a page swapping parts says "no map" for a moment
    // before it says its map), and before anything else happens (a Back right after)
    let queued = false;
    let live = true;
    const later = () => {
      if (queued) return;
      queued = true;
      queueMicrotask(() => {
        queued = false;
        if (live) write();
      });
    };
    kick = later;
    const unsub = subscribeAddress(later);
    return () => {
      live = false;
      kick = () => {};
      unsub();
    };
  }, []);

  // Back and Forward inside Tools
  useEffect(() => {
    let asking = false;
    const onPop = async (e: PopStateEvent) => {
      if (asking) return;
      const to = e.state as ToolsEntry | null;
      if (!isPageEntry(to) || to.section !== "tools" || !to.tools || useNav.getState().section !== "tools") return;
      const now = here();
      if (to.tools === now) return;
      if (hasUnsavedEdits()) {
        asking = true;
        const leave = await ask("Leave without saving your changes?", { ok: "Leave", danger: true });
        await new Promise((r) => setTimeout(r, 50));
        asking = false;
        if (!leave) {
          history.pushState({ ls: 1, section: "tools", server: to.server, tools: now, depth: (to.depth ?? 0) + 1 } satisfies ToolsEntry, "");
          return;
        }
        history.replaceState(to, "");
      }
      openAddress(to.tools, true);
    };
    // the mouse's back and forward buttons, Alt+← and Alt+→ (not while typing)
    const onMouse = (e: MouseEvent) => {
      if (e.button !== 3 && e.button !== 4) return;
      e.preventDefault();
      if (e.button === 3) history.back();
      else history.forward();
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented || !e.altKey || e.metaKey || e.ctrlKey || e.shiftKey || isField(e.target)) return;
      if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
      // (the map editor's arrows nudge without Alt, so Alt+← is Back there too: P5)
      if (document.querySelector(".sheet-backdrop")) return;
      e.preventDefault();
      if (e.key === "ArrowLeft") history.back();
      else history.forward();
    };
    window.addEventListener("popstate", onPop);
    window.addEventListener("mouseup", onMouse);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("popstate", onPop);
      window.removeEventListener("mouseup", onMouse);
      window.removeEventListener("keydown", onKey);
    };
  }, [ask]);

  // Focus mode: each tool as it was left; the overview never
  useEffect(() => {
    useShell.setState({ focus: !!tool && savedFocus()[tool] === true });
  }, [tool]);
  useEffect(() => {
    const on = focus && !!tool;
    document.documentElement.toggleAttribute("data-tools-focus", on);
    // 3D views and canvases follow their box (ResizeObserver); this is for anything that
    // only listens to the window
    const raf = requestAnimationFrame(() => window.dispatchEvent(new Event("resize")));
    return () => {
      cancelAnimationFrame(raf);
      document.documentElement.removeAttribute("data-tools-focus");
    };
  }, [focus, tool]);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Enter" || !e.shiftKey || !(e.metaKey || e.ctrlKey) || e.altKey) return;
      if (!useToolPage.getState().tool || document.querySelector(".sheet-backdrop") || (e.target instanceof HTMLElement && e.target.closest(".monaco-editor"))) return;
      e.preventDefault();
      e.stopPropagation();
      toggleFocus();
    };
    // (before the tools' own key handlers)
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);
}

function savedFocus(): Record<string, boolean> {
  try {
    const v = JSON.parse(localStorage.getItem("ls.tools.focus") ?? "{}");
    return v && typeof v === "object" ? v : {};
  } catch {
    return {};
  }
}

export function toggleFocus(on = !useShell.getState().focus) {
  const tool = useToolPage.getState().tool;
  if (!tool) return;
  useShell.setState({ focus: on });
  try {
    localStorage.setItem("ls.tools.focus", JSON.stringify({ ...savedFocus(), [tool]: on }));
  } catch {
    // storage off: it just isn't remembered
  }
}

/** The trail for the open tool: Tools › tool › (its crumbs, or its map). */
function useTrail(): Crumb[] {
  const tool = useToolPage((s) => s.tool);
  const reported = useShell((s) => s.crumbs[tool] ?? null);
  const replayMap = useTools((s) => (tool === "replay" ? s.map : null));
  const addr = useHere();
  return useMemo(() => {
    const entry = TOOLS.find((t) => t.id === tool);
    if (!entry) return [{ label: "Tools" }];
    const trail: Crumb[] = [
      { label: "Tools", address: "overview", title: "Every tool" },
      { label: entry.label, address: tool, title: `${entry.label}: back to the start` },
    ];
    if (reported) trail.push(...reported);
    else {
      const a = parseAddress(addr);
      if (a.tool === tool && a.map) trail.push({ label: tool === "replay" ? (replayMap ? `Recording on ${replayMap}` : "Recording") : a.map });
    }
    // the last one is where you are, not a link
    const last = trail[trail.length - 1];
    trail[trail.length - 1] = { label: last.label };
    return trail;
  }, [tool, reported, replayMap, addr]);
}

const Chevron = () => (
  <svg aria-hidden="true" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
    <path d="M9 6l6 6-6 6" />
  </svg>
);

const FocusIcon = ({ on }: { on: boolean }) => (
  <svg aria-hidden="true" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round">
    {on ? <path d="M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5" /> : <path d="M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5" />}
  </svg>
);

/** Back, then the trail. */
export function Breadcrumbs() {
  const ask = useConfirm();
  const trail = useTrail();
  if (trail.length < 2) return null;
  const up = [...trail].reverse().find((c) => c.address)?.address ?? null;
  return (
    <nav className="tools-crumbs" aria-label="Where you are in Tools">
      <button type="button" className="tools-back" title={`Back (${BACK_KEY}, or the mouse's back button)`} aria-label="Back" onClick={() => goBack(up, ask)}>
        <svg aria-hidden="true" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round">
          <path d="M15 6l-6 6 6 6" />
        </svg>
        Back
      </button>
      <ol>
        {trail.map((c, i) => (
          <li key={i}>
            {i > 0 && <Chevron />}
            {c.address ? (
              <button type="button" className="tools-crumb" title={c.title} onClick={() => void leaveTo(c.address!, ask)}>
                {c.label}
              </button>
            ) : (
              <span className="tools-crumb here" aria-current="page" title={c.label}>
                {c.label}
              </span>
            )}
          </li>
        ))}
      </ol>
    </nav>
  );
}

// The big workspaces, where filling the window helps most: a one-time hint points at Focus there.
const FOCUS_HINT_TOOLS = ["editor", "studio", "plugins", "creatures"];
const FOCUS_HINT_KEY = "ls.tools.focusHintSeen";
function focusHintSeen(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(FOCUS_HINT_KEY) ?? "[]");
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}
function markFocusHintSeen(tool: string) {
  try {
    localStorage.setItem(FOCUS_HINT_KEY, JSON.stringify([...new Set([...focusHintSeen(), tool])]));
  } catch {
    /* private mode: it just shows again next time */
  }
}

/** The button in a tool's header that turns Focus mode on and off (with a one-time hint in the big workspaces). */
export function FocusButton() {
  const focus = useShell((s) => s.focus);
  const tool = useToolPage((s) => s.tool);
  const [hint, setHint] = useState(false);
  useEffect(() => {
    if (!tool || focus || !FOCUS_HINT_TOOLS.includes(tool) || focusHintSeen().includes(tool)) return setHint(false);
    setHint(true);
    const t = window.setTimeout(() => {
      setHint(false);
      markFocusHintSeen(tool);
    }, 12000);
    return () => window.clearTimeout(t);
  }, [tool, focus]);
  const close = () => {
    setHint(false);
    if (tool) markFocusHintSeen(tool);
  };
  return (
    <span className="tools-focus-wrap">
      <button
        type="button"
        className="btn small tools-focus-btn"
        aria-pressed={focus}
        title={`${focus ? "Leave focus mode" : "Focus: the tool fills the window"} (${FOCUS_KEY})`}
        onClick={() => {
          close();
          toggleFocus();
        }}
      >
        <FocusIcon on={focus} />
        {focus ? "Exit focus" : "Focus"}
      </button>
      {hint && (
        <span className="tools-focus-hint" role="status">
          <span className="tools-focus-hint-text">
            Need more room? <b>Focus</b> fills the window with this tool <span className="tools-focus-hint-key">({FOCUS_KEY})</span>.
          </span>
          <button type="button" className="btn small tools-focus-hint-x" onClick={close}>
            Got it
          </button>
        </span>
      )}
    </span>
  );
}

/** In focus mode: the way back, in a corner. */
export function FocusPill(): ReactNode {
  const focus = useShell((s) => s.focus);
  const tool = useToolPage((s) => s.tool);
  if (!focus || !tool) return null;
  return (
    <button type="button" className="tools-focus-pill" title={`Show the app around the tool again (${FOCUS_KEY})`} onClick={() => toggleFocus(false)}>
      <FocusIcon on />
      Exit focus <kbd>{FOCUS_KEY}</kbd>
    </button>
  );
}
