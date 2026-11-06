import { useEffect, useState } from "react";
import { getJson, postJson } from "../api";
import { Icon } from "../icons";
import type { AppInfo } from "../types";
import { SectionHead, SteamProgress, useToast } from "../ui";
import type { Job, Scan } from "./Setup";

const BOT_NOTE = { great: "Bots play this well", ok: "Bots are so-so", weak: "Best with a friend", none: "Solo, no bots" };
const BOT_RATING: Record<string, "great" | "ok" | "weak" | "none"> = {
  hns: "ok", deathrun: "ok", surf: "ok", soccerjam: "weak", basebuilder: "weak", kreedz: "none",
};

/** Install or remove game modes and add-ons one at a time, on the game or server being managed. */
export function Mods({ app, scan, running, onChanged, openSetup }: { app: AppInfo | null; scan: Scan | null; running: boolean; onChanged: () => void; openSetup: () => void }) {
  const toast = useToast();
  const [job, setJob] = useState<Job | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
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

  if (!game || !scan) return <p className="muted">{scan ? "No game found." : "Loading…"}</p>;
  const installed = new Set(game.installed ?? []);
  const setUp = installed.has("core");
  const items = scan.catalog.filter((c) => !c.games || c.games.includes(game.id));
  const where = game.dedicated ? "this server" : game.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero";

  const change = async (id: string | null, remove: boolean) => {
    if (remove && !confirm(id ? "Remove it? Its files go, and anything it replaced is put back." : `Remove Loner-Strike from ${where}? Everything it changed is put back.`)) return;
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
        <div className="mod-list">
          {items.map((c) => {
            const on = installed.has(c.id);
            const rating = c.kind === "mode" ? BOT_RATING[c.id.replace(/^mode-/, "")] ?? "great" : null;
            return (
              <div key={c.id} className={`mod${on ? " on" : ""}`}>
                <Icon name={c.id.replace(/^(mode|addon)-/, "")} size={22} />
                <span className="mod-name">{c.name}<small>{c.kind === "addon" ? "Add-on" : "Game mode"}{on ? " · installed" : ""}</small></span>
                {c.description && <span className="mod-desc">{c.description}</span>}
                <div className="row">
                  {on ? (
                    <button className="btn danger" disabled={!!busy} onClick={() => change(c.id, true)}>{busy === c.id ? "Removing…" : "Remove"}</button>
                  ) : (
                    <button className="btn primary" disabled={!!busy} onClick={() => change(c.id, false)}>{busy === c.id ? "Installing…" : "Install"}</button>
                  )}
                  {rating && <span className={`bots-note ${rating}`}>{BOT_NOTE[rating]}</span>}
                </div>
              </div>
            );
          })}
        </div>
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
