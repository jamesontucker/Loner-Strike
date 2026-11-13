// Installed: everything that came from Community, where it went (maps in your game, prefabs in
// the editor, modes in your mode list), with Update when a newer version is listed, Remove (the
// way it went in, each file kept by the safety net first), and Remix for Game Mode Studio modes.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo, useState } from "react";
import { useNav } from "../../../store";
import { BusyButton, LoadError, Segmented, useToast } from "../../../ui";
import { errorText, useBusy } from "../common/api";
import { Empty, Loading } from "../common";
import { KIND_LABEL, ago, library, remix, uninstall, update, type Details, type Installed, type Model } from "./communityApi";

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
      toast(`${it.name} removed.`);
      refresh();
    });
  const upd = () =>
    run("Updating…", async () => {
      const r = await update(it.key);
      if (r.error) return toast(r.error, true);
      toast(r.job ? `Downloading the new version of ${it.name}…` : `${it.name} is up to date.`);
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
        {it.update && (
          <BusyButton className="btn primary" busy={working === "Updating…"} busyLabel="Updating…" label="Update" disabled={working !== null} onClick={upd} />
        )}
        {it.kind === "mode" && (
          <BusyButton className="btn" busy={working === "Making your copy…"} busyLabel="Making your copy…" label="Remix in Game Mode Studio" disabled={working !== null} onClick={rmx} />
        )}
        {!sure ? (
          <button className="btn" disabled={working !== null} onClick={() => setSure(true)}>Remove</button>
        ) : (
          <span className="cm-confirm-inline" role="alertdialog" aria-label="Remove it?">
            <span>Remove {it.name}{it.kind === "map" ? " and its files" : ""}? Servers carrying it drop it too.</span>
            <BusyButton className="btn danger" busy={working === "Removing…"} busyLabel="Removing…" label="Remove" onClick={remove} />
            <button className="btn" onClick={() => setSure(false)}>Keep it</button>
          </span>
        )}
      </div>
      {it.update && <p className="muted small-note">A newer version is listed. Updating removes this one first, then installs the new one with the same checks.</p>}
      {it.pulled && <p className="cm-warn small-note">The Loner-Strike team pulled this from Community since you installed it. You may want to remove it.</p>}
    </div>
  );
}

export default function InstalledList({ onOpen, onBrowse }: { onOpen: (model: Model, id: number) => void; onBrowse: () => void }) {
  const q = useQuery({ queryKey: ["tools", "community", "library"], queryFn: ({ signal }) => library(signal), retry: false });
  const [kind, setKind] = useState("all");
  const items = useMemo(() => (q.data?.items ?? []).filter((i) => kind === "all" || i.kind === kind), [q.data, kind]);
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
        </span>
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
              </div>
              <InstalledActions it={it} />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
