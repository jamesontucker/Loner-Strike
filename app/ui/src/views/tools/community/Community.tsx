// Community creations: maps, game modes, prefabs, sprays and textures made with Loner-Strike.
// Browse finds them (GameBanana pages tagged for Loner-Strike, plus the Loner-Strike index's
// extras), Try it installs one with the app's own safe installer and starts a game with bots,
// Publish packages your map for GameBanana, Yours lists what you published.
//
// Addresses (go("tools", ...)): "community", "community:publish", "community:publish:<map>"
// (the editor or nav fixer handing over a map), "community:yours", "community:mod:<id>" /
// "community:spray:<id>" (one creation).

import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { getJson } from "../../../api";
import { useToolArg } from "../../../stores/tools";
import { useNav } from "../../../store";
import type { GbJob } from "../../../types";
import { Banner, BusyButton, LoadError, Segmented, useToast } from "../../../ui";
import { errorText, useBusy } from "../common/api";
import { useReportAddress } from "../common/address";
import { useToolCrumbs, type Crumb } from "../common/shell";
import { Empty, Loading } from "../common";
import type { ToolProps } from "../registry";
import {
  BADGE_LABEL, KIND_LABEL, NOTE_MAX, REPORT_REASONS, ago, browse, details, launch, report, sizeText, startModes, tryIt,
  type Details, type Entry, type Model, type ReportReason, type Trust,
} from "./communityApi";
import Publish from "./Publish";
import Yours from "./Yours";
import "./community.css";

type Tab = "browse" | "publish" | "yours";

function fromArg(arg: string | null): { tab: Tab; map?: string; open?: { model: Model; id: number } } {
  if (!arg) return { tab: "browse" };
  const [a, b] = [arg.split(":")[0], arg.split(":").slice(1).join(":")];
  if (a === "publish") return { tab: "publish", map: b || undefined };
  if (a === "yours") return { tab: "yours" };
  if ((a === "mod" || a === "spray") && /^\d+$/.test(b)) return { tab: "browse", open: { model: a === "mod" ? "Mod" : "Spray", id: Number(b) } };
  return { tab: "browse" };
}

export default function Community({ maps }: ToolProps) {
  const arg = useToolArg();
  const start = fromArg(arg);
  const [tab, setTab] = useState<Tab>(start.tab);
  const [publishMap, setPublishMap] = useState<string | undefined>(start.map);
  const [open, setOpen] = useState<{ model: Model; id: number } | null>(start.open ?? null);
  useEffect(() => {
    const s = fromArg(arg);
    setTab(s.tab);
    if (s.map) setPublishMap(s.map);
    if (s.open) setOpen(s.open);
  }, [arg]);
  // each tab and creation has its own address, so Back and the crumbs step between them
  const shown = tab === "browse" && open ? open : null;
  const item = useQuery({ queryKey: ["tools", "community", "item", shown?.model, shown?.id], queryFn: ({ signal }) => details(shown!.model, shown!.id, signal), retry: false, enabled: !!shown });
  const tabCrumb: Record<Tab, Crumb> = { browse: { label: "Browse", address: "community" }, publish: { label: "Publish", address: "community:publish" }, yours: { label: "Yours", address: "community:yours" } };
  useReportAddress(
    "community",
    shown ? `community:${shown.model === "Mod" ? "mod" : "spray"}:${shown.id}` : tab === "publish" ? `community:publish${publishMap ? `:${publishMap}` : ""}` : tab === "yours" ? "community:yours" : null,
  );
  useToolCrumbs(
    "community",
    shown ? [tabCrumb.browse, { label: item.data?.name ?? "Creation" }] : tab === "publish" && publishMap ? [tabCrumb.publish, { label: publishMap }] : [tabCrumb[tab]],
  );

  return (
    <div className="cm stack-tight">
      <div className="cm-tabs">
        <Segmented label="Community" values="browse,publish,yours" labels="Browse,Publish,Yours" current={tab} onPick={(v) => setTab(v as Tab)} />
      </div>
      {tab === "browse" && (open ? <CreationPage model={open.model} id={open.id} onBack={() => setOpen(null)} /> : <BrowseList onOpen={(e) => setOpen({ model: e.model, id: e.id })} />)}
      {tab === "publish" && <Publish maps={maps} map={publishMap} onMap={setPublishMap} onDone={() => setTab("yours")} />}
      {tab === "yours" && <Yours onPublish={() => setTab("publish")} />}
    </div>
  );
}

// ---- browse --------------------------------------------------------------------------------

function BrowseList({ onOpen }: { onOpen: (e: Entry) => void }) {
  const [kind, setKind] = useState("");
  const [sort, setSort] = useState("new");
  const [text, setText] = useState("");
  const [q, setQ] = useState("");
  useEffect(() => {
    const t = setTimeout(() => setQ(text.trim()), 300);
    return () => clearTimeout(t);
  }, [text]);
  const refresh = useRef(false);
  const list = useQuery({
    queryKey: ["tools", "community", kind, q, sort],
    queryFn: ({ signal }) => {
      const r = refresh.current;
      refresh.current = false;
      return browse(kind, q, sort, r, signal);
    },
    retry: false,
    staleTime: 60_000,
  });
  const again = () => {
    refresh.current = true;
    list.refetch();
  };

  return (
    <>
      <p className="muted measure">
        Maps, game modes and more made with Loner-Strike. Try one and it's installed and ready to play with bots in a click. Only content and Game Mode Studio modes are shared: nothing that runs code.
      </p>
      <div className="toolbar cm-toolbar">
        <input className="search" type="search" aria-label="Search creations" placeholder="Search names, authors, modes" value={text} onChange={(e) => setText(e.target.value)} />
        <Segmented label="Kind" values=",map,mode,prefab,spray,texture" labels="All,Maps,Modes,Prefabs,Sprays,Textures" current={kind} onPick={setKind} />
        <label className="cm-sort">
          <span className="muted small-note">Sort</span>
          <select value={sort} onChange={(e) => setSort(e.target.value)} aria-label="Sort">
            <option value="new">Newest</option>
            <option value="liked">Most liked</option>
            <option value="name">Name</option>
          </select>
        </label>
        <BusyButton className="btn" busy={list.isFetching} label="Refresh" busyLabel="Loading…" onClick={again} />
      </div>
      {list.data && <SourceNote data={list.data} />}
      {list.isPending ? (
        <Loading text="Finding creations…" />
      ) : list.isError ? (
        <LoadError text={errorText(list.error, "Couldn't load the community list.")} onRetry={again} />
      ) : list.data.items.length === 0 ? (
        <Empty
          title={q || kind ? "Nothing matches" : "Nothing here yet"}
          text={
            q || kind
              ? "Try other words, or All."
              : <>Nobody has published with Loner-Strike yet. Be the first: Publish packages your map, and the tag <code>{list.data.tag}</code> on its GameBanana page makes it show up here.</>
          }
        />
      ) : (
        <ul className="cm-grid" aria-label="Creations">
          {list.data.items.map((e) => (
            <li key={e.key}>
              <Card e={e} onOpen={() => onOpen(e)} />
            </li>
          ))}
        </ul>
      )}
    </>
  );
}

function SourceNote({ data }: { data: { index: string; gamebanana: string } }) {
  if (data.gamebanana === "offline" && data.index === "none")
    return <Banner>Couldn't reach GameBanana or the Loner-Strike list. Check your internet connection and Refresh.</Banner>;
  if (data.gamebanana === "offline") return <Banner>GameBanana didn't answer, so this is the Loner-Strike list only. Trying one still needs GameBanana.</Banner>;
  if (data.index === "none") return <p className="muted small-note">The Loner-Strike list couldn't be reached: badges (works with bots, checked, modes) are missing for now.</p>;
  if (data.index === "cached") return <p className="muted small-note">Showing the Loner-Strike list from earlier (the download server didn't answer).</p>;
  return null;
}

function Badges({ list, trust }: { list: Entry["badges"]; trust?: Trust }) {
  return (
    <span className="cm-badges">
      {list.map((b) => (
        <span key={b} className={`tag cm-badge cm-badge-${b}`} title={BADGE_LABEL[b].help}>{BADGE_LABEL[b].label}</span>
      ))}
      {trust && (trust.level === "reviewed" || trust.level === "verified" || trust.level === "unreviewed") && (
        <span className={`tag cm-trust cm-trust-${trust.level}`} title={trust.note}>{trust.label}</span>
      )}
    </span>
  );
}

function Card({ e, onOpen }: { e: Entry; onOpen: () => void }) {
  return (
    <button className="cm-card" onClick={onOpen} title={e.summary || e.name}>
      <span className="cm-thumb">{e.image ? <img src={e.image} alt="" loading="lazy" referrerPolicy="no-referrer" /> : <span className="cm-noimg">{KIND_LABEL[e.kind]}</span>}</span>
      <span className="cm-card-body">
        <span className="cm-card-title">{e.name}</span>
        <span className="muted small-note">
          {KIND_LABEL[e.kind]}
          {e.author ? ` · ${e.author}` : ""}
          {e.updated || e.added ? ` · ${ago(e.updated || e.added)}` : ""}
          {e.have ? " · in your game" : ""}
        </span>
        {e.modes.length > 0 && <span className="muted small-note">Modes: {e.modes.join(", ")}</span>}
        <Badges list={e.badges} trust={e.trust} />
      </span>
    </button>
  );
}

// ---- one creation ----------------------------------------------------------------------------

type Stage = { state: "idle" } | { state: "asking" } | { state: "installing"; job: string; got?: number; size?: number; message?: string } | { state: "done"; map: string | null; prefabs?: string[] } | { state: "failed"; message: string };

function CreationPage({ model, id, onBack }: { model: Model; id: number; onBack: () => void }) {
  const q = useQuery({ queryKey: ["tools", "community", "item", model, id], queryFn: ({ signal }) => details(model, id, signal), retry: false });
  const [reporting, setReporting] = useState(false);
  const head = (
    <>
    <div className="tl-head">
      <div className="tl-head-title">
        <h3>{q.data?.name ?? "Creation"}</h3>
        {q.data && <span className="muted">{KIND_LABEL[q.data.kind]}{q.data.author ? ` by ${q.data.author}` : ""}</span>}
      </div>
      <div className="tl-head-actions">
        <button className="btn" onClick={onBack}>Back to the list</button>
        {q.data && <a className="btn" href={q.data.url} target="_blank" rel="noreferrer">Open on GameBanana</a>}
        <button className="btn" aria-expanded={reporting} onClick={() => setReporting((v) => !v)}>Report</button>
      </div>
    </div>
    {reporting && <ReportForm model={model} id={id} name={q.data?.name} onClose={() => setReporting(false)} />}
    </>
  );
  if (q.isPending) return <div className="panel">{head}<Loading text="Opening it…" /></div>;
  if (q.isError) return <div className="panel">{head}<LoadError text={errorText(q.error, "Couldn't open that one.")} onRetry={() => q.refetch()} /></div>;
  return <div className="panel">{head}<Creation d={q.data} /></div>;
}

function Creation({ d }: { d: Details }) {
  const toast = useToast();
  const [stage, setStage] = useState<Stage>({ state: "idle" });
  const [pic, setPic] = useState(0);
  const modes = useQuery({ queryKey: ["tools", "community", "modes"], queryFn: () => startModes(), staleTime: 60_000 });
  const map = d.item?.map?.name ?? null;
  const wanted = d.item?.map?.modes ?? [];
  const playable = useMemo(() => (modes.data?.modes ?? []).filter((m) => wanted.includes(m.id)), [modes.data, wanted]);

  // follow the install job
  useEffect(() => {
    if (stage.state !== "installing") return;
    let gone = false;
    const tick = async () => {
      const list = await getJson<GbJob[]>("/api/gb/jobs").catch(() => [] as GbJob[]);
      const j = Array.isArray(list) ? list.find((x) => x.id === stage.job) : undefined;
      if (gone || !j) return;
      if (j.state === "done") setStage({ state: "done", map: map ?? j.maps?.[0] ?? null });
      else if (j.state === "error") setStage({ state: "failed", message: j.message || "Couldn't install it." });
      else setStage({ state: "installing", job: stage.job, got: j.got, size: j.size, message: j.message });
    };
    const t = setInterval(tick, 1000);
    tick();
    return () => {
      gone = true;
      clearInterval(t);
    };
  }, [stage.state === "installing" ? stage.job : null]); // eslint-disable-line react-hooks/exhaustive-deps

  const go = async (confirmed: boolean) => {
    if (d.trust.confirm && !confirmed) return setStage({ state: "asking" });
    setStage({ state: "installing", job: "" });
    const r = await tryIt(d.model, d.id, confirmed);
    if (!r.error && r.kind === "prefab") {
      // prefabs go straight into the map editor's library (no install job)
      setStage({ state: "done", map: null, prefabs: r.prefabs ?? [] });
      return;
    }
    if (r.error || !r.job) {
      setStage({ state: "failed", message: r.error || "Couldn't start the install." });
      return;
    }
    setStage({ state: "installing", job: r.job });
  };

  const play = async (mode?: string) => {
    const m = stage.state === "done" ? stage.map : map;
    if (!m) return;
    const r = await launch(m, mode);
    if (r.error) toast(r.error, true);
    else toast("Starting the game…");
  };

  const already = d.have && d.kind === "map";
  const done = stage.state === "done" || already;
  const pct = stage.state === "installing" && stage.size ? Math.round(((stage.got ?? 0) / stage.size) * 100) : null;

  return (
    <div className="cm-detail">
      <div className="cm-media">
        {d.images.length > 0 ? (
          <>
            <img className="cm-shot" src={d.images[pic]} alt={`${d.name}, picture ${pic + 1}`} referrerPolicy="no-referrer" />
            {d.images.length > 1 && (
              <div className="cm-thumbs" role="group" aria-label="Pictures">
                {d.images.map((u, i) => (
                  <button key={u} aria-pressed={i === pic} onClick={() => setPic(i)} aria-label={`Picture ${i + 1}`}>
                    <img src={u} alt="" referrerPolicy="no-referrer" />
                  </button>
                ))}
              </div>
            )}
          </>
        ) : (
          <div className="cm-shot cm-noimg">No pictures</div>
        )}
      </div>
      <div className="cm-info stack-tight">
        <Badges list={(d.item ? badgesOf(d) : [])} trust={d.trust} />
        {d.item?.summary && <p className="cm-summary">{d.item.summary}</p>}
        <TrustNote trust={d.trust} listed={!!d.item} />
        {d.item?.map && <BotFacts d={d} />}

        <div className="cm-try">
          {stage.state === "asking" && d.trust.confirm ? (
            <div className="cm-confirm" role="alertdialog" aria-label="Before installing">
              <p>{d.trust.confirm}</p>
              <div className="row">
                <button className="btn primary" onClick={() => go(true)}>Install it</button>
                <button className="btn" onClick={() => setStage({ state: "idle" })}>Cancel</button>
              </div>
            </div>
          ) : stage.state === "installing" ? (
            <div className="cm-progress" aria-live="polite">
              <span className="spinner" aria-hidden="true" />
              <span>{stage.message || "Installing…"}{pct !== null ? ` ${pct}%` : ""}</span>
            </div>
          ) : done && map ? (
            <div className="stack-tight">
              <p className="cm-ok">{already && stage.state !== "done" ? `${map} is in your game.` : "Installed. Ready to play."}</p>
              <div className="row cm-play">
                <button className="btn primary" onClick={() => play()}>Play with bots</button>
                {playable.map((m) => (
                  <button key={m.id} className="btn" onClick={() => play(m.id)}>Play {m.name}</button>
                ))}
              </div>
              {wanted.length > playable.length && modes.data && (
                <p className="muted small-note">Made for {wanted.filter((w) => !playable.some((p) => p.id === w)).join(", ")} too; add {wanted.length - playable.length === 1 ? "that mode" : "those modes"} in Mods to play {wanted.length - playable.length === 1 ? "it" : "them"}.</p>
              )}
            </div>
          ) : stage.state === "done" && stage.prefabs ? (
            <div className="stack-tight">
              <p className="cm-ok">Added to your prefab library: {stage.prefabs.join(", ")}.</p>
              <div className="row">
                <button className="btn primary" onClick={() => useNav.getState().go("tools", "editor:prefabs")}>Open the map editor</button>
              </div>
            </div>
          ) : done ? (
            <p className="cm-ok">Installed.{d.kind === "spray" ? " Pick it in Get more > Sprays to use it." : ""}</p>
          ) : (
            <>
              <button className="btn primary" disabled={!d.canTry} onClick={() => go(false)}>Try it</button>
              {d.why && <p className="muted small-note">{d.why}</p>}
            </>
          )}
          {stage.state === "failed" && <p className="cm-bad" role="alert">{stage.message}</p>}
        </div>

        {d.text && (
          <details className="cm-more" open={!d.item?.summary}>
            <summary>About it (from GameBanana)</summary>
            <p className="cm-text">{d.text}</p>
          </details>
        )}
        {d.files.length > 0 && (
          <details className="cm-more">
            <summary>Files on the page</summary>
            <ul className="cm-files">
              {d.files.map((f) => (
                <li key={f.id}>
                  <code>{f.file}</code>
                  <span className="muted small-note">
                    {sizeText(f.size)} · {ago(f.added)}
                    {!f.clean ? " · flagged by GameBanana's virus check" : !f.supported ? " · not something the app installs" : ""}
                    {d.item?.file === f.id ? " · the one Loner-Strike lists" : ""}
                  </span>
                </li>
              ))}
            </ul>
          </details>
        )}
        <p className="muted small-note">{d.likes} likes · {d.downloads} downloads{d.category ? ` · ${d.category}` : ""}</p>
      </div>
    </div>
  );
}

/** Tells the Loner-Strike moderators something's wrong with a creation: a reason and an optional
 *  note, nothing about you. They can pull it from Community in one step. */
function ReportForm({ model, id, name, onClose }: { model: Model; id: number; name?: string; onClose: () => void }) {
  const [reason, setReason] = useState<ReportReason | "">("");
  const [note, setNote] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [sent, setSent] = useState(false);
  const { working, run } = useBusy();
  const send = () =>
    run("Sending…", async () => {
      if (!reason) return;
      setError(null);
      const r = await report(model, id, reason, note.trim());
      if (r.error) setError(r.error);
      else setSent(true);
    });
  if (sent)
    return (
      <div className="cm-report" role="status">
        <p className="cm-line ok">Thanks. The Loner-Strike moderators have your report{name ? ` about ${name}` : ""} and will take a look.</p>
        <div className="row"><button className="btn" onClick={onClose}>Close</button></div>
      </div>
    );
  return (
    <form className="cm-report" aria-label="Report this creation" onSubmit={(e) => { e.preventDefault(); send(); }}>
      <fieldset>
        <legend>What's wrong with it?</legend>
        {REPORT_REASONS.map((r) => (
          <label key={r.id} className="cm-report-reason">
            <input type="radio" name="reason" value={r.id} checked={reason === r.id} onChange={() => setReason(r.id)} />
            {r.label}
          </label>
        ))}
      </fieldset>
      <label className="cm-report-note">
        <span className="muted small-note">Anything that helps (optional, {NOTE_MAX - note.length} characters left)</span>
        <textarea rows={3} maxLength={NOTE_MAX} value={note} onChange={(e) => setNote(e.target.value)} />
      </label>
      <p className="muted small-note">Only this page, the reason and your note are sent: nothing about you or your computer.</p>
      {error && <p className="cm-bad" role="alert">{error}</p>}
      <div className="row">
        <BusyButton type="submit" className="btn primary" busy={working !== null} busyLabel={working ?? "Sending…"} label="Send report" disabled={!reason} />
        <button type="button" className="btn" onClick={onClose}>Cancel</button>
      </div>
    </form>
  );
}

function badgesOf(d: Details): Entry["badges"] {
  const it = d.item!;
  const b: Entry["badges"] = [];
  if (it.featured) b.push("featured");
  if (it.map?.nav && it.map.lsv) b.push("bots");
  else if (it.map?.nav) b.push("nav");
  if (it.health?.verdict === "good") b.push("checked");
  if (d.kind === "mode" && it.mode && it.mode.plugins.length === 0) b.push("data");
  return b;
}

function TrustNote({ trust, listed }: { trust: Trust; listed: boolean }) {
  const tone = trust.level === "blocked" || trust.level === "unreviewed" ? "bad" : trust.level === "verified" ? "warn" : "ok";
  return (
    <p className={`cm-trustline ${tone}`}>
      <strong>{trust.label}.</strong> {trust.note}
      {!listed && trust.level === "content" ? " It isn't in the Loner-Strike list yet, so there are no bot or health details." : ""}
    </p>
  );
}

function BotFacts({ d }: { d: Details }) {
  const m = d.item!.map!;
  const h = d.item!.health;
  return (
    <ul className="cm-facts">
      <li>{m.nav ? (m.lsv ? "Bots know this map from the first round (nav and map knowledge included)." : "Comes with a bot nav; their map knowledge fills in as you play.") : "No bot nav: bots learn the map the first time it loads."}</li>
      {m.nav && m.places && <li>Places are named, so bots call out where they are.</li>}
      {m.modes.length > 0 && <li>Made for: {m.modes.join(", ")}.</li>}
      {h && (
        <li>
          Health check ({h.minutes} min, {h.bots} bots): {h.verdict === "good" ? "no real problems" : h.verdict === "warn" ? "a few things worth a look" : "real problems found"}
          {h.falls ? `, ${h.falls} falls` : ""}, stuck {h.stuckPct.toFixed(1)}% of the time{h.coveragePct != null ? `, bots used ${Math.round(h.coveragePct)}% of the map` : ""}.
        </li>
      )}
    </ul>
  );
}
