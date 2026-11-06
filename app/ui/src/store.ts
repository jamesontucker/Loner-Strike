import { useEffect } from "react";
import { create } from "zustand";

// ---- where you are in the app ------------------------------------------------------

export type Section =
  | "play" | "maps" | "bots" | "match" | "fun" | "players" | "addons" | "friends" | "servers"
  | "admins" | "community" | "extras" | "welcome" | "more" | "mods" | "sounds" | "settings";

const SECTIONS: Section[] = ["play", "maps", "bots", "match", "fun", "players", "addons", "friends", "servers", "admins", "community", "extras", "welcome", "more", "mods", "sounds", "settings"];

function saved<T>(key: string, fallback: T, ok: (v: string) => boolean = () => true): T {
  try {
    const v = localStorage.getItem(key);
    return v !== null && ok(v) ? (v as unknown as T) : fallback;
  } catch {
    return fallback;
  }
}
function keep(key: string, v: string) {
  try {
    localStorage.setItem(key, v);
  } catch {
    /* private window */
  }
}

type Nav = {
  section: Section;
  /** The dedicated server whose pages are open; null is your own game. Every request
   *  names it (X-Target), and each one has its own cached data. */
  server: string | null;
  moreOpen: boolean;
  setupOpen: boolean;
  /** "Set up later" on the first run (per device) */
  setupLater: boolean;
  /** Settings to scroll to when it opens ("updates") */
  focus: string | null;
  go: (s: Section, focus?: string) => void;
  openServer: (id: string | null) => void;
  setMoreOpen: (v: boolean) => void;
  setSetupOpen: (v: boolean) => void;
  laterSetup: () => void;
  clearFocus: () => void;
};

export const useNav = create<Nav>((set, get) => ({
  section: saved<Section>("ls.section", "play", (v) => SECTIONS.includes(v as Section)),
  server: null,
  moreOpen: false,
  setupOpen: false,
  setupLater: saved<string>("ls.setupLater", "0") === "1",
  focus: null,
  go: (section, focus) => {
    // only the Servers area works on a server; every other page is your own game (and
    // Servers again, from a server's page, is the list)
    if (get().server && (section !== "servers" || get().section === "servers")) get().openServer(null);
    keep("ls.section", section);
    set({ section, moreOpen: false, focus: focus ?? null });
    // an open question about the old page closes (ConfirmProvider)
    window.dispatchEvent(new Event("ls-nav"));
    document.querySelector(".content")?.scrollTo({ top: 0 });
  },
  openServer: (server) => {
    if (server === get().server) return;
    abortTargetRequests();
    set({ server });
    window.dispatchEvent(new Event("ls-target"));
  },
  setMoreOpen: (moreOpen) => set({ moreOpen }),
  setSetupOpen: (setupOpen) => set({ setupOpen }),
  laterSetup: () => {
    keep("ls.setupLater", "1");
    set({ setupOpen: false, setupLater: true });
  },
  clearFocus: () => set({ focus: null }),
}));

/** The open target as the API names it (null: your own game). */
export const currentTarget = () => useNav.getState().server;

// ---- requests that belong to the open target -----------------------------------------
// Switching server cancels them, so a reply for the old one never lands on the new one's page.

let controllers = new Set<AbortController>();
export function targetSignal(): { signal: AbortSignal; done: () => void } {
  const c = new AbortController();
  controllers.add(c);
  return { signal: c.signal, done: () => controllers.delete(c) };
}
function abortTargetRequests() {
  const old = controllers;
  controllers = new Set();
  old.forEach((c) => c.abort());
}
export const isAbort = (e: unknown) => e instanceof DOMException && e.name === "AbortError";

// ---- what's in progress, for the whole app -------------------------------------------
// Kept outside any page, so a spinner and its label are still there after you switch
// pages and come back (the work itself carries on regardless).

type Activity = { running: Record<string, string>; start: (key: string, label: string) => boolean; end: (key: string) => void };
export const useActivityStore = create<Activity>((set, get) => ({
  running: {},
  start: (key, label) => {
    if (key in get().running) return false;
    set((s) => ({ running: { ...s.running, [key]: label } }));
    return true;
  },
  end: (key) =>
    set((s) => {
      const running = { ...s.running };
      delete running[key];
      return { running };
    }),
}));

// ---- unsaved edits on a server's tabs ------------------------------------------------
// Here rather than on the Servers page, which loads only when it's opened.

export const useDirtyStore = create<{ dirty: Record<string, boolean>; set: (key: string, v: boolean) => void }>((set) => ({
  dirty: {},
  set: (key, v) => set((s) => (!!s.dirty[key] === v ? s : { dirty: { ...s.dirty, [key]: v } })),
}));
/** A tab on a server's page reports whether it has unsaved edits (key: unique to the tab,
 *  e.g. "welcome"). The page asks before switching tabs or going back while any tab has
 *  some. Cleared when the tab closes. */
export function useDirty(key: string, dirty: boolean) {
  const set = useDirtyStore((s) => s.set);
  useEffect(() => set(key, dirty), [key, dirty, set]);
  useEffect(() => () => set(key, false), [key, set]);
}
/** Whether a server tab has unsaved edits right now (for leaving from outside the page). */
export const hasUnsavedEdits = () => Object.values(useDirtyStore.getState().dirty).some(Boolean);
