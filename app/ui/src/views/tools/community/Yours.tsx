// Yours: the creations this computer registered (sent to the Loner-Strike list, or waiting to
// be), and the packages made so far.

import { useQuery } from "@tanstack/react-query";
import { BusyButton, LoadError, useToast } from "../../../ui";
import { errorText } from "../common/api";
import { Empty, Loading } from "../common";
import { KIND_LABEL, ago, mine, reveal, sizeText } from "./communityApi";

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
                    {m.state === "sent" ? <span className="cm-ok">Sent to the Loner-Strike list</span> : <span className="cm-warn">Waiting to be sent (the list isn't taking entries from the app yet; it's tried again each time you open this)</span>}
                  </div>
                </div>
                <a className="btn small" href={m.url} target="_blank" rel="noreferrer">Open page</a>
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
