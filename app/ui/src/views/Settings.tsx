import { useState, type ReactNode } from "react";
import type { Online, Setting } from "../types";
import { SectionHead, Segmented, useCommand, useConfirm } from "../ui";

// Short explanations under settings, as in the old panel
const NOTES: Record<string, string> = {
  bot_profile_db: "Takes effect on the next map",
  gm_bot_weapons: "What bots buy and use",
  gm_weapons: "Restricts buying and hands out the gun",
  sv_airaccelerate: "Surf / bhop for surf maps",
  gm_hb_reaction: "Bots wait a human reaction time before shooting",
  gm_hb_aim: "Flicks that overshoot, aim wobble, sprays in bursts",
  gm_hb_mistakes: "The odd whiffed shot, late reaction or panic spray",
  gm_hb_bhop: "Locked In, Hackers and Gods hop between fights",
  gm_regulars: "The same regulars keep coming back, plus the odd newcomer",
  gm_churn: "Now and then someone leaves and a new player takes the slot",
  gm_hb_habits: "Each bot plays like its personality: favourite guns, warming up, getting tired, nervous reloads, a rival",
  gm_hb_tilt: "A bad run makes most bots sloppier (some careful); outnumbered or low on health, some panic",
  gm_hb_showoff: "Now and then after a kill: crouch spam, a jump or the knife out (never in a match)",
  gm_hb_afk: "Now and then a bot stands still for a few seconds at round start",
  gm_hb_sprays: "Bots spray a logo now and then",
  gm_hb_radio: "Radio calls that fit the moment: enemy spotted, enemy down, the team's caller says go",
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
const STYLE_NOTES: Record<string, string> = {
  "0": "CZ's own bots, unchanged",
  "4": "Slow to notice you, wobbly aim, spray until empty",
  "1": "Mixed skill: a couple of sharp players, lots of average, some bad",
  "2": "Quick, accurate, controlled bursts, play as a team",
  "3": "Seasoned: fast reactions, tight aim, few mistakes",
  "5": "Pro level: crisp flicks, short bursts, the occasional bunnyhop",
  "6": "Nearly an aimbot: snaps to heads, bunnyhops often",
  "7": "Everything maxed. Good luck",
};

export function SettingsList({ data, cat, refresh, q = "" }: { data: Online; cat: number; refresh: () => void; q?: string }) {
  const run = useCommand(refresh);
  const all = data.settings.filter((s) => s.cat === cat && s.visible);
  const words = q.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const list = all.filter((s) => {
    const text = `${s.name} ${NOTES[s.cvar] ?? ""} ${s.labels}`.toLowerCase();
    return words.every((w) => text.includes(w));
  });
  if (!all.length) return <p className="muted">Nothing to set here in this mode.</p>;
  if (!list.length) return <p className="muted">No settings match "{q.trim()}".</p>;
  return (
    <div className="settings">
      {list.map((s: Setting) => (
        <div className="setting" key={s.cvar}>
          <div className="name">
            {s.name}
            {(s.cvar === "gm_botstyle" ? STYLE_NOTES[s.value] : NOTES[s.cvar]) && (
              <span className="note">{s.cvar === "gm_botstyle" ? STYLE_NOTES[s.value] : NOTES[s.cvar]}</span>
            )}
          </div>
          <Segmented
            label={s.name}
            values={s.values}
            labels={s.labels}
            current={s.value}
            onPick={(v) => run("gm_set", [s.cvar, v], true)}
          />
        </div>
      ))}
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
  const count = data.settings.filter((s) => s.cat === cat && s.visible).length;
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
      </div>
    </div>
  );
}

/** Restart / end the round / more time, for the Match page. */
export function MatchButtons({ refresh }: { refresh: () => void }) {
  const ask = useConfirm();
  const run = useCommand(refresh);
  return (
    <div className="row">
      <button className="btn primary" onClick={() => run("gm_match", ["restart"])}>Restart round</button>
      <button className="btn" onClick={() => run("gm_do", ["endround"])}>End round now</button>
      <button className="btn" onClick={() => run("gm_do", ["addtime"])}>Add 10 minutes</button>
      <button className="btn danger" onClick={async () => (await ask("Reset every match and fun setting for this mode to its defaults?", { ok: "Reset", danger: true })) && run("gm_match", ["reset"])}>
        Reset this mode's settings
      </button>
    </div>
  );
}
