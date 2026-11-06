/** How to run a server on a rented Linux machine (a VPS), condensed from docs/linux-server.md. */

const GUIDE = "https://github.com/jamesontucker/Loner-Strike/blob/main/docs/linux-server.md";

const PROVIDERS = [
  {
    name: "Oracle Cloud",
    user: "ubuntu",
    body: "Shape: AMD, VM.Standard.E2.1.Micro for the free tier, not Ampere. If the instance has no public IP, add an ephemeral public IP on its VNIC (Attached VNICs, IPv4 addresses, edit). Firewall: the subnet's security list. \"Out of capacity\" means no free machines there right now: try another availability domain, another shape, or later.",
  },
  {
    name: "Hetzner Cloud",
    user: "root",
    body: "Shared vCPU x86 (CX or CPX), not Arm (CAX). Keep Public IPv4 on. No firewall by default; if you attach one, allow inbound UDP 27015-27020.",
  },
  {
    name: "DigitalOcean",
    user: "root",
    body: "Any Basic droplet with Ubuntu. No firewall by default; if you use one (Networking, Firewalls), allow inbound UDP 27015-27020.",
  },
  {
    name: "Vultr",
    user: "root",
    body: "Cloud Compute, an Intel or AMD plan. No firewall by default; if the server is in a firewall group, allow inbound UDP 27015-27020.",
  },
  {
    name: "AWS Lightsail",
    user: "ubuntu",
    body: "Linux/Unix, OS only, Ubuntu. Upload your key under Change SSH key pair. Firewall: the instance's Networking tab, IPv4 firewall (Custom, UDP, 27015-27020). Attach a static IP so the address doesn't change.",
  },
];

const TROUBLE = [
  { q: "\"Failed to contact game server\"", a: "The provider's firewall doesn't allow UDP 27015-27020 inbound, or the server is stopped." },
  { q: "Couldn't reach it on port 22", a: "The machine is off, the address is wrong, or (on Oracle) it has no public IP." },
  { q: "The server didn't accept the key", a: "Wrong user (ubuntu or root), the key wasn't added when the machine was made, or you picked the .pub file." },
  { q: "The key has a passphrase", a: "Not supported yet. Make a key without one and add its .pub line to ~/.ssh/authorized_keys on the machine." },
  { q: "The server's identity has changed", a: "Expected if you rebuilt the machine: run ssh-keygen -R with its address, then delete the server here and add it again. Otherwise, don't connect." },
  { q: "The machine is aarch64", a: "It's ARM. Make an x86 (AMD or Intel) one." },
  { q: "Setup stopped part way", a: "Try again carries on where it stopped. Rebuild from scratch wipes the server's folder on the machine and starts over." },
];

export function HostingGuide() {
  return (
    <section className="stack-tight">
      <h3>Run a server on a VPS</h3>
      <p className="muted measure">
        Loner-Strike can set up an always-on server on a Linux machine you rent. It connects over SSH, installs Valve's server and Loner-Strike there and runs it as a
        service that keeps going when your computer is off. You need Ubuntu or Debian on an x86 (AMD or Intel) machine with a public IPv4 address. ARM machines (Oracle
        Ampere, AWS Graviton, Hetzner CAX, Raspberry Pi) can't run the game server.
      </p>

      <div className="panel">
        <h4>1. Choose a host and size</h4>
        <table className="sizing">
          <thead>
            <tr><th>Players (bots included)</th><th>CPU</th><th>Memory</th></tr>
          </thead>
          <tbody>
            <tr><td>Up to about 10, a few bots</td><td>1 vCPU</td><td>1 GB</td></tr>
            <tr><td>20 to 32, with bots</td><td>2 vCPU</td><td>2 to 4 GB</td></tr>
          </tbody>
        </table>
        <p className="muted measure">
          Free: Oracle Cloud's Always Free AMD machines (VM.Standard.E2.1.Micro), when Oracle has capacity, or Oracle's trial credits. Cheap: Hetzner, Vultr,
          DigitalOcean, OVH and Linode have x86 plans at around $4 to $7 a month. Pick a location near your players.
        </p>
      </div>

      <div className="panel">
        <h4>2. Make an SSH key</h4>
        <p className="muted">Mac, in Terminal:</p>
        <p><code>mkdir -p ~/.ssh</code></p>
        <p><code>ssh-keygen -t ed25519 -f ~/.ssh/loner-strike -N "" -C loner-strike</code></p>
        <p><code>cat ~/.ssh/loner-strike.pub</code></p>
        <p className="muted">Windows 10 or 11, in PowerShell (press Enter twice when it asks for a passphrase):</p>
        <p><code>mkdir $HOME\.ssh -Force</code></p>
        <p><code>ssh-keygen -t ed25519 -f $HOME\.ssh\loner-strike -C loner-strike</code></p>
        <p><code>Get-Content $HOME\.ssh\loner-strike.pub</code></p>
        <p className="muted measure">
          The line starting with <code>ssh-ed25519</code> is the public key; you paste it into your provider. Keys with a passphrase aren't supported yet. A PuTTY key
          (.ppk) needs exporting in PuTTYgen first: Conversions, Export OpenSSH key.
        </p>
      </div>

      <div className="panel">
        <h4>3. Create the machine</h4>
        <ol className="bring">
          <li>Make a new server (instance, droplet) with Ubuntu 24.04 or 22.04 on an x86 plan.</li>
          <li>Make sure it gets a public IPv4 address.</li>
          <li>Under SSH keys, paste the public key.</li>
          <li>Once it's running, note its public IP and its SSH user.</li>
        </ol>
        <div className="host-grid">
          {PROVIDERS.map((p) => (
            <div key={p.name}>
              <p><b>{p.name}</b> <span className="muted">· user {p.user}</span></p>
              <p className="muted small-note">{p.body}</p>
            </div>
          ))}
        </div>
      </div>

      <div className="panel">
        <h4>4. Open UDP 27015-27020</h4>
        <p className="muted measure">
          The app opens the ports in the machine's own firewall during setup. It can't reach your provider's firewall (security list, security group, firewall group),
          so add an inbound rule there: protocol UDP, ports 27015-27020, source anywhere (<code>0.0.0.0/0</code>). Leave SSH (TCP 22) open; the app uses it for
          everything else.
        </p>
      </div>

      <div className="panel">
        <h4>5. Add it here</h4>
        <ol className="bring">
          <li>New server, and pick A Linux server (VPS).</li>
          <li>Address: the public IP. User: <code>ubuntu</code> or <code>root</code>.</li>
          <li>SSH key: Browse to the private key, the file without .pub. On a Mac, press Cmd+Shift+G in the picker and type <code>~/.ssh</code>.</li>
          <li>
            Check the server. It shows the machine's fingerprint and whether it matches your computer's known_hosts. For a new machine, compare it with what{" "}
            <code>ssh-keygen -lf /etc/ssh/ssh_host_ed25519_key.pub</code> shows in your provider's web console.
          </li>
          <li>Pick a name, the game, and Set it up like: My game or Start fresh.</li>
          <li>Who can join: Listed on Steam shows it in the server browser; Anyone with the address keeps it unlisted. A join password keeps strangers out either way.</li>
          <li>Make the server.</li>
        </ol>
        <p className="muted measure">
          Setup takes 5 to 15 minutes. The server runs on UDP 27015 as a service called <code>lonerstrike-&lt;name&gt;</code>, keeps running with the app closed and
          restarts after a crash or a reboot.
        </p>
      </div>

      <div className="panel">
        <h4>6. Join and manage it</h4>
        <p className="muted measure">
          Join, Start, Stop and Open are in the server list. Join on a listed server works with the game open; on an unlisted one it restarts the game to connect. Friends
          use <code>connect &lt;address&gt;:27015</code> in the game console. The Overview tab has the server's console.
        </p>
        <p className="muted measure">
          Over SSH, <code>lsctl list</code>, <code>lsctl status</code>, <code>lsctl start</code>, <code>lsctl stop</code>, <code>lsctl restart</code> and{" "}
          <code>lsctl logs</code> manage it. Keep the machine updated with <code>sudo apt update &amp;&amp; sudo apt upgrade</code>; the server comes back by itself after
          a reboot.
        </p>
      </div>

      <div className="panel">
        <h4>Troubleshooting</h4>
        {TROUBLE.map((t) => (
          <p key={t.q} className="measure">
            <b>{t.q}.</b> <span className="muted">{t.a}</span>
          </p>
        ))}
      </div>

      <div className="panel">
        <h4>Removing it</h4>
        <p className="muted measure">
          Delete the server here; that stops the service and removes its folder on the machine. Then delete the machine at your provider so it stops costing money.
        </p>
        <p className="muted small-note">
          The full guide is on{" "}
          <a href={GUIDE} target="_blank" rel="noreferrer">
            GitHub
          </a>
          .
        </p>
      </div>
    </section>
  );
}
