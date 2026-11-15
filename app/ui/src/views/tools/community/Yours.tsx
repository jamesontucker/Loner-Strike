// Yours: the creations this computer registered (sent to the Loner-Strike list, or waiting to
// be), and the packages made so far.

import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { copyText } from "../../../api";
import { BusyButton, LoadError, useToast } from "../../../ui";
import { errorText, post } from "../common/api";
import { Empty, Loading } from "../common";
import { KIND_LABEL, ago, mine, reveal, sizeText, type Mine } from "./communityApi";

function MineState({ m }: { m: Mine }) {
  if (m.state === "listed" || m.state === "sent") return <span className="cm-ok">Listed in Community (it shows up once the list is next updated, within a few minutes)</span>;
  if (m.state === "pending") return <span className="cm-warn">Waiting for a moderator to look at it</span>;
  if (m.state === "refused") return <span className="cm-bad">Refused: {m.error || "the Loner-Strike list turned it down."} Fix it and add the link again in Publish.</span>;
  return <span className="cm-warn">Waiting to be sent (the Loner-Strike list didn't answer; it's tried again each time you open this)</span>;
}

type UnlistAnswer = { state?: "needs-proof" | "unlisted" | "listed" | null; code?: string | null; how?: string | null; note?: string | null; error?: string };

/** Unlist / List again for one of yours: the list service gives a code to put on the GameBanana
 *  page (that's the proof it's yours, as for registering), then does it once the page shows it. */
function UnlistControl({ m }: { m: Mine }) {
  const toast = useToast();
  const key = `${m.item.model === "Spray" ? "spray" : "mod"}:${m.item.gb}`;
  const [listed, setListed] = useState(true);
  const [proof, setProof] = useState<UnlistAnswer | null>(null);
  const [busy, setBusy] = useState(false);
  const ask = async () => {
    setBusy(true);
    try {
      const action = listed ? "unlist" : "list";
      const r = await post<UnlistAnswer>("/api/tools/community/unlist", { key, action });
      if (r.error) {
        // (the list knows better than this computer which way it is)
        if (/already unlisted/i.test(r.error)) setListed(false);
        else if (/already on the list/i.test(r.error)) setListed(true);
        toast(r.error, true);
        return;
      }
      if (r.state === "needs-proof") return setProof(r);
      setProof(null);
      setListed(r.state === "listed");
      toast(r.state === "unlisted" ? `${m.item.name} is unlisted. ${r.note ?? ""}` : `${m.item.name} is on the list again. ${r.note ?? ""}`);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="stack-tight">
      <BusyButton className="btn small" busy={busy} busyLabel="Checking…" label={proof ? "I've added it, check again" : listed ? "Unlist" : "List again"} onClick={() => void ask()} />
      {proof?.code && (
        <div className="small-note">
          <p>
            {proof.how} <code>{proof.code}</code>{" "}
            <button type="button" className="btn small" onClick={async () => toast((await copyText(proof.code ?? "")) ? "Copied." : "Couldn't copy it.")}>Copy</button>
          </p>
          {proof.note && <p className="muted">{proof.note}</p>}
        </div>
      )}
    </div>
  );
}

export default function Yours({ onPublish }: { onPublish: () => void }) {
  const toast = useToast();
  const q = useQuery({ queryKey: ["tools", "community", "mine"], queryFn: ({ signal }) => mine(signal), retry: false });
  if (q.isPending) return <Loading text="Loading what you've published…" />;
  if (q.isError) return <LoadError text={errorText(q.error, "Couldn't load what you've published.")} onRetry={() => q.refetch()} />;
  const { mine: list, packages } = q.data;
  if (!list.length && !packages.length)
    return (
      <Empty title="Nothing published yet" text="Make a package of a map you built, put it on GameBanana, and it's listed here.">
        <button className="btn primary" onClick={onPublish}>Publish a map</button>
      </Empty>
    );
  const show = async (file: string) => {
    const r = await reveal(file);
    if (r.error) toast(r.error, true);
  };
  return (
    <div className="stack-tight">
      <div className="row">
        <BusyButton className="btn" busy={q.isFetching} label="Refresh" busyLabel="Loading…" onClick={() => q.refetch()} />
      </div>
      {list.length > 0 && (
        <section className="panel">
          <h3>On GameBanana</h3>
          <ul className="cm-mine">
            {list.map((m) => (
              <li key={`${m.item.model}:${m.item.gb}`}>
                <div>
                  <strong>{m.item.name}</strong> <span className="muted small-note">{KIND_LABEL[m.item.kind]} · {ago(m.when)}</span>
                  <div className="small-note">
                    <MineState m={m} />
                  </div>
                </div>
                <div className="row">
                  <a className="btn small" href={m.url} target="_blank" rel="noreferrer">Open page</a>
                  {(m.state === "listed" || m.state === "sent") && <UnlistControl m={m} />}
                </div>
              </li>
            ))}
          </ul>
        </section>
      )}
      {packages.length > 0 && (
        <section className="panel">
          <h3>Packages</h3>
          <ul className="cm-mine">
            {packages.map((p) => (
              <li key={p.file}>
                <div>
                  <strong>{p.name}</strong> <span className="muted small-note"><code>{p.file}</code> · {sizeText(p.size)} · {ago(p.made)}</span>
                </div>
                <button className="btn small" onClick={() => show(p.file)}>Show in folder</button>
              </li>
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}
