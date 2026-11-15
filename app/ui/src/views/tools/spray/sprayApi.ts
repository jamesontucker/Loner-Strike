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

// the composer: the drawn spray handed over as a PNG, and saved designs (sprays/designs)
export const sendDrawn = (name: string, png: string) => post<{ name?: string; width?: number; height?: number }>("/api/tools/spray/use", { name, png });
export type DesignInfo = { id: string; name: string; updated: number; thumb: string | null };
export const listDesigns = () => getJson<{ designs: DesignInfo[]; error?: string }>("/api/tools/spray/designs");
export const openDesign = (id: string) => getJson<{ id: string; name: string; doc: unknown; error?: string }>(`/api/tools/spray/design?${new URLSearchParams({ id })}`);
export const saveDesign = (id: string | null, name: string, doc: unknown, thumb: string) =>
  post<{ ok?: boolean; id?: string; name?: string }>("/api/tools/spray/design/save", id ? { id, name, doc, thumb } : { name, doc, thumb });
export const deleteDesign = (id: string) => post<{ ok?: boolean }>("/api/tools/spray/design/delete", { id });
