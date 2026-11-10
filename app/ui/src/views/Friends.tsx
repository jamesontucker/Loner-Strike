import { useEffect, useState, type ReactNode } from "react";
import { copyText, getJson, HttpError, isAbort, ON_THIS_COMPUTER, postJson, targetKey, useActions, usePoll } from "../api";
import { useServers } from "../queries";
import { useNav } from "../store";
import { serverAddress, serverHost, type Online } from "../types";
import { BusyButton, Info, LoadError, SectionHead, Skeleton, useCommand, useConfirm, useToast } from "../ui";

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
  { id: "lan", name: "This network", blurb: "Friends in the same room or on the same Wi-Fi", tag: "LAN party" },
  { id: "direct", name: "Direct", blurb: "Your router opens the port itself. Best ping, nothing to install", tag: "UPnP" },
  { id: "playit", name: "playit.gg", blurb: "A free relay. Friends install nothing" },
  { id: "tailscale", name: "Tailscale", blurb: "A private network everyone joins" },
  { id: "zerotier", name: "ZeroTier", blurb: "A virtual LAN everyone joins" },
  { id: "radmin", name: "Radmin VPN", blurb: "A virtual LAN for gamers", tag: "Windows only" },
  { id: "hamachi", name: "Hamachi", blurb: "An older virtual LAN. Up to 5 people free" },
];

/** The computer running Loner-Strike, for steps that differ on a Mac and on Windows
 *  (null on a phone, where it isn't known). */
const OS: "mac" | "windows" | null = !ON_THIS_COMPUTER ? null : /Mac/.test(navigator.platform) ? "mac" : /Win/.test(navigator.platform) ? "windows" : null;

const GAME = "Counter-Strike";

/** What the top bar says while a sharing action runs. */
const SHARE_LABEL: Record<string, string> = {
  connect: "Connecting to playit.gg…",
  disconnect: "Disconnecting from playit.gg…",
  cancel: "Cancelling…",
  start: "Starting sharing…",
  stop: "Stopping sharing…",
  server: "Saving your server…",
  method: "Switching how friends connect…",
};

/** What each virtual LAN is, for its "?" */
const VLAN_INFO: Record<"zerotier" | "radmin" | "hamachi", string> = {
  zerotier: "ZeroTier links your computers into one private network over the internet, so your game shows up in friends' LAN tab as if you were in the same room. Everyone installs it and joins your network.",
  radmin: "Radmin VPN is a free virtual LAN for Windows, popular for old games. Everyone installs it and joins your network, then your game shows up in their LAN tab.",
  hamachi: "Hamachi is an older virtual LAN that many people still use for games. Everyone joins your network by its ID and password; free networks hold 5 people.",
};

function inviteText(name: string, pw: string, addr: string, intro: string[], lanTab: boolean) {
  return [
    `Join my ${GAME} game${name ? ` "${name}"` : ""}.`,
    ...(intro.length ? ["", ...intro] : []),
    "",
    ...(lanTab ? ["In the game, open Find Servers, then LAN, and join it. Or open the console (~) and type:"] : [
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
  if (await copyText(text)) toast("Copied");
  else toast("Couldn't copy. Select the text and copy it yourself.", true);
}

const NO_ADDRESS = "Couldn't find this computer's network address.";

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
  const [lanErr, setLanErr] = useState(false);
  const [direct, setDirect] = useState<Direct | null>(null);
  const [directErr, setDirectErr] = useState(false);
  // the first load of sharing failed (a later miss keeps what's shown)
  const [loadErr, setLoadErr] = useState(false);
  const { run: once, busy: busyKey } = useActions();
  const key = (what: string) => `friends:${targetKey()}:${what}`;
  const busy = (what: string) => busyKey(key(what));
  const directBusy = busy("direct");
  const [vlans, setVlans] = useState<VLans>({});
  // a server on a rented machine: friends connect straight to its address
  const open = useNav((n) => n.server);
  const servers = useServers().data;
  const mine = server && open ? servers?.servers.find((x) => x.id === open) : undefined;
  const vps = mine?.host ? mine : undefined;

  useEffect(() => {
    getJson<{ ip: string | null; port: number }>("/api/app/lan")
      .then((r) => {
        setLanAddr(r.ip ? `${r.ip}:${r.port}` : null);
        setLanErr(!r.ip);
      })
      .catch((e) => !isAbort(e) && setLanErr(true));
    getJson<Direct>("/api/app/direct").then((d) => (setDirect(d), setDirectErr(false))).catch((e) => !isAbort(e) && setDirectErr(true));
  }, []);
  const [tries, setTries] = useState(0);
  usePoll(() =>
    Promise.all([
      getJson<Share>("/api/share")
        .then((s) => ("phase" in s ? (setSh(s), setLoadErr(false)) : setMissing(true)))
        // an app without sharing at all; otherwise a missed poll keeps what's shown
        .catch((e: unknown) => {
          if (e instanceof HttpError && e.status === 404) setMissing(true);
          else if (!isAbort(e)) setLoadErr(true);
        }),
      getJson<VLans>("/api/app/vlans").then(setVlans).catch(() => setVlans({})),
    ]), 3000, [tries]);

  const act = async (action: string, extra: Record<string, unknown> = {}) => {
    let out: Share | null = null;
    await once(key(action), async () => {
      const r = await postJson<Share & { error?: string }>("/api/share", { action, ...extra }).catch(() => null);
      if (!r || (r.ok === false && r.error)) return toast(r?.error || "Couldn't change sharing. Try again.", true);
      setSh(r);
      out = r;
    }, SHARE_LABEL[action] ?? "Working…");
    return out as Share | null;
  };
  const setMethod = (m: Method) => {
    setMethodRaw(m);
    try { localStorage.setItem("ls.method", m); } catch { /* private window */ }
    if (m === "playit" || m === "tailscale") act("method", { method: m });
  };
  const setDirectOn = (on: boolean) => once(key("direct"), async () => {
    const r = await postJson<Direct>("/api/app/direct", { on }).catch(() => null);
    if (!r) return toast("Couldn't reach the app.", true);
    setDirect(r);
    if (r.error) toast(r.error, true);
  }, on ? "Asking your router to open the port…" : "Closing the port on your router…");

  if (missing) return <div className="stack"><SectionHead title="Play with friends" /><p className="muted">Sharing isn't in this version of the app yet.</p></div>;
  if (!sh)
    return (
      <div className="stack">
        <SectionHead title="Play with friends" />
        {loadErr ? <LoadError text="Couldn't load how friends connect." onRetry={() => (setLoadErr(false), setTries((n) => n + 1))} /> : <Skeleton />}
      </div>
    );

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
          <strong>Install {app}<Info text={VLAN_INFO[method as "zerotier" | "radmin" | "hamachi"]} /></strong>
          <p>{ip ? "Running." : <><a href={url} target="_blank" rel="noreferrer">Download {app}</a> (free) on this computer and every friend's. {extra}</>}</p>
        </li>
        <li className={ip ? "done" : "now"}>
          <strong>Make a network and share it</strong>
          <p>{idHint}</p>
        </li>
        <li className={ip ? "now" : ""}>
          <strong>Start your game</strong>
          <p>On the same {app} network, friends see your server in the <b>LAN</b> tab of <b>Find Servers</b>, or use the address below.</p>
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
        <li className={lanErr ? "now" : "done"}><strong>Get everyone on the same network</strong><p>Same Wi-Fi, or plugged into the same router.</p>{lanErr && <p className="err-text">{NO_ADDRESS}</p>}</li>
        <li className={data ? "done" : "now"}><strong>{server ? "Start the server" : "Start your game"}</strong>{OS === "mac" ? <p>The first time a friend joins, your Mac may ask whether to allow incoming connections for the game (CrossOver): click Allow.</p> : OS === "windows" ? <p>The first time, Windows may ask whether to let the game through its firewall: allow it on private networks. If friends still can't see it, Windows may think this Wi-Fi is public: in Settings, Network & internet, open your Wi-Fi and pick Private network.</p> : null}</li>
        <li className={data ? "now" : ""}><strong>Friends join from the LAN tab</strong><p>In Counter-Strike, open <b>Find Servers</b> and the <b>LAN</b> tab, and your server is there{serverName ? ` as "${serverName}"` : ""}.</p></li>
      </ol>
    );
  } else if (method === "direct") {
    // a server with a public address (a home server's dynamic DNS name, say) shares that
    address = directHere && direct?.address ? (mine?.public_address ? serverAddress(mine) : direct.address) : "";
    // a shared address (carrier-grade NAT) can't be reached from outside, open port or not
    live = !!address && !direct?.shared_address;
    steps = (
      <ol className="steps">
        <li className={direct?.on ? "done" : "now"}>
          <strong>Open the port on your router<Info text="Friends connect straight to your computer, so ping is as low as it gets. Needs UPnP switched on in your router, and doesn't work if your internet provider shares one address between customers." /></strong>
          <p>Loner-Strike asks your router to send game traffic to this computer (UPnP), and closes it again when you stop or quit.</p>
          {directElsewhere && <p className="muted">Direct is open for port {direct?.port} right now. Starting it here moves it to this {server ? "server" : "game"}.</p>}
          {directHere ? (
            <BusyButton className="btn danger" busy={directBusy} label="Stop" busyLabel="Stopping…" onClick={() => setDirectOn(false)} />
          ) : (
            <BusyButton className="btn primary" busy={directBusy} label="Start" busyLabel="Asking your router…" onClick={() => setDirectOn(true)} />
          )}
          {direct?.error && <p className="err-text">{direct.error}</p>}
          {directErr && !direct && <p className="err-text">{NO_ADDRESS}</p>}
          {direct?.shared_address && (
            <p className="err-text">Your internet connection shares its address with other customers (common with mobile and some fibre providers), so friends can't reach you directly. Use playit.gg instead.</p>
          )}
        </li>
        <li className={live ? "now" : ""}>
          <strong>Send friends your address</strong>
          <p>It's your home's internet address, so only share it with friends. Set a join password under Your server.</p>
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
          <strong>Make a free playit.gg account<Info text="playit.gg passes game traffic through its own servers, so you don't touch your router and friends install nothing. Ping is a little higher than Direct, depending on how far the relay is." /></strong>
          <p>{sh.connected ? "Done." : <><a href="https://playit.gg/login/create" target="_blank" rel="noreferrer">Sign up at playit.gg</a>, then confirm the email they send you. The free plan is enough.</>}</p>
        </li>
        <li className={sh.connected ? "done" : "now"}>
          <strong>Connect this computer to your account</strong>
          {sh.connected ? (
            <p>Done. <BusyButton className="btn small" busy={busy("disconnect")} label="Disconnect" busyLabel="Disconnecting…" onClick={async () => (await ask("Disconnect from playit.gg? You'll need to approve it again.", { ok: "Disconnect", danger: true })) && act("disconnect")} /></p>
          ) : sh.claim?.step === "failed" ? (
            <><p className="err-text">{sh.claim.error}</p><BusyButton className="btn primary" busy={busy("connect")} label="Try again" busyLabel="Connecting…" onClick={() => act("connect")} /></>
          ) : sh.claim ? (
            <><p><a href={sh.claim.url} target="_blank" rel="noreferrer">Open playit.gg to approve this computer</a>, then come back. {sh.claim.step === "approve" ? <><span className="spinner" aria-hidden="true" />Waiting for you to approve…</> : "Waiting for the page to open…"}</p><BusyButton busy={busy("cancel")} label="Cancel" busyLabel="Cancelling…" onClick={() => act("cancel")} /></>
          ) : (
            <BusyButton className="btn primary" busy={busy("connect")} label="Connect to playit.gg" busyLabel="Connecting…" onClick={async () => { const r = await act("connect"); if (r?.claim) window.open(r.claim.url, "_blank"); }} />
          )}
        </li>
        <li className={isLive ? "done" : sh.connected ? "now" : ""}>
          <strong>Share the game</strong>
          <p>{sh.connected ? "Sharing runs only while Loner-Strike is open." : "Available once this computer is connected."}</p>
          {playitElsewhere && <p className="muted">playit.gg is sharing {server ? "your game or another server" : "a server"} right now. Starting it here moves it.</p>}
          {sh.connected && (isLive || (sh.phase === "starting" && !playitElsewhere)
            ? <BusyButton className="btn danger" busy={busy("stop")} disabled={busy("start")} label="Stop sharing" busyLabel="Stopping…" onClick={() => act("stop")} />
            : <BusyButton className="btn primary" busy={busy("start")} disabled={busy("stop")} label="Start sharing" busyLabel="Starting…" onClick={() => act("start")} />)}
          {/* still picking an address while starting is a wait, not a fault (sharing.rs read_tunnel) */}
          {sh.error && <p className={sh.phase === "starting" && sh.error.startsWith("playit.gg is still picking") ? "muted" : "err-text"}>{sh.error}</p>}
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
        <li className={ts.installed ? "done" : "now"}><strong>Install Tailscale on this computer<Info text="Tailscale is a private network for your devices and the friends you invite. Only people on it can reach your game; each friend installs Tailscale too." /></strong><p>{ts.installed ? "Installed." : <a href="https://tailscale.com/download" target="_blank" rel="noreferrer">Download Tailscale (free) and sign in.</a>}</p></li>
        <li className={running ? "done" : ts.installed ? "now" : ""}><strong>Turn it on</strong><p>{running ? `Connected as ${ts.name || ts.ip}.` : ts.installed ? `Open the Tailscale app${OS === "mac" ? " from the menu bar" : OS === "windows" ? " from the system tray" : ""} and connect.` : "After installing."}</p></li>
        <li className={running ? "now" : ""}><strong>Let your friend in</strong><p>Your friend installs Tailscale too. Then share this computer with them, or invite them to your network, from the <a href="https://login.tailscale.com/admin/machines" target="_blank" rel="noreferrer">Tailscale admin console</a>.</p></li>
      </ol>
    );
  } else if (method === "zerotier") {
    steps = vlanSteps("ZeroTier", "zerotier.com", "https://www.zerotier.com/download/", null,
      "Create a free network at my.zerotier.com, join it from the ZeroTier app with its Network ID, and send friends the ID. Approve them in the network's member list when they join.");
  } else if (method === "radmin") {
    steps = vlanSteps("Radmin VPN", "radmin-vpn.com", "https://www.radmin-vpn.com/",
      OS === "mac" ? <b>Radmin VPN only runs on Windows, so it won't work from a Mac.</b> : null,
      "In Radmin VPN, choose Create network from the Network menu, give it a name and password, and send both to your friends so they can join.");
  } else if (method === "hamachi") {
    steps = vlanSteps("Hamachi", "vpn.net", "https://vpn.net/", <span className="muted">The free plan allows 5 people per network.</span>,
      "In Hamachi, create a new network with an ID and password, and send both to your friends so they can join it.");
  }

  const text = address ? inviteText(serverName, password, address, intro, lanTab) : "";

  // the name and join password, the same for every way of connecting
  const yourServer = (
    <form
      className="panel"
      onSubmit={async (e) => {
        e.preventDefault();
        if (busy("server")) return;
        const p = password.trim();
        if (!/^[A-Za-z0-9_-]{0,24}$/.test(p)) return toast("Passwords can use letters, numbers, - and _.", true);
        const r = await act("server", { name: serverName.trim(), password: p });
        if (!r) return;
        // what the app saved shows from now on (it may have tidied it)
        setName(null);
        setPw(null);
        toast(server ? (data ? `Saved for ${server.name}` : "Saved. It applies when the server starts.") : data ? "Saved and applied to the game" : "Saved. It applies when you start a game.");
      }}
    >
      <h3>Your server</h3>
      <label className="field">
        <span>Server name<Info text="Shown in the server browser and on the scoreboard." /></span>
        <input className="search" maxLength={48} placeholder="Like Friday night frags" value={serverName} onChange={(e) => setName(e.target.value)} />
      </label>
      <label className="field">
        <span>Join password<Info text="Everyone who joins needs this, however they connect. The invite message includes it." /></span>
        <div className="row">
          <input className="search" maxLength={24} placeholder="No password" spellCheck={false} value={password} onChange={(e) => setPw(e.target.value)} />
          <button type="button" className="btn" onClick={() => {
            const w = ["alpha", "bravo", "delta", "echo", "foxtrot", "kilo", "lima", "oscar", "sierra", "tango", "victor", "zulu"];
            setPw(`${w[Math.floor(Math.random() * w.length)]}-${100 + Math.floor(Math.random() * 900)}`);
          }}>Make one</button>
        </div>
        <small>Letters, numbers, - and _. Leave it empty to let in anyone who has the address.</small>
      </label>
      <BusyButton className="btn primary" type="submit" busy={busy("server")} label="Save" busyLabel="Saving…" />
    </form>
  );

  if (vps) {
    const at = serverAddress(vps);
    const invite = inviteText(serverName, password, at, [], false);
    return (
      <div className="stack">
        <SectionHead title="Play with friends" note={`${vps.name} runs on ${serverHost(vps)}, so friends connect straight to it. They need Counter-Strike on Steam.`} />
        <div className="friends">
          <div className="panel">
            <span className={`live ${vps.state === "running" ? "on" : ""}`}>{vps.state === "running" ? "Friends can join now" : "Start the server and friends can join"}</span>
            <div className="address">{at}</div>
            <div className="row">
              <button className="btn" onClick={() => copy(at, toast)}>Copy address</button>
            </div>
            <p className="muted small-note">
              If friends can't connect, check UDP port {vps.port} is open in your provider's firewall (security list, security group or firewall group). Setup opened
              it on the machine itself, but not at the provider.
            </p>
            <h3>Send this to your friends</h3>
            <textarea className="invite" readOnly value={invite} />
            <div className="row">
              <button className="btn primary" onClick={() => copy(invite, toast)}>Copy message</button>
            </div>
          </div>
          <div className="stack">{yourServer}</div>
        </div>
      </div>
    );
  }

  return (
    <div className="stack">
      <SectionHead title="Play with friends" note={server ? `Friends join ${server.name}. They need Counter-Strike on Steam. Pick how they connect.` : "They need Counter-Strike on Steam. Pick how they connect."} />
      {lanOnly && method !== "lan" && (
        <div className="banner"><div>This server only lets in people on your network. To use this, change "Who can join" in the server's Overview.</div></div>
      )}
      <div className="methods" role="radiogroup" aria-label="How friends connect">
        {METHODS.map((m) => (
          <button key={m.id} role="radio" aria-checked={method === m.id} className="method" onClick={() => setMethod(m.id)}>
            <span className="method-name">{m.name}{m.tag && !(m.id === "radmin" && OS === "windows") && <i>{m.tag}</i>}</span>
            <span className="method-blurb">{m.blurb}</span>
          </button>
        ))}
      </div>
      <div className="friends">
        <div>
          {steps}
          <p className="muted small-note">
            Friends can't get in? <a href="https://lonerstrike.com/docs#friends-cant-join" target="_blank" rel="noreferrer">What to check</a>.
          </p>
        </div>
        <div className="stack">
          {yourServer}

          {live && address && (
            <div className="panel">
              <span className={`live ${data ? "on" : ""}`}>{data ? "Friends can join now" : "Start the game and friends can join"}</span>
              <div className="address">{address}</div>
              {full && st && st.bots === 0 && (
                <div className="banner">
                  <div>{server ? "The server is full, so there's no room for friends. Add more Player slots on its Overview tab." : "Your game is full, so there's no room for friends."}</div>
                </div>
              )}
              {full && st && st.bots > 0 && (
                <div className="banner">
                  <div>The server is full of bots, so there's no room for friends.</div>
                  <BusyButton
                    className="btn primary"
                    busy={busy("make-room")}
                    label="Make room"
                    busyLabel="Making room…"
                    onClick={() => once(key("make-room"), () => run("gm_bots", ["count", String(Math.max(0, Math.min(st.bots, st.maxplayers - st.humans - 2)))]), "Kicking bots to make room…")}
                  />
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
          {!(live && address) && (
            <div className="panel">
              <h3>Send this to your friends</h3>
              <p className="muted">Your invite shows here once friends can connect.</p>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
