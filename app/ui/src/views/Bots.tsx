import { useEffect, useState } from "react";
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
  const sendCount = (n: number) => run("gm_bots", ["count", String(n)], true).then(() => setTimeout(() => setDragging(false), 1500));
  const learning = st.navleft > 0;

  return (
    <SettingsPage title="Bots" note="Difficulty changes re-add the bots so they pick it up straight away." data={data} cat={1} refresh={refresh}>
      {!st.nav && !learning && (
        <Banner action={<BusyButton className="btn primary" busy={busy(key("learn"))} label="Learn this map" busyLabel="Starting…" onClick={() => act(key("learn"), () => run("gm_nav", ["learn"]), `Starting to learn ${st.map}…`)} />}>
          Bots don't know {st.map} yet. Learning it takes a few minutes, and the map timer waits.
        </Banner>
      )}
      {learning && <Banner>Bots are learning {st.map}… {st.navleft} step{st.navleft === 1 ? "" : "s"} left.</Banner>}
      <div className="slider-row">
        <label htmlFor="botCount">Bots<Info text="How many bots are in the game." /></label>
        <input
          id="botCount"
          type="range"
          min={0}
          max={Math.max(1, st.maxplayers - 1)}
          value={count}
          onChange={(e) => {
            setDragging(true);
            setCount(+e.target.value);
          }}
          onPointerUp={() => sendCount(count)}
          onKeyUp={(e) => SLIDER_KEYS.includes(e.key) && sendCount(count)}
        />
        <output htmlFor="botCount">{count}</output>
        <div className="row">
          <BusyButton busy={busy(key("addt"))} label="Add a Terrorist" busyLabel="Adding…" onClick={() => bots("addt", "Adding a Terrorist bot…")} />
          <BusyButton busy={busy(key("addct"))} label="Add a CT" busyLabel="Adding…" onClick={() => bots("addct", "Adding a CT bot…")} />
          <BusyButton busy={busy(key("kill"))} label="Kill all" busyLabel="Killing…" onClick={() => bots("kill", "Killing all bots…")} />
          <BusyButton className="btn danger" busy={busy(key("kick"))} label="Kick all" busyLabel="Kicking…" onClick={async () => (await ask("Kick every bot? They stay out until you add more.", { ok: "Kick all", danger: true })) && bots("kick", "Kicking all bots…")} />
          <Info text="Kill all only kills the bots, and they're back next round. Kick all takes every bot out until you add more." />
        </div>
      </div>
    </SettingsPage>
  );
}
