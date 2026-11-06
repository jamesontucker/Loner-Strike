import type { Online } from "../types";
import { Banner, SectionHead, Segmented, Switch, useCommand } from "../ui";

export function Addons({ data, refresh }: { data: Online; refresh: () => void }) {
  const run = useCommand(refresh);
  const pending = data.addons.some((a) => a.on !== a.loaded);
  return (
    <div className="stack">
      <div>
        <SectionHead title="Add-ons" note="Add-ons work in every mode. Turning one on or off needs a map restart." />
        {pending && (
          <Banner action={<button className="btn primary" onClick={() => run("gm_restartmap")}>Restart map</button>}>
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
                <Switch on={!!a.on} label={a.name} onChange={(v) => run("gm_addon", [a.id, v ? "1" : "0"], true)} />
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
                        onPick={(v) => run("gm_addonset", [a.id, s.cvar, v], true)}
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
