import { useCallback, useRef, useState, type ReactNode } from "react";
import { options, sameValue, targetKey, useActions } from "../api";
import { BOT_STYLE_NOTE, BOT_STYLES } from "../games";
import type { Online, Setting } from "../types";
import { ModeVoteList } from "./ModeVote";
import { BusyButton, Info, SectionHead, Segmented, useCommand, useConfirm } from "../ui";

// Short explanations under settings, as in the old panel
const NOTES: Record<string, string> = {
  bot_profile_db: "Takes effect on the next map",
  gm_bot_weapons: "What bots buy and use",
  gm_weapons: "Restricts buying and hands out the gun",
  sv_airaccelerate: "Surf / bhop for surf maps",
  gm_hb_reaction: "Bots wait a human reaction time before shooting",
  gm_hb_aim: "Flicks that overshoot, aim wobble, sprays in bursts",
  gm_hb_mistakes: "The odd whiffed shot, late reaction or panic spray",
  gm_hb_bhop: "Locked in, Hackers and Gods hop between fights",
  gm_regulars: "The same regulars keep coming back, plus the odd newcomer",
  gm_churn: "Now and then someone leaves and a new player takes the slot",
  gm_hb_habits: "Each bot plays like its personality: favourite guns, warming up, getting tired, nervous reloads, a rival",
  gm_hb_tilt: "A bad run makes most bots sloppier (some careful); outnumbered or low on health, some panic",
  gm_hb_showoff: "Now and then after a kill: crouch spam, a jump or the knife out (never in a match)",
  gm_hb_afk: "Now and then a bot stands still for a few seconds at round start",
  gm_hb_sprays: "Bots spray a logo now and then",
  gm_hb_radio: "The odd radio call that fits the moment: enemy spotted, enemy down, the team's caller says go",
  gm_hb_hold: "Careful players and lurkers stop now and then and hold an angle for a few seconds",
  gm_botchat: "How often bots talk: kills, deaths, rage, encouraging teammates, answering you",
  gm_botchat_humans: "Bots answer what people type, and the one you name answers you",
  gm_botchat_deadchat: "In CS the living can't see what the dead say; like old pub servers, this shows dead bots' chat to everyone",
  gm_clans: "Bots with the same clan tag join together and play on the same side",
  gm_botjoin: "When a map starts, bots join a few at a time instead of all at once",
  gm_botvotes: "Bots join rock the vote and vote for maps; people's votes still decide",
  gm_botping: "Bots show a believable ping on the scoreboard instead of BOT",
  gm_rtv: "Say rtv in chat to vote for a new map, nominate <map> to suggest one",
  gm_welcome: "Shown when you or a friend joins. Say /welcome to see it again",
  mp_autoteambalance: "Moves players at round end to keep the teams even",
  mp_limitteams: "How many more players one team can have before joining it is blocked",
  mp_respawn_immunitytime: "Players can't be hurt for a moment after they spawn; good with deathmatch",
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

export function SettingsList({ data, cat, refresh, q = "" }: { data: Online; cat: number; refresh: () => void; q?: string }) {
  const run = useCommand(refresh);
  const { run: act, busy } = useActions();
  const key = (cvar: string) => `settings:${targetKey()}:${cvar}`;
  const pending = usePending();
  const pick = (s: Setting, v: string) => act(key(s.cvar), async () => {
    pending.set(s.cvar, v);
    if (!(await run("gm_set", [s.cvar, v], true))) pending.clear(s.cvar);
  }, `Changing ${s.name}…`);
  const all = data.settings.filter((s) => s.cat === cat && listed(s));
  const words = q.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const list = all.filter((s) => {
    const text = `${s.name} ${NOTES[s.cvar] ?? ""} ${s.labels}`.toLowerCase();
    return words.every((w) => text.includes(w));
  });
  if (!all.length) return <p className="muted">Nothing to set here in this mode.</p>;
  if (!list.length) return <p className="muted">No settings match "{q.trim()}".</p>;
  return (
    <div className="settings">
      {list.map((s: Setting) => {
        const value = pending.value(s.cvar, s.value);
        const style = s.cvar === "gm_botstyle";
        const note = style ? BOT_STYLE_NOTE[value] : NOTES[s.cvar];
        // our own names for the bot styles (the game's menu has older ones)
        const labels = style ? options(s.values, s.labels).map(([v, l]) => BOT_STYLES.find(([id]) => id === v)?.[1] ?? l).join("|") : s.labels;
        return (
          <div className="setting" key={s.cvar}>
            <div className="name">
              {s.name}
              {INFO[s.cvar] && !NOTES[s.cvar] && <Info text={INFO[s.cvar]} />}
              {note && <span className="note">{note}</span>}
            </div>
            <Segmented label={s.name} values={s.values} labels={labels} current={value} disabled={busy(key(s.cvar))} onPick={(v) => pick(s, v)} />
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
}: {
  title: string;
  note: string;
  data: Online;
  cat: number;
  refresh: () => void;
  children?: ReactNode;
}) {
  const [q, setQ] = useState("");
  const count = data.settings.filter((s) => s.cat === cat && listed(s)).length;
  return (
    <div className="stack">
      <div>
        <SectionHead title={title} note={note} />
        {children}
      </div>
      <div className="stack-tight">
        {count > 6 && (
          <input className="search full" type="search" placeholder={`Filter ${count} settings, like "round" or "gravity"`} value={q} onChange={(e) => setQ(e.target.value)} aria-label="Filter settings" />
        )}
        <SettingsList data={data} cat={cat} refresh={refresh} q={q} />
        {cat === 2 && <ModeVoteList data={data} />}
      </div>
    </div>
  );
}

/** Restart / end the round / more time, for the Rules page. */
export function MatchButtons({ refresh }: { refresh: () => void }) {
  const ask = useConfirm();
  const run = useCommand(refresh);
  const { run: act, busy } = useActions();
  const key = (what: string) => `match:${targetKey()}:${what}`;
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
        onClick={async () => (await ask("Reset every match and fun setting for this mode to its defaults?", { ok: "Reset", danger: true })) && act(key("reset"), () => run("gm_match", ["reset"]), "Resetting this mode's settings…")}
      />
    </div>
  );
}
