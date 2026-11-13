// Publish: package a map you made for GameBanana, then register its page so it shows up in
// Community with its badges. Uploading happens on GameBanana's site (it has no upload API);
// the app prepares everything to paste and checks the page afterwards.
//
// Other tools hand over a map with go("tools", "community:publish:<map>"), and the Game Mode
// Studio a mode with go("tools", "community:publish:mode:<id>").

import { useQuery } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { copyText } from "../../../api";
import { useNav } from "../../../store";
import type { MapInfo } from "../../../types";
import { BusyButton, LoadError, useToast } from "../../../ui";
import { errorText, useBusy } from "../common/api";
import { Empty, Loading, MapList } from "../common";
import { Segmented } from "../../../ui";
import { BOT_NOTE, BOT_RATING_INFO } from "../../../games";
import { modeReadiness, publish, publishMode, readiness, register, reveal, sizeText, startModes, studioGraphs, type Form, type MineState, type Package, type Shot } from "./communityApi";

const MAX_SHOTS = 6;
const MAX_SHOT = 8 << 20;

export default function Publish({ maps, map, onMap, onDone }: { maps: MapInfo[]; map?: string; onMap: (m: string | undefined) => void; onDone: () => void }) {
  const [what, setWhat] = useState<"map" | "mode">(map?.startsWith("mode:") ? "mode" : "map");
  if (map?.startsWith("mode:")) return <PublishMode id={map.slice(5)} onBack={() => onMap(undefined)} onDone={onDone} />;
  if (!map)
    return (
      <div className="stack-tight">
        <Segmented label="What to publish" values="map mode" labels="A map|A game mode" current={what} onPick={(v) => setWhat(v as "map" | "mode")} />
        {what === "map" ? (
          <>
            <p className="muted measure">
              Share a map you made. Loner-Strike checks it, packs it with its bot nav, map knowledge, download list and overview, and tells you exactly what to put on GameBanana. Once it's up, paste the link and it shows up here for everyone.
            </p>
            <MapList maps={maps} onPick={onMap} placeholder="Pick a map to publish" filter={(m) => m.source !== "stock"} none="Only maps you made or added can be published; the game's own maps can't." note={(m) => (m.nav ? null : "no bot nav yet")} />
          </>
        ) : (
          <ModeList onPick={(id) => onMap(`mode:${id}`)} />
        )}
      </div>
    );
  return <PublishMap map={map} onBack={() => onMap(undefined)} onDone={onDone} />;
}

/** Your Game Mode Studio modes (remixes included): only the graph is shared, each player's app builds it. */
function ModeList({ onPick }: { onPick: (id: string) => void }) {
  const q = useQuery({ queryKey: ["tools", "community", "studio-graphs"], queryFn: () => studioGraphs(), retry: false });
  if (q.isPending) return <Loading text="Loading your modes…" />;
  if (q.data?.error) return <LoadError text={q.data.error} onRetry={() => q.refetch()} />;
  const list = (q.data?.graphs ?? []).filter((g) => !g.damaged);
  return (
    <div className="stack-tight">
      <p className="muted measure">
        Share a mode you made in the Game Mode Studio (or a remix of someone's). Only its graph goes in the package: each player's Loner-Strike builds the mode on their own computer, so nothing that runs code is shared.
      </p>
      {list.length === 0 ? (
        <Empty title="No Game Mode Studio modes yet" text="Make one in Tools > Game Mode Studio, or remix one you installed from Community." >
          <button className="btn" onClick={() => useNav.getState().go("tools", "studio")}>Open Game Mode Studio</button>
        </Empty>
      ) : (
        <ul className="tl-maps">
          {list.map((g) => (
            <li key={g.id}>
              <button className="tl-map" onClick={() => onPick(g.id)}>
                <span className="tl-map-name">{g.name}</span>
                <span className="muted small-note">{g.id}{g.nodes != null ? ` · ${g.nodes} nodes` : ""}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

function PublishMode({ id, onBack, onDone }: { id: string; onBack: () => void; onDone: () => void }) {
  const toast = useToast();
  const q = useQuery({ queryKey: ["tools", "community", "ready-mode", id], queryFn: ({ signal }) => modeReadiness(id, signal), retry: false });
  const [title, setTitle] = useState("");
  const [summary, setSummary] = useState("");
  const [author, setAuthor] = useState(() => {
    try {
      return localStorage.getItem("ls.community.author") ?? "";
    } catch {
      return "";
    }
  });
  const [maps, setMaps] = useState("");
  const [bots, setBots] = useState("");
  const [done, setDone] = useState<{ pkg: Package; form: Form } | null>(null);
  const { working, run } = useBusy();
  useEffect(() => {
    if (!q.data) return;
    setTitle((t) => t || q.data.name);
    setSummary((s) => s || q.data.description);
    setBots((b) => b || q.data.botsSuggested || "");
  }, [q.data]);
  const head = (
    <div className="tl-head">
      <div className="tl-head-title">
        <h3>Publish {q.data?.name ?? id}</h3>
        <span className="muted">Game Mode Studio mode</span>
      </div>
      <div className="tl-head-actions">
        <button className="btn" onClick={onBack}>Pick something else</button>
        <BusyButton className="btn" busy={q.isFetching} label="Check again" busyLabel="Checking…" onClick={() => q.refetch()} />
      </div>
    </div>
  );
  if (q.isPending) return <div className="panel">{head}<Loading text="Checking the mode…" /></div>;
  if (q.isError) return <div className="panel">{head}<LoadError text={errorText(q.error, "Couldn't check that mode.")} onRetry={() => q.refetch()} /></div>;
  if (done) return <div className="panel">{head}<Upload pkg={done.pkg} form={done.form} onDone={onDone} /></div>;
  const r = q.data;
  const build = () =>
    run("Packing…", async () => {
      try {
        localStorage.setItem("ls.community.author", author.trim());
      } catch {
        /* remembered next time, maybe */
      }
      const res = await publishMode(id, { title, summary, author, maps: maps.split(/[\s,]+/).filter(Boolean), screenshots: [], bots });
      if (res.error) return toast(res.error, true);
      setDone({ pkg: res.package, form: res.form });
    });
  return (
    <div className="panel">
      {head}
      <section className="cm-ready">
        {r.blockers.map((b) => <p key={b} className="cm-line bad">{b}</p>)}
        {r.blockers.length === 0 && <p className="cm-line ok">The graph passes the Studio's checks ({r.nodes} nodes).</p>}
        {r.blockers.length > 0 && (
          <p className="cm-line tip">Fix these in the Game Mode Studio first. <button className="btn small" onClick={() => useNav.getState().go("tools", `studio:${id}`)}>Open it</button></p>
        )}
        {r.remixOf && <p className="cm-line tip">A remix of {r.remixOf.name}{r.remixOf.author ? ` by ${r.remixOf.author}` : ""}: the package and its page credit them.</p>}
      </section>
      {r.blockers.length === 0 && (
        <form className="cm-form" onSubmit={(e) => { e.preventDefault(); build(); }}>
          <label className="cm-field">
            <span>Name</span>
            <input value={title} maxLength={80} onChange={(e) => setTitle(e.target.value)} placeholder="What players will see" />
          </label>
          <label className="cm-field">
            <span>Made by</span>
            <input value={author} maxLength={40} onChange={(e) => setAuthor(e.target.value)} placeholder="Your GameBanana name" />
          </label>
          <label className="cm-field cm-wide">
            <span>About it</span>
            <textarea value={summary} maxLength={400} rows={3} onChange={(e) => setSummary(e.target.value)} placeholder="How it plays, in a sentence or two" />
          </label>
          <label className="cm-field">
            <span>How well bots play it</span>
            <select value={bots} onChange={(e) => setBots(e.target.value)}>
              <option value="">Not saying</option>
              {(["great", "ok", "weak", "none"] as const).map((r) => <option key={r} value={r}>{BOT_NOTE[r]}</option>)}
            </select>
            <span className="muted small-note">{BOT_RATING_INFO}{r.botsSuggested ? " The Studio suggested this from the graph." : ""}</span>
          </label>
          <label className="cm-field cm-wide">
            <span>Maps it's made for (optional)</span>
            <input value={maps} onChange={(e) => setMaps(e.target.value)} placeholder="de_dust2, cs_office" />
          </label>
          <p className="muted small-note cm-wide">Add screenshots on GameBanana's page itself (a Playtest shot works well).</p>
          <div className="cm-wide row">
            <BusyButton type="submit" className="btn primary" busy={working !== null} busyLabel={working ?? "Packing…"} label="Make the package" disabled={!title.trim() || !author.trim()} />
          </div>
        </form>
      )}
    </div>
  );
}

function PublishMap({ map, onBack, onDone }: { map: string; onBack: () => void; onDone: () => void }) {
  const toast = useToast();
  const q = useQuery({ queryKey: ["tools", "community", "ready", map], queryFn: ({ signal }) => readiness(map, signal), retry: false });
  const modes = useQuery({ queryKey: ["tools", "community", "modes"], queryFn: () => startModes(), staleTime: 60_000 });
  const [title, setTitle] = useState("");
  const [summary, setSummary] = useState("");
  const [author, setAuthor] = useState(() => {
    try {
      return localStorage.getItem("ls.community.author") ?? "";
    } catch {
      return "";
    }
  });
  const [picked, setPicked] = useState<string[]>([]);
  const [shots, setShots] = useState<Shot[]>([]);
  const [done, setDone] = useState<{ pkg: Package; form: Form } | null>(null);
  const { working, run } = useBusy();

  useEffect(() => {
    if (!q.data) return;
    setTitle((t) => t || q.data.ready.title);
    setPicked((p) => (p.length ? p : q.data.suggestedModes));
  }, [q.data]);

  const head = (
    <div className="tl-head">
      <div className="tl-head-title">
        <h3>Publish {map}</h3>
      </div>
      <div className="tl-head-actions">
        <button className="btn" onClick={onBack}>Pick another map</button>
        <BusyButton className="btn" busy={q.isFetching} label="Check again" busyLabel="Checking…" onClick={() => q.refetch()} />
      </div>
    </div>
  );
  if (q.isPending) return <div className="panel">{head}<Loading text={`Checking ${map}…`} /></div>;
  if (q.isError) return <div className="panel">{head}<LoadError text={errorText(q.error, "Couldn't check that map.")} onRetry={() => q.refetch()} /></div>;
  const r = q.data.ready;
  if (done) return <div className="panel">{head}<Upload pkg={done.pkg} form={done.form} onDone={onDone} /></div>;

  const addShots = async (files: FileList | null) => {
    if (!files) return;
    const room = MAX_SHOTS - shots.length;
    const list = Array.from(files).slice(0, Math.max(0, room));
    const read: Shot[] = [];
    for (const f of list) {
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
      try {
        localStorage.setItem("ls.community.author", author.trim());
      } catch {
        /* remembered next time, maybe */
      }
      const res = await publish(map, { title, summary, author, modes: picked, screenshots: shots });
      if (res.error) return toast(res.error, true);
      setDone({ pkg: res.package, form: res.form });
    });

  const allModes = modes.data?.modes ?? [];
  const toggle = (id: string) => setPicked((p) => (p.includes(id) ? p.filter((x) => x !== id) : [...p, id]));
  const blocked = r.blockers.length > 0;

  return (
    <div className="panel">
      {head}
      <section className="cm-ready">
        {r.blockers.map((b) => <p key={b} className="cm-line bad">{b}</p>)}
        {r.warnings.map((w) => <p key={w} className="cm-line warn">{w}</p>)}
        {!blocked && r.warnings.length === 0 && <p className="cm-line ok">Ready to share.</p>}
        {r.tips.map((t) => <p key={t} className="cm-line tip">{t}</p>)}
        <p className="muted small-note">
          Goes in the package: the map{r.map.nav ? ", its bot nav" : ""}{r.map.lsv ? ", finished bot map knowledge" : ""}, a fresh download list (.res)
          {r.files.length ? ` and ${r.files.length} file${r.files.length === 1 ? "" : "s"} it uses (${r.files.slice(0, 4).join(", ")}${r.files.length > 4 ? "…" : ""})` : ""}.
          {r.health ? ` Bot simulation from ${new Date(r.health.when * 1000).toLocaleDateString()}: ${r.health.verdict}.` : ""}
        </p>
      </section>

      {!blocked && (
        <form className="cm-form" onSubmit={(e) => { e.preventDefault(); build(); }}>
          <label className="cm-field">
            <span>Name</span>
            <input value={title} maxLength={80} onChange={(e) => setTitle(e.target.value)} placeholder="What players will see" />
          </label>
          <label className="cm-field">
            <span>Made by</span>
            <input value={author} maxLength={40} onChange={(e) => setAuthor(e.target.value)} placeholder="Your GameBanana name" />
          </label>
          <label className="cm-field cm-wide">
            <span>About it</span>
            <textarea value={summary} maxLength={400} rows={3} onChange={(e) => setSummary(e.target.value)} placeholder="One or two sentences: what it is and what makes it fun" />
          </label>
          <fieldset className="cm-field cm-wide">
            <legend>Game modes it's made for</legend>
            {allModes.length ? (
              <div className="cm-modes">
                {allModes.map((m) => (
                  <label key={m.id} className="check-row">
                    <input type="checkbox" checked={picked.includes(m.id)} onChange={() => toggle(m.id)} /> {m.name}
                  </label>
                ))}
              </div>
            ) : (
              <p className="muted small-note">{modes.isPending ? "Loading modes…" : "No modes found in your game; it'll be listed for the usual mode."}</p>
            )}
          </fieldset>
          <div className="cm-field cm-wide">
            <span>Screenshots ({shots.length}/{MAX_SHOTS})</span>
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
                  <input type="file" accept="image/png,image/jpeg" multiple hidden onChange={(e) => { addShots(e.target.files); e.target.value = ""; }} />
                </label>
              )}
            </div>
            <p className="muted small-note">PNG or JPEG, up to 8 MB each. Shots from the map viewer work well. They're saved next to the package for GameBanana's picture upload.</p>
          </div>
          <div className="cm-wide row">
            <BusyButton type="submit" className="btn primary" busy={working !== null} busyLabel={working ?? "Packing…"} label="Make the package" disabled={!title.trim() || !author.trim()} />
          </div>
        </form>
      )}
    </div>
  );
}

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
    <div className="cm-upload stack-tight">
      <p className="cm-line ok">Package made: <code>{pkg.file}</code> ({sizeText(pkg.size)}, {pkg.contents.length} files{pkg.screenshots.length ? `, ${pkg.screenshots.length} screenshots beside it` : ""}).</p>
      {pkg.warnings.map((w) => <p key={w} className="cm-line warn">{w}</p>)}
      <ol className="cm-steps">
        <li>
          <strong>Find the package.</strong> <button className="btn small" onClick={show}>Show it in the folder</button>
        </li>
        <li>
          <strong>Open GameBanana's submit page</strong> (sign in there), choose {form.game} &gt; {form.category}.{" "}
          <a className="btn small primary" href={form.submitUrl} target="_blank" rel="noreferrer">Open GameBanana</a>
          <div className="cm-copies">
            <CopyRow label="Name" value={form.title} />
            <CopyRow label="Description" value={form.text} long />
            <CopyRow label={`Attribute: ${form.tag.title}`} value={form.tag.value} />
          </div>
          <p className="muted small-note">
            Add the attribute <code>{form.tag.title}: {form.tag.value}</code> (it's how Community finds it), upload <code>{form.file}</code> unchanged, and add the screenshots.
          </p>
        </li>
        <li>
          <strong>Paste the page's link</strong> once GameBanana has checked the file:
          <form className="row cm-register" onSubmit={(e) => { e.preventDefault(); send(); }}>
            <input value={url} onChange={(e) => setUrl(e.target.value)} placeholder="https://gamebanana.com/mods/…" aria-label="GameBanana link" />
            <BusyButton type="submit" className="btn primary" busy={working !== null} busyLabel={working ?? "Checking…"} label="Add to Community" disabled={!url.trim()} />
          </form>
        </li>
      </ol>
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
            <p className="cm-line ok">Sent. A Loner-Strike moderator looks at it first; Yours shows when it's listed.</p>
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
    </div>
  );
}
