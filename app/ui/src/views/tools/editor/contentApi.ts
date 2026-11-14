// P5: the Content Browser's data (tools/editor/browser.rs): models, sprites and sounds with where
// they came from, sprite pictures, sound facts; Favorites and Recent (this computer only); the
// browser's selection (for the entity form's "Use selected" and "Show in Content Browser"); and
// the credits and permission of an asset from T's credits data (docs/notes/Assets.md 2.3, route
// agreed in docs/notes/P5.md), shown as "not checked yet" until that route is there.

import { create } from "zustand";

export type AssetKind = "model" | "sprite" | "sound";
export type AssetSource = "game" | "downloaded" | "community" | "mine" | "other";
export type AssetItem = {
  kind: AssetKind;
  folder: string;
  path: string;
  name: string;
  size: number;
  source: AssetSource;
  page: { key: string; name: string; url: string; author: string } | null;
};
export type DownloadGroup = { key: string; name: string; url: string; count: number };
export type AssetList = { items: AssetItem[]; downloads: DownloadGroup[]; capped?: boolean; game: boolean; error?: string };

const E = "/api/tools/editor/content";
const q = (a: { folder: string; path: string }) => `folder=${encodeURIComponent(a.folder)}&path=${encodeURIComponent(a.path)}`;

const lists = new Map<AssetKind, Promise<AssetList>>();
/** Every asset of a kind (fetched once; `fresh` asks again, after a download or a copy). */
export function assetList(kind: AssetKind, fresh = false): Promise<AssetList> {
  if (fresh) lists.delete(kind);
  let p = lists.get(kind);
  if (!p) {
    p = fetch(`${E}/list?kind=${kind}`, { headers: { "X-Panel": "1" } })
      .then((r) => r.json() as Promise<AssetList>)
      .catch(() => ({ items: [], downloads: [], game: false, error: "The list couldn't be read." }) as AssetList);
    lists.set(kind, p);
    void p.then((r) => r.error && lists.delete(kind));
  }
  return p;
}

export const spriteUrl = (a: { folder: string; path: string }, frame = 0) => `${E}/sprite.png?${q(a)}&frame=${frame}`;
export type SpriteInfo = { frames: number; width: number; height: number; format: string; orientation: string; size: number; error?: string };
export const spriteInfo = (a: { folder: string; path: string }) => fetch(`${E}/sprite/info?${q(a)}`, { headers: { "X-Panel": "1" } }).then((r) => r.json() as Promise<SpriteInfo>);
export type SoundInfo = { format: string; rate?: number; bits?: number; channels?: number; seconds?: number; loops?: boolean; warn?: string | null; note?: string; size?: number; error?: string };
export const soundInfo = (a: { folder: string; path: string }) => fetch(`${E}/sound/info?${q(a)}`, { headers: { "X-Panel": "1" } }).then((r) => r.json() as Promise<SoundInfo>);
/** The editor's sound route plays the first one the game would find (the list's own). */
export const soundPlayUrl = (path: string) => `/api/tools/editor/sound?path=${encodeURIComponent(path)}`;

// ---- credits and permission (T's data) --------------------------------------------------------

export type Verdict = "ok" | "ask" | "no" | "stock" | "unknown";
export type Credits = {
  source: "stock" | "gamebanana" | "community" | "mine" | "manual" | "unknown";
  page?: { url: string; name: string } | null;
  submitter?: { id?: number; name: string } | null;
  authors?: { group?: string; name: string; role?: string; url?: string }[];
  verdict: Verdict;
  why?: string;
  licence?: string;
  ported?: boolean;
};
const credits = new Map<string, Promise<Credits | null>>();
/** null: the credits data isn't there yet (T's route not in this build) or can't be read. */
export function assetCredits(a: { folder: string; path: string }): Promise<Credits | null> {
  const k = `${a.folder}/${a.path}`;
  let p = credits.get(k);
  if (!p) {
    p = fetch(`/api/tools/credits/asset?${q(a)}`, { headers: { "X-Panel": "1" } })
      .then(async (r) => (r.ok ? ((await r.json()) as Credits & { error?: string }) : null))
      .then((c) => (c && !("error" in c && c.error) && c.verdict ? c : null))
      .catch(() => null);
    credits.set(k, p);
  }
  return p;
}

/** The badge words for a verdict. */
export function verdictBadge(v: Verdict | null | undefined): { label: string; tone: "ok" | "ask" | "no" | "muted"; tip: string } {
  switch (v) {
    case "ok":
      return { label: "OK to share", tone: "ok", tip: "Its page lets you use it in a map you share (credit the authors)." };
    case "ask":
      return { label: "Ask the author", tone: "ask", tip: "Its page says to ask before using it in a map you share (often GameBanana's default terms)." };
    case "no":
      return { label: "Not allowed", tone: "no", tip: "Its page says no to using it in a map you share." };
    case "stock":
      return { label: "Game's own", tone: "muted", tip: "Every player has it: it isn't shipped with your map." };
    default:
      return { label: "Not checked yet", tone: "muted", tip: "Where it came from and what it allows isn't known (yet)." };
  }
}

// ---- Favorites, Recent, the selection ----------------------------------------------------------

const FAV = "ls.editor.content.favorites";
const REC = "ls.editor.content.recent";
const read = (k: string): string[] => {
  try {
    const v = JSON.parse(localStorage.getItem(k) ?? "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x === "string").slice(0, 200) : [];
  } catch {
    return [];
  }
};
const write = (k: string, v: string[]) => {
  try {
    localStorage.setItem(k, JSON.stringify(v));
  } catch {
    /* private window */
  }
};
export const assetKey = (a: { kind: string; path: string }) => `${a.kind}:${a.path.toLowerCase()}`;

type BrowserState = {
  favorites: string[];
  recent: string[];
  /** what's picked in the browser (the entity form's "Use selected") */
  selected: AssetItem | null;
  /** asked to show this one (the entity form's "Show in Content Browser") */
  reveal: { kind: AssetKind; path: string; n: number } | null;
  drawer: boolean;
  /** asked to open a section ("prefab" from another page's "editor:prefabs") */
  goto: { section: string; n: number } | null;
  set: (p: Partial<BrowserState>) => void;
};
export const useBrowser = create<BrowserState>((set) => ({ favorites: read(FAV), recent: read(REC), selected: null, reveal: null, drawer: false, goto: null, set }));

/** Opens a section of the browser (models, sprites, sounds, texture, prefab, gamebanana). */
export function gotoSection(section: string) {
  const s = useBrowser.getState();
  try {
    localStorage.setItem("ls.editor.content.section", section);
  } catch {
    /* private window */
  }
  s.set({ goto: { section, n: (s.goto?.n ?? 0) + 1 } });
}

export function toggleFavorite(a: AssetItem) {
  const s = useBrowser.getState();
  const k = assetKey(a);
  const favorites = s.favorites.includes(k) ? s.favorites.filter((x) => x !== k) : [k, ...s.favorites];
  write(FAV, favorites);
  s.set({ favorites });
}

/** Marks an asset as just used (placed, chosen for a key). */
export function used(a: { kind: string; path: string }) {
  const s = useBrowser.getState();
  const k = assetKey(a);
  const recent = [k, ...s.recent.filter((x) => x !== k)].slice(0, 40);
  write(REC, recent);
  s.set({ recent });
}

/** The kind an entity key's value names (by its FGD type, or its file type). */
export function kindOfKey(value: string, fgdType?: string): AssetKind | null {
  const v = value.trim().toLowerCase();
  if (fgdType === "studio" || v.endsWith(".mdl")) return "model";
  if (fgdType === "sprite" || v.endsWith(".spr")) return "sprite";
  if (fgdType === "sound" || v.endsWith(".wav") || v.endsWith(".mp3")) return "sound";
  return null;
}

/** The value an entity key takes for an asset (sounds without "sound/"). */
export const keyValue = (a: { kind: string; path: string }) => (a.kind === "sound" ? a.path.replace(/^sound\//i, "") : a.path);

/** Does this key value name this asset? */
export function sameAsset(value: string, a: { kind: string; path: string }) {
  const v = value.trim().replace(/\\/g, "/").replace(/^[*#!]/, "").toLowerCase();
  const p = a.path.toLowerCase();
  return v === p || (a.kind === "sound" && `sound/${v}` === p);
}

/** Opens the browser on this asset (its panel, the right section, scrolled to it). */
export function revealAsset(kind: AssetKind, path: string) {
  const s = useBrowser.getState();
  s.set({ reveal: { kind, path, n: (s.reveal?.n ?? 0) + 1 } });
}
