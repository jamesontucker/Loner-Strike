// The map doctor: pick a map (a downloaded one, usually) and see what it's missing or has
// broken: texture files, models, sounds, sprites, sky, the download list players get, the
// bots' nav. Fixes that are safe are one click; for the rest it says where to get the files.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState, type ReactNode } from "react";
import { useSetupJob, waitForSetupJob } from "../../../queries";
import { useNav } from "../../../store";
import { useTools, useToolArg } from "../../../stores/tools";
import type { MapInfo } from "../../../types";
import { Banner, BusyButton, LoadError, Skeleton, useToast } from "../../../ui";
import { JobProgress } from "../../JobProgress";
import type { ToolProps } from "../registry";
import { checkMap, copyFiles, learnMap, learnResult, writeRes, type FileRow, type Report } from "./doctorApi";
import { MapList } from "./MapList";
import "./doctor.css";

export default function Doctor({ maps }: ToolProps) {
  const arg = useToolArg();
  const [map, setMap] = useState<string | null>(arg);
  useEffect(() => {
    if (arg) setMap(arg);
  }, [arg]);
  if (!map)
    return (
      <div className="stack-tight">
        <p className="muted measure">
          Pick a map to check. The doctor looks for texture files, models, sounds and sky it needs, the list of files players download with it, and the bots' nav, and fixes what it safely can.
        </p>
        <MapList maps={maps} onPick={setMap} placeholder="Pick a map to check" note={(m: MapInfo) => (m.nav ? null : "no bot nav yet")} />
      </div>
    );
  return <Checkup map={map} onBack={() => setMap(null)} />;
}

type Tone = "ok" | "warn" | "bad";

function Line({ tone, children, action }: { tone: Tone; children: ReactNode; action?: ReactNode }) {
  const word = tone === "ok" ? "Fine" : tone === "warn" ? "Worth a look" : "Problem";
  return (
    <div className={`doc-line ${tone}`}>
      <span className="doc-mark" role="img" aria-label={word} title={word}>{tone === "ok" ? "✓" : tone === "warn" ? "!" : "✕"}</span>
      <div className="doc-text">{children}</div>
      {action && <div className="doc-action">{action}</div>}
    </div>
  );
}

const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

function FileList({ rows }: { rows: FileRow[] }) {
  return (
    <ul className="doc-files">
      {rows.map((f) => (
        <li key={f.path} className={f.status}>
          <code>{f.path}</code>
          <span className="muted small-note">
            {f.status === "ok" ? `in ${f.folder}${f.stock ? " (every player has it)" : ""}` : f.status === "elsewhere" ? `only in ${f.folder}` : "not on this computer"}
            {f.error ? ` · can't be read: ${f.error}` : ""}
          </span>
        </li>
      ))}
    </ul>
  );
}

function Checkup({ map, onBack }: { map: string; onBack: () => void }) {
  const toast = useToast();
  const qc = useQueryClient();
  const q = useQuery({ queryKey: ["tools", "doctor", map], queryFn: () => checkMap(map), retry: false });
  const [busy, setBusy] = useState<string | null>(null);
  const job = useSetupJob();
  const learning = job.data?.state === "running" && job.data.kind === "navlearn";

  const reload = () => qc.invalidateQueries({ queryKey: ["tools", "doctor", map] });

  const act = async (key: string, fn: () => Promise<{ ok?: boolean; error?: string; message?: string }>, fail: string) => {
    setBusy(key);
    const r = await fn();
    setBusy(null);
    if (r.error || !r.ok) toast(r.error || fail, true);
    else toast(r.message || "Done.");
    reload();
  };

  const teach = async (relearn: boolean) => {
    setBusy("learn");
    const r = await learnMap(map, relearn);
    if (r.error) {
      setBusy(null);
      return toast(r.error, true);
    }
    const j = await waitForSetupJob();
    const res = await learnResult();
    setBusy(null);
    const msg = res?.result?.message;
    if (!j) toast("Lost track of the bots learning the map. Check again in a minute.", true);
    else if (j.state === "done") toast(msg || "Bots learned the map.");
    else toast(msg || j.error || "Bots couldn't learn the map.", true);
    reload();
  };

  const nameplaces = () => {
    useTools.getState().setTool("callouts");
    useTools.setState({ arg: map });
  };

  const head = (
    <div className="row between doc-head">
      <div>
        <h3 className="doc-title">{map}</h3>
        {q.data?.title && q.data.title !== map && <span className="muted">{q.data.title}</span>}
      </div>
      <div className="row">
        <button className="btn" onClick={onBack}>Check another map</button>
        <BusyButton className="btn" busy={q.isFetching} label="Check again" busyLabel="Checking…" onClick={() => q.refetch()} />
      </div>
    </div>
  );

  if (q.isPending)
    return (
      <div className="panel">
        {head}
        <p className="muted" role="status"><span className="spinner" aria-hidden="true" /> Checking {map}…</p>
        <Skeleton rows={4} />
      </div>
    );
  if (q.isError) return <div className="panel">{head}<LoadError text={(q.error as Error).message || "Couldn't check that map."} onRetry={() => q.refetch()} /></div>;
  const r: Report = q.data;
  const locked = r.inUse;
  const off = locked || busy !== null;

  // ---- what's where
  const wads = r.textures.wads;
  const others = r.files;
  const elsewhere = [...wads, ...others].filter((f) => f.status === "elsewhere");
  const missing = [...wads, ...others].filter((f) => f.status === "missing");
  const byKind = (k: FileRow["kind"]) => others.filter((f) => f.kind === k);
  const kinds: [FileRow["kind"], string][] = [["sky", "Sky"], ["model", "Models"], ["sprite", "Sprites"], ["sound", "Sounds"], ["overview", "Overview"]];
  const search = `https://gamebanana.com/search?_sSearchString=${encodeURIComponent(r.name)}`;

  const copyAll = () => act("copy", () => copyFiles(map, elsewhere.map((f) => f.path)), "Couldn't copy the files.");

  return (
    <div className="doc stack-tight">
      <div className="panel">
        {head}
        {locked && <Banner>The game has {map} loaded right now. Change to another map (or close the game) to use the fixes.</Banner>}
        <p className={`doc-summary ${r.problems ? "warn" : "ok"}`}>
          {r.problems === 0 ? "Nothing wrong found. This map has everything it needs." : `${plural(r.problems, "thing")} to look at below.`}
        </p>
        <p className="muted small-note">Looked for files where the game looks: {r.looksIn.join(", then ")}.</p>
      </div>

      <section className="panel">
        <h3>Map file</h3>
        {r.bsp.ok ? (
          <Line tone="ok">The map file reads fine{r.size ? ` (${(r.size / 1048576).toFixed(1)} MB)` : ""}.</Line>
        ) : (
          <Line tone="bad">
            The map file is damaged: {r.bsp.error}. The game may not load it. Getting it again usually fixes this.
          </Line>
        )}
      </section>

      <section className="panel">
        <h3>Textures</h3>
        {r.textures.missing.length === 0 ? (
          <Line tone="ok">
            All {r.textures.total} textures are there{r.textures.embedded ? ` (${r.textures.embedded} built into the map)` : ""}.
          </Line>
        ) : (
          <Line tone="bad">
            {plural(r.textures.missing.length, "texture")} can't be found, so those walls show as a checkerboard
            {r.textures.fixable ? `; copying the texture files below brings back ${r.textures.fixable}` : ""}.
            <details className="doc-more">
              <summary>Show them</summary>
              <p className="doc-tex">{r.textures.missing.join(", ")}</p>
            </details>
          </Line>
        )}
        {wads.length > 0 && (
          <>
            <h4 className="doc-sub">Texture files the map uses</h4>
            <FileList rows={wads} />
          </>
        )}
      </section>

      <section className="panel">
        <h3>Sky, models, sprites and sounds</h3>
        {others.length === 0 ? (
          <Line tone="ok">The map doesn't use any extra files.</Line>
        ) : (
          kinds.map(([k, label]) => {
            const rows = byKind(k);
            if (!rows.length) return null;
            const bad = rows.filter((f) => f.status !== "ok");
            return (
              <div key={k}>
                <Line tone={bad.some((f) => f.status === "missing") ? "bad" : bad.length ? "warn" : "ok"}>
                  <strong>{label}</strong>: {bad.length ? `${bad.length} of ${rows.length} aren't where the game looks` : `all ${rows.length} there`}.
                  <details className="doc-more" open={bad.length > 0 && bad.length <= 8}>
                    <summary>{bad.length ? "Show files" : "Show them"}</summary>
                    <FileList rows={bad.length ? [...bad, ...rows.filter((f) => f.status === "ok")] : rows} />
                  </details>
                </Line>
              </div>
            );
          })
        )}
      </section>

      {(elsewhere.length > 0 || missing.length > 0) && (
        <section className="panel">
          <h3>Fix missing files</h3>
          {elsewhere.length > 0 && (
            <Line
              tone="warn"
              action={<BusyButton className="btn primary" busy={busy === "copy"} disabled={off} label={`Copy ${plural(elsewhere.length, "file")}`} busyLabel="Copying…" onClick={copyAll} />}
            >
              {plural(elsewhere.length, "file is", "files are")} on this computer, but in another game's folder ({[...new Set(elsewhere.map((f) => f.folder))].join(", ")}), where {r.game} doesn't look. Copying puts them into {r.game}. Nothing is replaced.
            </Line>
          )}
          {missing.length > 0 && (
            <Line
              tone="bad"
              action={
                <div className="row">
                  {r.gamebanana ? (
                    <a className="btn primary" href={r.gamebanana.url} target="_blank" rel="noreferrer">Open its GameBanana page</a>
                  ) : (
                    <a className="btn" href={search} target="_blank" rel="noreferrer">Search GameBanana</a>
                  )}
                  <button className="btn" onClick={() => useNav.getState().go("more")}>Get more</button>
                </div>
              }
            >
              {plural(missing.length, "file isn't", "files aren't")} on this computer. They usually come with the map's download: getting it again {r.gamebanana ? `("${r.gamebanana.name}")` : "from where you found it"} and installing it over this one fixes it.
            </Line>
          )}
        </section>
      )}

      <section className="panel">
        <h3>Download list for players (.res)</h3>
        {r.res.stockMap ? (
          <Line tone="ok">This is one of the game's own maps: every player has it and its files, so it needs no list.</Line>
        ) : r.res.wanted === 0 && !r.res.exists ? (
          <Line tone="ok">The map only uses files every player has, so it doesn't need a list.</Line>
        ) : r.res.missing.length === 0 ? (
          <Line tone="ok">
            {r.name}.res lists {plural(r.res.listed, "file")}: players joining your server download everything the map needs.
            {r.res.broken.length > 0 && <> It also names {plural(r.res.broken.length, "file")} that {r.res.broken.length === 1 ? "isn't" : "aren't"} on this computer; updating it leaves {r.res.broken.length === 1 ? "that" : "those"} out.</>}
          </Line>
        ) : (
          <Line
            tone="warn"
            action={<BusyButton className="btn primary" busy={busy === "res"} disabled={off} label={r.res.exists ? "Update the list" : "Make the list"} busyLabel="Saving…" onClick={() => act("res", () => writeRes(map), "Couldn't save the list.")} />}
          >
            {r.res.exists ? `${r.name}.res is missing ${plural(r.res.missing.length, "file")} the map uses` : `There's no ${r.name}.res`}, so players joining your server{" "}
            {r.res.exists ? "won't get those" : "only get the map itself"} and may see missing textures or crash.
            <details className="doc-more">
              <summary>Show the files</summary>
              <ul className="doc-files">{r.res.missing.map((p) => <li key={p}><code>{p}</code></li>)}</ul>
            </details>
          </Line>
        )}
        {r.res.exists && r.res.missing.length === 0 && r.res.broken.length > 0 && (
          <div className="row doc-pad">
            <BusyButton className="btn" busy={busy === "res"} disabled={off} label="Update the list" busyLabel="Saving…" onClick={() => act("res", () => writeRes(map), "Couldn't save the list.")} />
          </div>
        )}
      </section>

      <section className="panel">
        <h3>Bots</h3>
        {learning && (
          <div className="doc-pad">
            <JobProgress title="Teaching bots the map…" onEnd={() => reload()} />
          </div>
        )}
        {!r.nav.exists ? (
          <Line tone="warn" action={<BusyButton className="btn primary" busy={busy === "learn"} disabled={off || learning} label="Teach bots this map" busyLabel="Starting…" onClick={() => teach(false)} />}>
            There's no bot nav for this map, so bots can't find their way around it. Teaching them takes a few minutes on a hidden server in the background.
          </Line>
        ) : r.nav.error ? (
          <Line tone="bad" action={<BusyButton className="btn" busy={busy === "learn"} disabled={off || learning} label="Teach bots again" busyLabel="Starting…" onClick={() => teach(true)} />}>
            The bot nav ({r.nav.file}) is damaged: {r.nav.error}.
          </Line>
        ) : (
          <>
            {r.nav.matchesMap === false ? (
              <Line tone="warn" action={<BusyButton className="btn" busy={busy === "learn"} disabled={off || learning} label="Teach bots again" busyLabel="Starting…" onClick={() => teach(true)} />}>
                The bot nav was made for a different version of this map, so bots may walk into walls. Teaching them again makes a new one{r.nav.namedAreas ? " (its place names would need naming again)" : ""}.
              </Line>
            ) : (
              <Line tone="ok">Bots have a nav for this map ({plural(r.nav.areas ?? 0, "area")}).</Line>
            )}
            <Line tone={r.nav.places ? "ok" : "warn"} action={<button className="btn" onClick={nameplaces}>{r.nav.places ? "Edit place names" : "Name places"}</button>}>
              {r.nav.places
                ? `${plural(r.nav.places, "place")} named (${r.nav.namedAreas} of ${r.nav.areas} areas). Bots use them on the radio and in chat.`
                : "No places are named, so bots can't say where they are (\"enemy at Long\")."}
            </Line>
            {!r.lsv.exists ? (
              <Line tone="ok">The bots work out the map's sightlines and cover the first time they play it.</Line>
            ) : r.lsv.error ? (
              <Line tone="warn">{r.lsv.error} The bots make a new one when they next play the map.</Line>
            ) : r.lsv.fits === false ? (
              <Line tone="warn">The bots' map knowledge was made for a different nav. They work it out again the next time they play the map.</Line>
            ) : (
              <Line tone="ok">{r.lsv.done ? "The bots know this map's sightlines and cover." : `The bots are still learning this map (${r.lsv.stage}); they carry on when they next play it.`}</Line>
            )}
          </>
        )}
      </section>
    </div>
  );
}
