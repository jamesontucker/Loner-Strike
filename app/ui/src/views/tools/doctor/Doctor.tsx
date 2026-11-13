// The map doctor: pick a map (a downloaded one, usually) and see what it's missing or has
// broken, as a checklist: the map file, texture files, sky/models/sprites/sounds, the download
// list players get (.res), the bots' nav, place names and knowledge, the overview. Fixes that
// are safe are one click ("Fix all safe" does every one); for the rest it says where to get
// the files, or opens the tool that fixes it.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { useSetupJob, waitForSetupJob } from "../../../queries";
import { useNav } from "../../../store";
import { useToolArg } from "../../../stores/tools";
import { useReportAddress } from "../common/address";
import type { MapInfo } from "../../../types";
import { Banner, BusyButton, Info, LoadError, Segmented, useToast } from "../../../ui";
import { JobProgress } from "../../JobProgress";
import type { ToolProps } from "../registry";
import { overviewInfo } from "../overview/overviewApi";
import { checkMap, copyFiles, learnMap, learnResult, writeRes, type FileRow, type Report } from "./doctorApi";
import { Empty, Loading, MapList } from "../common";
import "./doctor.css";

export default function Doctor({ maps }: ToolProps) {
  const arg = useToolArg();
  const [map, setMap] = useState<string | null>(arg);
  useReportAddress("doctor", map ? `doctor:${map}` : null);
  useEffect(() => {
    if (arg) setMap(arg);
  }, [arg]);
  if (!map)
    return (
      <div className="tl-page stack-tight">
        <div className="tl-head">
          <div className="tl-head-title">
            <h3>Pick a map to check</h3>
            <Info text="The doctor looks for the texture files, models, sounds and sky a map needs, the list of files players download with it, the bots' nav and the overview, and fixes what it safely can." />
          </div>
        </div>
        <MapList maps={maps} onPick={setMap} placeholder="Search maps" note={(m: MapInfo) => (m.nav ? null : "no bot nav yet")} />
      </div>
    );
  return <Checkup map={map} onBack={() => setMap(null)} />;
}

type Tone = "ok" | "warn" | "bad";
type Group = "files" | "textures" | "res" | "nav" | "overview";
type Check = { id: string; group: Group; tone: Tone; title: string; detail?: ReactNode; action?: ReactNode; more?: ReactNode; moreOpen?: boolean };

const GROUPS: [Group, string][] = [
  ["files", "Map and files"],
  ["textures", "Textures and WADs"],
  ["res", "Download list (.res)"],
  ["nav", "Bots: nav, places, knowledge"],
  ["overview", "Overview (radar)"],
];
const WORD: Record<Tone, string> = { ok: "Passed", warn: "Worth a look", bad: "Problem" };
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

function Row({ c }: { c: Check }) {
  return (
    <li className={`doc-line ${c.tone}`}>
      <span className="doc-mark" role="img" aria-label={WORD[c.tone]} title={WORD[c.tone]}>{c.tone === "ok" ? "✓" : c.tone === "warn" ? "!" : "✕"}</span>
      <div className="doc-text">
        <strong>{c.title}</strong>
        {c.detail && <span className="doc-detail">{c.detail}</span>}
        {c.more && (
          <details className="doc-more" open={c.moreOpen}>
            <summary>Details</summary>
            {c.more}
          </details>
        )}
      </div>
      {c.action && <div className="doc-action">{c.action}</div>}
    </li>
  );
}

function Checkup({ map, onBack }: { map: string; onBack: () => void }) {
  const toast = useToast();
  const qc = useQueryClient();
  const go = (address: string) => useNav.getState().go("tools", address);
  const q = useQuery({ queryKey: ["tools", "doctor", map], queryFn: () => checkMap(map), retry: false });
  const ov = useQuery({ queryKey: ["tools", "overview", map], queryFn: ({ signal }) => overviewInfo(map, signal), retry: false, enabled: q.isSuccess && !!q.data?.bsp.ok });
  const [busy, setBusy] = useState<string | null>(null);
  const [filter, setFilter] = useState<"all" | "todo">("todo");
  const job = useSetupJob();
  const learning = job.data?.state === "running" && job.data.kind === "navlearn";

  const reload = () => {
    void qc.invalidateQueries({ queryKey: ["tools", "doctor", map] });
    void qc.invalidateQueries({ queryKey: ["tools", "overview", map] });
  };

  const act = async (key: string, fn: () => Promise<{ ok?: boolean; error?: string; message?: string }>, fail: string, quiet = false) => {
    setBusy(key);
    const r = await fn();
    setBusy(null);
    if (r.error || !r.ok) toast(r.error || fail, true);
    else if (!quiet) toast(r.message || "Done.");
    reload();
    return !r.error && !!r.ok;
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

  const r: Report | undefined = q.data;
  const checks = useMemo(() => (r ? buildChecks(r, ov.data, ov.isPending && ov.fetchStatus !== "idle", ov.error) : []), [r, ov.data, ov.isPending, ov.fetchStatus, ov.error]);

  const title = (
    <>
      <span className="map-title">{map}</span>
      {r?.title && r.title !== map && <span className="muted small-note">{r.title}</span>}
      <button className="btn small" onClick={onBack}>Other map</button>
    </>
  );

  if (q.isPending)
    return (
      <div className="tools-page">
        <div className="tools-bar">{title}</div>
        <div className="panel"><Loading text={`Checking ${map}…`} /></div>
      </div>
    );
  if (q.isError || !r)
    return (
      <div className="tools-page">
        <div className="tools-bar">{title}</div>
        <LoadError title={`Couldn't check ${map}`} text={(q.error as Error)?.message || "Couldn't check that map."} onRetry={() => q.refetch()} />
      </div>
    );

  const locked = r.inUse;
  const off = locked || busy !== null;
  const all = [...r.textures.wads, ...r.files];
  const elsewhere = all.filter((f) => f.status === "elsewhere");
  const missing = all.filter((f) => f.status === "missing");
  const resFix = !r.res.stockMap && (r.res.missing.length > 0 || r.res.broken.length > 0);
  const safe = (elsewhere.length ? 1 : 0) + (resFix ? 1 : 0);
  const search = `https://gamebanana.com/search?_sSearchString=${encodeURIComponent(r.name)}`;

  const copyAll = () => act("copy", () => copyFiles(map, elsewhere.map((f) => f.path)), "Couldn't copy the files.");
  const saveRes = () => act("res", () => writeRes(map), "Couldn't save the list.");
  const fixAll = async () => {
    setBusy("all");
    const done: string[] = [];
    if (elsewhere.length) {
      const c = await copyFiles(map, elsewhere.map((f) => f.path));
      if (c.error || !c.ok) {
        setBusy(null);
        reload();
        return toast(c.error || "Couldn't copy the files.", true);
      }
      done.push(`copied ${plural(elsewhere.length, "file")}`);
    }
    if (resFix || elsewhere.length) {
      const w = await writeRes(map);
      if (w.error || !w.ok) {
        setBusy(null);
        reload();
        return toast(w.error || "Couldn't save the list.", true);
      }
      done.push(`updated ${r.name}.res`);
    }
    setBusy(null);
    toast(done.length ? `Fixed: ${done.join(", ")}.` : "Nothing to fix.");
    reload();
  };

  // the fix buttons, by check
  const actions: Record<string, ReactNode> = {
    copy: <BusyButton className="btn primary small" busy={busy === "copy"} disabled={off} label={`Copy ${plural(elsewhere.length, "file")}`} busyLabel="Copying…" onClick={copyAll} />,
    get: (
      <>
        {r.gamebanana ? <a className="btn small primary" href={r.gamebanana.url} target="_blank" rel="noreferrer">Its GameBanana page</a> : <a className="btn small" href={search} target="_blank" rel="noreferrer">Search GameBanana</a>}
        <button className="btn small" onClick={() => useNav.getState().go("more")}>Get more</button>
      </>
    ),
    res: <BusyButton className="btn primary small" busy={busy === "res"} disabled={off} label={r.res.exists ? "Update the list" : "Make the list"} busyLabel="Saving…" onClick={saveRes} />,
    teach: <BusyButton className="btn primary small" busy={busy === "learn"} disabled={off || learning} label="Teach bots this map" busyLabel="Starting…" onClick={() => teach(false)} />,
    reteach: <BusyButton className="btn small" busy={busy === "learn"} disabled={off || learning} label="Teach bots again" busyLabel="Starting…" onClick={() => teach(true)} />,
    navfix: <button className="btn small" onClick={() => go(`navfix:${map}`)}>Open in Nav fixer</button>,
    places: <button className="btn small" onClick={() => go(`callouts:${map}`)}>{r.nav.places ? "Edit place names" : "Name places"}</button>,
    radar: <button className="btn small" onClick={() => go(`radar:${map}`)}>{ov.data?.current?.imageFound ? "Open in Radar maker" : "Make one"}</button>,
  };
  const shownChecks = checks.map((c) => ({ ...c, action: c.action ?? (c.id in ACTION_FOR ? ACTION_FOR[c.id](actions, c) : undefined) }));
  const count = { ok: 0, warn: 0, bad: 0 } as Record<Tone, number>;
  for (const c of shownChecks) count[c.tone]++;
  const todo = count.warn + count.bad;
  const view = todo === 0 ? "all" : filter;

  return (
    <div className="tools-page doc-page">
      <div className="tools-bar">
        {title}
        <span className="push" />
        <BusyButton className="btn" busy={q.isFetching && !busy} label="Check again" busyLabel="Checking…" onClick={() => reload()} />
        {safe > 0 && <BusyButton className="btn primary" busy={busy === "all"} disabled={off} label={`Fix all safe (${safe})`} busyLabel="Fixing…" onClick={fixAll} />}
      </div>
      {locked && <Banner>The game has {map} loaded. Change to another map (or close the game) to use the fixes.</Banner>}

      <div className="tools-viewer doc-body">
        <div className="doc-report">
          <div className="doc-summary">
            <div className={`doc-verdict ${count.bad ? "bad" : count.warn ? "warn" : "ok"}`}>
              <strong>{count.bad ? plural(count.bad, "problem") : count.warn ? plural(count.warn, "thing") + " worth a look" : "All good"}</strong>
              <span className="muted small-note">{count.bad || count.warn ? `${count.ok} of ${shownChecks.length} checks passed` : `${shownChecks.length} checks passed: this map has everything it needs`}</span>
            </div>
            <div className="doc-counts" aria-label="Checks">
              <span className="doc-count bad">{count.bad}<small>problems</small></span>
              <span className="doc-count warn">{count.warn}<small>warnings</small></span>
              <span className="doc-count ok">{count.ok}<small>passed</small></span>
            </div>
            {todo > 0 && <Segmented label="Show" values="todo all" labels="Needs a look|All checks" current={view} onPick={(v) => setFilter(v as "all" | "todo")} />}
          </div>
          {learning && <div className="panel"><JobProgress title="Teaching bots the map…" onEnd={() => reload()} /></div>}
          {GROUPS.map(([g, label]) => {
            const rows = shownChecks.filter((c) => c.group === g);
            const list = view === "all" ? rows : rows.filter((c) => c.tone !== "ok");
            if (!rows.length) return null;
            const worst: Tone = rows.some((c) => c.tone === "bad") ? "bad" : rows.some((c) => c.tone === "warn") ? "warn" : "ok";
            return (
              <section key={g} className={`panel doc-group ${worst}`}>
                <h3>
                  <span className={`doc-dot ${worst}`} aria-hidden="true" />
                  {label}
                  <span className="muted small-note push">{rows.filter((c) => c.tone === "ok").length}/{rows.length} passed</span>
                </h3>
                {list.length ? <ul className="doc-lines">{list.map((c) => <Row key={c.id} c={c} />)}</ul> : <p className="small-note muted doc-allok">All passed.</p>}
              </section>
            );
          })}
          {view === "todo" && todo === 0 && <Empty plain text="Nothing needs a look." />}
        </div>

        <aside className="tools-side">
          <section className="tools-panel">
            <h3>This map</h3>
            <dl className="doc-facts">
              <dt>Game</dt><dd>{r.game}</dd>
              {r.size != null && <><dt>Size</dt><dd>{(r.size / 1048576).toFixed(1)} MB</dd></>}
              <dt>Textures</dt><dd>{r.textures.total}{r.textures.embedded ? ` (${r.textures.embedded} built in)` : ""}</dd>
              {r.nav.exists && <><dt>Nav</dt><dd>{plural(r.nav.areas ?? 0, "area")}, {plural(r.nav.places ?? 0, "place")}</dd></>}
              {r.gamebanana && <><dt>From</dt><dd><a href={r.gamebanana.url} target="_blank" rel="noreferrer">{r.gamebanana.name}</a></dd></>}
            </dl>
            <p className="small-note muted">Looked where the game looks: {r.looksIn.join(", then ")}.</p>
          </section>
          {safe > 0 && (
            <section className="tools-panel">
              <h3>Safe fixes <Info text="Copying files from another game's folder never replaces anything, and the .res only lists files. Both are kept in History, so you can undo them." /></h3>
              <ul className="doc-safe">
                {elsewhere.length > 0 && <li>Copy {plural(elsewhere.length, "file")} from {[...new Set(elsewhere.map((f) => f.folder))].join(", ")}</li>}
                {(resFix || elsewhere.length > 0) && <li>{r.res.exists ? "Update" : "Make"} {r.name}.res</li>}
              </ul>
              <BusyButton className="btn primary" busy={busy === "all"} disabled={off} label="Fix all safe" busyLabel="Fixing…" onClick={fixAll} />
            </section>
          )}
          {missing.length > 0 && (
            <section className="tools-panel">
              <h3>Missing files</h3>
              <p className="small-note">{plural(missing.length, "file isn't", "files aren't")} on this computer. Getting the map again {r.gamebanana ? `("${r.gamebanana.name}")` : "from where you found it"} usually brings them.</p>
              <div className="row">{actions.get}</div>
            </section>
          )}
          <section className="tools-panel">
            <h3>Other tools</h3>
            <div className="doc-links">
              <button className="btn small" onClick={() => go(`map:${map}`)}>Map viewer</button>
              <button className="btn small" disabled={!r.nav.exists} onClick={() => go(`navfix:${map}`)}>Nav fixer</button>
              <button className="btn small" disabled={!r.nav.exists} onClick={() => go(`callouts:${map}`)}>Callouts</button>
              <button className="btn small" onClick={() => go(`radar:${map}`)}>Radar maker</button>
              <button className="btn small" onClick={() => go(`health:${map}`)}>Health check</button>
            </div>
          </section>
        </aside>
      </div>
    </div>
  );
}

/** which fix buttons a check gets (by its id) */
const ACTION_FOR: Record<string, (a: Record<string, ReactNode>, c: Check) => ReactNode> = {
  elsewhere: (a) => a.copy,
  missing: (a) => a.get,
  "tex-missing": (a) => a.get,
  "res-missing": (a) => a.res,
  "res-broken": (a) => a.res,
  "nav-none": (a) => a.teach,
  "nav-damaged": (a) => a.reteach,
  "nav-old": (a) => a.reteach,
  "nav-ok": (a) => a.navfix,
  places: (a) => a.places,
  overview: (a) => a.radar,
};

function buildChecks(r: Report, ov: Awaited<ReturnType<typeof overviewInfo>> | undefined, ovLoading: boolean, ovError: unknown): Check[] {
  const out: Check[] = [];
  const add = (c: Check) => out.push(c);
  const all = [...r.textures.wads, ...r.files];
  const elsewhere = all.filter((f) => f.status === "elsewhere");
  const missing = all.filter((f) => f.status === "missing");

  // ---- map and files
  add(r.bsp.ok
    ? { id: "bsp", group: "files", tone: "ok", title: "The map file reads fine", detail: r.size ? `${(r.size / 1048576).toFixed(1)} MB` : undefined }
    : { id: "bsp", group: "files", tone: "bad", title: "The map file is damaged", detail: `${r.bsp.error}. The game may not load it; getting it again usually fixes this.` });
  const kinds: [FileRow["kind"], string][] = [["sky", "Sky"], ["model", "Models"], ["sprite", "Sprites"], ["sound", "Sounds"]];
  for (const [k, label] of kinds) {
    const rows = r.files.filter((f) => f.kind === k);
    if (!rows.length) continue;
    const bad = rows.filter((f) => f.status !== "ok");
    add({
      id: `files-${k}`,
      group: "files",
      tone: bad.some((f) => f.status === "missing") ? "bad" : bad.length ? "warn" : "ok",
      title: bad.length ? `${label}: ${bad.length} of ${rows.length} not where the game looks` : `${label}: all ${rows.length} there`,
      more: <FileList rows={[...bad, ...rows.filter((f) => f.status === "ok")]} />,
      moreOpen: bad.length > 0 && bad.length <= 6,
    });
  }
  if (!r.files.length) add({ id: "files-none", group: "files", tone: "ok", title: "No extra files needed", detail: "The map uses no custom sky, models, sprites or sounds." });
  if (elsewhere.length)
    add({ id: "elsewhere", group: "files", tone: "warn", title: `${plural(elsewhere.length, "file is", "files are")} in another game's folder`, detail: `In ${[...new Set(elsewhere.map((f) => f.folder))].join(", ")}, where ${r.game} doesn't look. Copying puts them in ${r.game}; nothing is replaced.` });
  if (missing.length)
    add({ id: "missing", group: "files", tone: "bad", title: `${plural(missing.length, "file isn't", "files aren't")} on this computer`, detail: "They usually come with the map's download.", more: <FileList rows={missing} /> });

  // ---- textures
  add(r.textures.missing.length === 0
    ? { id: "tex", group: "textures", tone: "ok", title: `All ${r.textures.total} textures found`, detail: r.textures.embedded ? `${r.textures.embedded} built into the map` : undefined }
    : {
        id: "tex-missing",
        group: "textures",
        tone: "bad",
        title: `${plural(r.textures.missing.length, "texture")} missing: shown as a checkerboard`,
        detail: r.textures.fixable ? `Copying the texture files brings back ${r.textures.fixable}.` : undefined,
        more: <p className="doc-tex">{r.textures.missing.join(", ")}</p>,
      });
  if (r.textures.wads.length) {
    const bad = r.textures.wads.filter((f) => f.status !== "ok");
    add({ id: "wads", group: "textures", tone: bad.some((f) => f.status === "missing") ? "bad" : bad.length ? "warn" : "ok", title: bad.length ? `${bad.length} of ${r.textures.wads.length} WAD files not where the game looks` : `All ${r.textures.wads.length} WAD files there`, more: <FileList rows={r.textures.wads} /> });
  }

  // ---- .res
  if (r.res.stockMap) add({ id: "res", group: "res", tone: "ok", title: "One of the game's own maps", detail: "Every player has it and its files; it needs no list." });
  else if (r.res.wanted === 0 && !r.res.exists) add({ id: "res", group: "res", tone: "ok", title: "No list needed", detail: "The map only uses files every player has." });
  else if (r.res.missing.length)
    add({
      id: "res-missing",
      group: "res",
      tone: "warn",
      title: r.res.exists ? `${r.name}.res is missing ${plural(r.res.missing.length, "file")}` : `There's no ${r.name}.res`,
      detail: `Players joining your server ${r.res.exists ? "won't get those" : "only get the map itself"} and may see missing textures or crash.`,
      more: <ul className="doc-files">{r.res.missing.map((p) => <li key={p}><code>{p}</code></li>)}</ul>,
    });
  else add({ id: "res", group: "res", tone: "ok", title: `${r.name}.res lists ${plural(r.res.listed, "file")}`, detail: r.res.wanted ? "Everything players may not have is on it." : undefined });
  if (!r.res.stockMap && r.res.exists && r.res.broken.length)
    add({ id: "res-broken", group: "res", tone: "warn", title: `${r.name}.res names ${plural(r.res.broken.length, "file")} not on this computer`, detail: "Updating the list leaves them out.", more: <ul className="doc-files">{r.res.broken.map((p) => <li key={p}><code>{p}</code></li>)}</ul> });

  // ---- nav
  if (!r.nav.exists) add({ id: "nav-none", group: "nav", tone: "warn", title: "No bot nav", detail: "Bots can't find their way around. Teaching them takes a few minutes on a hidden server." });
  else if (r.nav.error) add({ id: "nav-damaged", group: "nav", tone: "bad", title: "The bot nav is damaged", detail: `${r.nav.file}: ${r.nav.error}` });
  else {
    if (r.nav.matchesMap === false) add({ id: "nav-old", group: "nav", tone: "warn", title: "The nav is for another version of the map", detail: `Bots may walk into walls. Teaching them again makes a new one${r.nav.namedAreas ? " (place names would need naming again)" : ""}.` });
    else add({ id: "nav-ok", group: "nav", tone: "ok", title: `Bots have a nav (${plural(r.nav.areas ?? 0, "area")})` });
    add(r.nav.places
      ? { id: "places", group: "nav", tone: "ok", title: `${plural(r.nav.places, "place")} named`, detail: `${r.nav.namedAreas} of ${r.nav.areas} areas. Bots use them on the radio and in chat.` }
      : { id: "places", group: "nav", tone: "warn", title: "No places named", detail: "Bots can't say where they are (\"enemy at Long\")." });
    if (!r.lsv.exists) add({ id: "lsv", group: "nav", tone: "ok", title: "Bot knowledge: made on first play", detail: "Bots work out sightlines and cover the first time they play the map." });
    else if (r.lsv.error) add({ id: "lsv", group: "nav", tone: "warn", title: "Bot knowledge can't be read", detail: `${r.lsv.error} The bots make a new one when they next play the map.` });
    else if (r.lsv.fits === false) add({ id: "lsv", group: "nav", tone: "warn", title: "Bot knowledge is for another nav", detail: "They work it out again the next time they play the map." });
    else add({ id: "lsv", group: "nav", tone: "ok", title: r.lsv.done ? "Bots know the sightlines and cover" : `Bots are still learning the map (${r.lsv.stage})`, detail: r.lsv.done ? undefined : "They carry on when they next play it." });
  }

  // ---- overview
  if (ovLoading) add({ id: "overview-wait", group: "overview", tone: "ok", title: "Looking for the overview…" });
  else if (ovError) add({ id: "overview-err", group: "overview", tone: "warn", title: "Couldn't check the overview", detail: ovError instanceof Error ? ovError.message : undefined });
  else if (ov) {
    if (!ov.current) add({ id: "overview", group: "overview", tone: "warn", title: "No overview", detail: "Spectators and HLTV viewers see no map in overview mode. The Radar maker draws one from the map." });
    else if (!ov.current.imageFound) add({ id: "overview", group: "overview", tone: "bad", title: "The overview's picture is missing", detail: `${ov.current.file} names a picture that isn't there.` });
    else add({ id: "overview", group: "overview", tone: "ok", title: ov.current.ours ? "Has an overview made here" : "Has an overview", detail: ov.current.file });
  }
  return out;
}
