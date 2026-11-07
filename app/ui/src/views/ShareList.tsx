import { useEffect, useState } from "react";
import { getJson, ON_THIS_COMPUTER, postJson, targetKey, useActions } from "../api";
import { Info, Switch, useConfirm, useToast } from "../ui";

export type ListId = "admins" | "bans" | "filter" | "welcome" | "messages";
type Data = { lists: Record<ListId, string[]>; names: Record<ListId, string>; targets: { id: string; name: string }[] };

/** "Share across servers" for one list: a change saved on any of the ticked servers
 *  (or your game) is copied to the others. */
export function ShareList({ list, onShared }: { list: ListId; onShared?: () => void }) {
  const toast = useToast();
  const ask = useConfirm();
  const [data, setData] = useState<Data | null>(null);
  // your own game is "local:cstrike" or "local:czero" (each shares on its own)
  const me = targetKey() === "local" ? (data?.targets.find((t) => t.id.startsWith("local:"))?.id ?? "local") : targetKey();
  const { run, busy } = useActions();
  // switched on but nothing ticked yet: the list of servers to tick
  const [picking, setPicking] = useState(false);
  const load = () => getJson<Data>("/api/lists").then(setData).catch(() => null);
  useEffect(() => {
    load();
  }, []);
  if (!data || data.targets.length < 2) return null;

  const label = data.names[list].toLowerCase();
  const members = data.lists[list] ?? [];
  const on = members.includes(me) && members.length >= 2;
  const others = data.targets.filter((t) => t.id !== me);
  const nameOf = (id: string) => data.targets.find((t) => t.id === id)?.name ?? id;
  const key = `share:${list}:${me}`;

  // a phone sees what's shared but changes it from the computer
  if (!ON_THIS_COMPUTER) {
    return on ? <p className="muted small-note share-list">Shared with {members.filter((m) => m !== me).map(nameOf).join(", ")}.</p> : null;
  }

  const set = (next: string[], from: string, done: string) => run(key, async () => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/lists", { list, members: next, from }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    toast(done);
    await load();
    if (from !== me) onShared?.();
  }, `Sharing the ${label}…`);

  const turn = async (v: boolean) => {
    if (!v) {
      if (picking && !on) return setPicking(false);
      // the ones who stay keep sharing (they all have the same list, so any of them is the source)
      const rest = members.filter((m) => m !== me);
      return set(rest, rest.find((m) => data.targets.some((t) => t.id === m)) ?? rest[0] ?? me, `Stopped sharing the ${label}`);
    }
    // a group already shares it: join that one and take its list
    if (members.length >= 2) {
      const names = members.map(nameOf).join(", ");
      if (!(await ask(`Use the ${label} shared by ${names}? This one's are replaced with theirs.`, { ok: "Use the shared list" }))) return;
      // the source has to be one the app can open right now (your other game may not be)
      const from = members.find((m) => data.targets.some((t) => t.id === m)) ?? members[0];
      return set([...members, me], from, `Now sharing the ${label} with ${names}`);
    }
    // nothing shared yet: pick which servers to share with
    setPicking(true);
  };
  const tick = async (id: string, v: boolean) => {
    if (v) {
      if (!(await ask(`${nameOf(id)}'s ${label} are replaced with this one's. Go ahead?`, { ok: `Share the ${label}` }))) return;
      // the first tick starts the group, with this one in it
      const next = on ? [...members, id] : [me, id];
      return set(next, me, `${nameOf(id)} now shares the ${label}`).then(() => setPicking(false));
    }
    set(members.filter((m) => m !== id), me, `${nameOf(id)} keeps its own ${label} now`);
  };

  const working = busy(key);
  return (
    <div className="share-list">
      <label className="row">
        <Switch on={on || picking} onChange={turn} label="Share across servers" disabled={working} />
        <span>Share across servers<Info text={`Changes to the ${label} here are copied to the servers you tick, and theirs come here. Mods, plugins and other settings stay separate.`} /></span>
        {working && <span className="spinner" aria-hidden="true" />}
      </label>
      {picking && !on && <p className="muted small-note">Tick the servers to share with.</p>}
      {(on || picking) && (
        <div className="share-with">
          {others.map((t) => (
            <label key={t.id} className="follow-toggle small-note">
              <input type="checkbox" checked={on && members.includes(t.id)} disabled={working} onChange={(e) => tick(t.id, e.target.checked)} /> {t.name}
            </label>
          ))}
        </div>
      )}
      {!on && members.length >= 2 && <p className="muted small-note">Shared by {members.map(nameOf).join(", ")}.</p>}
    </div>
  );
}
