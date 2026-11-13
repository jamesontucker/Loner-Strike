// The Tools page's tools, in the order of its row. Each tool adds one entry here; keep
// entries one per line. A tool is a page with a default export taking ToolProps, loaded
// the first time it's opened (its own chunk). id is what the nav focus opens ("map:de_dust2"
// opens "map" with "de_dust2"; a tool reads that with useToolArg).

import type { ComponentType } from "react";
import type { MapInfo } from "../../types";

export type ToolProps = { maps: MapInfo[] };
export type ToolEntry = { id: string; label: string; blurb: string; icon?: string; beta?: boolean; load: () => Promise<{ default: ComponentType<ToolProps> }> };

export const TOOLS: ToolEntry[] = [
  { id: "map", label: "Map viewer", blurb: "Walk around a map, see every spawn, door and trigger, and the bots' nav and what they learned.", load: () => import("./MapViewer") },
  { id: "replay", label: "Replay", blurb: "Watch a recorded bot match on its map: where every bot went, why, and where it fell.", load: () => import("./Replay") },
  { id: "spray", label: "Spray maker", blurb: "Turn a picture into your in-game spray.", load: () => import("./spray/SprayMaker") },
  { id: "doctor", label: "Map doctor", blurb: "Find what a map is missing (textures, models, sounds, the download list, the bots' nav) and fix it.", beta: true, load: () => import("./doctor/Doctor") },
  { id: "health", label: "Map health check", blurb: "Bots play a map in the background for a few minutes, then a report says where they fall, get stuck or hit errors.", beta: true, load: () => import("./health/HealthCheck") },
  { id: "callouts", label: "Callouts", blurb: "Name places on a map's bot nav, so bots say where they are and understand where you mean.", beta: true, load: () => import("./callouts/Callouts") },
  { id: "editor", label: "Map editor", blurb: "Build your own maps with brushes, like Hammer, in one 3D view. Save as a project or export a .map.", beta: true, load: () => import("./editor/MapEditor") },
  { id: "assets", label: "Textures & models", blurb: "Browse every texture and model in your game, see models in 3D, add your own or get more from GameBanana.", beta: true, load: () => import("./assets/AssetBrowser") },
];
