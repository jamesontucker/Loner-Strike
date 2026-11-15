// Publish's credits: where each custom file the map ships came from (tools/credits.rs), who gets
// credit, and its terms. "Ask first" warns; "No" stops Publish until you record that you have the
// creator's permission (with a note: who said yes, where, when). A file nobody knows gets a credit you type.

import { useState } from "react";
import { BusyButton, useToast } from "../../../ui";
import { useBusy } from "../common/api";
import { notePermission, type CreditAsset } from "./communityApi";

const SOURCE: Record<string, string> = {
  gamebanana: "GameBanana download",
  community: "Loner-Strike Community",
  manual: "Credited by you",
  unknown: "Where's this from?",
};
const VERDICT: Record<string, [string, string]> = { ok: ["ok", "Can be shared"], ask: ["warn", "Ask first"], no: ["bad", "Not allowed"] };

function Row({ a, onChanged }: { a: CreditAsset; onChanged: () => void }) {
  const toast = useToast();
  const [open, setOpen] = useState(false);
  const [note, setNote] = useState("");
  const [who, setWho] = useState("");
  const [page, setPage] = useState("");
  const { working, run } = useBusy();
  const granted = !!a.permission?.granted;
  const v = granted ? (["ok", "Permission recorded"] as [string, string]) : VERDICT[a.verdict] ?? ["ok", ""];
  const save = () =>
    run("Saving…", async () => {
      const r =
        a.source === "unknown"
          ? await notePermission({ key: `sha:${a.sha256}`, granted: false, note: "", authors: [{ name: who.trim(), role: "", url: "" }], page: page.trim() })
          : await notePermission({ key: a.page, granted: true, note: note.trim() });
      if (r.error) return toast(r.error, true);
      setOpen(false);
      onChanged();
    });
  const clear = () =>
    run("Saving…", async () => {
      const r = await notePermission({ key: a.page, granted: false, note: "" });
      if (r.error) return toast(r.error, true);
      onChanged();
    });
  const canNote = a.source === "unknown" || (a.page && (a.verdict === "ask" || a.verdict === "no" || granted));
  return (
    <li className="cm-credit">
      <div className="cm-credit-main">
        <code>{a.rel}</code>
        <span className="small-note">
          {SOURCE[a.source] ?? a.source}
          {a.name ? `: ${a.name}` : ""}
          {a.authors.length ? ` · ${a.authors.map((x) => x.name).join(", ")}` : ""}
          {a.page ? <> · <a href={a.page} target="_blank" rel="noreferrer">page</a></> : null}
        </span>
        {v[1] && <span className={`cm-line ${v[0]} small-note`}>{v[1]}{granted && a.permission?.note ? `: ${a.permission.note}` : ""}</span>}
        {!granted && a.verdict !== "ok" && <span className="muted small-note">{a.why}</span>}
      </div>
      {canNote && !open && (
        <div className="row">
          {granted ? (
            <BusyButton className="btn small" busy={working !== null} busyLabel="Saving…" label="Clear permission" onClick={clear} />
          ) : (
            <button className="btn small" onClick={() => setOpen(true)}>{a.source === "unknown" ? "Add credit…" : "Permission granted…"}</button>
          )}
        </div>
      )}
      {open && (
        <form className="cm-credit-form" onSubmit={(e) => { e.preventDefault(); save(); }}>
          {a.source === "unknown" ? (
            <>
              <input value={who} maxLength={60} onChange={(e) => setWho(e.target.value)} placeholder="Who made it" aria-label="Who made it" />
              <input value={page} maxLength={200} onChange={(e) => setPage(e.target.value)} placeholder="Its GameBanana page (optional)" aria-label="Its page" />
            </>
          ) : (
            <input value={note} maxLength={300} onChange={(e) => setNote(e.target.value)} placeholder="How you got permission: who said yes, where, when" aria-label="How you got permission" />
          )}
          <BusyButton type="submit" className="btn small primary" busy={working !== null} busyLabel="Saving…" label="Save" disabled={a.source === "unknown" ? !who.trim() : !note.trim()} />
          <button type="button" className="btn small" onClick={() => setOpen(false)}>Cancel</button>
        </form>
      )}
    </li>
  );
}

/** The shipped files that need credit (stock and your own files are left out). */
export function CreditsPanel({ assets, onChanged }: { assets: CreditAsset[]; onChanged: () => void }) {
  const shown = assets.filter((a) => a.source !== "stock" && a.source !== "mine");
  if (!shown.length) return null;
  return (
    <section className="cm-credits">
      <h4>Credits for the files it uses</h4>
      <p className="muted small-note">Loner-Strike traces each file to where it came from. The credits go into the package's README, the GameBanana form and the Community listing.</p>
      <ul>{shown.map((a) => <Row key={a.rel} a={a} onChanged={onChanged} />)}</ul>
    </section>
  );
}
