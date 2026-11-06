import { useEffect, useState } from "react";
import type { Online } from "../types";
import { Banner, useCommand } from "../ui";
import { SettingsPage } from "./Settings";

export function Bots({ data, refresh }: { data: Online; refresh: () => void }) {
  const run = useCommand(refresh);
  const st = data.state;
  const [count, setCount] = useState(st.bots);
  const [dragging, setDragging] = useState(false);
  useEffect(() => {
    if (!dragging) setCount(st.bots);
  }, [st.bots, dragging]);

  return (
    <SettingsPage title="Bots" note="Difficulty changes re-add the bots so they pick it up straight away." data={data} cat={1} refresh={refresh}>
      {!st.nav && (
        <Banner action={<button className="btn primary" onClick={() => run("gm_nav", ["learn"])}>Learn this map</button>}>
          Bots don't know {st.map} yet. Learning it takes a few minutes, and the map timer waits.
        </Banner>
      )}
      {st.navleft > 0 && <Banner>Bots are learning {st.map}… {st.navleft} step{st.navleft === 1 ? "" : "s"} left.</Banner>}
      <div className="slider-row">
        <label htmlFor="botCount">Bots</label>
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
          onPointerUp={() => {
            run("gm_bots", ["count", String(count)], true).then(() => setTimeout(() => setDragging(false), 1500));
          }}
          onKeyUp={() => run("gm_bots", ["count", String(count)], true)}
        />
        <output htmlFor="botCount">{count}</output>
        <div className="row">
          <button className="btn" onClick={() => run("gm_bots", ["addt"])}>Add a Terrorist</button>
          <button className="btn" onClick={() => run("gm_bots", ["addct"])}>Add a CT</button>
          <button className="btn" onClick={() => run("gm_bots", ["kill"])}>Kill all</button>
          <button className="btn danger" onClick={() => confirm("Kick every bot?") && run("gm_bots", ["kick"])}>Kick all</button>
        </div>
      </div>
    </SettingsPage>
  );
}
