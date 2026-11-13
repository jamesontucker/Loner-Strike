// Small pieces other pages show for things installed from Community, each rendered with one line
// there: the Mods page's "From Community" modes, and the spray maker's community sprays. They load
// with those pages, so they stay light (the chip helpers, plain fetches).

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import { getJson, ON_THIS_COMPUTER, currentTarget } from "../../../api";
import { useNav } from "../../../store";
import { useToast } from "../../../ui";
import { post } from "../common/api";

type Row = { key: string; kind: string; name: string; author: string; url: string; mode: string; update: number | null; pulled: boolean };

function useRows(kind: string) {
  const q = useQuery({
    queryKey: ["tools", "community", "library"],
    queryFn: () => getJson<{ items?: Row[] }>("/api/tools/community/library"),
    enabled: ON_THIS_COMPUTER,
    staleTime: 60_000,
    retry: false,
  });
  return (q.data?.items ?? []).filter((i) => i.kind === kind);
}

function Rows({ rows, what }: { rows: Row[]; what: string }) {
  const toast = useToast();
  const qc = useQueryClient();
  const [busy, setBusy] = useState<string | null>(null);
  const remove = async (r: Row) => {
    setBusy(r.key);
    const res = await post<{ ok: boolean }>("/api/tools/community/uninstall", { key: r.key });
    setBusy(null);
    if (res.error) return toast(res.error, true);
    toast(`${r.name} removed.`);
    qc.invalidateQueries({ queryKey: ["tools", "community"] });
  };
  return (
    <ul className="cm-hook-rows" style={{ listStyle: "none", margin: 0, padding: 0, display: "grid", gap: 6 }}>
      {rows.map((r) => (
        <li key={r.key} style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: "6px 12px" }}>
          <strong>{r.name}</strong>
          <a className="tag" style={{ fontSize: 11, padding: "0 6px", textDecoration: "none" }} href={r.url} target="_blank" rel="noreferrer" title="Opens its page">
            Community{r.author ? ` · ${r.author}` : ""}
          </a>
          {r.update && <span className="tag" style={{ fontSize: 11, padding: "0 6px" }}>Update available</span>}
          <span style={{ flex: 1 }} />
          <button className="btn small" onClick={() => useNav.getState().go("tools", `community:${r.key.replace("-", ":")}`)}>Details</button>
          <button className="btn small" disabled={busy !== null} aria-busy={busy === r.key} onClick={() => remove(r)}>
            {busy === r.key ? <><span className="spinner" aria-hidden="true" />Removing…</> : "Remove"}
          </button>
        </li>
      ))}
      <li className="muted small-note">{what}</li>
    </ul>
  );
}

/** Mods page: the Game Mode Studio modes installed from Community (your own game only). */
export function CommunityModes() {
  const rows = useRows("mode");
  if (!rows.length || currentTarget()) return null;
  return (
    <section>
      <div className="section-head">
        <div className="section-title"><h3>Modes from Community</h3></div>
        <p>Made by others in the Game Mode Studio and built on this computer. They're in your mode list; Remix one in Tools &gt; Community to make it your own.</p>
      </div>
      <Rows rows={rows} what="To put one on a dedicated server, open the server's Community tab." />
    </section>
  );
}

/** Spray maker: sprays installed from Community (the last one installed is the one in the game). */
export function CommunitySprays() {
  const rows = useRows("spray");
  if (!rows.length) return null;
  return (
    <section className="panel" style={{ display: "grid", gap: 6 }}>
      <h3 style={{ margin: 0 }}>Sprays from Community</h3>
      <Rows rows={rows} what="Removing one puts back the spray you had before it." />
    </section>
  );
}
