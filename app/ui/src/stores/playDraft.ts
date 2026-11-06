import { create } from "zustand";
import { createJSONStorage, persist, type StateStorage } from "zustand/middleware";
import { queryClient } from "../queries";

// ---- the Start a game form, kept per game ---------------------------------------------
// What you picked on Play survives leaving the page and restarting the app. Only what you
// chose is kept; anything left alone follows the game (last map, the mode's own settings).

export type PlayDraft = {
  mode?: string;
  bots?: number;
  map?: string;
  /** settings changed on the form (cvar: value) */
  changed: Record<string, string>;
};

const EMPTY: PlayDraft = { changed: {} };

type Drafts = {
  /** by game id ("cstrike", "czero") */
  drafts: Record<string, PlayDraft>;
  set: (game: string, d: Partial<PlayDraft>) => void;
  /** a start sent, by target: the time, until the game comes up or the request fails
   *  (kept outside the page, so leaving and coming back doesn't offer Start twice) */
  launching: Record<string, number>;
  startLaunch: (target: string) => void;
  endLaunch: (target: string) => void;
};

// localStorage can throw (private window, storage off): then the draft lasts until the app closes
const safe: StateStorage = {
  getItem: (k) => {
    try {
      return localStorage.getItem(k);
    } catch {
      return null;
    }
  },
  setItem: (k, v) => {
    try {
      localStorage.setItem(k, v);
    } catch {
      /* private window */
    }
  },
  removeItem: (k) => {
    try {
      localStorage.removeItem(k);
    } catch {
      /* private window */
    }
  },
};

export const usePlayDrafts = create<Drafts>()(
  persist(
    (set) => ({
      drafts: {},
      set: (game, d) => set((s) => ({ drafts: { ...s.drafts, [game]: { ...EMPTY, ...s.drafts[game], ...d } } })),
      launching: {},
      startLaunch: (target) => set((s) => ({ launching: { ...s.launching, [target]: Date.now() } })),
      endLaunch: (target) =>
        set((s) => {
          if (!(target in s.launching)) return s;
          const launching = { ...s.launching };
          delete launching[target];
          return { launching };
        }),
    }),
    { name: "ls.playDraft", storage: createJSONStorage(() => safe), partialize: (s) => ({ drafts: s.drafts }) },
  ),
);

/** The form for one game, and a setter for it. */
export function usePlayDraft(game: string) {
  const draft = usePlayDrafts((s) => s.drafts[game]) ?? EMPTY;
  const set = usePlayDrafts((s) => s.set);
  return [draft, (d: Partial<PlayDraft>) => set(game, d)] as const;
}

// A game that comes up ends "Starting…", whichever page is open at the time.
queryClient.getQueryCache().subscribe((e) => {
  const [kind, target] = e.query.queryKey as [string, string | null];
  if (kind !== "state") return;
  if ((e.query.state.data as { online?: boolean } | undefined)?.online) usePlayDrafts.getState().endLaunch(target ?? "local");
});
