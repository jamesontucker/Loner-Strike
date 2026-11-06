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
  const load = () => getJson<Data>("/api/lists").then(setData).catch(() => null);
  useEffect(() => {
    load();
  }, []);
  if (!ON_THIS_COMPUTER || !data || data.targets.length < 2) return null;

  const label = data.names[list].toLowerCase();
  const members = data.lists[list] ?? [];
  const on = members.includes(me) && members.length >= 2;
  const others = data.targets.filter((t) => t.id !== me);
  const nameOf = (id: string) => data.targets.find((t) => t.id === id)?.name ?? id;
  const key = `share:${list}:${me}`;

  const set = (next: string[], from: string, done: string) => run(key, async () => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/lists", { list, members: next, from }).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    toast(done);
    await load();
    if (from !== me) onShared?.();
  }, `Sharing the ${label}…`);

  const turn = async (v: boolean) => {
    if (!v) return set(members.filter((m) => m !== me), me, `This one's ${label} are its own again`);
    // a group already shares it: join that one and take its list
    if (members.length >= 2) {
      const names = members.map(nameOf).join(", ");
      if (!(await ask(`Use the ${label} shared by ${names}? This one's are replaced with theirs.`, { ok: "Use the shared list" }))) return;
      return set([...members, me], members[0], `Now sharing the ${label} with ${names}`);
    }
    if (!(await ask(`Share these ${label} with ${others.map((t) => t.name).join(", ")}? Theirs are replaced with this one's.`, { ok: "Share" }))) return;
    set([me, ...others.map((t) => t.id)], me, `Sharing the ${label}`);
  };
  const tick = async (id: string, v: boolean) => {
    if (v) {
      if (!(await ask(`${nameOf(id)}'s ${label} are replaced with this one's. Go ahead?`, { ok: "Share" }))) return;
      return set([...members, id], me, `${nameOf(id)} now shares the ${label}`);
    }
    set(members.filter((m) => m !== id), me, `${nameOf(id)} keeps its own ${label} now`);
  };

  const working = busy(key);
  return (
    <div className="share-list">
      <label className="row">
        <Switch on={on} onChange={turn} label="Share across servers" disabled={working} />
        <span>Share across servers<Info text={`Changes to the ${label} here are copied to the servers you tick, and theirs come here. Mods, plugins and other settings stay separate.`} /></span>
        {working && <span className="spinner" aria-hidden="true" />}
      </label>
      {on && (
        <div className="share-with">
          {others.map((t) => (
            <label key={t.id} className="follow-toggle small-note">
              <input type="checkbox" checked={members.includes(t.id)} disabled={working} onChange={(e) => tick(t.id, e.target.checked)} /> {t.name}
            </label>
          ))}
        </div>
      )}
      {!on && members.length >= 2 && <p className="muted small-note">Shared by {members.map(nameOf).join(", ")}.</p>}
    </div>
  );
}
