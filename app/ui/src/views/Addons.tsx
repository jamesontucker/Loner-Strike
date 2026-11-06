import { targetKey, useActions } from "../api";
import { useNav } from "../store";
import type { Online } from "../types";
import { Banner, BusyButton, SectionHead, Segmented, Switch, useCommand } from "../ui";
import { usePending } from "./Settings";

/** Add-ons with their switch on Sounds instead (one switch for each, in one place). */
const ON_SOUNDS = ["quakesounds", "roundmusic"];

export function Addons({ data, refresh }: { data: Online; refresh: () => void }) {
  const run = useCommand(refresh);
  const go = useNav((s) => s.go);
  const { run: once, busy: busyKey } = useActions();
  const key = (what: string) => `addons:${targetKey()}:${what}`;
  const busy = (what: string) => busyKey(key(what));
  // what was just picked shows until the game reports it
  const pending = usePending();
  const send = (k: string, v: string, command: string, args: string[], label: string) => once(key(k), async () => {
    pending.set(k, v);
    if (!(await run(command, args, true))) pending.clear(k);
  }, label);
  const needsRestart = data.addons.some((a) => a.on !== a.loaded);
  return (
    <div className="stack">
      <div>
        <SectionHead title="Add-ons" note="Add-ons work in every mode. Turning one on or off needs a map restart." />
        {needsRestart && (
          <Banner action={<BusyButton className="btn primary" busy={busy("restartmap")} label="Restart map" busyLabel="Restarting…" onClick={() => once(key("restartmap"), () => run("gm_restartmap"), "Restarting the map…")} />}>
            Restart the map to apply your add-on changes.
          </Banner>
        )}
      </div>
      {!data.addons.length && (
        <div className="panel">
          <p className="muted">No add-ons installed. Get them from Mods.</p>
          <div className="row"><button className="btn primary" onClick={() => go("mods")}>Open Mods</button></div>
        </div>
      )}
      <div className="addons">
        {data.addons.map((a) => {
          const sets = data.addonsets.filter((s) => s.addon === a.id);
          const on = pending.value(`addon:${a.id}`, a.on ? "1" : "0") === "1";
          const onSounds = ON_SOUNDS.includes(a.id);
          return (
            <div className="addon" key={a.id}>
              <div className="addon-head">
                <div>
                  <h3>{a.name}</h3>
                  <p>
                    {a.desc}
                    {a.on !== a.loaded && <span className="muted"> · applies after a map restart</span>}
                  </p>
                </div>
                {onSounds ? (
                  <div className="row">
                    <span className="muted small-note">{a.on ? "On" : "Off"}. Turn it on or off in Sounds.</span>
                    <button className="btn" onClick={() => go("sounds")}>Open Sounds</button>
                  </div>
                ) : (
                  <Switch on={on} label={a.name} disabled={busy(`addon:${a.id}`)} onChange={(v) => send(`addon:${a.id}`, v ? "1" : "0", "gm_addon", [a.id, v ? "1" : "0"], `Turning ${a.name} ${v ? "on" : "off"}…`)} />
                )}
              </div>
              {sets.length > 0 && (
                <div className="settings">
                  {sets.map((s) => (
                    <div className="setting" key={s.cvar}>
                      <div className="name">{s.name}</div>
                      <Segmented
                        label={s.name}
                        values={s.values}
                        labels={s.labels}
                        current={pending.value(`set:${a.id}:${s.cvar}`, s.value)}
                        disabled={busy(`set:${a.id}:${s.cvar}`)}
                        onPick={(v) => send(`set:${a.id}:${s.cvar}`, v, "gm_addonset", [a.id, s.cvar, v], `Changing ${s.name}…`)}
                      />
                    </div>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
}
