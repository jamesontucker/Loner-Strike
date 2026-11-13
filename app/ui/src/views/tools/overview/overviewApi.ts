// The radar (overview) maker's server side (tools/overview.rs).

import { get, getBytes, post } from "../common/api";

export type Style = "game" | "clean" | "simple";
export type Roofs = "nav" | "cut";
export type Orient = "auto" | "north" | "turned";
export type Background = "green" | "black";
export type Format = "bmp" | "tga";
export type MarkKind = "bomb" | "buy" | "rescue" | "vip" | "spawns" | "hostages";

export type Choice = {
  style: Style;
  roofs: Roofs;
  /** the height above which nothing is drawn (roofs: "cut"); null: the app's pick */
  cut: number | null;
  orient: Orient;
  outline: boolean;
  marks: MarkKind[];
  background: Background;
};

export const DEFAULT_CHOICE: Choice = { style: "game", roofs: "nav", cut: null, orient: "auto", outline: true, marks: ["bomb", "rescue"], background: "green" };

export type OverviewInfo = {
  name: string;
  game: string;
  current: { file: string; readable: boolean; image: string | null; imageFound: boolean; ours: boolean; zoom: number | null; rotated: boolean | null } | null;
  hasNav: boolean;
  heights: { low: number; high: number; cut: number };
  frame: { zoom: number; origin: [number, number, number]; rotated: boolean };
  marks: Record<MarkKind, number>;
  stock: boolean;
  backups: boolean;
  inUse: boolean;
};

const base = (map: string) => `/api/tools/overview/${encodeURIComponent(map)}`;

function query(c: Choice): string {
  const q = new URLSearchParams({ style: c.style, roofs: c.roofs, orient: c.orient, outline: String(c.outline), marks: c.marks.join(","), background: c.background });
  if (c.cut !== null) q.set("cut", String(c.cut));
  return q.toString();
}

export const overviewInfo = (map: string, signal?: AbortSignal) => get<OverviewInfo>(base(map), signal);

/** Our picture for these choices, as an object URL (the caller revokes it). */
export async function previewUrl(map: string, c: Choice, signal?: AbortSignal): Promise<string> {
  const b = await getBytes(`${base(map)}/preview.png?${query(c)}`, signal);
  return URL.createObjectURL(new Blob([b], { type: "image/png" }));
}

/** The picture installed now, as an object URL. */
export async function currentUrl(map: string, signal?: AbortSignal): Promise<string> {
  const b = await getBytes(`${base(map)}/current.png`, signal, "This map has no overview yet.");
  return URL.createObjectURL(new Blob([b], { type: "image/png" }));
}

export const installOverview = (map: string, c: Choice, format: Format, res: boolean) =>
  post<{ ok?: boolean; message?: string }>(`${base(map)}/install`, { ...c, marks: c.marks.join(","), cut: c.cut ?? undefined, format, res });

export const restoreOverview = (map: string) => post<{ ok?: boolean; message?: string }>(`${base(map)}/restore`);
