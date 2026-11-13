// Creature Studio's server side (tools/creatures.rs): the game's creatures.ini read the way the
// bot brain reads it, saved back in place; the wave mixes per mode (waves.ini); models and sounds
// with who has them; and "apply to game" on the player's running map.

import { get, post } from "../common/api";

export type Vec3 = [number, number, number];

/** One creature type, as the game would use it. Keys are creatures.ini's (see docs/CREATURES.md). */
export type Creature = {
  name: string;
  base: string;
  display: string;
  category: string;
  hp: number;
  speed: number;
  walk_speed: number;
  aggression: number;
  detect_radius: number;
  detect_fov: number;
  hearing: number;
  attack: string;
  damage: number;
  attack_range: number;
  attack_rate: number;
  attack_delay: number;
  spread: number;
  flee_hp: number;
  behaviour: string;
  radius: number;
  target: string;
  hates: string;
  respawn: number;
  corpse_time: number;
  moves: boolean;
  frags: number;
  money: number;
  model: string;
  skin: number;
  body: number;
  scale: number;
  hull_min: Vec3;
  hull_max: Vec3;
  glow: Vec3;
  fallback_glow: Vec3;
  blood: string;
  /** idle, walk, run, attack, flinch, die ("" = by activity) */
  anim: string[];
  /** idle, alert, attack, hit, miss, pain, die: "a.wav b.wav | c.wav" */
  snd: string[];
  /** enraged below this part of its health (0 = never) */
  enrage_below: number;
  enrage_speed: number;
  enrage_damage: number;
  enrage_anim: number;
  /** tiers 2 and 3: skin / body where the model has one (-1 = its own), health times */
  tier2_skin: number;
  tier2_body: number;
  tier2_hp: number;
  tier3_skin: number;
  tier3_body: number;
  tier3_hp: number;
};

/** How a tier or enraged creature looks, the same on every type (creature.cpp ApplyLook). */
export const LOOKS = {
  tier2: { color: [255, 200, 0] as Vec3, amount: 10, scale: 1 },
  tier3: { color: [255, 70, 10] as Vec3, amount: 14, scale: 1.08 },
  enraged: { color: [255, 0, 0] as Vec3, amount: 20, scale: 1 },
};

export type TypeRow = Creature & { builtin: boolean; inFile: boolean; set: string[]; skipped: boolean; summary: string };
export type Problem = { at: string; field: string; text: string; error: boolean };
export type Limits = { ranges: { key: string; min: number; max: number }[]; categories: string[]; behaviours: string[]; attacks: string[]; targets: string[]; maxTypes: number; maxDisplay: number; maxName: number };
/** A map setting of a placed creature (lonerstrike.fgd's key). */
export type PlacedKey = { name: string; type: string; label: string; default?: string; help?: string; choices?: { value: string; label: string }[] };
/** A creature a mapper places in the map editor (ls_barnacle; turrets as ls_creature) instead of a mode's waves. */
export type Placed = { id: string; entity: string; type: string; title: string; model: string; about: string; how: string; keys: PlacedKey[] };
export type Status = { game: string; file: string; exists: boolean; types: TypeRow[]; templates: Creature[]; problems: Problem[]; limits: Limits; placed?: Placed[] };

export type Candidate = { path: string; found: boolean; folder?: string; kind: "game" | "hl" | "custom" | "missing" | "bad"; note: string };
export type ModelReport = { candidates: Candidate[]; picked: number | null; standIn: boolean };
export type SoundReport = { groups: { files: { file: string; found: boolean }[] }[]; used: number | null };
export type Checked = { problems: Problem[]; summaries: { name: string; summary: string }[]; models?: ModelReport; sounds?: SoundReport[] };

export type Seq = { index: number; name: string; activity: number; fps: number; frames: number };
export type Pick = { seq: number | null; how: "name" | "activity" | "player" | "none"; missing: boolean };
export type ModelLook = { report: ModelReport; model?: { folder: string; path: string }; sequences?: Seq[]; picks?: Pick[]; noRun?: boolean; modelError?: string };

export type ModelItem = { path: string; folder: string; kind: Candidate["kind"]; note: string; group: string; size: number };
export type SoundItem = { file: string; folder: string };

export type Live = { running: boolean; creatures?: boolean; message?: string; types?: { name: string; precached: boolean; standIn: boolean }[]; alive?: number };

export type WaveRow = { from: number; to: number | null; mix: [string, number][] };
export type WaveSet = { mode: string; boss: string; boss_every: number; count_scale: number; hp_scale: number; type_hp: boolean; rows: WaveRow[] };
export type WavesStatus = { file: string; exists: boolean; sets: WaveSet[]; defaults: Record<string, WaveSet>; types: string[]; modes: { id: string; name: string; reads: boolean; note: string }[] };

type Done<S> = { ok?: boolean; changed?: boolean; message?: string; status?: S };

export const creatureStatus = (signal?: AbortSignal) => get<Status>("/api/tools/creatures", signal);
export const checkTypes = (types: Creature[], look?: string) => post<Checked>("/api/tools/creatures/check", { types, look });
export const saveTypes = (types: Creature[]) => post<Done<Status>>("/api/tools/creatures/save", { types });
export const listModels = (signal?: AbortSignal) => get<{ models: ModelItem[] }>("/api/tools/creatures/models", signal);
export const lookModel = (models: string, anims: string[], ranged: boolean, signal?: AbortSignal) =>
  get<ModelLook>(`/api/tools/creatures/model?models=${encodeURIComponent(models)}&anims=${encodeURIComponent(anims.join(","))}&ranged=${ranged}`, signal);
export const listSounds = (signal?: AbortSignal) => get<{ sounds: SoundItem[] }>("/api/tools/creatures/sounds", signal);
export const liveStatus = (signal?: AbortSignal) => get<Live>("/api/tools/creatures/live", signal);
export const applyLive = () => post<{ ok?: boolean; message?: string; reply?: string }>("/api/tools/creatures/reload");
export const spawnOne = (type: string, calm: boolean) => post<{ ok?: boolean; message?: string; reply?: string }>("/api/tools/creatures/spawn", { type, calm });
export const wavesStatus = (signal?: AbortSignal) => get<WavesStatus>("/api/tools/creatures/waves", signal);
export const saveWaves = (w: WaveSet) => post<Done<WavesStatus>>("/api/tools/creatures/waves/save", w);
/** a sound under sound/ (the map editor's player route: game folders only) */
export const soundUrl = (file: string) => `/api/tools/editor/sound?path=${encodeURIComponent(file)}`;
