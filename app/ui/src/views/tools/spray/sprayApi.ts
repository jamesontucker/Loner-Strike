// The spray maker's server side (tools/spray.rs): the picture is chosen with the system's
// file dialog and kept by the app; the page only sends the choices.

import { getJson, postJson } from "../../../api";

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

type Err = { error?: string };
const unreachable = { error: "Couldn't reach the app." };

export const sprayStatus = () => getJson<Status>("/api/tools/spray");
export const pickPicture = () => postJson<Err & { cancelled?: boolean; name?: string; width?: number; height?: number }>("/api/tools/spray/pick").catch(() => unreachable);
export const previewSpray = (c: Choice) => postJson<Err & Partial<SprayImage>>("/api/tools/spray/preview", c).catch(() => unreachable);
export const installSpray = (choice: Choice, games: string[], lock: boolean) =>
  postJson<Err & { ok?: boolean; message?: string; installed?: string[] }>("/api/tools/spray/install", { choice, games, lock }).catch(() => unreachable);
export const restoreSpray = (game: string) => postJson<Err & { ok?: boolean; message?: string }>("/api/tools/spray/restore", { game }).catch(() => unreachable);
