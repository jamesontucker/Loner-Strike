// The bot people editor's server side (tools/people.rs): people.ini (kinds of people, how
// common each trait is, what never goes together), the regulars' names and who has which.

import { get, post } from "../common/api";

export type Pair = { key: string; value: string };
export type Arch = { name: string; weight: number; letter: string; style: string; fixed: Pair[]; group: string };
export type Weight = { trait: string; weights: number[] };
export type Conflict = { a: Pair; b: Pair };
export type People = { archetypes: Arch[]; weights: Weight[]; conflicts: Conflict[] };
export type Problem = { where: string; text: string; error: boolean };
export type Catalog = { traits: { key: string; values: string[] }[]; hobbies: string[]; letters: { letter: string; desc: string }[] };
export type Regular = { profile: string; name: string };
export type Who = { name: string; arch?: string; archIndex?: number; traits?: Record<string, string>; hobbies?: string[] };

export type Status = {
  game: string;
  people: People;
  problems: Problem[];
  names: string[];
  regulars: Regular[];
  newBeside: boolean;
  canRestore: boolean;
  catalog: Catalog;
};

export type NameEdits = { renamed: [string, string][]; removed: string[]; added: string[] };
type Done = { ok?: boolean; message?: string; live?: boolean; status?: Status };
type Loaded = { people?: People; problems?: Problem[]; cancelled?: boolean };

export const peopleStatus = (signal?: AbortSignal) => get<Status>("/api/tools/people", signal);
export const checkPeople = (people: People, names: string[]) => post<{ problems?: Problem[]; who?: Who[] }>("/api/tools/people/check", { people, names });
export const savePeople = (people: People | null, names: NameEdits, forget: string[]) => post<Done>("/api/tools/people/save", { people, names, forget });
export const restorePeople = () => post<Done>("/api/tools/people/restore");
export const shippedPeople = () => post<Loaded>("/api/tools/people/shipped");
export const exportPeople = (people: People) => post<{ ok?: boolean; message?: string; cancelled?: boolean }>("/api/tools/people/export", { people });
export const importPeople = () => post<Loaded>("/api/tools/people/import");
