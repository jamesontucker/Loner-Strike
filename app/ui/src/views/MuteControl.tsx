// The mute controls, shared by Stats & moderation and the Players page: kept apart from
// the Community page so the Players page doesn't load all of it.

import { useState } from "react";
import { postJson, targetKey, useActions } from "../api";
import { sharedRole } from "../queries";
import { BusyButton, useToast } from "../ui";

export type FilterMode = "off" | "replace" | "block";
export type Settings = { filter: FilterMode; words: string[]; chatlog: boolean; reports: boolean; stats_bots: boolean };
export type Gag = { id: string; name: string; until: number; chat: boolean; voice: boolean };
export type Data = { settings: Settings; gags: Gag[]; now: number; open_reports: number; plugin: boolean; dedicated: boolean; gags_error?: string };
export type Err = { ok: false; error: string };
export const failed = (r: unknown): r is Err => !!r && typeof r === "object" && "error" in r && !!(r as Err).error;
export const OFFLINE: Err = { ok: false, error: "Couldn't reach the app." };

export const STEAMID = /^STEAM_[0-5]:[01]:\d{1,10}$/;
export const MUTE_TIMES: [number, string][] = [[10, "10 minutes"], [30, "30 minutes"], [60, "1 hour"], [1440, "1 day"], [10080, "1 week"], [0, "Until removed"]];
const MUTE_WHAT: [string, string][] = [["chat", "Chat"], ["voice", "Voice"], ["both", "Chat and voice"]];

/** Action keys for the community pages, per game or server. */
export const ck = (what: string) => `community:${targetKey()}:${what}`;

/** Mute someone by SteamID: how long, and chat, voice or both. Bots and LAN players have no SteamID, so they never get these. */
export function MuteControl({ steamid, name, onDone, compact }: { steamid: string; name: string; onDone?: (d: Data) => void; compact?: boolean }) {
  const toast = useToast();
  const [minutes, setMinutes] = useState(30);
  const [what, setWhat] = useState("chat");
  const { run, busy } = useActions();
  if (!STEAMID.test(steamid)) return null;
  const mute = () => run(ck(`mute:${steamid}`), async () => {
    const r = await postJson<Data | Err>("/api/community/gag", { id: steamid, name, minutes, chat: what !== "voice", voice: what !== "chat" }).catch(() => OFFLINE);
    if (failed(r)) return toast(r.error, true);
    toast(minutes ? `${name || steamid} is muted for ${MUTE_TIMES.find((m) => m[0] === minutes)?.[1]}` : `${name || steamid} is muted until you unmute them`);
    onDone?.(r);
  }, `Muting ${name || steamid}…`);
  return (
    <div className={`row cm-mute${compact ? " compact" : ""}`}>
      <select value={minutes} onChange={(e) => setMinutes(Number(e.target.value))} aria-label={`How long to mute ${name}`}>
        {/* a Moderator mutes for up to a week */}
        {MUTE_TIMES.filter(([m]) => m > 0 || sharedRole() !== "moderator").map(([m, l]) => <option key={m} value={m}>{l}</option>)}
      </select>
      <select value={what} onChange={(e) => setWhat(e.target.value)} aria-label="Mute what">
        {MUTE_WHAT.map(([v, l]) => <option key={v} value={v}>{l}</option>)}
      </select>
      <BusyButton busy={busy(ck(`mute:${steamid}`))} label="Mute" busyLabel="Muting…" onClick={mute} />
    </div>
  );
}
