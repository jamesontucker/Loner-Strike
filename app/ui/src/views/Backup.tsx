import { useEffect, useMemo, useState } from "react";
import { getJsonAt, ON_COMPUTER_ONLY, postJsonAt, useActions } from "../api";
import { queryClient } from "../queries";
import type { ServerInfo } from "../types";
import { BusyButton, Info, Sheet, useConfirm, useToast } from "../ui";
import { JobProgress } from "./JobProgress";
import type { Job } from "./Setup";

/** Backing up servers and this computer's own game to a .lsbackup file, and restoring
 *  one: onto a new server here or on a Linux machine, over a server, or onto the game. */

type Part = { id: string; name: string };
type Source = { id: string; name: string; game: "cstrike" | "czero"; kind: "game" | "server"; host?: string | null };
type Machine = { id: string; host: string; user: string };
type Choices = { sources?: Source[]; machines?: Machine[]; parts?: Part[]; kept_dir?: string; app_version?: string; error?: string };
/** "servers": opened from the Servers page, so only servers (Settings has the whole app). */
export type Scope = "servers" | "all";
type Reply = { ok?: boolean; error?: string; cancelled?: boolean; job?: string; id?: string | null; need_password?: boolean; wrong_password?: boolean };

const GAME = { cstrike: "CS 1.6", czero: "Condition Zero" } as const;
const PART_NOTE: Record<string, string> = {
  settings: "Server settings, modes, presets, the welcome page and the rest of the Game Mode Menu",
  admins: "Admins, VIPs and regulars",
  bans: "Banned players and IPs, and mutes",
  rotation: "The map cycle, each mode's rotation and favourite maps",
  plugins: "Plugins you added yourself on the Mods page",
  content: "Maps, models and sounds you added, round music and skins, and the GameBanana downloads. For your game, maps you copied in yourself are only told apart from Valve's when a server here has Valve's files to compare with",
  mods: "The modes and add-ons installed",
};
const PREFS_NOTE = "Checking for updates when the app starts, keeping servers running when the app closes, and join and helper notifications. Not phone access, which game is picked, or folders: those belong to this computer.";
const PASSWORDS_NOTE = "The console (RCON) password, the join password, the HLTV spectator password, Discord webhooks, the website status tunnel token, and passwords in settings files (rcon_password and sv_password in server.cfg, admins' passwords in users.ini). They're locked with the password you pick here, and asked for when restoring.";
/** The player's own compiled plugins: code, so only restored when they say so. */
const PLUGIN_CODE_NOTE = "Compiled plugins (.amxx) run inside the server, and anyone could have made or changed this file. Only tick this if you made the backup yourself or trust whoever did. Left unticked, their source (.sma) still comes back, and you can build them again on the Mods page.";
/** [...s].length: characters, as the app counts them (not UTF-16 units) */
const chars = (s: string) => [...s].length;

/** "184 MB", "1.2 GB", "640 KB" */
export function bytes(n: number) {
  if (n >= 1e9) return `${(n / 1e9).toFixed(1)} GB`;
  if (n >= 1e7) return `${Math.round(n / 1e6)} MB`;
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(n / 1e3))} KB`;
}

const post = (path: string, body: unknown = {}) =>
  // these name their servers themselves: never the one that's open (no X-Target)
  postJsonAt<Reply & Record<string, unknown>>(path, body, null)
    .then((r) => (r.error === "forbidden" ? { ...r, error: ON_COMPUTER_ONLY } : r))
    .catch(() => ({ error: "Couldn't reach the app." }) as Reply & Record<string, unknown>);

function useBackupInfo() {
  const [info, setInfo] = useState<Choices | null>(null);
  useEffect(() => {
    let on = true;
    getJsonAt<Choices>("/api/backup/info", null).then((i) => on && setInfo(i)).catch(() => on && setInfo({ error: "Couldn't reach the app." }));
    return () => { on = false; };
  }, []);
  return info;
}

/** Ticks for the parts of a backup (all on to start with). */
function PartChecks({ parts, chosen, onChange, disabled }: { parts: Part[]; chosen: Set<string>; onChange: (s: Set<string>) => void; disabled?: boolean }) {
  const tick = (id: string, on: boolean) => {
    const n = new Set(chosen);
    if (on) n.add(id);
    else n.delete(id);
    onChange(n);
  };
  return (
    <div className="pick-list" role="group" aria-label="What's included">
      {parts.map((p) => (
        <label key={p.id} className="pick-row">
          <input type="checkbox" checked={chosen.has(p.id)} disabled={disabled} onChange={(e) => tick(p.id, e.target.checked)} />
          <span className="grow">{p.name}<br /><span className="muted small-note">{PART_NOTE[p.id]}</span></span>
        </label>
      ))}
    </div>
  );
}

// ---- backing up -------------------------------------------------------------------------

type Saved = { bytes: number; path: string; skipped?: string[]; app_prefs?: boolean; sources: { name: string; files: number; maps: number; components: number; secrets: boolean; odd_names: number; passwords_left_out?: number }[] };

/** The Back up window. `only`: just these (a server's page, Settings); otherwise every
 *  server and this computer's game, all ticked (Back up everything in Settings; servers only from the Servers page). */
export function BackupSheet({ only, scope = "all", onClose }: { only?: string[]; scope?: Scope; onClose: () => void }) {
  const toast = useToast();
  const info = useBackupInfo();
  const sources = useMemo(() => (info?.sources ?? []).filter((s) => (!only || only.includes(s.id)) && (scope === "all" || s.kind === "server")), [info, only, scope]);
  const [picked, setPicked] = useState<Set<string> | null>(null);
  const chosen = useMemo(() => picked ?? new Set(sources.map((s) => s.id)), [picked, sources]);
  const [full, setFull] = useState(false);
  const [parts, setParts] = useState<Set<string> | null>(null);
  const partSet = useMemo(() => parts ?? new Set((info?.parts ?? []).map((p) => p.id)), [parts, info]);
  const [lock, setLock] = useState(false);
  const [pw, setPw] = useState("");
  const [pw2, setPw2] = useState("");
  const [size, setSize] = useState<number | null>(null);
  const [sizeError, setSizeError] = useState("");
  const [saved, setSaved] = useState<Saved | null>(null);
  // Export all (not one server or the game): the app's own preferences can go in too
  const [prefs, setPrefs] = useState(true);
  const showPrefs = !only && scope === "all";
  const withPrefs = showPrefs && prefs;
  const { run, busy } = useActions();
  const saving = busy("backup:save");

  // the size, worked out again when what's in it changes
  const ids = [...chosen].sort().join(",");
  const partIds = [...partSet].sort().join(",");
  useEffect(() => {
    setSizeError("");
    if (!ids || !partIds) return setSize(null);
    let on = true;
    setSize(null);
    const t = setTimeout(() => {
      // `on`: an answer for what was picked before is ignored
      post("/api/backup/estimate", { sources: ids.split(","), content: full ? "full" : "light", parts: partIds.split(",") }).then((r) => {
        if (!on) return;
        if (r.error) setSizeError(r.error);
        else setSize(Number(r.bytes) || 0);
      });
    }, 300);
    return () => { on = false; clearTimeout(t); };
  }, [ids, partIds, full]);

  const tick = (id: string, on: boolean) => {
    const n = new Set(chosen);
    if (on) n.add(id);
    else n.delete(id);
    setPicked(n);
  };
  const why = !chosen.size ? "Pick what to back up." : !partSet.size ? "Pick at least one part." : lock && chars(pw) < 6 ? "Pick a password of at least 6 characters." : lock && pw !== pw2 ? "The two passwords aren't the same." : "";
  const save = () => run("backup:save", async () => {
    const r = await post("/api/backup/export", { sources: [...chosen], content: full ? "full" : "light", parts: [...partSet], password: lock ? pw : "", app_prefs: withPrefs });
    if (r.error) return toast(r.error, true);
    if (r.cancelled) return;
    setSaved(r as unknown as Saved);
    toast("Backup saved");
  }, "Saving the backup…");

  const title = only?.length === 1 ? `Back up ${sources[0]?.name ?? ""}` : scope === "servers" ? "Back up servers" : "Back up everything";
  return (
    <Sheet label={title} onClose={saving ? undefined : onClose}>
      <div className="sheet-body">
        <h2>{title}</h2>
        {!info ? (
          <p className="muted"><span className="spinner" aria-hidden="true" />Looking at your servers…</p>
        ) : info.error ? (
          <p className="err-text">{info.error}</p>
        ) : only && !sources.length ? (
          <>
            <p className="muted measure">{only.some((id) => id.startsWith("local:")) ? "The game isn't set up on this computer yet, so there's nothing of it to back up. Set it up first (Setup), then back it up." : "That server isn't set up yet, so there's nothing to back up."}</p>
            <div className="row"><button className="btn" onClick={onClose}>Close</button></div>
          </>
        ) : saved ? (
          <>
            <p>Saved {bytes(saved.bytes)} to <span className="mono small">{saved.path}</span>.</p>
            <ul className="plain-list">
              {saved.sources.map((s) => (
                <li key={s.name}>
                  <b>{s.name}</b>: {s.files} file{s.files === 1 ? "" : "s"}
                  {s.components ? `, ${s.components} mod${s.components === 1 ? "" : "s"}` : ""}
                  {s.maps ? `, ${s.maps} download${s.maps === 1 ? "" : "s"} to get again` : ""}
                  {s.secrets ? ", passwords and webhooks locked in" : ""}
                  {s.passwords_left_out ? `. Passwords in ${s.passwords_left_out} settings file${s.passwords_left_out === 1 ? "" : "s"} left out` : ""}
                  {s.odd_names ? `. ${s.odd_names} file${s.odd_names === 1 ? "" : "s"} with unusual names left out` : ""}
                </li>
              ))}
            </ul>
            {saved.app_prefs && <p className="muted small-note">App preferences included.</p>}
            {saved.skipped?.length ? <ul className="plain-list err-text">{saved.skipped.map((n) => <li key={n}>{n}</li>)}</ul> : null}
            <p className="muted measure">Keep it somewhere safe, like a USB stick or cloud storage. To bring it back: Servers, Restore a backup, or Settings, Backup & restore.</p>
            <div className="row"><button className="btn primary" onClick={onClose}>Done</button></div>
          </>
        ) : (
          <>
            {!only || only.length > 1 ? (
              <div className="pick-list" role="group" aria-label="What to back up">
                {sources.map((s) => (
                  <label key={s.id} className="pick-row">
                    <input type="checkbox" checked={chosen.has(s.id)} disabled={saving} onChange={(e) => tick(s.id, e.target.checked)} />
                    <span className="grow">{s.name}</span>
                    <span className="muted small-note">{s.kind === "game" ? "This computer's game" : s.host ? `${GAME[s.game]} on ${s.host}` : GAME[s.game]}</span>
                  </label>
                ))}
                {!sources.length && <p className="muted">Nothing to back up yet.</p>}
              </div>
            ) : null}
            <div className="field">
              <span>What it holds</span>
              <div className="methods" role="radiogroup" aria-label="What it holds">
                <button type="button" className="method" role="radio" aria-checked={!full} onClick={() => setFull(false)}>
                  <span className="method-name">Light</span>
                  <span className="method-blurb">Settings and lists. Re-downloads mods and maps when restoring.</span>
                </button>
                <button type="button" className="method" role="radio" aria-checked={full} onClick={() => setFull(true)}>
                  <span className="method-name">Everything included</span>
                  <span className="method-blurb">One big file with the maps and mod files. Mods' program files always come fresh from setup when restoring; everything else comes from the file.</span>
                </button>
              </div>
              <small>{sizeError ? `Couldn't work out the size: ${sizeError}` : size === null ? "Working out the size…" : `About ${bytes(size)}.`} Valve's own server files are never included: a new server still downloads those (about 1 GB).</small>
            </div>
            <div className="field">
              <span>What's included</span>
              <PartChecks parts={info.parts ?? []} chosen={partSet} onChange={setParts} disabled={saving} />
            </div>
            {showPrefs && (
              <label className="check-row">
                <input type="checkbox" checked={prefs} disabled={saving} onChange={(e) => setPrefs(e.target.checked)} />
                <span>Include app preferences<Info text={PREFS_NOTE} /></span>
              </label>
            )}
            <label className="check-row">
              <input type="checkbox" checked={lock} disabled={saving} onChange={(e) => setLock(e.target.checked)} />
              <span>Include passwords & webhooks<Info text={PASSWORDS_NOTE} /></span>
            </label>
            {lock ? (
              <div className="row">
                <label className="field">
                  <span>Password for the backup</span>
                  <input className="search" type="password" autoComplete="new-password" value={pw} onChange={(e) => setPw(e.target.value)} />
                </label>
                <label className="field">
                  <span>The same again</span>
                  <input className="search" type="password" autoComplete="new-password" value={pw2} onChange={(e) => setPw2(e.target.value)} aria-invalid={!!pw2 && pw !== pw2} />
                </label>
              </div>
            ) : (
              <p className="muted small-note">Without it the backup has no passwords or webhooks (not even those in server.cfg or users.ini): you set them again after restoring.</p>
            )}
            {lock && <p className="muted small-note">There's no way to get the passwords back out without this one, so keep it somewhere safe.</p>}
            <div className="row">
              <BusyButton className="btn primary" busy={saving} disabled={!!why} label="Save the backup" busyLabel="Saving…" onClick={save} />
              <button className="btn" disabled={saving} onClick={onClose}>Cancel</button>
              {why && <span className="muted small-note">{why}</span>}
            </div>
          </>
        )}
      </div>
    </Sheet>
  );
}

// ---- restoring ---------------------------------------------------------------------------

type InFile = { index: number; source: "game" | "server"; name: string; game: "cstrike" | "czero"; content: "light" | "full"; secrets: boolean; secrets_kept?: boolean; bytes: number; app_version: string; created: string; parts: string[]; components: number; maps: number; files: number; plugin_code?: number };
type AppPrefs = { manual_updates: boolean; keep_servers: boolean; mute_joins: boolean; mute_helpers?: boolean; smart_chat: boolean };
type Opened = { file: string; name?: string; sources: InFile[]; machines: Machine[]; app_prefs?: AppPrefs | null };
/** Where a restore goes: a new server here, a new server on a machine, over a server, or the game. */
type Where = "new-local" | "new-remote" | "server" | "game";
/** Preset by where the window was opened from. */
export type RestoreTo = { where: "server"; id: string } | { where: "game"; game: string };

/** The app preferences in an Export all file: shown, and put back on their own (they
 *  aren't part of any one server or game). */
function PrefsBack({ file, prefs }: { file: string; prefs: AppPrefs }) {
  const toast = useToast();
  const [done, setDone] = useState<string[] | null>(null);
  const { run, busy } = useActions();
  const working = busy("restore:prefs");
  const onOff = (on: boolean) => (on ? "on" : "off");
  const put = () => run("restore:prefs", async () => {
    const r = await post("/api/backup/app-prefs", { file });
    if (r.error) return toast(r.error, true);
    const notes = (r.notes as string[] | undefined) ?? [];
    setDone(notes);
    toast("App preferences put back");
  }, "Putting back the app preferences…");
  return (
    <div className="prefs-box">
      <div className="row between">
        <b>App preferences</b>
        {done ? <span className="muted small-note">Put back</span> : <BusyButton className="btn small" busy={working} label="Put them back" busyLabel="Putting back…" onClick={put} />}
      </div>
      <p className="muted small-note">
        Check for updates when the app starts: {onOff(!prefs.manual_updates)} · Keep servers running when the app closes: {onOff(prefs.keep_servers)} · Join notifications: {onOff(!prefs.mute_joins)} · Helper notifications: {prefs.mute_helpers == null ? "unchanged" : onOff(!prefs.mute_helpers)}
      </p>
      {done?.map((n) => <p key={n} className="small-note">{n}</p>)}
    </div>
  );
}

/** "20260107-142233" -> "7 Jan 2026" */
function made(stamp: string) {
  const m = /^(\d{4})(\d{2})(\d{2})/.exec(stamp);
  if (!m) return "";
  return new Date(Date.UTC(+m[1], +m[2] - 1, +m[3])).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

/** The Restore window: pick a backup, what's restored from it and where. */
export function RestoreSheet({ to, scope = "all", servers, onClose, onChanged }: { to?: RestoreTo; scope?: Scope; servers: ServerInfo[]; onClose: () => void; onChanged: () => void }) {
  const toast = useToast();
  const ask = useConfirm();
  const info = useBackupInfo();
  const [opened, setOpened] = useState<Opened | null>(null);
  const [index, setIndex] = useState(0);
  const [where, setWhere] = useState<Where>(to?.where ?? "new-local");
  const [machine, setMachine] = useState("");
  const [server, setServer] = useState(to?.where === "server" ? to.id : "");
  const [name, setName] = useState("");
  const [parts, setParts] = useState<Set<string> | null>(null);
  const [pw, setPw] = useState("");
  const [skipSecrets, setSkipSecrets] = useState(false);
  const [pluginCode, setPluginCode] = useState(false);
  const [updateMods, setUpdateMods] = useState(true);
  const [problem, setProblem] = useState("");
  const [running, setRunning] = useState(false);
  const [ended, setEnded] = useState<{ ok: boolean; notes: string[]; error?: string } | null>(null);
  const { run, busy } = useActions();
  const opening = busy("restore:open");
  const starting = busy("restore:start");

  const src = opened?.sources[index];
  const partList = (info?.parts ?? []).filter((p) => src?.parts.includes(p.id));
  const partSet = parts ?? new Set(partList.map((p) => p.id));
  const games = (info?.sources ?? []).filter((s) => s.kind === "game").map((s) => s.game);
  const over = servers.filter((s) => s.ready && !s.shared && s.game === src?.game);
  const machines = opened?.machines ?? [];

  const open = () => run("restore:open", async () => {
    const r = await post("/api/backup/open");
    if (r.error) return toast(r.error, true);
    if (r.cancelled) return;
    const o = r as unknown as Opened;
    setOpened(o);
    setIndex(0);
    setParts(null);
    setPluginCode(false);
    setPw("");
    setProblem("");
    setEnded(null);
    const first = o.sources[0];
    setName(first?.source === "server" ? first.name : "");
    setMachine(o.machines[0]?.id ?? "");
    fitWhere(first);
  }, "Opening the backup…");
  const pickSource = (i: number) => {
    setIndex(i);
    setParts(null);
    setPluginCode(false);
    setPw("");
    setProblem("");
    setEnded(null);
    const s = opened?.sources[i];
    setName(s?.source === "server" ? s.name : "");
    fitWhere(s);
  };
  // opened without a preset target (Settings, the Servers page): a game's backup goes back
  // into the game when it's here, a server's becomes a new server
  const fitWhere = (s?: InFile) => {
    if (to || !s) return;
    setWhere(s.source === "game" && scope === "all" && games.includes(s.game) ? "game" : "new-local");
  };

  const targetName = where === "server" ? servers.find((s) => s.id === server)?.name : where === "game" ? `your game (${src ? GAME[src.game] : ""})` : null;
  const why = !src
    ? "Open a backup first."
    : !partSet.size
      ? "Pick at least one part."
      : where === "new-remote" && !machine
        ? "Pick a machine."
        : where === "server" && !over.some((s) => s.id === server)
          ? "Pick a server."
          : where === "server" && over.find((s) => s.id === server)?.state === "running"
            ? "Stop it first."
            : where === "game" && !games.includes(src.game)
              ? `${GAME[src.game]} isn't installed on this computer.`
              : src.secrets && !src.secrets_kept && !skipSecrets && !pw
                ? "Type the backup's password, or restore without the passwords."
                : "";
  const restore = async () => {
    if (!src) return;
    if ((where === "server" || where === "game") && !(await ask(`Restore over ${targetName}? Its settings${partSet.has("mods") ? ", mods" : ""} and files are replaced with the ones in the backup. A copy of its settings and your own files is saved first, so you can undo it (maps and skins from GameBanana come back as downloads).`, { ok: "Restore over it", danger: true }))) return;
    await run("restore:start", async () => {
      setProblem("");
      const r = await post("/api/backup/restore", { file: opened!.file, index, target: where, machine, server, name, parts: [...partSet], password: pw, skip_secrets: skipSecrets, plugin_code: pluginCode && partSet.has("plugins"), update_mods: updateMods });
      if (r.wrong_password || r.need_password) return setProblem(r.error ?? "");
      if (r.error) return setProblem(r.error);
      setRunning(true);
      onChanged();
    }, "Starting the restore…");
  };
  const finished = (ok: boolean) => {
    const job = queryClient.getQueryData<Job>(["setupJob"]);
    setRunning(false);
    setEnded({ ok, notes: job?.notes ?? [], error: ok ? undefined : job?.error });
    toast(ok ? "Restored" : "The restore stopped. It says why below.", !ok);
    onChanged();
  };

  return (
    <Sheet label="Restore a backup" onClose={starting || running ? undefined : onClose}>
      <div className="sheet-body">
        <h2>Restore a backup</h2>
        {running ? (
          <JobProgress title="Restoring…" long={where === "new-remote"} onEnd={finished} />
        ) : ended ? (
          <>
            <p>{ended.ok ? "Done." : "The restore stopped part way."}</p>
            {!ended.ok && ended.error && <p className="err-text" role="alert">{ended.error}</p>}
            {ended.notes.length > 0 && <ul className="plain-list">{ended.notes.map((n) => <li key={n}>{n}</li>)}</ul>}
            {!ended.ok && where.startsWith("new") && <p className="muted small-note">The new server shows as not finished in the list. Delete it in the list, then restore again.</p>}
            <div className="row">
              {opened && opened.sources.length > 1 && <button className="btn" onClick={() => setEnded(null)}>Restore another one from this file</button>}
              <button className="btn primary" onClick={onClose}>Done</button>
            </div>
          </>
        ) : (
          <>
            <div className="row">
              <BusyButton className={opened ? "btn" : "btn primary"} busy={opening} label={opened ? "Open another backup" : "Choose a backup file"} busyLabel="Opening…" onClick={open} />
              {opened?.name && <span className="muted small-note mono">{opened.name}</span>}
            </div>
            {!opened && <p className="muted measure">A .lsbackup file from any Back up. Setup files exported from Settings by earlier versions open too.</p>}
            {!opened && info?.kept_dir && <p className="muted small-note">Before restoring over a server or your game, Loner-Strike saves a copy of it in <span className="mono">{info.kept_dir}</span>: open one from there to undo a restore.</p>}
            {scope === "all" && opened?.app_prefs && <PrefsBack file={opened.file} prefs={opened.app_prefs} />}
            {opened && opened.sources.length > 1 && (
              <div className="pick-list" role="radiogroup" aria-label="What to restore">
                {opened.sources.map((s, i) => (
                  <label key={i} className="pick-row">
                    <input type="radio" name="restore-source" checked={i === index} onChange={() => pickSource(i)} />
                    <span className="grow">{s.name}</span>
                    <span className="muted small-note">{s.source === "game" ? "A game" : "A server"} · {GAME[s.game]}</span>
                  </label>
                ))}
              </div>
            )}
            {src && (
              <>
                <p className="muted">
                  {src.source === "game" ? "A backup of a game" : "A backup of a server"} · {GAME[src.game]} · {src.content === "full" ? "Everything included" : "Light"} · {bytes(src.bytes)}
                  {src.components ? ` · ${src.components} mod${src.components === 1 ? "" : "s"}` : ""}
                  {src.maps ? ` · ${src.maps} download${src.maps === 1 ? "" : "s"} to get again` : ""}
                  {src.created ? ` · made ${made(src.created)}` : ""}
                  {src.app_version ? ` with Loner-Strike ${src.app_version}` : ""}
                </p>
                <div className="field">
                  <span>Restore it to</span>
                  <div className="methods" role="radiogroup" aria-label="Restore it to">
                    <button type="button" className="method" role="radio" aria-checked={where === "new-local"} onClick={() => setWhere("new-local")}>
                      <span className="method-name">A new server on this computer</span>
                      <span className="method-blurb">Downloads Valve's server, then puts everything back</span>
                    </button>
                    <button type="button" className="method" role="radio" aria-checked={where === "new-remote"} disabled={!machines.length} onClick={() => setWhere("new-remote")}>
                      <span className="method-name">A new server on a Linux machine</span>
                      <span className="method-blurb">{machines.length ? "One of the machines your servers are on" : "None yet: make a Linux server first"}</span>
                    </button>
                    <button type="button" className="method" role="radio" aria-checked={where === "server"} disabled={!over.length} onClick={() => setWhere("server")}>
                      <span className="method-name">Over a server you have</span>
                      <span className="method-blurb">{over.length ? "Replaces its settings, mods and files" : `No ${GAME[src.game]} servers yet`}</span>
                    </button>
                    {scope === "all" && <button type="button" className="method" role="radio" aria-checked={where === "game"} disabled={!games.includes(src.game)} onClick={() => setWhere("game")}>
                      <span className="method-name">This computer's game ({GAME[src.game]})</span>
                      <span className="method-blurb">The game you play with Play</span>
                    </button>}
                  </div>
                </div>
                {where.startsWith("new") && (
                  <label className="field">
                    <span>Name</span>
                    <input className="search" maxLength={48} placeholder="Like Friday night frags" value={name} onChange={(e) => setName(e.target.value)} />
                  </label>
                )}
                {where === "new-remote" && (
                  <label className="field">
                    <span>Machine<Info text="A machine one of your Linux servers is on. It gets its own port there (27015 to 27019)." /></span>
                    <select value={machine} onChange={(e) => setMachine(e.target.value)}>
                      {machines.map((m) => <option key={m.id} value={m.id}>{m.host} ({m.user})</option>)}
                    </select>
                  </label>
                )}
                {where === "server" && (
                  <label className="field">
                    <span>Server</span>
                    <select value={server} onChange={(e) => setServer(e.target.value)}>
                      <option value="">Pick one</option>
                      {over.map((s) => <option key={s.id} value={s.id}>{s.name}{s.host ? ` (on ${s.host})` : ""}</option>)}
                    </select>
                  </label>
                )}
                {src.source === "game" && where !== "game" && <p className="muted small-note">It's a backup of a game, so the server keeps its own name, slots and start options.</p>}
                {src.source === "server" && where === "game" && <p className="muted small-note">The server's own settings (name, slots, HLTV, website status, webhooks) don't apply to your game, so they're left out.</p>}
                <div className="field">
                  <span>What's restored</span>
                  <PartChecks parts={partList} chosen={partSet} onChange={setParts} disabled={starting} />
                  {src.plugin_code && partSet.has("plugins") ? (
                    <label className="check-row">
                      <input type="checkbox" checked={pluginCode} disabled={starting} onChange={(e) => setPluginCode(e.target.checked)} />
                      <span>Your own plugins (code from this backup): {src.plugin_code} compiled plugin{src.plugin_code === 1 ? "" : "s"}<Info text={PLUGIN_CODE_NOTE} /></span>
                    </label>
                  ) : null}
                  {src.content === "full" && partSet.has("mods") && (
                    <label className="check-row">
                      <input type="checkbox" checked={updateMods} disabled={starting} onChange={(e) => setUpdateMods(e.target.checked)} />
                      <span>
                        Update mods to this version of Loner-Strike
                        <Info text={`${src.app_version && info?.app_version && src.app_version !== info.app_version ? `This backup was made with ${src.app_version}. ` : ""}Mods from an older version get this version's fixes and patches after they're put back; their settings stay. Without it they're left as they were, and update by themselves later (or with Update now on the servers list).`} />
                      </span>
                    </label>
                  )}
                  {src.content === "full" && partSet.has("mods") && <small>Mods' program files aren't taken from a backup: setup installs them from its own checked downloads.</small>}
                  <small>Restored settings files (.cfg) can run game commands on the server, so only restore backups you made yourself or trust.</small>
                </div>
                {src.secrets && src.secrets_kept && <p className="muted small-note">This is a copy saved on this computer before a restore: its passwords come back too, without asking.</p>}
                {src.secrets && !src.secrets_kept && (
                  <div className="field">
                    <span>Passwords & webhooks<Info text={PASSWORDS_NOTE} /></span>
                    <input className="search" type="password" autoComplete="off" placeholder="The backup's password" disabled={skipSecrets} value={pw} onChange={(e) => { setPw(e.target.value); setProblem(""); }} />
                    <label className="check-row">
                      <input type="checkbox" checked={skipSecrets} onChange={(e) => { setSkipSecrets(e.target.checked); setProblem(""); }} />
                      <span>Restore without them</span>
                    </label>
                  </div>
                )}
                {problem && <p className="err-text" role="alert">{problem}</p>}
                <div className="row">
                  <BusyButton className="btn primary" busy={starting} disabled={!!why} label="Restore" busyLabel="Starting…" onClick={restore} />
                  <button className="btn" disabled={starting} onClick={onClose}>Cancel</button>
                  {why && <span className="muted small-note">{why}</span>}
                </div>
              </>
            )}
          </>
        )}
      </div>
    </Sheet>
  );
}
