// The demo manager's server side (tools/demos.rs).

import { get, post } from "../common/api";

export type Demo = {
  /** which folder: "game:cstrike", "game:czero", "server:<id>" */
  at: string;
  where: string;
  game: string;
  hltv: boolean;
  name: string;
  size: number;
  modified: number;
  /** the game can be started with it (plain name) */
  playable: boolean;
  map?: string;
  seconds?: number;
  protocol?: number;
  error?: string;
};

export type DemoList = { demos: Demo[]; places: { at: string; label: string; folder: string | null }[]; gameOpen: boolean };

export type DemoDetail = {
  map: string;
  game: string;
  protocol: number;
  seconds: number;
  parts: { title: string; seconds: number; frames: number }[];
  names: string[];
  server: string | null;
  maxPlayers: number | null;
  frames: number;
  problem: string | null;
  canReplay: boolean;
};

type Done = { ok?: boolean; message?: string };

export const listDemos = (signal?: AbortSignal) => get<DemoList>("/api/tools/demos", signal);
export const demoDetail = (d: Demo, signal?: AbortSignal) => get<DemoDetail>(`/api/tools/demos/detail?${new URLSearchParams({ at: d.at, name: d.name })}`, signal);
export const renameDemo = (d: Demo, to: string) => post<Done & { name?: string }>("/api/tools/demos/rename", { at: d.at, name: d.name, to });
export const deleteDemo = (d: Demo) => post<Done>("/api/tools/demos/delete", { at: d.at, name: d.name });
export const playDemo = (d: Demo) => post<Done & { gameOpen?: boolean; command?: string; copied?: boolean }>("/api/tools/demos/play", { at: d.at, name: d.name });
export const demoToReplay = (d: Demo) => post<Done & { id?: string; map?: string }>("/api/tools/demos/replay", { at: d.at, name: d.name });
