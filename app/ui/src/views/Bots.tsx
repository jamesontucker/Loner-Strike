import { useEffect, useRef, useState } from "react";
import { targetKey, useActions } from "../api";
import type { Online } from "../types";

/** Keys that move a range input: only these send the new bot count. */
const SLIDER_KEYS = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End", "PageUp", "PageDown"];
import { Banner, BusyButton, Info, useCommand, useConfirm } from "../ui";
import { SettingsPage } from "./Settings";

export function Bots({ data, refresh }: { data: Online; refresh: () => void }) {
  const ask = useConfirm();
  const run = useCommand(refresh);
  const { run: act, busy } = useActions();
  const key = (what: string) => `bots:${targetKey()}:${what}`;
  const bots = (what: string, label: string) => act(key(what), () => run("gm_bots", [what]), label);
  const st = data.state;
  const [count, setCount] = useState(st.bots);
  const [dragging, setDragging] = useState(false);
  useEffect(() => {
    if (!dragging) setCount(st.bots);
  }, [st.bots, dragging]);
  // send the count, then let the game's own number show again once it's had time to answer
  const settle = useRef(0);
  const sendCount = (n: number) => run("gm_bots", ["count", String(n)], true).then(() => setTimeout(() => setDragging(false), 1500));
  const learning = st.navleft > 0;
  const most = Math.max(1, st.maxplayers - 1);
  /** One more or one fewer, for the buttons beside the slider (easier than dragging on a phone). */
  const step = (d: number) => {
    const n = Math.min(most, Math.max(0, count + d));
    if (n === count) return;
    clearTimeout(settle.current);
    setDragging(true);
    setCount(n);
    settle.current = window.setTimeout(() => sendCount(n), 500);
  };

  return (
    <SettingsPage title="Bots" note="Difficulty changes re-add the bots so they pick it up straight away." data={data} cat={1} refresh={refresh}>
      {!st.nav && !learning && (
        <Banner action={<BusyButton className="btn primary" busy={busy(key("learn"))} label="Learn this map" busyLabel="Starting…" onClick={() => act(key("learn"), () => run("gm_nav", ["learn"]), `Starting to learn ${st.map}…`)} />}>
          Bots don't know {st.map} yet. Learning it takes a few minutes, and the map timer waits.
        </Banner>
      )}
      {learning && <Banner>Bots are learning {st.map}… {st.navleft} step{st.navleft === 1 ? "" : "s"} left.</Banner>}
      <div className="slider-row">
        <label htmlFor="botCount">Bots<Info text="How many bots play. Each one takes a player slot, so leave room for friends." /></label>
        <div className="slider-step">
        <button type="button" className="btn step" aria-label="One bot fewer" disabled={count <= 0} onClick={() => step(-1)}>−</button>
        <input
          id="botCount"
          type="range"
          min={0}
          max={most}
          value={count}
          onChange={(e) => {
            setDragging(true);
            setCount(+e.target.value);
            // screen readers and switch controls change it without pointer or key events:
            // a quiet moment sends it too
            clearTimeout(settle.current);
            const n = +e.target.value;
            settle.current = window.setTimeout(() => sendCount(n), 700);
          }}
          onPointerUp={() => {
            clearTimeout(settle.current);
            sendCount(count);
          }}
          onPointerCancel={() => setDragging(false)}
          onKeyUp={(e) => {
            if (!SLIDER_KEYS.includes(e.key)) return;
            clearTimeout(settle.current);
            sendCount(count);
          }}
        />
        <button type="button" className="btn step" aria-label="One more bot" disabled={count >= most} onClick={() => step(1)}>+</button>
        </div>
        <output htmlFor="botCount">{count}</output>
        <div className="row">
          <BusyButton busy={busy(key("addt"))} label="Add a Terrorist" busyLabel="Adding…" onClick={() => bots("addt", "Adding a Terrorist bot…")} />
          <BusyButton busy={busy(key("addct"))} label="Add a Counter-Terrorist" busyLabel="Adding…" onClick={() => bots("addct", "Adding a Counter-Terrorist bot…")} />
          <BusyButton busy={busy(key("kill"))} label="Kill all" busyLabel="Killing…" onClick={() => bots("kill", "Killing all bots…")} />
          <BusyButton className="btn danger" busy={busy(key("kick"))} label="Kick all" busyLabel="Kicking…" onClick={async () => (await ask("Kick every bot? They stay out until you add more.", { ok: "Kick all", danger: true })) && bots("kick", "Kicking all bots…")} />
          <Info text="Kill all only kills the bots, and they're back next round. Kick all takes every bot out until you add more." />
        </div>
      </div>
    </SettingsPage>
  );
}
