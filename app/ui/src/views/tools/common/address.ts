// Addresses: every spot in Tools has one, and go("tools", address) opens it.
//
//   overview                     the cards
//   <tool>                       a tool (registry.tsx id)
//   <tool>:<map>                 a tool on a map (map viewer "map", nav fixer "navfix", ...)
//   <tool>:<map>@x,y,z[,size]    ...looking at that spot (GoldSrc units)
//   replay:<id>                  a recording
//   replay:<id>@t[,player]       ...at that moment (game time), following that player
//
// Each open tool says where it is (useReportAddress); the map viewer and Replay are worked
// out from the shared view state. Tools uses that to come back to the last map per tool,
// for "Copy link to here", and for the "Open in..." links between tools.

import { useEffect } from "react";
import { create } from "zustand";
import { useToolPage } from "../../../stores/toolPage";
import { useTools } from "../../../stores/tools";
import type { Vec3 } from "../viewer/coords";

export type Address = { tool: string; arg: string; map: string; at: Vec3 | null; size: number | null };

/** "navfix:cs_rats2@1,2,3,96" -> its parts. */
export function parseAddress(s: string): Address {
  const text = s.trim().replace(/^ls-tools:/, "");
  const colon = text.indexOf(":");
  const tool = colon < 0 ? text : text.slice(0, colon);
  const arg = colon < 0 ? "" : text.slice(colon + 1);
  const [map, spot] = arg.split("@");
  const n = (spot ?? "").split(",").map(Number);
  const at = n.length >= 3 && n.slice(0, 3).every(Number.isFinite) ? ([n[0], n[1], n[2]] as Vec3) : null;
  return { tool, arg, map: map ?? "", at, size: at && Number.isFinite(n[3]) && n[3] > 0 ? n[3] : null };
}

/** The address of a spot on a map in a tool ("map", "navfix", "callouts"...). */
export function spotAddress(tool: string, map: string, at?: Vec3 | null, size?: number | null): string {
  if (!at) return `${tool}:${map}`;
  const xyz = at.map((v) => Math.round(v)).join(",");
  return `${tool}:${map}@${xyz}${size ? `,${Math.round(size)}` : ""}`;
}

/** Plain "x y z" (or "x, y, z", or "setpos x y z") typed in a box -> a point, or null. */
export function parsePoint(s: string): Vec3 | null {
  const n = s
    .replace(/^\s*setpos\s+/i, "")
    .split(/[\s,;]+/)
    .filter(Boolean)
    .map(Number);
  return n.length >= 3 && n.slice(0, 3).every(Number.isFinite) ? ([n[0], n[1], n[2]] as Vec3) : null;
}

const reported = create<{ byTool: Record<string, string | null> }>(() => ({ byTool: {} }));

const LAST = (tool: string) => `ls.tools.last.${tool}`;

/** The address a tool was last left at (its map), from an earlier visit; null if none. */
export function lastAddress(tool: string): string | null {
  try {
    const v = localStorage.getItem(LAST(tool));
    return v && parseAddress(v).tool === tool ? v : null;
  } catch {
    return null;
  }
}

/** Keeps where a tool is ("navfix:cs_rats2"; just "navfix" when it's back at its map list). */
function remember(tool: string, address: string | null) {
  try {
    localStorage.setItem(LAST(tool), address || tool);
  } catch {
    // storage off (a private window): it just isn't remembered
  }
}

/** The tool open last time ("" the overview), to come back to it. */
export function lastTool(): string {
  try {
    return localStorage.getItem("ls.tools.tool") ?? "";
  } catch {
    return "";
  }
}

/** Keeps which tool is open, and the map viewer's map and Replay's recording as they change. */
export function rememberWhereYouAre(): () => void {
  const a = useToolPage.subscribe((s, was) => {
    if (s.tool === was.tool) return;
    try {
      localStorage.setItem("ls.tools.tool", s.tool);
    } catch {
      // storage off
    }
  });
  const b = useTools.subscribe((s, was) => {
    if (s.map !== was.map && useToolPage.getState().tool === "map") remember("map", s.map ? `map:${s.map}` : null);
    if (s.replay !== was.replay && useToolPage.getState().tool === "replay") remember("replay", s.replay ? `replay:${s.replay}` : null);
  });
  return () => {
    a();
    b();
  };
}

/** A tool page says where it is (e.g. `navfix:${map}`, or null with no map open). */
export function useReportAddress(tool: string, address: string | null) {
  useEffect(() => {
    reported.setState((s) => ({ byTool: { ...s.byTool, [tool]: address } }));
    remember(tool, address);
  }, [tool, address]);
  useEffect(() => () => reported.setState((s) => ({ byTool: { ...s.byTool, [tool]: null } })), [tool]);
}

/** What a tool page last said about where it is (null before it says). */
export const useReportedAddress = (tool: string) => reported((s) => s.byTool[tool] ?? null);

/** Where Tools is right now, as an address ("" on the overview). */
export function currentAddress(): string {
  const tool = useToolPage.getState().tool;
  if (!tool) return "";
  const t = useTools.getState();
  if (tool === "map") {
    if (!t.map) return "map";
    // (a picked face's point, else where the camera is)
    const at = t.selection?.kind === "face" ? t.selection.point : (t.status?.pos ?? null);
    return spotAddress("map", t.map, at);
  }
  if (tool === "replay") return t.replay ? `replay:${t.replay}` : "replay";
  return reported.getState().byTool[tool] ?? tool;
}

/** Calls fn whenever the open tool, map or reported address changes. */
export function subscribeAddress(fn: () => void): () => void {
  let last = "";
  const check = () => {
    const now = currentAddress().replace(/@.*/, "");
    if (now !== last) {
      last = now;
      fn();
    }
  };
  const a = useToolPage.subscribe(check);
  const b = useTools.subscribe(check);
  const c = reported.subscribe(check);
  check();
  return () => {
    a();
    b();
    c();
  };
}
