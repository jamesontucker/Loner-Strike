// The crosshair & config builder's server side (tools/playercfg.rs): the player's own
// settings, written to userconfig.cfg in the game folder (a block of ours; their lines stay).

import { get, post } from "../common/api";

export type Bind = { key: string; action: string };
export type Choice = { settings: Record<string, string>; binds: Bind[] };

export type GameCfg = {
  game: "cstrike" | "czero";
  label: string;
  exists: boolean;
  /** what the game uses now, every setting the builder knows */
  current: Record<string, string>;
  /** config.cfg's binds plus the player's own (key -> command) */
  binds: Record<string, string>;
  /** what our block sets now */
  ours: Choice;
  hasBlock: boolean;
  ownLines: number;
  /** settings and keys a line of theirs after our block sets again (theirs win) */
  overridden: string[];
  canRestore: boolean;
};
export type Status = { games: GameCfg[]; running: boolean };
type Done = { ok?: boolean; message?: string; status?: GameCfg };

export const cfgStatus = (signal?: AbortSignal) => get<Status>("/api/tools/playercfg", signal);
export const previewCfg = (game: string, choice: Choice) => post<{ lines?: string[] }>("/api/tools/playercfg/preview", { game, choice });
export const saveCfg = (game: string, choice: Choice) => post<Done>("/api/tools/playercfg/save", { game, choice });
export const removeCfg = (game: string) => post<Done>("/api/tools/playercfg/remove", { game });
export const restoreCfg = (game: string) => post<Done>("/api/tools/playercfg/restore", { game });
