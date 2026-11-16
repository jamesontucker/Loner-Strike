// Installed: everything that came from Community, where it went (maps in your game, prefabs in
// the editor, modes in your mode list), with Update when a newer version is listed, Remove (the
// way it went in, each file kept by the safety net first), and Remix for Game Mode Studio modes.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { useNav } from "../../../store";
import { BusyButton, LoadError, Segmented, useToast } from "../../../ui";
import { errorText, useBusy } from "../common/api";
import { Empty, Loading } from "../common";
import { useOrigins } from "./chip";
import { KIND_LABEL, ago, goBack, library, remix, uninstall, update, updateAll, updateNotes, type Details, type Installed, type Model } from "./communityApi";

const keyOf = (model: Model, id: number) => `${model === "Spray" ? "spray" : "mod"}-${id}`;

function where(it: Installed): string {
  switch (it.kind) {
    case "map":
      return it.maps.length ? `In your maps: ${it.maps.join(", ")}` : "In your maps";
    case "mode":
      return it.mode ? `In your modes as "${it.mode}"` : "In your modes";
    case "prefab":
      return `In the map editor's prefab library${it.prefabs.length > 1 ? ` (${it.prefabs.length} prefabs)` : ""}`;
    case "spray":
      return "Your spray in the game";
    case "texture":
      return "Texture files in your game";
    default:
      return "";
  }
}

/** Update / Remove / Remix for one installed creation (its list row and its page). */
export function InstalledActions({ d, it: given, onChanged }: { d?: Details; it?: Installed; onChanged?: () => void }) {
  const toast = useToast();
  const qc = useQueryClient();
  const lib = useQuery({ queryKey: ["tools", "community", "library"], queryFn: ({ signal }) => library(signal), staleTime: 30_000, retry: false, enabled: !given });
  const it = given ?? (d ? lib.data?.items.find((x) => x.key === keyOf(d.model, d.id)) : undefined);
  const [sure, setSure] = useState(false);
  const { working, run } = useBusy();
  if (!it) return null;
  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["tools", "community"] });
    onChanged?.();
  };
  const remove = () =>
    run("Removing…", async () => {
      const r = await uninstall(it.key);
      setSure(false);
      if (r.error) return toast(r.error, true);
      // a map that comes with Loner-Strike: only Community's record went, the map stays
      toast(r.kept?.length ? `${r.kept.join(", ")} comes with Loner-Strike, so it stays installed. It's no longer listed as installed from Community.` : `${it.name} removed.`);
      refresh();
    });
  const [notes, setNotes] = useState(false);
  const upd = () =>
    run("Updating…", async () => {
      const r = await update(it.key);
      setNotes(false);
      if (r.error) return toast(r.error, true);
      toast(r.state === "waiting" ? r.message ?? `${it.name} is in use: the new version goes in at the next map change.` : r.job ? `Downloading the new version of ${it.name}…` : `${it.name} is updated.`);
      setTimeout(refresh, r.job ? 4000 : 0);
    });
  const back = () =>
    run("Going back…", async () => {
      const r = await goBack(it.key);
      if (r.error) return toast(r.error, true);
      toast(`${it.name}: the earlier version is back.`);
      setTimeout(refresh, r.job ? 4000 : 0);
    });
  const rmx = () =>
    run("Making your copy…", async () => {
      const r = await remix(it.key);
      if (r.error) return toast(r.error, true);
      toast(`"${r.name}" is in your Game Mode Studio modes, with credit to ${it.author || "its maker"}.`);
      useNav.getState().go("tools", `studio:${r.id}`);
    });
  return (
    <div className="cm-installed">
      <div className="row cm-actions">
        {it.update && !notes && <button className="btn primary" disabled={working !== null} onClick={() => setNotes(true)}>Update…</button>}
        {it.pending > 0 && <span className="small-note cm-warn">New version waiting: it goes in at the next map change.</span>}
        {it.canGoBack && (
          <BusyButton className="btn" busy={working === "Going back…"} busyLabel="Going back…" label="Go back to the previous version" disabled={working !== null} onClick={back} />
        )}
        {it.kind === "mode" && (
          <BusyButton className="btn" busy={working === "Making your copy…"} busyLabel="Making your copy…" label="Remix in Game Mode Studio" disabled={working !== null} onClick={rmx} />
        )}
        {!sure ? (
          <button className="btn" disabled={working !== null} onClick={() => setSure(true)}>Remove</button>
        ) : (
          <span className="cm-confirm-inline" role="alertdialog" aria-label="Remove it?">
            <span>{it.builtin ? <>Remove {it.name} from Community? It comes with Loner-Strike, so the map stays installed.</> : <>Remove {it.name}{it.kind === "map" ? " and its files" : ""}? Servers carrying it drop it too.</>}</span>
            <BusyButton className="btn danger" busy={working === "Removing…"} busyLabel="Removing…" label="Remove" onClick={remove} />
            <button className="btn" onClick={() => setSure(false)}>Keep it</button>
          </span>
        )}
      </div>
      {it.update && notes && (
        <div className="cm-confirm" role="dialog" aria-label={`Update ${it.name}`}>
          <UpdateNotes model={it.model} id={it.gb} />
          <p className="muted small-note">
            The version you have is kept until the new one works, and you can go back to it in one click. If it's in use now (the map loaded, the mode running) it goes in at the next map change; servers carrying it get it when nobody's playing it there. Things you copied into your own work (placed prefabs, remixes) don't change.
          </p>
          <div className="row">
            <BusyButton className="btn primary" busy={working === "Updating…"} busyLabel="Updating…" label="Update" onClick={upd} />
            <button className="btn" onClick={() => setNotes(false)}>Not now</button>
          </div>
        </div>
      )}
      {it.gone && !it.pulled && <p className="muted small-note">{it.gone}</p>}
      {it.pulled && <p className="cm-warn small-note">The Loner-Strike team pulled this from Community since you installed it. You may want to remove it.</p>}
    </div>
  );
}

/** The creator's update notes from GameBanana (what changed), newest first. */
function UpdateNotes({ model, id }: { model: Model; id: number }) {
  const q = useQuery({ queryKey: ["tools", "community", "notes", model, id], queryFn: ({ signal }) => updateNotes(model, id, signal), retry: false, staleTime: 300_000 });
  if (q.isPending) return <Loading text="Reading what changed…" />;
  if (q.isError || !q.data.notes.length) return <p className="muted small-note">The creator didn't write update notes on GameBanana.</p>;
  return (
    <div className="cm-notes">
      {q.data.notes.slice(0, 3).map((n, i) => (
        <div key={i}>
          <strong>{n.title || "Update"}</strong>{n.version ? ` · ${n.version}` : ""}<span className="muted small-note"> · {ago(n.date)}</span>
          {n.changes.length > 0 && (
            <ul className="cm-facts">
              {n.changes.map((c, j) => <li key={j}>{c.cat ? <span className="muted">{c.cat}: </span> : null}{c.text}</li>)}
            </ul>
          )}
          {n.text && <p className="small-note cm-text">{n.text}</p>}
        </div>
      ))}
    </div>
  );
}

export default function InstalledList({ onOpen, onBrowse }: { onOpen: (model: Model, id: number) => void; onBrowse: () => void }) {
  const q = useQuery({ queryKey: ["tools", "community", "library"], queryFn: ({ signal }) => library(signal), retry: false });
  const [kind, setKind] = useState("all");
  // (origin: the Studio modes you made from these)
  const origins = useOrigins();
  const remixedFrom = (it: Installed) =>
    Object.entries(origins?.modes ?? {}).filter(([, o]) => o.kind === "remix" && o.url === it.url).map(([id]) => id).join(", ");
  // updates first, then the newest installs
  const items = useMemo(
    () => (q.data?.items ?? []).filter((i) => kind === "all" || i.kind === kind).sort((a, b) => Number(!!b.update) - Number(!!a.update) || b.installed - a.installed),
    [q.data, kind],
  );
  const toast = useToast();
  const qc = useQueryClient();
  const { working, run } = useBusy();
  const all = () =>
    run("Updating everything…", async () => {
      const r = await updateAll();
      if (r.error) return toast(r.error, true);
      const bad = r.updates.filter((u) => u.error);
      const waiting = r.updates.filter((u) => u.result?.state === "waiting").length;
      toast(bad.length ? `${bad.length} couldn't be updated: ${bad[0].name}: ${bad[0].error}` : `${r.updates.length - waiting} updating${waiting ? `, ${waiting} waiting for the next map change` : ""}.`, bad.length > 0);
      setTimeout(() => qc.invalidateQueries({ queryKey: ["tools", "community"] }), 4000);
    });
  if (q.isPending) return <Loading text="Loading what you installed…" />;
  if (q.isError) return <LoadError text={errorText(q.error, "Couldn't load what you installed.")} onRetry={() => q.refetch()} />;
  if (!q.data.items.length)
    return (
      <Empty title="Nothing installed from Community yet" text="Try a map, mode or prefab from Browse and it shows up here, and in its usual place with a Community tag.">
        <button className="btn primary" onClick={onBrowse}>Browse creations</button>
      </Empty>
    );
  const updates = q.data.items.filter((i) => i.update).length;
  return (
    <div className="stack-tight">
      <div className="toolbar cm-toolbar">
        <Segmented label="Kind" values="all map mode prefab spray texture" labels="All Maps Modes Prefabs Sprays Textures" current={kind} onPick={setKind} />
        <span className="muted small-note" aria-live="polite">
          {q.data.items.length} installed{updates ? ` · ${updates} update${updates === 1 ? "" : "s"} available` : ""}
          {q.data.waiting ? ` · ${q.data.waiting} waiting for the next map change` : ""}
        </span>
        {updates > 1 && <BusyButton className="btn primary" busy={working !== null} busyLabel={working ?? "Updating…"} label="Update all" onClick={all} />}
      </div>
      {items.length === 0 ? (
        <Empty plain title="None of these" text="Pick All to see everything you installed." />
      ) : (
        <ul className="cm-lib" aria-label="Installed from Community">
          {items.map((it) => (
            <li key={it.key} className="panel cm-lib-row">
              <div className="cm-lib-main">
                <button className="cm-lib-name" onClick={() => onOpen(it.model, it.gb)} title="Open its page">{it.name}</button>
                <span className="muted small-note">
                  {KIND_LABEL[it.kind]}{it.author ? ` by ${it.author}` : ""} · installed {ago(it.installed)}
                </span>
                <span className="small-note">{where(it)}</span>
                {it.update && <span className="tag cm-badge cm-badge-update">Update available</span>}
                {remixedFrom(it) && <span className="tag cm-chip">You remixed this: {remixedFrom(it)}</span>}
              </div>
              <InstalledActions it={it} />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
