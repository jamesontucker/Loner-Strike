import { useEffect, useState } from "react";
import { getJson, postJson } from "../api";
import type { Online } from "../types";
import { SectionHead, useCommand, useToast } from "../ui";

type Share = {
  installed: boolean;
  connected: boolean;
  claim: { code: string; url: string; step: string; error: string } | null;
  phase: "off" | "starting" | "online" | "error";
  error: string;
  address: { host: string | null; ip: string | null; port: number } | null;
  account: string;
  name: string;
  password: string;
  method: "playit" | "tailscale";
  tailscale: { installed: boolean; state?: string; ip?: string | null; name?: string | null };
  port: number;
  ok?: boolean;
};

function addressOf(sh: Share) {
  if (sh.method === "tailscale") {
    const ts = sh.tailscale;
    return ts.state === "Running" && ts.ip ? `${ts.ip}:${sh.port}` : "";
  }
  return sh.address ? `${sh.address.ip || sh.address.host}:${sh.address.port}` : "";
}

function invite(sh: Share) {
  const addr = addressOf(sh);
  const pw = sh.password;
  return [
    `Join my Counter-Strike: Condition Zero game${sh.name ? ` "${sh.name}"` : ""}!`,
    ...(sh.method === "tailscale" ? ["", "Have Tailscale running and signed in to the network I shared with you first (tailscale.com/download)."] : []),
    "",
    "Paste this into your browser's address bar and Steam opens the game and joins:",
    `steam://connect/${addr}${pw ? "/" + pw : ""}`,
    "",
    "Or in the game, open the console (~) and type:",
    `${pw ? `password ${pw}; ` : ""}connect ${addr}`,
    "",
    `Or go to Find Servers > Favorites > Add a server and enter ${addr}${pw ? ` (password: ${pw})` : ""}`,
    "",
    "You need Counter-Strike: Condition Zero on Steam. The first time you join it downloads my maps, which can take a minute.",
  ].join("\n");
}

function lanInvite(sh: Share, addr: string) {
  const pw = sh.password;
  return [
    `LAN game${sh.name ? `: "${sh.name}"` : ""}!`,
    "",
    "In Counter-Strike, open Find Servers > LAN and join it. Or open the console (~) and type:",
    `${pw ? `password ${pw}; ` : ""}connect ${addr}`,
    ...(pw ? ["", `Password: ${pw}`] : []),
  ].join("\n");
}

async function copy(text: string, toast: (t: string, e?: boolean) => void) {
  try {
    await navigator.clipboard.writeText(text);
    toast("Copied");
  } catch {
    toast("Couldn't copy. Select the text and copy it yourself.", true);
  }
}

export function Friends({ data, refresh }: { data: Online | null; refresh: () => void }) {
  const toast = useToast();
  const run = useCommand(refresh);
  const [sh, setSh] = useState<Share | null>(null);
  const [name, setName] = useState<string | null>(null);
  const [pw, setPw] = useState<string | null>(null);
  const [missing, setMissing] = useState(false);
  // LAN needs nothing from the share service, so it's remembered here
  const [lan, setLanRaw] = useState(() => {
    try { return localStorage.getItem("ls.lan") === "1"; } catch { return false; }
  });
  const setLan = (v: boolean) => {
    setLanRaw(v);
    try { localStorage.setItem("ls.lan", v ? "1" : "0"); } catch { /* private window */ }
  };
  const [lanAddr, setLanAddr] = useState<{ ip: string | null; port: number } | null>(null);
  useEffect(() => {
    getJson<{ ip: string | null; port: number }>("/api/app/lan").then(setLanAddr).catch(() => setLanAddr(null));
  }, []);

  useEffect(() => {
    const load = () =>
      getJson<Share>("/api/share")
        .then((s) => {
          if ((s as unknown as { error?: string }).error && !("phase" in s)) setMissing(true);
          else setSh(s);
        })
        .catch(() => setMissing(true));
    load();
    const t = setInterval(load, 2000);
    return () => clearInterval(t);
  }, []);

  const act = async (action: string, extra: Record<string, unknown> = {}) => {
    const r = await postJson<Share & { error?: string }>("/api/share", { action, ...extra }).catch(() => null);
    if (!r || (r.ok === false && r.error)) return toast(r?.error || "That didn't work.", true), null;
    setSh(r);
    return r;
  };

  if (missing) return <div className="stack"><SectionHead title="Play with friends" /><p className="muted">Sharing isn't in this version of the app yet.</p></div>;
  if (!sh) return <p className="muted">Loading…</p>;

  const viaTs = sh.method === "tailscale";
  const live = viaTs ? sh.tailscale.state === "Running" && !!sh.tailscale.ip : sh.phase === "online";
  const addr = addressOf(sh);
  const friends = data ? data.players.filter((p) => !p.bot && !p.host).map((p) => p.name) : [];
  const st = data?.state;
  const full = st && st.humans + st.bots >= st.maxplayers;

  return (
    <div className="stack">
      <SectionHead title="Play with friends" note="Friends join over the internet, with nothing to change on your router. They need Condition Zero on Steam." />
      <div className="friends">
        <div className="stack">
          <div>
            <div className="seg" role="group" aria-label="How friends connect">
              <button aria-pressed={lan} onClick={() => setLan(true)}>Same network (LAN)</button>
              <button aria-pressed={!lan && !viaTs} onClick={() => { setLan(false); act("method", { method: "playit" }); }}>playit.gg</button>
              <button aria-pressed={!lan && viaTs} onClick={() => { setLan(false); act("method", { method: "tailscale" }); }}>Tailscale</button>
            </div>
            <p className="muted">
              {lan
                ? "Friends on the same Wi-Fi or network join directly. Nothing to install or sign up for: a LAN party."
                : viaTs
                ? "A private network that you and your friends join over the internet. Everyone installs the free Tailscale app."
                : "A free relay that gives your game a public address over the internet. Friends don't install anything; the password keeps strangers out."}
            </p>
          </div>
          {lan ? (
            <ol className="steps">
              <li className="done">
                <strong>Get everyone on the same network</strong>
                <p>Same Wi-Fi, or plugged into the same router.</p>
              </li>
              <li className="now">
                <strong>Start your game</strong>
                <p>Play from Loner-Strike as usual. The first time a friend joins, your Mac may ask whether to allow incoming connections for the game (Wine/CrossOver): click Allow.</p>
              </li>
              <li className="now">
                <strong>Friends join from the LAN tab</strong>
                <p>In Counter-Strike they open <b>Find Servers</b>, then the <b>LAN</b> tab, and your server shows up{sh.name ? ` as "${sh.name}"` : ""}. Or they type the address below in the console.</p>
              </li>
            </ol>
          ) : viaTs ? (
            <ol className="steps">
              <li className={sh.tailscale.installed ? "done" : "now"}>
                <strong>Install Tailscale on this computer</strong>
                <p>{sh.tailscale.installed ? "Installed." : <a href="https://tailscale.com/download" target="_blank" rel="noreferrer">Download Tailscale (free) and sign in.</a>}</p>
              </li>
              <li className={live ? "done" : sh.tailscale.installed ? "now" : ""}>
                <strong>Turn it on</strong>
                <p>{live ? `Connected as ${sh.tailscale.name || sh.tailscale.ip}.` : sh.tailscale.installed ? "Open the Tailscale app from the menu bar and connect." : "After installing."}</p>
              </li>
              <li className={live ? "now" : ""}>
                <strong>Let your friend in</strong>
                <p>Your friend installs Tailscale too. Then share this computer with them, or invite them to your network, from the <a href="https://login.tailscale.com/admin/machines" target="_blank" rel="noreferrer">Tailscale admin console</a>.</p>
              </li>
            </ol>
          ) : (
            <ol className="steps">
              <li className={sh.connected ? "done" : "now"}>
                <strong>Make a free playit.gg account</strong>
                <p>{sh.connected ? "Done." : <><a href="https://playit.gg/login/create" target="_blank" rel="noreferrer">Sign up at playit.gg</a>, then confirm the email they send you. The free plan is enough.</>}</p>
              </li>
              <li className={sh.connected ? "done" : "now"}>
                <strong>Connect this computer to your account</strong>
                {sh.connected ? (
                  <p>Done. <button className="btn small" onClick={() => confirm("Disconnect from playit.gg? You'll need to approve it again.") && act("disconnect")}>Disconnect</button></p>
                ) : sh.claim?.step === "failed" ? (
                  <><p className="err-text">{sh.claim.error}</p><button className="btn primary" onClick={() => act("connect")}>Try again</button></>
                ) : sh.claim ? (
                  <><p><a href={sh.claim.url} target="_blank" rel="noreferrer">Open playit.gg to approve this computer</a>, then come back. {sh.claim.step === "approve" ? "Waiting for you to approve…" : "Waiting for the page to open…"}</p><button className="btn" onClick={() => act("cancel")}>Cancel</button></>
                ) : (
                  <button className="btn primary" onClick={async () => { const r = await act("connect"); if (r?.claim) window.open(r.claim.url, "_blank"); }}>Connect to playit.gg</button>
                )}
              </li>
              <li className={live ? "done" : sh.connected ? "now" : ""}>
                <strong>Share the game</strong>
                <p>{sh.connected ? "Sharing runs only while Loner-Strike is open." : "Available once this computer is connected."}</p>
                {sh.connected && (live || sh.phase === "starting"
                  ? <button className="btn danger" onClick={() => act("stop")}>Stop sharing</button>
                  : <button className="btn primary" onClick={() => act("start")}>Start sharing</button>)}
                {sh.error && <p className="err-text">{sh.error}</p>}
              </li>
            </ol>
          )}
        </div>

        <div className="stack">
          <form
            className="panel"
            onSubmit={async (e) => {
              e.preventDefault();
              const p = (pw ?? sh.password).trim();
              if (!/^[A-Za-z0-9_-]{0,24}$/.test(p)) return toast("Passwords can use letters, numbers, - and _.", true);
              const r = await act("server", { name: (name ?? sh.name).trim(), password: p });
              if (r) toast(data ? "Saved and applied to the game" : "Saved. It applies when you start a game.");
            }}
          >
            <h3>Your server</h3>
            <label className="field">
              <span>Server name</span>
              <input className="search" maxLength={48} placeholder="e.g. Friday night frags" value={name ?? sh.name} onChange={(e) => setName(e.target.value)} />
            </label>
            <label className="field">
              <span>Password</span>
              <div className="row">
                <input className="search" maxLength={24} placeholder="No password" spellCheck={false} value={pw ?? sh.password} onChange={(e) => setPw(e.target.value)} />
                <button type="button" className="btn" onClick={() => {
                  const w = ["alpha", "bravo", "delta", "echo", "foxtrot", "kilo", "lima", "oscar", "sierra", "tango", "victor", "zulu"];
                  setPw(`${w[Math.floor(Math.random() * w.length)]}-${100 + Math.floor(Math.random() * 900)}`);
                }}>Make one</button>
              </div>
              <small>Letters, numbers, - and _. Leave it empty to let in anyone who has the address.</small>
            </label>
            <button className="btn primary" type="submit">Save</button>
          </form>

          {lan && (
            <div className="panel">
              <span className={`live ${data ? "on" : ""}`}>{data ? "Friends on your network can join now" : "Start the game and friends can join"}</span>
              {lanAddr?.ip ? <div className="address">{lanAddr.ip}:{lanAddr.port}</div> : <p className="err-text">This computer isn't on a network right now.</p>}
              {full && (
                <div className="banner">
                  <div>The server is full of bots, so there's no room for friends.</div>
                  <button className="btn primary" onClick={() => st && run("gm_bots", ["count", String(Math.max(0, Math.min(st.bots, st.maxplayers - st.humans - 2)))])}>Make room</button>
                </div>
              )}
              {friends.length > 0 && <p>In your game: {friends.join(", ")}</p>}
              {lanAddr?.ip && (
                <>
                  <h3>Tell your friends</h3>
                  <textarea
                    className="invite"
                    readOnly
                    value={lanInvite(sh, `${lanAddr.ip}:${lanAddr.port}`)}
                  />
                  <div className="row">
                    <button className="btn primary" onClick={() => copy(lanInvite(sh, `${lanAddr.ip}:${lanAddr.port}`), toast)}>Copy message</button>
                    <button className="btn" onClick={() => copy(`${lanAddr.ip}:${lanAddr.port}`, toast)}>Copy address</button>
                  </div>
                </>
              )}
            </div>
          )}

          {!lan && (live || sh.phase === "starting") && (
            <div className="panel">
              <span className={`live ${live ? "on" : ""}`}>{live ? "Friends can join now" : "Starting…"}</span>
              {addr ? <div className="address">{addr}</div> : <p className="muted">Getting an address ready…</p>}
              {!data && <p className="err-text">The game isn't running. Start it and friends can join.</p>}
              {full && (
                <div className="banner">
                  <div>The server is full of bots, so there's no room for friends.</div>
                  <button className="btn primary" onClick={() => st && run("gm_bots", ["count", String(Math.max(0, Math.min(st.bots, st.maxplayers - st.humans - 2)))])}>Make room</button>
                </div>
              )}
              {friends.length > 0 && <p>In your game: {friends.join(", ")}</p>}
              {addr && (
                <>
                  <h3>Send this to your friends</h3>
                  <textarea className="invite" readOnly value={invite(sh)} />
                  <div className="row">
                    <button className="btn primary" onClick={() => copy(invite(sh), toast)}>Copy message</button>
                    <button className="btn" onClick={() => copy(addr, toast)}>Copy address</button>
                  </div>
                </>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
