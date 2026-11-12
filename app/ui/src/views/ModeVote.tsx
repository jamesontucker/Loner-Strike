import { useQuery, useQueryClient } from "@tanstack/react-query";
import { getJsonAt, postJson, targetKey, useActions } from "../api";
import { useTarget } from "../queries";
import type { Online } from "../types";
import { BusyButton, Info, useToast } from "../ui";

type ModeVoteData = { modes: { id: string; name: string }[]; chosen: string[]; custom: boolean; error?: string };

/** Which modes players can vote for (rock the vote and the vote at map end), for the Rules page. */
export function ModeVoteList({ data }: { data: Online }) {
  const target = useTarget();
  const qc = useQueryClient();
  const toast = useToast();
  const { run, busy } = useActions();
  const queryKey = ["modevote", target];
  const q = useQuery({
    queryKey,
    queryFn: ({ signal }) => getJsonAt<ModeVoteData>("/api/modevote", target, signal),
    staleTime: 30000,
  });
  const setting = data.settings.find((s) => s.cvar === "gm_modevote");
  if (!setting) return null;
  const key = `modevote:${targetKey()}`;
  const working = busy(key);
  const send = (body: { modes?: string[]; reset?: boolean }, done: string) =>
    run(key, async () => {
      const r = await postJson<ModeVoteData>("/api/modevote", body);
      if (r.error) return toast(r.error, true);
      qc.setQueryData(queryKey, r);
      toast(done);
    }, "Saving the modes in the vote…");
  const d = q.data && !q.data.error ? q.data : null;
  const chosen = d?.chosen ?? [];
  const tick = (id: string, on: boolean) => {
    if (!d) return;
    // in the order of the mode list, so the file reads like modes.ini
    const next = d.modes.map((m) => m.id).filter((m) => (m === id ? on : chosen.includes(m)));
    send({ modes: next }, on ? "Added to the mode vote" : "Taken out of the mode vote");
  };
  const zombieToo = chosen.includes("zombie");
  return (
    <div className="modevote">
      <h3>
        Modes in the vote
        <Info text="Players pick from these when they rock the vote (type rtv in chat) and at the end of a map. Up to four show each time, the ones players nominated first. With the mode rotation on, these are the modes it goes through, in this order. A mode that needs its own maps, like Deathrun or Base Builder, is left out while no map for it is installed." />
        {working && (
          <span className="muted small-note">
            {" "}
            <span className="spinner" aria-hidden="true" />
            Saving…
          </span>
        )}
      </h3>
      {setting.value === "0" && <p className="muted small-note">Mode voting is off, so rock the vote only picks a map.</p>}
      {q.isError || q.data?.error ? (
        <p className="muted small-note">Couldn't load the list of modes.</p>
      ) : !d ? (
        <p className="muted small-note">Loading the modes…</p>
      ) : (
        <>
          <div className="share-with">
            {d.modes.map((m) => (
              <label key={m.id} className="follow-toggle small-note">
                <input type="checkbox" checked={chosen.includes(m.id)} disabled={working} onChange={(e) => tick(m.id, e.target.checked)} /> {m.name}
                {m.id === "zp5" && !zombieToo && <span className="muted"> (shows as Zombie Plague)</span>}
              </label>
            ))}
          </div>
          {d.custom && (
            <p className="small-note">
              <BusyButton className="btn small" busy={working} label="Use the default list" busyLabel="Saving…" onClick={() => send({ reset: true }, "Back to the default list")} />
            </p>
          )}
        </>
      )}
    </div>
  );
}
