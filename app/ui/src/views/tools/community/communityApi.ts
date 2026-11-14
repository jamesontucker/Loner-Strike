// Community creations' server side (tools/community.rs). Files live on GameBanana; the
// Loner-Strike index adds modes, bot readiness, bot simulation results and trust.

import { get, post, tryGet } from "../common/api";

export type Kind = "map" | "mode" | "prefab" | "spray" | "texture" | "other";
export type Model = "Mod" | "Spray";

export type Trust = {
  level: "content" | "data" | "reviewed" | "verified" | "unreviewed" | "blocked";
  label: string;
  allowed: boolean;
  confirm: string | null;
  note: string;
  /** what a mode does beyond its own game, in plain words ("Runs server commands"): a badge each */
  risks?: string[];
  /** the same as codes: sent back with the yes, so the install is bound to what was shown */
  riskCodes?: string[];
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
  /** game modes: how well bots play it (the app's own scale), "" not said */
  bots: "" | "great" | "ok" | "weak" | "none";
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
  /** a mode's "Plays on" and "Needs" (its requirements), each with whether you have it */
  needs?: Needs | null;
};

export type NeedLink = { model: Model; id: number; name: string };
export type Needs = {
  defaultMap: string | null;
  maps: { name: string; have: boolean; base: boolean; item: NeedLink | null }[];
  content: { path: string; kind: string; have: boolean; link: NeedLink | null }[];
};

export type Author = { name: string; role: string; url: string };
export type CreditAsset = {
  rel: string; sha256: string; source: "stock" | "gamebanana" | "community" | "mine" | "manual" | "unknown"; page: string; name: string;
  authors: Author[]; checklist: Record<string, string>; ported: string; verdict: "ok" | "ask" | "no" | "none"; why: string;
  permission: { granted: boolean; note: string; date: number; authors: Author[]; page: string } | null;
};
export type AssetCredit = { files: string[]; page: string; name: string; authors: Author[]; permission: string; note: string };
export type Readiness = {
  blockers: string[]; warnings: string[]; tips: string[]; map: MapMeta; health: HealthMeta | null; files: string[]; title: string; game: string;
  assets: CreditAsset[]; credits: AssetCredit[];
};
export const notePermission = (b: { key: string; granted: boolean; note: string; authors?: Author[]; page?: string }) =>
  post<{ ok: boolean }>(`/api/tools/credits/${b.authors ? "manual" : "permission"}`, b);

export type Package = {
  file: string; size: number; md5: string; sha256: string; contents: string[]; screenshots: string[]; item: Item; warnings: string[];
};

export type Form = {
  submitUrl: string; game: string; category: string; title: string; text: string; tag: { title: string; value: string }; file: string; md5: string;
  /** rows for GameBanana's Credits module */
  credits?: { group: string; name: string; role: string; url: string }[];
};

/** listed / pending (a moderator looks first) / refused (error says why) / waiting (not sent yet); "sent" from older versions */
export type MineState = "listed" | "pending" | "refused" | "waiting" | "sent";
export type Mine = { item: Item; url: string; state: MineState; when: number; error?: string };
export type PackageRow = { file: string; name: string; size: number; md5: string; made: number; map: string | null };

const base = "/api/tools/community";

export const browse = (kind: string, q: string, sort: string, refresh = false, signal?: AbortSignal) =>
  get<Browse>(`${base}/browse?kind=${encodeURIComponent(kind)}&q=${encodeURIComponent(q)}&sort=${encodeURIComponent(sort)}${refresh ? "&refresh=1" : ""}`, signal);

export const details = (model: Model, id: number, signal?: AbortSignal) => get<Details>(`${base}/item/${model.toLowerCase()}/${id}`, signal);

export const tryIt = (model: Model, id: number, confirmed: boolean, file?: number, risks?: string[]) =>
  post<{ ok: boolean; job: string; section: string; map: string | null; kind?: "prefab" | "mode"; prefabs?: string[]; wads?: string[]; mode?: string; name?: string; needsConfirm?: string; risks?: string[]; riskCodes?: string[]; differs?: boolean }>(`${base}/try`, { model, id, confirmed, ...(file ? { file } : {}), ...(risks ? { risks } : {}) });

export const readiness = (map: string, signal?: AbortSignal) =>
  get<{ ready: Readiness; suggestedModes: string[]; modeChoices?: { id: string; name: string; uid: string; studio: boolean }[]; tag: string }>(`${base}/publish/${encodeURIComponent(map)}`, signal);

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
  maps: string[]; prefabs: string[]; mode: string; installed: number; url: string; update: number | null; pulled: boolean; gone?: string | null; remixOf: Credit | null;
  /** a newer file waiting for the map or mode to be free */
  pending: number; previousFile: number; canGoBack: boolean;
};
export type { Chip } from "./chip";
import type { Chip } from "./chip";
export type Library = { items: Installed[]; maps: Record<string, Chip>; modes: Record<string, Chip>; prefabs: Record<string, Chip>; pages: Record<string, Chip>; updates: number; waiting: number };

export const library = (signal?: AbortSignal) => get<Library>(`${base}/library`, signal);
export const uninstall = (key: string) => post<{ ok: boolean; removed: string[] }>(`${base}/uninstall`, { key });
export const update = (key: string) => post<{ ok: boolean; job?: string; kind?: string; state?: "waiting" | "installing" | "done"; message?: string }>(`${base}/update`, { key });
export const updateAll = () => post<{ ok: boolean; updates: { key: string; name: string; result: { state?: string } | null; error: string | null }[] }>(`${base}/update-all`, {});
export const goBack = (key: string) => post<{ ok: boolean; state?: string; job?: string }>(`${base}/go-back`, { key });
export type UpdateNote = { title: string; version: string; date: number; text: string; changes: { cat: string; text: string }[] };
export const updateNotes = (model: Model, id: number, signal?: AbortSignal) => get<{ notes: UpdateNote[] }>(`${base}/notes/${model.toLowerCase()}/${id}`, signal);
export const remix = (key: string) => post<{ ok: boolean; id: string; name: string }>(`${base}/remix`, { key });

export type ModeReady = {
  id: string; name: string; description: string; nodes: number; blockers: string[]; remixOf: Credit | null; botsSuggested: string;
  /** the mode's requirements (requirements.rs), when the game is set up here */
  requirements?: { maps: { name: string }[]; known: { creatures: string[] } };
  requirementWarnings?: string[];
};
export const modeReadiness = (id: string, signal?: AbortSignal) => get<ModeReady>(`${base}/publish-mode/${encodeURIComponent(id)}`, signal);
export const publishMode = (id: string, body: { title: string; summary: string; author: string; maps: string[]; screenshots: Shot[]; bots: string }) =>
  post<{ package: Package; form: Form }>(`${base}/publish-mode/${encodeURIComponent(id)}`, body);
export type StudioGraph = { id: string; name: string; description?: string; nodes?: number; damaged?: boolean; installed?: boolean };
export const studioGraphs = () => tryGet<{ graphs: StudioGraph[] }>("/api/tools/studio/graphs");

export type ServerItem = { key: string; kind: Kind; name: string; author: string; url: string; maps: string[]; mode: string; on: boolean; there: boolean };
export type ServerView = { server: { id: string; name: string; remote: boolean; ready: boolean; game: string }; items: ServerItem[] };
export const serverContent = (id: string, signal?: AbortSignal) => get<ServerView>(`${base}/server/${encodeURIComponent(id)}`, signal);
export const setServerContent = (id: string, picks: { maps: string[]; modes: string[] }) =>
  post<{
    ok: boolean;
    job: string | null;
    problems: string[];
    uploaded: boolean | null;
    /** after a mode went on or came off: the server's menu lists it now, after its next map, or when it starts */
    modes?: "now" | "next_map" | "off";
    modesNote?: string | null;
  }>(`${base}/server/${encodeURIComponent(id)}`, picks);

export const KIND_LABEL: Record<Kind, string> = { map: "Map", mode: "Game mode", prefab: "Prefab", spray: "Spray", texture: "Textures", other: "Other" };

export const BADGE_LABEL: Record<Badge, { label: string; help: string }> = {
  featured: { label: "Featured", help: "Picked by the Loner-Strike team." },
  bots: { label: "Works with bots", help: "Ships with a bot nav and finished map knowledge: bots play it well from the first round." },
  nav: { label: "Bot nav", help: "Ships with a bot nav. The bots' map knowledge fills in as you play." },
  checked: { label: "Checked with bots", help: "A Loner-Strike map simulation found no real problems (bots didn't fall, get stuck or hit errors)." },
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
