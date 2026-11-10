import { useEffect, useRef, useState } from "react";
import { targetKey, useActions } from "../api";
import { botsNote, botTarget, type Online, type Setting } from "../types";

/** Keys that move a range input: only these send the new bot count. */
const SLIDER_KEYS = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"];
import { Banner, BusyButton, Info, useCommand, useConfirm } from "../ui";
import { BotCommands } from "./BotCommands";
import { SettingsPage, usePending, WithSaved, type Saved } from "./Settings";
import { SMARTER_CHAT, SmarterChat } from "./SmarterChat";

/** The Bots page: the game's bots live, or with no game the next game's bot settings. */
export function Bots({ data, refresh }: { data: Online | null; refresh: () => void }) {
  if (data) return <BotsPage data={data} refresh={refresh} />;
  return <WithSaved title="Bots">{(d, saved) => <BotsPage data={d} refresh={refresh} saved={saved} />}</WithSaved>;
}

function BotsPage({ data, refresh, saved }: { data: Online; refresh: () => void; saved?: Saved }) {
  const ask = useConfirm();
  const run = useCommand(refresh);
  const { run: act, busy } = useActions();
  const key = (what: string) => `bots:${targetKey()}:${what}`;
  const bots = (what: string, label: string) => act(key(what), () => run("gm_bots", [what]), label);
  const st = data.state;
  const personality = data.settings.find((s) => s.cvar === "gm_personality");
  // the bot count asked for (not the bots playing, which can lag behind or wait for people)
  const target = botTarget(st);
  const note = botsNote(st);
  // where the slider is while it's being moved, before anything is sent
  const [draft, setDraft] = useState<number | null>(null);
  // the count just sent, shown until the game reports it (or a few seconds pass)
  const [want, setWant] = useState<{ n: number; at: number } | null>(null);
  // the newest count to send, for the send that's going (it sends again if this changed)
  const wantN = useRef<number | null>(null);
  useEffect(() => {
    if (!want) return;
    if (want.n === target) return setWant(null);
    const t = setTimeout(() => setWant(null), Math.max(0, 4000 - (Date.now() - want.at)));
    return () => clearTimeout(t);
  }, [want, target]);
  const count = draft ?? want?.n ?? target;
  const settle = useRef(0);
  useEffect(() => () => clearTimeout(settle.current), []);
  const countBusy = busy(key("count"));
  /** Sends a new count, unless it's what's already asked for. A count picked while one is
   *  on its way goes right after it, so the last one picked is the one that sticks. */
  const sendCount = (n: number) => {
    clearTimeout(settle.current);
    setDraft(null);
    if (n === (wantN.current ?? want?.n ?? target)) return;
    wantN.current = n;
    setWant({ n, at: Date.now() });
    act(key("count"), async () => {
      let sent: number | null = null;
      try {
        while (wantN.current !== null && wantN.current !== sent) {
          sent = wantN.current;
          if (!(await (saved ? saved.setBots(sent) : run("gm_bots", ["count", String(sent)], true)))) {
            setWant(null);
            return;
          }
        }
      } finally {
        wantN.current = null;
      }
    }, "Saving the bot count…");
  };
  const learning = st.navleft > 0;
  // learning one map doesn't show in navleft: remember it was started until the map has its nav
  const [learnStarted, setLearnStarted] = useState<Record<string, boolean>>({});
  const learnSent = !!learnStarted[st.map] && !st.nav && !learning;
  const learn = () => act(key("learn"), async () => {
    if (await run("gm_nav", ["learn"])) setLearnStarted((l) => ({ ...l, [st.map]: true }));
  }, `Starting to learn ${st.map}…`);
  const most = Math.max(1, st.maxplayers - 1);
  /** One more or one fewer, for the buttons beside the slider (easier than dragging on a phone). */
  const step = (d: number) => {
    const n = Math.min(most, Math.max(0, count + d));
    if (n === count) return;
    clearTimeout(settle.current);
    setDraft(n);
    settle.current = window.setTimeout(() => sendCount(n), 500);
  };

  const resetShared = async () => {
    if (!(await ask("Reset bot and fun settings? This puts every bot and fun setting shared by all modes back to its default. The bot count stays, and so do each mode's own settings.", { ok: "Reset", danger: true }))) return;
    act(key("resetshared"), () => (saved ? saved.reset("shared") : run("gm_match", ["resetshared"])), "Resetting bot and fun settings…");
  };

  return (
    <div className="stack">
    <SettingsPage title="Bots" note={saved ? "" : "Difficulty changes re-add the bots so they pick it up straight away."} data={data} cat={1} refresh={refresh} pickMode saved={saved}>
      {!saved && !st.nav && !learning && !learnSent && (
        <Banner action={<BusyButton className="btn primary" busy={busy(key("learn"))} label="Learn this map" busyLabel="Starting…" onClick={learn} />}>
          Bots don't know {st.map} yet. Learning it takes a few minutes. The game may pause, then the map restarts.
        </Banner>
      )}
      {!saved && learnSent && <Banner><span className="spinner" aria-hidden="true" />Bots are learning {st.map}. The game may pause, then the map restarts.</Banner>}
      {!saved && learning && <Banner><span className="spinner" aria-hidden="true" />Bots are learning {st.map}… {st.navleft} map{st.navleft === 1 ? "" : "s"} left.</Banner>}
      <div className="slider-row">
        <label htmlFor="botCount">Bots<Info text="How many bots play. Each one takes a player slot, so leave room for friends." /></label>
        <div className="slider-step">
        <button type="button" className="btn step" aria-label="One bot fewer" disabled={countBusy || count <= 0} aria-busy={countBusy || undefined} onClick={() => step(-1)}>−</button>
        <input
          id="botCount"
          type="range"
          min={0}
          max={most}
          value={count}
          aria-describedby={note ? "botCountNote" : undefined}
          onChange={(e) => {
            const n = +e.target.value;
            setDraft(n);
            // screen readers and switch controls change it without pointer or key events:
            // a quiet moment sends it too
            clearTimeout(settle.current);
            settle.current = window.setTimeout(() => sendCount(n), 700);
          }}
          // (a tap without moving sends nothing: it's the same count)
          onPointerUp={() => sendCount(count)}
          onPointerCancel={() => {
            clearTimeout(settle.current);
            setDraft(null);
          }}
          onKeyUp={(e) => SLIDER_KEYS.includes(e.key) && sendCount(count)}
        />
        <button type="button" className="btn step" aria-label="One more bot" disabled={countBusy || count >= most} aria-busy={countBusy || undefined} onClick={() => step(1)}>+</button>
        </div>
        <output htmlFor="botCount">{count}</output>
        <span className="muted" aria-live="polite" id="botCountNote">
          {countBusy ? (
            <>
              <span className="spinner" aria-hidden="true" />
              Saving…
            </>
          ) : (
            note
          )}
        </span>
        {saved ? (
          <p className="muted small-note">Adding a bot to a team, killing and kicking bots work once a game is running.</p>
        ) : (
        <div className="row">
          <BusyButton busy={busy(key("addt"))} label="Add a Terrorist" busyLabel="Adding…" onClick={() => bots("addt", "Adding a Terrorist bot…")} />
          <BusyButton busy={busy(key("addct"))} label="Add a Counter-Terrorist" busyLabel="Adding…" onClick={() => bots("addct", "Adding a Counter-Terrorist bot…")} />
          <BusyButton busy={busy(key("kill"))} label="Kill all" busyLabel="Killing…" onClick={() => bots("kill", "Killing all bots…")} />
          <BusyButton className="btn danger" busy={busy(key("kick"))} label="Kick all" busyLabel="Kicking…" onClick={async () => (await ask("Kick every bot? They stay out until you add more.", { ok: "Kick all", danger: true })) && bots("kick", "Kicking all bots…")} />
          <Info text="Kill all only kills the bots, and they're back next round. Kick all takes every bot out until you add more." />
        </div>
        )}
      </div>
      {personality?.visible ? <PersonalitySlider setting={personality} refresh={refresh} nextGame={saved} /> : null}
    </SettingsPage>
    <BotCommands />
    {SMARTER_CHAT && <SmarterChat />}
    {/* resetting goes last, as on Rules and Mods */}
    <div className="panel row between">
      <div>
        <b>Reset bot and fun settings</b>
        <p className="muted">Every bot and fun setting shared by all modes goes back to its default. The bot count stays.</p>
      </div>
      <BusyButton className="btn danger" busy={busy(key("resetshared"))} label="Reset" busyLabel="Resetting…" onClick={resetShared} />
    </div>
    </div>
  );
}

/** A few words for a personality strength (gm_personality, percent). */
function strength(v: number) {
  if (v <= 0) return "Plain";
  if (v < 100) return "Mild";
  if (v === 100) return "Normal";
  if (v < 200) return "Strong";
  return "Exaggerated";
}

/** How strongly the bots' personalities show (gm_personality, 0 to 200 percent). */
function PersonalitySlider({ setting, refresh, nextGame }: { setting: Setting; refresh: () => void; nextGame?: Saved }) {
  const run = useCommand(refresh);
  const { run: act, busy } = useActions();
  const pending = usePending();
  const key = `settings:${targetKey()}:${nextGame ? "saved:" : ""}gm_personality`;
  const saved = Math.min(200, Math.max(0, Math.round(Number(pending.value(setting.cvar, setting.value)) || 0)));
  const [draft, setDraft] = useState<number | null>(null);
  const value = draft ?? saved;
  const settle = useRef(0);
  // the newest value to send, for the save that's going (it sends again if this changed)
  const next = useRef<number | null>(null);
  const save = (n: number) => {
    clearTimeout(settle.current);
    setDraft(null);
    if (n === (next.current ?? saved)) return;
    next.current = n;
    pending.set(setting.cvar, String(n));
    act(key, async () => {
      let sent: number | null = null;
      try {
        while (next.current !== null && next.current !== sent) {
          sent = next.current;
          if (!(await (nextGame ? nextGame.save(setting.cvar, String(sent)) : run("gm_set", [setting.cvar, String(sent)], true)))) {
            pending.clear(setting.cvar);
            return;
          }
        }
      } finally {
        next.current = null;
      }
    }, "Saving personality…");
  };
  const saving = busy(key);
  return (
    <div className="slider-row wide-out">
      <label htmlFor="botPersonality">
        Personality
        <Info text="How strongly each bot's personality shows. At 0 they play their style plainly: no tilt, quirks, show-off moments or habits, and they hardly chat. 100 is normal, 200 exaggerates it." />
      </label>
      <div className="slider-step">
        <input
          id="botPersonality"
          type="range"
          min={0}
          max={200}
          step={10}
          value={value}
          // stays usable while saving, so it keeps focus: a newer value goes after the save
          aria-busy={saving || undefined}
          aria-valuetext={`${value} percent, ${strength(value).toLowerCase()}`}
          onChange={(e) => {
            const n = +e.target.value;
            setDraft(n);
            // screen readers and switch controls change it without pointer or key events
            clearTimeout(settle.current);
            settle.current = window.setTimeout(() => save(n), 700);
          }}
          onPointerUp={() => save(value)}
          onKeyUp={(e) => SLIDER_KEYS.includes(e.key) && save(value)}
        />
      </div>
      <output htmlFor="botPersonality">{value}</output>
      <span className="muted" aria-live="polite">
        {saving ? (
          <>
            <span className="spinner" aria-hidden="true" />
            Saving…
          </>
        ) : (
          strength(value)
        )}
      </span>
    </div>
  );
}
