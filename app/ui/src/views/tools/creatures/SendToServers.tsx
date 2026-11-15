// "Send my creatures to…": your own creature types (with their custom bases, models and sounds)
// go to servers of yours, as one marked block in each server's creatures.ini; the server's own
// types are never touched. Each server says which of your types it has, up to date or changed,
// with Update and Remove (tools/creaturesend.rs).

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { BusyButton, Sheet, useToast } from "../../../ui";
import { addToServerModes, creatureServers, sendCreatures, type SendResult, type ServerCreatures } from "./creaturesApi";

const STATE_WORDS: Record<string, string> = {
  current: "Up to date",
  changed: "Changed here",
  missing: "Not there",
  theirs: "The server's own",
  gone: "Not on this computer",
};

export function SendToServers({ own, start, onClose }: { own: string[]; start?: string; onClose: () => void }) {
  const toast = useToast();
  const qc = useQueryClient();
  const q = useQuery({ queryKey: ["tools", "creatures", "servers"], queryFn: ({ signal }) => creatureServers(signal) });
  const servers = q.data?.servers ?? [];
  const [picked, setPicked] = useState<string[]>([]);
  const [types, setTypes] = useState<string[]>(start && own.includes(start) ? [start] : []);
  const [busy, setBusy] = useState<string | null>(null);
  const [summary, setSummary] = useState<SendResult | null>(null);

  const toggle = (list: string[], v: string) => (list.includes(v) ? list.filter((x) => x !== v) : [...list, v]);
  const run = async (label: string, ids: string[], add: string[], remove: string[], modes?: string[]) => {
    setBusy(label);
    try {
      const r = modes ? await addToServerModes(ids, modes, add) : await sendCreatures(ids, add, remove);
      if (r.error) return toast(r.error, true);
      setSummary(r);
      toast(r.message || "Sent.", !r.ok);
      await qc.invalidateQueries({ queryKey: ["tools", "creatures", "servers"] });
    } finally {
      setBusy(null);
    }
  };
  // modes on a server that load only the types they list, and which of these types they leave out
  const unlisted = (srv: ServerCreatures | undefined, names: string[]) => (srv?.limits ?? []).filter((m) => names.some((n) => !m.types.includes(n)));
  const lsNote = (id: string, sentNames: string[]) => {
    const modes = unlisted(servers.find((x) => x.id === id), sentNames);
    return modes.length ? `${modes.map((m) => m.name).join(", ")} only load the types they list: add ${sentNames.join(", ")} (below)` : "";
  };
  const sending = `Sending to ${picked.length} server${picked.length === 1 ? "" : "s"}…`;

  return (
    <Sheet className="sheet wide cs-send" label="Send my creatures to…" onClose={busy ? undefined : onClose}>
      <div className="sheet-body">
        <h2>Send my creatures to…</h2>
        <p className="muted">Your types go to your servers with their models and sounds. A server's own types are never changed. Servers use them from their next map.</p>
        {q.isPending ? (
          <p className="muted">
            <span className="spinner" aria-hidden="true" /> Looking at your servers…
          </p>
        ) : q.data?.error ? (
          <p className="cs-error">{q.data.error}</p>
        ) : servers.length === 0 ? (
          <p className="muted">You have no servers yet. Make one on the Servers page.</p>
        ) : (
          <div className="cs-send-grid">
            <fieldset>
              <legend>Servers</legend>
              {servers.map((s) => (
                <label key={s.id} className="tl-check">
                  <input type="checkbox" checked={picked.includes(s.id)} onChange={() => setPicked(toggle(picked, s.id))} /> {s.name}
                  {s.remote && <span className="muted"> (another computer: uploaded)</span>}
                  {!s.sameGame && <span className="muted"> ({s.game === "czero" ? "Condition Zero" : "CS 1.6"})</span>}
                </label>
              ))}
            </fieldset>
            <fieldset>
              <legend>Your types</legend>
              {own.length === 0 ? (
                <p className="muted">Make a type of your own first (New from a template).</p>
              ) : (
                own.map((n) => (
                  <label key={n} className="tl-check">
                    <input type="checkbox" checked={types.includes(n)} onChange={() => setTypes(toggle(types, n))} /> {n}
                  </label>
                ))
              )}
            </fieldset>
          </div>
        )}
        <div className="row">
          <BusyButton className="btn primary" busy={busy === "send"} disabled={!!busy || !picked.length || !types.length} label="Send" busyLabel={sending} onClick={() => void run("send", picked, types, [])} />
          <button type="button" className="btn" disabled={!!busy} onClick={onClose}>
            Close
          </button>
        </div>
        {summary && (
          <ul className="cs-send-summary" aria-label="What was sent">
            {summary.results.map((r) => (
              <li key={r.id} className={r.ok ? "" : "cs-error"}>
                <strong>{r.name ?? r.id}</strong>:{" "}
                {r.error ?? [r.types?.length ? `sent ${(r.sent ?? []).join(", ") || "nothing new"}` : "nothing to send", r.skipped?.length ? `kept the server's own ${r.skipped.join(", ")}` : "", r.kept?.length ? `kept its own ${r.kept.join(", ")}` : "", lsNote(r.id, r.sent ?? [])].filter(Boolean).join("; ")}
              </li>
            ))}
          </ul>
        )}
        {servers.length > 0 && <h3 className="cs-send-h">What's on each server</h3>}
        {servers.map((s: ServerCreatures) => (
          <section key={s.id} className="cs-send-server" aria-label={`On ${s.name}`}>
            <h4>{s.name}</h4>
            {s.types.length === 0 ? (
              <p className="muted">None of your types.</p>
            ) : (
              <ul>
                {s.types.map((t) => (
                  <li key={t.name} className="row">
                    <span className="cs-send-name">{t.name}</span>
                    <span className={`cs-tag cs-send-${t.state}`}>{STATE_WORDS[t.state] ?? t.state}</span>
                    {t.state === "changed" && (
                      <BusyButton className="btn small" busy={busy === `up:${s.id}:${t.name}`} disabled={!!busy} label="Update" busyLabel="Updating…" onClick={() => void run(`up:${s.id}:${t.name}`, [s.id], [t.name], [])} />
                    )}
                    {t.state === "missing" && (
                      <BusyButton className="btn small" busy={busy === `add:${s.id}:${t.name}`} disabled={!!busy} label="Add" busyLabel="Adding…" onClick={() => void run(`add:${s.id}:${t.name}`, [s.id], [t.name], [])} />
                    )}
                    {(s.limits?.length ?? 0) > 0 && (t.state === "current" || t.state === "changed") && s.limits!.every((m) => m.types.includes(t.name)) && <span className="cs-tag">In their lists</span>}
                    {(t.state === "current" || t.state === "changed" || t.state === "gone") && (
                      <BusyButton className="btn small" busy={busy === `rm:${s.id}:${t.name}`} disabled={!!busy} label="Remove" busyLabel="Removing…" onClick={() => void run(`rm:${s.id}:${t.name}`, [s.id], [], [t.name])} />
                    )}
                  </li>
                ))}
              </ul>
            )}
            {(() => {
              // modes that load only the types they list: one sent there that isn't listed won't show
              const sent = s.types.filter((t) => t.state === "current" || t.state === "changed").map((t) => t.name);
              const modes = (s.limits ?? []).map((m) => ({ ...m, missing: sent.filter((n) => !m.types.includes(n)) })).filter((m) => m.missing.length);
              if (!modes.length) return null;
              const names = [...new Set(modes.flatMap((m) => m.missing))];
              return (
                <div className="cs-send-waves" role="note">
                  <p>
                    These modes on {s.name} only load the creature types they list: {modes.map((m) => m.name).join(", ")}. Add {names.join(", ")} to their lists to use {names.length === 1 ? "it" : "them"} there.
                  </p>
                  <div className="row">
                    {modes.map((m) => (
                      <BusyButton key={m.id} className="btn small" busy={busy === `mode:${s.id}:${m.id}`} disabled={!!busy} label={`Add to ${m.name}'s creatures`} busyLabel="Adding…" onClick={() => void run(`mode:${s.id}:${m.id}`, [s.id], m.missing, [], [m.id])} />
                    ))}
                    {modes.length > 1 && (
                      <BusyButton className="btn small" busy={busy === `modes:${s.id}`} disabled={!!busy} label="Add to all of them" busyLabel="Adding…" onClick={() => void run(`modes:${s.id}`, [s.id], names, [], [])} />
                    )}
                  </div>
                </div>
              );
            })()}
          </section>
        ))}
      </div>
    </Sheet>
  );
}
