// Community creations: maps, game modes, prefabs, sprays and textures made with Loner-Strike.
// Browse finds them (GameBanana pages tagged for Loner-Strike, plus the Loner-Strike index's
// extras), Try it installs one with the app's own safe installer and starts a game with bots,
// Installed lists what came from Community (update, remove, remix a Studio mode), Publish
// packages your map or Studio mode for GameBanana, Published lists what you published.
//
// Addresses (go("tools", ...)): "community", "community:installed", "community:publish",
// "community:publish:<map>" (the editor or nav fixer handing over a map),
// "community:publish:mode:<id>" (the Game Mode Studio handing over a mode), "community:yours",
// "community:mod:<id>" / "community:spray:<id>" (one creation).

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useMemo, useRef, useState } from "react";
import { getJson } from "../../../api";
import { useToolArg } from "../../../stores/tools";
import { useNav } from "../../../store";
import type { GbJob } from "../../../types";
import type { BrainNeeds, NeedLink, Needs } from "./communityApi";
import { Banner, BusyButton, LoadError, Segmented, useToast } from "../../../ui";
import { errorText, useBusy } from "../common/api";
import { useReportAddress } from "../common/address";
import { useToolCrumbs, type Crumb } from "../common/shell";
import { Empty, Loading } from "../common";
import type { ToolProps } from "../registry";
import {
  BADGE_LABEL, KIND_LABEL, NOTE_MAX, REPORT_REASONS, ago, browse, details, launch, remixMap, report, sizeText, startModes, tryIt,
  type Details, type Entry, type Model, type ReportReason, type Trust,
} from "./communityApi";
import Publish from "./Publish";
import Yours from "./Yours";
import InstalledList, { InstalledActions } from "./Installed";
import "./community.css";
import { BOT_NOTE, BOT_RATING_INFO } from "../../../games";
import { DiscordButton } from "../../DiscordButton";

type Tab = "browse" | "installed" | "publish" | "yours";

/** Tools reopens a tool where it was left, even from an earlier session. Community always starts
 *  on Browse the first time it's opened in a session: a bare tab address (installed, yours,
 *  publish without a map) only counts once you've been here. Links to a creation or a map to
 *  publish are always followed. */
let openedThisSession = false;

function fromArg(arg: string | null): { tab: Tab; map?: string; open?: { model: Model; id: number }; kind?: string } {
  const first = !openedThisSession;
  if (!arg) return { tab: "browse" };
  if (first && (arg === "installed" || arg === "yours" || arg === "publish")) return { tab: "browse" };
  const [a, b] = [arg.split(":")[0], arg.split(":").slice(1).join(":")];
  if (a === "publish") return { tab: "publish", map: b || undefined };
  if (a === "browse") return { tab: "browse", kind: ["map", "mode", "prefab", "spray", "texture"].includes(b) ? b : "all" };
  if (a === "yours") return { tab: "yours" };
  if (a === "installed") return { tab: "installed" };
  if ((a === "mod" || a === "spray") && /^\d+$/.test(b)) return { tab: "browse", open: { model: a === "mod" ? "Mod" : "Spray", id: Number(b) } };
  return { tab: "browse" };
}

export default function Community({ maps }: ToolProps) {
  const arg = useToolArg();
  const [start] = useState(() => fromArg(arg));
  const [tab, setTab] = useState<Tab>(start.tab);
  const [publishMap, setPublishMap] = useState<string | undefined>(start.map);
  const [open, setOpen] = useState<{ model: Model; id: number } | null>(start.open ?? null);
  const [kind, setKind] = useState(start.kind ?? "all");
  const firstArg = useRef(true);
  useEffect(() => {
    openedThisSession = true;
  }, []);
  useEffect(() => {
    // (the first address was read above)
    if (firstArg.current) {
      firstArg.current = false;
      return;
    }
    const s = fromArg(arg);
    setTab(s.tab);
    if (s.map) setPublishMap(s.map);
    if (s.open) setOpen(s.open);
    if (s.kind) {
      setKind(s.kind);
      setOpen(null);
    }
  }, [arg]);
  // each tab and creation has its own address, so Back and the crumbs step between them
  const shown = tab === "browse" && open ? open : null;
  const item = useQuery({ queryKey: ["tools", "community", "item", shown?.model, shown?.id], queryFn: ({ signal }) => details(shown!.model, shown!.id, signal), retry: false, enabled: !!shown });
  const tabCrumb: Record<Tab, Crumb> = { browse: { label: "Browse", address: "community" }, installed: { label: "Installed", address: "community:installed" }, publish: { label: "Publish", address: "community:publish" }, yours: { label: "Published", address: "community:yours" } };
  useReportAddress(
    "community",
    shown ? `community:${shown.model === "Mod" ? "mod" : "spray"}:${shown.id}` : tab === "publish" ? `community:publish${publishMap ? `:${publishMap}` : ""}` : tab === "yours" ? "community:yours" : tab === "installed" ? "community:installed" : null,
  );
  useToolCrumbs(
    "community",
    shown ? [tabCrumb.browse, { label: item.data?.name ?? "Creation" }] : tab === "publish" && publishMap ? [tabCrumb.publish, { label: publishMap.replace(/^(mode|prefab):(mine:)?/, "") }] : [tabCrumb[tab]],
  );

  return (
    <div className="cm stack-tight">
      <div className="cm-tabs">
        <Segmented label="Community" values="browse installed publish yours" labels="Browse|Installed|Publish|Published" current={tab} onPick={(v) => setTab(v as Tab)} />
      </div>
      {tab === "browse" && !open && <DiscordButton note="Show off what you made, find people to play with, and tell us what Community needs." />}
      {tab === "browse" && (open ? <CreationPage model={open.model} id={open.id} onBack={() => setOpen(null)} /> : <BrowseList kind={kind} setKind={setKind} onOpen={(e) => setOpen({ model: e.model, id: e.id })} />)}
      {tab === "installed" && <InstalledList onOpen={(m, id) => { setOpen({ model: m, id }); setTab("browse"); }} onBrowse={() => setTab("browse")} />}
      {tab === "publish" && <Publish maps={maps} map={publishMap} onMap={setPublishMap} onDone={() => setTab("yours")} />}
      {tab === "yours" && <Yours onPublish={() => setTab("publish")} />}
    </div>
  );
}

// ---- browse --------------------------------------------------------------------------------

function BrowseList({ onOpen, kind, setKind }: { onOpen: (e: Entry) => void; kind: string; setKind: (k: string) => void }) {
  const [sort, setSort] = useState("new");
  // bot support (game modes): any, or at least so good
  const [botsAt, setBotsAt] = useState("any");
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
      return browse(kind === "all" ? "" : kind, q, sort, r, signal);
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
        <Segmented label="Kind" values="all map mode prefab spray texture" labels="All Maps Modes Prefabs Sprays Textures" current={kind} onPick={setKind} />
        <label className="cm-sort">
          <span className="muted small-note">Sort</span>
          <select value={sort} onChange={(e) => setSort(e.target.value)} aria-label="Sort">
            <option value="new">Newest</option>
            <option value="liked">Most liked</option>
            <option value="name">Name</option>
          </select>
        </label>
        {(kind === "all" || kind === "mode") && (
          <label className="cm-sort">
            <span className="muted small-note">Bots</span>
            <select value={botsAt} onChange={(e) => setBotsAt(e.target.value)} aria-label="Bot support">
              <option value="any">Any</option>
              <option value="great">{BOT_NOTE.great}</option>
              <option value="ok">{BOT_NOTE.ok} or better</option>
              <option value="weak">{BOT_NOTE.weak}</option>
              <option value="none">{BOT_NOTE.none}</option>
            </select>
          </label>
        )}
        <BusyButton className="btn" busy={list.isFetching} label="Refresh" busyLabel="Loading…" onClick={again} />
      </div>
      {list.data && <SourceNote data={list.data} />}
      {list.isPending ? (
        <Loading text="Finding creations…" />
      ) : list.isError ? (
        <LoadError text={errorText(list.error, "Couldn't load the community list.")} onRetry={again} />
      ) : list.data.items.length === 0 ? (
        <Empty
          title={q || kind !== "all" ? "Nothing matches" : "Nothing here yet"}
          text={
            q || kind !== "all"
              ? "Try other words, or All."
              : <>Nobody has published with Loner-Strike yet. Be the first: Publish packages your map, and the tag <code>{list.data.tag}</code> on its GameBanana page makes it show up here.</>
          }
        />
      ) : (
        <ul className="cm-grid" aria-label="Creations">
          {list.data.items.filter((e) => botsAt === "any" || (e.kind === "mode" && (botsAt === "ok" ? e.bots === "great" || e.bots === "ok" : e.bots === botsAt))).map((e) => (
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
  if (data.index === "newer") return <Banner>Update Loner-Strike to see the newest list: the Loner-Strike list is in a newer format than this version reads{data.gamebanana === "offline" ? "." : ", so badges may be missing or from earlier."}</Banner>;
  if (data.index === "none") return <p className="muted small-note">The Loner-Strike list couldn't be reached: badges (works with bots, checked, modes) are missing for now.</p>;
  if (data.index === "cached") return <p className="muted small-note">Showing the Loner-Strike list from earlier (the download server didn't answer).</p>;
  return null;
}

const needsSomething = (b: BrainNeeds) => b.componentsPlugin || b.components.length > 0 || b.creatures.length > 0 || b.features.length > 0;

/** What a mode needs beyond the game: Loner-Strike's components, creatures and bot-brain features,
 *  each marked "You have it" or "Update Loner-Strike" (the app's setup brings the newer brain; QA4-G F6). */
function BrainNeedsList({ b }: { b: BrainNeeds }) {
  if (!needsSomething(b)) return null;
  const mark = (have: boolean | undefined) =>
    have ? <span className="tag cm-badge-bots">You have it</span> : <span className="tag cm-trust-unreviewed" title="A newer Loner-Strike brings it: update the app, then the game files it offers">{b.here ? "Update Loner-Strike" : "Needs the game set up"}</span>;
  return (
    <>
      <h4>Needs</h4>
      <ul data-testid="cm-brain-needs">
        {(b.componentsPlugin || b.components.length > 0) && (
          <li>
            Loner-Strike's components{b.components.length ? `: ${b.components.join(", ")}` : ""} {mark(true)} <span className="muted small-note">they come with the app</span>
          </li>
        )}
        {b.creatures.length > 0 && (
          <li>
            Creatures: {b.creatures.includes("all") ? "the wave spawner's types" : b.creatures.join(", ")} {mark(b.creaturesHave)}
          </li>
        )}
        {b.features.map((f) => (
          <li key={f.id}>
            {f.name} <span className="muted small-note">(bot brain: {f.brain === "creature" ? "creatures" : "bots"} version {f.ver} or newer)</span> {mark(f.have)}
          </li>
        ))}
      </ul>
    </>
  );
}

/** A mode's "Plays on" and "Needs", each with whether you have it. */
function ModeNeeds({ needs }: { needs: Needs }) {
  const mark = (have: boolean) => <span className={`tag ${have ? "cm-badge-bots" : "cm-trust-unreviewed"}`}>{have ? "You have it" : "Missing"}</span>;
  return (
    <div className="cm-needs">
      {needs.maps.length > 0 && (
        <>
          <h4>Plays on</h4>
          <ul>
            {needs.maps.map((m) => (
              <li key={m.name}>
                {m.name}
                {m.name === needs.defaultMap ? " (default)" : ""} {mark(m.have)} {!m.have && !m.item && <span className="muted small-note">not shared on the community list</span>}
              </li>
            ))}
          </ul>
        </>
      )}
      {needs.brain && <BrainNeedsList b={needs.brain} />}
      {needs.content.length > 0 && (
        <>
          <h4>{needs.brain && needsSomething(needs.brain) ? "Also needs" : "Needs"}</h4>
          <ul>
            {needs.content.map((c) => (
              <li key={c.path}>
                <code>{c.path}</code> {mark(c.have)} {c.link && <span className="muted small-note">from {c.link.name || `page ${c.link.id}`}</span>}
              </li>
            ))}
          </ul>
        </>
      )}
    </div>
  );
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
      {trust?.risks?.map((r) => (
        <span key={r} className="tag cm-risk" title="This mode does more than its own game. You're asked before it installs.">
          {r}
        </span>
      ))}
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
        {e.kind === "mode" && e.bots && <span className={`bots-note ${e.bots}`} title={BOT_RATING_INFO}>{BOT_NOTE[e.bots]}</span>}
        {e.otherGame && <span className="tag cm-risk" title="Made for the other game: it may not work in yours. You're asked before it installs.">For {(e.games ?? []).includes("czero") ? "Condition Zero" : "Counter-Strike 1.6"}</span>}
        <Badges list={e.badges} trust={e.trust} />
      </span>
    </button>
  );
}

// ---- one creation ----------------------------------------------------------------------------

type Stage = { state: "idle" } | { state: "asking" } | { state: "installing"; job: string; got?: number; size?: number; message?: string } | { state: "done"; map: string | null; prefabs?: string[]; mode?: string } | { state: "failed"; message: string };

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
  const qc = useQueryClient();
  const [stage, setStage] = useState<Stage>({ state: "idle" });
  // (the question the app asked after reading the mode itself; the list's own is d.trust.confirm)
  const [ask, setAsk] = useState<string | null>(null);
  // (the risk codes that question named: sent back with the yes)
  const [askCodes, setAskCodes] = useState<string[] | null>(null);
  const [pic, setPic] = useState(0);
  // "Get everything": what the mode needs that isn't here (and has an item to get it from)
  const missingNeeds: NeedLink[] = [
    ...(d.needs?.maps ?? []).filter((m) => !m.have && m.item).map((m) => m.item!),
    ...(d.needs?.content ?? []).filter((c) => !c.have && c.link).map((c) => c.link!),
  ].filter((x, i, all) => all.findIndex((y) => y.model === x.model && y.id === x.id) === i);
  const [getting, setGetting] = useState<string | null>(null);
  const [getNotes, setGetNotes] = useState<string[]>([]);
  const getEverything = async () => {
    const notes: string[] = [];
    for (const it of missingNeeds) {
      setGetting(`Getting ${it.name || `item ${it.id}`}…`);
      const r = await tryIt(it.model, it.id, false);
      if (r.error) {
        notes.push(`${it.name || `Item ${it.id}`}: ${r.error}`);
        continue;
      }
      if (r.job) {
        // each through its own installer: one at a time
        for (;;) {
          await new Promise((res) => setTimeout(res, 1000));
          const list = await getJson<GbJob[]>("/api/gb/jobs").catch(() => [] as GbJob[]);
          const j = Array.isArray(list) ? list.find((x) => x.id === r.job) : undefined;
          if (!j || j.state === "done") break;
          if (j.state === "error") {
            notes.push(`${it.name || `Item ${it.id}`}: ${j.message || "couldn't install it"}`);
            break;
          }
        }
      }
    }
    setGetNotes(notes);
    setGetting(null);
    qc.invalidateQueries({ queryKey: ["tools", "community"] });
    // then the mode itself (asked first when it does more than its own game)
    await go(false);
  };
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
      if (j.state === "done") {
        setStage({ state: "done", map: map ?? j.maps?.[0] ?? null });
        // (the library notes it a moment after the job ends)
        setTimeout(() => qc.invalidateQueries({ queryKey: ["tools", "community"] }), 1500);
      }
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
    // exactly the risks the question showed (none when nothing was asked)
    const codes = confirmed ? (askCodes ?? d.trust.riskCodes ?? []) : [];
    const r = await tryIt(d.model, d.id, confirmed, undefined, codes);
    // the mode's own graph does more than what was shown: asked again before anything installs
    if (!r.error && r.needsConfirm) {
      setAsk(r.needsConfirm);
      setAskCodes(r.riskCodes ?? []);
      return setStage({ state: "asking" });
    }
    if (!r.error && r.kind === "prefab") {
      // prefabs go straight into the map editor's library (no install job)
      setStage({ state: "done", map: null, prefabs: r.prefabs ?? [] });
      qc.invalidateQueries({ queryKey: ["tools", "community", "library"] });
      return;
    }
    if (!r.error && r.kind === "mode") {
      // a Game Mode Studio mode, built on this computer (no install job)
      setStage({ state: "done", map: null, mode: r.mode });
      qc.invalidateQueries({ queryKey: ["tools", "community"] });
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
    // (a mode on its own starts on the first map it's made for, else the map you played last)
    if (!m && !mode) return;
    // (a mode starts on its default map, else the first it's made for)
    const r = await launch(m ?? d.needs?.defaultMap ?? d.item?.mode?.maps?.[0] ?? null, mode);
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
        {d.needs && <ModeNeeds needs={d.needs} />}

        <div className="cm-try">
          {stage.state === "asking" && (ask || d.trust.confirm) ? (
            <div className="cm-confirm" role="alertdialog" aria-label="Before installing">
              <p>{ask || d.trust.confirm}</p>
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
          ) : (stage.state === "done" && stage.mode) || (d.kind === "mode" && d.installed?.mode && stage.state !== "done") ? (
            <div className="stack-tight">
              <p className="cm-ok">{stage.state === "done" ? "Built on this computer and added to your modes." : "In your modes."}</p>
              <div className="row cm-play">
                <button className="btn primary" onClick={() => play(stage.state === "done" ? stage.mode : d.installed!.mode)}>Play with bots</button>
              </div>
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
              <button className="btn primary" disabled={!d.canTry || getting !== null} onClick={() => go(false)}>{missingNeeds.length ? "Just the mode" : "Try it"}</button>
              {d.kind === "map" && d.item?.source && <RemixButton model={d.model} id={d.id} name={d.name} />}
              {missingNeeds.length > 0 && (
                <button className="btn" disabled={!d.canTry || getting !== null} onClick={() => void getEverything()} title="The mode plus the maps and content it needs, each through its own installer and checks">
                  {getting !== null ? (
                    <>
                      <span className="spinner" aria-hidden="true" />
                      {getting}
                    </>
                  ) : (
                    "Get everything"
                  )}
                </button>
              )}
              {getNotes.map((n) => (
                <p key={n} className="muted small-note">{n}</p>
              ))}
              {d.why && <p className="muted small-note">{d.why}</p>}
              {d.otherGame && <p className="muted small-note">{d.otherGame.replace(/ Install it anyway\?$/, "")}</p>}
            </>
          )}
          {stage.state === "failed" && <p className="cm-bad" role="alert">{stage.message}</p>}
          {(d.installed || stage.state === "done") && stage.state !== "installing" && <InstalledActions d={d} onChanged={() => setStage({ state: "idle" })} />}
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
          Bot simulation ({h.minutes} min, {h.bots} bots): {h.verdict === "good" ? "no real problems" : h.verdict === "warn" ? "a few things worth a look" : "real problems found"}
          {h.falls ? `, ${h.falls} falls` : ""}, stuck {h.stuckPct.toFixed(1)}% of the time{h.coveragePct != null ? `, bots used ${Math.round(h.coveragePct)}% of the map` : ""}.
        </li>
      )}
    </ul>
  );
}

/** P5: "Remix this map": its editable source as a new project of yours (credited), opened in the editor. */
function RemixButton({ model, id, name }: { model: Model; id: number; name: string }) {
  const toast = useToast();
  const { working, run } = useBusy();
  const go = () =>
    run("Remixing…", async () => {
      const r = await remixMap(model, id);
      if (r.error) return toast(r.error, true);
      toast(`${r.name} is yours to change: a remix of ${name}. Publishing it credits the original.`);
      useNav.getState().go("tools", "editor");
    });
  return (
    <BusyButton
      className="btn"
      busy={working !== null}
      busyLabel={working ?? "Remixing…"}
      label="Remix this map"
      onClick={go}
    />
  );
}
