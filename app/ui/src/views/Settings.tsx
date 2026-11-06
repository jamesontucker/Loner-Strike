import type { ReactNode } from "react";
import type { Online, Setting } from "../types";
import { SectionHead, Segmented, useCommand } from "../ui";

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
  gm_rtv: "Say rtv in chat to vote for a new map, nominate <map> to suggest one",
  gm_welcome: "Shown when you or a friend joins. Say /welcome to see it again",
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

export function SettingsList({ data, cat, refresh }: { data: Online; cat: number; refresh: () => void }) {
  const run = useCommand(refresh);
  const list = data.settings.filter((s) => s.cat === cat && s.visible);
  if (!list.length) return <p className="muted">Nothing to set here in this mode.</p>;
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
  return (
    <div className="stack">
      <div>
        <SectionHead title={title} note={note} />
        {children}
      </div>
      <SettingsList data={data} cat={cat} refresh={refresh} />
    </div>
  );
}

/** Restart / end the round / more time, for the Match page. */
export function MatchButtons({ refresh }: { refresh: () => void }) {
  const run = useCommand(refresh);
  return (
    <div className="row">
      <button className="btn primary" onClick={() => run("gm_match", ["restart"])}>Restart round</button>
      <button className="btn" onClick={() => run("gm_do", ["endround"])}>End round now</button>
      <button className="btn" onClick={() => run("gm_do", ["addtime"])}>Add 10 minutes</button>
      <button className="btn danger" onClick={() => confirm("Reset every match and fun setting for this mode to its defaults?") && run("gm_match", ["reset"])}>
        Reset this mode's settings
      </button>
    </div>
  );
}
