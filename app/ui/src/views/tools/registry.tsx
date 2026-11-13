// The tools, listed in the sidebar under Tools (grouped Look / Fix / Make / Share, in this order) and
// as cards on the Tools overview. Each tool adds one entry here; keep entries one per line.
// A tool is a page with a default export taking ToolProps, loaded the first time it's opened
// (its own chunk). id is its address: go("tools", "editor") opens it, go("tools", "doctor:de_dust2")
// opens it with "de_dust2" (a tool reads that with useToolArg). The map viewer also takes
// "map:<name>@x,y,z" (looks at that spot) and Replay "replay:<id>".

import type { ComponentType } from "react";
import type { MapInfo } from "../../types";

export type ToolProps = { maps: MapInfo[] };
export type ToolGroup = "Look" | "Fix" | "Make" | "Share";
export const TOOL_GROUPS: ToolGroup[] = ["Look", "Fix", "Make", "Share"];
/** blurb: one line on the overview card and under the tool's title; icon: an icons.tsx name for the sidebar */
export type ToolEntry = { id: string; label: string; group: ToolGroup; blurb: string; icon?: string; beta?: boolean; load: () => Promise<{ default: ComponentType<ToolProps> }> };

export const TOOLS: ToolEntry[] = [
  { id: "map", label: "Map viewer", group: "Look", icon: "maps", blurb: "Walk around a map: spawns, doors, triggers, the bots' nav and what they learned.", load: () => import("./MapViewer") },
  { id: "replay", label: "Replay", group: "Look", icon: "play", blurb: "Watch a recorded bot match: where every bot went, why, and where it fell.", load: () => import("./Replay") },
  { id: "assets", label: "Textures & models", group: "Look", icon: "skins", blurb: "Browse every texture and model in your game, add your own or get more.", beta: true, load: () => import("./assets/AssetBrowser") },
  { id: "health", label: "Map health check", group: "Fix", icon: "stats", blurb: "Bots play a map for a few minutes; a report says where they fall, get stuck or hit errors.", beta: true, load: () => import("./health/HealthCheck") },
  { id: "doctor", label: "Map doctor", group: "Fix", icon: "admins", blurb: "Find what a map is missing (textures, models, sounds, its nav) and fix it.", beta: true, load: () => import("./doctor/Doctor") },
  { id: "navfix", label: "Nav fixer", group: "Fix", icon: "bots", blurb: "Fix where bots can walk: add the beams and ledges they missed, link areas, mark drops.", beta: true, load: () => import("./navfix/NavFixer") },
  { id: "callouts", label: "Callouts", group: "Fix", icon: "sound", blurb: "Name places on a map, so bots say where they are and understand where you mean.", beta: true, load: () => import("./callouts/Callouts") },
  { id: "editor", label: "Map editor", group: "Make", icon: "mods", blurb: "Build your own maps with brushes, like Hammer, in one 3D view.", beta: true, load: () => import("./editor/MapEditor") },
  { id: "spray", label: "Spray maker", group: "Make", icon: "fun", blurb: "Turn a picture into your in-game spray.", load: () => import("./spray/SprayMaker") },
  { id: "community", label: "Community", group: "Share", icon: "friends", blurb: "Find maps, modes and more made with Loner-Strike and try them in a click; publish your own.", beta: true, load: () => import("./community/Community") },
  { id: "plugins", label: "Plugin Studio", group: "Make", icon: "addons", blurb: "Write AMX Mod X plugins with suggestions and docs, then try them in your game in a click.", beta: true, load: () => import("./plugins/PluginStudio") },
];
