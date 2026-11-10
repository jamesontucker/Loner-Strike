import { targetKey, useActions } from "../api";
import { useNav } from "../store";
import type { Online } from "../types";
import { Banner, BusyButton, SectionHead, Segmented, Switch, useCommand } from "../ui";
import { SAVED_NOTE, usePending, WithSaved, type Saved } from "./Settings";

/** Add-ons with their switch on Sounds instead (one switch for each, in one place). */
const ON_SOUNDS = ["quakesounds", "roundmusic"];

/** Add-ons: switched in the game live, or with no game for the next one. */
export function Addons({ data, refresh }: { data: Online | null; refresh: () => void }) {
  if (data) return <AddonsPage data={data} refresh={refresh} />;
  return <WithSaved title="Add-ons">{(d, saved) => <AddonsPage data={d} refresh={refresh} saved={saved} />}</WithSaved>;
}

function AddonsPage({ data, refresh, saved }: { data: Online; refresh: () => void; saved?: Saved }) {
  const run = useCommand(refresh);
  const go = useNav((s) => s.go);
  const { run: once, busy: busyKey } = useActions();
  const key = (what: string) => `addons:${targetKey()}:${saved ? "saved:" : ""}${what}`;
  const busy = (what: string) => busyKey(key(what));
  // what was just picked shows until the game reports it
  const pending = usePending();
  // (no game: the same change goes into the next game's files)
  const send = (k: string, v: string, command: string, args: string[], label: string) => once(key(k), async () => {
    pending.set(k, v);
    const [id, a, b] = args;
    const ok = saved ? await saved.addon(command === "gm_addon" ? { id, on: a === "1" } : { id, cvar: a, value: b }) : await run(command, args, true);
    if (!ok) pending.clear(k);
  }, label);
  const needsRestart = !saved && data.addons.some((a) => a.on !== a.loaded);
  return (
    <div className="stack">
      <div>
        <SectionHead title="Add-ons" note={saved ? `Add-ons work in every mode. ${SAVED_NOTE}` : "Add-ons work in every mode. Turning one on or off needs a map restart."} />
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
                    {/* (said with the description, so the button sits where the other cards' switches do) */}
                    {onSounds && <span className="muted"> · {a.on ? "On" : "Off"}. Turn it on or off in Sounds.</span>}
                  </p>
                </div>
                {onSounds ? (
                  <button className="btn" onClick={() => go("sounds")}>Open Sounds</button>
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
