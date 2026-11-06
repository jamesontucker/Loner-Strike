import { targetKey, useActions } from "../api";
import type { Online } from "../types";
import { Banner, BusyButton, SectionHead, Segmented, Switch, useCommand } from "../ui";

export function Addons({ data, refresh }: { data: Online; refresh: () => void }) {
  const run = useCommand(refresh);
  const { run: once, busy: busyKey } = useActions();
  const key = (what: string) => `addons:${targetKey()}:${what}`;
  const busy = (what: string) => busyKey(key(what));
  const pending = data.addons.some((a) => a.on !== a.loaded);
  return (
    <div className="stack">
      <div>
        <SectionHead title="Add-ons" note="Add-ons work in every mode. Turning one on or off needs a map restart." />
        {pending && (
          <Banner action={<BusyButton className="btn primary" busy={busy("restartmap")} label="Restart map" busyLabel="Restarting…" onClick={() => once(key("restartmap"), () => run("gm_restartmap"), "Restarting the map…")} />}>
            Restart the map to apply your add-on changes.
          </Banner>
        )}
      </div>
      <div className="addons">
        {data.addons.map((a) => {
          const sets = data.addonsets.filter((s) => s.addon === a.id);
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
                <Switch on={!!a.on} label={a.name} disabled={busy(`addon:${a.id}`)} onChange={(v) => once(key(`addon:${a.id}`), () => run("gm_addon", [a.id, v ? "1" : "0"], true), `Turning ${a.name} ${v ? "on" : "off"}…`)} />
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
                        current={s.value}
                        disabled={busy(`set:${a.id}:${s.cvar}`)}
                        onPick={(v) => once(key(`set:${a.id}:${s.cvar}`), () => run("gm_addonset", [a.id, s.cvar, v], true), `Changing ${s.name}…`)}
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
