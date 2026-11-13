// The tools, listed in the sidebar under Tools (grouped Create / Map tools / Personalize / Watch & inspect / Share, in this order) and
// as cards on the Tools overview. Each tool adds one entry here; keep entries one per line.
// A tool is a page with a default export taking ToolProps, loaded the first time it's opened
// (its own chunk). id is its address: go("tools", "editor") opens it, go("tools", "doctor:de_dust2")
// opens it with "de_dust2" (a tool reads that with useToolArg). The map viewer also takes
// "map:<name>@x,y,z" (looks at that spot) and Replay "replay:<id>".

import type { ComponentType } from "react";
import type { MapInfo } from "../../types";

export type ToolProps = { maps: MapInfo[] };
export type ToolGroup = "Create" | "Map tools" | "Personalize" | "Watch & inspect" | "Share";
export const TOOL_GROUPS: ToolGroup[] = ["Create", "Map tools", "Personalize", "Watch & inspect", "Share"];
/** one line under each group's heading on the overview */
export const TOOL_GROUP_BLURBS: Record<ToolGroup, string> = {
  "Create": "Build your own maps, game modes, creatures and plugins.",
  "Map tools": "Get any map ready for bots and players: fix what's missing, teach the bots, name places, make its radar.",
  "Personalize": "Make the game yours: your crosshair and keys, who the bots are, your spray.",
  "Watch & inspect": "Look around maps, watch bot matches and demos, browse textures and models.",
  "Share": "Find what others made with Loner-Strike and share your own.",
};
/** blurb: one line on the overview card and under the tool's title; icon: an icons.tsx name for the sidebar */
export type ToolEntry = { id: string; label: string; group: ToolGroup; blurb: string; icon?: string; beta?: boolean; load: () => Promise<{ default: ComponentType<ToolProps> }> };

export const TOOLS: ToolEntry[] = [
  { id: "editor", label: "Map editor", group: "Create", icon: "tool-editor", blurb: "Build your own maps with brushes, like Hammer, in one 3D view.", beta: true, load: () => import("./editor/MapEditor") },
  { id: "studio", label: "Game Mode Studio", group: "Create", icon: "tool-studio", blurb: "Make a game mode from nodes and wires, then playtest it with bots and watch the nodes run.", beta: true, load: () => import("./studio/StudioPage") },
  { id: "creatures", label: "Creature Studio", group: "Create", icon: "tool-creatures", blurb: "Make the creatures modes throw at you: zombies, aliens, animals, turrets and bosses, with waves to match.", beta: true, load: () => import("./creatures/Creatures") },
  { id: "plugins", label: "Plugin Studio", group: "Create", icon: "tool-plugins", blurb: "Write AMX Mod X plugins with suggestions and docs, then try them in your game in a click.", beta: true, load: () => import("./plugins/PluginStudio") },
  { id: "doctor", label: "Map doctor", group: "Map tools", icon: "tool-doctor", blurb: "Find what a map is missing (textures, models, sounds, its nav) and fix it.", beta: true, load: () => import("./doctor/Doctor") },
  { id: "health", label: "Map health check", group: "Map tools", icon: "tool-health", blurb: "Bots play a map for a few minutes; a report says where they fall, get stuck or hit errors.", beta: true, load: () => import("./health/HealthCheck") },
  { id: "navfix", label: "Nav fixer", group: "Map tools", icon: "tool-navfix", blurb: "Fix where bots can walk: add the beams and ledges they missed, link areas, mark drops.", beta: true, load: () => import("./navfix/NavFixer") },
  { id: "callouts", label: "Callouts", group: "Map tools", icon: "tool-callouts", blurb: "Name places on a map, so bots say where they are and understand where you mean.", beta: true, load: () => import("./callouts/Callouts") },
  { id: "radar", label: "Radar maker", group: "Map tools", icon: "tool-radar", blurb: "Make a map's overview: the top-down picture spectators and HLTV viewers see, drawn from the map.", beta: true, load: () => import("./overview/Overview") },
  { id: "playercfg", label: "Crosshair & config", group: "Personalize", icon: "tool-playercfg", blurb: "Make your crosshair, keys, mouse and connection settings, with the crosshair drawn live.", beta: true, load: () => import("./playercfg/PlayerConfig") },
  { id: "people", label: "Bot people", group: "Personalize", icon: "tool-people", blurb: "Decide who the bots are: kinds of people, how they play and talk, and the regulars' names.", beta: true, load: () => import("./people/People") },
  { id: "spray", label: "Spray maker", group: "Personalize", icon: "tool-spray", blurb: "Make your in-game spray from your pictures, words and stickers: move, turn, colour, see it as the game draws it.", load: () => import("./spray/SprayMaker") },
  { id: "map", label: "Map viewer", group: "Watch & inspect", icon: "tool-map", blurb: "Walk around a map: spawns, doors, triggers, the bots' nav and what they learned.", load: () => import("./MapViewer") },
  { id: "replay", label: "Replay", group: "Watch & inspect", icon: "tool-replay", blurb: "Watch a recorded bot match: where every bot went, why, and where it fell.", load: () => import("./Replay") },
  { id: "demos", label: "Demos", group: "Watch & inspect", icon: "tool-demos", blurb: "Your recorded demos and your servers' HLTV ones: what's in them, rename, delete, watch.", beta: true, load: () => import("./demos/Demos") },
  { id: "assets", label: "Textures & models", group: "Watch & inspect", icon: "tool-assets", blurb: "Browse every texture and model in your game, add your own or get more.", beta: true, load: () => import("./assets/AssetBrowser") },
  { id: "community", label: "Community", group: "Share", icon: "tool-community", blurb: "Find maps, modes and more made with Loner-Strike and try them in a click; publish your own.", beta: true, load: () => import("./community/Community") },
];
