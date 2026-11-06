import { useEffect, useState } from "react";
import { getJson, setupJob, postJson, useActions } from "../api";
import type { AppInfo, PhoneInfo } from "../types";
import { ON_THIS_COMPUTER } from "../api";
import { applyScale, savedScale, BusyButton, Info, SCALES, SectionHead, Switch, useConfirm, useToast } from "../ui";
import type { Scan } from "./Setup";

type Prefs = { auto_update: boolean; keep_servers: boolean; game_folders: string[]; notify_joins: boolean };
type Storage = { downloads: number; pictures: number; folder: string };
const mb = (n?: number) => (n === undefined ? "…" : n < 1048576 ? `${Math.max(0, Math.round(n / 1024))} KB` : `${(n / 1048576).toFixed(n < 104857600 ? 1 : 0)} MB`);

export function AppSettings({ app, reloadApp, scan, openSetup, switchGame }: { app: AppInfo | null; reloadApp: () => void; scan: Scan | null; openSetup: () => void; switchGame: (hl: string | null, game: string) => Promise<void> }) {
  const ask = useConfirm();
  const toast = useToast();
  const [phone, setPhone] = useState<PhoneInfo | null>(null);
  const { run, busy } = useActions();
  // the update installed and the app is restarting: the button stays busy until it does
  const [restarting, setRestarting] = useState(false);
  const [checked, setChecked] = useState<string>("");

  const [prefs, setPrefs] = useState<Prefs | null>(null);
  const [storage, setStorage] = useState<Storage | null>(null);
  const [scale, setScale] = useState(savedScale);
  const loadStorage = () => getJson<Storage>("/api/app/storage").then(setStorage).catch(() => null);
  useEffect(() => {
    getJson<PhoneInfo>("/api/app/phone").then(setPhone).catch(() => setPhone(null));
    getJson<Prefs>("/api/app/prefs").then(setPrefs).catch(() => null);
    loadStorage();
  }, []);

  type R = { ok?: boolean; error?: string; cancelled?: boolean };
  const post = <T extends R>(path: string, body: unknown = {}) => postJson<T>(path, body).catch((): T => ({ error: "Couldn't reach the app." }) as T);
  const setPref = (p: Partial<Prefs>) => run(`app:pref:${Object.keys(p).join(",")}`, async () => {
    const r = await post<Prefs & R>("/api/app/prefs", p);
    if (r.error) return toast(r.error, true);
    setPrefs(r);
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
    if (!r) return toast("Couldn't collect them.", true);
    try {
      await navigator.clipboard.writeText(r.text);
      toast("Copied. Paste it into a GitHub issue.");
    } catch {
      toast("Couldn't copy.", true);
    }
  }, "Collecting diagnostics…");
  const addFolder = async () => {
    const picked = await post<{ dir?: string | null } & R>("/api/servers/pick-folder");
    if (!picked.dir) return;
    const r = await post<Prefs & R>("/api/app/game-folder", { dir: picked.dir });
    if (r.error) return toast(r.error, true);
    setPrefs((p) => (p ? { ...p, game_folders: r.game_folders } : p));
    toast("Added. Run setup to set it up.");
  };
  const removeFolder = async (dir: string) => {
    const r = await post<Prefs & R>("/api/app/game-folder/remove", { dir });
    if (!r.error) setPrefs((p) => (p ? { ...p, game_folders: r.game_folders } : p));
  };
  const exportSetup = () => run("app:export", async () => {
    const r = await post<{ files?: number } & R>("/api/app/export");
    if (r.error) return toast(r.error, true);
    if (r.ok) toast("Saved your setup.");
  }, "Exporting your setup…");
  const importSetup = async () => {
    if (!(await ask("Bring in a setup file? It replaces this game's mode settings, presets, admins and welcome page with the ones in the file.", { ok: "Choose a file" }))) return;
    await run("app:import", async () => {
      const r = await post<{ missing?: string[]; hl?: string; game?: string } & R>("/api/app/import");
      if (r.error) return toast(r.error, true);
      if (r.cancelled) return;
      if (r.missing?.length && (await ask(`Done. The setup also uses ${r.missing.length} mode${r.missing.length === 1 ? "" : "s"} or add-on${r.missing.length === 1 ? "" : "s"} that aren't installed here. Install them now?`, { ok: "Install them" }))) {
        const i = await post<{ job?: string } & R>("/api/setup/install", { hl: r.hl, games: [r.game], components: r.missing });
        if (i.error) return toast(i.error, true);
        toast("Installing… see Mods for progress.");
      } else toast("Setup brought in. It applies on the next map.");
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
    if (!app?.game || !inst || !(await ask("Remove Loner-Strike from this game and restore the original files? Close the game first.", { ok: "Remove", danger: true }))) return;
    const game = app.game.id;
    await run("app:uninstall", async () => {
      const r = await postJson<{ ok?: boolean; error?: string }>("/api/setup/uninstall", { hl: inst.hl, games: [game] }).catch(() => ({ error: "Couldn't start." }));
      if (r.error) return toast(r.error, true);
      toast("Removing… the app restarts when it's done.");
      // runs on even if you leave this page, so the spinner stays until it's done
      for (;;) {
        await new Promise((res) => setTimeout(res, 1000));
        const j = await setupJob<{ state: string; error?: string }>();
        if (!j || j.state === "running") continue;
        if (j.state === "error") toast(j.error || "Couldn't remove everything. Nothing was restarted.", true);
        else await postJson("/api/app/restart").catch(() => null); // the app going away can drop the reply
        return;
      }
    }, "Removing Loner-Strike…");
  };

  return (
    <div className="stack narrow app-settings">
      <section>
        <SectionHead title="Game" />
        <div className="panel">
          {app?.game ? (
            <>
              <p className="big">{app.game.id === "cstrike" ? "Counter-Strike 1.6" : "Counter-Strike: Condition Zero"}</p>
              <p className="muted mono">{app.game.dir}</p>
            </>
          ) : (
            <p>Couldn't find Condition Zero or Counter-Strike 1.6 on this computer.</p>
          )}
          {scan && (
            <div className="row" style={{ marginTop: 12 }}>
              {scan.installs.flatMap((i) =>
                i.games
                  .filter((g) => !(app?.game && app.game.id === g.id && app.game.dir.startsWith(i.hl)))
                  .map((g) => (
                    <BusyButton
                      key={i.hl + g.id}
                      busy={busy(`app:game:${i.hl}:${g.id}`)}
                      label={<>Switch to {g.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero"}{g.installed ? "" : " (not set up)"}</>}
                      busyLabel="Switching…"
                      onClick={() => switchGame(i.hl, g.id)}
                    />
                  )),
              )}
            </div>
          )}
          {!!prefs?.game_folders.length && (
            <ul className="folders">
              {prefs.game_folders.map((d) => (
                <li key={d}>
                  <span className="mono small">{d}</span>
                  <button className="btn small" onClick={() => removeFolder(d)}>Forget</button>
                </li>
              ))}
            </ul>
          )}
          {ON_THIS_COMPUTER && (
            <p className="muted small-note">
              Game not found? <button className="link inline" onClick={addFolder}>Choose its Half-Life folder</button> (the one with cstrike or czero inside).
              <Info text="For a game Loner-Strike didn't find by itself, like one on another drive. Folders you add are listed above; Forget only takes one off the list and deletes nothing." />
            </p>
          )}
        </div>
      </section>

      <section>
        <SectionHead title="Setup" note="Add or remove game modes and add-ons, or set up another game." />
        <div className="panel row between">
          <p className="muted">Everything Loner-Strike changed is backed up, so removing it puts your game back the way it was.</p>
          <div className="row">
            <button className="btn primary" onClick={openSetup}>Run setup</button>
            {app?.game && scan && (
              <BusyButton className="btn danger" busy={busy("app:uninstall")} label="Remove Loner-Strike" busyLabel="Removing…" onClick={removeAll} />
            )}
          </div>
        </div>
      </section>

      <section>
        <SectionHead title="Use on your phone" note="Control the game from your phone on the same Wi-Fi." />
        <div className="panel phone">
          <div className="row between setting-line toggle">
            <div>
              <b>Phone access</b>
              <p className="muted">Only phones that scan the code can connect. Turn it off when you don't need it.</p>
            </div>
            <Switch on={!!phone?.enabled} label="Phone access" disabled={busy("app:phone")} onChange={setPhoneOn} />
          </div>
          {phone?.enabled && phone.qr && (
            <div className="qr-row">
              <img className="qr" src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(phone.qr)}`} alt="QR code to open Loner-Strike on your phone" />
              <div>
                <p>Scan with your phone's camera.</p>
                <p className="muted">Or open this address on your phone and add it to your home screen:</p>
                <p className="address small">{phone.address}</p>
                <p className="muted">The first visit needs the code (it's in the QR). After that the phone stays signed in.</p>
              </div>
            </div>
          )}
          {phone?.error && <p className="err-text">{phone.error}</p>}
          {ON_THIS_COMPUTER && (
            <div className="row between setting-line">
              <div>
                <b>Sign out all phones</b>
                <p className="muted">For a lost phone, or a code you shared by mistake. Every phone has to scan the new code.</p>
              </div>
              <BusyButton className="btn danger" busy={busy("app:phones:reset")} label="Sign out phones" busyLabel="Signing out…" onClick={resetPhones} />
            </div>
          )}
        </div>
      </section>

      <section>
        <SectionHead title="App" />
        <div className="panel settings-list">
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
              <p className="muted">A notification when a person (not a bot) joins your game or one of your servers, while Loner-Strike is in the background.</p>
            </div>
            <Switch on={prefs?.notify_joins ?? true} label="Tell me when someone joins" disabled={busy("app:pref:notify_joins")} onChange={(v) => setPref({ notify_joins: v })} />
          </div>
          <div className="row between setting-line toggle">
            <div>
              <b>Keep servers running when I close Loner-Strike</b>
              <p className="muted">Off: closing the app stops your dedicated servers. On: they keep going, and the app picks them up again next time.</p>
            </div>
            <Switch on={!!prefs?.keep_servers} label="Keep servers running" disabled={busy("app:pref:keep_servers")} onChange={(v) => setPref({ keep_servers: v })} />
          </div>
        </div>
      </section>

      <section>
        <SectionHead title="Your setup" note="Save this game's mode settings, presets, admins and welcome page to a file, to move to another computer or set up a server the same way." />
        <div className="panel row between">
          <p className="muted">Passwords aren't included.</p>
          {ON_THIS_COMPUTER ? (
            <div className="row">
              <BusyButton busy={busy("app:export")} label="Export…" busyLabel="Exporting…" onClick={exportSetup} />
              <BusyButton busy={busy("app:import")} label="Import…" busyLabel="Importing…" onClick={importSetup} />
            </div>
          ) : (
            <p className="muted">Only on the computer running Loner-Strike.</p>
          )}
        </div>
      </section>

      <section>
        <SectionHead title="Storage" />
        <div className="panel settings-list">
          <div className="row between setting-line">
            <div>
              <b>Downloads</b> <span className="muted">{mb(storage?.downloads)}</span>
              <p className="muted">Mods and maps kept so reinstalling doesn't download them again.</p>
            </div>
            {ON_THIS_COMPUTER && <BusyButton busy={busy("app:clear:downloads")} disabled={!storage?.downloads} label="Clear" busyLabel="Clearing…" onClick={() => clear("downloads")} />}
          </div>
          <div className="row between setting-line">
            <div>
              <b>Map pictures</b> <span className="muted">{mb(storage?.pictures)}</span>
              <p className="muted">Screenshots for the map browser. They're fetched again as needed.</p>
            </div>
            {ON_THIS_COMPUTER && <BusyButton busy={busy("app:clear:pictures")} disabled={!storage?.pictures} label="Clear" busyLabel="Clearing…" onClick={() => clear("pictures")} />}
          </div>
          {ON_THIS_COMPUTER && (
            <div className="row between setting-line">
              <div>
                <b>Loner-Strike's folder</b>
                <p className="muted mono small">{storage?.folder}</p>
              </div>
              <button className="btn" onClick={() => post("/api/app/open-folder")}>Show in {navigator.platform.startsWith("Mac") ? "Finder" : "Explorer"}</button>
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

      <section>
        <SectionHead title="Updates" />
        <div className="panel settings-list">
          <div className="row between setting-line">
            <div>
              <b>Loner-Strike {app?.version}</b>
              {app?.update ? <p>Version {app.update.version} is available.</p> : <p className="muted">{checked || "You're up to date as of the last check."}</p>}
            </div>
            {app?.update ? (
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
            <Switch on={prefs?.auto_update ?? true} label="Check for updates when Loner-Strike starts" disabled={busy("app:pref:auto_update")} onChange={(v) => setPref({ auto_update: v })} />
          </div>
        </div>
      </section>

      <section>
        <SectionHead title="About" />
        <p className="muted">
          Re-live the golden days of Counter-Strike locally or with the friends you don't have. Not affiliated with Valve.{" "}
          <a href="https://github.com/jamesontucker/Loner-Strike" target="_blank" rel="noreferrer">Source and credits on GitHub</a>. Made by{" "}
          <a href="https://x.com/nanoshrine" target="_blank" rel="noreferrer">nanoshrine</a>.
        </p>
      </section>
    </div>
  );
}
