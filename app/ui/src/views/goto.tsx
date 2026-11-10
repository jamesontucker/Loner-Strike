import type { ReactNode } from "react";
import { useNav, type Section } from "../store";

/** For your own game, the sidebar page that matches a server tab (Servers.tsx TABS ids). */
const OWN_PAGE: Partial<Record<string, Section>> = {
  players: "players", moderation: "community", stats: "stats", admins: "admins", welcome: "welcome",
  mods: "mods", maps: "maps", skins: "skins", sounds: "sounds", extras: "extras",
  modes: "play", bots: "bots", match: "match", fun: "fun", join: "friends",
};

// a section inside the tab to open on (Moderation's "bans", say), taken once by the page that has it
let pendingSub: string | null = null;

/** Opens a tab of the open server's page (or your own game's page of the same kind), and
 *  optionally a section inside it. */
export function openTab(tab: string, sub?: string) {
  const nav = useNav.getState();
  pendingSub = sub ?? null;
  if (nav.server) return nav.openServerTab(tab as Section);
  const page = OWN_PAGE[tab];
  if (page) nav.go(page);
}

/** The section openTab asked for, if it's one of ids (taken, so it's used only once). */
export function takeSub<T extends string>(ids: readonly T[]): T | null {
  const s = pendingSub as T | null;
  if (!s || !ids.includes(s)) return null;
  pendingSub = null;
  return s;
}

/** A link in running text that opens another tab. */
export function TabLink({ tab, sub, children }: { tab: string; sub?: string; children: ReactNode }) {
  return <button type="button" className="link inline" onClick={() => openTab(tab, sub)}>{children}</button>;
}
