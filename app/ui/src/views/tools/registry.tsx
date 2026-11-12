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
];
