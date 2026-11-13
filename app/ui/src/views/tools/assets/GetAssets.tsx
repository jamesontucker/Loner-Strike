// Getting more: GameBanana's weapon skins, player models and textures through the app's own
// Get more (the same checks, size limits, backups and Remove), and your own .wad or .mdl
// files, read by the browser's parsers before anything is copied.

import { useQueryClient } from "@tanstack/react-query";
import { Suspense, useState } from "react";
import { getJson, usePoll } from "../../../api";
import type { GbJob } from "../../../types";
import { BusyButton, lazyPage, Skeleton, useToast } from "../../../ui";
import { addOwnFile } from "./assetsApi";

const GetMore = lazyPage(() => import("../../GetMore").then((m) => ({ default: m.GetMore })));

export function GetAssets() {
  const toast = useToast();
  const qc = useQueryClient();
  const [busy, setBusy] = useState<"wad" | "model" | null>(null);
  const [job, setJob] = useState<{ id: string; label: string } | null>(null);

  const refresh = () => {
    qc.invalidateQueries({ queryKey: ["tools", "assets", "wads"] });
    qc.invalidateQueries({ queryKey: ["tools", "assets", "models"] });
  };

  // a file going in through Get more's installer: wait for it to finish
  usePoll(async () => {
    if (!job) return;
    const jobs = await getJson<GbJob[]>("/api/gb/jobs");
    const j = Array.isArray(jobs) ? jobs.find((x) => x.id === job.id) : undefined;
    if (!j || (j.state !== "done" && j.state !== "error")) return;
    setJob(null);
    toast(j.message || (j.state === "done" ? "Added." : "Couldn't add it."), j.state === "error");
    refresh();
  }, 1000, [job?.id], !!job);

  const add = async (kind: "wad" | "model") => {
    setBusy(kind);
    const r = await addOwnFile(kind);
    setBusy(null);
    if (r.cancelled) return;
    if (r.error || !r.ok) return toast(r.error || "Couldn't add that file.", true);
    if (r.job) setJob({ id: r.job, label: r.message || "Adding…" });
    else {
      toast(r.message || "Added.");
      refresh();
    }
  };

  return (
    <div className="stack">
      <div className="panel">
        <h3>Add your own</h3>
        <p className="muted measure">
          A texture file (.wad) or a model (.mdl) from this computer. It's checked first, then copied into the game. A model that replaces one of the game's keeps the original, and it comes back when you remove the new one in Get more.
        </p>
        <div className="row">
          <BusyButton className="btn" busy={busy === "wad"} label="Add a WAD…" busyLabel="Opening…" onClick={() => add("wad")} disabled={!!job} />
          <BusyButton className="btn" busy={busy === "model"} label="Add a model…" busyLabel="Opening…" onClick={() => add("model")} disabled={!!job} />
          {job && <span className="muted" role="status"><span className="spinner" aria-hidden /> {job.label}</span>}
        </div>
      </div>
      <Suspense fallback={<Skeleton rows={4} />}>
        <GetMore only={["skins", "players", "textures"]} title="From GameBanana" onInstalled={refresh} />
      </Suspense>
    </div>
  );
}
