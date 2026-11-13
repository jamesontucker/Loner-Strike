// Community creations' server side (tools/community.rs). Files live on GameBanana; the
// Loner-Strike index adds modes, bot readiness, health check results and trust.

import { get, post, tryGet } from "../common/api";

export type Kind = "map" | "mode" | "prefab" | "spray" | "texture" | "other";
export type Model = "Mod" | "Spray";

export type Trust = {
  level: "content" | "data" | "reviewed" | "verified" | "unreviewed" | "blocked";
  label: string;
  allowed: boolean;
  confirm: string | null;
  note: string;
};

export type Badge = "featured" | "bots" | "nav" | "checked" | "data";

export type Entry = {
  model: Model;
  id: number;
  key: string;
  kind: Kind;
  name: string;
  author: string;
  summary: string;
  category: string;
  image: string | null;
  likes: number;
  added: number;
  updated: number;
  /** in Loner-Strike's index (else only tagged on GameBanana) */
  listed: boolean;
  map: string | null;
  modes: string[];
  /** the map is already in your game */
  have: boolean;
  badges: Badge[];
  trust: Trust;
  canTry: boolean;
  url: string;
};

export type Browse = {
  items: Entry[];
  index: "live" | "cached" | "none";
  gamebanana: "live" | "cached" | "offline";
  indexUpdated: number;
  tag: string;
};

export type MapMeta = { name: string; bspSize: number; bspSha256: string; modes: string[]; nav: boolean; places: boolean; lsv: boolean; res: boolean; overview: boolean };
export type HealthMeta = { verdict: "good" | "warn" | "bad" | string; when: number; mode: string; minutes: number; bots: number; stuckPct: number; falls: number; killsPerBotMin: number; coveragePct: number | null };
export type PluginMeta = { file: string; sha256: string; source: string };
export type ModeMeta = { id: string; plugins: PluginMeta[]; review: "none" | "verified" | "reviewed"; maps: string[] };
export type Item = {
  kind: Kind; model: Model; gb: number; file: number; md5: string; name: string; author: string; summary: string; games: string[];
  map: MapMeta | null; mode: ModeMeta | null; health: HealthMeta | null; screenshots: string[]; tags: string[]; featured: boolean;
  added: number; updated: number; status: "listed" | "hidden" | "revoked";
};

export type GbFile = { id: number; file: string; size: number; added: number; md5: string; clean: boolean; supported: boolean };

export type InstalledInfo = { key: string; file: number; installed: number; maps: string[]; mode: string; prefabs: string[]; update: number | null };

export type Details = {
  model: Model; id: number; kind: Kind; name: string; author: string; category: string; text: string; images: string[];
  files: GbFile[]; url: string; tagged: boolean; likes: number; downloads: number; item: Item | null; trust: Trust;
  canTry: boolean; why: string | null; have: boolean; installed: InstalledInfo | null;
};

export type Readiness = {
  blockers: string[]; warnings: string[]; tips: string[]; map: MapMeta; health: HealthMeta | null; files: string[]; title: string; game: string;
};

export type Package = {
  file: string; size: number; md5: string; sha256: string; contents: string[]; screenshots: string[]; item: Item; warnings: string[];
};

export type Form = { submitUrl: string; game: string; category: string; title: string; text: string; tag: { title: string; value: string }; file: string; md5: string };

/** listed / pending (a moderator looks first) / refused (error says why) / waiting (not sent yet); "sent" from older versions */
export type MineState = "listed" | "pending" | "refused" | "waiting" | "sent";
export type Mine = { item: Item; url: string; state: MineState; when: number; error?: string };
export type PackageRow = { file: string; name: string; size: number; md5: string; made: number; map: string | null };

const base = "/api/tools/community";

export const browse = (kind: string, q: string, sort: string, refresh = false, signal?: AbortSignal) =>
  get<Browse>(`${base}/browse?kind=${encodeURIComponent(kind)}&q=${encodeURIComponent(q)}&sort=${encodeURIComponent(sort)}${refresh ? "&refresh=1" : ""}`, signal);

export const details = (model: Model, id: number, signal?: AbortSignal) => get<Details>(`${base}/item/${model.toLowerCase()}/${id}`, signal);

export const tryIt = (model: Model, id: number, confirmed: boolean, file?: number) =>
  post<{ ok: boolean; job: string; section: string; map: string | null; kind?: "prefab" | "mode"; prefabs?: string[]; wads?: string[]; mode?: string; name?: string }>(`${base}/try`, { model, id, confirmed, ...(file ? { file } : {}) });

export const readiness = (map: string, signal?: AbortSignal) =>
  get<{ ready: Readiness; suggestedModes: string[]; tag: string }>(`${base}/publish/${encodeURIComponent(map)}`, signal);

export type Shot = { name: string; data: string };
export const publish = (map: string, body: { title: string; summary: string; author: string; modes: string[]; screenshots: Shot[] }) =>
  post<{ package: Package; form: Form }>(`${base}/publish/${encodeURIComponent(map)}`, body);

export const register = (url: string, file: string) =>
  post<{ ok: boolean; state: MineState; error?: string | null; item: Item; copy: string; studio?: boolean }>(`${base}/register`, { url, file });

export type ReportReason = "broken" | "malware" | "stolen" | "offensive" | "not-loner-strike" | "other";
export const REPORT_REASONS: { id: ReportReason; label: string }[] = [
  { id: "broken", label: "It doesn't work (crashes, missing files, broken)" },
  { id: "malware", label: "It looks harmful (malware, something that runs code)" },
  { id: "stolen", label: "It's someone else's work" },
  { id: "offensive", label: "It's offensive or inappropriate" },
  { id: "not-loner-strike", label: "It isn't a Loner-Strike creation" },
  { id: "other", label: "Something else" },
];
export const NOTE_MAX = 500;
/** Sends a report to the Loner-Strike moderators: the page, a reason and an optional note, nothing about you. */
export const report = (model: Model, id: number, reason: ReportReason, note: string) => post<{ ok: boolean }>(`${base}/report`, { model, id, reason, note });

export const mine = (signal?: AbortSignal) => get<{ mine: Mine[]; packages: PackageRow[] }>(`${base}/mine`, signal);

export const reveal = (file: string) => post<{ ok: boolean }>(`${base}/reveal`, { file });

export type StartMode = { id: string; name: string };
export const startModes = () => tryGet<{ modes: StartMode[] }>("/api/app/start-options");

/** Starts the game on a map with bots (optionally in a mode), like the Play page does. */
export const launch = (map: string | null, mode?: string) => post<{ ok?: boolean }>("/api/app/launch", { ...(map ? { map } : {}), ...(mode ? { mode } : {}) });

// ---- the library (tools/communitylib.rs): what's installed from Community ----

export type Credit = { model: Model; gb: number; name: string; author: string };
export type Installed = {
  key: string; kind: Kind; model: Model; gb: number; file: number; md5: string; name: string; author: string; game: string; section: string;
  maps: string[]; prefabs: string[]; mode: string; installed: number; url: string; update: number | null; pulled: boolean; remixOf: Credit | null;
};
export type { Chip } from "./chip";
import type { Chip } from "./chip";
export type Library = { items: Installed[]; maps: Record<string, Chip>; modes: Record<string, Chip>; prefabs: Record<string, Chip>; pages: Record<string, Chip> };

export const library = (signal?: AbortSignal) => get<Library>(`${base}/library`, signal);
export const uninstall = (key: string) => post<{ ok: boolean; removed: string[] }>(`${base}/uninstall`, { key });
export const update = (key: string) => post<{ ok: boolean; job?: string; kind?: string }>(`${base}/update`, { key });
export const remix = (key: string) => post<{ ok: boolean; id: string; name: string }>(`${base}/remix`, { key });

export type ModeReady = { id: string; name: string; description: string; nodes: number; blockers: string[]; remixOf: Credit | null };
export const modeReadiness = (id: string, signal?: AbortSignal) => get<ModeReady>(`${base}/publish-mode/${encodeURIComponent(id)}`, signal);
export const publishMode = (id: string, body: { title: string; summary: string; author: string; maps: string[]; screenshots: Shot[] }) =>
  post<{ package: Package; form: Form }>(`${base}/publish-mode/${encodeURIComponent(id)}`, body);
export type StudioGraph = { id: string; name: string; description?: string; nodes?: number; damaged?: boolean; installed?: boolean };
export const studioGraphs = () => tryGet<{ graphs: StudioGraph[] }>("/api/tools/studio/graphs");

export type ServerItem = { key: string; kind: Kind; name: string; author: string; url: string; maps: string[]; mode: string; on: boolean; there: boolean };
export type ServerView = { server: { id: string; name: string; remote: boolean; ready: boolean; game: string }; items: ServerItem[] };
export const serverContent = (id: string, signal?: AbortSignal) => get<ServerView>(`${base}/server/${encodeURIComponent(id)}`, signal);
export const setServerContent = (id: string, picks: { maps: string[]; modes: string[] }) =>
  post<{ ok: boolean; job: string | null; problems: string[]; uploaded: boolean | null }>(`${base}/server/${encodeURIComponent(id)}`, picks);

export const KIND_LABEL: Record<Kind, string> = { map: "Map", mode: "Game mode", prefab: "Prefab", spray: "Spray", texture: "Textures", other: "Other" };

export const BADGE_LABEL: Record<Badge, { label: string; help: string }> = {
  featured: { label: "Featured", help: "Picked by the Loner-Strike team." },
  bots: { label: "Works with bots", help: "Ships with a bot nav and finished map knowledge: bots play it well from the first round." },
  nav: { label: "Bot nav", help: "Ships with a bot nav. The bots' map knowledge fills in as you play." },
  checked: { label: "Checked with bots", help: "A Loner-Strike health check found no real problems (bots didn't fall, get stuck or hit errors)." },
  data: { label: "Settings only", help: "A Game Mode Studio mode: settings, rules and graphs only, nothing that runs code." },
};

export const sizeText = (b: number) => (b >= 1 << 20 ? `${(b / (1 << 20)).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1024))} KB`);

export const ago = (ts: number) => {
  if (!ts) return "";
  const d = Math.max(0, Date.now() / 1000 - ts);
  if (d < 3600) return "just now";
  if (d < 86400) return `${Math.round(d / 3600)} h ago`;
  if (d < 86400 * 60) return `${Math.round(d / 86400)} days ago`;
  return new Date(ts * 1000).toLocaleDateString();
};
