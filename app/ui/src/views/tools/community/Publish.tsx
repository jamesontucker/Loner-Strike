// Publish: package a map, a Game Mode Studio mode or a prefab for GameBanana, then register its
// page so it shows up in Community. Uploading happens on GameBanana's site (it has no upload API);
// the app prepares everything to paste and checks the page afterwards.
//
// The page is one column of numbered steps: what you're publishing, checks (a status list where
// each check says why in one line and has a button that fixes it), details, modes, pictures,
// credits, then the action. After packing: the upload steps, the terms tick and Register.
//
// Other tools hand over a map with go("tools", "community:publish:<map>"), the Game Mode Studio a
// mode with "community:publish:mode:<id>", the prefab library a prefab with "community:publish:prefab:<id>".

import { useQuery } from "@tanstack/react-query";
import { useEffect, useState, type ReactNode } from "react";
import { copyText, getJson } from "../../../api";
import { useNav } from "../../../store";
import type { MapInfo } from "../../../types";
import { BusyButton, LoadError, Segmented, useToast } from "../../../ui";
import { errorText, post, useBusy } from "../common/api";
import { Empty, Loading, MapList } from "../common";
import { CreditsPanel } from "./Credits";
import { drawCover } from "./coverCard"; // MCP: the automatic cover card
import { BOT_NOTE, BOT_RATING_INFO } from "../../../games";
import {
  modeReadiness, publish, publishMode, readiness, register, reveal, sizeText, studioGraphs,
  type AssetCredit, type Form, type MineState, type Package, type Readiness, type Shot, type SourceProject,
} from "./communityApi";

const MAX_SHOTS = 6;
const MAX_SHOT = 8 << 20;
type What = "map" | "mode" | "prefab";

export default function Publish({ maps, map, onMap, onDone }: { maps: MapInfo[]; map?: string; onMap: (m: string | undefined) => void; onDone: () => void }) {
  const [what, setWhat] = useState<What>(map?.startsWith("mode:") ? "mode" : map?.startsWith("prefab:") ? "prefab" : "map");
  if (map?.startsWith("mode:")) return <PublishMode id={map.slice(5)} onBack={() => onMap(undefined)} onDone={onDone} />;
  if (map?.startsWith("prefab:")) return <PublishPrefab id={map.slice(7)} onBack={() => onMap(undefined)} onDone={onDone} />;
  if (map) return <PublishMap map={map} onBack={() => onMap(undefined)} onDone={onDone} />;
  return (
    <div className="cm-pub">
      <Step n={1} title="What are you publishing?">
        <Segmented label="What to publish" values="map mode prefab" labels="A map|A game mode|A prefab" current={what} onPick={(v) => setWhat(v as What)} />
        {what === "map" && (
          <>
            <p className="muted small-note">A map you made: it's checked, then packed with its bot nav, map knowledge, download list and radar picture.</p>
            <MapList maps={maps} onPick={onMap} placeholder="Pick a map to publish" filter={(m) => m.source !== "stock"} none="Only maps you made or added can be published; the game's own maps can't." note={(m) => (m.nav ? null : "no bot nav yet")} />
          </>
        )}
        {what === "mode" && <ModeList onPick={(id) => onMap(`mode:${id}`)} />}
        {what === "prefab" && <PrefabList onPick={(id) => onMap(`prefab:${id}`)} />}
      </Step>
    </div>
  );
}

// ---- building blocks ---------------------------------------------------------------------------

function Step({ n, title, note, children, id }: { n: number; title: string; note?: ReactNode; children: ReactNode; id?: string }) {
  return (
    <section className="cm-step panel" id={id} aria-labelledby={`cm-step-${n}`}>
      <header className="cm-step-head">
        <span className="cm-step-n" aria-hidden="true">{n}</span>
        <div>
          <h3 id={`cm-step-${n}`}>{title}</h3>
          {note && <p className="muted small-note">{note}</p>}
        </div>
      </header>
      <div className="cm-step-body">{children}</div>
    </section>
  );
}

type Check = { state: "ok" | "warn" | "bad" | "tip"; title: string; why: string; fix?: { label: string; to: string } };
const MARK: Record<Check["state"], [string, string]> = { ok: ["✓", "Done"], warn: ["!", "Worth a look"], bad: ["✕", "Must fix"], tip: ["·", "Optional"] };

/** The checks: one line each, with the button that fixes it. */
function Checks({ rows }: { rows: Check[] }) {
  const go = useNav((s) => s.go);
  return (
    <ul className="cm-checks">
      {rows.map((c, i) => (
        // (two checks can share a title: "Requirements" for each file, P5 QA4-G)
        <li key={`${i}:${c.title}`} className={`cm-check ${c.state}`}>
          <span className="cm-check-mark" role="img" aria-label={MARK[c.state][1]} title={MARK[c.state][1]}>{MARK[c.state][0]}</span>
          <span className="cm-check-title">{c.title}</span>
          <span className="cm-check-why muted">{c.why}</span>
          {c.fix ? (
            c.fix.to.startsWith("#") ? (
              <button className="btn small" onClick={() => document.querySelector(c.fix!.to)?.scrollIntoView({ behavior: "smooth", block: "start" })}>{c.fix.label}</button>
            ) : (
              <button className="btn small" onClick={() => go("tools", c.fix!.to)}>{c.fix.label}</button>
            )
          ) : <span />}
        </li>
      ))}
    </ul>
  );
}

function useAuthor(): [string, (v: string) => void, () => void] {
  const [author, setAuthor] = useState(() => {
    try {
      return localStorage.getItem("ls.community.author") ?? "";
    } catch {
      return "";
    }
  });
  const remember = () => {
    try {
      localStorage.setItem("ls.community.author", author.trim());
    } catch {
      /* remembered next time, maybe */
    }
  };
  return [author, setAuthor, remember];
}

function Details({ title, setTitle, author, setAuthor, summary, setSummary, about }: {
  title: string; setTitle: (v: string) => void; author: string; setAuthor: (v: string) => void; summary: string; setSummary: (v: string) => void; about: string;
}) {
  return (
    <div className="cm-grid2">
      <label className="cm-field">
        <span>Name</span>
        <input className="search" value={title} maxLength={80} onChange={(e) => setTitle(e.target.value)} placeholder="What players will see" />
      </label>
      <label className="cm-field">
        <span>Made by</span>
        <input className="search" value={author} maxLength={40} onChange={(e) => setAuthor(e.target.value)} placeholder="Your GameBanana name" />
      </label>
      <label className="cm-field cm-span2">
        <span>About it</span>
        <textarea className="search" value={summary} maxLength={400} rows={3} onChange={(e) => setSummary(e.target.value)} placeholder={about} />
      </label>
    </div>
  );
}

/** Toggle chips, one per choice (names shown, ids sent). */
function Chips({ choices, picked, onToggle, label }: { choices: { id: string; name: string }[]; picked: string[]; onToggle: (id: string) => void; label: string }) {
  return (
    <div className="cm-chips" role="group" aria-label={label}>
      {choices.map((c) => (
        <button key={c.id} type="button" className="cm-chip-toggle" aria-pressed={picked.includes(c.id)} title={c.id} onClick={() => onToggle(c.id)}>
          {c.name}
        </button>
      ))}
    </div>
  );
}

function Action({ children, note }: { children: ReactNode; note?: ReactNode }) {
  return (
    <div className="cm-action panel">
      {note && <span className="muted small-note">{note}</span>}
      <div className="row">{children}</div>
    </div>
  );
}

function Head({ title, kind, onBack, busy, onRecheck }: { title: string; kind: string; onBack: () => void; busy: boolean; onRecheck: () => void }) {
  return (
    <div className="tl-head cm-pub-head">
      <div className="tl-head-title">
        <h3>Publish {title}</h3>
        <span className="muted">{kind}</span>
      </div>
      <div className="tl-head-actions">
        <button className="btn" onClick={onBack}>Pick something else</button>
        <BusyButton className="btn" busy={busy} label="Check again" busyLabel="Checking…" onClick={onRecheck} />
      </div>
    </div>
  );
}

// ---- lists to pick from ------------------------------------------------------------------------------

/** Your Game Mode Studio modes (remixes included): only the graph is shared, each player's app builds it. */
function ModeList({ onPick }: { onPick: (id: string) => void }) {
  const q = useQuery({ queryKey: ["tools", "community", "studio-graphs"], queryFn: () => studioGraphs(), retry: false });
  if (q.isPending) return <Loading text="Loading your modes…" />;
  if (q.data?.error) return <LoadError text={q.data.error} onRetry={() => q.refetch()} />;
  const list = (q.data?.graphs ?? []).filter((g) => !g.damaged);
  if (!list.length)
    return (
      <Empty title="No Game Mode Studio modes yet" text="Make one in Tools > Game Mode Studio, or remix one you installed from Community.">
        <button className="btn" onClick={() => useNav.getState().go("tools", "studio")}>Open Game Mode Studio</button>
      </Empty>
    );
  return (
    <>
      <p className="muted small-note">Only the mode's graph is shared: each player's Loner-Strike builds it on their own computer, so nothing that runs code is downloaded.</p>
      <ul className="tl-maps">
        {list.map((g) => (
          <li key={g.id}>
            <button className="tl-map" onClick={() => onPick(g.id)}>
              <span className="tl-map-name">{g.name}</span>
              <span className="muted small-note">{g.nodes != null ? `${g.nodes} nodes` : ""}</span>
            </button>
          </li>
        ))}
      </ul>
    </>
  );
}

type PrefabRow = { id: string; name: string; builtin: boolean; brushes: number; entities: number };

function PrefabList({ onPick }: { onPick: (id: string) => void }) {
  const q = useQuery({ queryKey: ["tools", "community", "my-prefabs"], queryFn: () => getJson<{ prefabs: PrefabRow[] }>("/api/tools/editor/prefabs"), retry: false });
  if (q.isPending) return <Loading text="Loading your prefabs…" />;
  if (q.isError) return <LoadError text={errorText(q.error, "Couldn't load your prefabs.")} onRetry={() => q.refetch()} />;
  const list = (q.data.prefabs ?? []).filter((p) => !p.builtin);
  if (!list.length)
    return (
      <Empty title="No prefabs of your own yet" text="In the map editor, pick some brushes and entities and choose Save what's picked.">
        <button className="btn" onClick={() => useNav.getState().go("tools", "editor:prefabs")}>Open the map editor</button>
      </Empty>
    );
  return (
    <>
      <p className="muted small-note">A prefab from your library: shared as a package other mappers add to their prefab library, with the credits of the models and textures it uses.</p>
      <ul className="tl-maps">
        {list.map((p) => (
          <li key={p.id}>
            <button className="tl-map" onClick={() => onPick(p.id)}>
              <span className="tl-map-name">{p.name}</span>
              <span className="muted small-note">{p.brushes} brushes{p.entities ? `, ${p.entities} entities` : ""}</span>
            </button>
          </li>
        ))}
      </ul>
    </>
  );
}

// ---- a map ----------------------------------------------------------------------------------------

/** The map's checks as a status list (from the server's readiness), each with its fix. */
/** The custom creatures a creation brings (packed with it, named for who made it). */
function creatureCheck(list?: string[]): Check | null {
  if (!list?.length) return null;
  return { state: "ok", title: "Custom creatures", why: `Includes: ${list.join(", ")}. Named for you, so they never clash with players' own types.` };
}

function mapChecks(map: string, r: Readiness): Check[] {
  const rows: Check[] = r.blockers.map((b) => ({ state: "bad" as const, title: "Can't publish yet", why: b }));
  const credit = (r.assets ?? []).filter((a) => (a.verdict === "ask" || a.verdict === "no") && !a.permission?.granted);
  const creditNo = credit.some((a) => a.verdict === "no");
  const files = r.warnings.filter((w) => /map doctor/i.test(w));
  rows.push(
    files.length
      ? { state: "warn", title: "Files and textures", why: files[0], fix: { label: "Open the map doctor", to: `doctor:${map}` } }
      : { state: "ok", title: "Files and textures", why: "Everything the map uses is where the game looks." },
    r.map.nav
      ? { state: "ok", title: "Bots know the map", why: "It ships with a bot nav." }
      : { state: "warn", title: "Bots know the map", why: "No nav that fits this map yet, so it won't get the Works with bots badge.", fix: { label: "Teach the bots", to: `doctor:${map}` } },
    r.map.lsv
      ? { state: "ok", title: "Bot map knowledge", why: "Finished, so bots play it well from the first round." }
      : { state: "tip", title: "Bot map knowledge", why: "Not finished yet; a run in the Map simulator fills it in.", fix: { label: "Open the Map simulator", to: `health:${map}` } },
    r.map.places
      ? { state: "ok", title: "Callouts", why: "Places are named, so bots say where they are." }
      : { state: "tip", title: "Callouts", why: "Name places so bots call out where they are.", fix: { label: "Open Callouts", to: `callouts:${map}` } },
    r.map.overview
      ? { state: "ok", title: "Radar picture", why: "It ships with an overview." }
      : { state: "tip", title: "Radar picture", why: "No overview (radar) picture for this map.", fix: { label: "Open the Radar maker", to: `radar:${map}` } },
    !r.health
      ? { state: "tip", title: "Bot simulation", why: "Run one: a good result earns the Checked with bots badge.", fix: { label: "Open the Map simulator", to: `health:${map}` } }
      : r.health.verdict === "bad"
        ? { state: "warn", title: "Bot simulation", why: "The last run found real problems (falls, stuck bots or errors).", fix: { label: "See the report", to: `health:${map}` } }
        : { state: "ok", title: "Bot simulation", why: `Last run ${new Date(r.health.when * 1000).toLocaleDateString()}: ${r.health.verdict}.` },
    credit.length
      ? { state: creditNo ? "bad" : "warn", title: "Credits and permissions", why: `${credit.length} file${credit.length === 1 ? "" : "s"} ${creditNo ? "may not be shared without permission" : "ask for permission first"}.`, fix: { label: "See credits", to: "#cm-credits" } }
      : { state: "ok", title: "Credits and permissions", why: "Everyone whose files it uses is credited." },
  );
  return rows;
}

  const cr = creatureCheck(r.creatures);
  if (cr) rows.push(cr);
  for (const w of r.warnings.filter((x) => /can be listed once|the full name is too long/.test(x))) rows.push({ state: "warn", title: "Creature names", why: w });
/** P5: which editor project is the map's, and whether its editable source goes in the package. */
function SourceStep({ projects, project, setProject, on, setOn }: { projects: SourceProject[]; project: string; setProject: (p: string) => void; on: boolean; setOn: (v: boolean) => void }) {
  const picked = projects.find((p) => p.path === project);
  return (
    <div className="stack-tight" data-testid="cm-source">
      <label className="cm-wide-check">
        <input type="checkbox" checked={on} onChange={(e) => setOn(e.target.checked)} /> Include the editable source (others can remix it)
      </label>
      <label className="small-note">
        The map's project{" "}
        <select value={project} onChange={(e) => setProject(e.target.value)} aria-label="The map's project">
          <option value="">None</option>
          {projects.map((p) => (
            <option key={p.path} value={p.path}>
              {p.name}.lsmap{p.match ? " (named like the map)" : ""}
            </option>
          ))}
        </select>
      </label>
      {!projects.length && <p className="muted small-note">No map projects in the editor's recent list. Open the map's project in the map editor once, then Check again.</p>}
      {on && <p className="muted small-note">Adds source/ to the package: the project, a Valve 220 .map, and a texture file with only the map's own textures (never the game's).</p>}
      {picked?.remixOf && (
        <p className="small-note">
          A remix of <b>{picked.remixOf.name}</b> by {picked.remixOf.author}: the package, its README and the GameBanana text credit it.
        </p>
      )}
    </div>
  );
}

function PublishMap({ map, onBack, onDone }: { map: string; onBack: () => void; onDone: () => void }) {
  const toast = useToast();
  const q = useQuery({ queryKey: ["tools", "community", "ready", map], queryFn: ({ signal }) => readiness(map, signal), retry: false });
  const [title, setTitle] = useState("");
  const [summary, setSummary] = useState("");
  const [author, setAuthor, remember] = useAuthor();
  const [picked, setPicked] = useState<string[]>([]);
  const [shots, setShots] = useState<Shot[]>([]);
  const [done, setDone] = useState<{ pkg: Package; form: Form } | null>(null);
  const { working, run } = useBusy();
  // P5: the map's editor project (its editable source when asked, and a remix's credit)
  const [project, setProject] = useState<string | null>(null);
  const [withSource, setWithSource] = useState(false);

  useEffect(() => {
    if (!q.data) return;
    setTitle((t) => t || q.data.ready.title);
    setPicked((p) => (p.length ? p : q.data.suggestedModes));
    setProject((p) => p ?? (q.data.projects ?? []).find((x) => x.match)?.path ?? "");
  }, [q.data]);

  const head = <Head title={map} kind="Map" onBack={onBack} busy={q.isFetching} onRecheck={() => q.refetch()} />;
  if (q.isPending) return <div className="cm-pub">{head}<Loading text={`Checking ${map}…`} /></div>;
  if (q.isError) return <div className="cm-pub">{head}<LoadError text={errorText(q.error, "Couldn't check that map.")} onRetry={() => q.refetch()} /></div>;
  const r = q.data.ready;
  if (done) return <div className="cm-pub">{head}<Upload pkg={done.pkg} form={done.form} onDone={onDone} /></div>;
  const choices = q.data.modeChoices ?? [];

  const addShots = async (files: FileList | null) => {
    if (!files) return;
    const room = MAX_SHOTS - shots.length;
    const read: Shot[] = [];
    for (const f of Array.from(files).slice(0, Math.max(0, room))) {
      if (!/^image\/(png|jpeg)$/.test(f.type)) {
        toast(`${f.name} isn't a PNG or JPEG picture.`, true);
        continue;
      }
      if (f.size > MAX_SHOT) {
        toast(`${f.name} is over 8 MB.`, true);
        continue;
      }
      const data = await new Promise<string>((ok, bad) => {
        const fr = new FileReader();
        fr.onload = () => ok(String(fr.result));
        fr.onerror = () => bad(fr.error);
        fr.readAsDataURL(f);
      }).catch(() => "");
      if (data) read.push({ name: f.name, data });
    }
    if (files.length > room) toast(`At most ${MAX_SHOTS} screenshots.`, true);
    setShots((s) => [...s, ...read]);
  };
  const build = () =>
    run("Packing…", async () => {
      remember();
      const res = await publish(map, { title, summary, author, modes: picked, screenshots: shots, project: project ?? "", source: withSource && !!project });
      if (res.error) return toast(res.error, true);
      setDone({ pkg: res.package, form: res.form });
    });
  const toggle = (id: string) => setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));
  const blocked = r.blockers.length > 0;

  return (
    <div className="cm-pub">
      {head}
      <Step n={1} title="Checks" note={blocked ? "Fix what's marked Must fix, then Check again." : "Optional items earn badges; nothing here stops you except Must fix."}>
        <Checks rows={mapChecks(map, r)} />
        <p className="muted small-note cm-ships">
          In the package: the map{r.map.nav ? ", its bot nav" : ""}{r.map.lsv ? ", bot map knowledge" : ""}, a fresh download list (.res)
          {r.files.length ? ` and ${r.files.length} file${r.files.length === 1 ? "" : "s"} it uses` : ""}.
        </p>
      </Step>
      <Step n={2} title="Details">
        <Details title={title} setTitle={setTitle} author={author} setAuthor={setAuthor} summary={summary} setSummary={setSummary} about="One or two sentences: what it is and what makes it fun" />
      </Step>
      <Step n={3} title="Game modes it's made for" note="Picked from the map's own Made for list when it has one. Players see these on its card.">
        {choices.length ? <Chips choices={choices} picked={picked} onToggle={toggle} label="Game modes" /> : <p className="muted small-note">No modes found in your game; it's listed for the usual mode.</p>}
      </Step>
      <Step n={4} title="Screenshots" note="PNG or JPEG, up to 8 MB each, up to 6. Saved next to the package for GameBanana's picture upload.">
        <div className="cm-shots">
          {shots.map((s, i) => (
            <figure key={i}>
              <img src={s.data} alt={s.name} />
              <button type="button" className="btn small" onClick={() => setShots(shots.filter((_, j) => j !== i))} aria-label={`Remove ${s.name}`}>Remove</button>
            </figure>
          ))}
          {shots.length < MAX_SHOTS && (
            <label className="btn cm-add">
              Add pictures
              <input type="file" accept="image/png,image/jpeg" multiple hidden onChange={(e) => { void addShots(e.target.files); e.target.value = ""; }} />
            </label>
          )}
        </div>
      </Step>
      <Step n={5} title="Credits and permissions" id="cm-credits" note="Where each file the map ships came from. Credits go into the README, GameBanana's Credits module and the Community listing.">
        {(r.assets ?? []).some((a) => a.source !== "stock" && a.source !== "mine") ? (
          <CreditsPanel assets={r.assets ?? []} onChanged={() => q.refetch()} />
        ) : (
          <p className="muted small-note">The map only uses the game's own files and yours: nobody else to credit.</p>
        )}
      </Step>
      <Step n={6} title="Editable source" note="Optional. Others can open the map in Loner-Strike's editor (Remix this map), or the .map in Hammer, J.A.C.K. or TrenchBroom, and make their own version that credits yours.">
        <SourceStep projects={q.data.projects ?? []} project={project ?? ""} setProject={setProject} on={withSource} setOn={setWithSource} />
      </Step>
      <Action note={blocked ? "Fix the checks marked Must fix first." : !title.trim() || !author.trim() ? "Give it a name and say who made it." : "Next: upload it to GameBanana and register the page."}>
        <BusyButton className="btn primary" busy={working !== null} busyLabel={working ?? "Packing…"} label="Make the package" disabled={blocked || !title.trim() || !author.trim() || (withSource && !project)} onClick={build} />
      </Action>
    </div>
  );
}

// ---- a game mode ----------------------------------------------------------------------------------

function PublishMode({ id, onBack, onDone }: { id: string; onBack: () => void; onDone: () => void }) {
  const toast = useToast();
  const q = useQuery({ queryKey: ["tools", "community", "ready-mode", id], queryFn: ({ signal }) => modeReadiness(id, signal), retry: false });
  const [title, setTitle] = useState("");
  const [summary, setSummary] = useState("");
  const [author, setAuthor, remember] = useAuthor();
  const [maps, setMaps] = useState("");
  const [bots, setBots] = useState("");
  const [done, setDone] = useState<{ pkg: Package; form: Form } | null>(null);
  const { working, run } = useBusy();
  // MCP: screenshots - the automatic cover card, playtest shots (F5 in the game), pictures added
  const [cover, setCover] = useState("");
  const [useCover, setUseCover] = useState(true);
  const [picked, setPicked] = useState<Shot[]>([]);
  const play = useQuery({ queryKey: ["tools", "community", "playtest-shots"], queryFn: () => getJson<{ shots?: (Shot & { map: string; at: number })[]; hint?: string; error?: string }>("/api/tools/community/playtest-shots"), staleTime: 10_000 });
  const tags = [
    ...(q.data?.requirements?.known?.creatures?.length ? ["Creatures"] : []),
    ...(bots && bots !== "none" ? ["With bots"] : []),
    ...(q.data?.requirements?.maps?.length ? [`${q.data.requirements.maps.length} map${q.data.requirements.maps.length === 1 ? "" : "s"}`] : []),
  ];
  useEffect(() => {
    let gone = false;
    // drawn again whenever the name, the line or the tags change
    const t = setTimeout(() => void drawCover({ name: title, line: summary, tags }).then((d) => !gone && setCover(d)), 250);
    return () => {
      gone = true;
      clearTimeout(t);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [title, summary, tags.join("|")]);
  const shotsToSend = (): Shot[] => [...(useCover && cover ? [{ name: "cover.png", data: cover }] : []), ...picked].slice(0, MAX_SHOTS);
  const togglePick = (s: Shot) => setPicked((p) => (p.some((x) => x.name === s.name) ? p.filter((x) => x.name !== s.name) : p.length + (useCover ? 1 : 0) >= MAX_SHOTS ? (toast(`At most ${MAX_SHOTS} screenshots.`, true), p) : [...p, s]));
  const addFiles = async (files: FileList | null) => {
    for (const f of Array.from(files ?? [])) {
      if (!/^image\/(png|jpeg)$/.test(f.type)) {
        toast(`${f.name} isn't a PNG or JPEG picture.`, true);
        continue;
      }
      if (f.size > MAX_SHOT) {
        toast(`${f.name} is over 8 MB.`, true);
        continue;
      }
      const data = await new Promise<string>((ok) => {
        const fr = new FileReader();
        fr.onload = () => ok(String(fr.result));
        fr.onerror = () => ok("");
        fr.readAsDataURL(f);
      });
      if (data) togglePick({ name: f.name, data });
    }
  };
  useEffect(() => {
    if (!q.data) return;
    setTitle((t) => t || q.data.name);
    setSummary((s) => s || q.data.description);
    setBots((b) => b || q.data.botsSuggested || "");
  }, [q.data]);
  const head = <Head title={q.data?.name ?? id} kind="Game Mode Studio mode" onBack={onBack} busy={q.isFetching} onRecheck={() => q.refetch()} />;
  if (q.isPending) return <div className="cm-pub">{head}<Loading text="Checking the mode…" /></div>;
  if (q.isError) return <div className="cm-pub">{head}<LoadError text={errorText(q.error, "Couldn't check that mode.")} onRetry={() => q.refetch()} /></div>;
  if (done) return <div className="cm-pub">{head}<Upload pkg={done.pkg} form={done.form} onDone={onDone} /></div>;
  const r = q.data;
  const blocked = r.blockers.length > 0;
  const checks: Check[] = blocked
    ? r.blockers.map((b) => ({ state: "bad" as const, title: "The graph has problems", why: b, fix: { label: "Open in the Studio", to: `studio:${id}` } }))
    : [{ state: "ok", title: "The graph", why: `Passes the Studio's checks (${r.nodes} nodes).` }];
  // (MCP: the mode's requirements: what players may not have)
  for (const w of r.requirementWarnings ?? []) checks.push({ state: "warn", title: "Requirements", why: w, fix: { label: "Open in the Studio", to: `studio:${id}` } });
  // what players need beyond the game: the package and its page say so (QA4-G F6)
  const nd = r.requirements?.needs;
  if (nd && (nd.componentsPlugin || nd.creatures.length || nd.features.length)) {
    const brain = [nd.modeVer ? `bots version ${nd.modeVer}` : "", nd.creatureVer ? `creatures version ${nd.creatureVer}` : ""].filter(Boolean).join(" and ");
    const parts = [nd.componentsPlugin ? "Loner-Strike's components" : "", nd.creatures.length ? `creatures (${nd.creatures.includes("all") ? "the wave spawner's types" : nd.creatures.join(", ")})` : "", brain ? `the bot brain, ${brain} or newer (${nd.features.map((f) => f.name).join("; ")})` : ""].filter(Boolean);
    checks.push({ state: "ok", title: "What players need", why: `Needs ${parts.join(", ")}. The package, its README and the page say so; players with an older Loner-Strike are told to update.` });
  }
  if (r.remixOf) checks.push({ state: "ok", title: "Remix credit", why: `A remix of ${r.remixOf.name}${r.remixOf.author ? ` by ${r.remixOf.author}` : ""}: the package and its page credit them.` });
  const build = () =>
    run("Packing…", async () => {
  const cr = creatureCheck(r.creatures);
  if (cr) checks.push(cr);
      remember();
      const res = await publishMode(id, { title, summary, author, maps: maps.split(/[\s,]+/).filter(Boolean), screenshots: shotsToSend(), bots });
      if (res.error) return toast(res.error, true);
      setDone({ pkg: res.package, form: res.form });
    });
  return (
    <div className="cm-pub">
      {head}
      <Step n={1} title="Checks"><Checks rows={checks} /></Step>
      <Step n={2} title="Details">
        <Details title={title} setTitle={setTitle} author={author} setAuthor={setAuthor} summary={summary} setSummary={setSummary} about="How it plays, in a sentence or two" />
      </Step>
      <Step n={3} title="How well bots play it" note={`${BOT_RATING_INFO}${r.botsSuggested ? " The Studio suggested one from the graph." : ""}`}>
        <Chips
          choices={[{ id: "", name: "Not saying" }, ...(["great", "ok", "weak", "none"] as const).map((k) => ({ id: k, name: BOT_NOTE[k] }))]}
          picked={[bots]}
          onToggle={setBots}
          label="Bot rating"
        />
      </Step>
      <Step n={4} title="Maps it's made for" note="Optional: map names, separated by commas.">
        <input className="cm-wide-input" value={maps} onChange={(e) => setMaps(e.target.value)} placeholder="de_dust2, cs_office" aria-label="Maps it's made for" />
      </Step>
      <Step n={5} title="Screenshots" note="PNG or JPEG, up to 8 MB each, up to 6. The cover card comes first; saved next to the package for GameBanana's picture upload.">
        <div className="cm-shots">
          <figure>
            {cover ? <img src={cover} alt="The cover card" /> : <span className="muted small-note"><span className="spinner" aria-hidden="true" />Drawing the cover card…</span>}
            <label className="small-note">
              <input type="checkbox" checked={useCover} onChange={(e) => setUseCover(e.target.checked)} /> Use the cover card (drawn from the name and the line)
            </label>
          </figure>
          {picked.filter((p) => !(play.data?.shots ?? []).some((x) => x.name === p.name)).map((p) => (
            <figure key={p.name}>
              <img src={p.data} alt={p.name} />
              <button type="button" className="btn small" onClick={() => togglePick(p)} aria-label={`Remove ${p.name}`}>Remove</button>
            </figure>
          ))}
          {picked.length + (useCover ? 1 : 0) < MAX_SHOTS && (
            <label className="btn cm-add">
              Add pictures
              <input type="file" accept="image/png,image/jpeg" multiple hidden onChange={(e) => { void addFiles(e.target.files); e.target.value = ""; }} />
            </label>
          )}
        </div>
        <h4>From your playtests</h4>
        {play.isPending ? (
          <p className="muted small-note"><span className="spinner" aria-hidden="true" />Looking for playtest screenshots…</p>
        ) : (play.data?.shots ?? []).length === 0 ? (
          <p className="muted small-note">{play.data?.error ?? play.data?.hint ?? "No playtest screenshots yet."}</p>
        ) : (
          <>
            <p className="muted small-note">{play.data?.hint} Click to pick.</p>
            <div className="cm-shots">
              {(play.data?.shots ?? []).map((sh) => {
                const on = picked.some((p) => p.name === sh.name);
                return (
                  <figure key={sh.name} className={on ? "cm-shot-on" : undefined}>
                    <button type="button" className="cm-shot-pick" aria-pressed={on} onClick={() => togglePick({ name: sh.name, data: sh.data })} title={`${sh.map}${on ? " (picked)" : ""}`}>
                      <img src={sh.data} alt={`${sh.map} playtest screenshot`} />
                    </button>
                    <figcaption className="small-note">{on ? "Picked · " : ""}{sh.map}</figcaption>
                  </figure>
                );
              })}
            </div>
          </>
        )}
      </Step>
      <Action note={blocked ? "Fix the graph first." : !title.trim() || !author.trim() ? "Give it a name and say who made it." : "Next: upload it to GameBanana and register the page."}>
        <BusyButton className="btn primary" busy={working !== null} busyLabel={working ?? "Packing…"} label="Make the package" disabled={blocked || !title.trim() || !author.trim()} onClick={build} />
      </Action>
    </div>
  );
}

// ---- a prefab -------------------------------------------------------------------------------------

type PrefabReady = { id: string; name: string; description: string; author: string; tags: string[]; brushes: number; entities: number; credits: AssetCredit[]; creatures?: string[]; blockers?: string[]; warnings?: string[] };

function PublishPrefab({ id, onBack, onDone }: { id: string; onBack: () => void; onDone: () => void }) {
  const toast = useToast();
  const q = useQuery({ queryKey: ["tools", "community", "ready-prefab", id], queryFn: () => getJson<PrefabReady & { error?: string }>(`/api/tools/community/publish-prefab/${encodeURIComponent(id)}`), retry: false });
  const [title, setTitle] = useState("");
  const [summary, setSummary] = useState("");
  const [author, setAuthor, remember] = useAuthor();
  const [done, setDone] = useState<{ pkg: Package; form: Form } | null>(null);
  const { working, run } = useBusy();
  useEffect(() => {
    if (!q.data || q.data.error) return;
    setTitle((t) => t || q.data.name);
    setSummary((s) => s || q.data.description);
    if (q.data.author) setAuthor(author || q.data.author);
  }, [q.data]); // eslint-disable-line react-hooks/exhaustive-deps
  const head = <Head title={q.data?.name ?? id.replace(/^mine:/, "")} kind="Prefab" onBack={onBack} busy={q.isFetching} onRecheck={() => q.refetch()} />;
  if (q.isPending) return <div className="cm-pub">{head}<Loading text="Reading the prefab…" /></div>;
  if (q.isError || q.data.error) return <div className="cm-pub">{head}<LoadError text={q.data?.error ?? errorText(q.error, "Couldn't read that prefab.")} onRetry={() => q.refetch()} /></div>;
  if (done) return <div className="cm-pub">{head}<Upload pkg={done.pkg} form={done.form} onDone={onDone} /></div>;
  const r = q.data;
  const build = () =>
    run("Packing…", async () => {
      remember();
      const res = await post<{ package: Package; form: Form }>(`/api/tools/community/publish-prefab/${encodeURIComponent(id)}`, { title, summary, author, modes: [], screenshots: [] });
      if (res.error) return toast(res.error, true);
      setDone({ pkg: res.package, form: res.form });
    });
  return (
    <div className="cm-pub">
      {head}
      <Step n={1} title="Checks">
        <Checks rows={[
          { state: "ok", title: "The prefab", why: `${r.brushes} brushes${r.entities ? `, ${r.entities} entities` : ""}; a plain .map any GoldSrc editor can paste.` },
          r.credits.length
            ? { state: "ok", title: "Credits", why: `Credits ${r.credits.length} source${r.credits.length === 1 ? "" : "s"} of models and textures it uses.` }
            : { state: "ok", title: "Credits", why: "Uses only the game's own files and yours." },
        ]} />
      </Step>
      <Step n={2} title="Details">
          ...(r.blockers ?? []).map((b) => ({ state: "bad" as const, title: "Can't publish yet", why: b })),
          ...[creatureCheck(r.creatures)].filter((c): c is Check => !!c),
          ...(r.warnings ?? []).map((w) => ({ state: "warn" as const, title: "Creature names", why: w })),
        <Details title={title} setTitle={setTitle} author={author} setAuthor={setAuthor} summary={summary} setSummary={setSummary} about="What it is, in a sentence" />
      </Step>
      {r.credits.length > 0 && (
        <Step n={3} title="Credits" note="Carried in the package, so whoever uses the prefab credits these too.">
          <ul className="cm-credit-list">
            {r.credits.map((c, i) => (
              <li key={i}><strong>{c.name || "Files"}</strong> <span className="muted small-note">{c.authors.map((a) => a.name).join(", ")} · {c.files.join(", ")}</span></li>
            ))}
          </ul>
        </Step>
      )}
      <Action note={!title.trim() || !author.trim() ? "Give it a name and say who made it." : "Next: upload it to GameBanana's Prefabs and register the page."}>
        <BusyButton className="btn primary" busy={working !== null} busyLabel={working ?? "Packing…"} label="Make the package" disabled={!title.trim() || !author.trim()} onClick={build} />
      </Action>
    </div>
  );
}

// ---- after packing: upload and register --------------------------------------------------------------

function CopyRow({ label, value, long }: { label: string; value: string; long?: boolean }) {
  const toast = useToast();
  return (
    <div className="cm-copy">
      <span className="muted small-note">{label}</span>
      {long ? <textarea readOnly rows={6} value={value} /> : <input readOnly value={value} />}
      <button type="button" className="btn small" onClick={async () => toast((await copyText(value)) ? "Copied." : "Couldn't copy.", false)}>Copy</button>
    </div>
  );
}

function Upload({ pkg, form, onDone }: { pkg: Package; form: Form; onDone: () => void }) {
  const toast = useToast();
  const [url, setUrl] = useState("");
  // (terms: registering is where the right to share matters most, so it's a required tick)
  const [agree, setAgree] = useState(false);
  const { working, run } = useBusy();
  const [result, setResult] = useState<{ state: MineState; error?: string | null; copy: string } | null>(null);
  const [studio, setStudio] = useState(false);

  const show = async () => {
    const r = await reveal(pkg.file);
    if (r.error) toast(r.error, true);
  };
  const send = () =>
    run("Checking the page…", async () => {
      const r = await register(url.trim(), pkg.file);
      setStudio(!!r.studio);
      if (r.error && !r.state) return toast(r.error, true);
      setResult({ state: r.state, error: r.error, copy: r.copy });
    });

  return (
    <>
      <Step n={1} title="The package is ready" note={`${pkg.file} · ${sizeText(pkg.size)} · ${pkg.contents.length} files${pkg.screenshots.length ? ` · ${pkg.screenshots.length} screenshots beside it` : ""}`}>
        {pkg.warnings.map((w) => <p key={w} className="cm-line warn">{w}</p>)}
        <div className="row"><button className="btn" onClick={show}>Show it in the folder</button></div>
      </Step>
      <Step n={2} title="Upload it to GameBanana" note={`Sign in there and choose ${form.game} > ${form.category}. Upload ${form.file} unchanged and add the screenshots.`}>
        <div className="row"><a className="btn primary" href={form.submitUrl} target="_blank" rel="noreferrer">Open GameBanana's submit page</a></div>
        <div className="cm-copies">
          <CopyRow label="Name" value={form.title} />
          <CopyRow label="Description" value={form.text} long />
          <CopyRow label={`Attribute: ${form.tag.title}`} value={form.tag.value} />
          {form.credits && form.credits.length > 0 && (
            <CopyRow label="Credits module (group | name | role)" value={form.credits.map((c) => `${c.group} | ${c.name} | ${c.role}${c.url ? ` | ${c.url}` : ""}`).join("\n")} long />
          )}
        </div>
        <p className="muted small-note">The attribute <code>{form.tag.title}: {form.tag.value}</code> is how Community finds it.</p>
      </Step>
      <Step n={3} title="Register the page" note="Paste the page's link once GameBanana has checked the file.">
        <form className="cm-register" onSubmit={(e) => { e.preventDefault(); send(); }}>
          <input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://gamebanana.com/mods/…" aria-label="GameBanana link" />
          <label className="check-row cm-agree">
            <input type="checkbox" checked={agree} onChange={(e) => setAgree(e.target.checked)} required />
            <span>I have the right to share this, and I accept the <a href="https://lonerstrike.com/terms" target="_blank" rel="noreferrer">Terms</a>.</span>
          </label>
          <div className="row">
            <BusyButton type="submit" className="btn primary" busy={working !== null} busyLabel={working ?? "Checking…"} label="Add to Community" disabled={!url.trim() || !agree} />
          </div>
        </form>
        {studio && (
          <div className="cm-result" role="status">
            <p className="cm-line warn">Plugin modes can't be shared yet. A mode made in Game Mode Studio can: the app builds its code on each player's computer.</p>
            <button className="btn primary" onClick={() => useNav.getState().go("tools", "studio")}>Open in Game Mode Studio</button>
          </div>
        )}
        {result && (
          <div className="cm-result" role="status">
            {result.state === "listed" || result.state === "sent" ? (
              <p className="cm-line ok">Listed. It shows up in Community within a few minutes, once the Loner-Strike list is next updated.</p>
            ) : result.state === "pending" ? (
              <p className="cm-line ok">Sent. A Loner-Strike moderator looks at it first; Published shows when it's listed.</p>
            ) : result.state === "refused" ? (
              <p className="cm-line bad">The Loner-Strike list turned it down: {result.error || "no reason given."}</p>
            ) : (
              <>
                <p className="cm-line warn">The page checks out, but the Loner-Strike list didn't answer. It's kept here and sent automatically later; you can also post this in the Loner-Strike Discord:</p>
                <CopyRow label="Entry" value={result.copy} long />
              </>
            )}
            <button className="btn" onClick={onDone}>See what you've published</button>
          </div>
        )}
      </Step>
    </>
  );
}
