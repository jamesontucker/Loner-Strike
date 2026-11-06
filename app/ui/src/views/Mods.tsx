import { useEffect, useState } from "react";
import { getJson, postJson } from "../api";
import { Icon } from "../icons";
import type { AppInfo } from "../types";
import { SectionHead, Skeleton, SteamProgress, useConfirm, useToast } from "../ui";
import type { Job, Scan } from "./Setup";

const BOT_NOTE = { great: "Bots play this well", ok: "Bots are so-so", weak: "Best with a friend", none: "Solo, no bots" };
const BOT_RATING: Record<string, "great" | "ok" | "weak" | "none"> = {
  hns: "ok", deathrun: "ok", surf: "ok", soccerjam: "weak", basebuilder: "weak", kreedz: "none",
};

// Part of Loner-Strike's own plugin: always there once the basics are installed
const BUILT_IN: { id: string; name: string; kind: "mode" | "addon"; description: string }[] = [
  { id: "classic", name: "Classic", kind: "mode", description: "Bomb defusal and hostage rescue, the way the game plays" },
  { id: "csdm", name: "Deathmatch", kind: "mode", description: "Teams, instant respawn, pick your guns" },
  { id: "csdm_ffa", name: "Free for all", kind: "mode", description: "Everyone for themselves, instant respawn" },
  { id: "knife", name: "Knife Arena", kind: "mode", description: "Knives only" },
  { id: "pistols", name: "Pistols Only", kind: "mode", description: "No primaries" },
  { id: "deagle", name: "Deagle Only", kind: "mode", description: "Free Deagle every spawn" },
  { id: "awp", name: "AWP Only", kind: "mode", description: "Free AWP every spawn" },
  { id: "scoutz", name: "Scoutz & Knivez", kind: "mode", description: "Scouts, low gravity, air control" },
  { id: "surf", name: "Surf", kind: "mode", description: "Air control, free guns, long rounds" },
  { id: "quakesounds", name: "Quake sounds", kind: "addon", description: "Killing spree and headshot announcers" },
];
const SORTS: [string, string][] = [["name", "A to Z"], ["installed", "Installed first"], ["kind", "Modes first"], ["bots", "Best with bots"]];
const FILTERS: [string, string][] = [["all", "All"], ["mode", "Game modes"], ["addon", "Add-ons"], ["installed", "Installed"]];
const RANK = { great: 0, ok: 1, weak: 2, none: 3 } as const;
const stored = (k: string, d: string) => {
  try {
    return localStorage.getItem(k) || d;
  } catch {
    return d;
  }
};
const store = (k: string, v: string) => {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* private window */
  }
};

/** Install or remove game modes and add-ons one at a time, on the game or server being managed. */
export function Mods({ app, scan, running, onChanged, openSetup }: { app: AppInfo | null; scan: Scan | null; running: boolean; onChanged: () => void; openSetup: () => void }) {
  const ask = useConfirm();
  const toast = useToast();
  const [job, setJob] = useState<Job | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [sort, setSortRaw] = useState(() => stored("ls.modsSort", "name"));
  const [filter, setFilterRaw] = useState(() => stored("ls.modsFilter", "all"));
  const setSort = (v: string) => (setSortRaw(v), store("ls.modsSort", v));
  const setFilter = (v: string) => (setFilterRaw(v), store("ls.modsFilter", v));
  const game = app?.game;

  // follow the setup job while one runs
  useEffect(() => {
    if (!busy) return;
    const t = setInterval(async () => {
      const j = await getJson<Job>("/api/setup/job").catch(() => null);
      if (!j) return;
      setJob(j);
      if (j.state === "done" || j.state === "error") {
        clearInterval(t);
        toast(j.state === "done" ? (running ? "Done. It loads on the next map." : "Done") : j.error || "That stopped.", j.state === "error");
        setBusy(null);
        onChanged();
      }
    }, 800);
    return () => clearInterval(t);
  }, [busy, running, toast, onChanged]);

  if (!game || !scan) return (scan ? <p className="muted">No game found.</p> : <Skeleton />);
  const installed = new Set(game.installed ?? []);
  const setUp = installed.has("core");
  type Item = { id: string; name: string; kind: "mode" | "addon"; description?: string; builtin: boolean; on: boolean; rating: "great" | "ok" | "weak" | "none" | null };
  const ratingFor = (kind: string, id: string) => (kind === "mode" ? BOT_RATING[id.replace(/^mode-/, "")] ?? "great" : null);
  const all: Item[] = [
    ...BUILT_IN.map((b) => ({ ...b, builtin: true, on: setUp, rating: ratingFor(b.kind, b.id) })),
    ...scan.catalog
      .filter((c) => !c.games || c.games.includes(game.id))
      .map((c) => ({ id: c.id, name: c.name, kind: c.kind, description: c.description, builtin: false, on: installed.has(c.id), rating: ratingFor(c.kind, c.id) })),
  ];
  const items = all
    .filter((i) => filter === "all" || (filter === "installed" ? i.on : i.kind === filter))
    .sort((a, b) => {
      const byName = a.name.localeCompare(b.name);
      if (sort === "installed") return Number(b.on) - Number(a.on) || byName;
      if (sort === "kind") return (a.kind === b.kind ? 0 : a.kind === "mode" ? -1 : 1) || byName;
      if (sort === "bots") return (a.rating ? RANK[a.rating] : 9) - (b.rating ? RANK[b.rating] : 9) || byName;
      return byName;
    });
  const where = game.dedicated ? "this server" : game.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero";

  const change = async (id: string | null, remove: boolean) => {
    if (remove && !(await ask(id ? "Remove it? Its files go, and anything it replaced is put back." : `Remove Loner-Strike from ${where}? Everything it changed is put back.`, { ok: "Remove", danger: true }))) return;
    const body = { hl: game.hl, games: [game.id], ...(id ? { components: [id] } : remove ? {} : { components: [] }) };
    const r = await postJson<{ ok?: boolean; error?: string }>(remove ? "/api/setup/uninstall" : "/api/setup/install", body).catch(() => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    setJob(null);
    setBusy(id ?? "core");
  };

  const now = job?.steps.find((s) => s.state === "running");
  return (
    <div className="stack">
      <SectionHead title="Mods" note={`Game modes and add-ons on ${where}. Everything is backed up, so removing something puts the original files back.`}>
        {!game.dedicated && <button className="btn" onClick={openSetup}>Run full setup</button>}
      </SectionHead>

      {busy && (
        <SteamProgress
          value={job?.progress ?? 0}
          title={job?.kind === "uninstall" ? "Removing…" : "Installing…"}
          text={now ? `${now.label}${now.detail ? ` · ${now.detail}` : ""}` : "Getting ready…"}
        />
      )}
      {running && !game.dedicated && !busy && <div className="banner"><div>The game is running. Changes load on the next map; restart the game if something doesn't show up.</div></div>}

      {!setUp ? (
        <div className="panel">
          <h3>Loner-Strike isn't on {where} yet</h3>
          <p className="muted measure">The basics come first: Metamod-P, AMX Mod X, ReGameDLL and the Game Mode Menu, with the built-in modes. Then add whatever you like below.</p>
          <div className="row">
            <button className="btn primary" disabled={!!busy} onClick={() => change(null, false)}>Install the basics</button>
          </div>
        </div>
      ) : (
        <>
        <div className="toolbar mods-toolbar">
          <div className="chips" role="group" aria-label="Show">
            {FILTERS.map(([id, label]) => <button key={id} className="chip" aria-pressed={filter === id} onClick={() => setFilter(id)}>{label}</button>)}
          </div>
          <label className="sort">
            <span>Sort</span>
            <select value={sort} onChange={(e) => setSort(e.target.value)}>
              {SORTS.map(([id, label]) => <option key={id} value={id}>{label}</option>)}
            </select>
          </label>
        </div>
        <div className="mod-list">
          {items.map((c) => {
            const { on, rating } = c;
            return (
              <div key={c.id} className={`mod${on ? " on" : ""}`}>
                <Icon name={c.id.replace(/^(mode|addon)-/, "")} size={22} />
                <span className="mod-name">{c.name}<small>{c.kind === "addon" ? "Add-on" : "Game mode"}{c.builtin ? " · built in" : on ? " · installed" : ""}</small></span>
                {c.description && <span className="mod-desc">{c.description}</span>}
                <div className="row">
                  {c.builtin ? (
                    <span className="muted small-note">Comes with Loner-Strike</span>
                  ) : on ? (
                    <button className="btn danger" disabled={!!busy} onClick={() => change(c.id, true)}>{busy === c.id ? "Removing…" : "Remove"}</button>
                  ) : (
                    <button className="btn primary" disabled={!!busy} onClick={() => change(c.id, false)}>{busy === c.id ? "Installing…" : "Install"}</button>
                  )}
                  {rating && <span className={`bots-note ${rating}`}>{BOT_NOTE[rating]}</span>}
                </div>
              </div>
            );
          })}
          {!items.length && <p className="muted">Nothing here.</p>}
        </div>
        </>
      )}

      {setUp && (
        <div className="panel row between">
          <p className="muted">Take Loner-Strike off {where} completely: every mod, the menu and the bot setup.</p>
          <button className="btn danger" disabled={!!busy} onClick={() => change(null, true)}>Remove Loner-Strike</button>
        </div>
      )}
    </div>
  );
}
