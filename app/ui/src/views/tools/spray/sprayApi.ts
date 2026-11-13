// The spray maker's server side (tools/spray.rs): the picture is chosen with the system's
// file dialog and kept by the app; the page only sends the choices.

import { getJson } from "../../../api";
import { post } from "../common/api";

export type Choice = {
  size: "small" | "medium" | "large";
  look: "colour" | "mono";
  /** "#rrggbb", the paint colour of a one-colour spray */
  paint: string;
  /** one-colour: paint the light parts instead of the dark ones */
  invert: boolean;
  clear: "alpha" | "background" | "none";
  dither: boolean;
};

export const DEFAULT_CHOICE: Choice = { size: "large", look: "colour", paint: "#ff7818", invert: false, clear: "alpha", dither: false };

export type SprayImage = { png: string; width: number; height: number; colour: boolean; colours: number; error?: undefined };
export type GameSpray = {
  game: "cstrike" | "czero";
  label: string;
  current: SprayImage | { error: string } | null;
  readOnly: boolean;
  canRestore: boolean;
};
export type Status = { games: GameSpray[]; picked: { name: string; width: number; height: number } | null; error?: string };


export const sprayStatus = () => getJson<Status>("/api/tools/spray");
export const pickPicture = () => post<{ cancelled?: boolean; name?: string; width?: number; height?: number }>("/api/tools/spray/pick");
export const previewSpray = (c: Choice) => post<Partial<Omit<SprayImage, "error">>>("/api/tools/spray/preview", c);
export const installSpray = (choice: Choice, games: string[], lock: boolean) =>
  post<{ ok?: boolean; message?: string; installed?: string[] }>("/api/tools/spray/install", { choice, games, lock });
export const restoreSpray = (game: string) => post<{ ok?: boolean; message?: string }>("/api/tools/spray/restore", { game });
