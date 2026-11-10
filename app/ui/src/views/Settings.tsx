import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { currentTarget, getJson, options, postJson, sameValue, targetKey, useActions } from "../api";
import { BOT_STYLE_NOTE, BOT_STYLES } from "../games";
import { reloadAppInfo, useAppInfo } from "../queries";
import { usePlayDrafts } from "../stores/playDraft";
import type { Addon, AddonSet, LiveState, Mode, Online, Setting } from "../types";
import { ModeVoteList } from "./ModeVote";
import { BusyButton, Info, LoadError, SectionHead, Segmented, Skeleton, useCommand, useConfirm, useToast } from "../ui";

// Short explanations under settings, as in the old panel
const NOTES: Record<string, string> = {
  bot_profile_db: "Takes effect on the next map",
  gm_bot_weapons: "What bots buy and use",
  gm_weapons: "Restricts buying and hands out the gun",
  sv_airaccelerate: "Lets players steer in mid-air. Surf maps need it",
  gm_hb_reaction: "Bots wait a human reaction time before shooting",
  gm_hb_aim: "Flicks that overshoot, aim wobble, sprays in bursts",
  gm_hb_mistakes: "The odd whiffed shot, late reaction or panic spray",
  gm_hb_bhop: "Locked in, Hackers and Gods hop between fights",
  gm_regulars: "The same regulars keep coming back, plus the odd newcomer",
  gm_churn: "Now and then someone leaves and a new player takes the slot",
  gm_hb_habits: "Each bot plays like its personality: favourite guns, warming up, getting tired, nervous reloads, a rival",
  gm_hb_tilt: "A bad run makes most bots sloppier (some careful); outnumbered or low on health, some panic",
  gm_hb_afk: "Now and then a bot is a moment slow to move at round start, and the odd regular one stands still a few seconds (not in deathmatch or matches)",
  gm_hb_sprays: "Bots spray the Loner-Strike logo now and then, after respawning in deathmatch too",
  gm_hb_disrespect: "Insults and teabagging (crouch spam over someone they killed). Stays off until you turn it on, and a Friendly lobby is never rude even then",
  gm_mood: "How the lobby gets along. Toxic brings rage, tilt and hack accusations, and with Bots can be rude on, trash talk and teabagging too. Friendly is warm and polite",
  gm_hb_showoff: "Now and then after a kill: a jump or the knife out, and crouch spam if bots can be rude (never in a match)",
  gm_bf_weapons: "Flicking between weapons, the silencer off and on, the scope in and out, reloading a nearly full gun",
  gm_bf_spawn: "Jumping out of spawn (teammates sometimes join in), and in freeze time jumping on the spot, the knife out and back, weapon flicking",
  gm_bf_flashlight: "The flashlight on and off a few times, where the server allows flashlights",
  gm_bf_salute: "Looking up and down at a teammate in freeze time, the classic gun salute",
  gm_bf_look: "Stopping for a moment to look around or glance down, and good aimers flicking between teammates' heads in freeze time",
  gm_bf_celebrate: "A few of the winners shoot into the air when the round is won",
  gm_bf_breakables: "Shooting the glass in view after the round, and a few slash the same crate every time they walk past",
  gm_bf_drop: "A well-off bot sometimes drops its gun for a broke teammate who asks in chat, like drop pls, and broke bots sometimes ask",
  gm_bf_bad: "Newer players reload after every kill, flick weapons for no reason, walk about with the knife out and pull a grenade out of spawn",
  gm_bf_cheeky: "On public games a few jokers slash a teammate on the way out of spawn (only with friendly fire off) and jump about after the round (and teabag a kill, with Bots can be rude on)",
  gm_hb_radio: "The odd radio call that fits the moment: enemy spotted, enemy down, the team's caller says go",
  gm_hb_hold: "Careful players and lurkers stop now and then and hold an angle for a few seconds (not in deathmatch)",
  gm_botchat: "How often bots talk: kills, deaths, rage, callouts like rush b, encouraging teammates, answering you",
  gm_botchat_humans: "Bots answer what people type in chat and do what they're asked, each by its personality",
  gm_botchat_stop: "Now and then a bot typing a long line stands still for the last few seconds of it",
  gm_botchat_deadchat: "In CS the living can't see what the dead say; like old pub servers, this shows dead bots' chat to everyone",
  gm_clans: "Bots with the same clan tag join together and play on the same side",
  gm_botjoin: "When a map starts, bots join a few at a time instead of all at once",
  gm_botvotes: "Bots join rock the vote and vote for maps; people's votes still decide",
  gm_botping: "Bots show a believable ping on the scoreboard instead of BOT",
  gm_rtv: "Say rtv in chat to vote for a new map, nominate <map> to suggest one",
  gm_welcome: "Shown when you or a friend joins. Say /welcome to see it again",
  gm_atmo_adverts: "Every few minutes a line in chat with the server's rules and tips, like pub servers ran. The lines are in adverts.ini",
  gm_atmo_spec: "Now and then a bot sits a few rounds out in spectator, then joins again",
  mp_autoteambalance: "Moves players at round end to keep the teams even",
  mp_limitteams: "How many more players one team can have before joining it is blocked",
  mp_respawn_immunitytime: "Players can't be hurt for a moment after they spawn; good with deathmatch",
  mp_forcerespawn: "Everyone, bots too, comes back this long after dying instead of waiting for the next round. Pair it with Spawn protection",
  gmdm_random_spawns: "Random spreads spawns over the whole map, from the bots' map knowledge. On a map the bots are still learning, team spawns are used until they finish, then random ones take over",
  gg_dm_spawn_random: "Random spreads spawns over the whole map, from the bots' map knowledge. On a map the bots are still learning, team spawns are used until they finish, then random ones take over",
};
// A "?" for settings whose name and choices don't explain them and that have no note
const INFO: Record<string, string> = {
  bot_quota_mode: "Fixed keeps exactly the number of bots you set. Fill server counts people too, so a bot leaves each time someone joins. Per human adds that many bots for each person playing.",
  bot_chatter: "The bots' voice lines, like calling out enemies. Minimal keeps only the important ones. Radio keeps only the standard radio messages.",
  bot_stop: "Every bot stops dead where it is until you turn this off. Handy for looking around a map or lining up a shot.",
  bot_defer_to_human: "When a person is on their team, bots leave the objective to them: planting, defusing and rescuing hostages.",
  bot_walk: "Bots never run, so they're slower and quieter. Good for practising.",
  bot_allow_rogues: "Now and then a bot ignores radio commands and the objective and goes off on its own, like a real pub player. None keeps every bot on task.",
  mp_freezetime: "Seconds at the start of each round when nobody can move, for buying.",
  mp_round_infinite: "Rounds never end on their own, not when a team is wiped out and not when the time runs out. Useful for practice.",
  mp_weapons_allow_map_placed: "Guns some maps leave lying on the ground, like on aim_ and fy_ maps. Remove takes them away.",
  wc3_races: "Chameleon is a ninth race whose skills are picked at random from the other races.",
  wc3_ult_cooldown: "How long before a player can use their ultimate, their race's strongest power, again.",
  zp_starting_ammo_packs: "Ammo packs are Zombie Plague's money: players spend them on extra items and weapons.",
  zp_nem_enabled: "Rounds where one zombie, the Nemesis, has huge health and takes on all the humans.",
  zp_surv_enabled: "Rounds where one human, the Survivor, gets a powerful gun and lots of health against all the zombies.",
  bb_build_time: "How long the builders have to put up barricades before the zombies are let out.",
  gm_modevote: "Players type rtv or rtm in chat to vote for a different mode. If it passes they pick the mode, then a map for it.",
  gm_modevote_every: "Every few maps, the vote near the end of the map picks the mode first, then a map for it. Needs mode voting on.",
  sh_maxbinds: "How many heroes with a power you trigger with a key each player can pick. The rest of their heroes work on their own.",
};

/** Settings the page shows as its own slider instead of in the list (Bots: Personality). */
export const SLIDER_SETTINGS = ["gm_personality"];
const listed = (s: Setting) => s.visible && !SLIDER_SETTINGS.includes(s.cvar);

/** The game's scope for a setting saved per mode (0: not saved, 1: every mode, 2: this mode). */
const SCOPE_MODE = 2;
const SCOPE_EVERY = 1;
const modeName = (data: Online) => data.modes.find((m) => m.id === data.state.mode)?.name ?? "this mode";
/** Whether every listed setting of a section is saved per mode, some are, or none. */
function scopeOf(data: Online, cat: number): "all" | "some" | "none" {
  const all = data.settings.filter((s) => s.cat === cat && listed(s));
  const per = all.filter((s) => s.scope === SCOPE_MODE).length;
  return !per ? "none" : per === all.length ? "all" : "some";
}
/** The page note's first part, the same on your game and on a server: where changes are saved. */
export function savedNote(data: Online, cat: number) {
  const scope = scopeOf(data, cat);
  const mode = modeName(data);
  return scope === "all" ? `Saved for ${mode}.` : scope === "some" ? `Settings marked ${mode} are saved for that mode only; the rest for every mode.` : "";
}
/** Rules and Fun & effects: the same words on your game and on servers. */
export const RULES_NOTE = (data: Online) => `${savedNote(data, 2)} Round changes take effect next round.`.trim();
export const FUN_NOTE = (data: Online) => `${savedNote(data, 3)} Most apply straight away. Round changes take effect next round.`.trim();

/** How long a picked value shows before the game's own answer wins regardless. */
const PENDING_MS = 3000;

/** Values just picked, per setting: shown instead of the polled one until the game
 *  reports the same value (or a few seconds pass), so a control doesn't jump back to
 *  the old value between the save and the next poll. */
export function usePending() {
  const picked = useRef(new Map<string, { v: string; at: number }>());
  const [, tick] = useState(0);
  const set = useCallback((k: string, v: string) => {
    picked.current.set(k, { v, at: Date.now() });
    tick((n) => n + 1);
    setTimeout(() => tick((n) => n + 1), PENDING_MS + 50);
  }, []);
  const clear = useCallback((k: string) => {
    picked.current.delete(k);
    tick((n) => n + 1);
  }, []);
  const value = (k: string, polled: string) => {
    const p = picked.current.get(k);
    if (!p) return polled;
    if (sameValue(polled, p.v) || Date.now() - p.at > PENDING_MS) {
      picked.current.delete(k);
      return polled;
    }
    return p.v;
  };
  return { set, clear, value };
}

/** The Rules page's tabs: the game's general settings by subject; anything else belongs to
 *  the mode (GunGame's weapon order, Zombie Plague's zombies, ...) and gets its own tab. */
const RULE_TABS: [string, string, string[]][] = [
  ["rounds", "Rounds & time", ["mp_roundtime", "mp_freezetime", "mp_timelimit", "mp_maxrounds", "mp_round_infinite", "mp_c4timer", "gm_nobomb"]],
  ["teams", "Teams", ["mp_friendlyfire", "mp_autoteambalance", "mp_limitteams"]],
  ["money", "Money & weapons", ["mp_startmoney", "mp_buytime", "mp_buy_anywhere", "mp_free_armor", "mp_auto_reload_weapons", "mp_weapons_allow_map_placed"]],
  ["players", "Players", ["mp_forcerespawn", "mp_respawn_immunitytime", "mp_falldamage", "mp_footsteps", "mp_scoreboard_showhealth", "mp_scoreboard_showmoney", "gm_welcome"]],
  ["voting", "Voting", ["gm_rtv", "gm_modevote", "gm_modevote_every"]],
];
const ruleTab = (cvar: string) => RULE_TABS.find(([, , cvars]) => cvars.includes(cvar))?.[0] ?? "mode";

// ---- editing a mode that isn't being played -----------------------------------------
// Live, the game itself says which settings a mode has (its plugins' cvars exist). For
// another mode the page uses what that mode showed the last time it was played, and for
// one it hasn't seen yet these rules (the same ones the Game Mode Menu applies).
const GG = ["gungame", "gungame_team"];
const DM = ["csdm", "csdm_ffa"];
const FAMILIES: [string, string[]][] = [
  ["gm_gg_", GG], ["gg_", GG], ["gmdm_", DM], ["gm_zp_", ["zombie", "zp5"]], ["zp_", ["zombie", "zp5"]],
  ["wc3_", ["war3"]], ["sh_", ["shero"]], ["hns_", ["hns"]], ["pbmoves_", ["paintball"]], ["kz_", ["kreedz"]],
  ["deathrun_", ["deathrun"]], ["dr_", ["deathrun"]], ["PBR_", ["paintball"]], ["bb_", ["basebuilder"]], ["sj_", ["soccerjam"]],
];
/** Settings only for modes with a flag in modes.ini (the Game Mode Menu's SD_NeedFlag). */
const NEEDS_FLAG: Record<string, string> = { gm_weapons: "w", gm_gg_order: "g", gm_zp_specials: "z", mp_forcerespawn: "w", gm_autorespawn: "w" };
/** General settings a mode replaces with its own (gamemodes.sma's setting_visible). */
const REPLACED_IN: Record<string, string[]> = { gm_nobomb: [...GG, ...DM], mp_respawn_immunitytime: [...GG, ...DM] };

/** Per game or server: servers can have different mods. */
const seenKey = (mode: string) => `ls.modeSettings.${targetKey()}.${mode}`;
function seenSettings(mode: string): string[] | null {
  try {
    const v = JSON.parse(localStorage.getItem(seenKey(mode)) ?? "null");
    return Array.isArray(v) ? v : null;
  } catch {
    return null;
  }
}
/** Remembers which settings the mode being played shows, for editing it later from another. */
function rememberSettings(data: Online) {
  try {
    const list = data.settings.filter((s) => s.visible && s.scope === SCOPE_MODE).map((s) => s.cvar).sort();
    if (list.length) localStorage.setItem(seenKey(data.state.mode), JSON.stringify(list));
  } catch {
    /* no storage: the rules below still work */
  }
}
function belongsTo(s: Setting, mode: Mode): boolean {
  const seen = seenSettings(mode.id);
  if (seen) return seen.includes(s.cvar);
  if (NEEDS_FLAG[s.cvar]) return mode.flags.includes(NEEDS_FLAG[s.cvar]);
  const fam = FAMILIES.find(([prefix]) => s.cvar.startsWith(prefix));
  if (fam) return fam[1].includes(mode.id);
  return !REPLACED_IN[s.cvar]?.includes(mode.id);
}

/** Another mode's saved settings, for the Rules page's mode picker. */
type Editing = { mode: Mode; values: Record<string, string | null>; save: (cvar: string, value: string) => Promise<boolean> };

// ---- settings for the next game, without a game running -------------------------------
// The pages edit the files the game applies when it starts (saved.rs). The page gets the
// same shape as a game's state, with what a running game would show worked out here.

/** What saved.rs answers. */
type SavedView = { mode: string; map: string; bots: number | null; modes: Mode[]; settings: Setting[]; addons: Addon[]; addonsets: AddonSet[]; foundation: boolean; error?: string };
type SavedReply = { ok?: boolean; error?: string };

/** Changing the next game's settings instead of a running one's. */
export type Saved = {
  /** the mode the per-mode settings are for: the one the next game starts in, or one picked */
  mode: string;
  setMode: (id: string | null) => void;
  save: (cvar: string, value: string) => Promise<boolean>;
  setBots: (n: number) => Promise<boolean>;
  addon: (body: { id: string; on?: boolean; cvar?: string; value?: string }) => Promise<boolean>;
  reset: (what: "shared" | "mode") => Promise<boolean>;
};

/** The note on every page without a game: where the changes go. */
export const SAVED_NOTE = "Changes apply when you start a game.";

/** Settings that need Loner-Strike's game DLL (its ls_bot), as gamemodes.sma's setting_visible. */
const NEEDS_LS_DLL = ["gm_bf_salute", "gm_bf_look", "gm_bf_celebrate", "gm_botchat_stop"];
/** Whether a setting shows for the next game: setting_visible's rules, without a game to ask. */
function savedVisible(s: Setting, mode: Mode | undefined, all: Setting[], foundation: boolean): boolean {
  if (s.scope === SCOPE_MODE) return !mode || belongsTo(s, mode);
  if (NEEDS_LS_DLL.includes(s.cvar) && !foundation) return false;
  const style = Number(all.find((x) => x.cvar === "gm_botstyle")?.value || "1");
  // the style's own personalities replace the roster; its switches only matter with a style on
  if (s.cvar === "bot_profile_db") return style === 0;
  if ((/^gm_(hb|bf)_/.test(s.cvar) || s.cvar === "gm_personality" || s.cvar === "gm_botchat_stop") && style === 0) return false;
  // only Locked In, Hackers, Gods and Mixed lobby's Locked In players hop
  if (s.cvar === "gm_hb_bhop") return style >= 5 && style <= 8;
  return true;
}

/** A game's state for pages that need one, from what the next game starts with. */
function savedState(v: SavedView): LiveState {
  // (no count saved yet: what Play starts a first game with)
  const bots = v.bots ?? 9;
  return {
    mode: v.mode, map: v.map, nav: 1, career: 0, navbatch: 0, navleft: 0, bots, quota: bots, humans: 0, maxplayers: 32, timeleft: -1, presets: 0,
    addons_pending: 0, version: "", crashmap: "", preset: "", peaceful: 0, paused: 0, recording: 0, timelimit: 0,
  };
}

/** Keeps Play's Start a game form in step with a change made here, so starting doesn't put
 *  back what the form had (it sends what was picked on it). Your own game only. */
function followOnPlay(gameId: string | undefined, cvar: string, value: string, otherMode: boolean) {
  if (!gameId || currentTarget()) return;
  const { drafts, set } = usePlayDrafts.getState();
  const d = drafts[gameId];
  if (!d) return;
  if (cvar === "bot_quota") return set(gameId, { bots: Number(value) });
  // (a mode's own setting changed for a mode other than the one the form starts in: not the form's)
  if (!(cvar in d.changed) || otherMode) return;
  set(gameId, { changed: { ...d.changed, [cvar]: value } });
}

/** The next game's settings for the pages to edit, and how to change them. */
export function useSaved() {
  const toast = useToast();
  const qc = useQueryClient();
  const gameId = useAppInfo().data?.game?.id;
  // your own game starts in the mode on Play's form, a server in the one it ran last
  const draftMode = usePlayDrafts((s) => (gameId && !currentTarget() ? s.drafts[gameId]?.mode : undefined));
  const [picked, setPicked] = useState<string | null>(null);
  const want = picked ?? draftMode ?? "";
  const q = useQuery({
    queryKey: ["savedSettings", targetKey(), want],
    queryFn: () => getJson<SavedView>(`/api/app/saved-settings${want ? `?mode=${encodeURIComponent(want)}` : ""}`),
    // another mode picked: the page stays while its settings load
    placeholderData: (prev) => prev,
  });
  const v = q.data && !q.data.error && Array.isArray(q.data.settings) ? q.data : null;
  const send = async (path: string, body: unknown) => {
    const r = await postJson<SavedReply>(path, body).catch((): SavedReply => ({ error: "Couldn't reach the app." }));
    if (r.error) toast(r.error, true);
    await Promise.all([qc.invalidateQueries({ queryKey: ["savedSettings", targetKey()] }), qc.invalidateQueries({ queryKey: ["startOptions"] }), qc.invalidateQueries({ queryKey: ["modeSettings", targetKey()] })]);
    return !r.error;
  };
  // (while a picked mode loads, the old one's view stays on screen: edits go to the picked one)
  const mode = q.isPlaceholderData ? want : v?.mode ?? want;
  const data: Online | null = v && {
    online: true,
    state: savedState(v),
    modes: v.modes,
    settings: v.settings.map((s) => ({ ...s, visible: savedVisible(s, v.modes.find((m) => m.id === v.mode), v.settings, v.foundation) ? 1 : 0 })),
    addons: v.addons,
    addonsets: v.addonsets,
    players: [],
    presets: [],
    events: [],
    me: {},
  };
  const saved: Saved = {
    mode,
    setMode: setPicked,
    save: async (cvar, value) => {
      const ok = await send("/api/app/saved-settings", { mode, cvar, value });
      // (the form starts in its own mode, or without one the mode the game ran last)
      const otherMode = v?.settings.find((s) => s.cvar === cvar)?.scope === SCOPE_MODE && (draftMode ? draftMode !== mode : picked !== null);
      if (ok) followOnPlay(gameId, cvar, value, otherMode);
      return ok;
    },
    setBots: async (n) => {
      const ok = await send("/api/app/saved-settings/bots", { bots: n });
      if (ok) {
        followOnPlay(gameId, "bot_quota", String(n), false);
        reloadAppInfo();
      }
      return ok;
    },
    addon: (body) => send("/api/app/saved-settings/addon", body),
    reset: (what) => send("/api/app/saved-settings/reset", { what, mode }),
  };
  return { q, error: q.data?.error, data, saved };
}

/** A page for the next game: loads what it starts with, then shows the page with it. */
export function WithSaved({ title, children }: { title: string; children: (data: Online, saved: Saved) => ReactNode }) {
  const { q, error, data, saved } = useSaved();
  if (data) return <>{children(data, saved)}</>;
  return (
    <div className="stack">
      <SectionHead title={title} note={SAVED_NOTE} />
      {error ? <p className="muted">{error}</p> : q.isError ? <LoadError text="Couldn't load the saved settings." onRetry={() => q.refetch()} /> : <Skeleton rows={4} />}
    </div>
  );
}

export function SettingsList({ data, cat, refresh, q = "", tab, editing, saved }: { data: Online; cat: number; refresh: () => void; q?: string; tab?: string; editing?: Editing; saved?: Saved }) {
  const run = useCommand(refresh);
  const { run: act, busy } = useActions();
  const key = (cvar: string) => `settings:${targetKey()}:${saved ? `saved:${saved.mode}` : editing ? editing.mode.id : "live"}:${cvar}`;
  const pending = usePending();
  const pick = (s: Setting, v: string) => act(key(s.cvar), async () => {
    pending.set(s.cvar, v);
    // no game: into the next game's files; another mode's own settings go to its saved
    // file; settings shared by every mode change live
    const ok = saved ? await saved.save(s.cvar, v) : editing && s.scope === SCOPE_MODE ? await editing.save(s.cvar, v) : await run("gm_set", [s.cvar, v], true);
    if (!ok) pending.clear(s.cvar);
  }, `Changing ${s.name}${editing ? ` for ${editing.mode.name}` : ""}…`);
  const all = editing
    ? // another mode: its own saved settings, and the ones every mode shares (as they are now)
      data.settings.filter((s) => s.cat === cat && !SLIDER_SETTINGS.includes(s.cvar) && (s.scope === SCOPE_MODE ? belongsTo(s, editing.mode) : s.scope === SCOPE_EVERY && listed(s)))
    : data.settings.filter((s) => s.cat === cat && listed(s));
  const words = q.trim().toLowerCase().split(/\s+/).filter(Boolean);
  // a search looks through every tab
  const list = all.filter((s) => words.length > 0 || !tab || ruleTab(s.cvar) === tab).filter((s) => {
    const text = `${s.name} ${NOTES[s.cvar] ?? ""} ${s.labels}`.toLowerCase();
    return words.every((w) => text.includes(w));
  });
  // with some settings per mode and some not, the per-mode ones carry the mode's name
  const markMode = !editing && scopeOf(data, cat) === "some";
  const mode = modeName(data);
  if (!all.length) return <p className="muted">Nothing to set here in this mode.</p>;
  // a tab whose only content is below the list (Voting's mode list)
  if (tab && !words.length && !list.length) return null;
  if (!list.length) return <p className="muted">No settings match "{q.trim()}".</p>;
  return (
    <div className="settings">
      {list.map((s: Setting) => {
        const value = pending.value(s.cvar, editing && s.scope === SCOPE_MODE ? (editing.values[s.cvar] ?? "") : s.value);
        const style = s.cvar === "gm_botstyle";
        // with Personality at Plain the bots hardly talk, so the lobby's mood has nothing to show in
        const personality = data.settings.find((x) => x.cvar === "gm_personality");
        const moodOff = s.cvar === "gm_mood" && !!personality && Number(pending.value("gm_personality", personality.value)) === 0;
        const note = moodOff ? "Personality is set to Plain, so bots hardly talk and the mood doesn't show" : style ? BOT_STYLE_NOTE[value] : NOTES[s.cvar];
        // our own names for the bot styles (the game's menu has older ones)
        const labels = style ? options(s.values, s.labels).map(([v, l]) => BOT_STYLES.find(([id]) => id === v)?.[1] ?? l).join("|") : s.labels;
        return (
          <div className="setting" key={s.cvar}>
            <div className="name">
              {s.name}
              {INFO[s.cvar] && !NOTES[s.cvar] && <Info text={INFO[s.cvar]} />}
              {markMode && s.scope === SCOPE_MODE && <span className="tag scope-tag" title={`Saved for ${mode} only`}>{mode}</span>}
              {editing && s.scope === SCOPE_EVERY && <span className="tag scope-tag" title="The same in every mode: changing it here changes it everywhere, straight away">Every mode</span>}
              {note && <span className="note">{note}</span>}
            </div>
            <Segmented label={s.name} values={s.values} labels={labels} current={value} disabled={busy(key(s.cvar)) || moodOff} onPick={(v) => pick(s, v)} />
          </div>
        );
      })}
    </div>
  );
}

export function SettingsPage({
  title,
  note,
  data,
  cat,
  refresh,
  children,
  pickMode = false,
  saved,
}: {
  title: string;
  note: string;
  data: Online;
  cat: number;
  refresh: () => void;
  children?: ReactNode;
  /** a picker for editing another mode's saved settings */
  pickMode?: boolean;
  /** no game running: the next game's settings */
  saved?: Saved;
}) {
  const [q, setQ] = useState("");
  const [picked, setPicked] = useState<string | null>(null);
  // the mode being edited: the one being played unless another is picked
  const [editId, setEditId] = useState<string | null>(null);
  const toast = useToast();
  const qc = useQueryClient();
  const playing = data.state.mode;
  // (what the next game would show is worked out here, so only a game's own list is kept)
  useEffect(() => {
    if (!saved) rememberSettings(data);
  }, [data, saved]);
  const other = !saved && pickMode && editId && editId !== playing ? data.modes.find((m) => m.id === editId) : undefined;
  const otherCvars = other ? data.settings.filter((s) => s.cat === cat && s.scope === SCOPE_MODE && belongsTo(s, other)).map((s) => s.cvar) : [];
  const savedQ = useQuery({
    queryKey: ["modeSettings", targetKey(), other?.id, cat],
    queryFn: () => getJson<{ values?: Record<string, string | null>; error?: string }>(`/api/app/mode-settings?mode=${encodeURIComponent(other!.id)}&cvars=${otherCvars.join(",")}`),
    enabled: !!other,
  });
  const editing: Editing | undefined = other && {
    mode: other,
    values: savedQ.data?.values ?? {},
    save: async (cvar, value) => {
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/app/mode-settings", { mode: other.id, cvar, value }).catch(() => ({ error: "Couldn't reach the app." }) as { ok?: boolean; error?: string });
      if (r.error) toast(r.error, true);
      await qc.invalidateQueries({ queryKey: ["modeSettings", targetKey(), other.id] });
      return !r.error;
    },
  };
  const shown = editing
    ? data.settings.filter((s) => s.cat === cat && (otherCvars.includes(s.cvar) || (s.scope === SCOPE_EVERY && listed(s))))
    : data.settings.filter((s) => s.cat === cat && listed(s));
  const count = shown.length;
  // Rules: tabs by subject, the mode's own settings first; tabs with nothing to set are left out
  const tabs: [string, string][] =
    cat === 2
      ? ([["mode", other ? other.name : modeName(data)], ...RULE_TABS.map(([id, label]) => [id, label])] as [string, string][]).filter(
          ([id]) => (id === "voting" && !editing) || shown.some((s) => ruleTab(s.cvar) === id),
        )
      : [];
  const tab = tabs.length ? (tabs.find(([id]) => id === picked) ?? tabs[0])[0] : undefined;
  const searching = q.trim().length > 0;
  return (
    <div className="stack">
      <div>
        <SectionHead title={title} note={editing ? `Saved for ${editing.mode.name}: used the next time it starts. Ones marked "Every mode" are shared by all modes and change straight away.` : saved ? `${SAVED_NOTE} ${note}`.trim() : note} />
        {children}
      </div>
      <div className="stack-tight">
        {pickMode && data.modes.length > 1 && (
          <label className="sort mode-pick">
            <span>Settings for</span>
            <select
              value={other ? other.id : playing}
              onChange={(e) => {
                if (saved) saved.setMode(e.target.value);
                else setEditId(e.target.value === playing ? null : e.target.value);
                setPicked(null);
              }}
            >
              {[...data.modes].sort((a, b) => a.name.localeCompare(b.name)).map((m) => (
                <option key={m.id} value={m.id}>{m.name}{m.id === playing && !saved ? " (playing now)" : ""}</option>
              ))}
            </select>
          </label>
        )}
        {count > 6 && (
          <input className="search full" type="search" placeholder={`Filter ${count} settings, like "round" or "gravity"`} value={q} onChange={(e) => setQ(e.target.value)} aria-label="Filter settings" />
        )}
        {tabs.length > 1 && !searching && (
          <div className="chips rule-tabs" role="tablist" aria-label={`${title} sections`}>
            {tabs.map(([id, label]) => (
              <button key={id} role="tab" className="chip" aria-selected={tab === id} onClick={() => setPicked(id)}>
                {label}
              </button>
            ))}
          </div>
        )}
        {editing && savedQ.data?.error ? (
          <p className="muted">{savedQ.data.error}</p>
        ) : editing && savedQ.isLoading ? (
          <p className="muted"><span className="spinner" aria-hidden="true" />Loading {editing.mode.name}'s settings…</p>
        ) : (
          <SettingsList data={data} cat={cat} refresh={refresh} q={q} tab={tab} editing={editing} saved={saved} />
        )}
        {cat === 2 && !editing && (tab === "voting" || searching) && <ModeVoteList data={data} />}
      </div>
    </div>
  );
}

/** Rules and Fun & effects with no game running: the next game's settings. */
export function SavedSettingsPage({ cat }: { cat: 2 | 3 }) {
  const title = cat === 2 ? "Rules" : "Fun & effects";
  return (
    <WithSaved title={title}>
      {(data, saved) => (
        <div className="stack">
          {/* where the round buttons are with a game: what they need (as Bots says for its buttons) */}
          <SettingsPage title={title} note={savedNote(data, cat)} data={data} cat={cat} refresh={() => undefined} pickMode saved={saved}>
            {cat === 2 && <p className="muted small-note">Restarting or ending the round, adding time and mode votes work once a game is running.</p>}
          </SettingsPage>
          {/* the reset, last and in its own panel, as Bots' and Mods' are */}
          {cat === 2 && <MatchButtons refresh={() => undefined} data={data} saved={saved} />}
        </div>
      )}
    </WithSaved>
  );
}

/** Restart / end the round / more time, for the Rules page. Without a game, only the reset (a panel at the page's foot). */
export function MatchButtons({ refresh, data, saved }: { refresh: () => void; data?: Online; saved?: Saved }) {
  const mode = data ? modeName(data) : "this mode";
  const ask = useConfirm();
  const run = useCommand(refresh);
  const { run: act, busy } = useActions();
  const key = (what: string) => `match:${targetKey()}:${what}`;
  const resetAsk = `Reset ${mode}'s own settings to their defaults? That includes its rules, fun settings and its own bot settings. Settings shared by every mode stay.`;
  if (saved) {
    return (
      <div className="panel row between">
        <div>
          <b>Reset {mode}'s settings</b>
          <p className="muted">Its own rules, fun settings and bot settings go back to their defaults. Settings shared by every mode stay.</p>
        </div>
        <BusyButton
          className="btn danger"
          busy={busy(key(`reset:${saved.mode}`))}
          label="Reset"
          busyLabel="Resetting…"
          onClick={async () => (await ask(resetAsk, { ok: "Reset", danger: true })) && act(key(`reset:${saved.mode}`), () => saved.reset("mode"), `Resetting ${mode}'s settings…`)}
        />
      </div>
    );
  }
  return (
    <div className="row">
      <BusyButton className="btn primary" busy={busy(key("restart"))} label="Restart round" busyLabel="Restarting…" onClick={() => act(key("restart"), () => run("gm_match", ["restart"]), "Restarting the round…")} />
      <BusyButton busy={busy(key("endround"))} label="End round" busyLabel="Ending…" onClick={() => act(key("endround"), () => run("gm_do", ["endround"]), "Ending the round…")} />
      <BusyButton busy={busy(key("addtime"))} label="Add 10 minutes" busyLabel="Adding…" onClick={() => act(key("addtime"), () => run("gm_do", ["addtime"]), "Adding 10 minutes…")} />
      <BusyButton busy={busy(key("modevote"))} label="Start a mode vote" busyLabel="Starting…" onClick={() => act(key("modevote"), () => run("gm_votemode"), "Starting a mode vote…")} />
      <BusyButton
        className="btn danger"
        busy={busy(key("reset"))}
        label="Reset this mode's settings"
        busyLabel="Resetting…"
        onClick={async () => (await ask(resetAsk, { ok: "Reset", danger: true })) && act(key("reset"), () => run("gm_match", ["reset"]), "Resetting this mode's settings…")}
      />
    </div>
  );
}
