import { useEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { copyText, getJson, getJsonAt, ON_COMPUTER_ONLY, postJson, useActions } from "../api";
import type { AppInfo, PhoneInfo } from "../types";
import { ON_THIS_COMPUTER } from "../api";
import { gameTitle } from "../games";
import { queryClient, useServers, waitForSetupJob } from "../queries";
import { useNav } from "../store";
import { applyScale, savedScale, BusyButton, Info, LoadError, SCALES, SectionHead, Sheet, Switch, useConfirm, useToast } from "../ui";
import type { Scan } from "./Setup";

type Prefs = { auto_update: boolean; keep_servers: boolean; game_folders: string[]; notify_joins: boolean };
type Storage = { downloads: number; pictures: number; folder: string };
const mb = (n: number) => (n < 1048576 ? `${Math.max(0, Math.round(n / 1024))} KB` : `${(n / 1048576).toFixed(n < 104857600 ? 1 : 0)} MB`);

// The app's own settings (always this computer's, whichever server is open), cached so
// Settings opens with the last values instead of loading them again each time.
const PREFS = ["appPrefs"];
const PHONE = ["appPhone"];
const STORAGE = ["appStorage"];
const usePrefs = () => useQuery({ queryKey: PREFS, queryFn: ({ signal }) => getJsonAt<Prefs>("/api/app/prefs", null, signal), staleTime: 300000 });
// a phone is connected by definition, and can't change phone access
const usePhone = () => useQuery({ queryKey: PHONE, queryFn: ({ signal }) => getJsonAt<PhoneInfo>("/api/app/phone", null, signal), staleTime: 300000, enabled: ON_THIS_COMPUTER });
type ServersDir = { dir?: string | null; default?: string | null; mac?: boolean };
const SERVERS_DIR = ["serversDir"];
const useServersDir = () => useQuery({ queryKey: SERVERS_DIR, queryFn: ({ signal }) => getJsonAt<ServersDir>("/api/app/servers-folder", null, signal), enabled: ON_THIS_COMPUTER });
const useStorage = () => useQuery({ queryKey: STORAGE, queryFn: ({ signal }) => getJsonAt<Storage>("/api/app/storage", null, signal), staleTime: 60000 });
const setPrefsData = (f: (p: Prefs) => Prefs) => queryClient.setQueryData<Prefs>(PREFS, (p) => (p ? f(p) : p));

/** Whether updates were looked for since the app started (here, from the menu or at start). */
let checkedOnce = false;
export const noteUpdateChecked = () => {
  checkedOnce = true;
};

export function AppSettings({ app, reloadApp, scan, openSetup, switchGame }: { app: AppInfo | null; reloadApp: () => void; scan: Scan | null; openSetup: () => void; switchGame: (hl: string | null, game: string) => Promise<void> }) {
  const [nuke, setNuke] = useState(false);
  const ask = useConfirm();
  const toast = useToast();
  const { run, busy } = useActions();
  // the update installed and the app is restarting: the button stays busy until it does
  const [restarting, setRestarting] = useState(false);
  const [checked, setChecked] = useState<string>("");
  // diagnostics on a phone: shown to copy by hand (a copy after waiting for them isn't allowed there)
  const [diag, setDiag] = useState<string | null>(null);

  const prefsQ = usePrefs();
  const phoneQ = usePhone();
  const storageQ = useStorage();
  const prefs = prefsQ.data ?? null;
  const phone = phoneQ.data ?? null;
  const storage = storageQ.data ?? null;
  const setPhone = (p: PhoneInfo) => queryClient.setQueryData(PHONE, p);
  const loadStorage = () => queryClient.invalidateQueries({ queryKey: STORAGE });
  const [scale, setScale] = useState(savedScale);

  // opened from the update dot or the menu: Updates comes into view, lit up for a moment
  const focus = useNav((s) => s.focus);
  const updatesRef = useRef<HTMLElement>(null);
  const [lit, setLit] = useState(false);
  useEffect(() => {
    if (focus !== "updates") return;
    useNav.getState().clearFocus();
    updatesRef.current?.scrollIntoView({ block: "start", behavior: "smooth" });
    setLit(true);
  }, [focus]);
  useEffect(() => {
    if (!lit) return;
    const t = setTimeout(() => setLit(false), 2000);
    return () => clearTimeout(t);
  }, [lit]);

  type R = { ok?: boolean; error?: string; cancelled?: boolean };
  const post = <T extends R>(path: string, body: unknown = {}) => postJson<T>(path, body).catch((): T => ({ error: "Couldn't reach the app." }) as T);
  const setPref = (p: Partial<Prefs>) => run(`app:pref:${Object.keys(p).join(",")}`, async () => {
    const r = await post<Prefs & R>("/api/app/prefs", p);
    if (r.error) return toast(r.error, true);
    queryClient.setQueryData(PREFS, r);
  }, "Saving the setting…");
  const resetPhones = async () => {
    if (!(await ask("Sign out every phone? Each one will need to scan the new code to connect again.", { ok: "Sign out phones", danger: true }))) return;
    await run("app:phones:reset", async () => {
      const r = await post<PhoneInfo & R>("/api/app/phone/reset");
      if (r.error) return toast(r.error, true);
      setPhone(r);
      toast("All phones signed out. Scan the new code to connect again.");
    }, "Signing out all phones…");
  };
  const clear = async (what: "downloads" | "pictures") => {
    const msg = what === "downloads" ? "Delete the downloaded mods and maps? They're downloaded again if you reinstall something." : "Delete the saved map pictures? They're looked up again as you browse maps.";
    if (!(await ask(msg, { ok: "Delete", danger: true }))) return;
    await run(`app:clear:${what}`, async () => {
      const r = await post("/api/app/storage/clear", { what });
      if (r.error) return toast(r.error, true);
      toast("Cleared");
      loadStorage();
    }, what === "downloads" ? "Clearing downloads…" : "Clearing map pictures…");
  };
  const copyDiagnostics = () => run("app:diagnostics", async () => {
    const r = await getJson<{ text: string }>("/api/app/diagnostics").catch(() => null);
    if (!r) return toast("Couldn't collect the diagnostics. Try again.", true);
    if (!ON_THIS_COMPUTER) return setDiag(r.text);
    if (await copyText(r.text)) toast("Copied. Paste it into a GitHub issue.");
    else setDiag(r.text);
  }, "Collecting diagnostics…");
  const addFolder = () => run("app:folder:add", async () => {
    const picked = await post<{ dir?: string | null } & R>("/api/servers/pick-folder", { purpose: "game" });
    if (picked.error) return toast(picked.error, true);
    if (!picked.dir) return;
    const r = await post<Prefs & R>("/api/app/game-folder", { dir: picked.dir });
    if (r.error) return toast(r.error, true);
    setPrefsData((p) => ({ ...p, game_folders: r.game_folders }));
    toast("Added. Run full setup to set up the game in it.");
  }, "Choosing a folder…");
  const serversDir = useServersDir();
  const pickServersDir = (reset = false) => run(reset ? "app:servers-dir:reset" : "app:servers-dir", async () => {
    let dir = "";
    if (!reset) {
      const picked = await post<{ dir?: string | null } & R>("/api/servers/pick-folder", { purpose: "servers" });
      if (picked.error) return toast(picked.error, true);
      if (!picked.dir) return;
      dir = picked.dir;
    }
    const r = await post<ServersDir & R>("/api/app/servers-folder", { dir });
    if (r.error) return toast(r.error, true);
    queryClient.setQueryData(SERVERS_DIR, r);
    toast(reset ? "New servers go in the usual place again" : "New servers go in that folder");
  }, reset ? "Going back to the usual servers folder…" : "Choosing a folder…");
  const dirBusy = busy("app:servers-dir") || busy("app:servers-dir:reset");
  const removeFolder = (dir: string) => run(`app:folder:remove:${dir}`, async () => {
    const r = await post<Prefs & R>("/api/app/game-folder/remove", { dir });
    if (r.error) return toast(r.error, true);
    setPrefsData((p) => ({ ...p, game_folders: r.game_folders }));
    toast("Forgotten. Nothing was deleted.");
  }, "Forgetting the folder…");
  const openFolder = () => run("app:open-folder", async () => {
    const r = await post("/api/app/open-folder");
    if (r.error) toast(r.error, true);
  }, "Opening the folder…");
  const exportSetup = () => run("app:export", async () => {
    const r = await post<{ files?: number } & R>("/api/app/export");
    if (r.error) return toast(r.error, true);
    if (r.ok) toast("Exported your setup");
  }, "Exporting your setup…");
  const importSetup = async () => {
    if (!(await ask("Import a setup file? It replaces this game's mode settings, presets, admins and welcome page with the ones in the file.", { ok: "Choose a file" }))) return;
    await run("app:import", async () => {
      const r = await post<{ missing?: string[]; hl?: string; game?: string } & R>("/api/app/import");
      if (r.error) return toast(r.error, true);
      if (r.cancelled) return;
      if (r.missing?.length && (await ask(`Done. The setup also uses ${r.missing.length} mode${r.missing.length === 1 ? "" : "s"} or add-on${r.missing.length === 1 ? "" : "s"} that aren't installed here. Install them now?`, { ok: "Install them" }))) {
        const i = await post<{ job?: string } & R>("/api/setup/install", { hl: r.hl, games: [r.game], components: r.missing });
        if (i.error) return toast(i.error, true);
        toast("Installing them. Mods shows the progress.");
      } else toast("Imported. It applies on the next map.");
      reloadApp();
    }, "Importing a setup…");
  };

  const setPhoneOn = (enabled: boolean) => run("app:phone", async () => {
    const r = await postJson<PhoneInfo>("/api/app/phone", { enabled }).catch(() => null);
    if (!r) return toast("Couldn't change phone access.", true);
    setPhone(r);
    if (r.error) toast(r.error, true);
  }, enabled ? "Turning on phone access…" : "Turning off phone access…");

  const check = () => run("app:update:check", async () => {
    const r = await postJson<{ available?: boolean; version?: string; error?: string }>("/api/app/update/check").catch((): { available?: boolean; error?: string } => ({ error: "Couldn't check for updates." }));
    noteUpdateChecked();
    if (r.error) return toast(r.error, true);
    setChecked(r.available ? "" : "You're on the latest version.");
    reloadApp();
  }, "Checking for updates…");

  const install = () => run("app:update:install", async () => {
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/app/update/install").catch(() => ({ error: "The update didn't install." }));
    if (r.error) toast(r.error, true);
    else {
      setRestarting(true);
      toast("Installed. Restarting…");
    }
  }, `Installing Loner-Strike ${app?.update?.version ?? "update"}…`);
  const removeAll = async () => {
    const inst = scan?.installs.find((i) => app?.game && app.game.dir.startsWith(i.hl));
    if (!app?.game || !inst) return;
    const title = gameTitle(app.game.id);
    // each game is set up on its own, so removing is per game too
    const other = inst.games.find((g) => g.id !== app.game!.id && g.installed);
    const otherNote = other ? ` ${gameTitle(other.id)} isn't touched: switch to it and remove it there too if you want.` : "";
    if (!(await ask(`Remove Loner-Strike from ${title} and restore its original files? Close the game first.${otherNote} Your settings, admins and stats are kept, in case you set it up again.`, { ok: `Remove from ${title}`, danger: true }))) return;
    const game = app.game.id;
    await run("app:uninstall", async () => {
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/setup/uninstall", { hl: inst.hl, games: [game] }).catch(() => ({ error: "Couldn't reach the app." }));
      if (r.error) return toast(r.error, true);
      toast(`Removing Loner-Strike from ${title}. The app restarts when it's done.`);
      // runs on even if you leave this page, so the spinner stays until it's done
      const j = await waitForSetupJob();
      if (!j) return toast("Lost track of the removal. Check Mods to see if it finished, then restart Loner-Strike.", true);
      if (j.state === "error") toast(j.error || "Couldn't remove everything. Nothing was restarted.", true);
      else {
        // that was the last game with Loner-Strike: the intro shows again next time, like a first run
        if (!other) {
          try { localStorage.removeItem("ls.setupLater"); } catch { /* private window */ }
        }
        await postJson("/api/app/restart").catch(() => null); // the app going away can drop the reply
      }
    }, `Removing Loner-Strike from ${title}…`);
  };

  const updates = (
    <section ref={updatesRef} className={lit ? "settings-lit" : undefined}>
      <SectionHead title="Updates" />
      <div className="panel settings-list">
        <div className="row between setting-line">
          <div>
            <b>Loner-Strike {app?.version}</b>
            {app?.update ? (
              <p>Version {app.update.version} is available.</p>
            ) : (
              // checked here, from the menu, or when the app started (with that switch on)
              <p className="muted">{checked || (checkedOnce || prefs?.auto_update ? "You're up to date as of the last check." : "Not checked yet.")}</p>
            )}
          </div>
          {app?.update && !ON_THIS_COMPUTER ? (
            <p className="muted">Install it on the computer running Loner-Strike.</p>
          ) : app?.update ? (
            <BusyButton className="btn primary" busy={busy("app:update:install") || restarting} label={`Install ${app.update.version}`} busyLabel={restarting ? "Restarting…" : "Installing…"} onClick={install} />
          ) : (
            <BusyButton busy={busy("app:update:check")} label="Check now" busyLabel="Checking…" onClick={check} />
          )}
        </div>
        <div className="row between setting-line toggle">
          <div>
            <b>Check when Loner-Strike starts</b>
            <p className="muted">It asks before installing anything.</p>
          </div>
          <Switch on={!!prefs?.auto_update} label="Check for updates when Loner-Strike starts" disabled={!prefs || busy("app:pref:auto_update")} onChange={(v) => setPref({ auto_update: v })} />
        </div>
      </div>
    </section>
  );

  return (
    <div className="stack narrow app-settings">
      {app?.update && updates}
      <section>
        <SectionHead title="Game" />
        <div className="panel">
          {app?.game ? (
            <>
              <p className="big">{gameTitle(app.game.id)}</p>
              <p className="muted mono">{app.game.dir}</p>
            </>
          ) : (
            <p>Couldn't find Condition Zero or Counter-Strike 1.6 on this computer.</p>
          )}
          {scan && ON_THIS_COMPUTER && (
            <div className="row" style={{ marginTop: 12 }}>
              {scan.installs.flatMap((i) =>
                i.games
                  .filter((g) => !(app?.game && app.game.id === g.id && app.game.dir.startsWith(i.hl)))
                  .map((g) => (
                    <BusyButton
                      key={i.hl + g.id}
                      busy={busy(`app:game:${i.hl}:${g.id}`)}
                      label={<>Switch to {gameTitle(g.id)}{g.installed ? "" : " (not set up)"}</>}
                      busyLabel="Switching…"
                      onClick={() => switchGame(i.hl, g.id)}
                    />
                  )),
              )}
            </div>
          )}
          {!ON_THIS_COMPUTER && <p className="muted small-note">Switching games is only on the computer running Loner-Strike.</p>}
          {!!prefs?.game_folders.length && (
            <ul className="folders">
              {prefs.game_folders.map((d) => (
                <li key={d}>
                  <span className="mono small">{d}</span>
                  {ON_THIS_COMPUTER && <BusyButton className="btn small" busy={busy(`app:folder:remove:${d}`)} label="Forget" busyLabel="Forgetting…" onClick={() => removeFolder(d)} />}
                </li>
              ))}
            </ul>
          )}
          {ON_THIS_COMPUTER && (
            <p className="muted small-note">
              Game not found?{" "}
              <BusyButton className="link inline" busy={busy("app:folder:add")} label="Choose its Half-Life folder" busyLabel="Choosing…" onClick={addFolder} /> (the one with cstrike or czero inside).
              <Info text="For a game Loner-Strike didn't find by itself, like one on another drive. Folders you add are listed above; Forget only takes one off the list and deletes nothing." />
            </p>
          )}
        </div>
      </section>

      {ON_THIS_COMPUTER && serversDir.data && (
        <section>
          <SectionHead title="Servers folder" note="Where new dedicated servers go. Servers you already made stay where they are." />
          <div className="panel stack-tight">
            <p className="mono small">{serversDir.data.dir || serversDir.data.default || "Not known until a game is set up"}</p>
            <div className="row">
              <BusyButton className="btn" busy={busy("app:servers-dir")} disabled={dirBusy} label="Change" busyLabel="Choosing…" onClick={() => pickServersDir()} />
              {serversDir.data.dir && <BusyButton className="btn" busy={busy("app:servers-dir:reset")} disabled={dirBusy} label="Use the usual place" busyLabel="Saving…" onClick={() => pickServersDir(true)} />}
              <Info text={serversDir.data.mac ? "On a Mac it has to be inside the same CrossOver bottle as the game, since the servers run through it." : "Any folder on this computer with room for the server files (about 1 GB each)."} />
            </div>
          </div>
        </section>
      )}

      <section>
        <SectionHead title="Setup" note="Add or remove game modes and add-ons, or set up another game." />
        <div className="panel row between">
          <p className="muted">Everything Loner-Strike changed is backed up, so removing it puts your game back the way it was.</p>
          {ON_THIS_COMPUTER ? (
            <div className="row">
              <button className="btn primary" onClick={openSetup}>Run full setup</button>
              <Info text="Set up another game, or add or remove many modes at once. Nothing already set up is lost." />
              {/* for working on the intro (music, sounds): only in a development build */}
              {import.meta.env.DEV && <button className="btn" onClick={() => useNav.getState().setSetupOpen(true, true)}>Watch the intro</button>}
              {app?.game && scan && (
                <BusyButton className="btn danger" busy={busy("app:uninstall")} label={`Remove from ${gameTitle(app.game.id)}`} busyLabel="Removing…" onClick={removeAll} />
              )}
              {scan && <button className="btn danger" onClick={() => setNuke(true)}>Remove everything</button>}
            </div>
          ) : (
            <p className="muted">{ON_COMPUTER_ONLY}</p>
          )}
        </div>
      </section>

      <section>
        <SectionHead title="Use on your phone" note="Control the game from your phone on the same Wi-Fi." />
        {ON_THIS_COMPUTER ? (
          <div className="panel phone">
            <div className="row between setting-line toggle">
              <div>
                <b>Phone access</b>
                <p className="muted">Only phones that scan the code can connect. Turn it off when you don't need it.</p>
              </div>
              <Switch on={!!phone?.enabled} label="Phone access" disabled={!phone || busy("app:phone")} onChange={setPhoneOn} />
            </div>
            {phone?.enabled && phone.qr && (
              <div className="qr-row">
                <img className="qr" src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(phone.qr)}`} alt="QR code to open Loner-Strike on your phone" />
                <div>
                  <p>Scan with your phone's camera.</p>
                  <p className="muted">Or open this address on your phone and add it to your home screen:</p>
                  <p className="address small">{phone.address}</p>
                  <p className="muted">The first visit needs the code (it's in the QR). After that the phone stays signed in.</p>
                  {/Win/.test(navigator.platform) && <p className="muted">If the phone can't open it, Windows may think this Wi-Fi is public: in Settings, Network & internet, open your Wi-Fi and pick Private network.</p>}
                </div>
              </div>
            )}
            {phone?.error && <p className="err-text">{phone.error}</p>}
            {phoneQ.isError && !phone && <LoadError text="Couldn't load phone access." onRetry={() => phoneQ.refetch()} />}
            <div className="row between setting-line">
              <div>
                <b>Sign out all phones</b>
                <p className="muted">For a lost phone, or a code you shared by mistake. Every phone has to scan the new code.</p>
              </div>
              <BusyButton className="btn danger" busy={busy("app:phones:reset")} label="Sign out phones" busyLabel="Signing out…" onClick={resetPhones} />
            </div>
          </div>
        ) : (
          <div className="panel phone">
            <p><b>This phone is connected</b></p>
            <p className="muted">Phone access and the code for other phones are only on the computer running Loner-Strike.</p>
          </div>
        )}
      </section>

      <section>
        <SectionHead title="App" />
        <div className="panel settings-list">
          {prefsQ.isError && !prefs && <LoadError text="Couldn't load these settings." onRetry={() => prefsQ.refetch()} />}
          <div className="row between setting-line">
            <div>
              <b>Text size</b>
              <p className="muted">For this {ON_THIS_COMPUTER ? "computer" : "phone"} only.</p>
            </div>
            <div className="seg" role="group" aria-label="Text size">
              {SCALES.map(([label, v]) => (
                <button key={label} aria-pressed={scale === v} onClick={() => { applyScale(v); setScale(v); }}>{label}</button>
              ))}
            </div>
          </div>
          <div className="row between setting-line toggle">
            <div>
              <b>Tell me when someone joins</b>
              <p className="muted">A notification on the computer running Loner-Strike when a person (not a bot) joins your game or one of your servers, while the app is in the background.</p>
            </div>
            <Switch on={!!prefs?.notify_joins} label="Tell me when someone joins" disabled={!prefs || busy("app:pref:notify_joins")} onChange={(v) => setPref({ notify_joins: v })} />
          </div>
          <div className="row between setting-line toggle">
            <div>
              <b>Keep servers running when I close Loner-Strike</b>
              <p className="muted">Off: closing the app stops your dedicated servers. On: they keep going, and the app picks them up again next time.</p>
            </div>
            <Switch on={!!prefs?.keep_servers} label="Keep servers running" disabled={!prefs || busy("app:pref:keep_servers")} onChange={(v) => setPref({ keep_servers: v })} />
          </div>
        </div>
      </section>

      <section>
        <SectionHead title="Your setup" note="Save this game's mode settings, presets, admins and welcome page to a file, to move to another computer or set up a server the same way." />
        <div className="panel row between">
          <p className="muted">Passwords aren't included.</p>
          {ON_THIS_COMPUTER ? (
            <div className="row">
              <BusyButton busy={busy("app:export")} label="Export" busyLabel="Exporting…" onClick={exportSetup} />
              <BusyButton busy={busy("app:import")} label="Import" busyLabel="Importing…" onClick={importSetup} />
            </div>
          ) : (
            <p className="muted">{ON_COMPUTER_ONLY}</p>
          )}
        </div>
      </section>

      <section>
        <SectionHead title="Storage" />
        <div className="panel settings-list">
          {storageQ.isError && !storage && <LoadError text="Couldn't work out how much space Loner-Strike uses." onRetry={() => storageQ.refetch()} />}
          <div className="row between setting-line">
            <div>
              <b>Downloads</b> <Size n={storage?.downloads} />
              <p className="muted">Mods and maps kept so reinstalling doesn't download them again.</p>
            </div>
            {ON_THIS_COMPUTER && <BusyButton busy={busy("app:clear:downloads")} disabled={!storage?.downloads} label="Clear" busyLabel="Clearing…" onClick={() => clear("downloads")} />}
          </div>
          <div className="row between setting-line">
            <div>
              <b>Map pictures</b> <Size n={storage?.pictures} />
              <p className="muted">Screenshots for the map browser. They're fetched again as needed.</p>
            </div>
            {ON_THIS_COMPUTER && <BusyButton busy={busy("app:clear:pictures")} disabled={!storage?.pictures} label="Clear" busyLabel="Clearing…" onClick={() => clear("pictures")} />}
          </div>
          {ON_THIS_COMPUTER && (
            <div className="row between setting-line">
              <div>
                <b>Loner-Strike's folder</b>
                {storage ? <p className="muted mono small">{storage.folder}</p> : <span className="size-skeleton wide" aria-hidden="true" />}
              </div>
              <BusyButton busy={busy("app:open-folder")} label={`Show in ${navigator.platform.startsWith("Mac") ? "Finder" : "Explorer"}`} busyLabel="Opening…" onClick={openFolder} />
            </div>
          )}
        </div>
      </section>

      <section>
        <SectionHead title="Help" />
        <div className="panel row between">
          <div>
            <b>Copy diagnostics</b>
            <p className="muted">Version, game folders, what's installed and recent errors, for a bug report. No passwords.</p>
          </div>
          <div className="row">
            <BusyButton busy={busy("app:diagnostics")} label="Copy" busyLabel="Collecting…" onClick={copyDiagnostics} />
            <a className="btn" href="https://github.com/jamesontucker/Loner-Strike/issues" target="_blank" rel="noreferrer">Report a problem</a>
          </div>
        </div>
      </section>

      {!app?.update && updates}

      {nuke && scan && <RemoveEverything scan={scan} onClose={() => setNuke(false)} />}

      {diag !== null && (
        <Sheet label="Diagnostics" onClose={() => setDiag(null)}>
          <div className="sheet-body">
            <h2>Diagnostics</h2>
            <p className="muted">Copy this into a GitHub issue. No passwords are in it.</p>
            <textarea className="invite code" readOnly value={diag} aria-label="Diagnostics" />
            <div className="row">
              <button className="btn primary" onClick={async () => {
                const ok = await copyText(diag);
                toast(ok ? "Copied. Paste it into a GitHub issue." : "Couldn't copy. Select the text and copy it by hand.", !ok);
              }}>Copy</button>
              <button className="btn" onClick={() => setDiag(null)}>Close</button>
            </div>
          </div>
        </Sheet>
      )}

      <section>
        <SectionHead title="About" />
        <p className="muted">
          Play Counter-Strike like it's 2004, alone or with the friends you don't have. Not affiliated with Valve.{" "}
          <a href="https://github.com/jamesontucker/Loner-Strike" target="_blank" rel="noreferrer">Source and credits on GitHub</a>. Made by{" "}
          <a href="https://x.com/nanoshrine" target="_blank" rel="noreferrer">nanoshrine</a>.
        </p>
      </section>
    </div>
  );
}

/** A size, or a grey bar while it loads. */
function Size({ n }: { n?: number }) {
  return n === undefined ? <span className="size-skeleton" aria-label="Loading" /> : <span className="muted">{mb(n)}</span>;
}

/** Remove everything: Loner-Strike out of every game, the servers on this computer deleted,
 *  and the app's own data gone, then the app quits. Servers on other machines keep running. */
function RemoveEverything({ scan, onClose }: { scan: Scan; onClose: () => void }) {
  const toast = useToast();
  const { run, busy } = useActions();
  const serversQ = useServers();
  const [sure, setSure] = useState(false);
  const [step, setStep] = useState("");
  const games = scan.installs.flatMap((i) => i.games.filter((g) => g.installed).map((g) => ({ hl: i.hl, id: g.id })));
  const servers = serversQ.data?.servers ?? [];
  const here = servers.filter((s) => !s.host);
  const away = servers.filter((s) => s.host);
  const list = (names: string[]) => names.join(", ");
  const working = busy("app:remove-everything");

  const go = () => run("app:remove-everything", async () => {
    // each game the normal way first: original files back, Loner-Strike's out
    for (const g of games) {
      setStep(`Removing from ${gameTitle(g.id)}…`);
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/setup/uninstall", { hl: g.hl, games: [g.id] }).catch(() => ({ error: "Couldn't reach the app." }));
      if (r.error) return toast(r.error, true);
      const j = await waitForSetupJob();
      if (!j || j.state === "error") return toast(j?.error || `Couldn't remove Loner-Strike from ${gameTitle(g.id)}. Nothing else was deleted.`, true);
    }
    setStep("Deleting the servers and the app's data…");
    try { localStorage.clear(); sessionStorage.clear(); } catch { /* private window */ }
    type Done = { ok?: boolean; error?: string; failed?: string[] };
    const r: Done = await postJson<Done>("/api/app/remove-everything").catch((): Done => ({ error: "Couldn't reach the app." }));
    if (r.error) return toast(r.error, true);
    const left = r.failed ?? [];
    setStep(left.length ? `Done, except: ${left.join("; ")}. Delete those by hand. Loner-Strike is quitting.` : "Everything is removed. Loner-Strike is quitting.");
  }, "Removing everything…");

  return (
    <Sheet label="Remove everything" onClose={() => !working && onClose()}>
      <div className="sheet-body">
        <h2>Remove everything</h2>
        <p>This takes Loner-Strike off this computer completely:</p>
        <ul className="plain-list">
          {games.length > 0 && <li>It's removed from {list(games.map((g) => gameTitle(g.id)))}, and their original files are put back. Your own maps and files from before Loner-Strike stay.</li>}
          {here.length > 0 && <li>The servers on this computer are stopped and deleted, with their folders: {list(here.map((s) => s.name))}.</li>}
          <li>Its settings, servers list, map records, caches and downloads are deleted, and so are the stats, admins and XP a normal remove keeps. Then the app quits.</li>
        </ul>
        {away.length > 0 && (
          <p className="muted">Servers on other machines aren't touched and keep running: {list(away.map((s) => s.name))}. To take one down too, delete it in Servers first.</p>
        )}
        <label className="check-row">
          <input type="checkbox" checked={sure} disabled={working} onChange={(e) => setSure(e.target.checked)} />
          <span>I understand this can't be undone</span>
        </label>
        {step && <p className="muted" role="status">{step}</p>}
        <div className="row">
          <BusyButton className="btn danger" busy={working} disabled={!sure} label="Remove everything" busyLabel="Removing…" onClick={go} />
          <button className="btn" disabled={working} onClick={onClose}>Cancel</button>
        </div>
      </div>
    </Sheet>
  );
}
