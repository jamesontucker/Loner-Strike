import type { ReactNode } from "react";
import { useNav, type Section } from "../store";

/** For your own game, the sidebar page that matches a server tab (Servers.tsx TABS ids). */
const OWN_PAGE: Partial<Record<string, Section>> = {
  players: "players", moderation: "community", stats: "stats", admins: "admins", welcome: "welcome",
  mods: "mods", maps: "maps", skins: "skins", sounds: "sounds", more: "more", extras: "extras",
  modes: "play", bots: "bots", match: "match", fun: "fun", addons: "addons", join: "friends",
};

// a section inside the tab to open on (Moderation's "bans", say), taken once by the page that has
// it, and only just after the link (a switch that was refused or went elsewhere leaves it unused,
// and it mustn't turn up later on another server's page)
let pendingSub: { sub: string; at: number } | null = null;
const SUB_FOR_MS = 3000;

/** Opens a tab of the open server's page (or your own game's page of the same kind), and
 *  optionally a section inside it. */
export function openTab(tab: string, sub?: string) {
  const nav = useNav.getState();
  pendingSub = sub ? { sub, at: Date.now() } : null;
  if (nav.server) return nav.openServerTab(tab as Section);
  const page = OWN_PAGE[tab];
  if (page) nav.go(page);
}

/** The section openTab asked for, if it's one of ids (taken, so it's used only once). */
export function takeSub<T extends string>(ids: readonly T[]): T | null {
  const p = pendingSub;
  if (!p || Date.now() - p.at > SUB_FOR_MS) {
    pendingSub = null;
    return null;
  }
  if (!ids.includes(p.sub as T)) return null;
  pendingSub = null;
  return p.sub as T;
}

/** A link in running text that opens another tab. */
export function TabLink({ tab, sub, children }: { tab: string; sub?: string; children: ReactNode }) {
  return <button type="button" className="link inline" onClick={() => openTab(tab, sub)}>{children}</button>;
}
