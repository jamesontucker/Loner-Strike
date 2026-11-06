import { useEffect, useState, type ReactNode } from "react";
import { getJson, postJson } from "../api";
import type { Online } from "../types";
import { SectionHead, Skeleton, useCommand, useConfirm, useToast } from "../ui";

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
  /** the port the playit tunnel is for right now (another game's or server's when it differs) */
  shared_port?: number;
  ok?: boolean;
};
type Direct = { on: boolean; address: string | null; error: string | null; shared_address: boolean; port?: number };
type VLans = Partial<Record<"zerotier" | "radmin" | "hamachi", string>>;

type Method = "lan" | "direct" | "playit" | "tailscale" | "zerotier" | "radmin" | "hamachi";

const METHODS: { id: Method; name: string; blurb: string; tag?: string }[] = [
  { id: "lan", name: "Same network", blurb: "Friends in the same room or on the same Wi-Fi", tag: "LAN party" },
  { id: "direct", name: "Direct", blurb: "Your router opens the port itself. Best ping, nothing to install", tag: "UPnP" },
  { id: "playit", name: "playit.gg", blurb: "A free relay. Friends install nothing" },
  { id: "tailscale", name: "Tailscale", blurb: "A private network everyone joins" },
  { id: "zerotier", name: "ZeroTier", blurb: "A virtual LAN everyone joins" },
  { id: "radmin", name: "Radmin VPN", blurb: "A virtual LAN for gamers", tag: "Windows only" },
  { id: "hamachi", name: "Hamachi", blurb: "The 2004 classic. Up to 5 people free", tag: "Retro" },
];

const GAME = "Counter-Strike";

function inviteText(name: string, pw: string, addr: string, intro: string[], lanTab: boolean) {
  return [
    `Join my ${GAME} game${name ? ` "${name}"` : ""}!`,
    ...(intro.length ? ["", ...intro] : []),
    "",
    ...(lanTab ? ["In the game, open Find Servers > LAN and join it. Or open the console (~) and type:"] : [
      "Paste this into your browser's address bar and Steam opens the game and joins:",
      `steam://connect/${addr}${pw ? "/" + pw : ""}`,
      "",
      "Or in the game, open the console (~) and type:",
    ]),
    `${pw ? `password ${pw}; ` : ""}connect ${addr}`,
    ...(pw ? ["", `Password: ${pw}`] : []),
    "",
    "The first time you join it downloads my maps, which can take a minute.",
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

function loadMethod(): Method {
  try {
    const m = localStorage.getItem("ls.method") as Method | null;
    if (m && METHODS.some((x) => x.id === m)) return m;
    return localStorage.getItem("ls.lan") === "1" ? "lan" : "playit";
  } catch {
    return "playit";
  }
}

/** server: when it's a dedicated server's page (its name and who it lets in). */
export function Friends({ data, refresh, server }: { data: Online | null; refresh: () => void; server?: { name: string; visibility: string } }) {
  const ask = useConfirm();
  const toast = useToast();
  const run = useCommand(refresh);
  const [sh, setSh] = useState<Share | null>(null);
  const [missing, setMissing] = useState(false);
  const [method, setMethodRaw] = useState<Method>(loadMethod);
  const [name, setName] = useState<string | null>(null);
  const [pw, setPw] = useState<string | null>(null);
  const [lanAddr, setLanAddr] = useState<string | null>(null);
  const [direct, setDirect] = useState<Direct | null>(null);
  const [directBusy, setDirectBusy] = useState(false);
  const [vlans, setVlans] = useState<VLans>({});

  useEffect(() => {
    const load = () => {
      getJson<Share>("/api/share")
        .then((s) => ("phase" in s ? setSh(s) : setMissing(true)))
        .catch(() => setMissing(true));
      getJson<VLans>("/api/app/vlans").then(setVlans).catch(() => setVlans({}));
    };
    load();
    getJson<{ ip: string | null; port: number }>("/api/app/lan").then((r) => setLanAddr(r.ip ? `${r.ip}:${r.port}` : null)).catch(() => null);
    getJson<Direct>("/api/app/direct").then(setDirect).catch(() => null);
    const t = setInterval(load, 3000);
    return () => clearInterval(t);
  }, []);

  const act = async (action: string, extra: Record<string, unknown> = {}) => {
    const r = await postJson<Share & { error?: string }>("/api/share", { action, ...extra }).catch(() => null);
    if (!r || (r.ok === false && r.error)) return toast(r?.error || "That didn't work.", true), null;
    setSh(r);
    return r;
  };
  const setMethod = (m: Method) => {
    setMethodRaw(m);
    try { localStorage.setItem("ls.method", m); } catch { /* private window */ }
    if (m === "playit" || m === "tailscale") act("method", { method: m });
  };
  const setDirectOn = async (on: boolean) => {
    setDirectBusy(true);
    const r = await postJson<Direct>("/api/app/direct", { on }).catch(() => null);
    setDirectBusy(false);
    if (!r) return toast("Couldn't reach the app.", true);
    setDirect(r);
    if (r.error) toast(r.error, true);
  };

  if (missing) return <div className="stack"><SectionHead title="Play with friends" /><p className="muted">Sharing isn't in this version of the app yet.</p></div>;
  if (!sh) return <Skeleton />;

  const serverName = name ?? sh.name;
  const password = pw ?? sh.password;
  const friends = data ? data.players.filter((p) => !p.bot && !p.host).map((p) => p.name) : [];
  const st = data?.state;
  const full = !!st && st.humans + st.bots >= st.maxplayers;

  // playit and Direct each serve one game or server at a time
  const here = sh.port;
  const playitElsewhere = sh.phase !== "off" && sh.shared_port !== undefined && sh.shared_port !== here;
  const directHere = !!direct?.on && (direct.port ?? here) === here;
  const directElsewhere = !!direct?.on && !directHere;
  const lanOnly = server?.visibility === "lan";

  // address + intro text + whether friends see it in the LAN tab, per method
  let address = "";
  let live = false;
  let intro: string[] = [];
  let lanTab = false;
  let steps: ReactNode = null;

  const vlanSteps = (app: string, site: string, url: string, extra: ReactNode, idHint: string) => {
    const ip = vlans[method as "zerotier" | "radmin" | "hamachi"];
    address = ip ?? "";
    live = !!ip;
    lanTab = true;
    intro = [`Have ${app} running and join my network first (${site}).`];
    return (
      <ol className="steps">
        <li className={ip ? "done" : "now"}>
          <strong>Install {app}</strong>
          <p>{ip ? "Running." : <><a href={url} target="_blank" rel="noreferrer">Download {app}</a> (free) on this computer and every friend's. {extra}</>}</p>
        </li>
        <li className={ip ? "done" : "now"}>
          <strong>Make a network and share it</strong>
          <p>{idHint}</p>
        </li>
        <li className={ip ? "now" : ""}>
          <strong>Start your game</strong>
          <p>On the same {app} network, friends see your server in <b>Find Servers → LAN</b>, or use the address below.</p>
        </li>
      </ol>
    );
  };

  if (method === "lan") {
    address = lanAddr ?? "";
    live = !!lanAddr;
    lanTab = true;
    steps = (
      <ol className="steps">
        <li className="done"><strong>Get everyone on the same network</strong><p>Same Wi-Fi, or plugged into the same router.</p></li>
        <li className="now"><strong>{server ? "Start the server" : "Start your game"}</strong><p>The first time a friend joins, your Mac may ask whether to allow incoming connections for the game (Wine/CrossOver): click Allow.</p></li>
        <li className="now"><strong>Friends join from the LAN tab</strong><p>In Counter-Strike: <b>Find Servers → LAN</b>, and your server is there{serverName ? ` as "${serverName}"` : ""}.</p></li>
      </ol>
    );
  } else if (method === "direct") {
    address = directHere && direct?.address ? direct.address : "";
    live = !!address;
    steps = (
      <ol className="steps">
        <li className={direct?.on ? "done" : "now"}>
          <strong>Open the port on your router</strong>
          <p>Loner-Strike asks your router to send game traffic to this computer (UPnP), and closes it again when you stop or quit.</p>
          {directElsewhere && <p className="muted">Direct is open for port {direct?.port} right now. Starting it here moves it to this {server ? "server" : "game"}.</p>}
          {directHere ? (
            <button className="btn danger" disabled={directBusy} onClick={() => setDirectOn(false)}>Stop</button>
          ) : (
            <button className="btn primary" disabled={directBusy} onClick={() => setDirectOn(true)}>{directBusy ? "Asking your router…" : "Start"}</button>
          )}
          {direct?.error && <p className="err-text">{direct.error}</p>}
          {direct?.shared_address && (
            <p className="err-text">Your internet connection shares its address with other customers (common with mobile and some fiber providers), so friends can't reach you directly. Use playit.gg instead.</p>
          )}
        </li>
        <li className={live ? "now" : ""}>
          <strong>Send friends your address</strong>
          <p>It's your home's internet address, so only share it with friends. Set a password above.</p>
        </li>
      </ol>
    );
  } else if (method === "playit") {
    const isLive = sh.method === "playit" && sh.phase === "online" && !playitElsewhere;
    address = isLive && sh.address ? `${sh.address.ip || sh.address.host}:${sh.address.port}` : "";
    live = isLive;
    steps = (
      <ol className="steps">
        <li className={sh.connected ? "done" : "now"}>
          <strong>Make a free playit.gg account</strong>
          <p>{sh.connected ? "Done." : <><a href="https://playit.gg/login/create" target="_blank" rel="noreferrer">Sign up at playit.gg</a>, then confirm the email they send you. The free plan is enough.</>}</p>
        </li>
        <li className={sh.connected ? "done" : "now"}>
          <strong>Connect this computer to your account</strong>
          {sh.connected ? (
            <p>Done. <button className="btn small" onClick={async () => (await ask("Disconnect from playit.gg? You'll need to approve it again.", { ok: "Disconnect", danger: true })) && act("disconnect")}>Disconnect</button></p>
          ) : sh.claim?.step === "failed" ? (
            <><p className="err-text">{sh.claim.error}</p><button className="btn primary" onClick={() => act("connect")}>Try again</button></>
          ) : sh.claim ? (
            <><p><a href={sh.claim.url} target="_blank" rel="noreferrer">Open playit.gg to approve this computer</a>, then come back. {sh.claim.step === "approve" ? "Waiting for you to approve…" : "Waiting for the page to open…"}</p><button className="btn" onClick={() => act("cancel")}>Cancel</button></>
          ) : (
            <button className="btn primary" onClick={async () => { const r = await act("connect"); if (r?.claim) window.open(r.claim.url, "_blank"); }}>Connect to playit.gg</button>
          )}
        </li>
        <li className={isLive ? "done" : sh.connected ? "now" : ""}>
          <strong>Share the game</strong>
          <p>{sh.connected ? "Sharing runs only while Loner-Strike is open." : "Available once this computer is connected."}</p>
          {playitElsewhere && <p className="muted">playit.gg is sharing {server ? "your game or another server" : "a server"} right now. Starting it here moves it.</p>}
          {sh.connected && (isLive || (sh.phase === "starting" && !playitElsewhere)
            ? <button className="btn danger" onClick={() => act("stop")}>Stop sharing</button>
            : <button className="btn primary" onClick={() => act("start")}>Start sharing</button>)}
          {sh.error && <p className="err-text">{sh.error}</p>}
        </li>
      </ol>
    );
  } else if (method === "tailscale") {
    const ts = sh.tailscale;
    const running = ts.state === "Running" && !!ts.ip;
    address = running ? `${ts.ip}:${sh.port}` : "";
    live = running;
    intro = ["Have Tailscale running and signed in to the network I shared with you first (tailscale.com/download)."];
    steps = (
      <ol className="steps">
        <li className={ts.installed ? "done" : "now"}><strong>Install Tailscale on this computer</strong><p>{ts.installed ? "Installed." : <a href="https://tailscale.com/download" target="_blank" rel="noreferrer">Download Tailscale (free) and sign in.</a>}</p></li>
        <li className={running ? "done" : ts.installed ? "now" : ""}><strong>Turn it on</strong><p>{running ? `Connected as ${ts.name || ts.ip}.` : ts.installed ? "Open the Tailscale app from the menu bar and connect." : "After installing."}</p></li>
        <li className={running ? "now" : ""}><strong>Let your friend in</strong><p>Your friend installs Tailscale too. Then share this computer with them, or invite them to your network, from the <a href="https://login.tailscale.com/admin/machines" target="_blank" rel="noreferrer">Tailscale admin console</a>.</p></li>
      </ol>
    );
  } else if (method === "zerotier") {
    steps = vlanSteps("ZeroTier", "zerotier.com", "https://www.zerotier.com/download/", null,
      "Create a free network at my.zerotier.com, join it from the ZeroTier app with its Network ID, and send friends the ID. Approve them in the network's member list when they join.");
  } else if (method === "radmin") {
    steps = vlanSteps("Radmin VPN", "radmin-vpn.com", "https://www.radmin-vpn.com/",
      <b>Radmin VPN only runs on Windows, so it won't work from a Mac.</b>,
      "In Radmin VPN, choose Network → Create network, give it a name and password, and send both to your friends so they can join.");
  } else if (method === "hamachi") {
    steps = vlanSteps("Hamachi", "vpn.net", "https://vpn.net/", <span className="muted">The free plan allows 5 people per network.</span>,
      "In Hamachi, create a new network with an ID and password, and send both to your friends so they can join it.");
  }

  const text = address ? inviteText(serverName, password, address, intro, lanTab) : "";

  return (
    <div className="stack">
      <SectionHead title="Play with friends" note={server ? `Friends join ${server.name}. They need Counter-Strike on Steam. Pick how they connect.` : "They need Counter-Strike on Steam. Pick how they connect."} />
      {lanOnly && method !== "lan" && (
        <div className="banner"><div>This server only lets in people on your network. To use this, change "Who can join" in the server's Overview.</div></div>
      )}
      <div className="methods" role="radiogroup" aria-label="How friends connect">
        {METHODS.map((m) => (
          <button key={m.id} role="radio" aria-checked={method === m.id} className="method" onClick={() => setMethod(m.id)}>
            <span className="method-name">{m.name}{m.tag && <i>{m.tag}</i>}</span>
            <span className="method-blurb">{m.blurb}</span>
          </button>
        ))}
      </div>
      <div className="friends">
        <div>{steps}</div>
        <div className="stack">
          <form
            className="panel"
            onSubmit={async (e) => {
              e.preventDefault();
              const p = password.trim();
              if (!/^[A-Za-z0-9_-]{0,24}$/.test(p)) return toast("Passwords can use letters, numbers, - and _.", true);
              const r = await act("server", { name: serverName.trim(), password: p });
              if (r) toast(data ? "Saved and applied to the game" : "Saved. It applies when you start a game.");
            }}
          >
            <h3>Your server</h3>
            <label className="field">
              <span>Server name</span>
              <input className="search" maxLength={48} placeholder="e.g. Friday night frags" value={serverName} onChange={(e) => setName(e.target.value)} />
            </label>
            <label className="field">
              <span>Password</span>
              <div className="row">
                <input className="search" maxLength={24} placeholder="No password" spellCheck={false} value={password} onChange={(e) => setPw(e.target.value)} />
                <button type="button" className="btn" onClick={() => {
                  const w = ["alpha", "bravo", "delta", "echo", "foxtrot", "kilo", "lima", "oscar", "sierra", "tango", "victor", "zulu"];
                  setPw(`${w[Math.floor(Math.random() * w.length)]}-${100 + Math.floor(Math.random() * 900)}`);
                }}>Make one</button>
              </div>
              <small>Letters, numbers, - and _. Leave it empty to let in anyone who has the address.</small>
            </label>
            <button className="btn primary" type="submit">Save</button>
          </form>

          {live && address && (
            <div className="panel">
              <span className={`live ${data ? "on" : ""}`}>{data ? "Friends can join now" : "Start the game and friends can join"}</span>
              <div className="address">{address}</div>
              {full && st && (
                <div className="banner">
                  <div>The server is full of bots, so there's no room for friends.</div>
                  <button className="btn primary" onClick={() => run("gm_bots", ["count", String(Math.max(0, Math.min(st.bots, st.maxplayers - st.humans - 2)))])}>Make room</button>
                </div>
              )}
              {friends.length > 0 && <p>In your game: {friends.join(", ")}</p>}
              <h3>Send this to your friends</h3>
              <textarea className="invite" readOnly value={text} />
              <div className="row">
                <button className="btn primary" onClick={() => copy(text, toast)}>Copy message</button>
                <button className="btn" onClick={() => copy(address, toast)}>Copy address</button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
