// What the app fetches, cached per target (your own game or a server) with TanStack Query:
// each target keeps its own data, a failed request keeps the last good answer, and a
// switch never shows one target's data on another's page.

import { QueryClient, useQuery } from "@tanstack/react-query";
import { useEffect, useRef, useState } from "react";
import { getJsonAt, ON_THIS_COMPUTER } from "./api";
import { useNav } from "./store";
import type { AppInfo, GameState, MapInfo, Online, ServerList } from "./types";
import type { Job } from "./views/Setup";
import type { Scan } from "./views/Setup";

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      // polling is per query; a window coming back to the front checks again
      refetchOnWindowFocus: true,
      staleTime: 1000,
    },
  },
});

/** The open target: null is your own game. */
export const useTarget = () => useNav((s) => s.server);

const LOST = ON_THIS_COMPUTER
  ? "Lost contact with Loner-Strike. If it keeps happening, restart the app."
  : "Can't reach your computer. Check it's on and on the same Wi-Fi as this phone.";

/** How long a game that stops answering still shows as live (a map change, a slow reply). */
const GRACE_MS = 6000;

/** The live game (or server), every 2 seconds while the window is visible. A short gap in
 *  answers (a map loading) keeps the last live state, flagged `reconnecting`, instead of
 *  every live page falling back to "start a game". */
export function useGameState() {
  const target = useTarget();
  const q = useQuery({
    queryKey: ["state", target],
    queryFn: ({ signal }) => getJsonAt<GameState>("/api/state", target, signal),
    refetchInterval: 2000,
    retry: 0,
  });
  const last = useRef<{ target: string | null; data: Online; at: number } | null>(null);
  // re-render once the grace period runs out, even if no new answer comes
  const [, tick] = useState(0);
  const data = q.data;
  // a failed check keeps the old data in TanStack Query: the newer of the two decides
  const failing = q.isError && q.errorUpdatedAt >= q.dataUpdatedAt;
  let state: GameState | null = null;
  let reconnecting = false;
  if (data?.online && !failing) {
    last.current = { target, data, at: q.dataUpdatedAt };
    state = data;
  } else if (data || q.isError) {
    const l = last.current;
    if (l && l.target === target && Date.now() - l.at < GRACE_MS) {
      state = l.data;
      reconnecting = true;
    } else if (failing) {
      state = { online: false, reason: LOST };
    } else {
      state = data ?? { online: false, reason: LOST };
    }
  }
  useEffect(() => {
    if (!reconnecting) return;
    const t = setTimeout(() => tick((n) => n + 1), GRACE_MS);
    return () => clearTimeout(t);
  }, [reconnecting]);
  const refresh = () => queryClient.invalidateQueries({ queryKey: ["state", target] });
  return { state, reconnecting, refresh };
}

/** The game (or server) the app is about: version, what's installed, updates. */
export function useAppInfo() {
  const target = useTarget();
  return useQuery({ queryKey: ["app", target], queryFn: ({ signal }) => getJsonAt<AppInfo>("/api/app", target, signal) });
}
export const reloadAppInfo = () => queryClient.invalidateQueries({ queryKey: ["app"] });

/** The target's maps. */
export function useMaps() {
  const target = useTarget();
  return useQuery({
    queryKey: ["maps", target],
    queryFn: async ({ signal }) => {
      const m = await getJsonAt<MapInfo[]>("/api/maps", target, signal);
      return Array.isArray(m) ? m : [];
    },
  });
}
/** Looks at the maps folder again (a map added or learned), for the open target. */
export async function reloadMaps() {
  const target = useNav.getState().server;
  try {
    const m = await getJsonAt<MapInfo[]>("/api/maps?refresh", target);
    if (Array.isArray(m)) queryClient.setQueryData(["maps", target], m);
  } catch {
    /* the last list stays */
  }
}

/** The Steam installs on this computer and what's set up in them. */
export function useScan() {
  return useQuery({ queryKey: ["scan"], queryFn: ({ signal }) => getJsonAt<Scan>("/api/setup/scan", null, signal), staleTime: Infinity });
}
export const reloadScan = () => queryClient.invalidateQueries({ queryKey: ["scan"] });

/** The dedicated servers, every 5 seconds. */
export function useServers() {
  return useQuery({
    queryKey: ["servers"],
    queryFn: ({ signal }) => getJsonAt<ServerList>("/api/servers", null, signal),
    refetchInterval: 5000,
  });
}
export const reloadServers = () => queryClient.invalidateQueries({ queryKey: ["servers"] });

// ---- the setup job (one for the whole app) --------------------------------------------

const fetchJob = (signal?: AbortSignal) => getJsonAt<Job>("/api/setup/job", null, signal);

/** The setup job, polled while one runs. Every page showing install progress reads this. */
export function useSetupJob() {
  return useQuery({
    queryKey: ["setupJob"],
    queryFn: ({ signal }) => fetchJob(signal),
    refetchInterval: (q) => (q.state.data?.state === "running" ? 800 : false),
  });
}

/** Follows the setup job to its end: the finished job, or null if the app stopped
 *  answering (10 failed checks in a row) so a caller can say so instead of spinning on.
 *  Call it right after starting a job. */
export async function waitForSetupJob(): Promise<Job | null> {
  let misses = 0;
  let seenRunning = false;
  for (;;) {
    await new Promise((r) => setTimeout(r, 800));
    let j: Job;
    try {
      j = await fetchJob();
    } catch {
      if (++misses >= 10) return null;
      continue;
    }
    misses = 0;
    queryClient.setQueryData(["setupJob"], j);
    if (j.state === "running") seenRunning = true;
    if (j.state === "done" || j.state === "error") return j;
    // the job went away (the app restarted): nothing more to follow
    if (j.state === "idle" && seenRunning) return null;
    if (j.state === "idle" && ++misses >= 10) return null;
  }
}
