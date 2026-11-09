// Shapes of what the app's local server returns (same protocol as the Python panel).

export type LiveState = {
  mode: string;
  map: string;
  nav: number;
  career: number;
  navbatch: number;
  navleft: number;
  /** bots playing now */
  bots: number;
  /** the bot count asked for (the saved target); missing on older plugins, use `bots` then */
  quota?: number;
  humans: number;
  maxplayers: number;
  timeleft: number;
  presets: number;
  addons_pending: number;
  version: string;
  crashmap: string;
  preset: string;
  peaceful: number;
  paused: number;
  recording: number;
  timelimit: number;
};

export type Mode = { id: string; name: string; desc: string; flags: string; options: number };
export type Setting = {
  cvar: string;
  name: string;
  cat: number;
  scope: number;
  value: string;
  values: string;
  labels: string;
  visible: number;
};
export type Addon = { id: string; name: string; desc: string; on: number; loaded: number };
export type AddonSet = { addon: string; cvar: string; name: string; value: string; values: string; labels: string };
export type Player = {
  uid: number;
  name: string;
  team: "T" | "CT" | "SPEC" | string;
  bot: number;
  alive: number;
  hp: number;
  armor: number;
  money: number;
  frags: number;
  deaths: number;
  god: number;
  noclip: number;
  frozen: number;
  host: number;
  /** SteamID (STEAM_0:1:123), or STEAM_ID_LAN / BOT / VALVE_ID_LAN when there isn't one */
  steamid?: string;
};
export type Preset = { id: string; name: string; desc: string; mode: string; map: string; bots: number; user: number };
export type GameEvent = { seq: number; time: number; kind?: string; [k: string]: unknown };

export type Online = {
  online: true;
  state: LiveState;
  modes: Mode[];
  settings: Setting[];
  addons: Addon[];
  addonsets: AddonSet[];
  players: Player[];
  presets: Preset[];
  events: GameEvent[];
  me: { uid?: number; haspos?: number };
};
export type Offline = { online: false; reason: string; game_open?: boolean };
export type GameState = Online | Offline;

export type MapInfo = {
  name: string;
  title?: string;
  cat: string;
  nav: boolean;
  thumb: "shot" | "radar" | "lookup" | null;
  by: string;
  source: "stock" | "download" | "added";
  mod?: number;
  modName?: string;
  pack?: string[];
  fav: boolean;
  hidden: boolean;
  recent: number;
};

export type GbItem = { id: number; name: string; author: string; type: string; image: string | null; likes: number; views: number; added: number; installed: boolean; audio?: string | null };
export type GbSection = [string, string, [number, string][]];
export type GbPage = { items: GbItem[]; total: number; done: boolean; page: number; categories: [number, string][]; section?: string; sections?: GbSection[] };
export type GbFile = {
  id: number; file: string; size: number; downloads: number; added: number; supported: boolean; virus: boolean;
  contents?: string[]; replaces?: string[]; conflicts?: { path: string; mod: number; name: string }[]; listed?: boolean;
};
export type GbDetails = { id: number; name: string; author: string; type: string; text: string; images: string[]; files: GbFile[]; url: string; installed: unknown; section?: string; audio?: string | null; replaces?: string[] };
export type GbJob = { id: string; mod: number; file: number; state: string; message?: string; got?: number; size?: number; maps?: string[]; name?: string };

export type AppInfo = {
  version: string;
  update: { version: string; notes?: string } | null;
  game: { id: string; dir: string; hl: string; dedicated?: boolean; port?: number; installed?: string[]; outdated?: string[]; unmanaged?: string[]; unbuilt?: string[] } | null;
  bots?: number | null;
  lastmap?: string | null;
};

export type PhoneInfo = { enabled: boolean; address?: string; url?: string; qr?: string; error?: string | null };

export type ServerInfo = {
  id: string;
  name: string;
  /** on another machine (a VPS): its address */
  host?: string | null;
  address?: string;
  /** a domain name (or IP, optionally with :port) pointing at it, shown in place of the IP */
  public_address?: string | null;
  /** the address shown and shared: the public address when set, else `address` */
  connect?: string;
  /** on another machine: why it didn't answer last time (it's shown stopped meanwhile) */
  unreachable?: string | null;
  /** What waits for a restart, kept by the app so it survives reloads. Older apps send just
   *  true (new files went up to its machine: used after a restart or map change); newer ones
   *  say what: { files, options, hltv, sounds } (or a list of those words). See restartPending. */
  /** on another machine with a daily restart on: the app does the restart, so only while it's open */
  restart_needs_app?: boolean;
  /** on another machine: its service still has the old start options, written before the next start */
  service_stale?: boolean;
  pending_restart?: boolean | RestartReason[] | Partial<Record<RestartReason, unknown>>;
  listen_ip?: string;
  extra_args?: string;
  /** bots keep playing with nobody on (bot_join_after_player 0) */
  bots_when_empty?: boolean;
  /** the host picked that; otherwise it follows the visibility (on when listed) */
  bots_when_empty_set?: boolean;
  /** on this computer: keeps running after the app closes */
  keep_running?: boolean;
  /** notifications: someone joins; helpers (shared control) make a big change */
  notify_joins?: boolean;
  notify_helpers?: boolean;
  /** Loner-Strike's parts older than this app's ("core" plugins, "foundation" bot brain, mods) */
  outdated?: string[];
  /** those updates going in now; started again after (it was running); waiting its turn */
  updating?: boolean;
  restarts_after?: boolean;
  update_queued?: boolean;
  /** what that update is doing ("stopping", "installing", "machine", "uploading", "starting"...) and its setup job */
  update_phase?: string | null;
  update_job?: string | null;
  /** they go in by themselves once nobody is playing */
  auto_update?: boolean;
  /** on the Loner-Strike server list (Find servers) */
  ls_listed?: boolean;
  /** the join password ("" for none) */
  password?: string;
  /** component ids installed on it */
  installed?: string[];
  game: "cstrike" | "czero";
  port: number;
  visibility: "lan" | "unlisted" | "listed";
  map: string;
  maxplayers: number;
  ready: boolean;
  state: "running" | "stopped" | "crashed" | "setting-up" | "failed" | "checking";
  uptime: number;
  error: string | null;
  imported: boolean;
  loner: boolean;
  dir: string;
  /** a server a friend shared with you (control/): your role there, and whether their app answers */
  shared?: { role: "admin" | "moderator"; reachable: boolean; why?: string };
};
/** What can wait for a restart: new files on the machine, start options, HLTV's settings,
 *  sound changes (used from the next map). */
export type RestartReason = "files" | "options" | "hltv" | "sounds";
export type RestartPending = Record<RestartReason, boolean> & {
  /** the app says what waits (not just "new files"): what it says replaces the page's own guess */
  known: boolean;
};
/** What waits for a restart on a server, from what the app reports (any of its shapes). */
export function restartPending(s?: ServerInfo | null): RestartPending {
  const p = s?.pending_restart;
  const none = { files: false, options: false, hltv: false, sounds: false };
  if (!p) return { ...none, known: false };
  if (p === true) return { ...none, files: true, known: false };
  if (Array.isArray(p)) return { files: p.includes("files"), options: p.includes("options"), hltv: p.includes("hltv"), sounds: p.includes("sounds"), known: true };
  if (typeof p === "object") return { files: !!p.files, options: !!p.options, hltv: !!p.hltv, sounds: !!p.sounds, known: true };
  return { ...none, known: false };
}
/** Anything on a server waits for a restart. */
export const waitsForRestart = (s?: ServerInfo | null) => {
  const r = restartPending(s);
  return r.files || r.options || r.hltv || r.sounds;
};
export type ServerList = { selected: string; local: string | null; servers: ServerInfo[]; steamcmd: boolean };

/** The Admins page's list (GET /api/admins), the parts the Players page uses. */
export type AdminRole = "owner" | "admin" | "moderator" | "vip";
export type AdminPerson = { id: string; name: string; role: AdminRole; expires: string | null; note: string; expired: boolean };
export type SeenPlayer = { id: string; name: string; aka: string[]; first: number; last: number; visits: number };
export type AdminList = { people: AdminPerson[]; seen: SeenPlayer[]; bans: { kind: "id" | "ip"; value: string }[]; today: string; users_ini: boolean };
export type AdminReply = AdminList | { ok: false; error: string };

/** The host name shown for a server: its public address's name, else the VPS's IP (none for a server on this computer without one). */
export const serverHost = (s: ServerInfo) => (s.public_address ? s.public_address.split(":")[0] : s.host) ?? null;

/** The address players connect to, as shown and shared. */
export const serverAddress = (s: ServerInfo) => s.connect ?? s.address ?? `${s.host ?? "127.0.0.1"}:${s.port}`;

/** The bot count asked for: the saved target, or (older plugins) the bots playing. */
export const botTarget = (st: LiveState) => (typeof st.quota === "number" ? st.quota : st.bots);

/** A few words when fewer bots are playing than asked for: "3 of 10 playing", or, with
 *  nobody on and the bots waiting for someone, "Bots join when someone does". */
export function botsNote(st: LiveState): string | null {
  const target = botTarget(st);
  if (st.bots >= target) return null;
  if (st.humans === 0 && st.bots === 0) return "Bots join when someone does";
  return `${st.bots} of ${target} playing`;
}
