// Shapes of what the app's local server returns (same protocol as the Python panel).

export type LiveState = {
  mode: string;
  map: string;
  nav: number;
  career: number;
  navbatch: number;
  navleft: number;
  bots: number;
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
export type Offline = { online: false; reason: string };
export type GameState = Online | Offline;

export type MapInfo = {
  name: string;
  title?: string;
  cat: string;
  nav: boolean;
  thumb: "shot" | "radar" | null;
  by: string;
  fav: boolean;
  hidden: boolean;
  recent: number;
};

export type GbItem = { id: number; name: string; author: string; type: string; image: string | null; likes: number; views: number; added: number; installed: boolean };
export type GbPage = { items: GbItem[]; total: number; done: boolean; page: number; categories: [number, string][] };
export type GbFile = { id: number; file: string; size: number; downloads: number; added: number; supported: boolean; virus: boolean };
export type GbDetails = { id: number; name: string; author: string; type: string; text: string; images: string[]; files: GbFile[]; url: string; installed: unknown };
export type GbJob = { id: string; mod: number; file: number; state: string; message?: string; got?: number; size?: number; maps?: string[]; name?: string };

export type AppInfo = {
  version: string;
  update: { version: string; notes?: string } | null;
  game: { id: string; dir: string } | null;
  bots?: number | null;
  lastmap?: string | null;
};

export type PhoneInfo = { enabled: boolean; address?: string; url?: string; qr?: string; error?: string | null };
