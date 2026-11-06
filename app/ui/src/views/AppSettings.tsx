import { useEffect, useState } from "react";
import { getJson, postJson } from "../api";
import type { AppInfo, PhoneInfo } from "../types";
import { SectionHead, Switch, useConfirm, useToast } from "../ui";
import type { Scan } from "./Setup";

export function AppSettings({ app, reloadApp, scan, openSetup, switchTo }: { app: AppInfo | null; reloadApp: () => void; scan: Scan | null; openSetup: () => void; switchTo: (id: string) => void }) {
  const ask = useConfirm();
  const toast = useToast();
  const [phone, setPhone] = useState<PhoneInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [checked, setChecked] = useState<string>("");

  useEffect(() => {
    getJson<PhoneInfo>("/api/app/phone").then(setPhone).catch(() => setPhone(null));
  }, []);

  const setPhoneOn = async (enabled: boolean) => {
    const r = await postJson<PhoneInfo>("/api/app/phone", { enabled }).catch(() => null);
    if (!r) return toast("Couldn't change phone access.", true);
    setPhone(r);
    if (r.error) toast(r.error, true);
  };

  const check = async () => {
    setChecking(true);
    const r = await postJson<{ available?: boolean; version?: string; error?: string }>("/api/app/update/check").catch((): { available?: boolean; error?: string } => ({ error: "Couldn't check for updates." }));
    setChecking(false);
    if (r.error) return toast(r.error, true);
    setChecked(r.available ? "" : "You're on the latest version.");
    reloadApp();
  };

  const install = async () => {
    setInstalling(true);
    const r = await postJson<{ ok?: boolean; error?: string }>("/api/app/update/install").catch(() => ({ error: "The update didn't install." }));
    if (r.error) {
      setInstalling(false);
      toast(r.error, true);
    } else toast("Installed. Restarting…");
  };

  return (
    <div className="stack narrow">
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
                    <button key={i.hl + g.id} className="btn" onClick={() => postJson("/api/app/game", { hl: i.hl, game: g.id }).then(() => switchTo(`local:${g.id}`))}>
                      Switch to {g.id === "cstrike" ? "Counter-Strike 1.6" : "Condition Zero"}
                      {g.installed ? "" : " (not set up)"}
                    </button>
                  )),
              )}
            </div>
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
              <button
                className="btn danger"
                onClick={async () => {
                  const inst = scan.installs.find((i) => app.game!.dir.startsWith(i.hl));
                  if (!inst || !(await ask("Remove Loner-Strike from this game and restore the original files? Close the game first.", { ok: "Remove", danger: true }))) return;
                  const r = await postJson<{ ok?: boolean; error?: string }>("/api/setup/uninstall", { hl: inst.hl, games: [app.game!.id] }).catch(() => ({ error: "Couldn't start." }));
                  if (r.error) toast(r.error, true);
                  else toast("Removing… the app restarts when it's done.");
                  const wait = setInterval(async () => {
                    const j = await getJson<{ state: string }>("/api/setup/job").catch(() => null);
                    if (j && j.state !== "running") { clearInterval(wait); postJson("/api/app/restart"); }
                  }, 1000);
                }}
              >
                Remove Loner-Strike
              </button>
            )}
          </div>
        </div>
      </section>

      <section>
        <SectionHead title="Use on your phone" note="Control the game from your phone on the same Wi-Fi." />
        <div className="panel phone">
          <div className="row between">
            <div>
              <p className="big">Phone access</p>
              <p className="muted">Only phones that scan this code can connect. Turn it off when you don't need it.</p>
            </div>
            <Switch on={!!phone?.enabled} label="Phone access" onChange={setPhoneOn} />
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
        </div>
      </section>

      <section>
        <SectionHead title="Updates" />
        <div className="panel row between">
          <div>
            <p className="big">Loner-Strike {app?.version}</p>
            {app?.update ? (
              <p>Version {app.update.version} is available.</p>
            ) : (
              <p className="muted">{checked || "Loner-Strike checks for updates when it starts."}</p>
            )}
          </div>
          {app?.update ? (
            <button className="btn primary" disabled={installing} onClick={install}>{installing ? "Installing…" : `Install ${app.update.version}`}</button>
          ) : (
            <button className="btn" disabled={checking} onClick={check}>{checking ? "Checking…" : "Check for updates"}</button>
          )}
        </div>
      </section>

      <section>
        <SectionHead title="About" />
        <p className="muted">
          Re-live the golden days of Counter-Strike locally or with the friends you don't have. Not affiliated with Valve.{" "}
          <a href="https://github.com/jamesontucker/Loner-Strike" target="_blank" rel="noreferrer">Source and credits on GitHub</a>.
        </p>
      </section>
    </div>
  );
}
