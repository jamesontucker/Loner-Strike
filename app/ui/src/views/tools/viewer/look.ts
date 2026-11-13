// The game's look for the open map: the player's picture settings, the sky box and the light
// at each model, from /api/tools/map/<name>/look (tools/render.rs), and the "Look like the
// game" / "Bright" choice (remembered on this device).

import { useQuery } from "@tanstack/react-query";
import { create } from "zustand";
import { GAME_DEFAULTS, SKY_SIDES, type LookMode, type LookSettings } from "./gameLook";

export type MapLook = {
  settings: LookSettings;
  /** which config files the settings came from ("cstrike/config.cfg") */
  settingsFrom: string[];
  sky: { name: string; sides: Record<string, boolean> } | null;
  sun: { colour: [number, number, number]; dir: [number, number, number] } | null;
  /** entities drawn with a model: the light at their spot (0..255+) and where it comes from */
  models: { index: number; model: string; light: [number, number, number]; dir: [number, number, number]; sky: boolean }[];
};

const base = (map: string) => `/api/tools/map/${encodeURIComponent(map)}`;

/** The six sky pictures' addresses, in the game's order, or null when one isn't there. */
export function skyUrls(map: string, look: MapLook | null): string[] | null {
  if (!look?.sky || !SKY_SIDES.every((s) => look.sky!.sides[s])) return null;
  return SKY_SIDES.map((s) => `${base(map)}/sky/${s}.png`);
}

async function getLook(map: string, signal?: AbortSignal): Promise<MapLook | null> {
  const r = await fetch(`${base(map)}/look`, { credentials: "same-origin", signal });
  if (!r.ok) return null; // an older app (or the dev mock): the game's defaults
  const v = await r.json().catch(() => null);
  if (!v || typeof v !== "object" || typeof v.error === "string") return null;
  const s = { ...GAME_DEFAULTS, ...(v.settings ?? {}) } as LookSettings;
  return { settings: s, settingsFrom: v.settingsFrom ?? [], sky: v.sky ?? null, sun: v.sun ?? null, models: Array.isArray(v.models) ? v.models : [] };
}

/** The map's look; null until it's here (or if the app can't say: the game's defaults). */
export function useLook(map: string | null, mock = false) {
  return useQuery({
    queryKey: ["tools", "look", map],
    queryFn: ({ signal }) => getLook(map!, signal),
    enabled: !!map && !mock,
    staleTime: 60_000,
    retry: false,
  });
}

const KEY = "ls.toolsLook";
function saved(): LookMode {
  try {
    return localStorage.getItem(KEY) === "bright" ? "bright" : "game";
  } catch {
    return "game";
  }
}

export const useLookMode = create<{ mode: LookMode; setMode: (m: LookMode) => void }>((set) => ({
  mode: saved(),
  setMode: (mode) => {
    try {
      localStorage.setItem(KEY, mode);
    } catch {
      /* remembered for this visit only */
    }
    set({ mode });
  },
}));
